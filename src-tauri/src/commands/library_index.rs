//! 库浏览的持久化文件索引（SQLite）。
//!
//! 取代原先"每 30s 全量遍历 + 整库 Vec 驻留内存"的做法:
//! - 首次访问某库时做一次遍历,把每个文件的元数据写入 SQLite(一次会话一次);
//! - 之后用 notify 监听库根,增量 upsert/删除,不再全量重扫;
//! - 分页/排序/计数全部走 SQL 索引,十万份也只在结果页那几十行上花开销,内存 O(页)。
//!
//! DB 落在 app_data_dir/library_index.db。跨重启保留;每次会话首访某库会重建该库
//! 行(捕捉软件关闭期间的增删),之后交给 watcher 保持同步。

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

/// 一行文件记录（供 library_site 映射成 Doc）。
pub struct LibRow {
    pub rel: String,
    pub name: String,
    pub ext: String,
    pub size: u64,
    pub mtime: i64,
}

static DB: Lazy<Mutex<Option<Connection>>> = Lazy::new(|| Mutex::new(None));
/// 本会话已完成首次索引的库（避免重复初始遍历）。
static INDEXED: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));
/// 保活的 watcher,一库一个。丢弃即停止监听,所以必须存起来。
static WATCHERS: Lazy<Mutex<std::collections::HashMap<String, RecommendedWatcher>>> =
    Lazy::new(|| Mutex::new(std::collections::HashMap::new()));

fn db_path() -> PathBuf {
    // 优先 app_data_dir;取不到 AppHandle 时退回临时目录,保证始终可用。
    if let Some(app) = crate::cognition::runtime::app_handle() {
        use tauri::Manager;
        if let Ok(dir) = app.path().app_data_dir() {
            let _ = std::fs::create_dir_all(&dir);
            return dir.join("library_index.db");
        }
    }
    std::env::temp_dir().join("guaxi_library_index.db")
}

fn with_db<T>(f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T, String> {
    let mut guard = DB.lock().unwrap();
    if guard.is_none() {
        let conn = Connection::open(db_path()).map_err(|e| format!("打开库索引失败: {e}"))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS files(
               lib TEXT NOT NULL, rel TEXT NOT NULL, name TEXT NOT NULL,
               ext TEXT NOT NULL, size INTEGER NOT NULL, mtime INTEGER NOT NULL,
               PRIMARY KEY(lib, rel));
             CREATE INDEX IF NOT EXISTS idx_files_mtime ON files(lib, mtime);
             CREATE INDEX IF NOT EXISTS idx_files_name ON files(lib, name);
             CREATE INDEX IF NOT EXISTS idx_files_size ON files(lib, size);",
        )
        .map_err(|e| format!("初始化库索引表失败: {e}"))?;
        *guard = Some(conn);
    }
    f(guard.as_ref().unwrap()).map_err(|e| format!("库索引查询失败: {e}"))
}
// PLACEHOLDER_INDEX

/// 由绝对路径与库根算出入库字段。目录/无法 stat 的返回 None。
fn file_fields(root: &Path, path: &Path) -> Option<(String, String, String, u64, i64)> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let rel = path.strip_prefix(root).ok()?.to_string_lossy().replace('\\', "/");
    if rel.is_empty() || rel.contains("/.git/") || rel.starts_with(".git/") {
        return None;
    }
    let name = path.file_name()?.to_string_lossy().to_string();
    let ext = path.extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Some((rel, name, ext, meta.len(), mtime))
}

fn upsert_path(conn: &Connection, lib: &str, root: &Path, path: &Path) {
    if let Some((rel, name, ext, size, mtime)) = file_fields(root, path) {
        let _ = conn.execute(
            "INSERT INTO files(lib, rel, name, ext, size, mtime) VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(lib, rel) DO UPDATE SET name=?3, ext=?4, size=?5, mtime=?6",
            rusqlite::params![lib, rel, name, ext, size as i64, mtime],
        );
    }
}

fn delete_path(conn: &Connection, lib: &str, root: &Path, path: &Path) {
    if let Ok(stripped) = path.strip_prefix(root) {
        let rel = stripped.to_string_lossy().replace('\\', "/");
        let _ = conn.execute("DELETE FROM files WHERE lib=?1 AND rel=?2", rusqlite::params![lib, rel]);
    }
}

/// 首次索引某库:清掉旧行,遍历一次全量写入(捕捉软件关闭期间的增删)。
fn initial_index(lib: &str, root: &Path) -> Result<(), String> {
    let mut rows: Vec<(String, String, String, u64, i64)> = Vec::new();
    for entry in ignore::WalkBuilder::new(root).hidden(true).git_ignore(false).build().flatten() {
        if let Some(f) = file_fields(root, entry.path()) {
            rows.push(f);
        }
    }
    with_db(|conn| {
        conn.execute("DELETE FROM files WHERE lib=?1", rusqlite::params![lib])?;
        let tx_sql = "INSERT OR REPLACE INTO files(lib, rel, name, ext, size, mtime) VALUES(?1,?2,?3,?4,?5,?6)";
        conn.execute_batch("BEGIN")?;
        {
            let mut stmt = conn.prepare(tx_sql)?;
            for (rel, name, ext, size, mtime) in &rows {
                stmt.execute(rusqlite::params![lib, rel, name, ext, *size as i64, mtime])?;
            }
        }
        conn.execute_batch("COMMIT")?;
        Ok(())
    })
}

/// 起一个 notify watcher,增量把库根的增删改同步进 SQLite。
fn start_watcher(lib: &str, root: &Path) {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(_) => return,
    };
    if watcher.watch(root, RecursiveMode::Recursive).is_err() {
        return;
    }
    WATCHERS.lock().unwrap().insert(lib.to_string(), watcher);

    let lib = lib.to_string();
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        for res in rx {
            let event: notify::Event = match res {
                Ok(e) => e,
                Err(_) => continue,
            };
            let _ = with_db(|conn| {
                for path in &event.paths {
                    // 存在且是文件 → upsert;否则(删除/改名旧路径) → delete。
                    if path.is_file() {
                        upsert_path(conn, &lib, &root, path);
                    } else {
                        delete_path(conn, &lib, &root, path);
                    }
                }
                Ok(())
            });
        }
    });
}

fn ensure_indexed(lib: &str, root: &Path) {
    {
        let indexed = INDEXED.lock().unwrap();
        if indexed.contains(lib) {
            return;
        }
    }
    if initial_index(lib, root).is_ok() {
        start_watcher(lib, root);
        INDEXED.lock().unwrap().insert(lib.to_string());
    }
}

fn order_clause(sort: &str) -> &'static str {
    match sort {
        "name" => "name COLLATE NOCASE ASC",
        "size" => "size DESC",
        _ => "mtime DESC",
    }
}

/// 分页查询某库(SQL 索引排序 + LIMIT/OFFSET),返回 (总数, 该页行)。
pub fn list_page(lib: &str, root: &Path, sort: &str, offset: usize, limit: usize) -> (usize, Vec<LibRow>) {
    ensure_indexed(lib, root);
    let order = order_clause(sort);
    with_db(|conn| {
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM files WHERE lib=?1",
            rusqlite::params![lib],
            |r| r.get(0),
        )?;
        let sql = format!(
            "SELECT rel, name, ext, size, mtime FROM files WHERE lib=?1 ORDER BY {order} LIMIT ?2 OFFSET ?3"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params![lib, limit as i64, offset as i64], |r| {
                Ok(LibRow {
                    rel: r.get(0)?,
                    name: r.get(1)?,
                    ext: r.get(2)?,
                    size: r.get::<_, i64>(3)? as u64,
                    mtime: r.get(4)?,
                })
            })?
            .filter_map(|x| x.ok())
            .collect::<Vec<_>>();
        Ok((total as usize, rows))
    })
    .unwrap_or((0, Vec::new()))
}
