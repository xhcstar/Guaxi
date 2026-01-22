use chrono::Datelike;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::{env, fs};
use tauri::Manager;
use tracing::{info, warn};
use uuid::Uuid;

const TODO_DIR_NAME: &str = "guaxi_todo";
const TODO_DB_NAME: &str = "todo.sqlite3";
const SCHEMA_VERSION: i64 = 3;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TodoDbInfo {
    pub db_path: String,
    pub schema_version: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TodoList {
    pub id: String,
    pub title: String,
    pub kind: String, // "system" | "custom" | "group"
    pub group_id: String, // group list id or ""
    pub order_index: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TodoTask {
    pub id: String,
    pub list_id: String,
    pub title: String,
    pub completed: bool,
    pub important: bool,
    pub my_day_key: String,  // "YYYY-MM-DD" or ""
    pub due_date: String,    // "YYYY-MM-DD" or ""
    pub repeat_rule: String, // 文本规则，例如 "每天" "每周" "每月" "工作日"
    pub notes: String,
    pub order_index: i64,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TodoStep {
    pub id: String,
    pub task_id: String,
    pub title: String,
    pub completed: bool,
    pub order_index: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TodoSnapshot {
    pub lists: Vec<TodoList>,
    pub tasks: Vec<TodoTask>,
    pub steps: Vec<TodoStep>,
    pub db_info: TodoDbInfo,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpsertListInput {
    pub id: Option<String>,
    pub title: String,
    pub kind: Option<String>,
    pub group_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpsertTaskInput {
    pub id: Option<String>,
    pub list_id: String,
    pub title: String,
    pub completed: Option<bool>,
    pub important: Option<bool>,
    pub my_day_key: Option<String>,
    pub due_date: Option<String>,
    pub repeat_rule: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpsertStepInput {
    pub id: Option<String>,
    pub task_id: String,
    pub title: String,
    pub completed: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchHit {
    pub task: TodoTask,
    pub matched_in: String, // "title" | "notes" | "step"
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn valid_date_key(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return true;
    }
    if s.len() != 10 {
        return false;
    }
    let bytes = s.as_bytes();
    bytes[4] == b'-' && bytes[7] == b'-'
}

fn resolve_db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let exe_path = env::current_exe().map_err(|e| format!("获取可执行文件路径失败: {}", e))?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "无法获取可执行文件所在目录".to_string())?
        .to_path_buf();

    let repos_dir = exe_dir.join("repos");
    if !repos_dir.exists() {
        fs::create_dir_all(&repos_dir).map_err(|e| format!("创建 repos 目录失败: {}", e))?;
    }

    let dir = repos_dir.join(TODO_DIR_NAME);
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("创建 TODO 数据目录失败: {}", e))?;
    }

    let new_path = dir.join(TODO_DB_NAME);

    if !new_path.exists() {
        if let Ok(base) = app.path().app_data_dir() {
            let old_dir = base.join(TODO_DIR_NAME);
            let old_path = old_dir.join(TODO_DB_NAME);
            if old_path.exists() {
                let _ = fs::copy(&old_path, &new_path);
            }
        }
    }

    Ok(new_path)
}

fn open_conn(db_path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("打开 TODO 数据库失败: {}", e))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| format!("设置 foreign_keys 失败: {}", e))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("设置 WAL 失败: {}", e))?;
    Ok(conn)
}

fn has_column(conn: &Connection, table: &str, col: &str) -> Result<bool, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({})", table))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .map_err(|e| e.to_string())?;
    for r in rows {
        if r.map_err(|e| e.to_string())? == col {
            return Ok(true);
        }
    }
    Ok(false)
}

fn setup_schema(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS todo_meta (
            k TEXT PRIMARY KEY,
            v TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    let ver: Option<String> = conn
        .query_row(
            "SELECT v FROM todo_meta WHERE k='schema_version'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let stored_ver: i64 = ver
        .as_deref()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    if ver.is_none() {
        conn.execute(
            "INSERT INTO todo_meta(k, v) VALUES('schema_version', ?1)",
            params![SCHEMA_VERSION.to_string()],
        )
        .map_err(|e| e.to_string())?;
    }

    conn.execute(
        "CREATE TABLE IF NOT EXISTS todo_lists (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            kind TEXT NOT NULL,
            group_id TEXT NOT NULL DEFAULT '',
            order_index INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    if !has_column(conn, "todo_lists", "group_id")? {
        conn.execute(
            "ALTER TABLE todo_lists ADD COLUMN group_id TEXT NOT NULL DEFAULT ''",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    if stored_ver < SCHEMA_VERSION {
        conn.execute(
            "UPDATE todo_meta SET v=?2 WHERE k='schema_version' AND v=?1",
            params![stored_ver.to_string(), SCHEMA_VERSION.to_string()],
        )
        .map_err(|e| e.to_string())?;
    }

    conn.execute(
        "CREATE TABLE IF NOT EXISTS todo_tasks (
            id TEXT PRIMARY KEY,
            list_id TEXT NOT NULL,
            title TEXT NOT NULL,
            completed INTEGER NOT NULL DEFAULT 0,
            important INTEGER NOT NULL DEFAULT 0,
            my_day_key TEXT NOT NULL DEFAULT '',
            due_date TEXT NOT NULL DEFAULT '',
            reminded_due_date TEXT NOT NULL DEFAULT '',
            repeat_rule TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            order_index INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            completed_at TEXT NOT NULL DEFAULT '',
            FOREIGN KEY(list_id) REFERENCES todo_lists(id) ON DELETE CASCADE
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    if !has_column(conn, "todo_tasks", "reminded_due_date")? {
        conn.execute(
            "ALTER TABLE todo_tasks ADD COLUMN reminded_due_date TEXT NOT NULL DEFAULT ''",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_todo_tasks_list_completed ON todo_tasks(list_id, completed, order_index)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_todo_tasks_important ON todo_tasks(important, completed)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_todo_tasks_myday ON todo_tasks(my_day_key, completed)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_todo_tasks_duedate ON todo_tasks(due_date, completed)",
        [],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS todo_steps (
            id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL,
            title TEXT NOT NULL,
            completed INTEGER NOT NULL DEFAULT 0,
            order_index INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(task_id) REFERENCES todo_tasks(id) ON DELETE CASCADE
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_todo_steps_task ON todo_steps(task_id, completed, order_index)",
        [],
    )
    .map_err(|e| e.to_string())?;

    ensure_system_lists(conn)?;
    Ok(())
}

fn ensure_system_lists(conn: &Connection) -> Result<(), String> {
    let now = now_iso();

    let inbox_exists: Option<String> = conn
        .query_row("SELECT id FROM todo_lists WHERE id='inbox'", [], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?;

    if inbox_exists.is_none() {
        info!("[TODO] 初始化系统列表 inbox");
        conn.execute(
            "INSERT INTO todo_lists(id, title, kind, order_index, created_at, updated_at)
             VALUES('inbox', '任务', 'system', 0, ?1, ?1)",
            params![now],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn row_to_list(row: &rusqlite::Row) -> Result<TodoList, rusqlite::Error> {
    Ok(TodoList {
        id: row.get(0)?,
        title: row.get(1)?,
        kind: row.get(2)?,
        group_id: row.get(3)?,
        order_index: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn row_to_task(row: &rusqlite::Row) -> Result<TodoTask, rusqlite::Error> {
    Ok(TodoTask {
        id: row.get(0)?,
        list_id: row.get(1)?,
        title: row.get(2)?,
        completed: row.get::<_, i64>(3)? != 0,
        important: row.get::<_, i64>(4)? != 0,
        my_day_key: row.get(5)?,
        due_date: row.get(6)?,
        repeat_rule: row.get(7)?,
        notes: row.get(8)?,
        order_index: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        completed_at: row.get(12)?,
    })
}

fn row_to_step(row: &rusqlite::Row) -> Result<TodoStep, rusqlite::Error> {
    Ok(TodoStep {
        id: row.get(0)?,
        task_id: row.get(1)?,
        title: row.get(2)?,
        completed: row.get::<_, i64>(3)? != 0,
        order_index: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn get_db_info(app: &tauri::AppHandle) -> Result<TodoDbInfo, String> {
    let p = resolve_db_path(app)?;
    Ok(TodoDbInfo {
        db_path: p.to_string_lossy().to_string(),
        schema_version: SCHEMA_VERSION,
    })
}

fn with_db<T>(
    app: &tauri::AppHandle,
    f: impl FnOnce(&mut Connection) -> Result<T, String>,
) -> Result<T, String> {
    let db_path = resolve_db_path(app)?;
    let mut conn = open_conn(&db_path)?;
    setup_schema(&conn)?;
    f(&mut conn)
}

#[tauri::command]
pub fn todo_init(app: tauri::AppHandle) -> Result<TodoDbInfo, String> {
    info!("[TODO] todo_init called");
    with_db(&app, |_conn| Ok(get_db_info(&app)?))
}

#[tauri::command]
pub fn todo_get_snapshot(app: tauri::AppHandle) -> Result<TodoSnapshot, String> {
    info!("[TODO] todo_get_snapshot called");
    with_db(&app, |conn| {
        let db_info = get_db_info(&app)?;

        let mut lists_stmt = conn
            .prepare("SELECT id, title, kind, group_id, order_index, created_at, updated_at FROM todo_lists ORDER BY order_index ASC")
            .map_err(|e| e.to_string())?;
        let lists = lists_stmt
            .query_map([], row_to_list)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let mut tasks_stmt = conn
            .prepare(
                "SELECT id, list_id, title, completed, important, my_day_key, due_date, repeat_rule, notes, order_index, created_at, updated_at, completed_at
                 FROM todo_tasks
                 ORDER BY list_id ASC, order_index ASC",
            )
            .map_err(|e| e.to_string())?;
        let tasks = tasks_stmt
            .query_map([], row_to_task)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let mut steps_stmt = conn
            .prepare(
                "SELECT id, task_id, title, completed, order_index, created_at, updated_at
                 FROM todo_steps
                 ORDER BY task_id ASC, order_index ASC",
            )
            .map_err(|e| e.to_string())?;
        let steps = steps_stmt
            .query_map([], row_to_step)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(TodoSnapshot {
            lists,
            tasks,
            steps,
            db_info,
        })
    })
}

#[tauri::command]
pub fn todo_upsert_list(app: tauri::AppHandle, input: UpsertListInput) -> Result<TodoList, String> {
    with_db(&app, |conn| {
        let title = input.title.trim().to_string();
        if title.is_empty() {
            return Err("列表名称不能为空".to_string());
        }
        if title.len() > 80 {
            return Err("列表名称过长（最多 80 字符）".to_string());
        }

        let kind = input.kind.unwrap_or_else(|| "custom".to_string());
        let kind = if kind == "system" {
            "system"
        } else if kind == "group" {
            "group"
        } else {
            "custom"
        }
        .to_string();

        let mut group_id = input.group_id.unwrap_or_default().trim().to_string();
        if kind != "custom" {
            group_id = "".to_string();
        }
        if !group_id.is_empty() {
            let gid_exists: Option<String> = conn
                .query_row(
                    "SELECT id FROM todo_lists WHERE id=?1 AND kind='group'",
                    params![group_id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| e.to_string())?;
            if gid_exists.is_none() {
                group_id = "".to_string();
            }
        }

        let id = input
            .id
            .unwrap_or_else(|| Uuid::new_v4().to_string())
            .trim()
            .to_string();

        if id.is_empty() {
            return Err("列表 id 不能为空".to_string());
        }

        if id == "inbox" && kind != "system" {
            return Err("inbox 为系统列表，禁止修改类型".to_string());
        }

        let now = now_iso();

        let exists: Option<String> = conn
            .query_row("SELECT id FROM todo_lists WHERE id=?1", params![id], |r| r.get(0))
            .optional()
            .map_err(|e| e.to_string())?;

        if exists.is_none() {
            let max_order: i64 = conn
                .query_row("SELECT COALESCE(MAX(order_index), 0) FROM todo_lists", [], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            let order_index = max_order + 1;

            conn.execute(
                "INSERT INTO todo_lists(id, title, kind, group_id, order_index, created_at, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![id, title, kind, group_id, order_index, now],
            )
            .map_err(|e| e.to_string())?;

            info!(
                "[TODO] 新建列表: id={}, kind={}, group_id={}, order_index={}",
                id,
                kind,
                group_id,
                order_index
            );
        } else {
            if id == "inbox" {
                conn.execute(
                    "UPDATE todo_lists SET title=?2, updated_at=?3 WHERE id=?1",
                    params![id, title, now],
                )
                .map_err(|e| e.to_string())?;

                info!("[TODO] 更新系统列表 inbox 标题: {}", title);
            } else {
                let prev: Option<(String, String)> = conn
                    .query_row(
                        "SELECT kind, group_id FROM todo_lists WHERE id=?1",
                        params![id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()
                    .map_err(|e| e.to_string())?;

                conn.execute(
                    "UPDATE todo_lists SET title=?2, kind=?3, group_id=?4, updated_at=?5 WHERE id=?1",
                    params![id, title, kind, group_id, now],
                )
                .map_err(|e| e.to_string())?;

                if let Some((pk, pg)) = prev {
                    if pk != kind || pg != group_id {
                        info!(
                            "[TODO] 更新列表归属: id={}, kind {} -> {}, group_id {} -> {}",
                            id,
                            pk,
                            kind,
                            pg,
                            group_id
                        );
                    } else {
                        info!("[TODO] 更新列表: id={}, kind={}, group_id={}", id, kind, group_id);
                    }
                } else {
                    info!("[TODO] 更新列表: id={}, kind={}, group_id={}", id, kind, group_id);
                }
            }
        }

        conn.query_row(
            "SELECT id, title, kind, group_id, order_index, created_at, updated_at FROM todo_lists WHERE id=?1",
            params![id],
            row_to_list,
        )
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn todo_delete_list(app: tauri::AppHandle, list_id: String) -> Result<(), String> {
    with_db(&app, |conn| {
        let id = list_id.trim();
        if id.is_empty() {
            return Err("list_id 不能为空".to_string());
        }
        if id == "inbox" {
            return Err("系统列表 inbox 不允许删除".to_string());
        }

        let kind: Option<String> = conn
            .query_row(
                "SELECT kind FROM todo_lists WHERE id=?1",
                params![id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        if kind.as_deref() == Some("group") {
            conn.execute(
                "UPDATE todo_lists SET group_id='', updated_at=?2 WHERE group_id=?1",
                params![id, now_iso()],
            )
            .map_err(|e| e.to_string())?;
        }

        let affected = conn
            .execute("DELETE FROM todo_lists WHERE id=?1", params![id])
            .map_err(|e| e.to_string())?;
        if affected == 0 {
            warn!("[TODO] 删除列表未命中: {}", id);
        } else {
            info!("[TODO] 删除列表成功: {}", id);
        }
        Ok(())
    })
}

#[tauri::command]
pub fn todo_reorder_lists(app: tauri::AppHandle, ordered_ids: Vec<String>) -> Result<(), String> {
    with_db(&app, |conn| {
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        let mut idx = 0i64;
        tx.execute(
            "UPDATE todo_lists SET order_index=0 WHERE id='inbox'",
            [],
        )
        .map_err(|e| e.to_string())?;

        for id in ordered_ids {
            let id = id.trim().to_string();
            if id.is_empty() || id == "inbox" {
                continue;
            }
            idx += 1;
            tx.execute(
                "UPDATE todo_lists SET order_index=?2, updated_at=?3 WHERE id=?1",
                params![id, idx, now_iso()],
            )
            .map_err(|e| e.to_string())?;
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    })
}

fn next_due_date(due: &str, rule: &str) -> Option<String> {
    let due = due.trim();
    let rule = rule.trim();
    if due.is_empty() || rule.is_empty() {
        return None;
    }
    let parsed = chrono::NaiveDate::parse_from_str(due, "%Y-%m-%d").ok()?;
    let next = match rule {
        "每天" => parsed.succ_opt()?,
        "每周" => parsed.checked_add_days(chrono::Days::new(7))?,
        "每月" => {
            let y = parsed.year();
            let m = parsed.month();
            let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            chrono::NaiveDate::from_ymd_opt(ny, nm, parsed.day())
                .or_else(|| chrono::NaiveDate::from_ymd_opt(ny, nm, 1)?.with_day0(27))
                .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(ny, nm, 1).unwrap())
        }
        "工作日" => {
            let mut d = parsed;
            for _ in 0..10 {
                d = d.succ_opt()?;
                let wd = d.weekday();
                if wd != chrono::Weekday::Sat && wd != chrono::Weekday::Sun {
                    return Some(d.format("%Y-%m-%d").to_string());
                }
            }
            return None;
        }
        _ => return None,
    };
    Some(next.format("%Y-%m-%d").to_string())
}

#[tauri::command]
pub fn todo_upsert_task(app: tauri::AppHandle, input: UpsertTaskInput) -> Result<TodoTask, String> {
    with_db(&app, |conn| {
        let title = input.title.trim().to_string();
        if title.is_empty() {
            return Err("任务标题不能为空".to_string());
        }
        if title.len() > 200 {
            return Err("任务标题过长（最多 200 字符）".to_string());
        }

        let list_id = input.list_id.trim().to_string();
        if list_id.is_empty() {
            return Err("list_id 不能为空".to_string());
        }

        let list_kind: Option<String> = conn
            .query_row(
                "SELECT kind FROM todo_lists WHERE id=?1",
                params![list_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        let list_id = if matches!(list_kind.as_deref(), Some("system") | Some("custom")) {
            list_id
        } else {
            "inbox".to_string()
        };

        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string()).trim().to_string();
        if id.is_empty() {
            return Err("任务 id 不能为空".to_string());
        }

        let my_day_key = input.my_day_key.unwrap_or_default();
        if !valid_date_key(&my_day_key) {
            return Err("my_day_key 格式错误（应为 YYYY-MM-DD 或空）".to_string());
        }

        let due_date = input.due_date.unwrap_or_default();
        if !valid_date_key(&due_date) {
            return Err("due_date 格式错误（应为 YYYY-MM-DD 或空）".to_string());
        }

        let now = now_iso();

        let exists: Option<String> = conn
            .query_row("SELECT id FROM todo_tasks WHERE id=?1", params![id], |r| r.get(0))
            .optional()
            .map_err(|e| e.to_string())?;

        if exists.is_none() {
            let max_order: i64 = conn
                .query_row(
                    "SELECT COALESCE(MAX(order_index), 0) FROM todo_tasks WHERE list_id=?1",
                    params![list_id],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            let order_index = max_order + 1;

            conn.execute(
                "INSERT INTO todo_tasks(
                    id, list_id, title, completed, important, my_day_key, due_date, repeat_rule, notes,
                    order_index, created_at, updated_at, completed_at
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, '')",
                params![
                    id,
                    list_id,
                    title,
                    input.completed.unwrap_or(false) as i64,
                    input.important.unwrap_or(false) as i64,
                    my_day_key,
                    due_date,
                    input.repeat_rule.unwrap_or_default(),
                    input.notes.unwrap_or_default(),
                    order_index,
                    now
                ],
            )
            .map_err(|e| e.to_string())?;
        } else {
            conn.execute(
                "UPDATE todo_tasks SET
                    list_id=?2,
                    title=?3,
                    completed=COALESCE(?4, completed),
                    important=COALESCE(?5, important),
                    my_day_key=COALESCE(?6, my_day_key),
                    reminded_due_date=CASE WHEN COALESCE(?7, due_date) != due_date THEN '' ELSE reminded_due_date END,
                    due_date=COALESCE(?7, due_date),
                    repeat_rule=COALESCE(?8, repeat_rule),
                    notes=COALESCE(?9, notes),
                    updated_at=?10
                 WHERE id=?1",
                params![
                    id,
                    list_id,
                    title,
                    input.completed.map(|b| b as i64),
                    input.important.map(|b| b as i64),
                    Some(my_day_key),
                    Some(due_date),
                    input.repeat_rule,
                    input.notes,
                    now
                ],
            )
            .map_err(|e| e.to_string())?;
        }

        conn.query_row(
            "SELECT id, list_id, title, completed, important, my_day_key, due_date, repeat_rule, notes, order_index, created_at, updated_at, completed_at
             FROM todo_tasks WHERE id=?1",
            params![id],
            row_to_task,
        )
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn todo_delete_task(app: tauri::AppHandle, task_id: String) -> Result<(), String> {
    with_db(&app, |conn| {
        let id = task_id.trim();
        if id.is_empty() {
            return Err("task_id 不能为空".to_string());
        }
        let affected = conn
            .execute("DELETE FROM todo_tasks WHERE id=?1", params![id])
            .map_err(|e| e.to_string())?;
        if affected == 0 {
            warn!("[TODO] 删除任务未命中: {}", id);
        } else {
            info!("[TODO] 删除任务成功: {}", id);
        }
        Ok(())
    })
}

#[tauri::command]
pub fn todo_set_task_completed(app: tauri::AppHandle, task_id: String, completed: bool) -> Result<TodoTask, String> {
    with_db(&app, |conn| {
        let id = task_id.trim().to_string();
        if id.is_empty() {
            return Err("task_id 不能为空".to_string());
        }

        let mut task: TodoTask = conn
            .query_row(
                "SELECT id, list_id, title, completed, important, my_day_key, due_date, repeat_rule, notes, order_index, created_at, updated_at, completed_at
                 FROM todo_tasks WHERE id=?1",
                params![id],
                row_to_task,
            )
            .map_err(|e| e.to_string())?;

        let now = now_iso();

        if completed && !task.repeat_rule.trim().is_empty() && !task.due_date.trim().is_empty() {
            if let Some(next_due) = next_due_date(&task.due_date, &task.repeat_rule) {
                info!("[TODO] 重复任务完成：推进截止日期 {} -> {}", task.due_date, next_due);
                conn.execute(
                    "UPDATE todo_tasks SET completed=0, completed_at=?2, due_date=?3, reminded_due_date='', updated_at=?2 WHERE id=?1",
                    params![id, now, next_due],
                )
                .map_err(|e| e.to_string())?;
            } else {
                conn.execute(
                    "UPDATE todo_tasks SET completed=1, completed_at=?2, updated_at=?2 WHERE id=?1",
                    params![id, now],
                )
                .map_err(|e| e.to_string())?;
            }
        } else {
            let completed_at = if completed { now.clone() } else { "".to_string() };
            conn.execute(
                "UPDATE todo_tasks SET completed=?2, completed_at=?3, updated_at=?4 WHERE id=?1",
                params![id, completed as i64, completed_at, now],
            )
            .map_err(|e| e.to_string())?;
        }

        task = conn
            .query_row(
                "SELECT id, list_id, title, completed, important, my_day_key, due_date, repeat_rule, notes, order_index, created_at, updated_at, completed_at
                 FROM todo_tasks WHERE id=?1",
                params![id],
                row_to_task,
            )
            .map_err(|e| e.to_string())?;

        Ok(task)
    })
}

#[tauri::command]
pub fn todo_toggle_task_important(app: tauri::AppHandle, task_id: String) -> Result<TodoTask, String> {
    with_db(&app, |conn| {
        let id = task_id.trim().to_string();
        if id.is_empty() {
            return Err("task_id 不能为空".to_string());
        }
        conn.execute(
            "UPDATE todo_tasks SET important=CASE WHEN important=1 THEN 0 ELSE 1 END, updated_at=?2 WHERE id=?1",
            params![id, now_iso()],
        )
        .map_err(|e| e.to_string())?;

        conn.query_row(
            "SELECT id, list_id, title, completed, important, my_day_key, due_date, repeat_rule, notes, order_index, created_at, updated_at, completed_at
             FROM todo_tasks WHERE id=?1",
            params![id],
            row_to_task,
        )
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn todo_set_task_myday(app: tauri::AppHandle, task_id: String, my_day_key: String) -> Result<TodoTask, String> {
    with_db(&app, |conn| {
        let id = task_id.trim().to_string();
        if id.is_empty() {
            return Err("task_id 不能为空".to_string());
        }
        if !valid_date_key(&my_day_key) {
            return Err("my_day_key 格式错误（应为 YYYY-MM-DD 或空）".to_string());
        }

        conn.execute(
            "UPDATE todo_tasks SET my_day_key=?2, updated_at=?3 WHERE id=?1",
            params![id, my_day_key, now_iso()],
        )
        .map_err(|e| e.to_string())?;

        conn.query_row(
            "SELECT id, list_id, title, completed, important, my_day_key, due_date, repeat_rule, notes, order_index, created_at, updated_at, completed_at
             FROM todo_tasks WHERE id=?1",
            params![id],
            row_to_task,
        )
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn todo_set_task_due_date(app: tauri::AppHandle, task_id: String, due_date: String) -> Result<TodoTask, String> {
    with_db(&app, |conn| {
        let id = task_id.trim().to_string();
        if id.is_empty() {
            return Err("task_id 不能为空".to_string());
        }
        if !valid_date_key(&due_date) {
            return Err("due_date 格式错误（应为 YYYY-MM-DD 或空）".to_string());
        }

        conn.execute(
            "UPDATE todo_tasks SET due_date=?2, reminded_due_date=CASE WHEN due_date!=?2 THEN '' ELSE reminded_due_date END, updated_at=?3 WHERE id=?1",
            params![id, due_date, now_iso()],
        )
        .map_err(|e| e.to_string())?;

        conn.query_row(
            "SELECT id, list_id, title, completed, important, my_day_key, due_date, repeat_rule, notes, order_index, created_at, updated_at, completed_at
             FROM todo_tasks WHERE id=?1",
            params![id],
            row_to_task,
        )
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn todo_set_task_repeat_rule(app: tauri::AppHandle, task_id: String, repeat_rule: String) -> Result<TodoTask, String> {
    with_db(&app, |conn| {
        let id = task_id.trim().to_string();
        if id.is_empty() {
            return Err("task_id 不能为空".to_string());
        }
        if repeat_rule.len() > 40 {
            return Err("repeat_rule 过长（最多 40 字符）".to_string());
        }

        conn.execute(
            "UPDATE todo_tasks SET repeat_rule=?2, updated_at=?3 WHERE id=?1",
            params![id, repeat_rule, now_iso()],
        )
        .map_err(|e| e.to_string())?;

        conn.query_row(
            "SELECT id, list_id, title, completed, important, my_day_key, due_date, repeat_rule, notes, order_index, created_at, updated_at, completed_at
             FROM todo_tasks WHERE id=?1",
            params![id],
            row_to_task,
        )
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn todo_set_task_notes(app: tauri::AppHandle, task_id: String, notes: String) -> Result<TodoTask, String> {
    with_db(&app, |conn| {
        let id = task_id.trim().to_string();
        if id.is_empty() {
            return Err("task_id 不能为空".to_string());
        }
        if notes.len() > 20000 {
            return Err("备注过长（最多 20000 字符）".to_string());
        }

        conn.execute(
            "UPDATE todo_tasks SET notes=?2, updated_at=?3 WHERE id=?1",
            params![id, notes, now_iso()],
        )
        .map_err(|e| e.to_string())?;

        conn.query_row(
            "SELECT id, list_id, title, completed, important, my_day_key, due_date, repeat_rule, notes, order_index, created_at, updated_at, completed_at
             FROM todo_tasks WHERE id=?1",
            params![id],
            row_to_task,
        )
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn todo_upsert_step(app: tauri::AppHandle, input: UpsertStepInput) -> Result<TodoStep, String> {
    with_db(&app, |conn| {
        let title = input.title.trim().to_string();
        if title.is_empty() {
            return Err("步骤标题不能为空".to_string());
        }
        if title.len() > 200 {
            return Err("步骤标题过长（最多 200 字符）".to_string());
        }

        let task_id = input.task_id.trim().to_string();
        if task_id.is_empty() {
            return Err("task_id 不能为空".to_string());
        }

        let task_exists: Option<String> = conn
            .query_row("SELECT id FROM todo_tasks WHERE id=?1", params![task_id], |r| r.get(0))
            .optional()
            .map_err(|e| e.to_string())?;
        if task_exists.is_none() {
            return Err("任务不存在，无法新增步骤".to_string());
        }

        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string()).trim().to_string();
        if id.is_empty() {
            return Err("步骤 id 不能为空".to_string());
        }

        let now = now_iso();

        let exists: Option<String> = conn
            .query_row("SELECT id FROM todo_steps WHERE id=?1", params![id], |r| r.get(0))
            .optional()
            .map_err(|e| e.to_string())?;

        if exists.is_none() {
            let max_order: i64 = conn
                .query_row(
                    "SELECT COALESCE(MAX(order_index), 0) FROM todo_steps WHERE task_id=?1",
                    params![task_id],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            let order_index = max_order + 1;

            conn.execute(
                "INSERT INTO todo_steps(id, task_id, title, completed, order_index, created_at, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![
                    id,
                    task_id,
                    title,
                    input.completed.unwrap_or(false) as i64,
                    order_index,
                    now
                ],
            )
            .map_err(|e| e.to_string())?;
        } else {
            conn.execute(
                "UPDATE todo_steps SET title=?2, completed=COALESCE(?3, completed), updated_at=?4 WHERE id=?1",
                params![id, title, input.completed.map(|b| b as i64), now],
            )
            .map_err(|e| e.to_string())?;
        }

        conn.query_row(
            "SELECT id, task_id, title, completed, order_index, created_at, updated_at FROM todo_steps WHERE id=?1",
            params![id],
            row_to_step,
        )
        .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn todo_delete_step(app: tauri::AppHandle, step_id: String) -> Result<(), String> {
    with_db(&app, |conn| {
        let id = step_id.trim();
        if id.is_empty() {
            return Err("step_id 不能为空".to_string());
        }
        let affected = conn
            .execute("DELETE FROM todo_steps WHERE id=?1", params![id])
            .map_err(|e| e.to_string())?;
        if affected == 0 {
            warn!("[TODO] 删除步骤未命中: {}", id);
        }
        Ok(())
    })
}

#[tauri::command]
pub fn todo_reorder_steps(app: tauri::AppHandle, task_id: String, ordered_step_ids: Vec<String>) -> Result<(), String> {
    with_db(&app, |conn| {
        let task_id = task_id.trim().to_string();
        if task_id.is_empty() {
            return Err("task_id 不能为空".to_string());
        }

        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let mut idx = 0i64;

        for sid in ordered_step_ids {
            let sid = sid.trim().to_string();
            if sid.is_empty() {
                continue;
            }
            idx += 1;
            tx.execute(
                "UPDATE todo_steps SET order_index=?3, updated_at=?4 WHERE id=?1 AND task_id=?2",
                params![sid, task_id, idx, now_iso()],
            )
            .map_err(|e| e.to_string())?;
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[tauri::command]
pub fn todo_search_tasks(app: tauri::AppHandle, query: String, limit: Option<i64>) -> Result<Vec<SearchHit>, String> {
    with_db(&app, |conn| {
        let q = query.trim().to_string();
        if q.is_empty() {
            return Ok(vec![]);
        }
        let lim = limit.unwrap_or(200).clamp(1, 1000);

        let like = format!("%{}%", q.replace('%', "\\%").replace('_', "\\_"));

        let mut hits: Vec<SearchHit> = Vec::new();

        let mut stmt = conn
            .prepare(
                "SELECT id, list_id, title, completed, important, my_day_key, due_date, repeat_rule, notes, order_index, created_at, updated_at, completed_at
                 FROM todo_tasks
                 WHERE title LIKE ?1 ESCAPE '\\' OR notes LIKE ?1 ESCAPE '\\'
                 ORDER BY updated_at DESC
                 LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![like, lim], row_to_task)
            .map_err(|e| e.to_string())?;

        for r in rows {
            let t = r.map_err(|e| e.to_string())?;
            let matched_in = if t.title.contains(&q) { "title" } else { "notes" }.to_string();
            hits.push(SearchHit { task: t, matched_in });
        }

        if (hits.len() as i64) < lim {
            let mut stmt2 = conn
                .prepare(
                    "SELECT t.id, t.list_id, t.title, t.completed, t.important, t.my_day_key, t.due_date, t.repeat_rule, t.notes, t.order_index, t.created_at, t.updated_at, t.completed_at
                     FROM todo_tasks t
                     WHERE t.id IN (
                        SELECT s.task_id FROM todo_steps s WHERE s.title LIKE ?1 ESCAPE '\\'
                     )
                     ORDER BY t.updated_at DESC
                     LIMIT ?2",
                )
                .map_err(|e| e.to_string())?;

            let rows2 = stmt2
                .query_map(params![like, lim], row_to_task)
                .map_err(|e| e.to_string())?;
            for r in rows2 {
                let t = r.map_err(|e| e.to_string())?;
                if hits.iter().any(|h| h.task.id == t.id) {
                    continue;
                }
                hits.push(SearchHit {
                    task: t,
                    matched_in: "step".to_string(),
                });
                if (hits.len() as i64) >= lim {
                    break;
                }
            }
        }

        Ok(hits)
    })
}

#[tauri::command]
pub fn todo_poll_due_reminders(app: tauri::AppHandle, limit: Option<i64>) -> Result<Vec<TodoTask>, String> {
    info!("[TODO] todo_poll_due_reminders called");
    with_db(&app, |conn| {
        let lim = limit.unwrap_or(50).clamp(1, 200);
        let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();

        let tasks: Vec<TodoTask> = {
            let mut stmt = conn
                .prepare(
                    "SELECT id, list_id, title, completed, important, my_day_key, due_date, repeat_rule, notes, order_index, created_at, updated_at, completed_at
                     FROM todo_tasks
                     WHERE completed=0
                       AND due_date!=''
                       AND due_date <= ?1
                       AND reminded_due_date != due_date
                     ORDER BY due_date ASC, updated_at DESC
                     LIMIT ?2",
                )
                .map_err(|e| e.to_string())?;

            let rows = stmt
                .query_map(params![today, lim], row_to_task)
                .map_err(|e| e.to_string())?;

            let mut tasks: Vec<TodoTask> = Vec::new();
            for r in rows {
                tasks.push(r.map_err(|e| e.to_string())?);
            }
            tasks
        };

        if tasks.is_empty() {
            return Ok(vec![]);
        }

        let tx = conn.transaction().map_err(|e| e.to_string())?;
        for t in &tasks {
            tx.execute(
                "UPDATE todo_tasks SET reminded_due_date=?2 WHERE id=?1",
                params![t.id, t.due_date],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;

        info!("[TODO] due reminders polled: {}", tasks.len());
        Ok(tasks)
    })
}