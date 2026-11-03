//! 活动总线：append-only 的用户行为轨迹（打开 / 阅读 / 摘录 / 搜索 / 保存）。
//!
//! 与 `episodes` 的分工：episodes 记「系统做过什么」（对话、Agent 步骤、投递、
//! 简报），activity 记「人做过什么」（读了哪章、摘了什么、搜了什么、改了哪个文件）。
//! 这一层只回答「发生过什么」，不做任何判断——会话化、记忆沉淀、学习状态都是
//! 后置任务，不在写入路径上跑。
//!
//! 写入纪律：前端批量提交；单条字段裁剪长度；失败只落日志、绝不把错误抛回 UI。
//! 记录行为不能拖垮主流程，这是这一层唯一重要的约束。

use rusqlite::{params, params_from_iter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::warn;

use crate::cognition::store::{ensure_db, now_iso};

/// 短字段（app / kind / target_kind / kb_name / session_id）的上限。
const MAX_SHORT_CHARS: usize = 64;
/// 目标与路径的上限。Windows 长路径之外的东西也没有分析价值。
const MAX_PATH_CHARS: usize = 2048;
/// meta JSON 的上限：前端额外信息只用于调试与后置分析，不需要整篇正文。
const MAX_META_CHARS: usize = 4096;
/// 默认保留天数。
pub const DEFAULT_KEEP_DAYS: i64 = 180;

/// 一条活动事件。字段全部可缺省，只有 `app` / `kind` 是必需的。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    /// 发生时刻（RFC3339）。缺省由后端补当前时间。
    #[serde(default)]
    pub at: Option<String>,
    /// 哪个面：repo / editor / reader / mail / search / task / git ...
    pub app: String,
    /// 发生了什么：open / read / edit / note / highlight / search / convert / quiz ...
    pub kind: String,
    /// 目标类型：book / file / node / url / query / kb。
    #[serde(default)]
    pub target_kind: Option<String>,
    /// 目标标识：绝对路径、节点 id、URL、查询串。
    #[serde(default)]
    pub target_id: Option<String>,
    #[serde(default)]
    pub kb_name: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub line: Option<i64>,
    /// 本次行为持续时长（毫秒）。打开/搜索这类瞬时事件不带。
    #[serde(default)]
    pub duration_ms: Option<i64>,
    /// 0..1 的进度（阅读比例、书页比例），用于后置的覆盖率统计。
    #[serde(default)]
    pub progress: Option<f64>,
    /// 一次窗口/标签生命周期内的会话 id，缺省由前端生成。
    #[serde(default)]
    pub session_id: Option<String>,
    /// 附加信息（文件名、结果条数、卡片类型等）。
    #[serde(default)]
    pub meta: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRow {
    pub id: i64,
    pub at: String,
    pub app: String,
    pub kind: String,
    pub target_kind: Option<String>,
    pub target_id: Option<String>,
    pub kb_name: Option<String>,
    pub path: Option<String>,
    pub line: Option<i64>,
    pub duration_ms: Option<i64>,
    pub progress: Option<f64>,
    pub meta: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActivityStats {
    pub total: i64,
    pub today: i64,
    pub window_days: i64,
    pub by_app: Vec<(String, i64)>,
    pub by_kind: Vec<(String, i64)>,
}

fn clip(value: &str, max: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    trimmed.chars().take(max).collect()
}

fn opt_clip(value: Option<&str>, max: usize) -> Option<String> {
    value
        .map(|v| clip(v, max))
        .filter(|v| !v.is_empty())
}

/// 规范化之后、准备落库的一行。
struct Normalized {
    at: Option<String>,
    app: String,
    kind: String,
    target_kind: Option<String>,
    target_id: Option<String>,
    kb_name: Option<String>,
    path: Option<String>,
    line: Option<i64>,
    duration_ms: Option<i64>,
    progress: Option<f64>,
    session_id: Option<String>,
    meta: Option<String>,
}

/// 规范化一条事件；`app` / `kind` 为空时返回 None（这条直接丢弃）。
fn normalize(event: &ActivityEvent) -> Option<Normalized> {
    let app = clip(&event.app, MAX_SHORT_CHARS);
    let kind = clip(&event.kind, MAX_SHORT_CHARS);
    if app.is_empty() || kind.is_empty() {
        return None;
    }
    Some(Normalized {
        at: opt_clip(event.at.as_deref(), MAX_SHORT_CHARS),
        app,
        kind,
        target_kind: opt_clip(event.target_kind.as_deref(), MAX_SHORT_CHARS),
        target_id: opt_clip(event.target_id.as_deref(), MAX_PATH_CHARS),
        kb_name: opt_clip(event.kb_name.as_deref(), MAX_SHORT_CHARS),
        path: opt_clip(event.path.as_deref(), MAX_PATH_CHARS),
        line: event.line.filter(|v| *v >= 0),
        duration_ms: event.duration_ms.filter(|v| *v > 0).map(|v| v.min(24 * 3600 * 1000)),
        progress: event.progress.filter(|v| v.is_finite()).map(|v| v.clamp(0.0, 1.0)),
        session_id: opt_clip(event.session_id.as_deref(), MAX_SHORT_CHARS),
        meta: event.meta.as_ref().and_then(|value| {
            let text = value.to_string();
            if text.len() > MAX_META_CHARS {
                None
            } else {
                Some(text)
            }
        }),
    })
}

/// 批量写入。返回实际写入的条数。
pub fn log_events(events: Vec<ActivityEvent>) -> Result<usize, String> {
    if events.is_empty() {
        return Ok(0);
    }
    let mut conn = ensure_db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut written = 0usize;
    for event in &events {
        let Some(row) = normalize(event) else {
            continue;
        };
        tx.execute(
            "INSERT INTO activity(at,app,kind,target_kind,target_id,kb_name,path,line,\
             duration_ms,progress,session_id,meta) \
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                row.at.unwrap_or_else(now_iso),
                row.app,
                row.kind,
                row.target_kind,
                row.target_id,
                row.kb_name,
                row.path,
                row.line,
                row.duration_ms,
                row.progress,
                row.session_id,
                row.meta
            ],
        )
        .map_err(|e| e.to_string())?;
        written += 1;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(written)
}

/// 最近的活动，按时间倒序。`app` / `kind` 用于调试视图的筛选。
pub fn recent(
    limit: Option<usize>,
    app: Option<&str>,
    kind: Option<&str>,
) -> Result<Vec<ActivityRow>, String> {
    let limit = limit.unwrap_or(50).clamp(1, 500) as i64;
    let mut sql = String::from(
        "SELECT id,at,app,kind,target_kind,target_id,kb_name,path,line,duration_ms,progress,meta \
         FROM activity WHERE 1=1",
    );
    let mut values: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(app) = opt_clip(app, MAX_SHORT_CHARS) {
        sql.push_str(" AND app=?");
        values.push(rusqlite::types::Value::Text(app));
    }
    if let Some(kind) = opt_clip(kind, MAX_SHORT_CHARS) {
        sql.push_str(" AND kind=?");
        values.push(rusqlite::types::Value::Text(kind));
    }
    sql.push_str(" ORDER BY id DESC LIMIT ?");
    values.push(rusqlite::types::Value::Integer(limit));

    let conn = ensure_db()?;
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(values), |r| {
            Ok(ActivityRow {
                id: r.get(0)?,
                at: r.get(1)?,
                app: r.get(2)?,
                kind: r.get(3)?,
                target_kind: r.get(4)?,
                target_id: r.get(5)?,
                kb_name: r.get(6)?,
                path: r.get(7)?,
                line: r.get(8)?,
                duration_ms: r.get(9)?,
                progress: r.get(10)?,
                meta: r.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// 概览：总量、今日量、窗口内的 app/kind 分布。
pub fn stats(days: Option<i64>) -> Result<ActivityStats, String> {
    let window_days = days.unwrap_or(7).clamp(1, 365);
    let conn = ensure_db()?;
    let today = crate::cognition::store::today();
    let cutoff = (chrono::Local::now() - chrono::Duration::days(window_days)).to_rfc3339();

    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM activity", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let today_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM activity WHERE substr(at,1,10)=?1",
            params![today],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    let collect = |sql: &str| -> Result<Vec<(String, i64)>, String> {
        let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![cutoff], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?;
        let mut list = Vec::new();
        for row in rows {
            list.push(row.map_err(|e| e.to_string())?);
        }
        Ok(list)
    };
    let by_app = collect(
        "SELECT app,COUNT(*) FROM activity WHERE at>=?1 GROUP BY app ORDER BY 2 DESC LIMIT 12",
    )?;
    let by_kind = collect(
        "SELECT kind,COUNT(*) FROM activity WHERE at>=?1 GROUP BY kind ORDER BY 2 DESC LIMIT 12",
    )?;

    Ok(ActivityStats {
        total,
        today: today_count,
        window_days,
        by_app,
        by_kind,
    })
}

/// 按保留期裁剪。与 episode 的裁剪同一节奏（每日维护跑一次）。
pub fn prune(keep_days: i64) -> Result<usize, String> {
    let keep = keep_days.max(1);
    let cutoff = (chrono::Local::now() - chrono::Duration::days(keep)).to_rfc3339();
    let conn = ensure_db()?;
    let removed = conn
        .execute("DELETE FROM activity WHERE at < ?1", params![cutoff])
        .map_err(|e| e.to_string())?;
    if removed > 0 {
        warn!("[cognition] 裁剪过期活动 {} 条（保留 {} 天）", removed, keep);
    }
    Ok(removed)
}
