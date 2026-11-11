//! 简历工坊的本地存储：多份简历、版本快照与自定义模板。
//!
//! 数据库放在 `repos/guaxi_resume/resume.sqlite3`，与 TODO、语言学习等模块一致，
//! 便于用户整目录备份。简历正文以 JSON 存储，后端只负责索引字段（标题、岗位、
//! 模板），结构升级由前端的 schemaVersion 决定。

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const RESUME_DIR_NAME: &str = "guaxi_resume";
const RESUME_DB_NAME: &str = "resume.sqlite3";
const SCHEMA_VERSION: i64 = 1;
const AUTO_VERSION_LIMIT: i64 = 20;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResumeDbInfo {
    pub db_path: String,
    pub schema_version: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResumeDocument {
    pub id: String,
    pub title: String,
    pub template_id: String,
    pub data_json: String,
    pub favorite: bool,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResumeSummary {
    pub id: String,
    pub title: String,
    pub role: String,
    pub template_id: String,
    pub favorite: bool,
    pub section_count: i64,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResumeVersion {
    pub id: String,
    pub resume_id: String,
    pub kind: String,
    pub label: String,
    pub data_json: String,
    pub pinned: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResumeTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub html: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SaveDocumentInput {
    pub id: Option<String>,
    pub title: String,
    pub template_id: String,
    pub data_json: String,
    pub favorite: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SaveVersionInput {
    pub resume_id: String,
    pub label: String,
    pub kind: String,
    pub data_json: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SaveTemplateInput {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub html: String,
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

fn resolve_db_path() -> Result<PathBuf, String> {
    let dir = crate::paths::repos_dir().join(RESUME_DIR_NAME);
    fs::create_dir_all(&dir).map_err(|e| format!("创建简历数据目录失败: {}", e))?;
    Ok(dir.join(RESUME_DB_NAME))
}

fn open_conn(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| format!("打开简历数据库失败: {}", e))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| e.to_string())?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| e.to_string())?;
    Ok(conn)
}

fn with_db<T>(f: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
    let path = resolve_db_path()?;
    let conn = open_conn(&path)?;
    ensure_schema(&conn)?;
    f(&conn)
}

fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS resume_meta (
            k TEXT PRIMARY KEY,
            v TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS resume_documents (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT '',
            template_id TEXT NOT NULL DEFAULT 'builtin:blueprint',
            data_json TEXT NOT NULL,
            favorite INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS resume_versions (
            id TEXT PRIMARY KEY,
            resume_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            label TEXT NOT NULL,
            data_json TEXT NOT NULL,
            pinned INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS resume_templates (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            html TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_resume_versions_resume
            ON resume_versions(resume_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_resume_documents_updated
            ON resume_documents(updated_at DESC);",
    )
    .map_err(|e| e.to_string())?;

    let current: Option<String> = conn
        .query_row("SELECT v FROM resume_meta WHERE k='schema_version'", [], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    if current.is_none() {
        conn.execute(
            "INSERT INTO resume_meta(k, v) VALUES('schema_version', ?1)",
            params![SCHEMA_VERSION.to_string()],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn profile_field(data_json: &str, key: &str) -> String {
    serde_json::from_str::<serde_json::Value>(data_json)
        .ok()
        .and_then(|v| v.get("profile").and_then(|p| p.get(key)).and_then(|x| x.as_str()).map(|s| s.to_string()))
        .unwrap_or_default()
}

fn section_count(data_json: &str) -> i64 {
    serde_json::from_str::<serde_json::Value>(data_json)
        .ok()
        .and_then(|v| v.get("sections").and_then(|s| s.as_array()).map(|a| a.len() as i64))
        .unwrap_or(0)
}

fn row_to_document(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResumeDocument> {
    Ok(ResumeDocument {
        id: row.get(0)?,
        title: row.get(1)?,
        template_id: row.get(2)?,
        data_json: row.get(3)?,
        favorite: row.get::<_, i64>(4)? != 0,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        deleted_at: row.get(7)?,
    })
}

fn row_to_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResumeSummary> {
    let data_json: String = row.get(6)?;
    let role = profile_field(&data_json, "target");
    let sections = section_count(&data_json);
    Ok(ResumeSummary {
        id: row.get(0)?,
        title: row.get(1)?,
        role: if role.is_empty() { profile_field(&data_json, "title") } else { role },
        template_id: row.get(2)?,
        favorite: row.get::<_, i64>(3)? != 0,
        section_count: sections,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        deleted_at: row.get(7)?,
    })
}

fn row_to_version(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResumeVersion> {
    Ok(ResumeVersion {
        id: row.get(0)?,
        resume_id: row.get(1)?,
        kind: row.get(2)?,
        label: row.get(3)?,
        data_json: row.get(4)?,
        pinned: row.get::<_, i64>(5)? != 0,
        created_at: row.get(6)?,
    })
}

fn row_to_template(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResumeTemplate> {
    Ok(ResumeTemplate {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        html: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

#[tauri::command]
pub fn resume_init() -> Result<ResumeDbInfo, String> {
    with_db(|_| {
        let path = resolve_db_path()?;
        Ok(ResumeDbInfo {
            db_path: path.to_string_lossy().to_string(),
            schema_version: SCHEMA_VERSION,
        })
    })
}

#[tauri::command]
pub fn resume_list_documents(include_deleted: Option<bool>) -> Result<Vec<ResumeSummary>, String> {
    with_db(|conn| {
        let sql = if include_deleted.unwrap_or(false) {
            "SELECT id, title, template_id, favorite, created_at, updated_at, data_json, deleted_at
             FROM resume_documents ORDER BY deleted_at = '', updated_at DESC"
        } else {
            "SELECT id, title, template_id, favorite, created_at, updated_at, data_json, deleted_at
             FROM resume_documents WHERE deleted_at = '' ORDER BY favorite DESC, updated_at DESC"
        };
        let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], row_to_summary)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(rows)
    })
}

#[tauri::command]
pub fn resume_get_document(id: String) -> Result<ResumeDocument, String> {
    with_db(|conn| {
        conn.query_row(
            "SELECT id, title, template_id, data_json, favorite, created_at, updated_at, deleted_at
             FROM resume_documents WHERE id = ?1",
            params![id],
            row_to_document,
        )
        .map_err(|e| format!("读取简历失败: {}", e))
    })
}

#[tauri::command]
pub fn resume_save_document(input: SaveDocumentInput) -> Result<ResumeDocument, String> {
    with_db(|conn| {
        let now = now_iso();
        let title = if input.title.trim().is_empty() {
            "未命名简历".to_string()
        } else {
            input.title.trim().to_string()
        };
        let role = profile_field(&input.data_json, "target");

        if let Some(id) = input.id.as_deref().filter(|s| !s.is_empty()) {
            let favorite = input.favorite.unwrap_or(false);
            conn.execute(
                "UPDATE resume_documents
                 SET title = ?2, template_id = ?3, data_json = ?4, role = ?5, favorite = ?6, updated_at = ?7
                 WHERE id = ?1",
                params![id, title, input.template_id, input.data_json, role, favorite as i64, now],
            )
            .map_err(|e| format!("保存简历失败: {}", e))?;
        } else {
            let id = new_id();
            conn.execute(
                "INSERT INTO resume_documents
                    (id, title, role, template_id, data_json, favorite, created_at, updated_at, deleted_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, '')",
                params![
                    id,
                    title,
                    role,
                    input.template_id,
                    input.data_json,
                    input.favorite.unwrap_or(false) as i64,
                    now
                ],
            )
            .map_err(|e| format!("新建简历失败: {}", e))?;
            return conn
                .query_row(
                    "SELECT id, title, template_id, data_json, favorite, created_at, updated_at, deleted_at
                     FROM resume_documents WHERE id = ?1",
                    params![id],
                    row_to_document,
                )
                .map_err(|e| e.to_string());
        }

        conn.query_row(
            "SELECT id, title, template_id, data_json, favorite, created_at, updated_at, deleted_at
             FROM resume_documents WHERE id = ?1",
            params![input.id.unwrap_or_default()],
            row_to_document,
        )
        .map_err(|e| format!("保存后读取简历失败: {}", e))
    })
}

#[tauri::command]
pub fn resume_set_favorite(id: String, favorite: bool) -> Result<(), String> {
    with_db(|conn| {
        conn.execute(
            "UPDATE resume_documents SET favorite = ?2 WHERE id = ?1",
            params![id, favorite as i64],
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn resume_delete_document(id: String) -> Result<(), String> {
    with_db(|conn| {
        conn.execute(
            "UPDATE resume_documents SET deleted_at = ?2 WHERE id = ?1",
            params![id, now_iso()],
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn resume_restore_document(id: String) -> Result<(), String> {
    with_db(|conn| {
        conn.execute(
            "UPDATE resume_documents SET deleted_at = '' WHERE id = ?1",
            params![id],
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn resume_purge_document(id: String) -> Result<(), String> {
    with_db(|conn| {
        conn.execute("DELETE FROM resume_versions WHERE resume_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM resume_documents WHERE id = ?1", params![id])
            .map(|_| ())
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn resume_duplicate_document(id: String) -> Result<ResumeDocument, String> {
    with_db(|conn| {
        let source = conn
            .query_row(
                "SELECT id, title, template_id, data_json, favorite, created_at, updated_at, deleted_at
                 FROM resume_documents WHERE id = ?1",
                params![id],
                row_to_document,
            )
            .map_err(|e| format!("读取原简历失败: {}", e))?;
        let new_id_value = new_id();
        let now = now_iso();
        let title = format!("{} 副本", source.title);
        conn.execute(
            "INSERT INTO resume_documents
                (id, title, role, template_id, data_json, favorite, created_at, updated_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6, '')",
            params![
                new_id_value,
                title,
                profile_field(&source.data_json, "target"),
                source.template_id,
                source.data_json,
                now
            ],
        )
        .map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT id, title, template_id, data_json, favorite, created_at, updated_at, deleted_at
             FROM resume_documents WHERE id = ?1",
            params![new_id_value],
            row_to_document,
        )
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn resume_list_versions(resume_id: String) -> Result<Vec<ResumeVersion>, String> {
    with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT id, resume_id, kind, label, data_json, pinned, created_at
                 FROM resume_versions WHERE resume_id = ?1
                 ORDER BY pinned DESC, created_at DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![resume_id], row_to_version)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(rows)
    })
}

#[tauri::command]
pub fn resume_save_version(input: SaveVersionInput) -> Result<ResumeVersion, String> {
    with_db(|conn| {
        let id = new_id();
        let now = now_iso();
        let kind = if input.kind == "auto" { "auto" } else { "manual" };
        conn.execute(
            "INSERT INTO resume_versions (id, resume_id, kind, label, data_json, pinned, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
            params![id, input.resume_id, kind, input.label, input.data_json, now],
        )
        .map_err(|e| format!("保存版本失败: {}", e))?;

        if kind == "auto" {
            conn.execute(
                "DELETE FROM resume_versions WHERE resume_id = ?1 AND kind = 'auto' AND pinned = 0
                 AND id NOT IN (
                    SELECT id FROM resume_versions WHERE resume_id = ?1 AND kind = 'auto' AND pinned = 0
                    ORDER BY created_at DESC LIMIT ?2
                 )",
                params![input.resume_id, AUTO_VERSION_LIMIT],
            )
            .map_err(|e| e.to_string())?;
        }

        conn.query_row(
            "SELECT id, resume_id, kind, label, data_json, pinned, created_at
             FROM resume_versions WHERE id = ?1",
            params![id],
            row_to_version,
        )
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn resume_delete_version(id: String) -> Result<(), String> {
    with_db(|conn| {
        conn.execute("DELETE FROM resume_versions WHERE id = ?1 AND pinned = 0", params![id])
            .map(|_| ())
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn resume_set_version_pinned(id: String, pinned: bool) -> Result<(), String> {
    with_db(|conn| {
        conn.execute(
            "UPDATE resume_versions SET pinned = ?2 WHERE id = ?1",
            params![id, pinned as i64],
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn resume_list_templates() -> Result<Vec<ResumeTemplate>, String> {
    with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, html, created_at, updated_at
                 FROM resume_templates ORDER BY updated_at DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], row_to_template)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(rows)
    })
}

#[tauri::command]
pub fn resume_save_template(input: SaveTemplateInput) -> Result<ResumeTemplate, String> {
    with_db(|conn| {
        let now = now_iso();
        let name = if input.name.trim().is_empty() {
            "未命名模板".to_string()
        } else {
            input.name.trim().to_string()
        };
        let description = input.description.unwrap_or_default();

        if let Some(id) = input.id.as_deref().filter(|s| !s.is_empty()) {
            conn.execute(
                "UPDATE resume_templates SET name = ?2, description = ?3, html = ?4, updated_at = ?5
                 WHERE id = ?1",
                params![id, name, description, input.html, now],
            )
            .map_err(|e| format!("保存模板失败: {}", e))?;
            return conn
                .query_row(
                    "SELECT id, name, description, html, created_at, updated_at
                     FROM resume_templates WHERE id = ?1",
                    params![id],
                    row_to_template,
                )
                .map_err(|e| e.to_string());
        }

        let id = new_id();
        conn.execute(
            "INSERT INTO resume_templates (id, name, description, html, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![id, name, description, input.html, now],
        )
        .map_err(|e| format!("新建模板失败: {}", e))?;
        conn.query_row(
            "SELECT id, name, description, html, created_at, updated_at
             FROM resume_templates WHERE id = ?1",
            params![id],
            row_to_template,
        )
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn resume_delete_template(id: String) -> Result<(), String> {
    with_db(|conn| {
        conn.execute("DELETE FROM resume_templates WHERE id = ?1", params![id])
            .map(|_| ())
            .map_err(|e| e.to_string())
    })
}
