//! 邮箱本地库：账户、文件夹、邮件列表与正文、规则、待发队列、检索索引。
//!
//! 存储策略：
//! - 每次都开连接的短连接模式（与 `resource_hunter::store` 一致），WAL 下并发安全；
//! - 列表（`messages`）与正文（`message_bodies`）分表：列表页只扫小表，正文按 uid 点查；
//! - 检索索引：FTS5 + jieba 预分词。中文没有词边界，`unicode61` 会把整段汉字当成一个词，
//!   搜「报告」搜不到「季度报告」；trigram 分词器能绕开，但它要求查询词至少 3 个字符，
//!   中文最常见的两字词（报告 / 发票 / 合同）恰好全部失效。所以沿用库里已有的做法
//!   （`searcher.rs` 同款）：写入前用 jieba 把文本切成空格分隔的词，查询串也切一遍，
//!   再交给 `unicode61`。索引里存的是词序列，中文两字词照样能命中。

use super::model::{
    Account, FolderRole, MailFolder, MailRule, MailSettings, MessageDetail, MessageSummary,
    OutboxItem, SearchQuery,
};
use once_cell::sync::OnceCell;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

static DB_PATH: OnceCell<PathBuf> = OnceCell::new();

fn data_root(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {e}"))?
        .join("mail");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建邮箱数据目录失败: {e}"))?;
    Ok(dir)
}

pub fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    data_root(app).map(|d| d.join("mail.db"))
}

fn open() -> Result<Connection, String> {
    let path = DB_PATH.get().ok_or("邮箱库未初始化")?;
    let conn = Connection::open(path).map_err(|e| format!("打开邮箱库失败: {e}"))?;
    conn.pragma_update(None, "journal_mode", "WAL").ok();
    conn.pragma_update(None, "busy_timeout", 5000).ok();
    Ok(conn)
}

pub fn init(app: &AppHandle) -> Result<(), String> {
    let path = db_path(app)?;
    let _ = DB_PATH.set(path);
    let conn = open()?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS accounts (
            id TEXT PRIMARY KEY,
            email TEXT NOT NULL,
            display_name TEXT NOT NULL DEFAULT '',
            imap_host TEXT NOT NULL DEFAULT '',
            imap_port INTEGER NOT NULL DEFAULT 993,
            imap_security TEXT NOT NULL DEFAULT 'ssl',
            smtp_host TEXT NOT NULL DEFAULT '',
            smtp_port INTEGER NOT NULL DEFAULT 465,
            smtp_security TEXT NOT NULL DEFAULT 'ssl',
            username TEXT NOT NULL DEFAULT '',
            auth_kind TEXT NOT NULL DEFAULT 'password',
            proxy_json TEXT,
            accent TEXT NOT NULL DEFAULT '#5b7cfa',
            signature TEXT NOT NULL DEFAULT '',
            folder_map_json TEXT NOT NULL DEFAULT '{}',
            created_at INTEGER NOT NULL DEFAULT 0,
            last_sync_at INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS folders (
            account_id TEXT NOT NULL,
            name TEXT NOT NULL,
            display_name TEXT NOT NULL DEFAULT '',
            delimiter TEXT,
            role TEXT NOT NULL DEFAULT 'custom',
            unread INTEGER NOT NULL DEFAULT 0,
            total INTEGER NOT NULL DEFAULT 0,
            uid_validity INTEGER NOT NULL DEFAULT 0,
            uid_next INTEGER NOT NULL DEFAULT 0,
            selectable INTEGER NOT NULL DEFAULT 1,
            PRIMARY KEY (account_id, name)
        );

        CREATE TABLE IF NOT EXISTS messages (
            account_id TEXT NOT NULL,
            folder TEXT NOT NULL,
            uid INTEGER NOT NULL,
            message_id TEXT,
            subject TEXT NOT NULL DEFAULT '',
            from_name TEXT NOT NULL DEFAULT '',
            from_addr TEXT NOT NULL DEFAULT '',
            to_json TEXT NOT NULL DEFAULT '[]',
            cc_json TEXT NOT NULL DEFAULT '[]',
            date INTEGER NOT NULL DEFAULT 0,
            size INTEGER NOT NULL DEFAULT 0,
            snippet TEXT NOT NULL DEFAULT '',
            seen INTEGER NOT NULL DEFAULT 0,
            flagged INTEGER NOT NULL DEFAULT 0,
            answered INTEGER NOT NULL DEFAULT 0,
            draft INTEGER NOT NULL DEFAULT 0,
            deleted INTEGER NOT NULL DEFAULT 0,
            has_attachments INTEGER NOT NULL DEFAULT 0,
            thread_key TEXT NOT NULL DEFAULT '',
            labels TEXT NOT NULL DEFAULT '',
            outgoing INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (account_id, folder, uid)
        );
        CREATE INDEX IF NOT EXISTS idx_messages_date ON messages (account_id, folder, date DESC);
        CREATE INDEX IF NOT EXISTS idx_messages_thread ON messages (thread_key);
        CREATE INDEX IF NOT EXISTS idx_messages_from ON messages (from_addr);

        CREATE TABLE IF NOT EXISTS message_bodies (
            account_id TEXT NOT NULL,
            folder TEXT NOT NULL,
            uid INTEGER NOT NULL,
            html TEXT,
            text TEXT NOT NULL DEFAULT '',
            attachments_json TEXT NOT NULL DEFAULT '[]',
            body_fetched INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (account_id, folder, uid)
        );

        CREATE TABLE IF NOT EXISTS rules (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL DEFAULT '',
            enabled INTEGER NOT NULL DEFAULT 1,
            priority INTEGER NOT NULL DEFAULT 0,
            match_mode TEXT NOT NULL DEFAULT 'all',
            condition_json TEXT NOT NULL DEFAULT '{}',
            action_json TEXT NOT NULL DEFAULT '{}',
            stop_processing INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS outbox (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL,
            subject TEXT NOT NULL DEFAULT '',
            to_addrs TEXT NOT NULL DEFAULT '',
            payload_json TEXT NOT NULL,
            due_at INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL DEFAULT 0,
            sent INTEGER NOT NULL DEFAULT 0,
            claimed_at INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL DEFAULT ''
        );
        "#,
    )
    .map_err(|e| format!("建表失败: {e}"))?;

    // 追加列：老版本建的表没有 claimed_at，直接 ALTER 会报「重复列」，忽略即可。
    let _ = conn.execute(
        "ALTER TABLE outbox ADD COLUMN claimed_at INTEGER NOT NULL DEFAULT 0",
        [],
    );

    ensure_search_index(&conn)?;
    Ok(())
}

/// 建检索索引。普通（非 external-content）FTS5 表：rowid 与 `messages.rowid` 对齐，
/// 由 `reindex_message` 在列表/正文写入后重建该行。
fn ensure_search_index(conn: &Connection) -> Result<(), String> {
    let exists: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='messages_fts'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if exists.is_none() {
        conn.execute_batch(
            r#"
            CREATE VIRTUAL TABLE messages_fts USING fts5(
                subject, from_addr, from_name, to_text, body,
                tokenize='unicode61 remove_diacritics 2'
            );
            "#,
        )
        .map_err(|e| format!("创建检索索引失败: {e}"))?;
    }
    Ok(())
}

/// jieba 预分词：把中文切成空格分隔的词，英文/数字/标点原样保留。
///
/// 复用 `tantivy_jieba`（与 `searcher.rs` 同一个全局词典实例）：
/// 「索引认的词」与「查询认的词」因此天然一致。
pub fn segment(text: &str) -> String {
    use tantivy::tokenizer::{TokenStream, Tokenizer};
    if text.trim().is_empty() {
        return String::new();
    }
    let mut tokenizer = tantivy_jieba::JiebaTokenizer::new();
    let mut stream = tokenizer.token_stream(text);
    let mut out = String::with_capacity(text.len() + text.len() / 2);
    while stream.advance() {
        let token = stream.token();
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&token.text);
    }
    out
}

fn fts_available_conn(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name='messages_fts'",
        [],
        |_| Ok(()),
    )
    .optional()
    .ok()
    .flatten()
    .is_some()
}

pub fn fts_available() -> bool {
    open().map(|c| fts_available_conn(&c)).unwrap_or(false)
}

/// 重建一封邮件的索引行。列表（主题/地址）与正文（body）任一变化都调用它。
fn reindex_message(
    tx: &rusqlite::Transaction<'_>,
    account_id: &str,
    folder: &str,
    uid: u32,
) -> Result<(), String> {
    if !fts_available_conn(tx) {
        return Ok(());
    }
    let row: Option<(i64, String, String, String, String, String)> = tx
        .query_row(
            "SELECT m.rowid, m.subject, m.from_addr, m.from_name, m.to_json,
                    COALESCE((SELECT b.text FROM message_bodies b
                              WHERE b.account_id=m.account_id AND b.folder=m.folder AND b.uid=m.uid), '')
             FROM messages m WHERE m.account_id=?1 AND m.folder=?2 AND m.uid=?3",
            params![account_id, folder, uid],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((rowid, subject, from_addr, from_name, to_json, body)) = row else {
        return Ok(());
    };
    tx.execute("DELETE FROM messages_fts WHERE rowid=?1", params![rowid])
        .ok();
    tx.execute(
        "INSERT INTO messages_fts(rowid, subject, from_addr, from_name, to_text, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            rowid,
            segment(&subject),
            segment(&from_addr),
            segment(&from_name),
            segment(&to_json),
            segment(&body),
        ],
    )
    .map_err(|e| format!("写入索引失败: {e}"))?;
    Ok(())
}

// ---------------- 账户 ----------------

const ACCOUNT_COLS: &str = "id, email, display_name, imap_host, imap_port, imap_security, \
    smtp_host, smtp_port, smtp_security, username, auth_kind, proxy_json, accent, signature, \
    folder_map_json, created_at, last_sync_at";

fn row_to_account(row: &Row<'_>) -> rusqlite::Result<Account> {
    let proxy_json: Option<String> = row.get(11)?;
    let folder_map_json: String = row.get(14)?;
    Ok(Account {
        id: row.get(0)?,
        email: row.get(1)?,
        display_name: row.get(2)?,
        imap_host: row.get(3)?,
        imap_port: row.get(4)?,
        imap_security: super::model::Security::parse(&row.get::<_, String>(5)?),
        smtp_host: row.get(6)?,
        smtp_port: row.get(7)?,
        smtp_security: super::model::Security::parse(&row.get::<_, String>(8)?),
        username: row.get(9)?,
        auth_kind: row.get(10)?,
        proxy: proxy_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        accent: row.get(12)?,
        signature: row.get(13)?,
        folder_map: serde_json::from_str(&folder_map_json).unwrap_or_default(),
        created_at: row.get(15)?,
        last_sync_at: row.get(16)?,
    })
}

pub fn list_accounts() -> Result<Vec<Account>, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {ACCOUNT_COLS} FROM accounts ORDER BY created_at ASC"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], row_to_account)
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn get_account(id: &str) -> Result<Option<Account>, String> {
    let conn = open()?;
    conn.query_row(
        &format!("SELECT {ACCOUNT_COLS} FROM accounts WHERE id = ?1"),
        params![id],
        row_to_account,
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn upsert_account(account: &Account) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        r#"INSERT INTO accounts (id, email, display_name, imap_host, imap_port, imap_security,
                smtp_host, smtp_port, smtp_security, username, auth_kind, proxy_json, accent,
                signature, folder_map_json, created_at, last_sync_at)
           VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
           ON CONFLICT(id) DO UPDATE SET
                email=excluded.email, display_name=excluded.display_name,
                imap_host=excluded.imap_host, imap_port=excluded.imap_port,
                imap_security=excluded.imap_security, smtp_host=excluded.smtp_host,
                smtp_port=excluded.smtp_port, smtp_security=excluded.smtp_security,
                username=excluded.username, auth_kind=excluded.auth_kind,
                proxy_json=excluded.proxy_json, accent=excluded.accent,
                signature=excluded.signature, folder_map_json=excluded.folder_map_json"#,
        params![
            account.id,
            account.email,
            account.display_name,
            account.imap_host,
            account.imap_port,
            account.imap_security.as_str(),
            account.smtp_host,
            account.smtp_port,
            account.smtp_security.as_str(),
            account.username,
            account.auth_kind,
            account
                .proxy
                .as_ref()
                .and_then(|p| serde_json::to_string(p).ok()),
            account.accent,
            account.signature,
            serde_json::to_string(&account.folder_map).unwrap_or_else(|_| "{}".into()),
            if account.created_at == 0 {
                chrono::Utc::now().timestamp()
            } else {
                account.created_at
            },
            account.last_sync_at,
        ],
    )
    .map_err(|e| format!("保存账户失败: {e}"))?;
    Ok(())
}

pub fn delete_account(id: &str) -> Result<(), String> {
    let mut conn = open()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM accounts WHERE id = ?1", params![id])
        .ok();
    tx.execute("DELETE FROM folders WHERE account_id = ?1", params![id])
        .ok();
    if fts_available_conn(&tx) {
        tx.execute(
            "DELETE FROM messages_fts WHERE rowid IN
             (SELECT rowid FROM messages WHERE account_id = ?1)",
            params![id],
        )
        .ok();
    }
    tx.execute("DELETE FROM messages WHERE account_id = ?1", params![id])
        .ok();
    tx.execute("DELETE FROM message_bodies WHERE account_id = ?1", params![id])
        .ok();
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn touch_account_sync(id: &str, at: i64) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "UPDATE accounts SET last_sync_at = ?1 WHERE id = ?2",
        params![at, id],
    )
    .ok();
    Ok(())
}

// ---------------- 文件夹 ----------------

pub fn replace_folders(account_id: &str, folders: &[MailFolder]) -> Result<(), String> {
    let mut conn = open()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM folders WHERE account_id = ?1", params![account_id])
        .ok();
    for f in folders {
        tx.execute(
            r#"INSERT OR REPLACE INTO folders
               (account_id, name, display_name, delimiter, role, unread, total, uid_validity, uid_next, selectable)
               VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)"#,
            params![
                account_id,
                f.name,
                f.display_name,
                f.delimiter,
                f.role.as_str(),
                f.unread,
                f.total,
                f.uid_validity,
                f.uid_next,
                f.selectable as i32,
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_folders(account_id: &str) -> Result<Vec<MailFolder>, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare(
            "SELECT account_id, name, display_name, delimiter, role, unread, total, uid_validity, uid_next, selectable
             FROM folders WHERE account_id = ?1 ORDER BY role ASC, display_name ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![account_id], |row| {
            Ok(MailFolder {
                account_id: row.get(0)?,
                name: row.get(1)?,
                display_name: row.get(2)?,
                delimiter: row.get(3)?,
                role: FolderRole::parse(&row.get::<_, String>(4)?),
                unread: row.get(5)?,
                total: row.get(6)?,
                uid_validity: row.get(7)?,
                uid_next: row.get(8)?,
                selectable: row.get::<_, i32>(9)? != 0,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn update_folder_counts(
    account_id: &str,
    name: &str,
    unread: u32,
    total: u32,
    uid_validity: u32,
    uid_next: u32,
) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "UPDATE folders SET unread=?1, total=?2, uid_validity=?3, uid_next=?4 WHERE account_id=?5 AND name=?6",
        params![unread, total, uid_validity, uid_next, account_id, name],
    )
    .ok();
    Ok(())
}

/// 未读总数（左侧账户徽标）。
pub fn unread_counts() -> Result<Vec<(String, u32)>, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare("SELECT account_id, SUM(unread) FROM folders WHERE role='inbox' GROUP BY account_id")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u32))
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// UIDVALIDITY 变化时整段作废某个文件夹的本地缓存。
pub fn clear_folder_messages(account_id: &str, folder: &str) -> Result<(), String> {
    let mut conn = open()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if fts_available_conn(&tx) {
        tx.execute(
            "DELETE FROM messages_fts WHERE rowid IN
             (SELECT rowid FROM messages WHERE account_id=?1 AND folder=?2)",
            params![account_id, folder],
        )
        .ok();
    }
    tx.execute(
        "DELETE FROM messages WHERE account_id=?1 AND folder=?2",
        params![account_id, folder],
    )
    .ok();
    tx.execute(
        "DELETE FROM message_bodies WHERE account_id=?1 AND folder=?2",
        params![account_id, folder],
    )
    .ok();
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 文件夹里本地已知的最大 UID（增量同步的起点）。
pub fn last_uid(account_id: &str, folder: &str) -> Result<u32, String> {
    let conn = open()?;
    let value: Option<i64> = conn
        .query_row(
            "SELECT MAX(uid) FROM messages WHERE account_id=?1 AND folder=?2",
            params![account_id, folder],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(value.unwrap_or(0).max(0) as u32)
}

pub fn folder_uid_state(account_id: &str, folder: &str) -> Result<(u32, u32), String> {
    let conn = open()?;
    conn.query_row(
        "SELECT uid_validity, uid_next FROM folders WHERE account_id=?1 AND name=?2",
        params![account_id, folder],
        |r| Ok((r.get::<_, u32>(0)?, r.get::<_, u32>(1)?)),
    )
    .optional()
    .map(|r| r.unwrap_or((0, 0)))
    .map_err(|e| e.to_string())
}

// ---------------- 邮件 ----------------

const MSG_COLS: &str = "account_id, folder, uid, message_id, subject, from_name, from_addr, \
    to_json, cc_json, date, size, snippet, seen, flagged, answered, draft, deleted, \
    has_attachments, thread_key, labels, outgoing";

fn row_to_summary(row: &Row<'_>) -> rusqlite::Result<MessageSummary> {
    Ok(MessageSummary {
        account_id: row.get(0)?,
        folder: row.get(1)?,
        uid: row.get(2)?,
        message_id: row.get(3)?,
        subject: row.get(4)?,
        from_name: row.get(5)?,
        from_addr: row.get(6)?,
        to_json: row.get(7)?,
        cc_json: row.get(8)?,
        date: row.get(9)?,
        size: row.get(10)?,
        snippet: row.get(11)?,
        seen: row.get::<_, i32>(12)? != 0,
        flagged: row.get::<_, i32>(13)? != 0,
        answered: row.get::<_, i32>(14)? != 0,
        draft: row.get::<_, i32>(15)? != 0,
        deleted: row.get::<_, i32>(16)? != 0,
        has_attachments: row.get::<_, i32>(17)? != 0,
        thread_key: row.get(18)?,
        labels: row.get(19)?,
        outgoing: row.get::<_, i32>(20)? != 0,
    })
}

/// 增量同步写回列表。
pub fn upsert_summaries(items: &[MessageSummary]) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }
    let mut conn = open()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for m in items {
        tx.execute(
            r#"INSERT INTO messages (account_id, folder, uid, message_id, subject, from_name, from_addr,
                    to_json, cc_json, date, size, snippet, seen, flagged, answered, draft, deleted,
                    has_attachments, thread_key, labels, outgoing)
               VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)
               ON CONFLICT(account_id, folder, uid) DO UPDATE SET
                    message_id=excluded.message_id, subject=excluded.subject,
                    from_name=excluded.from_name, from_addr=excluded.from_addr,
                    to_json=excluded.to_json, cc_json=excluded.cc_json, date=excluded.date,
                    size=excluded.size, snippet=excluded.snippet, seen=excluded.seen,
                    flagged=excluded.flagged, answered=excluded.answered, draft=excluded.draft,
                    deleted=excluded.deleted, has_attachments=excluded.has_attachments,
                    thread_key=excluded.thread_key, labels=excluded.labels"#,
            params![
                m.account_id,
                m.folder,
                m.uid,
                m.message_id,
                m.subject,
                m.from_name,
                m.from_addr,
                m.to_json,
                m.cc_json,
                m.date,
                m.size,
                m.snippet,
                m.seen as i32,
                m.flagged as i32,
                m.answered as i32,
                m.draft as i32,
                m.deleted as i32,
                m.has_attachments as i32,
                m.thread_key,
                m.labels,
                m.outgoing as i32,
            ],
        )
        .map_err(|e| format!("写入邮件失败: {e}"))?;
        reindex_message(&tx, &m.account_id, &m.folder, m.uid)?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 正文入库时同步刷新全文索引。
pub fn upsert_body(
    account_id: &str,
    folder: &str,
    uid: u32,
    html: Option<&str>,
    text: &str,
    attachments_json: &str,
) -> Result<(), String> {
    let mut conn = open()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        r#"INSERT INTO message_bodies (account_id, folder, uid, html, text, attachments_json, body_fetched)
           VALUES (?1,?2,?3,?4,?5,?6,1)
           ON CONFLICT(account_id, folder, uid) DO UPDATE SET
                html=excluded.html, text=excluded.text,
                attachments_json=excluded.attachments_json, body_fetched=1"#,
        params![account_id, folder, uid, html, text, attachments_json],
    )
    .map_err(|e| e.to_string())?;
    reindex_message(&tx, account_id, folder, uid)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 缓存命中时返回 (html, text, 附件 JSON, 是否已经取过正文)。
pub fn get_body(
    account_id: &str,
    folder: &str,
    uid: u32,
) -> Result<Option<(Option<String>, String, String, bool)>, String> {
    let conn = open()?;
    conn.query_row(
        "SELECT html, text, attachments_json, body_fetched FROM message_bodies
         WHERE account_id=?1 AND folder=?2 AND uid=?3",
        params![account_id, folder, uid],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i32>(3)? != 0,
            ))
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn get_summary(
    account_id: &str,
    folder: &str,
    uid: u32,
) -> Result<Option<MessageSummary>, String> {
    let conn = open()?;
    conn.query_row(
        &format!("SELECT {MSG_COLS} FROM messages WHERE account_id=?1 AND folder=?2 AND uid=?3"),
        params![account_id, folder, uid],
        row_to_summary,
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn delete_message(account_id: &str, folder: &str, uid: u32) -> Result<(), String> {
    let mut conn = open()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if fts_available_conn(&tx) {
        if let Some(rowid) = tx
            .query_row(
                "SELECT rowid FROM messages WHERE account_id=?1 AND folder=?2 AND uid=?3",
                params![account_id, folder, uid],
                |r| r.get::<_, i64>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
        {
            tx.execute("DELETE FROM messages_fts WHERE rowid=?1", params![rowid])
                .ok();
        }
    }
    tx.execute(
        "DELETE FROM messages WHERE account_id=?1 AND folder=?2 AND uid=?3",
        params![account_id, folder, uid],
    )
    .ok();
    tx.execute(
        "DELETE FROM message_bodies WHERE account_id=?1 AND folder=?2 AND uid=?3",
        params![account_id, folder, uid],
    )
    .ok();
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 本地标记（已读 / 星标 / 分类标签）。服务器确认由调用方另行（`imap::set_flags`）。
pub fn set_local_flags(
    account_id: &str,
    folder: &str,
    uids: &[u32],
    seen: Option<bool>,
    flagged: Option<bool>,
    labels: Option<&str>,
) -> Result<(), String> {
    let mut conn = open()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for uid in uids {
        if let Some(v) = seen {
            tx.execute(
                "UPDATE messages SET seen=?1 WHERE account_id=?2 AND folder=?3 AND uid=?4",
                params![v as i32, account_id, folder, uid],
            )
            .ok();
        }
        if let Some(v) = flagged {
            tx.execute(
                "UPDATE messages SET flagged=?1 WHERE account_id=?2 AND folder=?3 AND uid=?4",
                params![v as i32, account_id, folder, uid],
            )
            .ok();
        }
        if let Some(v) = labels {
            tx.execute(
                "UPDATE messages SET labels=?1 WHERE account_id=?2 AND folder=?3 AND uid=?4",
                params![v, account_id, folder, uid],
            )
            .ok();
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_recent(
    account_id: &str,
    folder: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<MessageSummary>, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {MSG_COLS} FROM messages WHERE account_id=?1 AND folder=?2 AND deleted=0
             ORDER BY date DESC, uid DESC LIMIT ?3 OFFSET ?4"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![account_id, folder, limit, offset], row_to_summary)
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

// ---------------- 检索 ----------------

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// FTS5 查询串：用户输入先用 jieba 切词（与写入侧同一套分词），
/// 每个词加双引号（避免 `AND`/`OR`/`*` 被当成语法），词间 AND。
/// 最后一个纯 ASCII 词加 `*`，让 `invo` 能命中 `invoice`。
fn fts_query(text: &str) -> String {
    let segmented = segment(text);
    let terms: Vec<&str> = segmented.split_whitespace().collect();
    if terms.is_empty() {
        return String::new();
    }
    let last = terms.len() - 1;
    terms
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let escaped = w.replace('"', "\"\"");
            let ascii_word = w.chars().all(|c| c.is_ascii_alphanumeric());
            if i == last && ascii_word && w.len() >= 2 {
                format!("\"{escaped}\"*")
            } else {
                format!("\"{escaped}\"")
            }
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

pub fn search(query: &SearchQuery) -> Result<Vec<MessageSummary>, String> {
    let limit = query.limit.unwrap_or(200).min(2000);
    let offset = query.offset.unwrap_or(0);
    let use_fts = fts_available() && !query.text.trim().is_empty();

    let mut sql = String::from("SELECT ");
    for (i, col) in MSG_COLS.split(',').map(|s| s.trim()).enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("m.{col}"));
    }
    sql.push_str(" FROM messages m");
    if use_fts {
        sql.push_str(" JOIN messages_fts f ON f.rowid = m.rowid");
    }
    sql.push_str(" WHERE m.deleted = 0");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if !query.account_ids.is_empty() {
        let placeholders = vec!["?"; query.account_ids.len()].join(",");
        sql.push_str(&format!(" AND m.account_id IN ({placeholders})"));
        for id in &query.account_ids {
            args.push(Box::new(id.clone()));
        }
    }
    if let Some(folder) = query.folder.as_ref().filter(|f| !f.trim().is_empty()) {
        sql.push_str(" AND m.folder = ?");
        args.push(Box::new(folder.clone()));
    }
    if let Some(scope) = query.scope.as_deref() {
        match scope {
            "unread" => sql.push_str(" AND m.seen = 0 AND m.outgoing = 0"),
            "flagged" => sql.push_str(" AND m.flagged = 1"),
            "attachments" => sql.push_str(" AND m.has_attachments = 1"),
            "drafts" => {
                sql.push_str(" AND m.folder IN (SELECT name FROM folders WHERE role = 'drafts')")
            }
            _ => {}
        }
    }
    if let Some(true) = query.unread_only {
        sql.push_str(" AND m.seen = 0");
    }
    if let Some(true) = query.starred_only {
        sql.push_str(" AND m.flagged = 1");
    }
    if let Some(want) = query.has_attachment {
        sql.push_str(if want {
            " AND m.has_attachments = 1"
        } else {
            " AND m.has_attachments = 0"
        });
    }
    if let Some(after) = query.after {
        sql.push_str(" AND m.date >= ?");
        args.push(Box::new(after));
    }
    if let Some(before) = query.before {
        sql.push_str(" AND m.date <= ?");
        args.push(Box::new(before));
    }
    if let Some(min) = query.min_size {
        sql.push_str(" AND m.size >= ?");
        args.push(Box::new(min as i64));
    }
    if let Some(max) = query.max_size {
        sql.push_str(" AND m.size <= ?");
        args.push(Box::new(max as i64));
    }
    for (col, value) in [
        ("m.from_addr", query.from.as_ref()),
        ("m.subject", query.subject.as_ref()),
    ] {
        if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
            sql.push_str(&format!(" AND {col} LIKE ? ESCAPE '\\'"));
            args.push(Box::new(format!("%{}%", escape_like(v.trim()))));
        }
    }
    if let Some(v) = query.to.as_ref().filter(|v| !v.trim().is_empty()) {
        sql.push_str(" AND (m.to_json LIKE ? ESCAPE '\\' OR m.cc_json LIKE ? ESCAPE '\\')");
        let pat = format!("%{}%", escape_like(v.trim()));
        args.push(Box::new(pat.clone()));
        args.push(Box::new(pat));
    }
    if !query.labels.is_empty() {
        let ors = query
            .labels
            .iter()
            .map(|_| "m.labels LIKE ? ESCAPE '\\'")
            .collect::<Vec<_>>()
            .join(" OR ");
        sql.push_str(&format!(" AND ({ors})"));
        for label in &query.labels {
            args.push(Box::new(format!("%{}%", escape_like(label))));
        }
    }

    if use_fts {
        let expr = fts_query(&query.text);
        if !expr.is_empty() {
            // MATCH 左操作数可以是表名（整表）或列名（单列）。
            let target = match query.field.as_deref().unwrap_or("all") {
                "subject" => "f.subject",
                "from" => "f.from_addr",
                "to" => "f.to_text",
                "body" => "f.body",
                _ => "f",
            };
            sql.push_str(&format!(" AND {target} MATCH ?"));
            args.push(Box::new(expr));
        }
    } else if !query.text.trim().is_empty() {
        let pat = format!("%{}%", escape_like(query.text.trim()));
        sql.push_str(
            " AND (m.subject LIKE ? ESCAPE '\\' OR m.from_addr LIKE ? ESCAPE '\\' \
             OR m.from_name LIKE ? ESCAPE '\\' OR m.to_json LIKE ? ESCAPE '\\' \
             OR m.uid IN (SELECT uid FROM message_bodies WHERE account_id=m.account_id \
                          AND folder=m.folder AND text LIKE ? ESCAPE '\\'))",
        );
        for _ in 0..5 {
            args.push(Box::new(pat.clone()));
        }
    }

    let sort_col = match query.sort.as_deref().unwrap_or("date") {
        "size" => "m.size",
        "sender" => "m.from_name",
        "subject" => "m.subject",
        _ => "m.date",
    };
    let dir = if query.descending.unwrap_or(true) {
        "DESC"
    } else {
        "ASC"
    };
    sql.push_str(&format!(
        " ORDER BY {sort_col} {dir}, m.uid {dir} LIMIT ? OFFSET ?"
    ));
    args.push(Box::new(limit as i64));
    args.push(Box::new(offset as i64));

    let conn = open()?;
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("检索语句无效: {e}"))?;
    let params_ref: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let rows = stmt
        .query_map(params_ref.as_slice(), row_to_summary)
        .map_err(|e| format!("检索失败: {e}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| format!("检索读取失败: {e}"))?;
    Ok(rows)
}

/// 搜索建议：本地已有发件人与主题里抽取匹配项。
pub fn suggest(prefix: &str, limit: u32) -> Result<Vec<String>, String> {
    let conn = open()?;
    let pattern = format!("%{}%", escape_like(prefix.trim()));
    let mut out: Vec<String> = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT from_addr FROM messages WHERE from_addr LIKE ?1 ESCAPE '\\'
                 AND from_addr <> '' LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let values: Vec<String> = stmt
            .query_map(params![pattern, limit], |r| r.get::<_, String>(0))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default();
        out.extend(values);
    }
    {
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT subject FROM messages WHERE subject LIKE ?1 ESCAPE '\\'
                 AND subject <> '' LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let values: Vec<String> = stmt
            .query_map(params![pattern, limit], |r| r.get::<_, String>(0))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default();
        out.extend(values);
    }
    out.sort_by_key(|s| s.to_lowercase());
    out.dedup();
    out.truncate(limit as usize);
    Ok(out)
}

/// 会话聚合：同一 thread_key 下的所有消息，按日期正序。
pub fn thread(thread_key: &str) -> Result<Vec<MessageSummary>, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {MSG_COLS} FROM messages WHERE thread_key=?1 AND deleted=0 ORDER BY date ASC, uid ASC"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![thread_key], row_to_summary)
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// 标签清单（规则分类结果的去重列表），供侧栏「分类」区。
pub fn labels() -> Result<Vec<(String, u32)>, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare("SELECT labels FROM messages WHERE labels <> ''")
        .map_err(|e| e.to_string())?;
    let mut counts: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    for row in rows.flatten() {
        for label in row.split(',') {
            let label = label.trim();
            if !label.is_empty() {
                *counts.entry(label.to_string()).or_insert(0) += 1;
            }
        }
    }
    Ok(counts.into_iter().collect())
}

// ---------------- 规则 ----------------

pub fn list_rules() -> Result<Vec<MailRule>, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, enabled, priority, match_mode, condition_json, action_json, stop_processing, created_at
             FROM rules ORDER BY priority ASC, created_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let cond: String = row.get(5)?;
            let act: String = row.get(6)?;
            Ok(MailRule {
                id: row.get(0)?,
                name: row.get(1)?,
                enabled: row.get::<_, i32>(2)? != 0,
                priority: row.get(3)?,
                match_mode: row.get(4)?,
                condition: serde_json::from_str(&cond).unwrap_or_default(),
                action: serde_json::from_str(&act).unwrap_or_default(),
                stop_processing: row.get::<_, i32>(7)? != 0,
                created_at: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn upsert_rule(rule: &MailRule) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        r#"INSERT INTO rules (id, name, enabled, priority, match_mode, condition_json, action_json, stop_processing, created_at)
           VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
           ON CONFLICT(id) DO UPDATE SET name=excluded.name, enabled=excluded.enabled,
                priority=excluded.priority, match_mode=excluded.match_mode,
                condition_json=excluded.condition_json, action_json=excluded.action_json,
                stop_processing=excluded.stop_processing"#,
        params![
            rule.id,
            rule.name,
            rule.enabled as i32,
            rule.priority,
            rule.match_mode,
            serde_json::to_string(&rule.condition).unwrap_or_else(|_| "{}".into()),
            serde_json::to_string(&rule.action).unwrap_or_else(|_| "{}".into()),
            rule.stop_processing as i32,
            if rule.created_at == 0 {
                chrono::Utc::now().timestamp()
            } else {
                rule.created_at
            },
        ],
    )
    .map_err(|e| format!("保存规则失败: {e}"))?;
    Ok(())
}

pub fn delete_rule(id: &str) -> Result<(), String> {
    let conn = open()?;
    conn.execute("DELETE FROM rules WHERE id=?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------- 待发队列 ----------------

pub fn enqueue_outbox(item: &OutboxItem, payload_json: &str) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "INSERT OR REPLACE INTO outbox (id, account_id, subject, to_addrs, payload_json, due_at, created_at, sent)
         VALUES (?1,?2,?3,?4,?5,?6,?7,0)",
        params![
            item.id,
            item.account_id,
            item.subject,
            item.to,
            payload_json,
            item.due_at,
            item.created_at
        ],
    )
    .map_err(|e| format!("写入待发队列失败: {e}"))?;
    Ok(())
}

pub fn list_outbox() -> Result<Vec<OutboxItem>, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare("SELECT id, account_id, subject, to_addrs, due_at, created_at FROM outbox WHERE sent=0 ORDER BY due_at ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(OutboxItem {
                id: row.get(0)?,
                account_id: row.get(1)?,
                subject: row.get(2)?,
                to: row.get(3)?,
                due_at: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// 取到期的待发邮件，并**在同一个事务里把它们标记为已认领**。
///
/// 这一步不能省：启动时补发一次、定时器再补发一次，两个调用会在同一秒里看到同一行，
/// 结果是同一封邮件发两遍。认领窗口取 5 分钟——超过这个时间还没发出去的（进程崩过）
/// 允许下一次重试。
pub fn take_due_outbox(now: i64) -> Result<Vec<(String, String)>, String> {
    let mut conn = open()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let stale = now - 300;
    let ids: Vec<String> = {
        let mut stmt = tx
            .prepare(
                "SELECT id FROM outbox
                 WHERE sent=0 AND due_at<=?1 AND claimed_at<=?2
                 ORDER BY due_at ASC",
            )
            .map_err(|e| e.to_string())?;
        // 先把行收进 Vec，再让 stmt 在块结束时析构：直接返回 MappedRows 的迭代结果
        // 会让 stmt 在临时值仍被借用时被释放（E0597）。
        let collected: Vec<String> = stmt
            .query_map(params![now, stale], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        collected
    };
    let mut rows: Vec<(String, String)> = Vec::new();
    for id in ids {
        let changed = tx
            .execute(
                "UPDATE outbox SET claimed_at=?1 WHERE id=?2 AND sent=0 AND claimed_at<=?3",
                params![now, id, stale],
            )
            .unwrap_or(0);
        if changed == 0 {
            continue;
        }
        if let Ok(payload) = tx.query_row(
            "SELECT payload_json FROM outbox WHERE id=?1",
            params![id],
            |r| r.get::<_, String>(0),
        ) {
            rows.push((id, payload));
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn mark_outbox_sent(id: &str) -> Result<(), String> {
    let conn = open()?;
    conn.execute("UPDATE outbox SET sent=1 WHERE id=?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 释放认领：发送失败后立刻允许重试。
pub fn release_outbox(id: &str) -> Result<(), String> {
    let conn = open()?;
    conn.execute("UPDATE outbox SET claimed_at=0 WHERE id=?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn drop_outbox(id: &str) -> Result<(), String> {
    let conn = open()?;
    conn.execute("DELETE FROM outbox WHERE id=?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------- 设置 ----------------

const META_SETTINGS_KEY: &str = "settings";

pub fn load_settings() -> MailSettings {
    let Ok(conn) = open() else {
        return MailSettings::default();
    };
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key=?1",
            params![META_SETTINGS_KEY],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten();
    raw.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_settings(settings: &MailSettings) -> Result<(), String> {
    let conn = open()?;
    let json = serde_json::to_string(settings).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![META_SETTINGS_KEY, json],
    )
    .map_err(|e| format!("保存设置失败: {e}"))?;
    Ok(())
}

/// 清空本地缓存（可限定账户）。账户凭据、规则、设置都保留。
pub fn clear_cache(account_id: Option<&str>) -> Result<(), String> {
    let mut conn = open()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if fts_available_conn(&tx) {
        match account_id {
            Some(id) => {
                tx.execute(
                    "DELETE FROM messages_fts WHERE rowid IN
                     (SELECT rowid FROM messages WHERE account_id=?1)",
                    params![id],
                )
                .ok();
            }
            None => {
                tx.execute("DELETE FROM messages_fts", []).ok();
            }
        }
    }
    match account_id {
        Some(id) => {
            tx.execute("DELETE FROM messages WHERE account_id=?1", params![id])
                .ok();
            tx.execute("DELETE FROM message_bodies WHERE account_id=?1", params![id])
                .ok();
        }
        None => {
            tx.execute("DELETE FROM messages", []).ok();
            tx.execute("DELETE FROM message_bodies", []).ok();
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 清理超出保留期的本地缓存（服务器上的邮件不动）。
pub fn purge_older_than(cutoff: i64) -> Result<usize, String> {
    let mut conn = open()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if fts_available_conn(&tx) {
        tx.execute(
            "DELETE FROM messages_fts WHERE rowid IN
             (SELECT rowid FROM messages WHERE date < ?1 AND flagged = 0)",
            params![cutoff],
        )
        .ok();
    }
    let count = tx
        .execute(
            "DELETE FROM messages WHERE date < ?1 AND flagged = 0",
            params![cutoff],
        )
        .unwrap_or(0);
    tx.execute(
        "DELETE FROM message_bodies WHERE NOT EXISTS (SELECT 1 FROM messages m \
         WHERE m.account_id=message_bodies.account_id AND m.folder=message_bodies.folder \
         AND m.uid=message_bodies.uid)",
        [],
    )
    .ok();
    tx.commit().map_err(|e| e.to_string())?;
    Ok(count)
}

/// 详细视图的基础壳：列表行 + 空正文，由调用方补齐。
pub fn base_detail(summary: &MessageSummary) -> MessageDetail {
    MessageDetail {
        account_id: summary.account_id.clone(),
        folder: summary.folder.clone(),
        uid: summary.uid,
        message_id: summary.message_id.clone(),
        subject: summary.subject.clone(),
        from_name: summary.from_name.clone(),
        from_addr: summary.from_addr.clone(),
        to: summary.to_addrs(),
        cc: summary.cc_addrs(),
        bcc: Vec::new(),
        reply_to: Vec::new(),
        date: summary.date,
        in_reply_to: String::new(),
        references: String::new(),
        html: None,
        text: String::new(),
        attachments: Vec::new(),
        headers: Vec::new(),
        labels: summary.labels.clone(),
        seen: summary.seen,
        flagged: summary.flagged,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_splits_chinese_into_words() {
        let out = segment("季度报告");
        assert!(out.contains(' '), "中文应当被切分成多个词: {out}");
        assert_eq!(segment("   "), "");
    }

    #[test]
    fn fts_query_quotes_terms_and_prefixes_ascii_tail() {
        // 纯英文查询：最后一个词加前缀通配，输入 `invo` 能命中 `invoice`。
        assert_eq!(fts_query("invo"), "\"invo\"*");
        // 中英混排：中文词不加通配（jieba 给出的已是完整词），词之间是 AND。
        let q = fts_query("invoice 报告");
        assert_eq!(q, "\"invoice\" AND \"报告\"");
    }

    #[test]
    fn escape_like_escapes_wildcards() {
        assert_eq!(escape_like("50%_off"), "50\\%\\_off");
    }
}
