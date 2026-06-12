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

use crate::commands::task_org::{self, OrgPatch, TaskItem};

const TASK_DIR_NAME: &str = "guaxi_task";
const TASK_DB_NAME: &str = "task.sqlite3";
const SCHEMA_VERSION: i64 = 1;
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
    if ver.is_none() {
        conn.execute(
            "INSERT INTO task_meta(k,v) VALUES('schema_version', ?1)",
            params![SCHEMA_VERSION.to_string()],
        )
        .map_err(|e| e.to_string())?;
    }

    conn.execute(
        "CREATE TABLE IF NOT EXISTS task_items (
            id TEXT PRIMARY KEY,
            file_path TEXT NOT NULL,
            line INTEGER NOT NULL,
            level INTEGER NOT NULL,
            title TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT '',
            done INTEGER NOT NULL DEFAULT 0,
            priority TEXT NOT NULL DEFAULT '',
            category TEXT NOT NULL DEFAULT '',
            tags TEXT NOT NULL DEFAULT '',
            scheduled TEXT NOT NULL DEFAULT '',
            deadline TEXT NOT NULL DEFAULT '',
            closed TEXT NOT NULL DEFAULT '',
            clock_minutes INTEGER NOT NULL DEFAULT 0,
            properties TEXT NOT NULL DEFAULT '{}',
            parent_id TEXT NOT NULL DEFAULT '',
            raw_body TEXT NOT NULL DEFAULT ''
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    for (idx, col) in [
        ("idx_task_status", "status"),
        ("idx_task_category", "category"),
        ("idx_task_deadline", "deadline"),
        ("idx_task_scheduled", "scheduled"),
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

fn row_to_item(row: &rusqlite::Row) -> rusqlite::Result<TaskItem> {
    let tags_str: String = row.get("tags")?;
    let props_str: String = row.get("properties")?;
    Ok(TaskItem {
        id: row.get("id")?,
        file_path: row.get("file_path")?,
        line: row.get::<_, i64>("line")? as usize,
        level: row.get::<_, i64>("level")? as usize,
        title: row.get("title")?,
        status: row.get("status")?,
        done: row.get::<_, i64>("done")? != 0,
        priority: row.get("priority")?,
        category: row.get("category")?,
        tags: if tags_str.is_empty() {
            vec![]
        } else {
            tags_str.split('\u{1f}').map(|s| s.to_string()).collect()
        },
        scheduled: row.get("scheduled")?,
        deadline: row.get("deadline")?,
        closed: row.get("closed")?,
        clock_minutes: row.get("clock_minutes")?,
        properties: serde_json::from_str(&props_str).unwrap_or_default(),
        parent_id: row.get("parent_id")?,
        raw_body: row.get("raw_body")?,
    })
}

fn upsert_item(conn: &Connection, it: &TaskItem) -> Result<(), String> {
    let tags = it.tags.join("\u{1f}");
    let props = serde_json::to_string(&it.properties).unwrap_or_else(|_| "{}".to_string());
    conn.execute(
        "INSERT INTO task_items
            (id,file_path,line,level,title,status,done,priority,category,tags,scheduled,deadline,closed,clock_minutes,properties,parent_id,raw_body)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
         ON CONFLICT(id) DO UPDATE SET
            file_path=excluded.file_path, line=excluded.line, level=excluded.level, title=excluded.title,
            status=excluded.status, done=excluded.done, priority=excluded.priority, category=excluded.category,
            tags=excluded.tags, scheduled=excluded.scheduled, deadline=excluded.deadline, closed=excluded.closed,
            clock_minutes=excluded.clock_minutes, properties=excluded.properties, parent_id=excluded.parent_id, raw_body=excluded.raw_body",
        params![
            it.id, it.file_path, it.line as i64, it.level as i64, it.title, it.status, it.done as i64,
            it.priority, it.category, tags, it.scheduled, it.deadline, it.closed, it.clock_minutes, props,
            it.parent_id, it.raw_body
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 扫描 task 根下三类目录的 .org，解析并重建索引（先清空对应文件的旧行）。
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
            // 清空该文件旧行
            conn.execute("DELETE FROM task_items WHERE file_path=?1", params![file_path])
                .map_err(|e| e.to_string())?;
            let items = task_org::parse_org(&content, &file_path, cat);
            for it in &items {
                upsert_item(conn, it)?;
            }
            total += items.len();
        }
    }
    info!(total, "[TaskManager] scanned and indexed org tasks");
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
pub fn task_query(query: TaskQuery) -> Result<Vec<TaskItem>, String> {
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
        sql.push_str(" AND (title LIKE ? OR raw_body LIKE ?)");
        let like = format!("%{}%", k);
        args.push(Box::new(like.clone()));
        args.push(Box::new(like));
    }
    if let (Some(f), Some(t)) = (
        query.date_from.as_ref().filter(|s| !s.is_empty()),
        query.date_to.as_ref().filter(|s| !s.is_empty()),
    ) {
        sql.push_str(
            " AND ((scheduled<>'' AND scheduled BETWEEN ? AND ?) \
               OR (deadline<>'' AND deadline BETWEEN ? AND ?) \
               OR (closed<>'' AND closed BETWEEN ? AND ?))",
        );
        for _ in 0..3 {
            args.push(Box::new(f.clone()));
            args.push(Box::new(t.clone()));
        }
    }
    sql.push_str(" ORDER BY (deadline<>'') DESC, deadline ASC, priority ASC, line ASC");

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let refs: Vec<&dyn rusqlite::types::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let rows = stmt
        .query_map(refs.as_slice(), row_to_item)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
pub struct TaskUpdateInput {
    pub id: String,
    pub status: Option<String>,
    pub title: Option<String>,
    pub scheduled: Option<String>,
    pub deadline: Option<String>,
    pub notes: Option<String>,
}

/// 看板编辑：按 id 定位到 .org 文件+行，做字段级最小回写，再重解析该文件更新 DB。
#[tauri::command]
pub fn task_update_item(input: TaskUpdateInput) -> Result<Vec<TaskItem>, String> {
    let conn = init()?;
    let (file_path, line, category): (String, i64, String) = conn
        .query_row(
            "SELECT file_path, line, category FROM task_items WHERE id=?1",
            params![input.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| format!("找不到任务: {}", e))?;

    let content = fs::read_to_string(&file_path).map_err(|e| format!("读取 .org 失败: {}", e))?;
    let patch = OrgPatch {
        status: input.status,
        title: input.title,
        scheduled: input.scheduled,
        deadline: input.deadline,
        notes: input.notes,
    };
    let new_content = task_org::apply_patch(&content, line as usize, &patch)?;
    fs::write(&file_path, &new_content).map_err(|e| format!("写回 .org 失败: {}", e))?;

    // 重解析该文件更新 DB
    conn.execute("DELETE FROM task_items WHERE file_path=?1", params![file_path])
        .map_err(|e| e.to_string())?;
    let items = task_org::parse_org(&new_content, &file_path, &category);
    for it in &items {
        upsert_item(&conn, it)?;
    }
    Ok(items)
}

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

    let org_text = match input.source_kind.as_str() {
        "json" => {
            let items: Vec<TaskItem> =
                serde_json::from_str(&input.content).map_err(|e| format!("解析 JSON 失败: {}", e))?;
            items_to_org(&items)
        }
        _ => input.content.clone(),
    };

    let mut existing = fs::read_to_string(&target).unwrap_or_default();
    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    existing.push_str(&org_text);
    if !existing.ends_with('\n') {
        existing.push('\n');
    }
    fs::write(&target, &existing).map_err(|e| format!("写入导入目标失败: {}", e))?;

    let conn = init()?;
    scan_and_index(&conn)
}

/// TaskItem[] → org 文本（用于 JSON 导入与导出）。
fn items_to_org(items: &[TaskItem]) -> String {
    let mut out = String::new();
    for it in items {
        let level = if it.level == 0 { 1 } else { it.level };
        out.push_str(&"*".repeat(level));
        out.push(' ');
        if !it.status.is_empty() {
            out.push_str(&it.status);
            out.push(' ');
        }
        if !it.priority.is_empty() {
            out.push_str(&format!("[#{}] ", it.priority));
        }
        out.push_str(it.title.trim());
        if !it.tags.is_empty() {
            out.push_str(&format!("  :{}:", it.tags.join(":")));
        }
        out.push('\n');
        if !it.scheduled.is_empty() {
            out.push_str(&format!("   SCHEDULED: <{}>\n", it.scheduled));
        }
        if !it.deadline.is_empty() {
            out.push_str(&format!("   DEADLINE: <{}>\n", it.deadline));
        }
        if !it.raw_body.trim().is_empty() {
            out.push_str(it.raw_body.trim());
            out.push('\n');
        }
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
    let (from, to, label) = period_range(&input.kind, &input.date, &input.from, &input.to)?;
    let items = task_query(TaskQuery {
        date_from: Some(from.clone()),
        date_to: Some(to.clone()),
        ..Default::default()
    })?;

    if input.format == "json" {
        return serde_json::to_string_pretty(&items).map_err(|e| e.to_string());
    }

    // markdown
    let mut md = format!("# {}\n\n", label);
    let done: Vec<&TaskItem> = items.iter().filter(|i| i.done).collect();
    let doing: Vec<&TaskItem> = items.iter().filter(|i| i.status == "DOING" || i.status == "NEXT").collect();
    let todo: Vec<&TaskItem> = items.iter().filter(|i| !i.done && i.status != "DOING" && i.status != "NEXT").collect();
    let total = items.len();
    let total_minutes: i64 = items.iter().map(|i| i.clock_minutes).sum();

    md.push_str(&format!(
        "- 任务总数：{}，已完成：{}，进行中：{}，待办：{}\n- 累计工时：{} 小时 {} 分钟\n\n",
        total, done.len(), doing.len(), todo.len(), total_minutes / 60, total_minutes % 60
    ));

    let section = |title: &str, list: &[&TaskItem], out: &mut String| {
        if list.is_empty() {
            return;
        }
        out.push_str(&format!("## {}\n\n", title));
        let mut by_cat: BTreeMap<String, Vec<&TaskItem>> = BTreeMap::new();
        for it in list {
            by_cat.entry(category_label(&it.category)).or_default().push(it);
        }
        for (cat, its) in by_cat {
            out.push_str(&format!("### {}\n\n", cat));
            for it in its {
                let date = if !it.deadline.is_empty() {
                    format!(" (截止 {})", it.deadline)
                } else if !it.scheduled.is_empty() {
                    format!(" (计划 {})", it.scheduled)
                } else {
                    String::new()
                };
                out.push_str(&format!("- {}{}\n", it.title, date));
            }
            out.push('\n');
        }
    };
    section("已完成", &done, &mut md);
    section("进行中", &doing, &mut md);
    section("待办", &todo, &mut md);

    Ok(md)
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
