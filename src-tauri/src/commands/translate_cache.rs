//! 翻译结果的持久化缓存（SQLite）。
//!
//! 为什么要落盘：全文翻译一篇文档会触发几百次块级翻译，而同一篇文档反复打开、
//! 或者不同文档里出现相同段落（标题、免责声明、术语行）是常态。只有进程内缓存的话
//! 每次重启都要重新打一遍免费接口——既慢，又是被风控封禁的主要原因。
//!
//! 设计取舍：
//! - 主键是 blake3(源语言|目标语言|原文) 的十六进制，定长、可索引，长段落也不会撑爆索引。
//! - 写入走后台批量 writer：翻译链路只把行丢进 channel，不等磁盘，避免拖慢首屏出译文。
//! - 只按 LRU 上限裁剪，不设过期时间——译文不会自己变质，留着就是纯收益。
//! - 命中时更新 used_at / hit_count，裁剪时优先淘汰最久没用到的。

use once_cell::sync::Lazy;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tracing::{info, warn};

const CACHE_DIR_NAME: &str = "guaxi_translate_cache";
const CACHE_DB_NAME: &str = "translate_cache.sqlite3";

/// 缓存行数上限。超过后按 used_at 淘汰最久未用的 10%。
/// 一行大约几百字节，20 万行大概几十 MB，对桌面端完全可接受。
const MAX_ROWS: i64 = 200_000;

#[derive(Debug, Clone)]
struct CacheRow {
    key: String,
    source_lang: String,
    target_lang: String,
    source_text: String,
    translated_text: String,
    engine: String,
}

fn cache_key(from: &str, to: &str, text: &str) -> String {
    let normalized = format!(
        "{}|{}|{}",
        from.trim().to_ascii_lowercase(),
        to.trim().to_ascii_lowercase(),
        text.trim()
    );
    blake3::hash(normalized.as_bytes()).to_hex().to_string()
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn resolve_db_path() -> Result<PathBuf, String> {
    let dir = crate::paths::repos_dir().join(CACHE_DIR_NAME);
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("创建翻译缓存目录失败: {}", e))?;
    }
    Ok(dir.join(CACHE_DB_NAME))
}

fn open_conn(db_path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("打开翻译缓存库失败: {}", e))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("设置 WAL 失败: {}", e))?;
    // 缓存库丢一两条无所谓，用 NORMAL 换写入吞吐，避免全文翻译时频繁 fsync。
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| format!("设置 synchronous 失败: {}", e))?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS translation_cache (
            key TEXT PRIMARY KEY,
            source_lang TEXT NOT NULL DEFAULT 'auto',
            target_lang TEXT NOT NULL DEFAULT 'zh',
            source_text TEXT NOT NULL,
            translated_text TEXT NOT NULL,
            engine TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL DEFAULT 0,
            used_at INTEGER NOT NULL DEFAULT 0,
            hit_count INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_translation_cache_used_at
            ON translation_cache(used_at);

        -- 词条（划词面板的音标/释义/短语/例句/近义词）整包缓存。
        -- 和译文分表存：它一行是一个 JSON 包，命中率高但条数远少于译文。
        CREATE TABLE IF NOT EXISTS word_entry_cache (
            key TEXT PRIMARY KEY,
            word TEXT NOT NULL,
            source_lang TEXT NOT NULL DEFAULT 'auto',
            target_lang TEXT NOT NULL DEFAULT 'zh',
            kind TEXT NOT NULL DEFAULT 'basic',
            payload TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT 0,
            used_at INTEGER NOT NULL DEFAULT 0,
            hit_count INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_word_entry_cache_used_at
            ON word_entry_cache(used_at);
        "#,
    )
    .map_err(|e| format!("初始化翻译缓存表失败: {}", e))?;
    Ok(conn)
}

/// 全局连接。SQLite 单连接 + 互斥就够了：这里的读写都是毫秒级小事务。
static CONN: Lazy<Mutex<Option<Connection>>> = Lazy::new(|| Mutex::new(None));
static DISABLED: AtomicBool = AtomicBool::new(false);

/// 拿到连接并执行闭包。库打不开（只读安装目录等）时整块降级为「无持久缓存」，不影响翻译。
fn with_conn<T>(f: impl FnOnce(&Connection) -> Result<T, String>) -> Option<T> {
    if DISABLED.load(Ordering::Relaxed) {
        return None;
    }
    let mut guard = match CONN.lock() {
        Ok(g) => g,
        Err(_) => {
            warn!("[TranslateCache] 连接锁损坏，停用持久缓存");
            DISABLED.store(true, Ordering::Relaxed);
            return None;
        }
    };
    if guard.is_none() {
        let path = match resolve_db_path() {
            Ok(p) => p,
            Err(e) => {
                warn!("[TranslateCache] {}，停用持久缓存", e);
                DISABLED.store(true, Ordering::Relaxed);
                return None;
            }
        };
        match open_conn(&path) {
            Ok(c) => {
                info!("[TranslateCache] 缓存库就绪: {}", path.to_string_lossy());
                *guard = Some(c);
            }
            Err(e) => {
                warn!("[TranslateCache] {}，停用持久缓存", e);
                DISABLED.store(true, Ordering::Relaxed);
                return None;
            }
        }
    }
    let conn = guard.as_ref()?;
    match f(conn) {
        Ok(v) => Some(v),
        Err(e) => {
            warn!("[TranslateCache] 操作失败: {}", e);
            None
        }
    }
}

// ---------------------------------------------------------------- 读

fn select_batch(
    conn: &Connection,
    keys: &[String],
) -> Result<Vec<(String, String)>, String> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; keys.len()].join(",");
    let sql = format!(
        "SELECT key, translated_text FROM translation_cache WHERE key IN ({})",
        placeholders
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(keys.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        if let Ok(pair) = row {
            out.push(pair);
        }
    }
    Ok(out)
}

/// 批量查缓存。返回 `原文 -> 译文`，未命中的键不出现在结果里。
/// 命中的行会顺手更新 used_at / hit_count，供 LRU 裁剪参考。
pub async fn get_many(from: &str, to: &str, texts: &[String]) -> HashMap<String, String> {
    if texts.is_empty() || DISABLED.load(Ordering::Relaxed) {
        return HashMap::new();
    }
    // 同一批里可能有重复原文，先按 key 去重，避免白查。
    let mut key_to_text: HashMap<String, String> = HashMap::new();
    for text in texts {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        key_to_text
            .entry(cache_key(from, to, trimmed))
            .or_insert_with(|| text.clone());
    }
    if key_to_text.is_empty() {
        return HashMap::new();
    }

    let keys: Vec<String> = key_to_text.keys().cloned().collect();
    let found = tokio::task::spawn_blocking(move || {
        with_conn(|conn| {
            let mut all = Vec::new();
            // SQLite 默认变量上限 999，分批查。
            for chunk in keys.chunks(400) {
                all.extend(select_batch(conn, chunk)?);
            }
            if !all.is_empty() {
                let now = now_secs();
                let hit_keys: Vec<String> = all.iter().map(|(k, _)| k.clone()).collect();
                for chunk in hit_keys.chunks(400) {
                    let placeholders = vec!["?"; chunk.len()].join(",");
                    let sql = format!(
                        "UPDATE translation_cache SET used_at = ?, hit_count = hit_count + 1 WHERE key IN ({})",
                        placeholders
                    );
                    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
                    for k in chunk {
                        binds.push(Box::new(k.clone()));
                    }
                    let refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
                    let _ = conn.execute(&sql, refs.as_slice());
                }
            }
            Ok(all)
        })
        .unwrap_or_default()
    })
    .await
    .unwrap_or_default();

    let mut out = HashMap::new();
    for (key, translated) in found {
        if let Some(text) = key_to_text.get(&key) {
            out.insert(text.clone(), translated);
        }
    }
    out
}

// ---------------------------------------------------------------- 写（后台批量）

fn insert_rows(conn: &Connection, rows: &[CacheRow]) -> Result<usize, String> {
    let now = now_secs();
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let mut written = 0usize;
    {
        let mut stmt = tx
            .prepare(
                r#"
                INSERT INTO translation_cache
                    (key, source_lang, target_lang, source_text, translated_text, engine, created_at, used_at, hit_count)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 0)
                ON CONFLICT(key) DO UPDATE SET
                    translated_text = excluded.translated_text,
                    engine = excluded.engine,
                    used_at = excluded.used_at
                "#,
            )
            .map_err(|e| e.to_string())?;
        for row in rows {
            match stmt.execute(params![
                row.key,
                row.source_lang,
                row.target_lang,
                row.source_text,
                row.translated_text,
                row.engine,
                now,
            ]) {
                Ok(_) => written += 1,
                Err(e) => warn!("[TranslateCache] 写入单行失败: {}", e),
            }
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(written)
}

/// 超出上限时淘汰最久未使用的一批，避免缓存库无限增长。
fn prune_if_needed(conn: &Connection) {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM translation_cache", [], |r| r.get(0))
        .unwrap_or(0);
    if count <= MAX_ROWS {
        return;
    }
    let victims = (count - MAX_ROWS).max(MAX_ROWS / 10);
    match conn.execute(
        "DELETE FROM translation_cache WHERE key IN (
             SELECT key FROM translation_cache ORDER BY used_at ASC LIMIT ?1
         )",
        params![victims],
    ) {
        Ok(n) => info!("[TranslateCache] LRU 裁剪 {} 行（原 {} 行）", n, count),
        Err(e) => warn!("[TranslateCache] 裁剪失败: {}", e),
    }
}

static WRITER: Lazy<UnboundedSender<CacheRow>> = Lazy::new(|| {
    let (tx, mut rx) = unbounded_channel::<CacheRow>();
    // 后台 writer：攒够一批或到时间就落一次盘。翻译链路只做 send，不等磁盘。
    tauri::async_runtime::spawn(async move {
        const MAX_BATCH: usize = 200;
        let mut pending: Vec<CacheRow> = Vec::new();
        let mut writes_since_prune = 0usize;
        loop {
            let first = match rx.recv().await {
                Some(row) => row,
                None => break,
            };
            pending.clear();
            pending.push(first);
            // 尽量把已经排队的行一起收掉。
            while pending.len() < MAX_BATCH {
                match rx.try_recv() {
                    Ok(row) => pending.push(row),
                    Err(_) => break,
                }
            }
            if pending.len() < MAX_BATCH {
                // 给后续行一个小窗口，把「逐块翻译」的写入合并成一个事务。
                tokio::time::sleep(Duration::from_millis(400)).await;
                while pending.len() < MAX_BATCH {
                    match rx.try_recv() {
                        Ok(row) => pending.push(row),
                        Err(_) => break,
                    }
                }
            }

            let batch = std::mem::take(&mut pending);
            let batch_len = batch.len();
            let _ = tokio::task::spawn_blocking(move || {
                with_conn(|conn| {
                    let written = insert_rows(conn, &batch)?;
                    Ok(written)
                })
            })
            .await;
            writes_since_prune += batch_len;
            if writes_since_prune >= 2000 {
                writes_since_prune = 0;
                let _ = tokio::task::spawn_blocking(|| {
                    with_conn(|conn| {
                        prune_if_needed(conn);
                        Ok(())
                    })
                })
                .await;
            }
        }
    });
    tx
});

/// 把一条译文排进持久缓存（非阻塞）。
pub fn queue_put(from: &str, to: &str, text: &str, translated: &str, engine: &str) {
    if DISABLED.load(Ordering::Relaxed) {
        return;
    }
    let source = text.trim();
    let target = translated.trim();
    if source.is_empty() || target.is_empty() || target.contains("[Translation Failed]") {
        return;
    }
    let row = CacheRow {
        key: cache_key(from, to, source),
        source_lang: from.trim().to_ascii_lowercase(),
        target_lang: to.trim().to_ascii_lowercase(),
        source_text: source.to_string(),
        translated_text: target.to_string(),
        engine: engine.trim().to_string(),
    };
    if WRITER.send(row).is_err() {
        warn!("[TranslateCache] writer 已关闭，停用持久缓存");
        DISABLED.store(true, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------- 词条整包缓存

/// 词条上限。一行是一个 JSON 包（含例句短语），比译文行大，条数给小一些。
const MAX_WORD_ROWS: i64 = 30_000;

fn word_key(word: &str, from: &str, to: &str, kind: &str) -> String {
    let normalized = format!(
        "{}|{}|{}|{}",
        word.trim().to_ascii_lowercase(),
        from.trim().to_ascii_lowercase(),
        to.trim().to_ascii_lowercase(),
        kind.trim()
    );
    blake3::hash(normalized.as_bytes()).to_hex().to_string()
}

/// 查词条缓存。命中时更新 used_at / hit_count。
/// `kind` 区分 basic（音标/释义）与 details（短语/例句/近义反义），两者分别缓存。
pub async fn get_word_entry(word: &str, from: &str, to: &str, kind: &str) -> Option<String> {
    if DISABLED.load(Ordering::Relaxed) || word.trim().is_empty() {
        return None;
    }
    let key = word_key(word, from, to, kind);
    tokio::task::spawn_blocking(move || {
        with_conn(|conn| {
            let payload: Option<String> = conn
                .query_row(
                    "SELECT payload FROM word_entry_cache WHERE key = ?1",
                    params![key],
                    |row| row.get(0),
                )
                .ok();
            if payload.is_some() {
                let _ = conn.execute(
                    "UPDATE word_entry_cache SET used_at = ?1, hit_count = hit_count + 1 WHERE key = ?2",
                    params![now_secs(), key],
                );
            }
            Ok(payload)
        })
        .flatten()
    })
    .await
    .ok()
    .flatten()
}

/// 写词条缓存。词条一次只产出一两条，直接在后台任务里落盘，不用走批量 writer。
pub fn queue_put_word_entry(word: &str, from: &str, to: &str, kind: &str, payload: &str) {
    if DISABLED.load(Ordering::Relaxed) {
        return;
    }
    let w = word.trim().to_string();
    let body = payload.trim().to_string();
    if w.is_empty() || body.is_empty() {
        return;
    }
    let key = word_key(&w, from, to, kind);
    let source_lang = from.trim().to_ascii_lowercase();
    let target_lang = to.trim().to_ascii_lowercase();
    let kind = kind.trim().to_string();

    tauri::async_runtime::spawn(async move {
        let _ = tokio::task::spawn_blocking(move || {
            with_conn(|conn| {
                let now = now_secs();
                conn.execute(
                    r#"
                    INSERT INTO word_entry_cache
                        (key, word, source_lang, target_lang, kind, payload, created_at, used_at, hit_count)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 0)
                    ON CONFLICT(key) DO UPDATE SET
                        payload = excluded.payload,
                        used_at = excluded.used_at
                    "#,
                    params![key, w, source_lang, target_lang, kind, body, now],
                )
                .map_err(|e| e.to_string())?;

                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM word_entry_cache", [], |r| r.get(0))
                    .unwrap_or(0);
                if count > MAX_WORD_ROWS {
                    let victims = (count - MAX_WORD_ROWS).max(MAX_WORD_ROWS / 10);
                    let _ = conn.execute(
                        "DELETE FROM word_entry_cache WHERE key IN (
                             SELECT key FROM word_entry_cache ORDER BY used_at ASC LIMIT ?1
                         )",
                        params![victims],
                    );
                }
                Ok(())
            })
        })
        .await;
    });
}

// ---------------------------------------------------------------- 维护 / 命令

#[derive(Serialize, Debug, Clone, Default)]
pub struct TranslateCacheStats {
    pub available: bool,
    pub db_path: String,
    pub rows: i64,
    pub total_hits: i64,
    /// 词条整包缓存的条数与命中次数。
    pub word_rows: i64,
    pub word_hits: i64,
    pub db_size_bytes: u64,
    pub max_rows: i64,
}

/// 缓存统计，用于设置页展示「已缓存多少段、命中多少次」。
#[tauri::command]
pub async fn translate_cache_stats() -> Result<TranslateCacheStats, String> {
    let path = resolve_db_path().unwrap_or_default();
    let db_path = path.to_string_lossy().to_string();
    let db_size_bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let counted = tokio::task::spawn_blocking(|| {
        with_conn(|conn| {
            let one = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0) };
            Ok((
                one("SELECT COUNT(*) FROM translation_cache"),
                one("SELECT COALESCE(SUM(hit_count), 0) FROM translation_cache"),
                one("SELECT COUNT(*) FROM word_entry_cache"),
                one("SELECT COALESCE(SUM(hit_count), 0) FROM word_entry_cache"),
            ))
        })
    })
    .await
    .ok()
    .flatten();

    let (rows, total_hits, word_rows, word_hits) = counted.unwrap_or((0, 0, 0, 0));
    Ok(TranslateCacheStats {
        available: !DISABLED.load(Ordering::Relaxed),
        db_path,
        rows,
        total_hits,
        word_rows,
        word_hits,
        db_size_bytes,
        max_rows: MAX_ROWS,
    })
}

/// 清空持久翻译缓存（译文 + 词条）。例如换了引擎、想强制重译。
#[tauri::command]
pub async fn translate_cache_clear() -> Result<i64, String> {
    let removed = tokio::task::spawn_blocking(|| {
        with_conn(|conn| {
            let one = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0) };
            let before = one("SELECT COUNT(*) FROM translation_cache")
                + one("SELECT COUNT(*) FROM word_entry_cache");
            conn.execute("DELETE FROM translation_cache", [])
                .map_err(|e| e.to_string())?;
            conn.execute("DELETE FROM word_entry_cache", [])
                .map_err(|e| e.to_string())?;
            let _ = conn.execute_batch("VACUUM");
            Ok(before)
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .unwrap_or(0);
    info!("[TranslateCache] 已清空 {} 条缓存（译文 + 词条）", removed);
    Ok(removed)
}

