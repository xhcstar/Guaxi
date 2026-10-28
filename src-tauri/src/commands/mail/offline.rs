//! 离线写队列：网络不可用时把写操作排队，恢复后按顺序重放。
//!
//! 之前断网时所有操作都是「弹个 toast 报错」，用户点了标已读、点了星标，
//! 界面上没变化，重连后也不补——这正是邮箱客户端里最容易被骂的行为。
//!
//! 设计要点：
//! - 只排队**幂等**的写操作：标记已读 / 星标 / 移动 / 删除、以及「保存草稿」。
//!   发信不进这个队列（有独立的 outbox + 退避），否则「刚在离线状态下写的一封信」
//!   会和「撤回窗口」的语义搅在一起。
//! - 每条操作记 account / kind / payload。重放时按账户加同步锁，
//!   与实时操作共用同一条长连接，不会互相踩。
//! - 队列持久化进 SQLite 的 `offline_queue` 表（迁移 v3 建），崩溃后不丢。

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

/// 一条离线操作。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineOp {
    pub id: String,
    pub account_id: String,
    /// `seen` | `flagged` | `move` | `delete` | `draft`
    pub kind: String,
    pub folder: String,
    #[serde(default)]
    pub uids: Vec<u32>,
    /// 布尔参数（seen / flagged 的值）。
    #[serde(default)]
    pub value: bool,
    /// 目标文件夹（move 用）或草稿的原始报文（draft 用，base64）。
    #[serde(default)]
    pub payload: String,
    #[serde(default)]
    pub created_at: i64,
    /// 已经重放失败过几次。
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub last_error: String,
}

fn conn() -> Result<rusqlite::Connection, String> {
    super::store::connection()
}

pub fn init_table() -> Result<(), String> {
    let conn = conn()?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS offline_queue (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            folder TEXT NOT NULL DEFAULT '',
            uids_json TEXT NOT NULL DEFAULT '[]',
            value INTEGER NOT NULL DEFAULT 0,
            payload TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL DEFAULT 0,
            attempts INTEGER NOT NULL DEFAULT 0,
            last_error TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_offline_queue_account ON offline_queue (account_id, created_at);
        "#,
    )
    .map_err(|e| format!("创建离线队列表失败: {e}"))
}

/// 入队。同一账户 / 同类型 / 同文件夹 / 同 UID 集合的操作会合并，
/// 避免「来回点已读未读十次」排十条一模一样的命令。
pub fn enqueue(op: &OfflineOp) -> Result<(), String> {
    init_table()?;
    let conn = conn()?;
    // 合并：同账户 + 同 kind + 同 folder + 完全相同的 uids 集合，取最新一次。
    let mut stmt = conn
        .prepare(
            "SELECT id, uids_json FROM offline_queue
             WHERE account_id=?1 AND kind=?2 AND folder=?3
             ORDER BY created_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let existing: Vec<(String, String)> = stmt
        .query_map(params![op.account_id, op.kind, op.folder], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    let mut sorted = op.uids.clone();
    sorted.sort_unstable();
    for (id, uids_json) in existing {
        let mut theirs: Vec<u32> = serde_json::from_str(&uids_json).unwrap_or_default();
        theirs.sort_unstable();
        if theirs == sorted {
            conn.execute(
                "UPDATE offline_queue SET value=?1, payload=?2, last_error='', attempts=0,
                        created_at=?3 WHERE id=?4",
                params![op.value as i32, op.payload, op.created_at, id],
            )
            .map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    conn.execute(
        "INSERT INTO offline_queue (id, account_id, kind, folder, uids_json, value, payload,
             created_at, attempts, last_error)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,0,'')",
        params![
            op.id,
            op.account_id,
            op.kind,
            op.folder,
            serde_json::to_string(&op.uids).unwrap_or_else(|_| "[]".into()),
            op.value as i32,
            op.payload,
            op.created_at,
        ],
    )
    .map_err(|e| format!("写入离线队列失败: {e}"))?;
    Ok(())
}

pub fn list(account_id: Option<&str>) -> Result<Vec<OfflineOp>, String> {
    init_table()?;
    let conn = conn()?;
    let sql = match account_id {
        Some(_) => {
            "SELECT id, account_id, kind, folder, uids_json, value, payload, created_at, attempts, last_error
             FROM offline_queue WHERE account_id=?1 ORDER BY created_at ASC"
        }
        None => {
            "SELECT id, account_id, kind, folder, uids_json, value, payload, created_at, attempts, last_error
             FROM offline_queue ORDER BY created_at ASC"
        }
    };
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let map_row = |row: &rusqlite::Row<'_>| {
        Ok(OfflineOp {
            id: row.get(0)?,
            account_id: row.get(1)?,
            kind: row.get(2)?,
            folder: row.get(3)?,
            uids: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
            value: row.get::<_, i32>(5)? != 0,
            payload: row.get(6)?,
            created_at: row.get(7)?,
            attempts: row.get(8)?,
            last_error: row.get(9)?,
        })
    };
    let rows = match account_id {
        Some(id) => stmt
            .query_map(params![id], map_row)
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?,
        None => stmt
            .query_map([], map_row)
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?,
    };
    Ok(rows)
}

pub fn remove(id: &str) -> Result<(), String> {
    let conn = conn()?;
    conn.execute("DELETE FROM offline_queue WHERE id=?1", params![id])
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// 删除账户时一并清掉它的离线操作：账户没了，这些操作再也投不出去。
pub fn delete_account_queue(account_id: &str) -> Result<usize, String> {
    init_table()?;
    let conn = conn()?;
    conn.execute(
        "DELETE FROM offline_queue WHERE account_id=?1",
        params![account_id],
    )
    .map_err(|e| e.to_string())
}


pub fn count(account_id: &str) -> Result<i64, String> {
    init_table()?;
    let conn = conn()?;
    conn.query_row(
        "SELECT COUNT(*) FROM offline_queue WHERE account_id=?1",
        params![account_id],
        |r| r.get(0),
    )
    .optional()
    .map(|v| v.unwrap_or(0))
    .map_err(|e| e.to_string())
}

/// 重放结果。
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayResult {
    pub replayed: usize,
    pub failed: usize,
    pub errors: Vec<String>,
    pub remaining: i64,
}


/// 该错误是否像是在「网络不可用」时产生的——只有这种情况才入离线队列。
///
/// 判定刻意保守：只看 IO / TLS / 连接失败。认证失败、服务器拒绝之类不该排队，
/// 因为它们重放一百次也不会成功，只会让队列变成垃圾桶。
pub fn is_offline_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    const NET_MARKERS: &[&str] = &[
        "timed out",
        "timeout",
        "connection refused",
        "connection reset",
        "connection aborted",
        "network is unreachable",
        "no route to host",
        "dns",
        "failed to lookup",
        "temporarily unavailable",
        "broken pipe",
        "unexpected eof",
        "服务器关闭了连接",
        "连接失败",
        "网络",
        "读取服务器响应失败",
        "超时",
        "i/o",
    ];
    // 认证失败里也可能带 "connection" 字样，先排除掉。
    const NON_OFFLINE: &[&str] = &[
        "认证失败",
        "authentication",
        "invalid credentials",
        "登录失败",
        "拒绝",
        "unknown user",
    ];
    if NON_OFFLINE.iter().any(|m| lower.contains(m)) {
        return false;
    }
    NET_MARKERS.iter().any(|m| lower.contains(m))
}
