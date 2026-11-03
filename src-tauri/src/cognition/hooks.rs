//! 与既有链路的接线点。
//!
//! 所有钩子遵守同一条纪律：**永不改变宿主行为**。认知层不可用、未启用、或出错时，
//! 钩子静默退化成空操作，调用方拿到的结果与改造前完全一致。
//! 这是把一整层横切能力塞进一个已经在跑的应用里唯一可接受的方式。

use crate::cognition::{episode, percept, recall, runtime};
use tracing::debug;

/// 对话开始：起一段情节 → 召回接地（原地前置到 question）→ 把引证广播给前端。
///
/// 返回 episode id（空串表示没有记轨迹）。前端应在流结束后带完整答案调
/// `cognition_chat_finish`，否则这一轮的引证命中标签就丢了。
pub async fn hook_chat_start(
    app: &tauri::AppHandle,
    question: &mut String,
    sessionid: &str,
    modelid: i32,
) -> String {
    if !runtime::enabled() {
        return String::new();
    }
    let original = question.clone();
    let episode_id = episode::start("chat", sessionid, &modelid.to_string(), &original, "");
    if !recall::should_ground(&original) {
        return episode_id;
    }
    let (prefix, res) = recall::ground_question(&episode_id, &original, None).await;
    if prefix.is_empty() {
        if res.degraded {
            debug!("[cognition] 本轮未接地：embedding 服务未就绪");
        }
        return episode_id;
    }
    *question = format!("{prefix}{original}");
    // 广播引证：前端渲染引证条，用户点开时上报 clicked。
    use tauri::Emitter;
    let _ = app.emit(
        "cognition_grounded",
        serde_json::json!({
            "episodeId": episode_id,
            "citations": res.citations,
            "channels": res.channels,
            "elapsedMs": res.elapsed_ms,
        }),
    );
    episode_id
}

/// 对话链路返回 Err 时收尾。
pub fn hook_chat_failed(episode_id: &str, err: &str) {
    if episode_id.is_empty() {
        return;
    }
    episode::add_step(episode_id, "error", "dispatch", err);
    episode::finish(episode_id, "", "error", 0, 0);
}

/// Agent 开始：与对话同构，但不改写问题——Agent 有自己的工具去取上下文，
/// 前置一大段召回反而会挤占它的工具预算。这里只记轨迹 + 把引证作为提示交给前端。
pub async fn hook_agent_start(
    app: &tauri::AppHandle,
    question: &str,
    sessionid: &str,
    modelid: i32,
) -> String {
    if !runtime::enabled() {
        return String::new();
    }
    let episode_id = episode::start("agent", sessionid, &modelid.to_string(), question, "");
    if !recall::should_ground(question) {
        return episode_id;
    }
    // 只落引证、不拼提示词：Agent 可以自己调 guaxi_recall 拿更精确的东西。
    if let Ok(cites) = recall::recall_brief(question, None, 5).await {
        if !cites.is_empty() {
            episode::record_citations(&episode_id, &cites);
            use tauri::Emitter;
            let _ = app.emit(
                "cognition_grounded",
                serde_json::json!({ "episodeId": episode_id, "citations": cites, "mode": "agent" }),
            );
        }
    }
    episode_id
}

/// 感知入口：任何采集链路都走这里，不要各自去调 percept::ingest，
/// 以便未来加限流、加来源开关时只改一处。
pub fn hook_percept(
    source_kind: &str,
    title: &str,
    text: &str,
    uri: &str,
    source_id: &str,
    kb_name: &str,
) {
    if !runtime::enabled() {
        return;
    }
    if !source_enabled(source_kind) {
        return;
    }
    // 自动采集的促销/广告页不进大脑。用户主动采集(capture/manual)不受影响。
    if matches!(source_kind, "crawl" | "rss") && percept::looks_like_noise(title, uri) {
        debug!("[cognition] 感知入库跳过（噪声）: {} {}", title, uri);
        return;
    }
    let input = percept::PerceptInput {
        source_kind: source_kind.to_string(),
        source_id: source_id.to_string(),
        uri: uri.to_string(),
        title: title.to_string(),
        text: text.to_string(),
        kb_name: kb_name.to_string(),
        trust: None,
        meta: None,
    };
    match percept::ingest(input) {
        Ok(r) if !r.duplicate => debug!(
            "[cognition] 感知入库 kind={} chars={} neutralized={}",
            source_kind, r.chars, r.neutralized
        ),
        Ok(_) => {}
        Err(e) => debug!("[cognition] 感知入库跳过（{}）: {}", source_kind, e),
    }
}

/// 逐来源开关。默认策略刻意保守：
/// - 抓取/RSS/采集/文件默认**开**（这些本来就是用户主动配置的采集链路）；
/// - 剪贴板与屏幕默认**关**（涉及用户在别处的活动，必须显式同意）。
pub fn source_enabled(source_kind: &str) -> bool {
    let default_on = matches!(source_kind, "rss" | "crawl" | "capture" | "file" | "manual" | "chat");
    crate::cognition::store::get_bool(&format!("percept_source_{source_kind}"), default_on)
}

pub fn set_source_enabled(source_kind: &str, on: bool) -> Result<(), String> {
    crate::cognition::store::set_setting(
        &format!("percept_source_{source_kind}"),
        if on { "1" } else { "0" },
    )
}

/// 全部可配置来源（设置页渲染开关列表用）。
pub const PERCEPT_SOURCES: [&str; 7] =
    ["crawl", "rss", "capture", "file", "clipboard", "screen", "manual"];
