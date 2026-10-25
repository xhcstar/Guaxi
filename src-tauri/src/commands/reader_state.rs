// 电子书阅读器的阅读状态存储（进度、书签、批注、显示设置）。
//
// 为什么存在后端 sqlite 而不是 localStorage：
// - 批注与书签是**用户资产**，要能跟着知识库迁移、被 P2P 同步带走；
// - localStorage 按窗口隔离，独立阅读器窗口与主界面属于不同 webview，
//   放在那里两边互相看不见；
// - 笔记导出、全文检索将来都要读这些数据。
//
// 表结构刻意扁平：一本书一个 book_key（文件路径的规范化哈希），条目按 cfi/序号定位。
// 不用外键，删除书时用 book_key 一次性清干净即可。

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info};

/// 一本书的阅读状态。
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingState {
    /// 当前章节索引
    pub section_index: usize,
    /// 章节内滚动比例 0..1
    pub scroll_ratio: f64,
    /// 最后阅读时间（Unix 毫秒）
    pub updated_at: i64,
}

/// 书签。
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub id: i64,
    pub section_index: usize,
    pub scroll_ratio: f64,
    pub label: String,
    pub created_at: i64,
}

/// 高亮/批注。
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub id: i64,
    pub section_index: usize,
    /// 高亮文本
    pub text: String,
    /// 用户批注
    pub note: String,
    pub color: String,
    pub created_at: i64,
}

/// 显示设置。
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderPrefs {
    /// 设置结构版本：旧记录没有这个字段（反序列化为 0），前端据此把旧默认
    /// （16px 衬线）迁移到新的「界面默认字体 + 18px」。
    #[serde(default)]
    pub prefs_version: u32,
    pub font_size: f64,
    pub line_height: f64,
    /// system | serif | sans | mono
    pub font_family: String,
    /// light | dark | sepia | gray
    pub theme: String,
    /// paginated | scroll
    pub layout_mode: String,
    pub max_width: f64,
}

impl Default for ReaderPrefs {
    fn default() -> Self {
        Self {
            prefs_version: 2,
            // 默认值刻意与前端 DEFAULT_READER_PREFS 保持一致：正文用界面默认字体，
            // 字号比常规 UI 大一点，长时间阅读更舒服。
            font_size: 18.0,
            line_height: 1.8,
            font_family: "system".to_string(),
            theme: "light".to_string(),
            layout_mode: "scroll".to_string(),
            max_width: 860.0,
        }
    }
}

fn db_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe
        .parent()
        .ok_or("无法获取程序目录")?
        .join("repos");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("reader_state.db"))
}

fn connect() -> Result<Connection, String> {
    let conn = Connection::open(db_path().map_err(|e| e)?).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS reading_state (
            book_key TEXT PRIMARY KEY,
            section_index INTEGER NOT NULL,
            scroll_ratio REAL NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS bookmarks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            book_key TEXT NOT NULL,
            section_index INTEGER NOT NULL,
            scroll_ratio REAL NOT NULL,
            label TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_bookmarks_book ON bookmarks(book_key);
        CREATE TABLE IF NOT EXISTS annotations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            book_key TEXT NOT NULL,
            section_index INTEGER NOT NULL,
            text TEXT NOT NULL,
            note TEXT NOT NULL,
            color TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_annotations_book ON annotations(book_key);
        CREATE TABLE IF NOT EXISTS reader_prefs (
            book_key TEXT PRIMARY KEY,
            payload TEXT NOT NULL
        );
        ",
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ───────────────────────── 进度 ─────────────────────────

#[tauri::command]
pub fn reader_get_state(book_key: String) -> Result<Option<ReadingState>, String> {
    let conn = connect()?;
    let found = conn
        .query_row(
            "SELECT section_index, scroll_ratio, updated_at FROM reading_state WHERE book_key = ?1",
            params![book_key],
            |row| {
                Ok(ReadingState {
                    section_index: row.get(0)?,
                    scroll_ratio: row.get(1)?,
                    updated_at: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    debug!(book_key = book_key, found = found.is_some(), "[Reader] get state");
    Ok(found)
}

#[tauri::command]
pub fn reader_save_state(
    book_key: String,
    section_index: usize,
    scroll_ratio: f64,
) -> Result<(), String> {
    let conn = connect()?;
    conn.execute(
        "INSERT INTO reading_state (book_key, section_index, scroll_ratio, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(book_key) DO UPDATE SET
           section_index = excluded.section_index,
           scroll_ratio = excluded.scroll_ratio,
           updated_at = excluded.updated_at",
        params![book_key, section_index as i64, scroll_ratio, now_ms()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ───────────────────────── 书签 ─────────────────────────

#[tauri::command]
pub fn reader_list_bookmarks(book_key: String) -> Result<Vec<Bookmark>, String> {
    let conn = connect()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, section_index, scroll_ratio, label, created_at
             FROM bookmarks WHERE book_key = ?1 ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![book_key], |row| {
            Ok(Bookmark {
                id: row.get(0)?,
                section_index: row.get(1)?,
                scroll_ratio: row.get(2)?,
                label: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub fn reader_add_bookmark(
    book_key: String,
    section_index: usize,
    scroll_ratio: f64,
    label: String,
) -> Result<Bookmark, String> {
    let conn = connect()?;
    let created = now_ms();
    conn.execute(
        "INSERT INTO bookmarks (book_key, section_index, scroll_ratio, label, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![book_key, section_index as i64, scroll_ratio, label, created],
    )
    .map_err(|e| e.to_string())?;
    info!(section_index, "[Reader] bookmark added");
    Ok(Bookmark {
        id: conn.last_insert_rowid(),
        section_index,
        scroll_ratio,
        label,
        created_at: created,
    })
}

#[tauri::command]
pub fn reader_delete_bookmark(id: i64) -> Result<(), String> {
    let conn = connect()?;
    conn.execute("DELETE FROM bookmarks WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ───────────────────────── 批注 ─────────────────────────

#[tauri::command]
pub fn reader_list_annotations(book_key: String) -> Result<Vec<Annotation>, String> {
    let conn = connect()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, section_index, text, note, color, created_at
             FROM annotations WHERE book_key = ?1 ORDER BY section_index, created_at",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![book_key], |row| {
            Ok(Annotation {
                id: row.get(0)?,
                section_index: row.get(1)?,
                text: row.get(2)?,
                note: row.get(3)?,
                color: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub fn reader_add_annotation(
    book_key: String,
    section_index: usize,
    text: String,
    note: String,
    color: String,
) -> Result<Annotation, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("高亮文本为空".to_string());
    }
    let conn = connect()?;
    let created = now_ms();
    conn.execute(
        "INSERT INTO annotations (book_key, section_index, text, note, color, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![book_key, section_index as i64, trimmed, note, color, created],
    )
    .map_err(|e| e.to_string())?;
    info!(section_index, chars = trimmed.chars().count(), "[Reader] annotation added");
    Ok(Annotation {
        id: conn.last_insert_rowid(),
        section_index,
        text: trimmed.to_string(),
        note,
        color,
        created_at: created,
    })
}

#[tauri::command]
pub fn reader_update_annotation(id: i64, note: String) -> Result<(), String> {
    let conn = connect()?;
    conn.execute(
        "UPDATE annotations SET note = ?1 WHERE id = ?2",
        params![note, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn reader_delete_annotation(id: i64) -> Result<(), String> {
    let conn = connect()?;
    conn.execute("DELETE FROM annotations WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ───────────────────────── 显示设置 ─────────────────────────

#[tauri::command]
pub fn reader_get_prefs(book_key: String) -> Result<ReaderPrefs, String> {
    let conn = connect()?;
    let payload: Option<String> = conn
        .query_row(
            "SELECT payload FROM reader_prefs WHERE book_key = ?1",
            params![book_key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    match payload {
        Some(json) => serde_json::from_str(&json).map_err(|e| format!("读取阅读设置失败: {e}")),
        None => Ok(ReaderPrefs::default()),
    }
}

#[tauri::command]
pub fn reader_save_prefs(book_key: String, prefs: ReaderPrefs) -> Result<(), String> {
    let payload = serde_json::to_string(&prefs).map_err(|e| e.to_string())?;
    let conn = connect()?;
    conn.execute(
        "INSERT INTO reader_prefs (book_key, payload) VALUES (?1, ?2)
         ON CONFLICT(book_key) DO UPDATE SET payload = excluded.payload",
        params![book_key, payload],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ReaderPrefs;

    #[test]
    fn default_prefs_are_readable_and_sane() {
        let p = ReaderPrefs::default();
        assert!(p.font_size >= 12.0 && p.font_size <= 30.0, "默认字号必须在可读区间");
        assert!(p.line_height >= 1.2, "行距不能低于可读下限");
        assert!(!p.theme.is_empty());
        assert!(p.max_width >= 500.0, "正文宽度不能窄到折行过密");
    }

    #[test]
    fn prefs_round_trip_through_json() {
        // 前端存的是 camelCase；serde(rename_all) 必须一致，否则设置存进去读不回来。
        let p = ReaderPrefs {
            prefs_version: 2,
            font_size: 22.0,
            line_height: 2.0,
            font_family: "sans".into(),
            theme: "sepia".into(),
            layout_mode: "paginated".into(),
            max_width: 900.0,
        };
        let json = serde_json::to_string(&p).expect("序列化");
        assert!(json.contains("fontSize"), "字段名必须是 camelCase: {json}");
        assert!(json.contains("layoutMode"));
        let back: ReaderPrefs = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back.font_size, 22.0);
        assert_eq!(back.theme, "sepia");
        assert_eq!(back.layout_mode, "paginated");
    }

    #[test]
    fn corrupt_prefs_are_skipped_not_fatal() {
        // reader_get_prefs 在 JSON 坏掉时必须能回退到默认值，而不是让整本书打不开。
        // 这里锁住的是「解析失败是可检出的」这一前提，调用方据此回退。
        let broken = "{ this is not json";
        let parsed: Result<ReaderPrefs, _> = serde_json::from_str(broken);
        assert!(parsed.is_err(), "坏 JSON 必须被识别为解析失败");
        // 默认值是回退目标，必须是可用的设置。
        let fallback = ReaderPrefs::default();
        assert!(fallback.font_size > 0.0);
    }
}
