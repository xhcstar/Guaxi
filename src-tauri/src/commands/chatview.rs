use chrono::Utc;
use futures_util::StreamExt;
use rand::Rng;
use regex::Regex;
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use crate::commands::settingview;
use serde_json::{json, Value};
use std::convert::TryFrom;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tracing::{debug, error, info};
use url::Url;
// use std::error::Error;

enum ChatModel {
    CHATGPT,
    DEEPSEEK,
    QWEN,
    KIMI,
    DOUBAO,
    LOCALQWEN,
    LOCALDEEPSEEK,
    YUANBAO_WEB,
    CHATGPT_WEB,
}

impl TryFrom<i32> for ChatModel {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ChatModel::CHATGPT),
            1 => Ok(ChatModel::DEEPSEEK),
            2 => Ok(ChatModel::QWEN),
            3 => Ok(ChatModel::KIMI),
            4 => Ok(ChatModel::DOUBAO),
            5 => Ok(ChatModel::LOCALQWEN),
            6 => Ok(ChatModel::LOCALDEEPSEEK),
            7 => Ok(ChatModel::YUANBAO_WEB),
            8 => Ok(ChatModel::CHATGPT_WEB),
            _ => Err("Invalid ChatModel ID".to_string()),
        }
    }
}

#[derive(Deserialize)]
pub struct ChatRequest {
    modelid: i32,
    question: String,
    topicid: i32,
    sessionid: String,
}

#[derive(Serialize, Debug)]
pub struct ChatResponse {
    answer: String,
    sessionid: String,
}

#[derive(Deserialize)]
pub struct DeepSeekMessage {
    content: String,
}

#[derive(Deserialize)]
pub struct DeepSeekChoice {
    message: DeepSeekMessage,
}

#[derive(Deserialize)]
pub struct DeepSeekResponse {
    choices: Vec<DeepSeekChoice>,
}

#[derive(Deserialize)]
struct LocalDeepSeekResponse {
    message: DeepSeekMessage,
}

async fn collect_content_from_local_deepseek(data: &str) -> Result<String, String> {
    let mut full_answer = String::new();

    // 分割每一行
    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // 解析每一行的 JSON
        let response: LocalDeepSeekResponse =
            serde_json::from_str(line).map_err(|e| e.to_string())?;

        // 提取 content 字段并拼接
        full_answer.push_str(&response.message.content);
    }

    // 返回完整的回答
    Ok(full_answer)
}

fn stream_extract_content(line: &str) -> Option<String> {
    let mut s = line.trim();
    if s.is_empty() {
        return None;
    }

    if let Some(rest) = s.strip_prefix("data:") {
        s = rest.trim();
    }

    if s.is_empty() || s == "[DONE]" {
        return None;
    }

    let v: Value = serde_json::from_str(s).ok()?;

    if let Some(c) = v.pointer("/message/content").and_then(|v| v.as_str()) {
        if !c.is_empty() {
            return Some(c.to_string());
        }
    }

    if let Some(c) = v
        .pointer("/choices/0/delta/content")
        .and_then(|v| v.as_str())
    {
        if !c.is_empty() {
            return Some(c.to_string());
        }
    }

    if let Some(c) = v
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
    {
        if !c.is_empty() {
            return Some(c.to_string());
        }
    }

    None
}

fn drain_complete_lines(buf: &mut String) -> Vec<String> {
    let mut lines = Vec::new();
    while let Some(idx) = buf.find('\n') {
        let line = buf[..idx].to_string();
        buf.drain(..=idx);
        lines.push(line);
    }
    lines
}

#[derive(Default)]
struct CodeFenceTracker {
    in_fence: bool,
    tick_run: usize,
    last_char: Option<char>,
}

impl CodeFenceTracker {
    fn push(&mut self, s: &str) {
        for ch in s.chars() {
            self.last_char = Some(ch);
            if ch == '`' {
                self.tick_run += 1;
                if self.tick_run == 3 {
                    self.in_fence = !self.in_fence;
                    self.tick_run = 0;
                }
            } else {
                self.tick_run = 0;
            }
        }
    }

    fn closing_delta(&self) -> Option<String> {
        if !self.in_fence {
            return None;
        }

        let needs_newline = match self.last_char {
            Some('\n') | None => false,
            _ => true,
        };

        Some(if needs_newline {
            "\n```".to_string()
        } else {
            "```".to_string()
        })
    }
}

fn generate_session_id() -> String {
    // 获取当前时间戳（秒级）
    let timestamp = Utc::now().timestamp();

    // 生成一个 4 位随机数
    let random_number: u32 = rand::rng().random_range(1000..9999);

    // 将时间戳和随机数组合成 session_id
    format!("session_{}_{}", timestamp, random_number)
}

// 调用 DeepSeek API 获取回答
pub async fn get_answer_from_deepseek(
    question: &str,
    sessionid: &str,
) -> Result<ChatResponse, String> {
    let api_key = settingview::get_llm_config().deepseek_api_key;
    if api_key.trim().is_empty() {
        return Err("DeepSeek API Key 未配置".to_string());
    }

    let url = "https://api.deepseek.com/chat/completions";
    let session_id;

    info!(
        "get_answer_from_deepseek: Received request: {:?} {}",
        question, sessionid
    );

    if sessionid.is_empty() {
        session_id = generate_session_id();
        info!("get_answer_from_deepseek: sessionid is empty, generating a new one.");
    } else {
        session_id = sessionid.to_string();
        info!("get_answer_from_deepseek: session_id: {}", session_id);
    }

    let request_body = json!({
        "model": "deepseek-chat",
        "messages": [{"role": "user", "content": question}],
        // "session_id": session_id
    });

    info!(
        "get_answer_from_deepseek: request_body: {:?} ",
        request_body
    );

    let client = Client::new();
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        error!(
            "get_answer_from_deepseek: response error: {:?} ",
            response.status()
        );
        eprintln!("Request failed: {:?}", response.status());
    } else {
        info!("get_answer_from_deepseek: response: {:?} ", response);
    }

    let mut stream = response.bytes_stream();
    let mut full_data = Vec::new();

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => full_data.extend_from_slice(&chunk),
            Err(e) => {
                eprintln!("Error while reading stream: {}", e);
            }
        }
    }
    debug!("get_answer_from_deepseek: full_data: {:?} ", full_data);

    let data = String::from_utf8(full_data).expect("Failed to convert bytes to string");
    info!("get_answer_from_deepseek: Data: {:?} ", data);

    let mut chat_response: DeepSeekResponse =
        serde_json::from_str(&data).expect("Failed to parse JSON");

    info!(
        "get_chat_answer: chat_response {} ",
        chat_response.choices[0].message.content
    );

    if let Some(message) = chat_response
        .choices
        .get_mut(0)
        .map(|choice| &mut choice.message)
    {
        message.content = normalize_math_symbols(message.content.clone());
    }

    info!(
        "Answer replaced for math symble: {}",
        chat_response.choices[0].message.content
    );

    let answer_content = chat_response
        .choices
        .get(0)
        .map(|choice| choice.message.content.clone())
        .unwrap_or_else(|| "No answer received.".to_string());

    let chatrsp = ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    };

    Ok(chatrsp)
}

fn excerpt_for_log(s: &str) -> String {
    const LIMIT: usize = 400;
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= LIMIT {
            out.push('…');
            break;
        }
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
    out
}

fn normalize_math_symbols(content: String) -> String {
    if content.is_empty() {
        return content;
    }

    let raw = content;
    let out = raw
        .replace("\\[", "$")
        .replace("\\]", "$")
        .replace("\\(", "$")
        .replace("\\)", "$");

    if raw.contains("\\[")
        || raw.contains("\\]")
        || raw.contains("\\(")
        || raw.contains("\\)")
        || raw.contains("```")
    {
        info!(
            "normalize_math_symbols: raw={} out={}",
            excerpt_for_log(&raw),
            excerpt_for_log(&out)
        );
    }

    out
}

async fn openai_compat_sse_stream(
    app: &AppHandle,
    url: &str,
    api_key: &str,
    request_body: Value,
) -> Result<(), String> {
    let client = Client::new();
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Upstream error: {} {}", status, body));
    }

    let mut stream = response.bytes_stream();
    let mut carry = String::new();
    let mut buffer = String::new();
    let mut fence = CodeFenceTracker::default();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => {
                carry.push_str(&String::from_utf8_lossy(&data));

                for line in drain_complete_lines(&mut carry) {
                    if let Some(content) = stream_extract_content(&line) {
                        fence.push(&content);
                        buffer.push_str(&content);
                    }
                }

                if buffer.is_empty() {
                    continue;
                }

                if buffer.ends_with('\\') || buffer.ends_with(' ') || buffer.ends_with('\n') {
                    continue;
                }

                let out = normalize_math_symbols(buffer.clone());
                app.emit("modle-streaming-respond-output", &out)
                    .map_err(|e| e.to_string())?;

                buffer.clear();
            }
            Err(e) => return Err(format!("Error while reading stream: {}", e)),
        }
    }

    let tail = carry.trim();
    if !tail.is_empty() {
        if let Some(content) = stream_extract_content(tail) {
            fence.push(&content);
            buffer.push_str(&content);
        }
    }

    if !buffer.is_empty() {
        let out = normalize_math_symbols(buffer);
        app.emit("modle-streaming-respond-output", &out)
            .map_err(|e| e.to_string())?;
    }

    if let Some(closer) = fence.closing_delta() {
        app.emit("modle-streaming-respond-output", &closer)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

async fn openai_compat_chat(
    url: &str,
    api_key: &str,
    model: &str,
    question: &str,
) -> Result<String, String> {
    let request_body = json!({
        "model": model,
        "messages": [{"role": "user", "content": question}],
    });

    let client = Client::new();
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Upstream error: {} {}", status, body));
    }

    let data = response.text().await.map_err(|e| e.to_string())?;
    let mut chat_response: DeepSeekResponse = serde_json::from_str(&data).map_err(|e| e.to_string())?;

    let content = chat_response
        .choices
        .get_mut(0)
        .map(|c| normalize_math_symbols(c.message.content.clone()))
        .unwrap_or_else(|| "No answer received.".to_string());

    Ok(content)
}

pub async fn get_answer_from_chatgpt(
    question: &str,
    sessionid: &str,
) -> Result<ChatResponse, String> {
    info!(
        "get_answer_from_chatgpt: Received request: {} {}",
        question, sessionid
    );

    let api_key = settingview::get_llm_config().chatgpt_api_key;
    if api_key.trim().is_empty() {
        return Err("ChatGPT API Key 未配置".to_string());
    }

    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let answer_content = openai_compat_chat(
        "https://api.openai.com/v1/chat/completions",
        &api_key,
        "gpt-4o-mini",
        question,
    )
    .await?;

    Ok(ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    })
}

pub async fn get_answer_from_kimi(question: &str, sessionid: &str) -> Result<ChatResponse, String> {
    info!(
        "get_answer_from_kimi: Received request: {} {}",
        question, sessionid
    );

    let api_key = settingview::get_llm_config().kimi_api_key;
    if api_key.trim().is_empty() {
        return Err("Kimi API Key 未配置".to_string());
    }

    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let answer_content = openai_compat_chat(
        "https://api.moonshot.cn/v1/chat/completions",
        &api_key,
        "moonshot-v1-8k",
        question,
    )
    .await?;

    Ok(ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    })
}

pub async fn get_answer_from_qwen(question: &str, sessionid: &str) -> Result<ChatResponse, String> {
    info!(
        "get_answer_from_qwen: Received request: {} {}",
        question, sessionid
    );

    let api_key = settingview::get_llm_config().qwen_api_key;
    if api_key.trim().is_empty() {
        return Err("Qwen API Key 未配置".to_string());
    }

    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let answer_content = openai_compat_chat(
        "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
        &api_key,
        "qwen-plus",
        question,
    )
    .await?;

    Ok(ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    })
}

pub async fn get_answer_from_doubao(
    question: &str,
    sessionid: &str,
) -> Result<ChatResponse, String> {
    info!(
        "get_answer_from_doubao: Received request: {} {}",
        question, sessionid
    );

    let api_key = settingview::get_llm_config().doubao_api_key;
    if api_key.trim().is_empty() {
        return Err("豆包 API Key 未配置".to_string());
    }

    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let answer_content = openai_compat_chat(
        "https://ark.cn-beijing.volces.com/api/v3/chat/completions",
        &api_key,
        "doubao-pro-32k",
        question,
    )
    .await?;

    Ok(ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    })
}

pub async fn get_answer_from_local_deepseek(
    question: &str,
    sessionid: &str,
) -> Result<ChatResponse, String> {
    let url = "http://localhost:11434/api/chat";
    let session_id;

    info!(
        "get_answer_from_local_deepseek: Received request: {} {}",
        question, sessionid
    );

    if sessionid.is_empty() {
        session_id = generate_session_id();
        info!("get_answer_from_local_deepseek: sessionid is empty, generating a new one.");
    } else {
        session_id = sessionid.to_string();
        info!("get_answer_from_local_deepseek: session_id: {}", session_id);
    }

    let request_body = json!({
        "model": "deepseek-r1:8b",
        "messages": [{"role": "user", "content": question}],
        "stream": false,
        "session_id": session_id
    });

    info!(
        "get_answer_from_local_deepseek: request_body: {:?} ",
        request_body
    );

    // 禁用系统代理
    let client = ClientBuilder::new()
        .no_proxy() // 禁用代理
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post(url)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        error!(
            "get_answer_from_local_deepseek: response error: {:?} ",
            response.status()
        );
        eprintln!("Request failed: {:?}", response.status());
    } else {
        info!("get_answer_from_local_deepseek: response: {:?} ", response);
    }

    let mut stream = response.bytes_stream();
    let mut full_data = Vec::new();

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => full_data.extend_from_slice(&chunk),
            Err(e) => {
                eprintln!("Error while reading stream: {}", e);
            }
        }
    }
    debug!(
        "get_answer_from_local_deepseek: full_data: {:?} ",
        full_data
    );

    let data = String::from_utf8(full_data).expect("Failed to convert bytes to string");
    info!("get_answer_from_local_deepseek: Data: {:?} ", data);

    // // 替换 `\[` 和 `\]` 及相关换行符
    // let re_block = Regex::new(r"\\\[\s*\n|\n\s*\\\]").unwrap();
    // // 替换 \( 和 \) 及相关换行符
    // let re_inline = Regex::new(r"\\\(\s*|\s*\\\)").unwrap();

    // // 对 content 进行替换
    // if let Some(message) = chat_response
    //     .choices
    //     .get_mut(0)
    //     .map(|choice| &mut choice.message)
    // {
    //     message.content = re_block.replace_all(&message.content, "$$").to_string();
    //     message.content = re_inline.replace_all(&message.content, "$").to_string();
    // }

    // info!("get_answer_from_local_deepseek: Answer replaced for math symble: {}", chat_response.choices[0].message.content);

    // let answer_content = chat_response
    //     .choices
    //     .get(0)
    //     .map(|choice| choice.message.content.clone())
    //     .unwrap_or_else(|| "No answer received.".to_string());

    // 使用?运算符传播错误
    let answer_content = collect_content_from_local_deepseek(&data).await?;

    let chatrsp = ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    };
    info!("get_answer_from_local_deepseek: Answer: {:?}", chatrsp);
    Ok(chatrsp)
}

pub async fn get_answer_from_chatgpt_stream(
    app: AppHandle,
    question: &str,
    sessionid: &str,
) -> Result<(), String> {
    info!(
        "get_answer_from_chatgpt_stream: Received request: {} {}",
        question, sessionid
    );

    let api_key = settingview::get_llm_config().chatgpt_api_key;
    if api_key.trim().is_empty() {
        return Err("ChatGPT API Key 未配置".to_string());
    }

    let request_body = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": question}],
        "stream": true,
    });

    openai_compat_sse_stream(
        &app,
        "https://api.openai.com/v1/chat/completions",
        &api_key,
        request_body,
    )
    .await
}
pub async fn get_answer_from_deepseek_stream(
    app: AppHandle,
    question: &str,
    sessionid: &str,
) -> Result<(), String> {
    let api_key = settingview::get_llm_config().deepseek_api_key;
    if api_key.trim().is_empty() {
        return Err("DeepSeek API Key 未配置".to_string());
    }

    info!(
        "get_answer_from_deepseek_stream: Received request: {:?} {}",
        question, sessionid
    );

    let request_body = json!({
        "model": "deepseek-chat",
        "messages": [{"role": "user", "content": question}],
        "stream": true,
    });

    openai_compat_sse_stream(
        &app,
        "https://api.deepseek.com/chat/completions",
        &api_key,
        request_body,
    )
    .await
}

pub async fn get_answer_from_kimi_stream(
    app: AppHandle,
    question: &str,
    sessionid: &str,
) -> Result<(), String> {
    let api_key = settingview::get_llm_config().kimi_api_key;
    if api_key.trim().is_empty() {
        return Err("Kimi API Key 未配置".to_string());
    }

    info!(
        "get_answer_from_kimi_stream: Received request: {:?} {}",
        question, sessionid
    );

    let request_body = json!({
        "model": "moonshot-v1-8k",
        "messages": [{"role": "user", "content": question}],
        "temperature": 0.3,
        "stream": true,
    });

    openai_compat_sse_stream(
        &app,
        "https://api.moonshot.cn/v1/chat/completions",
        &api_key,
        request_body,
    )
    .await
}

pub async fn get_answer_from_doubao_stream(
    app: AppHandle,
    question: &str,
    sessionid: &str,
) -> Result<(), String> {
    let rsp = get_answer_from_doubao(question, sessionid).await?;
    app.emit("modle-streaming-respond-output", &rsp.answer)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_answer_from_qwen_stream(
    app: AppHandle,
    question: &str,
    sessionid: &str,
) -> Result<(), String> {
    let rsp = get_answer_from_qwen(question, sessionid).await?;
    app.emit("modle-streaming-respond-output", &rsp.answer)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_answer_from_local_qwen_stream(
    app: AppHandle,
    question: &str,
    sessionid: &str,
) -> Result<(), String> {
    let url = "http://localhost:11434/api/chat";
    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let request_body = json!({
        "model": "qwen3:8b",
        "messages": [{"role": "user", "content": question}],
        "stream": true,  // 启用流式传输
        "session_id": session_id,
    });

    info!(
        "get_answer_from_local_qwen_stream: request_body: {:?}",
        request_body
    );

    // 禁用系统代理
    let client = ClientBuilder::new()
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post(url)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!(
            "Error calling Local Qwen API: {:?}",
            response.status()
        ));
    }

    let mut stream = response.bytes_stream();
    let mut carry = String::new();
    let mut buffer = String::new();
    let mut fence = CodeFenceTracker::default();

    // 流式输出数据
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => {
                carry.push_str(&String::from_utf8_lossy(&data));

                for line in drain_complete_lines(&mut carry) {
                    if let Some(content) = stream_extract_content(&line) {
                        fence.push(&content);
                        buffer.push_str(&content);
                    }
                }

                if buffer.is_empty() {
                    continue;
                }

                if buffer.ends_with('\\') || buffer.ends_with(' ') || buffer.ends_with('\n') {
                    continue;
                }

                let out = normalize_math_symbols(buffer.clone());
                app.emit("modle-streaming-respond-output", &out)
                    .map_err(|e| e.to_string())?;

                buffer.clear();
            }
            Err(e) => {
                return Err(format!("Error while reading stream: {}", e));
            }
        }
    }

    let tail = carry.trim();
    if !tail.is_empty() {
        if let Some(content) = stream_extract_content(tail) {
            fence.push(&content);
            buffer.push_str(&content);
        }
    }

    // 流结束后，处理剩余的缓冲区数据
    if !buffer.is_empty() {
        let out = normalize_math_symbols(buffer);

        app.emit("modle-streaming-respond-output", &out)
            .map_err(|e| e.to_string())?;
        debug!(
            "get_answer_from_local_qwen_stream: Emitting remaining buffered data: {}",
            out
        );
        debug!("get_answer_from_local_qwen_stream: emit event modle-streaming-respond-output");
    }

    if let Some(closer) = fence.closing_delta() {
        app.emit("modle-streaming-respond-output", &closer)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub async fn get_answer_from_local_deepseek_stream(
    app: AppHandle,
    question: &str,
    sessionid: &str,
) -> Result<(), String> {
    let url = "http://localhost:11434/api/chat";
    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let request_body = json!({
        "model": "deepseek-r1:8b",
        "messages": [{"role": "user", "content": question}],
        "stream": true,  // 启用流式传输
        "session_id": session_id,
    });

    info!(
        "get_answer_from_local_deepseek_stream: request_body: {:?}",
        request_body
    );

    // 禁用系统代理
    let client = ClientBuilder::new()
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post(url)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!(
            "Error calling DeepSeek API: {:?}",
            response.status()
        ));
    }

    let mut stream = response.bytes_stream();
    let mut carry = String::new();
    let mut buffer = String::new();
    let mut last_emitted = String::new(); // 新增：记录上次发送的内容
    let mut fence = CodeFenceTracker::default();

    // 流式输出数据
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => {
                carry.push_str(&String::from_utf8_lossy(&data));

                for line in drain_complete_lines(&mut carry) {
                    if let Some(content) = stream_extract_content(&line) {
                        fence.push(&content);
                        buffer.push_str(&content);
                    }
                }

                debug!(
                    "get_answer_from_local_deepseek_stream: origin buffer: {:?}",
                    buffer
                );

                if buffer.is_empty() {
                    continue;
                }

                // 跳过特定字符的情况保持不变
                if buffer.ends_with('\\') || buffer.ends_with(' ') || buffer.ends_with('\n') {
                    continue;
                }

                debug!(
                    "get_answer_from_local_deepseek_stream: buffer: {:?}",
                    buffer
                );

                let out = normalize_math_symbols(buffer.clone());

                if out != last_emitted && !out.is_empty() {
                    app.emit("modle-streaming-respond-output", &out)
                        .map_err(|e| e.to_string())?;
                    last_emitted = out.clone();
                    debug!(
                        "get_answer_from_local_deepseek_stream: Streaming data: {:?}",
                        out
                    );
                }

                buffer.clear();
            }
            Err(e) => {
                return Err(format!("Error while reading stream: {}", e));
            }
        }
    }

    let tail = carry.trim();
    if !tail.is_empty() {
        if let Some(content) = stream_extract_content(tail) {
            fence.push(&content);
            buffer.push_str(&content);
        }
    }

    // 流结束后，处理剩余的缓冲区数据
    if !buffer.is_empty() {
        let out = normalize_math_symbols(buffer);

        if out != last_emitted && !out.is_empty() {
            app.emit("modle-streaming-respond-output", &out)
                .map_err(|e| e.to_string())?;
            debug!(
                "get_answer_from_local_deepseek_stream: Emitting remaining buffered data: {}",
                out
            );
        }

        debug!("get_answer_from_local_deepseek_stream: emit event modle-streaming-respond-output");
    }

    if let Some(closer) = fence.closing_delta() {
        app.emit("modle-streaming-respond-output", &closer)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub async fn get_answer_from_local_qwen(
    question: &str,
    sessionid: &str,
) -> Result<ChatResponse, String> {
    let url = "http://localhost:11434/api/chat";
    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    info!(
        "get_answer_from_local_qwen: Received request: {} {}",
        question, sessionid
    );

    let request_body = json!({
        "model": "qwen3:8b",
        "messages": [{"role": "user", "content": question}],
        "stream": false,
        "session_id": session_id
    });

    let client = ClientBuilder::new()
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post(url)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("Error calling Local Qwen API: {:?}", response.status()));
    }

    let data = response.text().await.map_err(|e| e.to_string())?;
    let answer_content = collect_content_from_local_deepseek(&data).await?;

    Ok(ChatResponse {
        answer: normalize_math_symbols(answer_content),
        sessionid: request_body["session_id"].as_str().unwrap_or("").to_string(),
    })
}

fn sanitize_filename_component(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    if s.is_empty() {
        return s;
    }
    for ch in ['\\', '/', ':', '*', '?', '"', '<', '>', '|'] {
        s = s.replace(ch, "_");
    }
    s
}

fn ensure_chat_record_root() -> Result<PathBuf, String> {
    let root = PathBuf::from(settingview::get_chat_record_root());
    if !root.exists() {
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    }
    Ok(root)
}

#[derive(Deserialize)]
pub struct ExportChatSessionRequest {
    pub model_id: Option<i32>,
    pub model_name: Option<String>,
    pub topic_id: Option<i32>,
    pub session_id: Option<String>,
    pub content: Value,
}

#[tauri::command]
pub async fn export_chat_session(request: ExportChatSessionRequest) -> Result<String, String> {
    let root = ensure_chat_record_root()?;

    let model = sanitize_filename_component(
        request
            .model_name
            .as_deref()
            .unwrap_or("chat"),
    );
    let sid = sanitize_filename_component(request.session_id.as_deref().unwrap_or(""));

    let ts = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let file_name = if sid.trim().is_empty() {
        format!("{}_{}.json", ts, model)
    } else {
        format!("{}_{}_{}.json", ts, model, sid)
    };

    let file_path = root.join(file_name);

    let payload = json!({
        "exported_at": Utc::now().to_rfc3339(),
        "model_id": request.model_id,
        "model_name": model,
        "topic_id": request.topic_id,
        "session_id": request.session_id,
        "content": request.content,
    });

    let data = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    fs::write(&file_path, data).map_err(|e| e.to_string())?;

    Ok(file_path.to_string_lossy().to_string())
}

#[derive(Deserialize)]
pub struct SaveChatSessionRequest {
    pub model_id: Option<i32>,
    pub model_name: Option<String>,
    pub topic_id: Option<i32>,
    pub session_id: Option<String>,
    pub content: Value,
}

#[tauri::command]
pub async fn save_chat_session(request: SaveChatSessionRequest) -> Result<String, String> {
    let root = ensure_chat_record_root()?;
    let sessions_dir = root.join("sessions");
    if !sessions_dir.exists() {
        fs::create_dir_all(&sessions_dir).map_err(|e| e.to_string())?;
    }

    let model = sanitize_filename_component(
        request
            .model_name
            .as_deref()
            .unwrap_or("chat"),
    );

    let sid_raw = if let Some(sid) = request.session_id.as_deref() {
        sid.to_string()
    } else {
        format!(
            "model{}_topic{}",
            request.model_id.unwrap_or(-1),
            request.topic_id.unwrap_or(-1)
        )
    };
    let sid = sanitize_filename_component(&sid_raw);

    let file_name = if model.trim().is_empty() {
        format!("{}.json", sid)
    } else {
        format!("{}_{}.json", sid, model)
    };

    let file_path = sessions_dir.join(file_name);

    let payload = json!({
        "saved_at": Utc::now().to_rfc3339(),
        "model_id": request.model_id,
        "model_name": request.model_name,
        "topic_id": request.topic_id,
        "session_id": request.session_id,
        "content": request.content,
    });

    let data = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    fs::write(&file_path, data).map_err(|e| e.to_string())?;

    Ok(file_path.to_string_lossy().to_string())
}

fn web_model_url(provider: &str) -> Option<&'static str> {
    match provider {
        "yuanbao" => Some("https://yuanbao.tencent.com"),
        "chatgpt" => Some("https://chatgpt.com"),
        _ => None,
    }
}

fn web_model_capture_init_script(provider: &str) -> String {
    let provider_key = provider.trim().to_lowercase();
    let provider_json = serde_json::to_string(&provider_key).unwrap_or_else(|_| "\"\"".to_string());
    let endpoint_json = serde_json::to_string("http://127.0.0.1:8888/api/web-model-chat")
        .unwrap_or_else(|_| "\"\"".to_string());

    include_str!("../web_scripts/web_model_capture_init.js")
        .replace("__GUAXI_PROVIDER_JSON__", &provider_json)
        .replace("__GUAXI_ENDPOINT_JSON__", &endpoint_json)
}

async fn ensure_web_model_window(app: &tauri::AppHandle, provider_key: &str) -> Result<(), String> {
    let url = web_model_url(provider_key).ok_or("Unsupported provider".to_string())?;
    let label = format!("web-model-{}", provider_key);

    if app.get_webview_window(&label).is_some() {
        return Ok(());
    }

    let external = Url::parse(url).map_err(|e| e.to_string())?;
    let init_script = web_model_capture_init_script(provider_key);

    WebviewWindowBuilder::new(app, &label, WebviewUrl::External(external))
        .title(url)
        .inner_size(1100.0, 780.0)
        .min_inner_size(820.0, 560.0)
        .initialization_script(init_script.as_str())
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    Ok(())
}

#[derive(Deserialize)]
pub struct WebModelSendRequest {
    provider: String,
    model_id: i32,
    topic_id: i32,
    session_id: String,
    content: String,
}

#[tauri::command]
pub async fn web_model_send_message(
    app: tauri::AppHandle,
    request: WebModelSendRequest,
) -> Result<(), String> {
    let provider_key = request.provider.trim().to_lowercase();
    ensure_web_model_window(&app, &provider_key).await?;

    let label = format!("web-model-{}", provider_key);
    let w = app
        .get_webview_window(&label)
        .ok_or("web model window not found".to_string())?;

    let content_json = serde_json::to_string(&request.content).map_err(|e| e.to_string())?;
    let session_json = serde_json::to_string(&request.session_id).map_err(|e| e.to_string())?;

    let js = format!(
        r#"(() => {{
  try {{
    window.__GUAXI_MODEL_ID__ = {model_id};
    window.__GUAXI_TOPIC_ID__ = {topic_id};
    window.__GUAXI_SESSION_ID__ = {session_id};

    const provider = {provider};
    const text = {content};

    function findInput() {{
      const sendBtn = document.querySelector('#yuanbao-send-btn');
      if (provider === 'yuanbao' && sendBtn) {{
        const scope = (sendBtn.closest('form') || sendBtn.closest('main, [role="main"], body') || document.body);
        return (
          scope.querySelector('textarea') ||
          scope.querySelector('[contenteditable="true"]') ||
          scope.querySelector('[role="textbox"]') ||
          document.querySelector('textarea') ||
          document.querySelector('[contenteditable="true"]') ||
          document.querySelector('[role="textbox"]')
        );
      }}

      if (provider === 'chatgpt') {{
        return (
          document.querySelector('#prompt-textarea') ||
          document.querySelector('textarea[data-testid="prompt-textarea" i]') ||
          document.querySelector('textarea') ||
          document.querySelector('[contenteditable="true"]') ||
          document.querySelector('[role="textbox"]')
        );
      }}

      return (
        document.querySelector('#prompt-textarea') ||
        document.querySelector('textarea') ||
        document.querySelector('[contenteditable="true"]') ||
        document.querySelector('[role="textbox"]')
      );
    }}

    function setNativeValue(el, value) {{
      try {{
        const proto = el instanceof HTMLTextAreaElement
          ? HTMLTextAreaElement.prototype
          : HTMLInputElement.prototype;
        const desc = Object.getOwnPropertyDescriptor(proto, 'value');
        if (desc && typeof desc.set === 'function') {{
          desc.set.call(el, value);
          return true;
        }}
      }} catch (e) {{}}
      try {{
        el.value = value;
        return true;
      }} catch (e) {{}}
      return false;
    }}

    function setValue(el) {{
      if (!el) return false;

      try {{ el.focus(); }} catch (e) {{}}

      if (typeof el.value === 'string') {{
        if (!setNativeValue(el, text)) return false;
        try {{ el.dispatchEvent(new InputEvent('input', {{ bubbles: true, data: text }})); }} catch (e) {{ el.dispatchEvent(new Event('input', {{ bubbles: true }})); }}
        try {{ el.dispatchEvent(new Event('change', {{ bubbles: true }})); }} catch (e) {{}}
        return true;
      }}

      if (el.isContentEditable) {{
        try {{
          el.textContent = '';
          if (document.execCommand) {{
            document.execCommand('selectAll', false, null);
            document.execCommand('insertText', false, text);
          }} else {{
            el.textContent = text;
          }}
        }} catch (e) {{
          try {{ el.textContent = text; }} catch (e2) {{ return false; }}
        }}

        try {{ el.dispatchEvent(new InputEvent('input', {{ bubbles: true, data: text }})); }} catch (e) {{ el.dispatchEvent(new Event('input', {{ bubbles: true }})); }}
        return true;
      }}

      return false;
    }}

    function isDisabled(el) {{
      if (!el) return true;
      if (el.disabled) return true;
      const aria = (el.getAttribute && el.getAttribute('aria-disabled')) || '';
      if (aria.toString().toLowerCase() === 'true') return true;
      return false;
    }}

    function click(el) {{
      if (!el) return false;
      if (isDisabled(el)) return false;
      try {{ el.click(); return true; }} catch (e) {{}}
      try {{
        el.dispatchEvent(new MouseEvent('click', {{ bubbles: true, cancelable: true, view: window }}));
        return true;
      }} catch (e) {{}}
      return false;
    }}

    function bestSendButton(input) {{
      const root = (input && (input.closest('form') || input.closest('main, [role="main"], body'))) || document.body;
      const inputRect = input && input.getBoundingClientRect ? input.getBoundingClientRect() : null;
      const nodes = Array.from(root.querySelectorAll('button, [role="button"], a'));

      let best = null;
      let bestScore = 0;

      for (const b of nodes) {{
        if (!b || isDisabled(b)) continue;

        const label = ((b.getAttribute && (b.getAttribute('aria-label') || b.getAttribute('title') || b.getAttribute('data-testid') || b.getAttribute('id'))) || '').toString();
        const txt = ((b.innerText || b.textContent) || '').toString().trim();
        const cls = (b.className || '').toString();
        const type = (b.getAttribute && b.getAttribute('type')) || '';

        let score = 0;

        if (/语音|voice|speech/i.test(label) || /语音|voice|speech/i.test(txt) || /speech|voice/i.test(cls)) score -= 200;

        if ((b.id || '') === 'yuanbao-send-btn') score += 220;
        if (/send|发送|提交|确定/i.test(label)) score += 90;
        if (/send|发送|提交|确定/i.test(txt)) score += 80;
        if (/send/i.test(cls)) score += 35;
        if (type.toString().toLowerCase() === 'submit') score += 70;
        if (label.toString().toLowerCase().includes('send')) score += 30;
        if (label.includes('发送')) score += 30;
        if (b.querySelector && b.querySelector('svg')) score += 5;

        if (inputRect && b.getBoundingClientRect) {{
          const r = b.getBoundingClientRect();
          const dx = (r.left - inputRect.right);
          const dy = Math.abs(r.top - inputRect.top);
          const dist = Math.sqrt(dx * dx + dy * dy);
          if (Number.isFinite(dist)) score -= Math.min(50, dist / 10);
          if (r.width <= 80 && r.height <= 80) score += 5;
        }}

        if (score > bestScore) {{
          bestScore = score;
          best = b;
        }}
      }}

      return bestScore > 10 ? best : null;
    }}

    function trySubmitForm(input) {{
      const form = input && (input.form || (input.closest && input.closest('form')));
      if (!form) return false;
      try {{
        if (typeof form.requestSubmit === 'function') {{
          form.requestSubmit();
          return true;
        }}
      }} catch (e) {{}}
      try {{
        form.dispatchEvent(new Event('submit', {{ bubbles: true, cancelable: true }}));
        return true;
      }} catch (e) {{}}
      return false;
    }}

    function tryEnter(input) {{
      try {{
        const down = new KeyboardEvent('keydown', {{ key: 'Enter', code: 'Enter', bubbles: true, cancelable: true }});
        const up = new KeyboardEvent('keyup', {{ key: 'Enter', code: 'Enter', bubbles: true, cancelable: true }});
        input.dispatchEvent(down);
        input.dispatchEvent(up);
        return true;
      }} catch (e) {{}}
      return false;
    }}

    function sendNow() {{
      const input = findInput();
      if (!setValue(input)) return;

      if (trySubmitForm(input)) return;

      if (provider === 'yuanbao') {{
        const ybtn = document.querySelector('#yuanbao-send-btn');
        if (ybtn && click(ybtn)) return;
      }}

      const btn = bestSendButton(input);
      if (btn && click(btn)) return;

      tryEnter(input);
    }}

    sendNow();
    setTimeout(sendNow, 120);
    setTimeout(sendNow, 420);
    setTimeout(sendNow, 900);
  }} catch (e) {{}}
}})();"#,
        model_id = request.model_id,
        topic_id = request.topic_id,
        session_id = session_json,
        provider = serde_json::to_string(&provider_key).unwrap_or_else(|_| "\"\"".to_string()),
        content = content_json
    );

    w.eval(js.as_str()).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn open_web_model_window(app: tauri::AppHandle, provider: String) -> Result<(), String> {
    let provider_key = provider.trim().to_lowercase();
    ensure_web_model_window(&app, &provider_key).await?;

    let label = format!("web-model-{}", provider_key);
    if let Some(w) = app.get_webview_window(&label) {
        w.show().map_err(|e| format!("显示窗口失败: {}", e))?;
        w.set_focus().ok();
    }

    Ok(())
}

#[tauri::command]
pub async fn prepare_web_model_window(app: tauri::AppHandle, provider: String) -> Result<(), String> {
    let provider_key = provider.trim().to_lowercase();
    ensure_web_model_window(&app, &provider_key).await?;
    Ok(())
}

#[tauri::command]
pub async fn get_chat_answer_stream(app: AppHandle, request: ChatRequest) -> Result<(), String> {
    info!("Enter get_chat_answer_stream");
    let model =
        ChatModel::try_from(request.modelid).map_err(|e| format!("Invalid model ID: {}", e))?;
    info!(
        "Received request: model_id:{} question:{} session_id:{}",
        request.modelid, request.question, request.sessionid
    );

    match model {
        ChatModel::CHATGPT => {
            match get_answer_from_chatgpt_stream(app, &request.question, &request.sessionid).await {
                Ok(()) => {
                    info!("get_chat_answer_stream CHATGPT: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling ChatGPT API: {}", err)),
            }
        }
        ChatModel::DEEPSEEK => {
            match get_answer_from_deepseek_stream(app, &request.question, &request.sessionid).await
            {
                Ok(()) => {
                    info!("get_chat_answer_stream DEEPSEEK: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling DeepSeek API: {}", err)),
            }
        }
        ChatModel::KIMI => {
            match get_answer_from_kimi_stream(app, &request.question, &request.sessionid).await {
                Ok(()) => {
                    info!("get_chat_answer_stream KIMI: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Kimi API: {}", err)),
            }
        }
        ChatModel::QWEN => {
            match get_answer_from_qwen_stream(app, &request.question, &request.sessionid).await {
                Ok(()) => {
                    info!("get_chat_answer_stream QWEN: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Qwen API: {}", err)),
            }
        }
        ChatModel::DOUBAO => {
            match get_answer_from_doubao_stream(app, &request.question, &request.sessionid).await {
                Ok(()) => {
                    info!("get_chat_answer_stream DOUBAO: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Doubao API: {}", err)),
            }
        }
        ChatModel::LOCALQWEN => {
            match get_answer_from_local_qwen_stream(app, &request.question, &request.sessionid)
                .await
            {
                Ok(()) => {
                    info!("get_chat_answer_stream LOCALQWEN: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Local Qwen API: {}", err)),
            }
        }
        ChatModel::LOCALDEEPSEEK => {
            match get_answer_from_local_deepseek_stream(app, &request.question, &request.sessionid)
                .await
            {
                Ok(()) => {
                    info!("get_chat_answer_stream LOCALDEEPSEEK: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Local Model API: {}", err)),
            }
        }
        ChatModel::YUANBAO_WEB => {
            let session_id = if request.sessionid.trim().is_empty() {
                generate_session_id()
            } else {
                request.sessionid.to_string()
            };

            web_model_send_message(
                app,
                WebModelSendRequest {
                    provider: "yuanbao".to_string(),
                    model_id: request.modelid,
                    topic_id: request.topicid,
                    session_id,
                    content: request.question.to_string(),
                },
            )
            .await?;

            Ok(())
        }
        ChatModel::CHATGPT_WEB => {
            let session_id = if request.sessionid.trim().is_empty() {
                generate_session_id()
            } else {
                request.sessionid.to_string()
            };

            web_model_send_message(
                app,
                WebModelSendRequest {
                    provider: "chatgpt".to_string(),
                    model_id: request.modelid,
                    topic_id: request.topicid,
                    session_id,
                    content: request.question.to_string(),
                },
            )
            .await?;

            Ok(())
        }
    }
}

#[tauri::command]
pub async fn get_chat_answer(request: ChatRequest) -> Result<ChatResponse, String> {
    info!("Enter get_chat_answer");
    let model =
        ChatModel::try_from(request.modelid).map_err(|e| format!("Invalid model ID: {}", e))?;
    info!(
        "Received request: model_id:{} question:{} session_id:{}",
        request.modelid, request.question, request.sessionid
    );

    match model {
        ChatModel::CHATGPT => {
            match get_answer_from_chatgpt(&request.question, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer CHATGPT: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling ChatGPT API: {}", err)),
            }
        }
        ChatModel::DEEPSEEK => {
            match get_answer_from_deepseek(&request.question, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer DEEPSEEK: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling DeepSeek API: {}", err)),
            }
        }
        ChatModel::KIMI => {
            match get_answer_from_kimi(&request.question, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer KIMI: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling Kimi API: {}", err)),
            }
        }
        ChatModel::QWEN => {
            match get_answer_from_qwen(&request.question, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer QWEN: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling Qwen API: {}", err)),
            }
        }
        ChatModel::DOUBAO => {
            match get_answer_from_doubao(&request.question, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer DOUBAO: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling Doubao API: {}", err)),
            }
        }
        ChatModel::LOCALQWEN => {
            match get_answer_from_local_qwen(&request.question, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer LOCALQWEN: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling Local Qwen API: {}", err)),
            }
        }
        ChatModel::LOCALDEEPSEEK => {
            match get_answer_from_local_deepseek(&request.question, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer LOCALDEEPSEEK: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling Local Model API: {}", err)),
            }
        }
        ChatModel::YUANBAO_WEB => Ok(ChatResponse {
            answer: "该模型为网页模式，请调用 open_web_model_window(provider='yuanbao') 进行对话。".to_string(),
            sessionid: if request.sessionid.is_empty() {
                generate_session_id()
            } else {
                request.sessionid
            },
        }),
        ChatModel::CHATGPT_WEB => Ok(ChatResponse {
            answer: "该模型为网页模式，请调用 open_web_model_window(provider='chatgpt') 进行对话。".to_string(),
            sessionid: if request.sessionid.is_empty() {
                generate_session_id()
            } else {
                request.sessionid
            },
        }),
    }
}
