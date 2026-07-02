//! `conversion_tasks` 表的读写封装。
//!
//! 复用 `repoview::init_db()`（每次打开独立连接，与全库其它表共用 repos.db）。
//! 所有时间戳统一使用 UTC ISO8601（由 SQLite `strftime` 或 chrono 生成），
//! 以便前端与其它设备一致解析。

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::commands::repoview::init_db;

/// 任务状态机：
/// pending -> probing -> running -> (succeeded | failed)
/// 任意非终态可被 cancel 置为 canceled。failed/canceled 可 retry 回到 pending。
pub const STATUS_PENDING: &str = "pending";
pub const STATUS_PROBING: &str = "probing";
pub const STATUS_RUNNING: &str = "running";
pub const STATUS_SUCCEEDED: &str = "succeeded";
pub const STATUS_FAILED: &str = "failed";
pub const STATUS_CANCELED: &str = "canceled";

/// 未完成状态：启动时据此恢复（断点续跑）。
pub const ACTIVE_STATUSES: &[&str] = &[STATUS_PENDING, STATUS_PROBING, STATUS_RUNNING];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionTask {
    pub id: i64,
    pub uuid: String,
    #[serde(rename = "importedFileId", skip_serializing_if = "Option::is_none")]
    pub imported_file_id: Option<i64>,
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(rename = "inputPath")]
    pub input_path: String,
    #[serde(rename = "outputDir")]
    pub output_dir: String,
    #[serde(rename = "expectedMdPath", skip_serializing_if = "Option::is_none")]
    pub expected_md_path: Option<String>,
    #[serde(rename = "requestedEngine")]
    pub requested_engine: String,
    #[serde(rename = "resolvedEngine", skip_serializing_if = "Option::is_none")]
    pub resolved_engine: Option<String>,
    #[serde(rename = "probeJson", skip_serializing_if = "Option::is_none")]
    pub probe_json: Option<String>,
    pub status: String,
    #[serde(rename = "progressCurrent")]
    pub progress_current: i64,
    #[serde(rename = "progressTotal")]
    pub progress_total: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub attempts: i64,
    #[serde(rename = "maxAttempts")]
    pub max_attempts: i64,
    pub priority: i64,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "startedAt", skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(rename = "finishedAt", skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
}

/// 新建任务的入参。
#[derive(Debug, Clone)]
pub struct NewConversionTask {
    pub uuid: String,
    pub imported_file_id: Option<i64>,
    pub file_name: String,
    pub input_path: String,
    pub output_dir: String,
    pub expected_md_path: Option<String>,
    pub requested_engine: String,
    pub max_attempts: i64,
    pub priority: i64,
}

const SELECT_COLS: &str = "id, uuid, imported_file_id, file_name, input_path, output_dir, \
    expected_md_path, requested_engine, resolved_engine, probe_json, status, \
    progress_current, progress_total, stage, message, error, attempts, max_attempts, \
    priority, created_at, updated_at, started_at, finished_at";

fn row_to_task(row: &Row) -> rusqlite::Result<ConversionTask> {
    Ok(ConversionTask {
        id: row.get(0)?,
        uuid: row.get(1)?,
        imported_file_id: row.get(2)?,
        file_name: row.get(3)?,
        input_path: row.get(4)?,
        output_dir: row.get(5)?,
        expected_md_path: row.get(6)?,
        requested_engine: row.get(7)?,
        resolved_engine: row.get(8)?,
        probe_json: row.get(9)?,
        status: row.get(10)?,
        progress_current: row.get(11)?,
        progress_total: row.get(12)?,
        stage: row.get(13)?,
        message: row.get(14)?,
        error: row.get(15)?,
        attempts: row.get(16)?,
        max_attempts: row.get(17)?,
        priority: row.get(18)?,
        created_at: row.get(19)?,
        updated_at: row.get(20)?,
        started_at: row.get(21)?,
        finished_at: row.get(22)?,
    })
}

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// 插入一条新任务，返回其 id。
pub fn insert_task(t: &NewConversionTask) -> Result<i64, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let now = now_iso();
    conn.execute(
        "INSERT INTO conversion_tasks \
         (uuid, imported_file_id, file_name, input_path, output_dir, expected_md_path, \
          requested_engine, status, progress_current, progress_total, attempts, max_attempts, \
          priority, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', 0, 100, 0, ?8, ?9, ?10, ?10)",
        params![
            t.uuid,
            t.imported_file_id,
            t.file_name,
            t.input_path,
            t.output_dir,
            t.expected_md_path,
            t.requested_engine,
            t.max_attempts,
            t.priority,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// 若已存在针对同一 input_path 且处于活动态的任务，返回它（避免重复入队）。
pub fn find_active_by_input(input_path: &str) -> Result<Option<ConversionTask>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let placeholders = ACTIVE_STATUSES
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT {} FROM conversion_tasks WHERE input_path = ?1 AND status IN ({}) \
         ORDER BY id DESC LIMIT 1",
        SELECT_COLS, placeholders
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    // 参数：input_path + 各状态
    let mut params_vec: Vec<&dyn rusqlite::ToSql> = vec![&input_path];
    for s in ACTIVE_STATUSES {
        params_vec.push(s);
    }
    let res = stmt
        .query_row(params_vec.as_slice(), row_to_task)
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(res)
}

pub fn get_task(id: i64) -> Result<Option<ConversionTask>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let sql = format!("SELECT {} FROM conversion_tasks WHERE id = ?1", SELECT_COLS);
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    stmt.query_row(params![id], row_to_task)
        .optional()
        .map_err(|e| e.to_string())
}

/// 列出任务，可选按状态过滤；按 priority DESC, id ASC 排序（配合 worker 取任务顺序）。
pub fn list_tasks(status_filter: Option<&str>, limit: i64) -> Result<Vec<ConversionTask>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let (sql, has_filter) = match status_filter {
        Some(_) => (
            format!(
                "SELECT {} FROM conversion_tasks WHERE status = ?1 \
                 ORDER BY priority DESC, id ASC LIMIT ?2",
                SELECT_COLS
            ),
            true,
        ),
        None => (
            format!(
                "SELECT {} FROM conversion_tasks ORDER BY priority DESC, id ASC LIMIT ?1",
                SELECT_COLS
            ),
            false,
        ),
    };
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = if has_filter {
        stmt.query_map(params![status_filter.unwrap(), limit], row_to_task)
    } else {
        stmt.query_map(params![limit], row_to_task)
    }
    .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// 取下一个可执行任务（pending 中优先级最高、最早入队的），并原子置为 probing。
/// 返回被认领的任务；无可执行任务时返回 None。
pub fn claim_next_pending() -> Result<Option<ConversionTask>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let sql = format!(
        "SELECT {} FROM conversion_tasks WHERE status = 'pending' \
         ORDER BY priority DESC, id ASC LIMIT 1",
        SELECT_COLS
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let task = stmt
        .query_row([], row_to_task)
        .optional()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    if let Some(mut t) = task {
        let now = now_iso();
        let changed = conn
            .execute(
                "UPDATE conversion_tasks SET status = 'probing', started_at = COALESCE(started_at, ?2), \
                 attempts = attempts + 1, updated_at = ?2 WHERE id = ?1 AND status = 'pending'",
                params![t.id, now],
            )
            .map_err(|e| e.to_string())?;
        if changed == 0 {
            // 竞态：被别的 worker 抢走
            return Ok(None);
        }
        t.status = STATUS_PROBING.to_string();
        t.attempts += 1;
        t.updated_at = now;
        Ok(Some(t))
    } else {
        Ok(None)
    }
}

/// 记录探针结果与解析后的引擎。
pub fn set_resolved_engine(id: i64, engine: &str, probe_json: Option<&str>) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE conversion_tasks SET resolved_engine = ?2, probe_json = ?3, status = 'running', \
         updated_at = ?4 WHERE id = ?1",
        params![id, engine, probe_json, now_iso()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 更新进度（running 阶段高频调用）。
pub fn update_progress(
    id: i64,
    current: i64,
    total: i64,
    stage: Option<&str>,
    message: Option<&str>,
) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE conversion_tasks SET progress_current = ?2, progress_total = ?3, \
         stage = COALESCE(?4, stage), message = COALESCE(?5, message), updated_at = ?6 \
         WHERE id = ?1",
        params![id, current, total, stage, message, now_iso()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn mark_succeeded(id: i64, md_path: Option<&str>) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let now = now_iso();
    conn.execute(
        "UPDATE conversion_tasks SET status = 'succeeded', progress_current = 100, \
         progress_total = 100, message = '转换完成', error = NULL, \
         expected_md_path = COALESCE(?2, expected_md_path), finished_at = ?3, updated_at = ?3 \
         WHERE id = ?1",
        params![id, md_path, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 标记失败。若还有重试额度（attempts < max_attempts）则回落到 pending 以便自动重试，
/// 返回 true 表示将自动重试。
pub fn mark_failed(id: i64, error: &str) -> Result<bool, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let now = now_iso();
    let (attempts, max_attempts): (i64, i64) = conn
        .query_row(
            "SELECT attempts, max_attempts FROM conversion_tasks WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    if attempts < max_attempts {
        conn.execute(
            "UPDATE conversion_tasks SET status = 'pending', error = ?2, \
             message = '等待重试', updated_at = ?3 WHERE id = ?1",
            params![id, error, now],
        )
        .map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        conn.execute(
            "UPDATE conversion_tasks SET status = 'failed', error = ?2, \
             finished_at = ?3, updated_at = ?3 WHERE id = ?1",
            params![id, error, now],
        )
        .map_err(|e| e.to_string())?;
        Ok(false)
    }
}

/// 取消任务（仅非终态可取消）。返回是否发生变更。
pub fn mark_canceled(id: i64) -> Result<bool, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let now = now_iso();
    let changed = conn
        .execute(
            "UPDATE conversion_tasks SET status = 'canceled', message = '已取消', \
             finished_at = ?2, updated_at = ?2 \
             WHERE id = ?1 AND status IN ('pending','probing','running')",
            params![id, now],
        )
        .map_err(|e| e.to_string())?;
    Ok(changed > 0)
}

/// 重置任务以重试（failed/canceled -> pending，清零已用重试次数上限内计数）。
pub fn reset_for_retry(id: i64) -> Result<bool, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let now = now_iso();
    let changed = conn
        .execute(
            "UPDATE conversion_tasks SET status = 'pending', attempts = 0, error = NULL, \
             progress_current = 0, stage = NULL, message = '等待转换', \
             started_at = NULL, finished_at = NULL, updated_at = ?2 \
             WHERE id = ?1 AND status IN ('failed','canceled')",
            params![id, now],
        )
        .map_err(|e| e.to_string())?;
    Ok(changed > 0)
}

/// 启动时恢复：把中断在 probing/running 的任务退回 pending（断点续跑）。
/// 返回被恢复的任务数。
pub fn requeue_interrupted() -> Result<usize, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let now = now_iso();
    let changed = conn
        .execute(
            "UPDATE conversion_tasks SET status = 'pending', message = '重启后恢复', updated_at = ?1 \
             WHERE status IN ('probing','running')",
            params![now],
        )
        .map_err(|e| e.to_string())?;
    Ok(changed as usize)
}

/// 是否已被取消（worker 在子进程执行期间轮询用）。
pub fn is_canceled(id: i64) -> Result<bool, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let status: Option<String> = conn
        .query_row(
            "SELECT status FROM conversion_tasks WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(status.as_deref() == Some(STATUS_CANCELED))
}
