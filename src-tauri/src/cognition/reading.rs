//! 阅读情节：把活动总线里的「打开 / 阅读 / 读完」整理成可召回的情节记忆。
//!
//! 这是 M2 里「阅读行为接进记忆」的那一半。规则刻意保守：
//! - 一次打开到读完之间算一段情节（`episodes.kind='read'`），问题就是书名/文件名，
//!   收尾时 `outcome='ok'`，这样打分循环会给它求向量，之后的召回能像「历史问答」
//!   一样想起「我读过这本书」。
//! - 进度步骤按 5% 或 10 分钟节流，不会把最后 3% 刷成几十条步骤。
//! - 读完（progress ≥ 0.98 或显式 finish）时推高一次兴趣画像；同一目标 6 小时内
//!   不重复开新情节，避免停在末页反复触发「读完」。
//! - 没有收尾的阅读情节由日常维护以 `abandoned` 关闭，不会永远挂在 running。

use std::collections::HashMap;

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde_json::json;
use tracing::{debug, warn};

use crate::cognition::activity::ActivityEvent;
use crate::cognition::store::{ensure_db, now_iso};
use crate::cognition::{episode, runtime};

/// 同一目标读完后的冷却窗口：窗口内的后续进度事件不再开新情节。
const FINISH_COOLDOWN_MS: i64 = 6 * 3600 * 1000;
/// 进度步骤的最小间隔与最小增量。
const STEP_MIN_INTERVAL_MS: i64 = 10 * 60 * 1000;
const STEP_MIN_PROGRESS_DELTA: f64 = 0.05;
/// 判定「读完」的进度阈值。
const FINISH_PROGRESS: f64 = 0.98;
/// 兴趣强化的相似度闸门。
const REINFORCE_SIM_GATE: f32 = 0.45;

#[derive(Debug, Clone)]
struct ReadingSession {
    episode_id: String,
    kb: String,
    title: String,
    path: String,
    started_ms: i64,
    last_step_ms: i64,
    last_progress: f64,
}

#[derive(Debug, Clone)]
pub struct FinishedBook {
    pub kb: String,
    pub title: String,
    pub path: String,
}

static SESSIONS: Lazy<Mutex<HashMap<String, ReadingSession>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static FINISHED_AT: Lazy<Mutex<HashMap<String, i64>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn now_ms() -> i64 {
    chrono::Local::now().timestamp_millis()
}

fn meta_str(event: &ActivityEvent, key: &str) -> String {
    event
        .meta
        .as_ref()
        .and_then(|m| m.get(key))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn basename(path: &str) -> String {
    path.replace('\\', "/")
        .rsplit('/')
        .find(|p| !p.is_empty())
        .unwrap_or("")
        .to_string()
}

fn title_of(event: &ActivityEvent) -> String {
    let name = meta_str(event, "fileName");
    if !name.is_empty() {
        return name;
    }
    let name = meta_str(event, "importFileName");
    if !name.is_empty() {
        return name;
    }
    let path = event
        .target_id
        .as_deref()
        .or(event.path.as_deref())
        .unwrap_or("");
    basename(path)
}

fn target_key(event: &ActivityEvent) -> Option<String> {
    event
        .target_id
        .clone()
        .or_else(|| event.path.clone())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn in_cooldown(key: &str, now: i64) -> bool {
    let mut finished = FINISHED_AT.lock();
    // 顺手清理过期记录，避免这张表只增不减。
    finished.retain(|_, at| now - *at < FINISH_COOLDOWN_MS);
    finished.get(key).map(|at| now - *at < FINISH_COOLDOWN_MS).unwrap_or(false)
}

/// 取或建一段阅读情节。`open` 事件带来书名与库；`read` 事件缺这些信息时用兜底值。
fn ensure_session(key: &str, event: &ActivityEvent) -> ReadingSession {
    if let Some(existing) = SESSIONS.lock().get(key).cloned() {
        return existing;
    }
    let title = {
        let t = title_of(event);
        if t.is_empty() { "未命名文档".to_string() } else { t }
    };
    let kb = event.kb_name.clone().unwrap_or_default();
    let path = event
        .target_id
        .clone()
        .or_else(|| event.path.clone())
        .unwrap_or_default();
    let episode_id = episode::start("read", "", "", &title, &kb);
    episode::add_step(
        &episode_id,
        "open",
        &title,
        &json!({ "path": path, "kb": kb }).to_string(),
    );
    let session = ReadingSession {
        episode_id,
        kb,
        title,
        path,
        started_ms: now_ms(),
        last_step_ms: 0,
        last_progress: 0.0,
    };
    SESSIONS.lock().insert(key.to_string(), session.clone());
    debug!("[cognition] 阅读情节开始 title={} kb={}", session.title, session.kb);
    session
}

fn finish_session(key: &str, session: ReadingSession) -> FinishedBook {
    let elapsed = (now_ms() - session.started_ms).max(0);
    episode::add_step(
        &session.episode_id,
        "finish",
        "读完",
        &json!({ "path": session.path }).to_string(),
    );
    // outcome 必须是 ok：打分循环只给 ok 的情节补向量，episodic 召回也只取 ok。
    episode::finish(&session.episode_id, "读完", "ok", 0, elapsed);
    FINISHED_AT.lock().insert(key.to_string(), now_ms());
    SESSIONS.lock().remove(key);
    debug!("[cognition] 阅读情节收尾 title={}", session.title);
    FinishedBook {
        kb: session.kb,
        title: session.title,
        path: session.path,
    }
}

/// 处理一批活动事件，返回本次读完的书（需要在异步侧做兴趣强化的那些）。
///
/// 同步、幂等、绝不报错上抛——它挂在活动写入的后面，不能反过来影响记录行为。
pub fn note_events(events: &[ActivityEvent]) -> Vec<FinishedBook> {
    if !runtime::enabled() {
        return Vec::new();
    }
    let mut finished = Vec::new();
    let now = now_ms();
    for event in events {
        let (is_open, is_read) = match (event.app.as_str(), event.kind.as_str()) {
            ("repo", "open") => (true, false),
            ("reader", "read") | ("reader", "finish") => (false, true),
            _ => (false, false),
        };
        if !is_open && !is_read {
            continue;
        }
        let Some(key) = target_key(event) else {
            continue;
        };
        if in_cooldown(&key, now) {
            continue;
        }
        let session = ensure_session(&key, event);
        if is_open {
            continue;
        }

        let progress = event.progress.filter(|p| p.is_finite()).unwrap_or(-1.0);
        let explicit_finish = event.kind == "finish";
        // 至少先见过一次进度，才算「这次真的读了」。否则停在第 100% 重开窗口时，
        // 第一条进度事件就会凭空造出一段「读完」情节。
        let had_progress = session.last_progress > 0.0;
        let due_step = explicit_finish
            || now - session.last_step_ms >= STEP_MIN_INTERVAL_MS
            || (progress >= 0.0 && progress - session.last_progress >= STEP_MIN_PROGRESS_DELTA);

        if due_step {
            let page = event
                .meta
                .as_ref()
                .and_then(|m| m.get("page"))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let total = event
                .meta
                .as_ref()
                .and_then(|m| m.get("total"))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            episode::add_step(
                &session.episode_id,
                "progress",
                &format!(
                    "{}%",
                    if progress >= 0.0 {
                        (progress * 100.0).round().clamp(0.0, 100.0) as i64
                    } else {
                        0
                    }
                ),
                &json!({
                    "progress": if progress >= 0.0 { Some(progress) } else { None },
                    "page": page,
                    "total": total,
                })
                .to_string(),
            );
        }

        if let Some(existing) = SESSIONS.lock().get_mut(&key) {
            if due_step {
                existing.last_step_ms = now;
            }
            if progress >= 0.0 {
                existing.last_progress = progress;
            }
        }

        if explicit_finish || (had_progress && progress >= FINISH_PROGRESS) {
            let latest = SESSIONS.lock().get(&key).cloned().unwrap_or(session);
            finished.push(finish_session(&key, latest));
        }
    }
    finished
}

/// 日常维护：把超过时限仍未收尾的阅读情节标记为放弃，并清掉过期冷却记录。
pub fn close_stale(max_age_hours: i64) -> Result<usize, String> {
    let hours = max_age_hours.max(1);
    let cutoff = (chrono::Local::now() - chrono::Duration::hours(hours)).to_rfc3339();
    let conn = ensure_db()?;
    let closed = conn
        .execute(
            "UPDATE episodes SET ended_at=?2, outcome='abandoned' \
             WHERE kind='read' AND ended_at IS NULL AND started_at < ?1",
            rusqlite::params![cutoff, now_iso()],
        )
        .map_err(|e| e.to_string())?;

    let cutoff_ms = now_ms() - hours * 3600 * 1000;
    SESSIONS.lock().retain(|_, s| s.started_ms >= cutoff_ms);
    FINISHED_AT.lock().retain(|_, at| now_ms() - *at < FINISH_COOLDOWN_MS);
    if closed > 0 {
        warn!("[cognition] 关闭 {} 段超时未收尾的阅读情节", closed);
    }
    Ok(closed)
}

/// 读完后的兴趣强化：用书名（加库名消歧）找最近的图谱节点，把兴趣权重推高一点。
///
/// 与感知打分时的锚点强化是同一套语义，只是这里没有 percept 可锚——
/// 用户读完一本书这件事本身值得记在画像里。
pub async fn reinforce_finished(books: Vec<FinishedBook>) {
    if books.is_empty() || !runtime::enabled() {
        return;
    }
    for book in books {
        if book.title.trim().chars().count() < 2 {
            continue;
        }
        let query = if book.kb.trim().is_empty() {
            book.title.clone()
        } else {
            format!("{}\n{}", book.title, book.kb)
        };
        let Ok(vecs) = crate::commands::embedding::embed_texts(&[query], false).await else {
            debug!(
                "[cognition] 阅读完成强化跳过（embedding 不可用）title={} path={}",
                book.title, book.path
            );
            continue;
        };
        let Some(qv) = vecs.first() else { continue };

        let kbs: Vec<String> = if book.kb.trim().is_empty() {
            crate::commands::kgraph::list_kb_names()
                .into_iter()
                .take(8)
                .collect()
        } else {
            vec![book.kb.clone()]
        };
        let mut best: Option<(String, String, f32)> = None;
        for kb in kbs {
            let Ok(hits) = crate::commands::kgraph::kg_vec_topk(&kb, qv, 1, None) else {
                continue;
            };
            if let Some((node, sim)) = hits.into_iter().next() {
                if best.as_ref().map(|(_, _, s)| sim > *s).unwrap_or(true) {
                    best = Some((kb, node.id, sim));
                }
            }
        }
        if let Some((kb, node_id, sim)) = best {
            if sim >= REINFORCE_SIM_GATE {
                crate::cognition::profile::reinforce_from_node(&kb, &node_id, (sim as f64).min(0.8));
                debug!(
                    "[cognition] 阅读完成强化兴趣 title={} kb={} sim={:.2} path={}",
                    book.title, kb, sim, book.path
                );
            }
        }
    }
}
