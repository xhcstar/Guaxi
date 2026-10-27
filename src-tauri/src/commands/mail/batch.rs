//! 批量写入：一次同步只开一个连接、一个事务。
//!
//! 背景：POP3 / IMAP 同步是「逐封」循环，每封都要做四件事——分配 uid、写摘要、
//! 写正文、缓存原文。如果每个函数各自 `Connection::open()`，一次同步 1000 封就是
//! 4000 次打开 + 4000 次 `PRAGMA journal_mode=WAL`（每次都要拿锁），实测这是收信
//! 卡顿的主要来源：等待几乎全在 `busy_timeout` 上，而不是网络。
//!
//! 这里提供一组「带连接」的版本：调用方开一次连接，循环里复用；
//! 事务批量提交，避免每封一次 fsync。
//!
//! 为什么不把现有函数改成接受 `&Connection`：
//! 那些函数被几十处调用（标记、移动、删除、规则），全部改造面太大且容易改错。
//! 同步是唯一的热路径，只给它开一条批量通道，风险最小。

use super::*;
use rusqlite::{params, Connection, OptionalExtension};
use tracing::debug;

/// 一批邮件的写入器。持有连接，批量提交。
pub struct BodyBatch<'a> {
    conn: &'a Connection,
}

impl<'a> BodyBatch<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// 同一个 UIDL 永远得到同一个 uid：只查表，不存在时不分配（由 `assign` 负责）。
    pub fn uid_for(&self, account_id: &str, uidl: &str) -> Result<Option<u32>, String> {
        self.conn
            .query_row(
                "SELECT uid FROM pop3_uids WHERE account_id=?1 AND uidl=?2",
                params![account_id, uidl],
                |r| r.get::<_, i64>(0),
            )
            .optional()
            .map(|value| value.map(|v| v as u32))
            .map_err(|e| e.to_string())
    }

    /// 分配一个稳定 uid（已存在则复用）。
    pub fn assign_uid(&self, account_id: &str, uidl: &str) -> Result<u32, String> {
        if let Some(uid) = self.uid_for(account_id, uidl)? {
            return Ok(uid);
        }
        let next: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(uid), 0) + 1 FROM pop3_uids WHERE account_id=?1",
                params![account_id],
                |r| r.get(0),
            )
            .unwrap_or(1);
        let next: i64 = next.max(
            self.conn
                .query_row(
                    "SELECT COALESCE(MAX(uid), 0) + 1 FROM messages WHERE account_id=?1",
                    params![account_id],
                    |r| r.get(0),
                )
                .unwrap_or(1),
        );
        self.conn
            .execute(
                "INSERT INTO pop3_uids (account_id, uidl, uid) VALUES (?1, ?2, ?3)",
                params![account_id, uidl, next],
            )
            .map_err(|e| format!("分配 POP3 uid 失败: {e}"))?;
        Ok(next as u32)
    }

    /// 写摘要 + 正文 + 原文缓存，全在一个连接里完成（调用方负责事务边界）。
    pub fn store_message(
        &self,
        summary: &MessageSummary,
        html: Option<&str>,
        text: &str,
        attachments_json: &str,
        raw: Option<&[u8]>,
    ) -> Result<(), String> {
        store::upsert_summaries_on(self.conn, std::slice::from_ref(summary))?;
        store::upsert_body_on(
            self.conn,
            &summary.account_id,
            &summary.folder,
            summary.uid,
            html,
            text,
            attachments_json,
        )?;
        if let Some(raw) = raw {
            store::cache_raw_body_on(
                self.conn,
                &summary.account_id,
                &summary.folder,
                summary.uid,
                raw,
            )
            .ok();
        }
        Ok(())
    }

}

/// 用一个连接跑一段批量写入；`f` 里返回 Err 时整批回滚。
pub fn with_batch<T>(
    f: impl FnOnce(&BodyBatch<'_>) -> Result<T, String>,
) -> Result<T, String> {
    let mut conn = store::open_direct()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let batch = BodyBatch::new(&tx);
    let result = f(&batch)?;
    tx.commit().map_err(|e| {
        debug!(error = %e, "mail: 批量写入提交失败");
        e.to_string()
    })?;
    Ok(result)
}
