//! 学习状态层：主题掌握度、复习队列与学习概览。
//!
//! 数据来源只有三处，刻意不让它自己造信号：
//! - 阅读活动（`activity` 里 app=reader 的进度事件）→ 材料级覆盖度与投入；
//! - 阅读情节（`episodes` 里 kind=read 且 outcome=ok）→ 读完与耗时；
//! - 复习队列（`review_state`）→ 节点级保持率与可靠性。
//!
//! 掌握度是一个可解释的加权分，不是黑箱：
//! - 材料：`0.5×覆盖 + 0.2×投入 + 0.3×读完`；
//! - 节点：`0.6×FSRS 保持率 + 0.4×可靠性`（记住次数 / 总次数）。
//! 置信度随「样本量」（阅读场次、复习次数）增长；看板会把低置信度的条目排在后面，
//! 避免只读过一次的书被当成「已掌握」或「待加强」。

use std::collections::{HashMap, HashSet};

use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::json;
use tracing::debug;

use crate::cognition::fsrs;
use crate::cognition::store::{ensure_db, now_iso, today};

/// 掌握度统计的阅读窗口。
const WINDOW_DAYS: i64 = 180;
/// 概览触发刷新时，距上次刷新至少多久（秒）。
const REFRESH_THROTTLE_SECS: i64 = 600;
/// 掌握 / 待加强的两条分界。
const MASTERED_AT: f64 = 0.7;
const WEAK_BELOW: f64 = 0.35;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicRow {
    pub key: String,
    pub topic: String,
    /// material | node
    pub source: String,
    pub kb_name: Option<String>,
    pub ref_id: Option<String>,
    pub coverage: f64,
    pub retention: f64,
    pub engagement: f64,
    pub mastery: f64,
    pub confidence: f64,
    pub sessions: i64,
    pub finished: bool,
    pub reps: i64,
    pub lapses: i64,
    pub last_touched_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DueReview {
    pub node_id: String,
    pub kb_name: String,
    pub label: String,
    pub due_date: String,
    pub reps: i64,
    pub lapses: i64,
    pub stability: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishedItem {
    pub title: String,
    pub ended_at: Option<String>,
    pub minutes: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LearningOverview {
    pub days: i64,
    pub updated_at: String,
    pub topics_total: i64,
    pub mastered: i64,
    pub learning: i64,
    pub weak: i64,
    pub finished_books: i64,
    pub reading_minutes: i64,
    pub streak_days: i64,
    pub due_reviews: i64,
    pub overdue_reviews: i64,
    pub topics: Vec<TopicRow>,
    pub due: Vec<DueReview>,
    pub recent_finished: Vec<FinishedItem>,
}

fn basename(path: &str) -> String {
    path.replace('\\', "/")
        .rsplit('/')
        .find(|p| !p.is_empty())
        .unwrap_or("")
        .to_string()
}

fn strip_ext(name: &str) -> String {
    let trimmed = name.trim();
    match trimmed.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && ext.len() <= 8 => stem.to_string(),
        _ => trimmed.to_string(),
    }
}

fn normalize_title(raw: &str) -> String {
    strip_ext(&basename(raw)).trim().to_lowercase()
}

fn meta_title(meta: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(meta) else {
        return String::new();
    };
    value
        .get("fileName")
        .or_else(|| value.get("importFileName"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn days_since(iso: &str) -> f64 {
    let Ok(time) = chrono::DateTime::parse_from_rfc3339(iso) else {
        return 0.0;
    };
    let delta = chrono::Local::now().signed_duration_since(time.with_timezone(&chrono::Local));
    (delta.num_seconds() as f64 / 86400.0).max(0.0)
}

#[derive(Debug, Clone)]
struct MaterialAgg {
    title: String,
    path: String,
    kb: String,
    max_progress: f64,
    days: HashSet<String>,
    last_at: String,
}

fn upsert_topic(
    conn: &rusqlite::Connection,
    key: &str,
    topic: &str,
    source: &str,
    kb: Option<&str>,
    ref_id: Option<&str>,
    coverage: f64,
    retention: f64,
    engagement: f64,
    mastery: f64,
    confidence: f64,
    sessions: i64,
    finished: bool,
    reps: i64,
    lapses: i64,
    last_touched: Option<&str>,
    extra: serde_json::Value,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO topic_mastery(topic_key,topic,source,kb_name,ref_id,coverage,retention,\
         engagement,mastery,confidence,sessions,finished,reps,lapses,last_touched_at,updated_at,extra) \
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17) \
         ON CONFLICT(topic_key) DO UPDATE SET \
           topic=excluded.topic, source=excluded.source, kb_name=excluded.kb_name, \
           ref_id=excluded.ref_id, coverage=excluded.coverage, retention=excluded.retention, \
           engagement=excluded.engagement, mastery=excluded.mastery, \
           confidence=excluded.confidence, sessions=excluded.sessions, \
           finished=excluded.finished, reps=excluded.reps, lapses=excluded.lapses, \
           last_touched_at=excluded.last_touched_at, updated_at=excluded.updated_at, \
           extra=excluded.extra",
        params![
            key,
            topic,
            source,
            kb,
            ref_id,
            coverage,
            retention,
            engagement,
            mastery,
            confidence,
            sessions,
            if finished { 1 } else { 0 },
            reps,
            lapses,
            last_touched,
            now_iso(),
            extra.to_string()
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 重算全部掌握度。每日维护调用一次；设置页/看板的刷新按钮也会触发。
pub fn refresh_mastery() -> Result<usize, String> {
    let conn = ensure_db()?;
    let cutoff = (chrono::Local::now() - chrono::Duration::days(WINDOW_DAYS)).to_rfc3339();

    // ── 材料：阅读活动聚合 ──
    let mut materials: HashMap<String, MaterialAgg> = HashMap::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT COALESCE(target_id,''),COALESCE(kb_name,''),COALESCE(meta,'{}'),\
                 COALESCE(progress,0),at FROM activity WHERE app='reader' AND kind='read' \
                 AND at>=?1 AND COALESCE(target_id,'')<>'' ORDER BY at ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![cutoff], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, f64>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for (path, kb, meta, progress, at) in rows.flatten() {
            let key = format!("material:{}", path.to_lowercase());
            let title = {
                let from_meta = meta_title(&meta);
                if from_meta.is_empty() {
                    strip_ext(&basename(&path))
                } else {
                    strip_ext(&from_meta)
                }
            };
            let day = at.get(0..10).unwrap_or("").to_string();
            let agg = materials.entry(key).or_insert_with(|| MaterialAgg {
                title,
                path: path.clone(),
                kb: kb.clone(),
                max_progress: 0.0,
                days: HashSet::new(),
                last_at: at.clone(),
            });
            agg.max_progress = agg.max_progress.max(progress.clamp(0.0, 1.0));
            if !day.is_empty() {
                agg.days.insert(day);
            }
            if at > agg.last_at {
                agg.last_at = at;
            }
            if agg.kb.is_empty() && !kb.is_empty() {
                agg.kb = kb;
            }
        }
    }

    // ── 材料：读完的情节（按标题回到对应材料）──
    let mut finished_by_title: HashMap<String, (i64, Option<String>, i64)> = HashMap::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT COALESCE(question,''),COALESCE(ended_at,''),COALESCE(started_at,''),\
                 COALESCE(latency_ms,0) FROM episodes WHERE kind='read' AND outcome='ok' \
                 AND started_at>=?1 ORDER BY started_at DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![cutoff], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for (title, ended, started, latency) in rows.flatten() {
            let norm = normalize_title(&title);
            if norm.is_empty() {
                continue;
            }
            let entry = finished_by_title
                .entry(norm)
                .or_insert((0, None, 0));
            entry.0 += 1;
            let when = if ended.is_empty() { started } else { ended };
            if entry
                .1
                .as_deref()
                .map(|t| when.as_str() > t)
                .unwrap_or(true)
            {
                entry.1 = Some(when);
            }
            entry.2 += latency;
        }
    }

    let mut written = 0usize;
    for (key, agg) in materials.iter() {
        let norm = normalize_title(&agg.title);
        let episode = finished_by_title.get(&norm);
        let sessions = agg.days.len() as i64;
        let finished = agg.max_progress >= 0.98 || episode.is_some();
        let engagement = (sessions as f64 / 5.0).clamp(0.0, 1.0);
        let mastery = (0.5 * agg.max_progress
            + 0.2 * engagement
            + 0.3 * if finished { 1.0 } else { 0.0 })
            .clamp(0.0, 1.0);
        let confidence = (((sessions + if finished { 2 } else { 0 } + 1) as f64) / 6.0)
            .clamp(0.0, 1.0);
        let last_touched = episode
            .and_then(|(_, at, _)| at.clone())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| agg.last_at.clone());
        upsert_topic(
            &conn,
            key,
            &agg.title,
            "material",
            Some(agg.kb.as_str()).filter(|k| !k.is_empty()),
            Some(agg.path.as_str()),
            agg.max_progress,
            0.0,
            engagement,
            mastery,
            confidence,
            sessions,
            finished,
            0,
            0,
            Some(last_touched.as_str()),
            json!({ "path": agg.path }),
        )?;
        written += 1;
    }

    // 读完但没有阅读进度事件的材料（例如在别的设备上读完）也要出现在看板里。
    for (norm, (count, ended, _)) in finished_by_title.iter() {
        if *count <= 0 {
            continue;
        }
        let exists = materials
            .values()
            .any(|m| normalize_title(&m.title) == *norm);
        if exists {
            continue;
        }
        let key = format!("episode:{}", norm);
        let title = norm.clone();
        upsert_topic(
            &conn,
            &key,
            &title,
            "material",
            None,
            Some(norm.as_str()),
            1.0,
            0.0,
            0.4,
            0.8,
            0.45,
            1,
            true,
            0,
            0,
            ended.as_deref(),
            json!({ "from": "episode" }),
        )?;
        written += 1;
    }

    // ── 节点：复习队列保持率 ──
    {
        let mut stmt = conn
            .prepare(
                "SELECT node_id,kb_name,label,reps,lapses,last_review,due_date,stability,\
                 difficulty,interval_days FROM review_state",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, Option<f64>>(7)?,
                    r.get::<_, Option<f64>>(8)?,
                    r.get::<_, i64>(9)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for (node_id, kb, label, reps, lapses, last_review, due, stability, difficulty, interval) in
            rows.flatten()
        {
            let s = stability
                .filter(|v| *v > 0.0)
                .unwrap_or_else(|| interval.max(1) as f64);
            let elapsed = last_review.as_deref().map(days_since).unwrap_or(0.0);
            let retention = if last_review.is_some() {
                fsrs::retrievability(s, elapsed)
            } else {
                1.0
            };
            let reliability = ((reps + 1) as f64) / ((reps + lapses + 2) as f64);
            let mastery = (0.6 * retention + 0.4 * reliability).clamp(0.0, 1.0);
            let engagement = (reps as f64 / 5.0).clamp(0.0, 1.0);
            let confidence = (((reps + lapses) + 1) as f64 / 6.0).clamp(0.0, 1.0);
            upsert_topic(
                &conn,
                &format!("node:{}", node_id),
                &label,
                "node",
                Some(kb.as_str()),
                Some(node_id.as_str()),
                0.0,
                retention,
                engagement,
                mastery,
                confidence,
                reps,
                false,
                reps,
                lapses,
                last_review.as_deref(),
                json!({ "dueDate": due, "stability": stability, "difficulty": difficulty }),
            )?;
            written += 1;
        }
    }

    let _ = crate::cognition::store::set_setting("learning_refreshed_at", &now_iso());
    debug!("[cognition] 学习状态刷新：{} 个主题", written);
    Ok(written)
}

/// 单条复习反馈之后就地更新该节点的掌握度（不必等每日刷新）。
pub fn touch_node(node_id: &str) -> Result<(), String> {
    let conn = ensure_db()?;
    let row: Option<(String, String, i64, i64, Option<String>, String, Option<f64>, Option<f64>, i64)> = conn
        .query_row(
            "SELECT kb_name,label,reps,lapses,last_review,due_date,stability,difficulty,interval_days \
             FROM review_state WHERE node_id=?1",
            params![node_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                ))
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((kb, label, reps, lapses, last_review, due, stability, difficulty, interval)) = row
    else {
        return Ok(());
    };
    let s = stability
        .filter(|v| *v > 0.0)
        .unwrap_or_else(|| interval.max(1) as f64);
    let elapsed = last_review.as_deref().map(days_since).unwrap_or(0.0);
    let retention = if last_review.is_some() {
        fsrs::retrievability(s, elapsed)
    } else {
        1.0
    };
    let reliability = ((reps + 1) as f64) / ((reps + lapses + 2) as f64);
    let mastery = (0.6 * retention + 0.4 * reliability).clamp(0.0, 1.0);
    let engagement = (reps as f64 / 5.0).clamp(0.0, 1.0);
    let confidence = (((reps + lapses) + 1) as f64 / 6.0).clamp(0.0, 1.0);
    upsert_topic(
        &conn,
        &format!("node:{}", node_id),
        &label,
        "node",
        Some(kb.as_str()),
        Some(node_id),
        0.0,
        retention,
        engagement,
        mastery,
        confidence,
        reps,
        false,
        reps,
        lapses,
        last_review.as_deref(),
        json!({ "dueDate": due, "stability": stability, "difficulty": difficulty }),
    )
}

fn maybe_refresh() {
    let last = crate::cognition::store::get_setting("learning_refreshed_at").unwrap_or_default();
    if last.is_empty() {
        let _ = refresh_mastery();
        return;
    }
    let age = days_since(&last) * 86400.0;
    if age >= REFRESH_THROTTLE_SECS as f64 {
        let _ = refresh_mastery();
    }
}

/// 学习看板数据。`days` 决定「近 N 天读完」与耗时窗口（默认 30）。
pub fn overview(days: Option<i64>) -> Result<LearningOverview, String> {
    maybe_refresh();
    let conn = ensure_db()?;
    let days = days.unwrap_or(30).clamp(1, 365);
    let cutoff = (chrono::Local::now() - chrono::Duration::days(days)).to_rfc3339();
    let today_str = today();

    let mut topics: Vec<TopicRow> = {
        let mut stmt = conn
            .prepare(
                "SELECT topic_key,topic,source,kb_name,ref_id,coverage,retention,engagement,\
                 mastery,confidence,sessions,finished,reps,lapses,last_touched_at,updated_at \
                 FROM topic_mastery ORDER BY mastery ASC, confidence DESC LIMIT 300",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(TopicRow {
                    key: r.get(0)?,
                    topic: r.get(1)?,
                    source: r.get(2)?,
                    kb_name: r.get(3)?,
                    ref_id: r.get(4)?,
                    coverage: r.get(5)?,
                    retention: r.get(6)?,
                    engagement: r.get(7)?,
                    mastery: r.get(8)?,
                    confidence: r.get(9)?,
                    sessions: r.get(10)?,
                    finished: r.get::<_, i64>(11)? != 0,
                    reps: r.get(12)?,
                    lapses: r.get(13)?,
                    last_touched_at: r.get(14)?,
                    updated_at: r.get(15)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };
    // 置信度低的排到后面：只读过一次的书不该被当成「待加强」的结论。
    topics.sort_by(|a, b| {
        a.mastery
            .partial_cmp(&b.mastery)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal))
    });

    let topics_total = topics.len() as i64;
    let mastered = topics.iter().filter(|t| t.mastery >= MASTERED_AT).count() as i64;
    let weak = topics.iter().filter(|t| t.mastery < WEAK_BELOW).count() as i64;
    let learning = (topics_total - mastered - weak).max(0);

    let due_reviews: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM review_state WHERE due_date<=?1",
            params![today_str],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let overdue_reviews: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM review_state WHERE due_date<?1",
            params![today_str],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let due: Vec<DueReview> = {
        let mut stmt = conn
            .prepare(
                "SELECT node_id,kb_name,label,due_date,reps,lapses,stability FROM review_state \
                 WHERE due_date<=?1 ORDER BY due_date ASC, reps ASC LIMIT 10",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![today_str], |r| {
                Ok(DueReview {
                    node_id: r.get(0)?,
                    kb_name: r.get(1)?,
                    label: r.get(2)?,
                    due_date: r.get(3)?,
                    reps: r.get(4)?,
                    lapses: r.get(5)?,
                    stability: r.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    let finished_books: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM episodes WHERE kind='read' AND outcome='ok' AND started_at>=?1",
            params![cutoff],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let reading_minutes: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(latency_ms),0)/60000 FROM episodes \
             WHERE kind='read' AND outcome='ok' AND started_at>=?1",
            params![cutoff],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let recent_finished: Vec<FinishedItem> = {
        let mut stmt = conn
            .prepare(
                "SELECT COALESCE(question,''),COALESCE(ended_at,''),COALESCE(latency_ms,0) \
                 FROM episodes WHERE kind='read' AND outcome='ok' ORDER BY started_at DESC LIMIT 10",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(FinishedItem {
                    title: r.get(0)?,
                    ended_at: Some(r.get::<_, String>(1)?).filter(|s| !s.is_empty()),
                    minutes: r.get::<_, i64>(2)? / 60000,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    // 连续阅读天数：从今天（或昨天）往前数，哪天有过阅读活动就 +1。
    // 单独用 365 天窗口：连续天数不该被「近 N 天读完」的统计窗口截断。
    let streak_days = {
        let streak_cutoff = (chrono::Local::now() - chrono::Duration::days(365)).to_rfc3339();
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT substr(at,1,10) FROM activity WHERE app='reader' AND kind='read' \
                 AND at>=?1 ORDER BY 1 DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![streak_cutoff], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let day_set: HashSet<String> = rows.flatten().collect();
        let mut cursor = chrono::Local::now().date_naive();
        if !day_set.contains(&cursor.format("%Y-%m-%d").to_string()) {
            cursor -= chrono::Duration::days(1);
        }
        let mut streak = 0i64;
        while day_set.contains(&cursor.format("%Y-%m-%d").to_string()) {
            streak += 1;
            cursor -= chrono::Duration::days(1);
        }
        streak
    };

    let updated_at = crate::cognition::store::get_setting("learning_refreshed_at")
        .unwrap_or_else(now_iso);

    Ok(LearningOverview {
        days,
        updated_at,
        topics_total,
        mastered,
        learning,
        weak,
        finished_books,
        reading_minutes,
        streak_days,
        due_reviews,
        overdue_reviews,
        topics,
        due,
        recent_finished,
    })
}
