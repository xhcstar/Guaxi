//! 认知层的 Tauri 命令面（前端唯一入口）。
//!
//! 命名统一 `cognition_*`，避免与既有 500 多个命令冲突。所有命令都很薄——
//! 逻辑在各子模块里，这里只做参数适配与错误透传。

use crate::cognition::{belief, brief, episode, evolve, nudge, percept, profile, recall, runtime, store, tools};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 顶层状态：设置页与看板顶部一次拿全。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CognitionStatus {
    pub enabled: bool,
    pub db_path: String,
    pub data_dir: String,
    pub schema_version: i64,
    pub percept: percept::PerceptStats,
    pub episode: episode::EpisodeStats,
    pub nudge: nudge::NudgeStats,
    pub belief: belief::BeliefStats,
    pub eval_items: i64,
    pub recall_weights: std::collections::HashMap<String, f64>,
    pub tiers: Vec<(String, bool)>,
    pub deliberate_interval_secs: i64,
    pub deliberate_modelid: i64,
    pub max_interrupts_per_day: i64,
    pub max_interrupts_per_hour: i64,
    pub episode_keep_days: i64,
    pub nudge_threshold_base: f64,
    pub brief_hour: u32,
    pub quiet_hours: profile::QuietHours,
    /// embedding 服务是否已就绪（召回与打分都依赖它）。
    pub embedding_ready: bool,
}

#[tauri::command]
pub async fn cognition_status() -> Result<CognitionStatus, String> {
    let embedding_ready = crate::commands::embedding::kg_embedding_status()
        .await
        .map(|s| s.ready)
        .unwrap_or(false);
    let prof = profile::load_profile().unwrap_or_default();
    Ok(CognitionStatus {
        enabled: runtime::enabled(),
        db_path: store::cog_db_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        data_dir: store::cog_data_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        schema_version: store::COG_SCHEMA_VERSION,
        percept: percept::stats().unwrap_or_default(),
        episode: episode::stats().unwrap_or_default(),
        nudge: nudge::stats().unwrap_or_default(),
        belief: belief::stats().unwrap_or_default(),
        eval_items: evolve::eval_set_size(),
        recall_weights: recall::channel_weights(),
        tiers: vec![
            ("read".into(), true),
            ("write".into(), tools::tier_granted(tools::Tier::Write)),
            ("network".into(), tools::tier_granted(tools::Tier::Network)),
            ("exec".into(), tools::tier_granted(tools::Tier::Exec)),
            ("gui".into(), tools::tier_granted(tools::Tier::Gui)),
        ],
        deliberate_interval_secs: store::get_i64("deliberate_interval_secs", 300),
        deliberate_modelid: store::get_i64("deliberate_modelid", 6),
        max_interrupts_per_day: store::get_i64("max_interrupts_per_day", 3),
        max_interrupts_per_hour: store::get_i64("max_interrupts_per_hour", 1),
        episode_keep_days: store::get_i64("episode_keep_days", 180),
        nudge_threshold_base: store::get_f64("nudge_threshold_base", 0.5),
        brief_hour: prof.brief_hour,
        quiet_hours: prof.quiet_hours,
        embedding_ready,
    })
}

#[tauri::command]
pub fn cognition_set_enabled(enabled: bool) -> Result<(), String> {
    runtime::set_enabled(enabled)
}

/// 白名单化的设置写入。**不做**任意键透传——那等于把整个行为面开放给前端。
#[tauri::command]
pub fn cognition_set_setting(key: String, value: String) -> Result<(), String> {
    const ALLOWED: [&str; 9] = [
        "deliberate_interval_secs",
        "deliberate_modelid",
        "max_interrupts_per_day",
        "max_interrupts_per_hour",
        "daily_token_budget",
        "nudge_threshold_base",
        "episode_keep_days",
        "percept_daily_cap",
        "percept_unread_keep_days",
    ];
    let k = key.trim();
    if !ALLOWED.contains(&k) {
        return Err(format!("不允许写入的设置项: {k}"));
    }
    store::set_setting(k, value.trim())
}

// ───────────────────────── 感知

#[tauri::command]
pub fn cognition_percept_list(query: Option<percept::PerceptQuery>) -> Result<Vec<percept::Percept>, String> {
    percept::list_percepts(&query.unwrap_or_default())
}

#[tauri::command]
pub fn cognition_percept_ingest(input: percept::PerceptInput) -> Result<percept::IngestResult, String> {
    percept::ingest(input)
}

#[tauri::command]
pub fn cognition_percept_set_state(
    id: String,
    state: String,
    reason: Option<String>,
) -> Result<(), String> {
    percept::set_state(&id, &state, reason.as_deref())
}

/// 取完整正文。返回的 `content` 已包不可信声明块，前端渲染时应给出视觉区分。
#[tauri::command]
pub fn cognition_percept_get(id: String) -> Result<Value, String> {
    let (title, text, uri) = percept::percept_text(&id).ok_or_else(|| "感知不存在".to_string())?;
    Ok(serde_json::json!({ "id": id, "title": title, "uri": uri, "text": text }))
}

/// 手动触发一批打分（设置页的"立即处理"按钮）。
#[tauri::command]
pub async fn cognition_score_percepts(limit: Option<usize>) -> Result<usize, String> {
    percept::score_pending(limit.unwrap_or(24)).await
}

/// 立即清理未读的自动采集内容（设置页的"清理垃圾"按钮）。
///
/// 传 `keepDays=0` 表示把当前所有未读的抓取内容都清掉（被引用过的仍会保留）。
#[tauri::command]
pub fn cognition_purge_percepts(keep_days: Option<i64>) -> Result<usize, String> {
    percept::purge_stale_unread(keep_days)
}

// ───────────────────────── 召回

#[tauri::command]
pub async fn cognition_recall(
    query: String,
    options: Option<recall::RecallOptions>,
) -> Result<recall::RecallResult, String> {
    Ok(recall::recall(&query, options.unwrap_or_default()).await)
}

// ───────────────────────── 主动提醒

#[tauri::command]
pub fn cognition_nudge_list(
    state: Option<String>,
    kind: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<nudge::Nudge>, String> {
    nudge::list_nudges(state.as_deref(), kind.as_deref(), limit.unwrap_or(40))
}

#[tauri::command]
pub fn cognition_nudge_decide(id: String, accepted: bool) -> Result<(), String> {
    nudge::record_decision(&id, accepted)
}

#[tauri::command]
pub async fn cognition_deliberate_now(
    app: tauri::AppHandle,
    modelid: Option<i32>,
) -> Result<nudge::DeliberateReport, String> {
    let m = modelid.unwrap_or_else(|| store::get_i64("deliberate_modelid", 6) as i32);
    Ok(nudge::deliberate(Some(&app), m).await)
}

#[tauri::command]
pub fn cognition_review_feedback(node_id: String, remembered: bool) -> Result<(), String> {
    nudge::review_feedback(&node_id, remembered)
}

#[tauri::command]
pub fn cognition_policy_train() -> Result<nudge::TrainReport, String> {
    nudge::train_policy()
}

#[tauri::command]
pub fn cognition_policy_reset() -> Result<(), String> {
    nudge::reset_policy()
}

// ───────────────────────── 简报

#[tauri::command]
pub fn cognition_brief_get(day: Option<String>, kind: Option<String>) -> Result<Option<brief::Brief>, String> {
    brief::get(
        &day.unwrap_or_else(store::today),
        &kind.unwrap_or_else(|| "morning".into()),
    )
}

#[tauri::command]
pub async fn cognition_brief_generate(
    app: tauri::AppHandle,
    kind: Option<String>,
    modelid: Option<i32>,
) -> Result<brief::Brief, String> {
    let m = modelid.unwrap_or_else(|| store::get_i64("deliberate_modelid", 6) as i32);
    brief::generate(Some(&app), m, &kind.unwrap_or_else(|| "morning".into())).await
}

#[tauri::command]
pub fn cognition_brief_list(limit: Option<usize>) -> Result<Vec<Value>, String> {
    brief::list(limit.unwrap_or(30))
}

#[tauri::command]
pub fn cognition_brief_mark_opened(day: String, kind: Option<String>) -> Result<(), String> {
    brief::mark_opened(&day, &kind.unwrap_or_else(|| "morning".into()))
}

// ───────────────────────── 画像

#[tauri::command]
pub fn cognition_profile_get() -> Result<profile::Profile, String> {
    profile::load_profile()
}

#[tauri::command]
pub fn cognition_profile_save(markdown: String) -> Result<profile::Profile, String> {
    profile::save_profile(&markdown)
}

#[tauri::command]
pub fn cognition_interest_upsert(topic: String, weight: f64) -> Result<(), String> {
    profile::upsert_interest(&topic, weight)
}

#[tauri::command]
pub fn cognition_interest_remove(topic: String) -> Result<usize, String> {
    profile::remove_interest(&topic)
}

// ───────────────────────── 信念

#[tauri::command]
pub fn cognition_belief_list(
    kb_name: Option<String>,
    status: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<belief::Belief>, String> {
    belief::list_beliefs(kb_name.as_deref(), status.as_deref(), limit.unwrap_or(80))
}

#[tauri::command]
pub async fn cognition_belief_extract(
    kb_name: String,
    modelid: Option<i32>,
    max_sections: Option<usize>,
) -> Result<belief::ExtractResult, String> {
    let m = modelid.unwrap_or_else(|| store::get_i64("deliberate_modelid", 6) as i32);
    belief::extract_beliefs(&kb_name, m, max_sections.unwrap_or(20)).await
}

#[tauri::command]
pub fn cognition_belief_set_status(id: String, status: String) -> Result<(), String> {
    belief::set_status(&id, &status)
}

#[tauri::command]
pub fn cognition_belief_remove(id: String) -> Result<(), String> {
    belief::remove(&id)
}

#[tauri::command]
pub fn cognition_belief_evidence(belief_id: String) -> Result<Vec<Value>, String> {
    belief::evidence_of(&belief_id)
}

#[tauri::command]
pub async fn cognition_detect_contradictions(
    modelid: Option<i32>,
    max_judge: Option<usize>,
) -> Result<Vec<belief::Contradiction>, String> {
    let m = modelid.unwrap_or_else(|| store::get_i64("deliberate_modelid", 6) as i32);
    belief::detect_contradictions(m, max_judge.unwrap_or(4)).await
}

// ───────────────────────── 情节与引证

#[tauri::command]
pub fn cognition_episode_recent(
    kind: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<episode::EpisodeRow>, String> {
    episode::recent(limit.unwrap_or(30), kind.as_deref())
}

#[tauri::command]
pub fn cognition_episode_steps(episode_id: String) -> Result<Vec<Value>, String> {
    episode::steps(&episode_id)
}

#[tauri::command]
pub fn cognition_episode_citations(episode_id: String) -> Result<Vec<Value>, String> {
    episode::citations_of(&episode_id)
}

/// 前端上报"用户点开了这条引证"。这是检索权重学习最强的正标签。
#[tauri::command]
pub fn cognition_citation_clicked(episode_id: String, ref_id: String) -> Result<(), String> {
    episode::mark_citation_clicked(&episode_id, &ref_id)
}

// ───────────────────────── 工具面与 MCP

#[tauri::command]
pub fn cognition_tools_list() -> Vec<tools::ToolSpec> {
    tools::list_tools()
}

#[tauri::command]
pub fn cognition_tool_set_tier(tier: String, granted: bool) -> Result<(), String> {
    let t = match tier.trim() {
        "write" => tools::Tier::Write,
        "network" => tools::Tier::Network,
        "exec" => tools::Tier::Exec,
        "read" => tools::Tier::Read,
        other => return Err(format!("未知等级: {other}")),
    };
    tools::set_tier_granted(t, granted)
}

/// 前端直接调用某个工具（设置页的"试一下"按钮，也供未来的 UI 自动化用）。
#[tauri::command]
pub async fn cognition_tool_call(name: String, args: Option<Value>) -> Result<Value, String> {
    tools::dispatch("ui", &name, &args.unwrap_or(serde_json::json!({})), false).await
}

#[tauri::command]
pub fn cognition_tool_audit(limit: Option<usize>) -> Result<Vec<Value>, String> {
    let conn = store::ensure_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT at,caller,tool,tier,COALESCE(args,''),ok,ms,COALESCE(error,'') \
             FROM tool_audit ORDER BY at DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![limit.unwrap_or(100).clamp(1, 1000) as i64], |r| {
            Ok(serde_json::json!({
                "at": r.get::<_, String>(0)?,
                "caller": r.get::<_, String>(1)?,
                "tool": r.get::<_, String>(2)?,
                "tier": r.get::<_, String>(3)?,
                "args": r.get::<_, String>(4)?,
                "ok": r.get::<_, i64>(5)? == 1,
                "ms": r.get::<_, i64>(6)?,
                "error": r.get::<_, String>(7)?,
            }))
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|x| x.ok()).collect())
}

/// MCP 接入信息：给用户复制到外部客户端（Claude Code / Cursor）的配置片段。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInfo {
    pub endpoint: String,
    pub bridge_path: String,
    pub bridge_exists: bool,
    pub tools_total: usize,
    pub tools_enabled: usize,
    /// 可直接粘贴的外部客户端配置（JSON）。
    pub client_config: String,
}

#[tauri::command]
pub fn cognition_mcp_info() -> Result<McpInfo, String> {
    let bridge = bridge_binary_path();
    let list = tools::list_tools();
    let bridge_str = bridge.to_string_lossy().to_string();
    let client_config = serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": {
            "guaxi": { "command": bridge_str, "args": [] }
        }
    }))
    .unwrap_or_default();
    Ok(McpInfo {
        endpoint: "http://127.0.0.1:8888/mcp".to_string(),
        bridge_path: bridge_str,
        bridge_exists: bridge.is_file(),
        tools_total: list.len(),
        tools_enabled: list.iter().filter(|t| t.enabled).count(),
        client_config,
    })
}

/// stdio 桥接可执行文件的位置：与主程序同目录。
pub fn bridge_binary_path() -> std::path::PathBuf {
    let name = if cfg!(windows) { "guaxi_mcp.exe" } else { "guaxi_mcp" };
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(name)))
        .unwrap_or_else(|| std::path::PathBuf::from(name))
}

// ───────────────────────── 进化闩（评测集 / 调参 / 回滚）

#[tauri::command]
pub async fn cognition_eval_mine(
    kb_name: String,
    modelid: Option<i32>,
    target_n: Option<usize>,
) -> Result<evolve::MineReport, String> {
    let m = modelid.unwrap_or_else(|| store::get_i64("deliberate_modelid", 6) as i32);
    evolve::mine_eval_set(&kb_name, m, target_n.unwrap_or(40)).await
}

#[tauri::command]
pub async fn cognition_eval_run(limit: Option<usize>) -> Result<evolve::EvalRun, String> {
    evolve::run_eval(limit.unwrap_or(200)).await
}

#[tauri::command]
pub async fn cognition_eval_tune(limit: Option<usize>) -> Result<evolve::EvalRun, String> {
    evolve::tune_recall_weights(limit.unwrap_or(200)).await
}

#[tauri::command]
pub fn cognition_eval_runs(
    target: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<evolve::EvalRun>, String> {
    evolve::list_runs(target.as_deref(), limit.unwrap_or(50))
}

#[tauri::command]
pub fn cognition_recall_weights_rollback() -> Result<(), String> {
    evolve::rollback_recall_weights()
}

// ───────────────────────── 维护

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceRequest {
    #[serde(default)]
    pub decay_interests: bool,
    #[serde(default)]
    pub prune_episodes: bool,
    #[serde(default)]
    pub expire_nudges: bool,
    #[serde(default)]
    pub rederive_profile: bool,
}

/// 对话流结束后由前端上报完整答案。
///
/// 为什么不在后端收：流式回复是逐段 emit 给前端的，后端要拿到全文就得在
/// 每个 provider 的流循环里插一段累积逻辑（十来处），代价与风险都远高于让前端回报一次。
/// 而拿到全文的价值很实在——它决定了"哪条引证真被用上"，也就是检索权重学习的标签。
#[tauri::command]
pub fn cognition_chat_finish(
    episode_id: String,
    answer: String,
    outcome: Option<String>,
    tokens_out: Option<i64>,
    latency_ms: Option<i64>,
) -> Result<(), String> {
    if episode_id.trim().is_empty() {
        return Ok(());
    }
    episode::finish(
        &episode_id,
        &answer,
        &outcome.unwrap_or_else(|| "ok".into()),
        tokens_out.unwrap_or(0),
        latency_ms.unwrap_or(0),
    );
    Ok(())
}

/// 感知来源开关（剪贴板/屏幕这类默认关闭，必须显式同意）。
#[tauri::command]
pub fn cognition_sources() -> Vec<(String, bool)> {
    crate::cognition::hooks::PERCEPT_SOURCES
        .iter()
        .map(|s| (s.to_string(), crate::cognition::hooks::source_enabled(s)))
        .collect()
}

#[tauri::command]
pub fn cognition_set_source_enabled(source_kind: String, enabled: bool) -> Result<(), String> {
    crate::cognition::hooks::set_source_enabled(source_kind.trim(), enabled)
}

/// 前端主动送一段内容进感知（剪贴板监听、划词、手动粘贴都走它）。
#[tauri::command]
pub fn cognition_capture(
    source_kind: String,
    text: String,
    title: Option<String>,
    uri: Option<String>,
) -> Result<percept::IngestResult, String> {
    if !crate::cognition::hooks::source_enabled(source_kind.trim()) {
        return Err(format!("来源 {source_kind} 未启用，请先在设置里开启"));
    }
    percept::ingest(percept::PerceptInput {
        source_kind: source_kind.trim().to_string(),
        source_id: String::new(),
        uri: uri.unwrap_or_default(),
        title: title.unwrap_or_default(),
        text,
        kb_name: String::new(),
        trust: None,
        meta: None,
    })
}

/// 单条提醒详情（前端从通知点进来时用 id 直取）。
#[tauri::command]
pub fn cognition_nudge_get(id: String) -> Result<Option<nudge::Nudge>, String> {
    nudge::get_nudge(&id)
}

/// 各召回通路的历史命中率：`[{channel, hits, total, rate}]`。
///
/// 这是"检索到底哪一路在起作用"的答案，也是调参前该先看的东西——
/// 如果某一路长期零命中，问题多半在数据（没建向量、没有 percept）而不在权重。
#[tauri::command]
pub fn cognition_channel_stats(days: Option<i64>) -> Vec<Value> {
    episode::channel_stats(days.unwrap_or(30))
        .into_iter()
        .map(|(channel, hits, total)| {
            let rate = if total > 0 { hits as f64 / total as f64 } else { 0.0 };
            serde_json::json!({ "channel": channel, "hits": hits, "total": total, "rate": rate })
        })
        .collect()
}

#[tauri::command]
pub fn cognition_maintenance(request: MaintenanceRequest) -> Result<Value, String> {
    let mut out = serde_json::Map::new();
    if request.decay_interests {
        out.insert("decayed".into(), profile::decay_interests()?.into());
    }
    if request.prune_episodes {
        let keep = store::get_i64("episode_keep_days", 180);
        out.insert("pruned".into(), episode::prune(keep)?.into());
    }
    if request.expire_nudges {
        out.insert("expired".into(), nudge::expire_stale()?.into());
    }
    if request.rederive_profile {
        out.insert("interests".into(), profile::rederive_from_markdown()?.into());
    }
    Ok(Value::Object(out))
}
