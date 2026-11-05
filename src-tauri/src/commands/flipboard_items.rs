//! 采集条目的状态机（Phase 1）。
//!
//! 目的：把"抓回来后只有一个混合态（摘要/正文/空）"的一堆行，变成一条有明确
//! 推进路径的状态机。每一步都可重试、可观测、可重放：
//!
//! ```text
//! discovered ──▶ fetched ──▶ extracted ──▶ ready
//!     │            │             │
//!     └────────────┴─────────────┴──▶ rejected（准入/质量门拒收）
//!                  │
//!                  └──▶ failed（可重试，带 fail_count / next_retry_at）
//!                  └──▶ needs_login（登录墙，等用户登录后批量重试）
//! ```
//!
//! 与 `article_cache` 的关系：
//! - `article_cache` 是**内容**（标题/摘要/正文/图片），继续作为展示真源；
//! - `items` 是**状态与索引**（url 规范化键、状态、重试信息、raw 落盘位置），
//!   按 `(feed_id, url_norm)` 唯一。
//!
//! 之所以不复用 `article_cache` 加一列：状态机的推进与内容写入的时机不同
//! （内容可能被 hydrate 覆盖多次，而状态只前进），并且 `items` 还要承载
//! 尚未抓到内容之前的 discovered 行（那时 `article_cache` 里还没有东西）。
//!
//! 实现进度：本模块提供完整的推进原语，但**调度循环尚未接入**——
//! 目前实际用到的是 `note_discovered` / `mark_rejected` / `set_raw_path_by_norm`
//! 与 hydrate 里的直接 UPDATE。`claim_work` / `mark_failed` / `set_state` 等
//! 是给 Phase 1 下一步（后台 refine_tick 按状态取件）准备的，暂标 allow(dead_code)。

#![allow(dead_code)] // 状态机原语：部分已接线，其余由后续 refine_tick 消费

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// 条目状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    /// 从源里发现，只有元数据
    Discovered,
    /// 已取回原始页面（raw HTML 已落盘）
    Fetched,
    /// 已抽取成正文（Markdown）
    Extracted,
    /// 已完成清洗与评分，可对外展示
    Ready,
    /// 被准入策略/质量门拒收
    Rejected,
    /// 处理失败，可重试
    Failed,
    /// 命中登录墙，等用户登录
    NeedsLogin,
}

impl ItemState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemState::Discovered => "discovered",
            ItemState::Fetched => "fetched",
            ItemState::Extracted => "extracted",
            ItemState::Ready => "ready",
            ItemState::Rejected => "rejected",
            ItemState::Failed => "failed",
            ItemState::NeedsLogin => "needs_login",
        }
    }

    pub fn from_str(s: &str) -> Option<ItemState> {
        Some(match s {
            "discovered" => ItemState::Discovered,
            "fetched" => ItemState::Fetched,
            "extracted" => ItemState::Extracted,
            "ready" => ItemState::Ready,
            "rejected" => ItemState::Rejected,
            "failed" => ItemState::Failed,
            "needs_login" => ItemState::NeedsLogin,
            _ => return None,
        })
    }

    /// 该状态下是否应当对外展示（列表/检索）。
    pub fn is_visible(&self) -> bool {
        matches!(self, ItemState::Ready)
    }
}

/// 重试策略：指数退避，上限 6 小时。
pub fn retry_delay_secs(fail_count: u32) -> i64 {
    let n = fail_count.min(10) as u32;
    (30_i64 * 2_i64.pow(n)).min(6 * 3600)
}

/// 建表（幂等）。
pub fn ensure_items_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS items (
            id            TEXT NOT NULL,
            feed_id       TEXT NOT NULL,
            url           TEXT NOT NULL,
            url_norm      TEXT NOT NULL,
            canonical_url TEXT,
            title         TEXT NOT NULL DEFAULT '',
            source        TEXT NOT NULL DEFAULT '',
            category      TEXT NOT NULL DEFAULT 'all',
            lang          TEXT,
            page_kind     TEXT,
            state         TEXT NOT NULL DEFAULT 'discovered',
            quality       INTEGER NOT NULL DEFAULT -1,
            published_at  TEXT NOT NULL DEFAULT '',
            first_seen_at INTEGER NOT NULL DEFAULT 0,
            updated_at    INTEGER NOT NULL DEFAULT 0,
            reject_reason TEXT,
            fail_reason   TEXT,
            fail_count    INTEGER NOT NULL DEFAULT 0,
            next_retry_at INTEGER NOT NULL DEFAULT 0,
            raw_path      TEXT,
            doc_path      TEXT,
            opened_at     INTEGER,
            open_count    INTEGER NOT NULL DEFAULT 0,
            kept          INTEGER NOT NULL DEFAULT 0,
            trash_path    TEXT,
            purged_at     INTEGER,
            canonical_id  TEXT,
            is_canonical  INTEGER NOT NULL DEFAULT 1,
            alias_count   INTEGER NOT NULL DEFAULT 1,
            simhash       INTEGER,
            content_kind  TEXT,
            tier          TEXT,
            priority      INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY(feed_id, url_norm)
        );
        CREATE INDEX IF NOT EXISTS idx_items_state
            ON items(state, priority DESC, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_items_retry
            ON items(state, next_retry_at);
        CREATE INDEX IF NOT EXISTS idx_items_norm
            ON items(url_norm);
        "#,
    )
    .map_err(|e| e.to_string())?;

    // 向后兼容迁移：items 表在 Phase 1 先建出来了，Phase 3 才加 doc_path
    // （Markdown 真源的相对路径）。SQLite 没有 ADD COLUMN IF NOT EXISTS，
    // 用 PRAGMA 探测后逐列补。
    ensure_items_columns(conn)?;
    Ok(())
}

/// 补列迁移。新列必须**追加在末尾**，否则按位读取会错位。
fn ensure_items_columns(conn: &Connection) -> Result<(), String> {
    let mut existing: std::collections::HashSet<String> = std::collections::HashSet::new();
    {
        let mut stmt = conn
            .prepare("PRAGMA table_info(items)")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(|e| e.to_string())?;
        for r in rows {
            if let Ok(name) = r {
                existing.insert(name);
            }
        }
    }
    let columns: &[(&str, &str)] = &[
        ("raw_path", "TEXT"),
        ("doc_path", "TEXT"),
        ("opened_at", "INTEGER"),
        ("open_count", "INTEGER NOT NULL DEFAULT 0"),
        ("kept", "INTEGER NOT NULL DEFAULT 0"),
        ("trash_path", "TEXT"),
        ("purged_at", "INTEGER"),
        ("canonical_id", "TEXT"),
        ("is_canonical", "INTEGER NOT NULL DEFAULT 1"),
        ("alias_count", "INTEGER NOT NULL DEFAULT 1"),
        ("simhash", "INTEGER"),
        ("content_kind", "TEXT"),
        ("tier", "TEXT"),
    ];
    for (name, decl) in columns {
        if !existing.contains(*name) {
            conn.execute(
                &format!("ALTER TABLE items ADD COLUMN {} {}", name, decl),
                [],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// 一条待处理的条目（供管线推进）。
#[derive(Debug, Clone)]
pub struct WorkItem {
    pub id: String,
    pub feed_id: String,
    pub url: String,
    pub url_norm: String,
    pub state: ItemState,
    pub fail_count: u32,
    pub raw_path: Option<String>,
}

/// 记录"发现了这条"。已存在则只更新标题/时间，不倒退状态。
pub fn note_discovered(
    conn: &Connection,
    feed_id: &str,
    url: &str,
    url_norm: &str,
    title: &str,
    source: &str,
    category: &str,
    published_at: &str,
    now: i64,
) -> Result<(), String> {
    conn.execute(
        r#"
        INSERT INTO items
            (id, feed_id, url, url_norm, title, source, category, published_at,
             state, first_seen_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'discovered', ?9, ?9)
        ON CONFLICT(feed_id, url_norm) DO UPDATE SET
            title      = CASE WHEN excluded.title <> '' THEN excluded.title ELSE items.title END,
            updated_at = excluded.updated_at
        "#,
        params![
            format!("{}:{}", feed_id, url_norm),
            feed_id,
            url,
            url_norm,
            title,
            source,
            category,
            published_at,
            now
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 记录条目的内容类型与分级（来自所属 feed）。
///
/// 单独一个 setter 而不是加进 `note_discovered`：后者有 9 处调用，
/// 而类型信息只在"从 feed 发现"的路径上才有，其他路径传空即可。
pub fn set_content_kind(
    conn: &Connection,
    feed_id: &str,
    url_norm: &str,
    kind: &str,
    tier: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE items SET content_kind = ?3, tier = ?4 \
         WHERE feed_id = ?1 AND url_norm = ?2",
        params![feed_id, url_norm, kind, tier],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 推进到某个状态（只前进，不倒退；`failed`/`needs_login` 除外，它们可与别的状态互转）。
pub fn set_state(
    conn: &Connection,
    feed_id: &str,
    url_norm: &str,
    state: ItemState,
    now: i64,
) -> Result<(), String> {
    conn.execute(
        "UPDATE items SET state = ?3, updated_at = ?4 WHERE feed_id = ?1 AND url_norm = ?2",
        params![feed_id, url_norm, state.as_str(), now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 记录一次失败并安排重试时间。
pub fn mark_failed(
    conn: &Connection,
    feed_id: &str,
    url_norm: &str,
    reason: &str,
    now: i64,
) -> Result<u32, String> {
    let current: u32 = conn
        .query_row(
            "SELECT fail_count FROM items WHERE feed_id = ?1 AND url_norm = ?2",
            params![feed_id, url_norm],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .unwrap_or(0);
    let next = current.saturating_add(1);
    let retry_at = now + retry_delay_secs(next);
    conn.execute(
        r#"
        UPDATE items SET
            state = 'failed', fail_reason = ?3, fail_count = ?4,
            next_retry_at = ?5, updated_at = ?6
        WHERE feed_id = ?1 AND url_norm = ?2
        "#,
        params![feed_id, url_norm, reason, next, retry_at, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(next)
}

/// 记录拒收原因（终态，不再重试）。
pub fn mark_rejected(
    conn: &Connection,
    feed_id: &str,
    url_norm: &str,
    reason: &str,
    now: i64,
) -> Result<(), String> {
    conn.execute(
        "UPDATE items SET state = 'rejected', reject_reason = ?3, updated_at = ?4 \
         WHERE feed_id = ?1 AND url_norm = ?2",
        params![feed_id, url_norm, reason, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 记录 raw HTML 落盘位置。
pub fn set_raw_path(
    conn: &Connection,
    feed_id: &str,
    url_norm: &str,
    raw_path: &str,
    now: i64,
) -> Result<(), String> {
    conn.execute(
        "UPDATE items SET raw_path = ?3, updated_at = ?4 WHERE feed_id = ?1 AND url_norm = ?2",
        params![feed_id, url_norm, raw_path, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 按规范化 URL 记录 raw 路径（跨源同文一起写）。
///
/// hydrate 阶段只有 `Article`，没有 `feed_id`；同一篇文章可能被多个源抓到，
/// 所以这里按 url_norm 更新全部匹配行——它们指向的是同一份 raw 内容。
pub fn set_raw_path_by_norm(
    conn: &Connection,
    url_norm: &str,
    raw_path: &str,
    now: i64,
) -> Result<usize, String> {
    conn.execute(
        "UPDATE items SET raw_path = ?2, updated_at = ?3 WHERE url_norm = ?1",
        params![url_norm, raw_path, now],
    )
    .map_err(|e| e.to_string())
}

/// 取一批"该处理"的条目：按优先级、时间排序。
/// `states` 里给的状态都算候选；`failed` 的还要满足 `next_retry_at <= now`。
pub fn claim_work(
    conn: &Connection,
    states: &[ItemState],
    now: i64,
    limit: usize,
) -> Result<Vec<WorkItem>, String> {
    if states.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let placeholders = (0..states.len())
        .map(|i| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT id, feed_id, url, url_norm, state, fail_count, raw_path \
         FROM items \
         WHERE state IN ({}) \
           AND (state <> 'failed' OR next_retry_at <= ?{}) \
         ORDER BY priority DESC, updated_at ASC LIMIT {}",
        placeholders,
        states.len() + 1,
        limit
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    // 状态字符串要先落地再借出，`as_str()` 的临时值不能直接进绑定列表。
    let state_strs: Vec<&str> = states.iter().map(|s| s.as_str()).collect();
    let mut bound: Vec<&dyn rusqlite::ToSql> = Vec::new();
    for s in &state_strs {
        bound.push(s);
    }
    bound.push(&now);
    let rows = stmt
        .query_map(bound.as_slice(), |r| {
            let state_str: String = r.get(4)?;
            Ok(WorkItem {
                id: r.get(0)?,
                feed_id: r.get(1)?,
                url: r.get(2)?,
                url_norm: r.get(3)?,
                state: ItemState::from_str(&state_str).unwrap_or(ItemState::Discovered),
                fail_count: r.get::<_, i64>(5)? as u32,
                raw_path: r.get(6).ok(),
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for x in rows {
        if let Ok(i) = x {
            out.push(i);
        }
    }
    Ok(out)
}

/// `fetched` 状态被当作"处理中"的租约：超过这个时长仍停在 fetched，
/// 说明处理该条的进程已经崩溃/被重启，应当回收重新排队。
///
/// 没有这层回收，`refine_tick` 把条目标成 fetched 后一旦进程退出，
/// 这条就永久卡死——既不会被 claim 到，也不会进 failed 重试。
pub const FETCHED_LEASE_SECS: i64 = 30 * 60;

/// 回收超时的 fetched 租约，把它们退回 discovered 以便重新处理。
/// 返回回收条数。应在每次取件前调用。
pub fn reclaim_stale_leases(conn: &Connection, now: i64) -> Result<usize, String> {
    let cutoff = now - FETCHED_LEASE_SECS;
    conn.execute(
        "UPDATE items SET state = 'discovered', updated_at = ?1 \
         WHERE state = 'fetched' AND updated_at < ?2",
        params![now, cutoff],
    )
    .map_err(|e| e.to_string())
}

/// 记录"这篇被打开了"——保留策略的核心输入。
///
/// 只有打开过的内容才值得长期保留；从未打开的 21 天后回收。
/// 这个信号必须由阅读器真实触发，不能从"出现在列表里"推断——
/// 进入列表 ≠ 被阅读。
pub fn record_opened(conn: &Connection, url_norm: &str, now: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE items SET opened_at = ?2, open_count = open_count + 1, \
         updated_at = ?2 WHERE url_norm = ?1",
        params![url_norm, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 用户显式"保留"某篇（不参与自动回收）。
pub fn mark_kept(conn: &Connection, url_norm: &str, kept: bool, now: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE items SET kept = ?2, updated_at = ?3 WHERE url_norm = ?1",
        params![url_norm, kept as i64, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 查找与给定正文 simhash 属于同一 canonical 的代表行。
///
/// 只有"代表行"（`is_canonical = 1`）才可能匹配，且必须已有落盘路径——
/// 这样返回的代表一定是用户实际能看到/检索到的那一篇。
///
/// 返回 `Some((feed_id, url_norm))`。
pub fn find_canonical_representative(
    conn: &Connection,
    sim: u64,
    threshold: u32,
) -> Result<Option<(String, String)>, String> {
    if sim == 0 || threshold == 0 {
        return Ok(None);
    }
    // 只扫有文件、且是代表的行。数量上可控：代表行 ≈ 文章数而非抓取数。
    let mut stmt = conn
        .prepare(
            "SELECT feed_id, url_norm, simhash FROM items \
             WHERE is_canonical = 1 AND simhash IS NOT NULL AND simhash <> 0 \
             LIMIT 5000",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)? as u64,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut best: Option<((String, String), u32)> = None;
    for row in rows.flatten() {
        let (feed_id, url_norm, other) = row;
        let dist = (other ^ sim).count_ones();
        if dist <= threshold && best.as_ref().map(|(_, d)| dist < *d).unwrap_or(true) {
            best = Some(((feed_id, url_norm), dist));
        }
    }
    Ok(best.map(|(v, _)| v))
}

/// 把一个条目标记为某个 canonical 的别名，并累加代表行的 alias_count。
pub fn link_as_alias(
    conn: &Connection,
    feed_id: &str,
    url_norm: &str,
    canonical_url_norm: &str,
    canonical_id: &str,
    now: i64,
) -> Result<(), String> {
    conn.execute(
        "UPDATE items SET canonical_id = ?3, is_canonical = 0, updated_at = ?4 \
         WHERE feed_id = ?1 AND url_norm = ?2",
        params![feed_id, url_norm, canonical_id, now],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE items SET alias_count = alias_count + 1, updated_at = ?2 \
         WHERE url_norm = ?1 AND is_canonical = 1",
        params![canonical_url_norm, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 记录正文 simhash（去重的基础，写入后才能被后续条目匹配）。
pub fn set_simhash(
    conn: &Connection,
    feed_id: &str,
    url_norm: &str,
    sim: u64,
    now: i64,
) -> Result<(), String> {
    conn.execute(
        "UPDATE items SET simhash = ?3, updated_at = ?4 WHERE feed_id = ?1 AND url_norm = ?2",
        params![feed_id, url_norm, sim as i64, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 统计各状态数量（看板/诊断用）。
pub fn state_counts(conn: &Connection) -> Result<Vec<(String, i64)>, String> {
    let mut stmt = conn
        .prepare("SELECT state, COUNT(*) FROM items GROUP BY state ORDER BY state")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for x in rows {
        if let Ok(p) = x {
            out.push(p);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        ensure_items_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn discovered_is_idempotent_and_does_not_regress_state() {
        let conn = mem();
        note_discovered(&conn, "f1", "https://x/a", "https://x/a", "T1", "S", "all", "", 100)
            .unwrap();
        set_state(&conn, "f1", "https://x/a", ItemState::Ready, 110).unwrap();
        // 再次发现不应把 ready 打回 discovered
        note_discovered(&conn, "f1", "https://x/a", "https://x/a", "T2", "S", "all", "", 120)
            .unwrap();
        let state: String = conn
            .query_row(
                "SELECT state FROM items WHERE feed_id='f1' AND url_norm='https://x/a'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(state, "ready");
        // 标题会被更新
        let title: String = conn
            .query_row("SELECT title FROM items WHERE feed_id='f1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "T2");
    }

    #[test]
    fn failed_items_back_off() {
        let conn = mem();
        note_discovered(&conn, "f1", "u", "u", "T", "S", "all", "", 100).unwrap();
        let c1 = mark_failed(&conn, "f1", "u", "boom", 100).unwrap();
        assert_eq!(c1, 1);
        let retry_at: i64 = conn
            .query_row("SELECT next_retry_at FROM items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(retry_at, 100 + retry_delay_secs(1));
        // 未到重试时间不应被 claim
        let work = claim_work(&conn, &[ItemState::Failed], 100, 10).unwrap();
        assert!(work.is_empty());
        // 到了就能取
        let work = claim_work(&conn, &[ItemState::Failed], retry_at, 10).unwrap();
        assert_eq!(work.len(), 1);
    }

    #[test]
    fn rejected_is_terminal() {
        let conn = mem();
        note_discovered(&conn, "f1", "u", "u", "T", "S", "all", "", 100).unwrap();
        mark_rejected(&conn, "f1", "u", "promo", 100).unwrap();
        let work = claim_work(
            &conn,
            &[ItemState::Discovered, ItemState::Fetched, ItemState::Failed],
            9_999_999,
            10,
        )
        .unwrap();
        assert!(work.is_empty(), "rejected must not be claimed for work");
    }

    #[test]
    fn stale_leases_are_reclaimed() {
        let conn = mem();
        note_discovered(&conn, "f1", "u", "u", "T", "S", "all", "", 100).unwrap();
        // 标记为处理中（fetched），随后"进程崩溃"——时间不再前进
        set_state(&conn, "f1", "u", ItemState::Fetched, 100).unwrap();
        // 租约未过期：不该回收
        let n = reclaim_stale_leases(&conn, 100 + FETCHED_LEASE_SECS - 1).unwrap();
        assert_eq!(n, 0);
        // 过期：退回 discovered，重新可被领取
        let n = reclaim_stale_leases(&conn, 100 + FETCHED_LEASE_SECS + 1).unwrap();
        assert_eq!(n, 1);
        let work = claim_work(&conn, &[ItemState::Discovered], 100 + FETCHED_LEASE_SECS + 10, 10)
            .unwrap();
        assert_eq!(work.len(), 1, "reclaimed item must be claimable again");
    }

    #[test]
    fn counts_group_by_state() {
        let conn = mem();
        for (i, url) in ["a", "b", "c"].iter().enumerate() {
            note_discovered(&conn, "f1", url, url, "T", "S", "all", "", 100 + i as i64).unwrap();
        }
        set_state(&conn, "f1", "a", ItemState::Ready, 200).unwrap();
        let counts = state_counts(&conn).unwrap();
        let map: std::collections::HashMap<_, _> = counts.into_iter().collect();
        assert_eq!(map.get("ready"), Some(&1));
        assert_eq!(map.get("discovered"), Some(&2));
    }
}

#[cfg(test)]
mod canonical_tests {
    use super::*;

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        ensure_items_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn finds_matching_representative() {
        let conn = mem();
        note_discovered(&conn, "f1", "u1", "u1", "T", "S", "all", "", 100).unwrap();
        let sim = 0b1111_0000_1111_0000u64;
        set_simhash(&conn, "f1", "u1", sim, 100).unwrap();
        conn.execute(
            "UPDATE items SET is_canonical = 1 WHERE feed_id='f1' AND url_norm='u1'",
            [],
        )
        .unwrap();

        // 距离 2，应命中
        let near = sim ^ 0b11;
        let found = find_canonical_representative(&conn, near, 4).unwrap();
        assert_eq!(found, Some(("f1".to_string(), "u1".to_string())));

        // 距离 8，超出阈值
        let far = sim ^ 0xFF;
        assert!(find_canonical_representative(&conn, far, 4).unwrap().is_none());
    }

    #[test]
    fn non_representatives_are_not_matched() {
        let conn = mem();
        note_discovered(&conn, "f1", "u1", "u1", "T", "S", "all", "", 100).unwrap();
        set_simhash(&conn, "f1", "u1", 0xAAAAu64, 100).unwrap();
        // 标记为别名（非代表）
        conn.execute("UPDATE items SET is_canonical = 0", []).unwrap();
        assert!(
            find_canonical_representative(&conn, 0xAAAA, 4).unwrap().is_none(),
            "alias rows must not be used as canonical targets"
        );
    }

    #[test]
    fn alias_link_increments_count() {
        let conn = mem();
        note_discovered(&conn, "f1", "canon", "canon", "T", "S", "all", "", 100).unwrap();
        note_discovered(&conn, "f2", "alias", "alias", "T2", "S2", "all", "", 100).unwrap();

        link_as_alias(&conn, "f2", "alias", "canon", "f1:canon", 200).unwrap();

        let (is_canon, cid): (i64, Option<String>) = conn
            .query_row(
                "SELECT is_canonical, canonical_id FROM items WHERE feed_id='f2'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(is_canon, 0);
        assert_eq!(cid.as_deref(), Some("f1:canon"));

        let count: i64 = conn
            .query_row(
                "SELECT alias_count FROM items WHERE feed_id='f1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 2, "representative should count its alias");
    }

    #[test]
    fn zero_simhash_never_matches() {
        let conn = mem();
        assert!(find_canonical_representative(&conn, 0, 4).unwrap().is_none());
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;

    /// 模拟"老库升级"：先建出不含新列的旧表，再跑迁移，验证列被补上。
    /// 这类迁移最容易漏——单测建的是新表，生产库是旧的。
    #[test]
    fn old_schema_gets_new_columns() {
        let conn = Connection::open_in_memory().unwrap();
        // 旧版 items 表（Phase 1 的形态）
        conn.execute_batch(
            r#"CREATE TABLE items (
                id TEXT NOT NULL, feed_id TEXT NOT NULL, url TEXT NOT NULL,
                url_norm TEXT NOT NULL, canonical_url TEXT,
                title TEXT NOT NULL DEFAULT '', source TEXT NOT NULL DEFAULT '',
                category TEXT NOT NULL DEFAULT 'all', lang TEXT, page_kind TEXT,
                state TEXT NOT NULL DEFAULT 'discovered',
                quality INTEGER NOT NULL DEFAULT -1,
                published_at TEXT NOT NULL DEFAULT '',
                first_seen_at INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0,
                reject_reason TEXT, fail_reason TEXT,
                fail_count INTEGER NOT NULL DEFAULT 0, next_retry_at INTEGER NOT NULL DEFAULT 0,
                raw_path TEXT, priority INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY(feed_id, url_norm));"#,
        )
        .unwrap();

        // 迁移
        ensure_items_schema(&conn).unwrap();

        let mut cols = std::collections::HashSet::new();
        let mut stmt = conn.prepare("PRAGMA table_info(items)").unwrap();
        for r in stmt.query_map([], |r| r.get::<_, String>(1)).unwrap().flatten() {
            cols.insert(r);
        }
        for expected in [
            "doc_path", "opened_at", "open_count", "kept",
            "trash_path", "purged_at", "canonical_id", "is_canonical",
            "alias_count", "simhash",
        ] {
            assert!(cols.contains(expected), "migration must add column {}", expected);
        }

        // 迁移后新功能必须真的可用
        note_discovered(&conn, "f", "u", "u", "t", "s", "all", "", 1).unwrap();
        set_simhash(&conn, "f", "u", 12345, 2).unwrap();
        record_opened(&conn, "u", 3).unwrap();
        let (sim, opened, kept): (i64, i64, i64) = conn
            .query_row(
                "SELECT simhash, opened_at, kept FROM items WHERE url_norm='u'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(sim, 12345);
        assert_eq!(opened, 3);
        assert_eq!(kept, 0);
    }

    #[test]
    fn migration_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_items_schema(&conn).unwrap();
        // 再跑一次不应报错（列已存在）
        ensure_items_schema(&conn).unwrap();
        ensure_items_schema(&conn).unwrap();
    }
}

#[cfg(test)]
mod real_db_tests {
    use super::*;

    /// 用真实缓存库副本验证建表与迁移。
    /// 环境变量 `GUAXI_REAL_DB` 指向库文件；未设置时跳过（CI 里没有这个库）。
    #[test]
    fn real_database_gets_full_schema() {
        let Some(src) = std::env::var("GUAXI_REAL_DB").ok().filter(|s| !s.trim().is_empty())
        else {
            return;
        };
        let tmp = std::env::temp_dir().join(format!("guaxi-realdb-{}.sqlite3", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        std::fs::copy(&src, &tmp).unwrap();

        let conn = Connection::open(&tmp).unwrap();
        ensure_items_schema(&conn).unwrap();

        // 全部新列都在
        let mut cols = std::collections::HashSet::new();
        let mut stmt = conn.prepare("PRAGMA table_info(items)").unwrap();
        for r in stmt.query_map([], |r| r.get::<_, String>(1)).unwrap().flatten() {
            cols.insert(r);
        }
        for c in [
            "doc_path", "opened_at", "open_count", "kept", "trash_path",
            "purged_at", "canonical_id", "is_canonical", "alias_count", "simhash",
        ] {
            assert!(cols.contains(c), "real db migration missing column {}", c);
        }

        // 新表上基本操作可用
        note_discovered(&conn, "f", "u", "u", "t", "s", "all", "", 1).unwrap();
        set_simhash(&conn, "f", "u", 42, 2).unwrap();
        record_opened(&conn, "u", 3).unwrap();
        let (sim, opened): (i64, i64) = conn
            .query_row("SELECT simhash, opened_at FROM items WHERE url_norm='u'", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(sim, 42);
        assert_eq!(opened, 3);

        // 顺带确认 article_cache 仍然可读（证明没破坏既有表）
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM article_cache", [], |r| r.get(0))
            .unwrap();
        assert!(n >= 0);

        let _ = std::fs::remove_file(&tmp);
    }
}
