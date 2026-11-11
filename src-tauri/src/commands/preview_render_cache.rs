//! 渲染结果（数学公式 / mermaid 图表）的持久化缓存（SQLite）。
//!
//! 为什么落盘：Markdown 渲染的耗时几乎全在 MathJax 编译公式（实测 ~1.1ms/条，
//! 240 条约 400ms，占整篇渲染 97%）和 mermaid 布局图表（实测 ~35ms/图，10 个约 320ms）。
//! 而这些结果**只由源码决定、永远不会变**。只做进程内记忆化的话，重开文档、新开窗口
//! 都要重算；落盘之后「见过的公式/图表」在任何会话、任何窗口都是零成本。
//!
//! 设计取舍（与 translate_cache.rs 保持一致）：
//! - 主键是 blake3(engine | 载荷) 的十六进制，定长、可索引，长公式也不会撑爆索引。
//!   `engine` 串里带上 MathJax/mermaid 的版本（由前端传入），引擎升级后旧行自然不再命中，
//!   不会把旧格式的结果串到新版本上。
//! - 写入走后台批量 writer：渲染链路只把行丢进 channel，不等磁盘，不拖慢首屏。
//! - 只按 LRU 上限裁剪，不设过期时间——渲染结果不会自己变质，留着就是纯收益。
//! - 命中时更新 used_at / hit_count，裁剪时优先淘汰最久没用到的。
//!
//! 关于 mermaid：存的是**渲染 id 前缀已归一化**的 SVG（前端负责把 render id 换成占位符）。
//! mermaid 会把 render id 写进十几个元素 id、`<style>#id{…}` 选择器和 `url(#id_…)` 箭头引用，
//! 所以直接复用缓存的 SVG 会在同一文档里造出重复 id、让 `url(#…)` 绑到别的图上；
//! 取回后由前端把占位符换成一个新的唯一 id 即可（实测：同源码不同 id 的产物只差这个前缀）。

use once_cell::sync::Lazy;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tracing::{info, warn};

const CACHE_DIR_NAME: &str = "guaxi_preview_render_cache";
const CACHE_DB_NAME: &str = "preview_render_cache.sqlite3";

/// 公式行数上限。一行是 SVG（几 KB），5 万行大概一两百 MB 量级，桌面端可接受。
const MAX_MATH_ROWS: i64 = 50_000;
/// 图表行数上限。图表 SVG 比公式大得多，条数给小一些。
const MAX_MERMAID_ROWS: i64 = 20_000;

/// 每种类型的落点：表名 + 上限。前端只传 kind 字符串，其余在这里收口。
fn table_spec(kind: &str) -> Option<(&'static str, i64)> {
    match kind {
        "math" => Some(("math_render_cache", MAX_MATH_ROWS)),
        "mermaid" => Some(("mermaid_render_cache", MAX_MERMAID_ROWS)),
        _ => None,
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 载荷 -> 主键。`payload` 由前端构造并保证唯一性（数学是 `display\0tex`，图表是 `configSig\0source`）。
fn cache_key(engine: &str, payload: &str) -> String {
    let joined = format!("{}\u{0}{}", engine.trim(), payload);
    blake3::hash(joined.as_bytes()).to_hex().to_string()
}

fn resolve_db_path() -> Result<PathBuf, String> {
    let exe_dir = env::current_exe()
        .map_err(|e| format!("获取可执行文件路径失败: {}", e))?
        .parent()
        .ok_or_else(|| "无法获取可执行文件所在目录".to_string())?
        .to_path_buf();
    let dir = exe_dir.join("repos").join(CACHE_DIR_NAME);
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("创建渲染缓存目录失败: {}", e))?;
    }
    Ok(dir.join(CACHE_DB_NAME))
}

fn open_conn(db_path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("打开渲染缓存库失败: {}", e))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("设置 WAL 失败: {}", e))?;
    // 缓存库丢一两条无所谓，用 NORMAL 换写入吞吐。
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| format!("设置 synchronous 失败: {}", e))?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS math_render_cache (
            key TEXT PRIMARY KEY,
            payload TEXT NOT NULL,
            svg TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT 0,
            used_at INTEGER NOT NULL DEFAULT 0,
            hit_count INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_math_render_cache_used_at
            ON math_render_cache(used_at);

        CREATE TABLE IF NOT EXISTS mermaid_render_cache (
            key TEXT PRIMARY KEY,
            payload TEXT NOT NULL,
            svg TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT 0,
            used_at INTEGER NOT NULL DEFAULT 0,
            hit_count INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_mermaid_render_cache_used_at
            ON mermaid_render_cache(used_at);
        "#,
    )
    .map_err(|e| format!("初始化渲染缓存表失败: {}", e))?;
    Ok(conn)
}

/// 全局连接。SQLite 单连接 + 互斥就够了：这里的读写都是毫秒级小事务。
static CONN: Lazy<Mutex<Option<Connection>>> = Lazy::new(|| Mutex::new(None));
/// 库打不开（只读安装目录等）时整块降级为「无持久缓存」，渲染本身完全不受影响。
static DISABLED: AtomicBool = AtomicBool::new(false);

fn with_conn<T>(f: impl FnOnce(&Connection) -> Result<T, String>) -> Option<T> {
    if DISABLED.load(Ordering::Relaxed) {
        return None;
    }
    let mut guard = match CONN.lock() {
        Ok(g) => g,
        Err(_) => {
            warn!("[PreviewRenderCache] 连接锁损坏，停用持久缓存");
            DISABLED.store(true, Ordering::Relaxed);
            return None;
        }
    };
    if guard.is_none() {
        let path = match resolve_db_path() {
            Ok(p) => p,
            Err(e) => {
                warn!("[PreviewRenderCache] {}，停用持久缓存", e);
                DISABLED.store(true, Ordering::Relaxed);
                return None;
            }
        };
        match open_conn(&path) {
            Ok(c) => {
                info!("[PreviewRenderCache] 缓存库就绪: {}", path.to_string_lossy());
                *guard = Some(c);
            }
            Err(e) => {
                warn!("[PreviewRenderCache] {}，停用持久缓存", e);
                DISABLED.store(true, Ordering::Relaxed);
                return None;
            }
        }
    }
    let conn = guard.as_ref()?;
    match f(conn) {
        Ok(v) => Some(v),
        Err(e) => {
            warn!("[PreviewRenderCache] 操作失败: {}", e);
            None
        }
    }
}

// ---------------------------------------------------------------- 写

#[derive(Debug, Clone)]
struct CacheRow {
    table: &'static str,
    max_rows: i64,
    key: String,
    payload: String,
    svg: String,
}

fn insert_rows(conn: &Connection, rows: &[CacheRow]) -> Result<usize, String> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("开启事务失败: {}", e))?;
    let now = now_secs();
    let mut written = 0usize;
    {
        // 两张表的语句形状一致，按表名分组各 prepare 一次。
        for table in ["math_render_cache", "mermaid_render_cache"] {
            let sql = format!(
                "INSERT INTO {} (key, payload, svg, created_at, used_at, hit_count)
                 VALUES (?1, ?2, ?3, ?4, ?4, 0)
                 ON CONFLICT(key) DO UPDATE SET used_at = ?4",
                table
            );
            let mut stmt = match tx.prepare(&sql) {
                Ok(s) => s,
                Err(e) => {
                    warn!("[PreviewRenderCache] prepare {} 失败: {}", table, e);
                    continue;
                }
            };
            for row in rows.iter().filter(|r| r.table == table) {
                match stmt.execute(params![row.key, row.payload, row.svg, now]) {
                    Ok(_) => written += 1,
                    Err(e) => warn!("[PreviewRenderCache] 写入 {} 失败: {}", table, e),
                }
            }
        }
    }
    tx.commit().map_err(|e| format!("提交事务失败: {}", e))?;
    Ok(written)
}

/// 超过上限就按 used_at 淘汰最久未用的一批。一次多删 10%，避免频繁触发。
fn prune_if_needed(conn: &Connection, table: &str, max_rows: i64) {
    let count: i64 = conn
        .query_row(&format!("SELECT COUNT(*) FROM {}", table), [], |r| r.get(0))
        .unwrap_or(0);
    if count <= max_rows {
        return;
    }
    let victims = (count - max_rows).max(max_rows / 10);
    let deleted = conn.execute(
        &format!(
            "DELETE FROM {} WHERE key IN (
                 SELECT key FROM {} ORDER BY used_at ASC LIMIT ?1
             )",
            table, table
        ),
        params![victims],
    );
    match deleted {
        Ok(n) => info!("[PreviewRenderCache] 淘汰 {} 行最久未用的缓存（{}）", n, table),
        Err(e) => warn!("[PreviewRenderCache] 淘汰 {} 失败: {}", table, e),
    }
}

static WRITER: Lazy<UnboundedSender<CacheRow>> = Lazy::new(|| {
    let (tx, mut rx) = unbounded_channel::<CacheRow>();
    // 后台 writer：攒够一批或到时间就落一次盘。渲染链路只做 send，不等磁盘。
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
            while pending.len() < MAX_BATCH {
                match rx.try_recv() {
                    Ok(row) => pending.push(row),
                    Err(_) => break,
                }
            }
            if pending.len() < MAX_BATCH {
                // 给后续行一个小窗口，把同一篇文档的写入合并成一个事务。
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
            let tables: Vec<(&'static str, i64)> = {
                let mut v: Vec<(&'static str, i64)> = Vec::new();
                for row in &batch {
                    if !v.iter().any(|(t, _)| *t == row.table) {
                        v.push((row.table, row.max_rows));
                    }
                }
                v
            };
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
                let _ = tokio::task::spawn_blocking(move || {
                    with_conn(|conn| {
                        for (table, max_rows) in tables {
                            prune_if_needed(conn, table, max_rows);
                        }
                        Ok(())
                    })
                })
                .await;
            }
        }
    });
    tx
});

/// 把一条渲染结果排进持久缓存（非阻塞）。
fn queue_put(kind: &str, engine: &str, payload: &str, svg: &str) {
    if DISABLED.load(Ordering::Relaxed) {
        return;
    }
    let Some((table, max_rows)) = table_spec(kind) else {
        warn!("[PreviewRenderCache] 未知的 kind: {}", kind);
        return;
    };
    if payload.is_empty() || svg.is_empty() {
        return;
    }
    let row = CacheRow {
        table,
        max_rows,
        key: cache_key(engine, payload),
        payload: payload.to_string(),
        svg: svg.to_string(),
    };
    if WRITER.send(row).is_err() {
        warn!("[PreviewRenderCache] writer 已关闭，停用持久缓存");
        DISABLED.store(true, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------- 读

/// 按主键批量查，顺带把命中行的 used_at / hit_count 更新掉（供 LRU 参考）。
fn select_batch(
    conn: &Connection,
    table: &str,
    keys: &[String],
) -> Result<Vec<(String, String)>, String> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; keys.len()].join(",");
    let sql = format!("SELECT key, svg FROM {} WHERE key IN ({})", table, placeholders);
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(keys.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows.flatten() {
        out.push(row);
    }
    let now = now_secs();
    for chunk in keys.chunks(400) {
        let ph = vec!["?"; chunk.len()].join(",");
        let sql = format!(
            "UPDATE {} SET used_at = ?1, hit_count = hit_count + 1 WHERE key IN ({})",
            table, ph
        );
        if let Ok(mut stmt) = conn.prepare(&sql) {
            let mut binds: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
            for k in chunk {
                binds.push(Box::new(k.clone()));
            }
            let _ = stmt.execute(rusqlite::params_from_iter(binds.iter().map(|b| b.as_ref())));
        }
    }
    Ok(out)
}

/// 批量查缓存。入参是前端构造的 payload，返回 `payload -> svg`，未命中的不出现在结果里。
async fn get_many(kind: &str, engine: &str, payloads: &[String]) -> HashMap<String, String> {
    let Some((table, _)) = table_spec(kind) else {
        return HashMap::new();
    };
    if payloads.is_empty() || DISABLED.load(Ordering::Relaxed) {
        return HashMap::new();
    }
    // 同一批里可能有重复 payload，先去重，避免白查。
    let mut key_to_payload: HashMap<String, String> = HashMap::new();
    for payload in payloads {
        if payload.is_empty() {
            continue;
        }
        key_to_payload
            .entry(cache_key(engine, payload))
            .or_insert_with(|| payload.clone());
    }
    if key_to_payload.is_empty() {
        return HashMap::new();
    }

    let keys: Vec<String> = key_to_payload.keys().cloned().collect();
    let found = tokio::task::spawn_blocking(move || {
        with_conn(|conn| {
            let mut all = Vec::new();
            // SQLite 默认变量上限 999，分批查。
            for chunk in keys.chunks(400) {
                all.extend(select_batch(conn, table, chunk)?);
            }
            Ok(all)
        })
    })
    .await
    .ok()
    .flatten()
    .unwrap_or_default();

    let mut out = HashMap::with_capacity(found.len());
    for (key, svg) in found {
        if let Some(payload) = key_to_payload.get(&key) {
            out.insert(payload.clone(), svg);
        }
    }
    out
}

// ---------------------------------------------------------------- 对外命令

#[derive(Debug, Deserialize)]
pub struct CacheEntry {
    /// 前端构造的稳定载荷（数学 `display\0tex`；图表 `configSig\0source`）
    pub payload: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct CacheStats {
    pub math_rows: i64,
    pub mermaid_rows: i64,
    pub disabled: bool,
}

/// 批量查渲染缓存。`engine` 由前端传入（内含 MathJax/mermaid 版本），参与主键。
#[tauri::command]
pub async fn preview_render_cache_get(
    kind: String,
    engine: String,
    payloads: Vec<String>,
) -> Result<HashMap<String, String>, String> {
    Ok(get_many(&kind, &engine, &payloads).await)
}

/// 批量回填渲染缓存。非阻塞：只入队列，由后台 writer 落盘。
#[tauri::command]
pub async fn preview_render_cache_put(
    kind: String,
    engine: String,
    entries: Vec<CacheEntry>,
) -> Result<usize, String> {
    let mut queued = 0usize;
    for entry in entries {
        queue_put(&kind, &engine, &entry.payload, &entry.value);
        queued += 1;
    }
    Ok(queued)
}

#[tauri::command]
pub async fn preview_render_cache_stats() -> Result<CacheStats, String> {
    let stats = tokio::task::spawn_blocking(|| {
        with_conn(|conn| {
            let one = |table: &str| -> i64 {
                conn.query_row(&format!("SELECT COUNT(*) FROM {}", table), [], |r| r.get(0))
                    .unwrap_or(0)
            };
            Ok(CacheStats {
                math_rows: one("math_render_cache"),
                mermaid_rows: one("mermaid_render_cache"),
                disabled: false,
            })
        })
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(stats.unwrap_or(CacheStats {
        math_rows: 0,
        mermaid_rows: 0,
        disabled: DISABLED.load(Ordering::Relaxed),
    }))
}

#[tauri::command]
pub async fn preview_render_cache_clear() -> Result<i64, String> {
    let removed = tokio::task::spawn_blocking(|| {
        with_conn(|conn| {
            let one = |table: &str| -> i64 {
                conn.query_row(&format!("SELECT COUNT(*) FROM {}", table), [], |r| r.get(0))
                    .unwrap_or(0)
            };
            let before = one("math_render_cache") + one("mermaid_render_cache");
            conn.execute("DELETE FROM math_render_cache", [])
                .map_err(|e| e.to_string())?;
            conn.execute("DELETE FROM mermaid_render_cache", [])
                .map_err(|e| e.to_string())?;
            let _ = conn.execute_batch("VACUUM");
            Ok(before)
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .unwrap_or(0);
    info!("[PreviewRenderCache] 已清空 {} 条缓存", removed);
    Ok(removed)
}
