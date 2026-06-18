//! LLM 增强层：用已接入的硅基流动(DeepSeek/Qwen)对清洗后的正文做
//! 相关性打分 + 摘要 + 标签（一次调用合并产出，控成本）。
//!
//! 设计：
//! - 单次调用返回结构化 JSON `{relevance, summary, tags, cleaned?}`，避免多次往返。
//! - 截断正文控 token；带预算护栏（每 tick / 全局上限），默认仅对开启 llm_enhance
//!   且过了关键词/本地相关性初筛的文章调用。
//! - API Key 未配置时静默跳过（返回 None），不阻断抓取链路。

use crate::commands::chatview;
use std::sync::atomic::{AtomicU32, Ordering};
use tracing::warn;

/// 全局 LLM 调用计数（粗略预算护栏，避免一次抓取打爆配额/成本）。
static LLM_CALLS: AtomicU32 = AtomicU32::new(0);
/// 单进程生命周期内的 LLM 调用上限（可按需调大；防止失控）。
const LLM_CALL_CAP: u32 = 2000;

#[derive(Debug, Clone, Default)]
pub struct LlmResult {
    /// 相关性 0-100；-1 表示模型未给出。
    pub relevance: i32,
    pub summary: String,
    pub tags: Vec<String>,
    /// 可选的二次清洗 Markdown（模型认为原文有噪声时返回）。
    pub cleaned_markdown: Option<String>,
}

/// 是否还有 LLM 预算。
pub fn has_budget() -> bool {
    LLM_CALLS.load(Ordering::Relaxed) < LLM_CALL_CAP
}

/// 对单篇文章做 LLM 增强。`interest` 为用户兴趣描述（可空）。
/// 返回 `None` 表示跳过（无 key / 无预算 / 调用失败）——调用方应优雅降级。
pub async fn enhance(title: &str, markdown: &str, interest: &str) -> Option<LlmResult> {
    if !has_budget() {
        return None;
    }
    // 截断正文，控 token。
    let body: String = markdown.chars().take(6000).collect();

    let interest_line = if interest.trim().is_empty() {
        "（用户未提供兴趣描述，relevance 给 50 即可）".to_string()
    } else {
        format!("用户的兴趣描述：{}", interest.trim())
    };

    let prompt = format!(
        r#"你是一个知识收集助手。请阅读下面这篇文章，并严格只输出一个 JSON 对象（不要任何额外解释、不要 markdown 代码围栏）。

{interest}

文章标题：{title}

文章正文（可能被截断）：
{body}

请输出如下结构的 JSON：
{{
  "relevance": <0-100 的整数，表示与用户兴趣的相关程度；无兴趣描述时给 50>,
  "summary": "<60字以内的中文摘要>",
  "tags": ["<3-6个主题标签>"],
  "cleaned": "<如果原文有明显噪声(导航/广告/乱码)，给出清洗后的正文Markdown；否则留空字符串>"
}}"#,
        interest = interest_line,
        title = title,
        body = body,
    );

    LLM_CALLS.fetch_add(1, Ordering::Relaxed);
    let resp = match chatview::get_answer_from_siliconflow(&prompt, &[], "").await {
        Ok(r) => r,
        Err(e) => {
            // Key 未配置等：静默降级。
            warn!("flipboard_llm: enhance call failed/skipped: {}", e);
            return None;
        }
    };

    parse_llm_json(&resp.answer)
}

/// 容错解析：从模型回答里抽取第一个 JSON 对象并解析。
fn parse_llm_json(answer: &str) -> Option<LlmResult> {
    let start = answer.find('{')?;
    let end = answer.rfind('}')?;
    if end <= start {
        return None;
    }
    let json_str = &answer[start..=end];
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;

    let relevance = v
        .get("relevance")
        .and_then(|x| x.as_i64())
        .map(|n| n.clamp(0, 100) as i32)
        .unwrap_or(-1);
    let summary = v
        .get("summary")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let tags = v
        .get("tags")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let cleaned = v
        .get("cleaned")
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Some(LlmResult {
        relevance,
        summary,
        tags,
        cleaned_markdown: cleaned,
    })
}
