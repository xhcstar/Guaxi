//! 插件专用的本地模型补全接口。
//!
//! 插件跑在 Python worker 里，拿不到设置页里加密保存的 API Key，也不该拿到。
//! 这里把「用哪个供应商、密钥是什么」留在 Rust 侧，只向 loopback 暴露一个
//! `POST /api/plugin/llm`，请求体是纯文本 prompt，响应是 `{success, text}`。

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

use crate::commands::llm_provider::{self, ResolvedProvider};

#[derive(Debug, Deserialize)]
pub struct PluginLlmRequest {
    pub prompt: String,
    #[serde(default)]
    pub system: String,
    #[serde(default)]
    pub model_id: Option<i32>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// 要求模型只输出 JSON（结构化编排用）。部分代理不支持时会自动去掉后重试。
    #[serde(default)]
    pub json_mode: bool,
    /// 关闭思考型模型的 reasoning（否则 max_tokens 会被思考过程吃光，正文为空）。
    #[serde(default)]
    pub disable_reasoning: bool,
}

#[derive(Debug, Serialize)]
pub struct PluginLlmResponse {
    pub success: bool,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

/// 未指定 model_id 时的默认顺序：cc-switch → 自定义 → 内置（DeepSeek 优先）。
/// 任一来源解析失败就继续试下一个，全都没有密钥时给出可读错误。
fn resolve_provider(model_id: Option<i32>) -> Result<ResolvedProvider, String> {
    if let Some(id) = model_id {
        return llm_provider::resolve_chat(id);
    }
    const ORDER: &[i32] = &[11, 10, 1, 0, 2, 3, 4, 5];
    let mut last_error = String::new();
    for id in ORDER {
        match llm_provider::resolve_chat(*id) {
            Ok(p) => return Ok(p),
            Err(e) => last_error = e,
        }
    }
    Err(format!(
        "没有可用的模型供应商，请在设置 → 大模型中填写 API Key 或启用 cc-switch 代理（{last_error}）"
    ))
}

// 只服务于下面的单元测试：生产路径直接用 extract_text_ex 拿 (文本, 是否 reasoning)。
#[cfg(test)]
fn extract_text(v: &Value) -> Option<String> {
    extract_text_ex(v).map(|(text, _)| text)
}

/// 返回 (文本, 是否只来自 reasoning_content)。
fn extract_text_ex(v: &Value) -> Option<(String, bool)> {
    let message = v.get("choices")?.get(0)?.get("message")?;
    if let Some(s) = message.get("content").and_then(|c| c.as_str()) {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return Some((trimmed.to_string(), false));
        }
    }
    if let Some(parts) = message.get("content").and_then(|c| c.as_array()) {
        let joined: String = parts
            .iter()
            .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
            .collect();
        if !joined.trim().is_empty() {
            return Some((joined.trim().to_string(), false));
        }
    }
    message
        .get("reasoning_content")
        .and_then(|c| c.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|text| (text, true))
}

async fn send_once(
    client: &reqwest::Client,
    provider: &ResolvedProvider,
    body: &Value,
) -> Result<(reqwest::StatusCode, String), String> {
    let mut builder = client.post(&provider.endpoint).json(body);
    if !provider.api_key.is_empty() {
        builder = builder.bearer_auth(&provider.api_key);
    }
    let resp = builder
        .send()
        .await
        .map_err(|e| format!("请求模型失败: {e}"))?;
    let status = resp.status();
    let raw = resp.text().await.map_err(|e| e.to_string())?;
    Ok((status, raw))
}

pub async fn complete(req: PluginLlmRequest) -> Result<PluginLlmResponse, String> {
    if req.prompt.trim().is_empty() {
        return Err("prompt 不能为空".to_string());
    }
    let provider = resolve_provider(req.model_id)?;

    let mut messages = Vec::new();
    if !req.system.trim().is_empty() {
        messages.push(json!({ "role": "system", "content": req.system }));
    }
    messages.push(json!({ "role": "user", "content": req.prompt }));

    let mut body = json!({
        "model": provider.model,
        "messages": messages,
        "stream": false,
        "temperature": req.temperature.unwrap_or(0.3),
        "max_tokens": req.max_tokens.unwrap_or(4096),
    });
    if req.json_mode {
        body["response_format"] = json!({ "type": "json_object" });
    }
    if req.disable_reasoning {
        // 兼容两家写法：DeepSeek/cc-switch 认 thinking，部分代理认 reasoning_effort。
        body["thinking"] = json!({ "type": "disabled" });
        body["reasoning_effort"] = json!("none");
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;
    let (mut status, mut raw) = send_once(&client, &provider, &body).await?;
    if !status.is_success() && req.disable_reasoning {
        // 不认识 thinking 参数的代理：去掉后重试。
        if let Some(obj) = body.as_object_mut() {
            obj.remove("thinking");
            obj.remove("reasoning_effort");
        }
        let retry = send_once(&client, &provider, &body).await?;
        status = retry.0;
        raw = retry.1;
    }
    if !status.is_success() && req.json_mode {
        // 一些代理/模型不支持 response_format，去掉后重试一次。
        if let Some(obj) = body.as_object_mut() {
            obj.remove("response_format");
        }
        let retry = send_once(&client, &provider, &body).await?;
        status = retry.0;
        raw = retry.1;
    }
    if !status.is_success() {
        let brief: String = raw.chars().take(300).collect();
        return Err(format!("模型返回 {}: {brief}", status.as_u16()));
    }
    let parsed: Value =
        serde_json::from_str(&raw).map_err(|e| format!("解析模型响应失败: {e}"))?;
    let (text, from_reasoning) =
        extract_text_ex(&parsed).ok_or_else(|| "模型响应里没有文本内容".to_string())?;
    if req.json_mode && from_reasoning {
        // 只拿到思考过程时不能当成结果，否则前端会解析失败并误以为是配置问题。
        return Err("模型只返回了思考过程，没有给出正文 JSON。请换一个支持 JSON 输出的模型，或在设置里提高 max tokens。".to_string());
    }
    if req.json_mode {
        let head = text.trim_start();
        if !(head.starts_with('{') || head.starts_with('[') || head.starts_with("```")) {
            let preview: String = head.chars().take(120).collect();
            tracing::warn!(
                provider = %provider.label,
                text_head = %preview,
                "模型没有按 JSON 返回"
            );
        }
    }
    Ok(PluginLlmResponse {
        success: true,
        text,
        message: None,
        provider: Some(provider.label),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_string_and_parts_content() {
        let plain = json!({ "choices": [{ "message": { "content": " hello " } }] });
        assert_eq!(extract_text(&plain).unwrap(), "hello");

        let parts = json!({ "choices": [{ "message": { "content": [
            { "type": "text", "text": "a" },
            { "type": "text", "text": "b" }
        ] } }] });
        assert_eq!(extract_text(&parts).unwrap(), "ab");
    }

    #[test]
    fn empty_content_falls_back_to_reasoning() {
        let v = json!({ "choices": [{ "message": { "content": "", "reasoning_content": "thinking" } }] });
        assert_eq!(extract_text(&v).unwrap(), "thinking");
    }
}
