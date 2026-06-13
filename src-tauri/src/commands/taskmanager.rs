//! 任务管理后端：.org 文件为真相源，SQLite 仅作索引/缓存（看板/日历/报表/画像查询）。
//!
//! 复用 commands/todo.rs 的 SQLite 模式（WAL、即用即关连接、Result<T,String>）。
//! 目录结构：<task_root>/{work,life,study}/*.org，task_root 默认 exe 同级 `task/`，可配置覆盖。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::{env, fs};

use chrono::{Datelike, Local, NaiveDate};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tracing::{info, warn};

use crate::commands::task_block::{self, TaskBlock};

const TASK_DIR_NAME: &str = "guaxi_task";
const TASK_DB_NAME: &str = "task.sqlite3";
const SCHEMA_VERSION: i64 = 4;
const CATEGORIES: &[&str] = &["work", "life", "study"];

// ───────── 配置（task 根路径）─────────

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TaskConfig {
    /// task 根目录绝对路径；为空表示用默认（exe 同级 task/）。
    #[serde(default)]
    pub root: String,
    /// 任务库 git 远程仓库地址（origin）。
    #[serde(default)]
    pub remote_url: String,
    #[serde(default)]
    pub git_username: String,
    #[serde(default)]
    pub git_email: String,
}

fn exe_dir() -> Result<PathBuf, String> {
    let p = env::current_exe().map_err(|e| format!("获取可执行文件路径失败: {}", e))?;
    p.parent()
        .map(|d| d.to_path_buf())
        .ok_or_else(|| "无法获取可执行文件目录".to_string())
}

fn task_config_path() -> Result<PathBuf, String> {
    let dir = exe_dir()?.join("config");
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("创建 config 目录失败: {}", e))?;
    }
    Ok(dir.join("task_config.json"))
}

fn default_task_root() -> Result<PathBuf, String> {
    Ok(exe_dir()?.join("task"))
}

fn load_task_config() -> TaskConfig {
    if let Ok(p) = task_config_path() {
        if let Ok(text) = fs::read_to_string(&p) {
            if let Ok(cfg) = serde_json::from_str::<TaskConfig>(&text) {
                return cfg;
            }
        }
    }
    TaskConfig::default()
}

fn save_task_config(cfg: &TaskConfig) -> Result<(), String> {
    let p = task_config_path()?;
    fs::write(&p, serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?)
        .map_err(|e| format!("写入任务配置失败: {}", e))
}

/// 解析出有效的 task 根目录（配置优先，否则默认），并确保 work/life/study 存在。
fn resolve_task_root() -> Result<PathBuf, String> {
    let cfg = load_task_config();
    let root = if cfg.root.trim().is_empty() {
        default_task_root()?
    } else {
        PathBuf::from(cfg.root.trim())
    };
    if !root.exists() {
        fs::create_dir_all(&root).map_err(|e| format!("创建 task 根目录失败: {}", e))?;
    }
    for c in CATEGORIES {
        let sub = root.join(c);
        if !sub.exists() {
            fs::create_dir_all(&sub).map_err(|e| format!("创建 {} 子目录失败: {}", c, e))?;
        }
    }
    Ok(root)
}

// ───────── SQLite ─────────

fn resolve_db_path() -> Result<PathBuf, String> {
    let repos = exe_dir()?.join("repos");
    if !repos.exists() {
        fs::create_dir_all(&repos).map_err(|e| format!("创建 repos 目录失败: {}", e))?;
    }
    let dir = repos.join(TASK_DIR_NAME);
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("创建任务数据目录失败: {}", e))?;
    }
    Ok(dir.join(TASK_DB_NAME))
}

fn open_conn(db_path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("打开任务数据库失败: {}", e))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("设置 WAL 失败: {}", e))?;
    Ok(conn)
}

fn setup_schema(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS task_meta (k TEXT PRIMARY KEY, v TEXT NOT NULL)",
        [],
    )
    .map_err(|e| e.to_string())?;
    let ver: Option<String> = conn
        .query_row("SELECT v FROM task_meta WHERE k='schema_version'", [], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    // schema 变更（org→block DSL）：旧版本直接重建索引表（索引可由 .org 重新扫描得到）。
    let needs_reset = match ver.as_deref() {
        Some(v) => v != SCHEMA_VERSION.to_string(),
        None => false,
    };
    if needs_reset {
        conn.execute("DROP TABLE IF EXISTS task_items", [])
            .map_err(|e| e.to_string())?;
    }
    if ver.as_deref() != Some(&SCHEMA_VERSION.to_string()) {
        conn.execute(
            "INSERT INTO task_meta(k,v) VALUES('schema_version', ?1)
             ON CONFLICT(k) DO UPDATE SET v=excluded.v",
            params![SCHEMA_VERSION.to_string()],
        )
        .map_err(|e| e.to_string())?;
    }

    // task_items：索引列用于筛选/排序，block_json 存完整 TaskBlock 供读取。
    conn.execute(
        "CREATE TABLE IF NOT EXISTS task_items (
            internal_id TEXT PRIMARY KEY,
            id TEXT NOT NULL DEFAULT '',
            parent_id TEXT NOT NULL DEFAULT '',
            category TEXT NOT NULL DEFAULT '',
            task_type TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT '',
            done INTEGER NOT NULL DEFAULT 0,
            priority TEXT NOT NULL DEFAULT '',
            severity TEXT NOT NULL DEFAULT '',
            origin TEXT NOT NULL DEFAULT '',
            deadline TEXT NOT NULL DEFAULT '',
            create_time TEXT NOT NULL DEFAULT '',
            completed_time TEXT NOT NULL DEFAULT '',
            progress TEXT NOT NULL DEFAULT '',
            fix_status TEXT NOT NULL DEFAULT '',
            description TEXT NOT NULL DEFAULT '',
            file_path TEXT NOT NULL DEFAULT '',
            block_start_line INTEGER NOT NULL DEFAULT 0,
            block_end_line INTEGER NOT NULL DEFAULT 0,
            block_json TEXT NOT NULL DEFAULT '{}'
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    for (idx, col) in [
        ("idx_task_status", "status"),
        ("idx_task_category", "category"),
        ("idx_task_type", "task_type"),
        ("idx_task_deadline", "deadline"),
        ("idx_task_file", "file_path"),
    ] {
        conn.execute(
            &format!("CREATE INDEX IF NOT EXISTS {} ON task_items({})", idx, col),
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn init() -> Result<Connection, String> {
    let db = resolve_db_path()?;
    let conn = open_conn(&db)?;
    setup_schema(&conn)?;
    Ok(conn)
}

fn row_to_block(row: &rusqlite::Row) -> rusqlite::Result<TaskBlock> {
    let json: String = row.get("block_json")?;
    Ok(serde_json::from_str(&json).unwrap_or_default())
}

fn upsert_block(conn: &Connection, b: &TaskBlock) -> Result<(), String> {
    let json = serde_json::to_string(b).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO task_items
            (internal_id,id,parent_id,category,task_type,status,done,priority,severity,origin,
             deadline,create_time,completed_time,progress,fix_status,description,file_path,block_start_line,block_end_line,block_json)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)
         ON CONFLICT(internal_id) DO UPDATE SET
            id=excluded.id, parent_id=excluded.parent_id, category=excluded.category, task_type=excluded.task_type,
            status=excluded.status, done=excluded.done, priority=excluded.priority, severity=excluded.severity,
            origin=excluded.origin, deadline=excluded.deadline, create_time=excluded.create_time,
            completed_time=excluded.completed_time,
            progress=excluded.progress, fix_status=excluded.fix_status, description=excluded.description,
            file_path=excluded.file_path, block_start_line=excluded.block_start_line,
            block_end_line=excluded.block_end_line, block_json=excluded.block_json",
        params![
            b.internal_id, b.id, b.parent_id, b.manage_type, b.task_type, b.status, b.done as i64,
            b.priority, b.severity, b.origin, b.deadline, b.create_time, b.completed_time, b.progress, b.fix_status,
            b.description, b.file_path, b.block_start_line as i64, b.block_end_line as i64, json
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 扫描 task 根下三类目录的 .org，解析任务块并重建索引（先清空对应文件旧行）。
/// 解析前为缺失 internal_id / other-id 的块就地补全并回写源文件。
fn scan_and_index(conn: &Connection) -> Result<usize, String> {
    let root = resolve_task_root()?;
    let mut total = 0usize;
    for cat in CATEGORIES {
        let dir = root.join(cat);
        if !dir.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()) != Some("org".to_string()) {
                continue;
            }
            let file_path = path.to_string_lossy().to_string();
            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            // 补全缺失 id 并回写（最小行编辑，保留格式）
            let (content, changed) = task_block::backfill_ids(&content, cat);
            if changed {
                let _ = fs::write(path, &content);
            }
            // 清空该文件旧行
            conn.execute("DELETE FROM task_items WHERE file_path=?1", params![file_path])
                .map_err(|e| e.to_string())?;
            let blocks = task_block::parse_blocks(&content, &file_path, cat);
            for b in &blocks {
                if b.internal_id.trim().is_empty() {
                    continue; // 理论上已补全
                }
                upsert_block(conn, b)?;
            }
            total += blocks.len();
        }
    }
    info!(total, "[TaskManager] scanned and indexed guaxi task blocks");
    Ok(total)
}

// ───────── Tauri 命令 ─────────

#[tauri::command]
pub fn task_get_config() -> Result<TaskConfig, String> {
    let mut cfg = load_task_config();
    if cfg.root.trim().is_empty() {
        cfg.root = default_task_root()?.to_string_lossy().to_string();
    }
    Ok(cfg)
}

#[tauri::command]
pub fn task_set_config(root: String) -> Result<TaskConfig, String> {
    let mut cfg = load_task_config();
    cfg.root = root.trim().to_string();
    save_task_config(&cfg)?;
    // 确保目录就绪
    let _ = resolve_task_root()?;
    Ok(cfg)
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TaskGitConfig {
    pub remote_url: String,
    pub git_username: String,
    pub git_email: String,
}

/// 读取任务库 git 远程仓库配置。
#[tauri::command]
pub fn task_get_git_config() -> Result<TaskGitConfig, String> {
    let cfg = load_task_config();
    Ok(TaskGitConfig {
        remote_url: cfg.remote_url,
        git_username: cfg.git_username,
        git_email: cfg.git_email,
    })
}

/// 保存任务库 git 配置并应用到任务库根目录（init 仓库 + 设置 user / origin）。
#[tauri::command]
pub fn task_set_git_config(
    remote_url: String,
    git_username: String,
    git_email: String,
) -> Result<(), String> {
    let mut cfg = load_task_config();
    cfg.remote_url = remote_url.trim().to_string();
    cfg.git_username = git_username.trim().to_string();
    cfg.git_email = git_email.trim().to_string();
    save_task_config(&cfg)?;

    let root = resolve_task_root()?;
    ensure_task_git_repo(&root, &cfg)
}

/// 确保任务库根目录是一个配置好 user/origin 的 git 仓库。
fn ensure_task_git_repo(root: &Path, cfg: &TaskConfig) -> Result<(), String> {
    use git2::{Repository, Signature};

    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let repo = if root.join(".git").exists() {
        Repository::open(root).map_err(|e| e.to_string())?
    } else {
        Repository::init(root).map_err(|e| e.to_string())?
    };

    let git_username = if cfg.git_username.trim().is_empty() {
        "Guaxi".to_string()
    } else {
        cfg.git_username.trim().to_string()
    };
    let git_email = if cfg.git_email.trim().is_empty() {
        "guaxi@localhost".to_string()
    } else {
        cfg.git_email.trim().to_string()
    };

    {
        let mut repo_cfg = repo.config().map_err(|e| e.to_string())?;
        repo_cfg
            .set_str("user.name", git_username.as_str())
            .map_err(|e| e.to_string())?;
        repo_cfg
            .set_str("user.email", git_email.as_str())
            .map_err(|e| e.to_string())?;
    }

    // origin 远程
    let url = cfg.remote_url.trim();
    if !url.is_empty() {
        match repo.find_remote("origin") {
            Ok(_) => {
                repo.remote_set_url("origin", url).map_err(|e| e.to_string())?;
            }
            Err(_) => {
                repo.remote("origin", url).map_err(|e| e.to_string())?;
            }
        }
    }

    // 首次提交（保证有 HEAD，便于后续 push/pull）
    if repo.head().is_err() {
        let readme_rel = Path::new("README.md");
        let readme_abs = root.join(readme_rel);
        if !readme_abs.exists() {
            fs::write(&readme_abs, "# task\n").map_err(|e| e.to_string())?;
        }
        let mut index = repo.index().map_err(|e| e.to_string())?;
        index.add_path(readme_rel).map_err(|e| e.to_string())?;
        index.write().map_err(|e| e.to_string())?;
        let tree_id = index.write_tree().map_err(|e| e.to_string())?;
        let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;
        let sig = Signature::now(git_username.as_str(), git_email.as_str())
            .map_err(|e| e.to_string())?;
        repo.commit(Some("refs/heads/master"), &sig, &sig, "first commit", &tree, &[])
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// 返回 task 根与三类子目录的绝对路径（供前端文件树/编辑模式使用）。
#[tauri::command]
pub fn task_ensure_dirs() -> Result<serde_json::Value, String> {
    let root = resolve_task_root()?;
    let mut subs = serde_json::Map::new();
    for c in CATEGORIES {
        subs.insert(c.to_string(), serde_json::Value::String(root.join(c).to_string_lossy().to_string()));
    }
    Ok(serde_json::json!({
        "root": root.to_string_lossy().to_string(),
        "categories": subs,
    }))
}

#[tauri::command]
pub fn task_scan_and_index() -> Result<usize, String> {
    let conn = init()?;
    scan_and_index(&conn)
}

#[derive(Debug, Deserialize, Default)]
pub struct TaskQuery {
    pub category: Option<String>,      // work/life/study，空=全部
    pub status: Option<String>,        // 指定状态
    pub done: Option<bool>,            // true=仅完成 false=仅未完成
    pub date_from: Option<String>,     // YYYY-MM-DD（按 scheduled/deadline/closed 任一落在区间）
    pub date_to: Option<String>,
    pub keyword: Option<String>,       // 标题/正文模糊
}

#[tauri::command]
pub fn task_query(query: TaskQuery) -> Result<Vec<TaskBlock>, String> {
    let conn = init()?;
    let mut sql = String::from("SELECT * FROM task_items WHERE 1=1");
    let mut args: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(c) = query.category.as_ref().filter(|s| !s.is_empty()) {
        sql.push_str(" AND category=?");
        args.push(Box::new(c.clone()));
    }
    if let Some(s) = query.status.as_ref().filter(|s| !s.is_empty()) {
        sql.push_str(" AND status=?");
        args.push(Box::new(s.clone()));
    }
    if let Some(d) = query.done {
        sql.push_str(" AND done=?");
        args.push(Box::new(if d { 1i64 } else { 0i64 }));
    }
    if let Some(k) = query.keyword.as_ref().filter(|s| !s.is_empty()) {
        sql.push_str(" AND (description LIKE ? OR id LIKE ?)");
        let like = format!("%{}%", k);
        args.push(Box::new(like.clone()));
        args.push(Box::new(like));
    }
    if let (Some(f), Some(t)) = (
        query.date_from.as_ref().filter(|s| !s.is_empty()),
        query.date_to.as_ref().filter(|s| !s.is_empty()),
    ) {
        sql.push_str(" AND (deadline<>'' AND deadline BETWEEN ? AND ?)");
        args.push(Box::new(f.clone()));
        args.push(Box::new(t.clone()));
    }
    sql.push_str(" ORDER BY (deadline<>'') DESC, deadline ASC, priority ASC, block_start_line ASC");

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let refs: Vec<&dyn rusqlite::types::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let rows = stmt
        .query_map(refs.as_slice(), row_to_block)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// 看板/预览编辑：按 internal_id 定位块，按字段级更新后回写源文件，再重解析该文件更新 DB。
/// `fields` 为要更新的字段（键用 TaskBlock 字段名，如 status/description/deadline/content...）。
#[derive(Debug, Deserialize)]
pub struct TaskUpdateInput {
    pub id: String, // = internal_id
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub title: Option<String>, // 兼容看板：映射到 description
    #[serde(default)]
    pub deadline: Option<String>,
    #[serde(default)]
    pub notes: Option<String>, // 兼容看板编辑抽屉：映射到 content
    /// 通用字段补丁（优先级高于上面的兼容字段）。键=TaskBlock 字段名。
    #[serde(default)]
    pub fields: Option<std::collections::BTreeMap<String, serde_json::Value>>,
}

#[tauri::command]
pub fn task_update_item(input: TaskUpdateInput) -> Result<Vec<TaskBlock>, String> {
    let conn = init()?;
    let (file_path, category): (String, String) = conn
        .query_row(
            "SELECT file_path, category FROM task_items WHERE internal_id=?1",
            params![input.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| format!("找不到任务: {}", e))?;

    let content = fs::read_to_string(&file_path).map_err(|e| format!("读取 .org 失败: {}", e))?;
    let mut blocks = task_block::parse_blocks(&content, &file_path, &category);
    let target_idx = blocks
        .iter()
        .position(|b| b.internal_id == input.id)
        .ok_or_else(|| "块不存在".to_string())?;

    // 应用补丁
    apply_block_fields(&mut blocks[target_idx], &input);

    let new_block_text = task_block::serialize_block(&blocks[target_idx]);
    let new_content = task_block::replace_block_in_content(
        &content,
        blocks[target_idx].block_start_line,
        blocks[target_idx].block_end_line,
        &new_block_text,
    );
    fs::write(&file_path, &new_content).map_err(|e| format!("写回 .org 失败: {}", e))?;

    // 重解析该文件更新 DB
    conn.execute("DELETE FROM task_items WHERE file_path=?1", params![file_path])
        .map_err(|e| e.to_string())?;
    let fresh = task_block::parse_blocks(&new_content, &file_path, &category);
    for b in &fresh {
        if !b.internal_id.trim().is_empty() {
            upsert_block(&conn, b)?;
        }
    }
    Ok(fresh)
}

fn apply_block_fields(b: &mut TaskBlock, input: &TaskUpdateInput) {
    if let Some(s) = &input.status {
        b.status = s.trim().to_uppercase();
        b.done = b.status == "DONE" || b.status == "CANCELLED";
    }
    if let Some(t) = &input.title {
        b.description = t.clone();
    }
    if let Some(d) = &input.deadline {
        b.deadline = d.clone();
    }
    if let Some(n) = &input.notes {
        b.content = n.clone();
    }
    if let Some(map) = &input.fields {
        for (k, v) in map {
            let s = v.as_str().map(|x| x.to_string()).unwrap_or_else(|| v.to_string());
            match k.as_str() {
                "id" => b.id = s,
                "parent_id" => b.parent_id = s,
                "task_type" => b.task_type = s.to_lowercase(),
                "deadline" => b.deadline = s,
                "create_time" => b.create_time = s,
                "origin" => b.origin = s,
                "severity" => b.severity = s,
                "priority" => b.priority = s.trim_start_matches("[#").trim_end_matches(']').to_uppercase(),
                "status" => {
                    b.status = s.trim().to_uppercase();
                    b.done = b.status == "DONE" || b.status == "CANCELLED";
                }
                "progress" => b.progress = s,
                "fix_status" => b.fix_status = s,
                "description" => b.description = s,
                "root_cause" => b.root_cause = s,
                "condition_of_occurrence" => b.condition_of_occurrence = s,
                "fixed_details" => b.fixed_details = s,
                "testing_suggestions" => b.testing_suggestions = s,
                "affected_release" => b.affected_release = s,
                "review_link" => b.review_link = s,
                "content" => b.content = s,
                "extra_impacts" => {
                    if let Some(arr) = v.as_array() {
                        b.extra_impacts = arr
                            .iter()
                            .map(|x| x.as_str().map(|y| y.to_string()).unwrap_or_default())
                            .filter(|x| !x.is_empty())
                            .collect();
                    }
                }
                _ => {}
            }
        }
    }
    // 完成时间同步：变为完成态且原本无完成时间 → 写入当前时间；取消完成 → 清空。
    if b.done {
        if b.completed_time.trim().is_empty() {
            b.completed_time = now_cn_datetime();
        }
    } else {
        b.completed_time = String::new();
    }
}

use task_block::now_cn_datetime;

/// 导入：把外部 .org/.json 数据规整后追加到指定 category 的目标 .org 文件，然后重扫描。
#[derive(Debug, Deserialize)]
pub struct TaskImportInput {
    pub category: String,             // work/life/study
    pub target_file_name: String,     // 目标 .org 文件名（无则用 inbox.org）
    pub source_kind: String,          // "org" | "json"
    pub content: String,              // 源内容
}

#[tauri::command]
pub fn task_import(input: TaskImportInput) -> Result<usize, String> {
    let root = resolve_task_root()?;
    let cat = if CATEGORIES.contains(&input.category.as_str()) {
        input.category.clone()
    } else {
        "work".to_string()
    };
    let file_name = if input.target_file_name.trim().is_empty() {
        "inbox.org".to_string()
    } else if input.target_file_name.ends_with(".org") {
        input.target_file_name.clone()
    } else {
        format!("{}.org", input.target_file_name)
    };
    let target = root.join(&cat).join(&file_name);

    let import_text = match input.source_kind.as_str() {
        "json" => {
            let blocks: Vec<TaskBlock> =
                serde_json::from_str(&input.content).map_err(|e| format!("解析 JSON 失败: {}", e))?;
            blocks_to_text(&blocks)
        }
        _ => input.content.clone(),
    };

    let mut existing = fs::read_to_string(&target).unwrap_or_default();
    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    existing.push_str(&import_text);
    if !existing.ends_with('\n') {
        existing.push('\n');
    }
    fs::write(&target, &existing).map_err(|e| format!("写入导入目标失败: {}", e))?;

    let conn = init()?;
    scan_and_index(&conn)
}

/// TaskBlock[] → DSL 文本（用于 JSON 导入）。
fn blocks_to_text(blocks: &[TaskBlock]) -> String {
    let mut out = String::new();
    for b in blocks {
        out.push_str(&task_block::serialize_block(b));
        out.push('\n');
    }
    out
}

/// 导出报表：按时间范围聚合，渲染 markdown 或 json。
#[derive(Debug, Deserialize)]
pub struct ReportInput {
    pub kind: String,        // daily/weekly/halfyear/yearly/custom
    pub date: String,        // 锚点日期 YYYY-MM-DD（custom 用 from/to）
    pub from: Option<String>,
    pub to: Option<String>,
    pub format: String,      // "md" | "json"
}

fn period_range(kind: &str, anchor: &str, from: &Option<String>, to: &Option<String>) -> Result<(String, String, String), String> {
    let parse = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| e.to_string());
    let today = Local::now().date_naive();
    let a = if anchor.is_empty() { today } else { parse(anchor)? };
    let (f, t, label) = match kind {
        "daily" => (a, a, format!("日报 {}", a)),
        "weekly" => {
            let wd = a.weekday().num_days_from_monday() as i64;
            let start = a - chrono::Duration::days(wd);
            let end = start + chrono::Duration::days(6);
            (start, end, format!("周报 {} ~ {}", start, end))
        }
        "halfyear" => {
            let (start, end) = if a.month() <= 6 {
                (NaiveDate::from_ymd_opt(a.year(), 1, 1).unwrap(), NaiveDate::from_ymd_opt(a.year(), 6, 30).unwrap())
            } else {
                (NaiveDate::from_ymd_opt(a.year(), 7, 1).unwrap(), NaiveDate::from_ymd_opt(a.year(), 12, 31).unwrap())
            };
            (start, end, format!("半年报 {} ~ {}", start, end))
        }
        "yearly" => {
            let start = NaiveDate::from_ymd_opt(a.year(), 1, 1).unwrap();
            let end = NaiveDate::from_ymd_opt(a.year(), 12, 31).unwrap();
            (start, end, format!("年报 {}", a.year()))
        }
        _ => {
            let start = parse(from.as_deref().unwrap_or(""))?;
            let end = parse(to.as_deref().unwrap_or(""))?;
            (start, end, format!("报表 {} ~ {}", start, end))
        }
    };
    Ok((f.to_string(), t.to_string(), label))
}

#[tauri::command]
pub fn task_export_report(input: ReportInput) -> Result<String, String> {
    // 日报/周报/月报/半年报/年报 都用统一的报表模板（按 [start,end] 周期）
    const PERIOD_KINDS: &[&str] = &["daily", "weekly", "monthly", "halfyear", "yearly"];
    if PERIOD_KINDS.contains(&input.kind.as_str()) {
        return build_period_report(&input.kind, &input.from, &input.to);
    }
    let (from, to, label) = period_range(&input.kind, &input.date, &input.from, &input.to)?;
    let items = task_query(TaskQuery {
        date_from: Some(from.clone()),
        date_to: Some(to.clone()),
        ..Default::default()
    })?;

    if input.format == "json" {
        return serde_json::to_string_pretty(&items).map_err(|e| e.to_string());
    }

    // markdown（通用汇总；周报专用模板见阶段4）
    let mut md = format!("# {}\n\n", label);
    let done: Vec<&TaskBlock> = items.iter().filter(|i| i.done).collect();
    let doing: Vec<&TaskBlock> = items.iter().filter(|i| i.status == "DOING" || i.status == "NEXT").collect();
    let todo: Vec<&TaskBlock> = items.iter().filter(|i| !i.done && i.status != "DOING" && i.status != "NEXT").collect();
    let total = items.len();

    md.push_str(&format!(
        "- 任务总数：{}，已完成：{}，进行中：{}，待办：{}\n\n",
        total, done.len(), doing.len(), todo.len()
    ));

    let section = |title: &str, list: &[&TaskBlock], out: &mut String| {
        if list.is_empty() {
            return;
        }
        out.push_str(&format!("## {}\n\n", title));
        let mut by_cat: BTreeMap<String, Vec<&TaskBlock>> = BTreeMap::new();
        for it in list {
            by_cat.entry(category_label(&it.manage_type)).or_default().push(it);
        }
        for (cat, its) in by_cat {
            out.push_str(&format!("### {}\n\n", cat));
            for it in its {
                let date = if !it.deadline.is_empty() {
                    format!(" (截止 {})", it.deadline)
                } else {
                    String::new()
                };
                let title = if it.description.is_empty() { it.id.clone() } else { it.description.clone() };
                out.push_str(&format!("- {}{}\n", title, date));
            }
            out.push('\n');
        }
    };
    section("已完成", &done, &mut md);
    section("进行中", &doing, &mut md);
    section("待办", &todo, &mut md);

    Ok(md)
}

fn format_cn_date(d: NaiveDate) -> String {
    format!("{}年{:02}月{:02}日", d.year(), d.month(), d.day())
}

/// 解析日期：支持 中文(2026年06月13日[ 时:分:秒]) 与 ISO/斜杠(2026-06-13 / 2026/6/13)。
fn parse_cn_date(s: &str) -> Option<NaiveDate> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    // 提取前三组数字作为 年/月/日
    let mut nums: Vec<i32> = Vec::new();
    let mut cur = String::new();
    for ch in t.chars() {
        if ch.is_ascii_digit() {
            cur.push(ch);
        } else if !cur.is_empty() {
            if let Ok(n) = cur.parse() {
                nums.push(n);
            }
            cur.clear();
            if nums.len() >= 3 {
                break;
            }
        }
    }
    if !cur.is_empty() && nums.len() < 3 {
        if let Ok(n) = cur.parse() {
            nums.push(n);
        }
    }
    if nums.len() >= 3 {
        NaiveDate::from_ymd_opt(nums[0], nums[1] as u32, nums[2] as u32)
    } else {
        None
    }
}

/// 统一报表模板（日报/周报/月报/半年报/年报）。
/// 完成项 = completed_time 落在 [from, to] 的 bug/task；剩余 = to 之前仍未完成的 bug/task。
/// from/to 为空时按 kind 取「今天起回溯 一天/一周/一月/半年/一年」。
fn build_period_report(kind: &str, from: &Option<String>, to: &Option<String>) -> Result<String, String> {
    let today = Local::now().date_naive();
    let end = to
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(today);
    let span_days = match kind {
        "daily" => 1,
        "weekly" => 7,
        "monthly" => 30,
        "halfyear" => 182,
        "yearly" => 365,
        _ => 7,
    };
    let start = from
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(end - chrono::Duration::days(span_days));

    // 完成窗口 [start, end]；无法解析的完成时间视为落在窗口内，避免丢失。
    let completed_in_window = |b: &TaskBlock| -> bool {
        match parse_cn_date(&b.completed_time) {
            Some(d) => d >= start && d <= end,
            None => true,
        }
    };

    let blocks = task_query(TaskQuery::default())?;
    // bug/任务（非「其他」均按 bug/任务对待，含 #TYPE 为空或未识别的块）
    let is_bug_or_task = |b: &TaskBlock| b.task_type != "other";
    let done: Vec<&TaskBlock> = blocks
        .iter()
        .filter(|b| b.status == "DONE" && is_bug_or_task(b) && completed_in_window(b))
        .collect();
    let others: Vec<&TaskBlock> = blocks
        .iter()
        .filter(|b| b.task_type == "other" && (b.status != "DONE" || completed_in_window(b)))
        .collect();
    // 剩余工作：end 之前状态不是「已完成(DONE)」的 bug/任务（任何其它状态，含空值/取消/进行中等都算）
    let remaining: Vec<&TaskBlock> = blocks
        .iter()
        .filter(|b| b.status != "DONE" && is_bug_or_task(b))
        .collect();

    let mut out = String::new();
    out.push_str("---------------------------------------------------------------------------------------\n");
    out.push_str(&format!("* Date: {}\n\n", format_cn_date(end)));

    out.push_str("Bug修复和需求开发:\n");
    for b in &done {
        out.push_str(&format!("{}\n", b.description));
        let fix = b.fix_status.trim();
        let rc = b.root_cause.trim();
        if !fix.is_empty() || !rc.is_empty() {
            out.push_str(&format!("{} {}\n", fix, rc));
        }
    }
    out.push('\n');

    out.push_str("其他工作:\n");
    for b in &others {
        out.push_str(&format!("{}\n", b.description));
    }
    out.push('\n');

    out.push_str("review:\n");
    out.push_str("Q1 - ship次数：0, comment次数：0\n");
    out.push_str("Q2 - ship次数：0, comment次数：0\n");
    out.push_str("Q3 - ship次数：0, comment次数：0\n");
    out.push_str("Q4 - ship次数：0, comment次数：0\n");
    out.push_str("本周 - ship次数：0 ，comment次数：0\n\n");

    out.push_str("剩余工作：\n");
    for b in &remaining {
        out.push_str(&format!("{}\n", b.description));
    }

    Ok(out)
}

fn category_label(cat: &str) -> String {
    match cat {
        "work" => "工作".to_string(),
        "life" => "生活".to_string(),
        "study" => "学习".to_string(),
        other => other.to_string(),
    }
}

/// 读 .org 文件内容（供面板内编辑）。
#[tauri::command]
pub fn task_read_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| format!("读取失败: {}", e))
}

/// 写 .org 文件内容并重扫描索引。
#[tauri::command]
pub fn task_write_file(path: String, content: String) -> Result<usize, String> {
    fs::write(&path, &content).map_err(|e| format!("写入失败: {}", e))?;
    let conn = init()?;
    scan_and_index(&conn)
}

/// 写报表文件到任意路径（不触发扫描索引）。
#[tauri::command]
pub fn task_write_report_file(path: String, content: String) -> Result<(), String> {
    fs::write(&path, &content).map_err(|e| format!("写入报表失败: {}", e))?;
    Ok(())
}

/// 校验任务块语法，返回诊断（错误/警告）。供编辑器/预览实时提示。
#[tauri::command]
pub fn task_validate_content(content: String, category: Option<String>) -> Result<Vec<task_block::Diagnostic>, String> {
    Ok(task_block::validate_content(&content, &category.unwrap_or_default()))
}

// ───────── 文件监听（.org 变更 → 重索引 → 通知前端）─────────

// 保持 watcher 存活；记录已监听的根目录，避免重复监听。
static TASK_WATCHER: OnceLock<Mutex<Option<(PathBuf, RecommendedWatcher)>>> = OnceLock::new();

/// 监听 task 根目录的 .org 变更。变更后增量重扫描并向前端发 `guaxi:task-files-changed`。
/// 幂等：对同一根目录重复调用不会重复监听。
#[tauri::command]
pub fn task_start_watching(app: tauri::AppHandle) -> Result<(), String> {
    let root = resolve_task_root()?;
    let holder = TASK_WATCHER.get_or_init(|| Mutex::new(None));
    let mut guard = holder.lock().map_err(|e| e.to_string())?;
    if let Some((existing, _)) = guard.as_ref() {
        if *existing == root {
            return Ok(()); // 已在监听同一目录
        }
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })
    .map_err(|e| format!("创建任务监听器失败: {}", e))?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| format!("监听任务目录失败: {}", e))?;

    // 后台线程：去抖 + 重索引 + 通知
    let app_for_thread = app.clone();
    std::thread::spawn(move || {
        loop {
            // 阻塞等待第一条事件
            let first = match rx.recv() {
                Ok(ev) => ev,
                Err(_) => break, // 发送端释放，watcher 已停
            };
            let mut relevant = event_touches_org(&first);
            // 去抖：在 400ms 静默窗口内继续收集
            loop {
                match rx.recv_timeout(std::time::Duration::from_millis(400)) {
                    Ok(ev) => {
                        if event_touches_org(&ev) {
                            relevant = true;
                        }
                    }
                    Err(_) => break,
                }
            }
            if !relevant {
                continue;
            }
            match init().and_then(|conn| scan_and_index(&conn)) {
                Ok(n) => {
                    info!(count = n, "[TaskManager] reindexed after fs change");
                    let _ = app_for_thread.emit("guaxi:task-files-changed", n);
                }
                Err(e) => warn!("[TaskManager] reindex after fs change failed: {}", e),
            }
        }
    });

    *guard = Some((root, watcher));
    Ok(())
}

fn event_touches_org(res: &notify::Result<notify::Event>) -> bool {
    match res {
        Ok(ev) => ev.paths.iter().any(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("org"))
                .unwrap_or(false)
        }),
        Err(_) => false,
    }
}
