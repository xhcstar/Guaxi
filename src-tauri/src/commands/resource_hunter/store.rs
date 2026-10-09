//! 资源库持久化：SQLite。资源条目、下载任务、查询历史、设置。
//! 采用「每次操作开一个连接」的模式（与 translate_cache 一致），WAL 下并发安全；
//! 高频的下载进度由调度器在内存中持有，只周期性落盘，写入并不密集。
use super::model::{DownloadStatus, DownloadTask, ResourceItem, ResourceKind};
use once_cell::sync::OnceCell;
use rusqlite::Connection;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

static DB_PATH: OnceCell<PathBuf> = OnceCell::new();
static LIB_VERSION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// 资源库版本号：任何写操作后自增，供全文索引判断是否需要重建。
pub fn library_version() -> u64 {
    LIB_VERSION.load(std::sync::atomic::Ordering::Relaxed)
}
fn bump_library() {
    LIB_VERSION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// 模块数据根目录：<app_data>/resource_hunter。
fn data_root(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {e}"))?
        .join("resource_hunter");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {e}"))?;
    Ok(dir)
}

/// 默认下载落地目录：优先用户设置的 download_dir，否则 <app_data>/resource_hunter/downloads。
pub fn downloads_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(Some(custom)) = get_setting("download_dir") {
        let custom = custom.trim().to_string();
        if !custom.is_empty() {
            let p = PathBuf::from(&custom);
            std::fs::create_dir_all(&p).map_err(|e| format!("创建下载目录失败: {e}"))?;
            return Ok(p);
        }
    }
    let dir = data_root(app)?.join("downloads");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建下载目录失败: {e}"))?;
    Ok(dir)
}

/// 工具（yt-dlp/ffmpeg）缓存目录：<app_data>/resource_hunter/tools。
pub fn tools_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = data_root(app)?.join("tools");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建工具目录失败: {e}"))?;
    Ok(dir)
}

fn open() -> Result<Connection, String> {
    let path = DB_PATH.get().ok_or("资源库未初始化")?;
    let conn = Connection::open(path).map_err(|e| format!("打开资源库失败: {e}"))?;
    conn.pragma_update(None, "journal_mode", "WAL").ok();
    conn.pragma_update(None, "busy_timeout", 5000).ok();
    Ok(conn)
}

/// 初始化：解析路径、建表。命令入口首次调用时执行。
pub fn init(app: &AppHandle) -> Result<(), String> {
    let path = data_root(app)?.join("library.db");
    let _ = DB_PATH.set(path);
    let conn = open()?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS resources (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            kind TEXT NOT NULL,
            source TEXT,
            source_url TEXT,
            download_url TEXT,
            format TEXT,
            local_path TEXT,
            size_hint INTEGER,
            hash TEXT,
            tags TEXT,
            status TEXT NOT NULL DEFAULT 'discovered',
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS downloads (
            id TEXT PRIMARY KEY,
            payload TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS queries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            keyword TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        "#,
    )
    .map_err(|e| format!("建表失败: {e}"))?;
    Ok(())
}

fn kind_str(k: ResourceKind) -> &'static str {
    match k {
        ResourceKind::Book => "book",
        ResourceKind::Paper => "paper",
        ResourceKind::Video => "video",
        ResourceKind::Audio => "audio",
        ResourceKind::Image => "image",
        ResourceKind::Code => "code",
        ResourceKind::Dataset => "dataset",
        ResourceKind::Other => "other",
    }
}

/// 落地一条已入库资源（下载完成或收藏）。
pub fn upsert_resource(item: &ResourceItem, local_path: Option<&str>) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        r#"INSERT INTO resources
           (id,title,kind,source,source_url,download_url,format,local_path,size_hint,tags,status,created_at)
           VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
           ON CONFLICT(id) DO UPDATE SET
             local_path=excluded.local_path,
             status=excluded.status"#,
        rusqlite::params![
            item.id,
            item.title,
            kind_str(item.kind),
            item.source,
            item.source_url,
            item.download_url,
            item.format,
            local_path,
            item.size_hint.map(|v| v as i64),
            item.tags.join(","),
            if local_path.is_some() { "downloaded" } else { "discovered" },
            chrono::Utc::now().timestamp(),
        ],
    )
    .map_err(|e| format!("写入资源失败: {e}"))?;
    bump_library();
    Ok(())
}

/// 持久化/更新一个下载任务（整体 JSON 落盘）。
pub fn save_task(task: &DownloadTask) -> Result<(), String> {
    let conn = open()?;
    let payload = serde_json::to_string(task).map_err(|e| e.to_string())?;
    let status = serde_json::to_value(task.status)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "queued".into());
    conn.execute(
        r#"INSERT INTO downloads (id,payload,status,created_at)
           VALUES (?1,?2,?3,?4)
           ON CONFLICT(id) DO UPDATE SET payload=excluded.payload, status=excluded.status"#,
        rusqlite::params![task.id, payload, status, task.created_at],
    )
    .map_err(|e| format!("写入下载任务失败: {e}"))?;
    Ok(())
}

/// 载入所有未完成/历史任务（重启恢复队列）。
pub fn load_tasks() -> Result<Vec<DownloadTask>, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare("SELECT payload FROM downloads ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        if let Ok(json) = row {
            if let Ok(t) = serde_json::from_str::<DownloadTask>(&json) {
                out.push(t);
            }
        }
    }
    Ok(out)
}

pub fn delete_task(id: &str) -> Result<(), String> {
    let conn = open()?;
    conn.execute("DELETE FROM downloads WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 已入库资源列表（可按类型过滤，空表示全部）。
pub fn list_resources(kind: Option<ResourceKind>) -> Result<Vec<serde_json::Value>, String> {
    let conn = open()?;
    let mut sql = String::from(
        "SELECT id,title,kind,source,source_url,local_path,size_hint,tags,status,created_at FROM resources",
    );
    if kind.is_some() {
        sql.push_str(" WHERE kind=?1");
    }
    sql.push_str(" ORDER BY created_at DESC LIMIT 500");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let map_row = |r: &rusqlite::Row| -> rusqlite::Result<serde_json::Value> {
        Ok(serde_json::json!({
            "id": r.get::<_, String>(0)?,
            "title": r.get::<_, String>(1)?,
            "kind": r.get::<_, String>(2)?,
            "source": r.get::<_, Option<String>>(3)?,
            "source_url": r.get::<_, Option<String>>(4)?,
            "local_path": r.get::<_, Option<String>>(5)?,
            "size_hint": r.get::<_, Option<i64>>(6)?,
            "tags": r.get::<_, Option<String>>(7)?,
            "status": r.get::<_, String>(8)?,
            "created_at": r.get::<_, i64>(9)?,
        }))
    };
    let rows = if let Some(k) = kind {
        stmt.query_map([kind_str(k)], map_row).map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect()
    } else {
        stmt.query_map([], map_row).map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect()
    };
    Ok(rows)
}

pub fn record_query(keyword: &str) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "INSERT INTO queries (keyword,created_at) VALUES (?1,?2)",
        rusqlite::params![keyword, chrono::Utc::now().timestamp()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_setting(key: &str) -> Result<Option<String>, String> {
    let conn = open()?;
    let v = conn
        .query_row("SELECT value FROM settings WHERE key=?1", [key], |r| r.get::<_, String>(0))
        .ok();
    Ok(v)
}

pub fn set_setting(key: &str, value: &str) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "INSERT INTO settings (key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 删除一条资源库记录（可选连带删除本地文件）。
pub fn delete_resource(id: &str, remove_file: bool) -> Result<(), String> {
    let conn = open()?;
    if remove_file {
        if let Ok(Some(path)) = conn
            .query_row("SELECT local_path FROM resources WHERE id=?1", [id], |r| {
                r.get::<_, Option<String>>(0)
            })
            .map_err(|e| e.to_string())
        {
            let _ = std::fs::remove_file(&path);
        }
    }
    conn.execute("DELETE FROM resources WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    bump_library();
    Ok(())
}

/// 更新一条资源的标签（逗号分隔存储）。
pub fn update_resource_tags(id: &str, tags: &[String]) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "UPDATE resources SET tags=?1 WHERE id=?2",
        rusqlite::params![tags.join(","), id],
    )
    .map_err(|e| e.to_string())?;
    bump_library();
    Ok(())
}

/// 资源库里出现过的所有标签（去重）。
pub fn all_tags() -> Result<Vec<String>, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare("SELECT tags FROM resources WHERE tags IS NOT NULL AND tags<>''")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let mut set = std::collections::BTreeSet::new();
    for row in rows.flatten() {
        for t in row.split(',') {
            let t = t.trim();
            if !t.is_empty() {
                set.insert(t.to_string());
            }
        }
    }
    Ok(set.into_iter().collect())
}

/// 导出全部资源（不限条数），供备份/同步。
pub fn export_all() -> Result<Vec<serde_json::Value>, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare("SELECT id,title,kind,source,source_url,download_url,format,local_path,size_hint,tags,status,created_at FROM resources ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "id": r.get::<_, String>(0)?,
                "title": r.get::<_, String>(1)?,
                "kind": r.get::<_, String>(2)?,
                "source": r.get::<_, Option<String>>(3)?,
                "source_url": r.get::<_, Option<String>>(4)?,
                "download_url": r.get::<_, Option<String>>(5)?,
                "format": r.get::<_, Option<String>>(6)?,
                "local_path": r.get::<_, Option<String>>(7)?,
                "size_hint": r.get::<_, Option<i64>>(8)?,
                "tags": r.get::<_, Option<String>>(9)?,
                "status": r.get::<_, String>(10)?,
                "created_at": r.get::<_, i64>(11)?,
            }))
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|x| x.ok()).collect())
}

/// 导入资源清单（按 id upsert），返回导入条数。
pub fn import_rows(rows: &[serde_json::Value]) -> Result<usize, String> {
    let conn = open()?;
    let mut n = 0usize;
    let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(|s| s.to_string());
    for row in rows {
        let id = match s(row, "id") {
            Some(x) if !x.is_empty() => x,
            _ => continue,
        };
        conn.execute(
            r#"INSERT INTO resources
               (id,title,kind,source,source_url,download_url,format,local_path,size_hint,tags,status,created_at)
               VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
               ON CONFLICT(id) DO UPDATE SET
                 title=excluded.title, tags=excluded.tags, local_path=excluded.local_path, status=excluded.status"#,
            rusqlite::params![
                id,
                s(row, "title").unwrap_or_default(),
                s(row, "kind").unwrap_or_else(|| "other".into()),
                s(row, "source"),
                s(row, "source_url"),
                s(row, "download_url"),
                s(row, "format"),
                s(row, "local_path"),
                row.get("size_hint").and_then(|v| v.as_i64()),
                s(row, "tags"),
                s(row, "status").unwrap_or_else(|| "downloaded".into()),
                row.get("created_at").and_then(|v| v.as_i64()).unwrap_or_else(|| chrono::Utc::now().timestamp()),
            ],
        )
        .map_err(|e| e.to_string())?;
        n += 1;
    }
    bump_library();
    Ok(n)
}

#[allow(dead_code)]
fn _status_unused(_: DownloadStatus) {}
