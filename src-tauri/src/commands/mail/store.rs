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
    Account, FolderRole, MailFolder, MailRule, MailSettings, MailTemplate, MessageDetail,
    MessageSummary, OutboxItem, SearchQuery, SenderRule,
};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// 数据库路径。用户可以把邮件库放到别的盘，所以不能用 OnceCell（不可变）。
static DB_PATH: once_cell::sync::Lazy<std::sync::RwLock<Option<PathBuf>>> =
    once_cell::sync::Lazy::new(|| std::sync::RwLock::new(None));

/// 默认数据根目录：<app_data>/mail。
fn default_data_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {e}"))
        .map(|dir| dir.join("mail"))
}

/// 当前生效的数据根目录：用户指定了自定义路径就用它，否则用默认位置。
pub fn data_root(app: &AppHandle) -> Result<PathBuf, String> {
    let custom = read_storage_dir();
    let dir = if custom.trim().is_empty() {
        default_data_root(app)?
    } else {
        PathBuf::from(custom.trim())
    };
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建邮箱数据目录失败: {e}"))?;
    Ok(dir)
}

fn current_db_path() -> Option<PathBuf> {
    DB_PATH.read().ok().and_then(|guard| guard.clone())
}

fn set_db_path(path: PathBuf) {
    if let Ok(mut guard) = DB_PATH.write() {
        *guard = Some(path);
    }
}

/// 自定义存储路径（存在 meta 表；空表示用默认位置）。
pub fn read_storage_dir() -> String {
    let Ok(conn) = open() else {
        // 库还没建好（首次启动）：退回默认，避免递归。
        return String::new();
    };
    conn.query_row("SELECT value FROM meta WHERE key='storage_dir'", [], |r| {
        r.get::<_, String>(0)
    })
    .optional()
    .ok()
    .flatten()
    .unwrap_or_default()
}

pub fn set_storage_dir(dir: &str) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('storage_dir', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![dir],
    )
    .map_err(|e| format!("保存存储路径失败: {e}"))?;
    Ok(())
}

pub fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    data_root(app).map(|d| d.join("mail.db"))
}

/// 供同模块族（通讯录）复用：通讯录与邮箱共用一个库，需要同一个连接入口。
pub(crate) fn connection() -> Result<Connection, String> {
    open()
}

/// 供批量通道使用：与内部 `open()` 同一套 pragma，但公开给 `batch` 模块。
pub(crate) fn open_direct() -> Result<Connection, String> {
    open()
}

fn open() -> Result<Connection, String> {
    let path = current_db_path().ok_or("邮箱库未初始化")?;
    let conn = Connection::open(path).map_err(|e| format!("打开邮箱库失败: {e}"))?;
    conn.pragma_update(None, "journal_mode", "WAL").ok();
    conn.pragma_update(None, "busy_timeout", 5000).ok();
    Ok(conn)
}

/// 当前 schema 版本。改表结构时 +1，并在 `migrate` 里追加对应的迁移步骤。
///
/// 为什么必须做版本化迁移：老用户的 `mail.db` 已经存在，`CREATE TABLE IF NOT EXISTS`
/// 只会补新表、不会补新列；一旦某天查询里用到新列，整个邮件页会直接报
/// 「no such column」而不是「少一个功能」。
pub const SCHEMA_VERSION: i32 = 6;

pub fn init(app: &AppHandle) -> Result<(), String> {
    let path = db_path(app)?;
    set_db_path(path);
    let mut conn = open()?;
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

        -- 十万封量级的检索支撑：
        -- 1) 跨账户「全部邮件」按时间倒序要能走索引，不能全表排序；
        -- 2) 未读 / 星标 / 附件是最高频的过滤条件，做成部分索引后
        --    查询只扫命中的行，不受总条数影响；
        -- 3) 大小写不敏感的发件人匹配（LOWER(from_addr)）单独建表达式索引，
        --    否则每次搜索都要对全表做函数计算。
        CREATE INDEX IF NOT EXISTS idx_messages_date_all ON messages (date DESC, uid DESC);
        CREATE INDEX IF NOT EXISTS idx_messages_unread ON messages (date DESC) WHERE seen = 0;
        CREATE INDEX IF NOT EXISTS idx_messages_flagged ON messages (date DESC) WHERE flagged = 1;
        CREATE INDEX IF NOT EXISTS idx_messages_attach ON messages (date DESC) WHERE has_attachments = 1;
        CREATE INDEX IF NOT EXISTS idx_messages_from_lower ON messages (LOWER(from_addr));

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

    migrate(&mut conn)?;
    ensure_search_index(&conn)?;
    Ok(())
}

/// 版本化迁移。`user_version` 为 0 表示「新库」或「迁移机制之前的老库」——
/// 两种情况都走同一套幂等步骤（新库的表已经是最新的，ALTER 会因列已存在而被跳过）。
fn migrate(conn: &mut Connection) -> Result<(), String> {
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if version >= SCHEMA_VERSION {
        return Ok(());
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if version < 1 {
        // v1：待发队列的重试与退避。老库只有 claimed_at 一道认领闩。
        add_column(&tx, "outbox", "attempts", "INTEGER NOT NULL DEFAULT 0")?;
        add_column(&tx, "outbox", "next_attempt_at", "INTEGER NOT NULL DEFAULT 0")?;
        add_column(&tx, "outbox", "last_error", "TEXT NOT NULL DEFAULT ''")?;
    }
    if version < 2 {
        // v2：多设备一致性。modseq 记录服务器给的状态修订号；flags_synced_at
        // 让「定期全量比对 FLAGS」有明确的时间起点，而不是每次同步都重拉。
        add_column(&tx, "messages", "modseq", "INTEGER NOT NULL DEFAULT 0")?;
        add_column(&tx, "folders", "modseq", "INTEGER NOT NULL DEFAULT 0")?;
        add_column(&tx, "folders", "flags_synced_at", "INTEGER NOT NULL DEFAULT 0")?;
    }
    if version < 3 {
        // v3：OAuth2 元数据（令牌本体在 keychain）、共享邮箱、Sieve、模板与收发件人名单。
        add_column(&tx, "accounts", "oauth_provider", "TEXT NOT NULL DEFAULT ''")?;
        add_column(&tx, "accounts", "oauth_expires", "INTEGER NOT NULL DEFAULT 0")?;
        add_column(&tx, "accounts", "delegate_json", "TEXT NOT NULL DEFAULT ''")?;
        add_column(&tx, "accounts", "sieve_json", "TEXT NOT NULL DEFAULT ''")?;
        tx.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS templates (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL DEFAULT '',
                subject TEXT NOT NULL DEFAULT '',
                html TEXT NOT NULL DEFAULT '',
                text TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL DEFAULT 0
            );

            -- 收/发件人名单：kind = allow（白名单）| block（黑名单）。
            -- 规则引擎在最前面按它判定，规则表里不再堆一长串手写匹配串。
            CREATE TABLE IF NOT EXISTS sender_rules (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL DEFAULT 'block',
                pattern TEXT NOT NULL,
                note TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL DEFAULT 0
            );
            "#,
        )
        .map_err(|e| format!("迁移到 v3 失败: {e}"))?;
    }
    if version < 4 {
        // v4：POP3 账户。POP3 没有 UID，服务器给的 UIDL 才是跨会话稳定的身份，
        // 而列表 / 正文 / 搜索都以 (account, folder, uid) 为主键。这里把 UIDL 映射成
        // 一个稳定的整数 uid：**同一个 UIDL 永远得到同一个 uid**，换会话也不会漂移。
        add_column(&tx, "accounts", "protocol", "TEXT NOT NULL DEFAULT 'imap'")?;
        add_column(&tx, "accounts", "pop3_host", "TEXT NOT NULL DEFAULT ''")?;
        add_column(&tx, "accounts", "pop3_port", "INTEGER NOT NULL DEFAULT 995")?;
        add_column(&tx, "accounts", "pop3_security", "TEXT NOT NULL DEFAULT 'ssl'")?;
        add_column(
            &tx,
            "accounts",
            "pop3_leave_on_server",
            "INTEGER NOT NULL DEFAULT 1",
        )?;
        tx.execute_batch(
            r#"
            -- UIDL → uid 的稳定映射。POP3 服务器只给 UIDL，模型里的主键却是整数 uid；
            -- 映射表保证「同一封邮件」无论在哪个会话、经过几次同步都落到同一行。
            CREATE TABLE IF NOT EXISTS pop3_uids (
                account_id TEXT NOT NULL,
                uidl TEXT NOT NULL,
                uid INTEGER NOT NULL,
                PRIMARY KEY (account_id, uidl),
                UNIQUE (account_id, uid)
            );
            "#,
        )
        .map_err(|e| format!("迁移到 v4 失败: {e}"))?;
    }
    if version < 5 {
        // v5：三件互相独立、但都是「本地状态」的事。
        //
        // 1) message_bodies.raw：整封原文缓存。POP3 没有「按 uid 取单个附件」的接口，
        //    IMAP 也得重下一整封；缓存之后附件打开 / 另存 .eml / 导出都能立刻命中。
        //    只对「打开过的邮件」缓存，按单封大小封顶（见 `RAW_CACHE_LIMIT`）。
        add_column(&tx, "message_bodies", "raw", "BLOB")?;
        // 2) outbox.staged：多次投递失败后不再自动重试，转为「在发件箱里等用户处置」。
        add_column(&tx, "outbox", "staged", "INTEGER NOT NULL DEFAULT 0")?;
        // 3) folders.local_only：只有本地才有的文件夹（发件箱）。服务器同步必须绕开它，
        //    否则会去 SELECT 一个永远不存在的名字，每一轮同步都报错。
        add_column(&tx, "folders", "local_only", "INTEGER NOT NULL DEFAULT 0")?;
        // 4) folders.user_created：是不是用户自己建（或显式保留）的文件夹。
        //    服务器同步会把 QQ / 163 自带的「其他文件夹」及其遗留子目录一起带下来，
        //    这些不是用户要的；侧栏只展示用户自己建的。老库里已有的文件夹
        //    一律按「有邮件的算用户在用」回填一次，不把用户之前建的目录藏起来。
        add_column(&tx, "folders", "user_created", "INTEGER NOT NULL DEFAULT 0")?;
        tx.execute(
            "UPDATE folders SET user_created = 1 WHERE total > 0 OR unread > 0",
            [],
        )
        .ok();
        // 老库补一次「存档」占位：这个角色以前没被自动补齐，用户的账户里往往没有。
        // 按**角色**判断是否已有（而不是按名字）：服务器上叫「存档」/「Archive」/
        // 「All Mail」的都有，只有缺这个角色时才补，否则会多出一个空文件夹。
        tx.execute(
            "INSERT OR IGNORE INTO folders
                 (account_id, name, display_name, delimiter, role, unread, total,
                  uid_validity, uid_next, selectable, local_only)
             SELECT a.id, 'Archive', '存档', NULL, 'archive', 0, 0, 0, 0, 1, 0
             FROM accounts a
             WHERE NOT EXISTS (SELECT 1 FROM folders f
                               WHERE f.account_id = a.id AND f.role = 'archive')",
            [],
        )
        .ok();
        // 发件箱是纯本地文件夹：每个账户都补一条。
        tx.execute(
            "INSERT OR IGNORE INTO folders
                 (account_id, name, display_name, delimiter, role, unread, total,
                  uid_validity, uid_next, selectable, local_only)
             SELECT a.id, 'Outbox', '发件箱', NULL, 'outbox', 0, 0, 0, 0, 1, 1
             FROM accounts a
             WHERE NOT EXISTS (SELECT 1 FROM folders f
                               WHERE f.account_id = a.id AND f.role = 'outbox')",
            [],
        )
        .ok();
    }
    if version < 6 {
        // v6：把「我的文件夹」还给用户自己。
        //
        // v5 回填时用「有邮件就算用户在用」把服务商自带的目录（QQ 的「其他文件夹」等）
        // 全标成了用户自己的；imap 每次同步又按「本地已存在 / 有邮件」把它标回来，
        // 于是删掉的文件夹下一轮同步又自己冒出来（真实反馈）。这里一次性清掉
        // 自定义角色的标记：之后只有在我们这边新建、或右键「保留在侧栏」的才会出现。
        // 标准角色（收件箱 / 已发送…）不受影响，它们走侧栏的固定区块。
        tx.execute("UPDATE folders SET user_created = 0 WHERE role = 'custom'", [])
            .map_err(|e| format!("迁移到 v6 失败: {e}"))?;
    }
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(|e| format!("写入 schema 版本失败: {e}"))?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 加列并在已存在时静默跳过。SQLite 没有 `ADD COLUMN IF NOT EXISTS`。
fn add_column(
    tx: &rusqlite::Transaction<'_>,
    table: &str,
    column: &str,
    decl: &str,
) -> Result<(), String> {
    let exists: bool = tx
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .any(|name| name == column);
    if exists {
        return Ok(());
    }
    tx.execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"), [])
        .map(|_| ())
        .map_err(|e| format!("迁移 {table}.{column} 失败: {e}"))
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
    tx: &Connection,
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
            // 正文若已加密，不做分词——密文索引既无意义又会污染检索结果。
            // 主题、发件人、收件人仍在索引里，信头级搜索不受影响。
            if body.starts_with(super::crypto::ENC_PREFIX) {
                String::new()
            } else {
                segment(&body)
            },
        ],
    )
    .map_err(|e| format!("写入索引失败: {e}"))?;
    Ok(())
}

// ---------------- 账户 ----------------

const ACCOUNT_COLS: &str = "id, email, display_name, imap_host, imap_port, imap_security, \
    smtp_host, smtp_port, smtp_security, username, auth_kind, proxy_json, accent, signature, \
    folder_map_json, created_at, last_sync_at, oauth_provider, oauth_expires, delegate_json, \
    sieve_json, protocol, pop3_host, pop3_port, pop3_security, pop3_leave_on_server";

fn row_to_account(row: &Row<'_>) -> rusqlite::Result<Account> {
    let proxy_json: Option<String> = row.get(11)?;
    let folder_map_json: String = row.get(14)?;
    let delegate_json: String = row.get(19)?;
    let sieve_json: String = row.get(20)?;
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
        oauth_provider: row.get(17)?,
        oauth_expires: row.get(18)?,
        proxy: proxy_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        accent: row.get(12)?,
        signature: row.get(13)?,
        folder_map: serde_json::from_str(&folder_map_json).unwrap_or_default(),
        delegate: serde_json::from_str(&delegate_json).unwrap_or_default(),
        sieve: serde_json::from_str(&sieve_json).ok(),
        created_at: row.get(15)?,
        last_sync_at: row.get(16)?,
        protocol: row.get(21)?,
        pop3_host: row.get(22)?,
        pop3_port: row.get(23)?,
        pop3_security: super::model::Security::parse(&row.get::<_, String>(24)?),
        pop3_leave_on_server: row.get::<_, i32>(25)? != 0,
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
                signature, folder_map_json, created_at, last_sync_at,
                oauth_provider, oauth_expires, delegate_json, sieve_json,
                protocol, pop3_host, pop3_port, pop3_security, pop3_leave_on_server)
           VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,
                   ?22,?23,?24,?25,?26)
           ON CONFLICT(id) DO UPDATE SET
                email=excluded.email, display_name=excluded.display_name,
                imap_host=excluded.imap_host, imap_port=excluded.imap_port,
                imap_security=excluded.imap_security, smtp_host=excluded.smtp_host,
                smtp_port=excluded.smtp_port, smtp_security=excluded.smtp_security,
                username=excluded.username, auth_kind=excluded.auth_kind,
                proxy_json=excluded.proxy_json, accent=excluded.accent,
                signature=excluded.signature, folder_map_json=excluded.folder_map_json,
                oauth_provider=excluded.oauth_provider, oauth_expires=excluded.oauth_expires,
                delegate_json=excluded.delegate_json, sieve_json=excluded.sieve_json,
                protocol=excluded.protocol, pop3_host=excluded.pop3_host,
                pop3_port=excluded.pop3_port, pop3_security=excluded.pop3_security,
                pop3_leave_on_server=excluded.pop3_leave_on_server"#,
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
            account.oauth_provider,
            account.oauth_expires,
            serde_json::to_string(&account.delegate).unwrap_or_else(|_| "[]".into()),
            account
                .sieve
                .as_ref()
                .and_then(|s| serde_json::to_string(s).ok())
                .unwrap_or_default(),
            account.protocol,
            account.pop3_host,
            account.pop3_port,
            account.pop3_security.as_str(),
            account.pop3_leave_on_server as i32,
        ],
    )
    .map_err(|e| format!("保存账户失败: {e}"))?;
    Ok(())
}

/// 只更新 OAuth 访问令牌的过期时间（刷新令牌后调用）。
pub fn touch_oauth_expires(id: &str, expires: i64) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "UPDATE accounts SET oauth_expires=?1 WHERE id=?2",
        params![expires, id],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// 删除账户。
///
/// `delete_local_mail = true` 时连同本机缓存的邮件、正文、文件夹一起删掉
/// （用户选择「不留任何痕迹」）；为 false 时只移除账户本身，
/// 邮件数据留在库里——用户换回这台机器、或者只是暂时不想要这个账户时不会被误删。
///
/// 待发队列 / 离线队列里的条目无论如何都删：账户没了，这些操作永远投不出去。
pub fn delete_account(id: &str, delete_local_mail: bool) -> Result<(), String> {
    let mut conn = open()?;
    {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        // 这里不能再 `.ok()`：以前所有语句的错误都被吞掉，账户没删掉也照样提交，
        // 界面上什么都看不出来，用户只能看到「点了删除，账户还在」。
        tx.execute("DELETE FROM accounts WHERE id = ?1", params![id])
            .map_err(|e| format!("删除账户失败: {e}"))?;
        tx.execute("DELETE FROM outbox WHERE account_id = ?1", params![id])
            .map_err(|e| format!("清理待发队列失败: {e}"))?;
        if delete_local_mail {
            // 检索索引必须先删：它是 messages 的外部内容表，跟着主表一起清。
            if fts_available_conn(&tx) {
                tx.execute(
                    "DELETE FROM messages_fts WHERE rowid IN
                     (SELECT rowid FROM messages WHERE account_id = ?1)",
                    params![id],
                )
                .map_err(|e| format!("清理检索索引失败: {e}"))?;
            }
            for sql in [
                "DELETE FROM messages WHERE account_id = ?1",
                "DELETE FROM message_bodies WHERE account_id = ?1",
                "DELETE FROM pop3_uids WHERE account_id = ?1",
                "DELETE FROM folders WHERE account_id = ?1",
            ] {
                tx.execute(sql, params![id])
                    .map_err(|e| format!("清理本机邮件失败: {e}"))?;
            }
        }
        tx.commit().map_err(|e| format!("提交删除失败: {e}"))?;
    }
    // 复核一次：事务「成功」但行还在（只可能来自并发写入）时，必须报错而不是假装成功。
    let still_there: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM accounts WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if still_there != 0 {
        return Err("账户仍然存在，删除没有生效".to_string());
    }
    let _ = super::offline::delete_account_queue(id);
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

/// 用服务器返回的文件夹表替换本地表。
///
/// `user_created` 是用户本地的判断（「这个目录是不是我自己建的 / 我要求保留在侧栏」），
/// 服务器不知道，所以刷新时必须从旧行里继承，否则每同步一次用户的标记就丢一次。
/// 发件箱这类纯本地文件夹、以及服务器这次没返回的自建文件夹，都要原样保留——
/// 新建后第一轮 LIST 还看不到它是常态，抹掉就等于用户的文件夹自己消失了。
pub fn replace_folders(account_id: &str, folders: &[MailFolder]) -> Result<(), String> {
    let mut conn = open()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut kept: std::collections::HashMap<String, (bool, bool)> = std::collections::HashMap::new();
    {
        let mut stmt = tx
            .prepare("SELECT name, user_created, local_only FROM folders WHERE account_id=?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![account_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i32>(1)? != 0,
                    row.get::<_, i32>(2)? != 0,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows.flatten() {
            kept.insert(row.0, (row.1, row.2));
        }
    }
    // 纯本地的文件夹（发件箱）和用户自己建的文件夹都可能不在服务器这次的 LIST 里：
    // 先取出来，等服务器结果写完再放回去。以前只保 `local_only`，于是「刚在服务器上
    // 建好、还没来得及被 LIST 出来」的那一轮刷新就把用户新建的目录抹掉了——
    // 用户看到的就是「新建文件夹过一会儿自己消失了」。
    let mut kept_rows: Vec<MailFolder> = Vec::new();
    {
        let mut stmt = tx
            .prepare(
                "SELECT account_id, name, display_name, delimiter, role, unread, total,
                        uid_validity, uid_next, selectable, local_only, user_created
                 FROM folders WHERE account_id=?1 AND (local_only=1 OR user_created=1)",
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
                    local_only: row.get::<_, i32>(10)? != 0,
                    user_created: row.get::<_, i32>(11)? != 0,
                })
            })
            .map_err(|e| e.to_string())?;
        for row in rows.flatten() {
            kept_rows.push(row);
        }
    }

    tx.execute("DELETE FROM folders WHERE account_id = ?1", params![account_id])
        .ok();
    // 服务器这次真的返回了的，以服务器的数据为准（未读数、UID 有效性都更新了）；
    // 只有服务器没返回的才把本地那份原样放回去。
    let server_names: std::collections::HashSet<&str> =
        folders.iter().map(|f| f.name.as_str()).collect();
    let revived = kept_rows
        .iter()
        .filter(|f| !server_names.contains(f.name.as_str()));
    for f in folders.iter().chain(revived) {
        let inherited = kept.get(&f.name).copied().unwrap_or((false, false));
        // 服务器同步带下来的文件夹：沿用旧的 user_created；新出现的按「没有标记」处理，
        // 由侧栏决定只展示用户自己建的那些。
        let user_created = f.user_created || inherited.0;
        let local_only = f.local_only || inherited.1;
        tx.execute(
            r#"INSERT OR REPLACE INTO folders
               (account_id, name, display_name, delimiter, role, unread, total, uid_validity, uid_next, selectable, local_only, user_created)
               VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)"#,
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
                local_only as i32,
                user_created as i32,
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
            "SELECT account_id, name, display_name, delimiter, role, unread, total,
                    uid_validity, uid_next, selectable, local_only, user_created
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
                local_only: row.get::<_, i32>(10)? != 0,
                user_created: row.get::<_, i32>(11)? != 0,
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

/// 一个文件夹的（总封数, 未读数）。
///
/// 以前重算角标是把整个文件夹的行都读出来再数一遍：规则一次搬几百封时，
/// 每搬一封就要再来两遍全表读取，几百万行的物化让「对已有邮件执行」看起来像卡死。
/// 计数交给 SQLite，一条索引查询就够。
pub fn folder_counts(account_id: &str, name: &str) -> Result<(u32, u32), String> {
    let conn = open()?;
    conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN seen=0 THEN 1 ELSE 0 END), 0)
         FROM messages WHERE account_id=?1 AND folder=?2 AND deleted=0",
        params![account_id, name],
        |row| {
            Ok((
                row.get::<_, i64>(0)? as u32,
                row.get::<_, i64>(1)? as u32,
            ))
        },
    )
    .map_err(|e| e.to_string())
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

/// 删除文件夹行及其本地缓存（服务器侧已由 imap::delete_folder 删掉）。
/// 插入 / 更新一条文件夹记录（本地新建文件夹、或者服务器创建成功后同步本地表）。
pub fn insert_folder_row(folder: &MailFolder) -> Result<(), String> {
    let conn = open()?;
    // 不覆盖已有的 user_created / local_only 携带的信息：以调用方给的为准，
    // 但同名已存在时保留原来的「用户建的」标记。
    let existing_user: Option<i32> = conn
        .query_row(
            "SELECT user_created FROM folders WHERE account_id=?1 AND name=?2",
            params![folder.account_id, folder.name],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    conn.execute(
        r#"INSERT OR REPLACE INTO folders
           (account_id, name, display_name, delimiter, role, unread, total, uid_validity, uid_next, selectable, local_only, user_created)
           VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)"#,
        params![
            folder.account_id,
            folder.name,
            folder.display_name,
            folder.delimiter,
            folder.role.as_str(),
            folder.unread,
            folder.total,
            folder.uid_validity,
            folder.uid_next,
            folder.selectable as i32,
            folder.local_only as i32,
            (folder.user_created || existing_user.unwrap_or(0) != 0) as i32,
        ],
    )
    .map_err(|e| format!("写入文件夹失败: {e}"))?;
    Ok(())
}

/// 把文件夹标成 / 取消「保留在侧栏」（用户右键菜单里的动作）。
pub fn set_folder_user_created(account_id: &str, name: &str, value: bool) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "UPDATE folders SET user_created=?1 WHERE account_id=?2 AND name=?3",
        params![value as i32, account_id, name],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_folder_row(account_id: &str, name: &str) -> Result<(), String> {
    let mut conn = open()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if fts_available_conn(&tx) {
        tx.execute(
            "DELETE FROM messages_fts WHERE rowid IN
             (SELECT rowid FROM messages WHERE account_id=?1 AND folder=?2)",
            params![account_id, name],
        )
        .ok();
    }
    tx.execute(
        "DELETE FROM messages WHERE account_id=?1 AND folder=?2",
        params![account_id, name],
    )
    .ok();
    tx.execute(
        "DELETE FROM message_bodies WHERE account_id=?1 AND folder=?2",
        params![account_id, name],
    )
    .ok();
    tx.execute(
        "DELETE FROM folders WHERE account_id=?1 AND name=?2",
        params![account_id, name],
    )
    .ok();
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 把 WAL 内容并回主库文件。
///
/// 复制 / 迁移 / 备份之前必须先做：WAL 模式下最近的写入还在 `-wal` 里，
/// 只复制主库文件会得到一份「少了几十封邮件」的旧数据。
pub fn checkpoint() -> Result<(), String> {
    let conn = open()?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|e| format!("整理数据库失败: {e}"))?;
    Ok(())
}

/// 切换运行期使用的数据库路径（迁移后调用）。
pub fn switch_db_path(path: std::path::PathBuf) -> Result<(), String> {
    set_db_path(path);
    Ok(())
}

/// 清掉所有依赖「当前库内容」的进程内缓存。
/// 恢复备份 / 切换路径后如果不清，加密开关仍按旧库回答，行为会错位。
pub fn reset_runtime_caches() {
    if let Ok(mut guard) = ENCRYPT_CACHE.write() {
        *guard = None;
    }
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

// ---------------- POP3：UIDL ↔ uid ----------------

/// 把服务器给的 UIDL 映射成一个稳定的本地 uid。
///
/// POP3 只有「本次会话的序号」和「跨会话稳定的 UIDL」，而本地模型的主键是整数 uid。
/// 这里在同一个事务里查表、必要时分配新号：**同一个 UIDL 永远得到同一个 uid**，
/// 所以重复下载同一封邮件不会产生两行，列表里的已读 / 星标也不会丢。
///
/// 分配规则是 `MAX(uid) + 1`（从 1 开始）。用自增而不是哈希：UIDL 长度与字符集
/// 由服务器决定（有的给 `<abc>@host`，有的给一串数字），哈希既可能撞号也没有必要。
pub fn pop3_uid_for(account_id: &str, uidl: &str) -> Result<u32, String> {
    if uidl.trim().is_empty() {
        return Err("UIDL 为空".to_string());
    }
    let mut conn = open()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if let Ok(uid) = tx.query_row(
        "SELECT uid FROM pop3_uids WHERE account_id=?1 AND uidl=?2",
        params![account_id, uidl],
        |r| r.get::<_, i64>(0),
    ) {
        return Ok(uid as u32);
    }
    let next: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(uid), 0) + 1 FROM pop3_uids WHERE account_id=?1",
            params![account_id],
            |r| r.get(0),
        )
        .unwrap_or(1);
    // 同一账户的 uid 还要和 messages 里已存在的 uid 错开：切换协议或导入历史数据时，
    // pop3_uids 可能比 messages 里的 uid 小，直接分配就会撞上已有邮件。
    let next: i64 = next.max(
        tx.query_row(
            "SELECT COALESCE(MAX(uid), 0) + 1 FROM messages WHERE account_id=?1",
            params![account_id],
            |r| r.get(0),
        )
        .unwrap_or(1),
    );
    tx.execute(
        "INSERT INTO pop3_uids (account_id, uidl, uid) VALUES (?1, ?2, ?3)",
        params![account_id, uidl, next],
    )
    .map_err(|e| format!("分配 POP3 uid 失败: {e}"))?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(next as u32)
}

/// 该账户已经收过的 UIDL（用于「只拉新邮件」）。
pub fn pop3_known_uids(account_id: &str) -> Result<std::collections::HashSet<String>, String> {
    let conn = open()?;
    pop3_known_uids_on(&conn, account_id)
}

/// 复用调用方连接的 UIDL 集合查询（批量同步用）。
pub fn pop3_known_uids_on(
    conn: &Connection,
    account_id: &str,
) -> Result<std::collections::HashSet<String>, String> {
    let mut stmt = conn
        .prepare("SELECT uidl FROM pop3_uids WHERE account_id=?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![account_id], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let mut out = std::collections::HashSet::new();
    for row in rows {
        out.insert(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// 反查：某个本地 uid 对应的 UIDL（POP3 打开单封时用）。
pub fn pop3_uidl_for(account_id: &str, uid: u32) -> Result<Option<String>, String> {
    let conn = open()?;
    conn.query_row(
        "SELECT uidl FROM pop3_uids WHERE account_id=?1 AND uid=?2",
        params![account_id, uid],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

/// 删除某封 POP3 邮件后清掉它的 UIDL 映射：服务器上已经没有了，
/// 留着只会让「下一封刚好复用同一个 UIDL」的服务器（少见）误判成已收过。
pub fn pop3_forget_uidl(account_id: &str, uidl: &str) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "DELETE FROM pop3_uids WHERE account_id=?1 AND uidl=?2",
        params![account_id, uidl],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// 本地移动换了 uid 之后，把 UIDL 映射跟着挪到新号上。
///
/// 不跟着挪，`pop3_uids` 还指着那条已经删掉的旧行，同步时「这封还在不在本地」
/// 就查不到，于是当成用户删掉了 → 忘记 UIDL → 下一轮重新下载一份。
pub fn pop3_remap_uid(account_id: &str, from: u32, to: u32) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "UPDATE pop3_uids SET uid=?3 WHERE account_id=?1 AND uid=?2",
        params![account_id, from, to],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// 这个 uid 在本账户的**任意**文件夹里还存在吗。
///
/// POP3 的 uid 实际是账户内全局唯一的（`pop3_uid_for` 就按全局 MAX+1 分配），
/// 所以「按 uid 找这封邮件」不该限定文件夹：规则自动归类、手动移动、移入垃圾箱
/// 都会换文件夹，只查收件箱就会把「搬走了」误判成「删掉了」。
pub fn message_exists_any_folder(account_id: &str, uid: u32) -> Result<bool, String> {
    let conn = open()?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE account_id=?1 AND uid=?2",
            params![account_id, uid],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(count > 0)
}

/// 该账户下一个没人占用的 uid：`messages` 与 `pop3_uids` 两张表一起看。
///
/// 需要新号时必须按 [`pop3_uid_for`] 同一套规则来，否则可能撞上别的文件夹里的
/// 邮件，或撞上 `pop3_uids` 的 UNIQUE(account_id, uid)。
pub fn next_free_uid(account_id: &str) -> Result<u32, String> {
    let conn = open()?;
    let max_of = |table: &str| -> i64 {
        conn.query_row(
            &format!("SELECT COALESCE(MAX(uid), 0) FROM {table} WHERE account_id=?1"),
            params![account_id],
            |r| r.get(0),
        )
        .unwrap_or(0)
    };
    Ok(max_of("messages").max(max_of("pop3_uids")).saturating_add(1) as u32)
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

/// 上次全量比对状态的时间（多设备一致性）。
pub fn folder_flags_synced_at(account_id: &str, folder: &str) -> i64 {
    open()
        .ok()
        .and_then(|conn| {
            conn.query_row(
                "SELECT flags_synced_at FROM folders WHERE account_id=?1 AND name=?2",
                params![account_id, folder],
                |r| r.get::<_, i64>(0),
            )
            .optional()
            .ok()
            .flatten()
        })
        .unwrap_or(0)
}

pub fn touch_folder_flags_synced(account_id: &str, folder: &str, at: i64) {
    if let Ok(conn) = open() {
        let _ = conn.execute(
            "UPDATE folders SET flags_synced_at=?1 WHERE account_id=?2 AND name=?3",
            params![at, account_id, folder],
        );
    }
}

/// 服务器上已不存在的 UID：本地必须一并清掉，否则「别的客户端删了邮件，
/// 桌面端还留着」——这正是多设备不一致最直观的形态。
pub fn drop_missing_uids(
    account_id: &str,
    folder: &str,
    present: &std::collections::HashSet<u32>,
) -> Result<usize, String> {
    let existing: Vec<u32> = {
        let conn = open()?;
        let mut stmt = conn
            .prepare("SELECT uid FROM messages WHERE account_id=?1 AND folder=?2")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![account_id, folder], |r| r.get::<_, u32>(0))
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        rows
    };
    let missing: Vec<u32> = existing.into_iter().filter(|u| !present.contains(u)).collect();
    for uid in &missing {
        let _ = delete_message(account_id, folder, *uid);
    }
    Ok(missing.len())
}

/// 批量回写服务器 FLAGS（全量比对的结果）。
///
/// 单独成一个函数而不是复用 `set_local_flags`：那一个只处理 seen/flagged/labels，
/// 而全量比对还要同步 `answered` / `draft` / `deleted`——否则「别的客户端回复过」
/// 这条信息也会丢。
pub fn apply_server_flags(
    account_id: &str,
    folder: &str,
    rows: &[(u32, bool, bool, bool, bool)],
) -> Result<usize, String> {
    if rows.is_empty() {
        return Ok(0);
    }
    let mut conn = open()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut changed = 0usize;
    for (uid, seen, flagged, answered, deleted) in rows {
        changed += tx
            .execute(
                "UPDATE messages SET seen=?1, flagged=?2, answered=?3, deleted=?4
                 WHERE account_id=?5 AND folder=?6 AND uid=?7",
                params![
                    *seen as i32,
                    *flagged as i32,
                    *answered as i32,
                    *deleted as i32,
                    account_id,
                    folder,
                    uid
                ],
            )
            .unwrap_or(0);
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(changed)
}

/// 本地某一页邮件的 UID（全量比对只比对「已经同步到本地的那部分」）。
pub fn uids_in_folder(account_id: &str, folder: &str) -> Result<Vec<u32>, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare("SELECT uid FROM messages WHERE account_id=?1 AND folder=?2 ORDER BY uid ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![account_id, folder], |r| r.get::<_, u32>(0))
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
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
    upsert_summaries_on(&tx, items)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 同一个写入逻辑，但复用调用方的连接 / 事务（批量同步用）。
pub fn upsert_summaries_on(
    conn: &Connection,
    items: &[MessageSummary],
) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }
    for m in items {
        conn.execute(
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
        reindex_message(conn, &m.account_id, &m.folder, m.uid)?;
    }
    Ok(())
}

/// 整封原文缓存的大小上限。超过这个值的邮件不缓存原文：
/// 附件打开本来就要多等一会儿，而把 50 MB 的原文塞进库里会让选择变慢、
/// 备份变大，得不偿失。打开过的普通邮件（绝大多数在几百 KB 以内）则秒开。
pub const RAW_CACHE_LIMIT: usize = 32 * 1024 * 1024;

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
    write_body(&tx, account_id, folder, uid, html, text, attachments_json)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 复用调用方连接 / 事务的正文写入（批量同步用：一次同步一个事务）。
pub fn upsert_body_on(
    conn: &Connection,
    account_id: &str,
    folder: &str,
    uid: u32,
    html: Option<&str>,
    text: &str,
    attachments_json: &str,
) -> Result<(), String> {
    write_body(conn, account_id, folder, uid, html, text, attachments_json)
}

fn write_body(
    tx: &Connection,
    account_id: &str,
    folder: &str,
    uid: u32,
    html: Option<&str>,
    text: &str,
    attachments_json: &str,
) -> Result<(), String> {
    // 本地加密（默认开）：正文与附件清单在落盘前加密。
    // 代价要说明白——正文加密后 FTS 索引不到它，正文搜索会退化，
    // 这一点由 `encrypted_body_index_placeholder` 在前面挡住，不会静默失效。
    let encrypt = encryption_enabled();
    let sealed_html = match html {
        Some(value) if encrypt => Some(super::crypto::encrypt(account_id, value)?),
        other => other.map(|v| v.to_string()),
    };
    let sealed_text = if encrypt {
        super::crypto::encrypt(account_id, text)?
    } else {
        text.to_string()
    };
    let sealed_attachments = if encrypt {
        super::crypto::encrypt(account_id, attachments_json)?
    } else {
        attachments_json.to_string()
    };

    // 注意不要覆盖 raw：正文与原文是两次不同的写入（正文先到，原文在打开邮件时补）。
    tx.execute(
        r#"INSERT INTO message_bodies (account_id, folder, uid, html, text, attachments_json, body_fetched)
           VALUES (?1,?2,?3,?4,?5,?6,1)
           ON CONFLICT(account_id, folder, uid) DO UPDATE SET
                html=excluded.html, text=excluded.text,
                attachments_json=excluded.attachments_json, body_fetched=1"#,
        params![account_id, folder, uid, sealed_html, sealed_text, sealed_attachments],
    )
    .map_err(|e| e.to_string())?;
    reindex_message(tx, account_id, folder, uid)?;
    Ok(())
}

/// 缓存一封邮件的完整原文（RFC822 字节）。
///
/// 为什么值得单独存一份：附件打开、另存 .eml、导出 PDF/Word 都要「整封原文」，
/// 而这些操作以前每次都重新从服务器拉一遍——QQ 上一封 800 KB 的邮件，
/// 拉取 + TLS 往返就是十秒级。缓存之后同一封邮件的后续操作全是本地读盘。
///
/// 只缓存「用户已经打开过」的邮件；超过 `RAW_CACHE_LIMIT` 的不缓存。
/// 原文与正文一样走本地加密：里面有附件内容，不能明文躺在库里。
pub fn cache_raw_body(account_id: &str, folder: &str, uid: u32, raw: &[u8]) -> Result<(), String> {
    let conn = open()?;
    cache_raw_body_on(&conn, account_id, folder, uid, raw)
}

/// 复用调用方连接的原文缓存（批量同步用）。
pub fn cache_raw_body_on(
    conn: &Connection,
    account_id: &str,
    folder: &str,
    uid: u32,
    raw: &[u8],
) -> Result<(), String> {
    if raw.is_empty() || raw.len() > RAW_CACHE_LIMIT {
        return Ok(());
    }
    let payload = if encryption_enabled() {
        super::crypto::encrypt_bytes(account_id, raw)?
    } else {
        raw.to_vec()
    };
    // 必须是 UPSERT 而不是 UPDATE：第一次打开邮件时正文行还没写（upsert_body 在解析之后），
    // 用 UPDATE 会静默命中 0 行，原文缓存永远存不进去——附件依旧每次重新下载。
    conn.execute(
        // 只写 raw 这一列：正文（html / text）与 body_fetched 由 `upsert_body` 负责，
        // 这里既不能把它们清空，也不能谎报「正文已取」。
        // 参数顺序必须和列顺序一一对应：以前把 payload 放在第一位，
        // 整段加密原文就被写进了 account_id，(account_id, folder, uid) 永远撞不上已有行，
        // 于是每次缓存都新插一行垃圾（库里堆出几千行、几个 G），而真正的 raw 一列没存进去。
        r#"INSERT INTO message_bodies (account_id, folder, uid, raw)
           VALUES (?1,?2,?3,?4)
           ON CONFLICT(account_id, folder, uid) DO UPDATE SET raw=excluded.raw"#,
        params![account_id, folder, uid, payload],
    )
    .map_err(|e| format!("缓存邮件原文失败: {e}"))?;
    Ok(())
}

/// 取缓存的原文；没有（或解密失败）返回 None，调用方回退到「连服务器重新拉」。
pub fn cached_raw_body(account_id: &str, folder: &str, uid: u32) -> Option<Vec<u8>> {
    let conn = open().ok()?;
    let payload: Option<Vec<u8>> = conn
        .query_row(
            "SELECT raw FROM message_bodies WHERE account_id=?1 AND folder=?2 AND uid=?3",
            params![account_id, folder, uid],
            |row| row.get::<_, Option<Vec<u8>>>(0),
        )
        .optional()
        .ok()?
        .flatten();
    let bytes = payload?;
    if bytes.is_empty() {
        return None;
    }
    // 历史 / 未加密的缓存是裸字节：前缀不匹配时原样返回。
    super::crypto::decrypt_bytes(account_id, &bytes).ok()
}

/// 本地加密开关的进程内缓存。默认开启——「默认安全」比「默认好用」更该是默认值。
/// 用 RwLock 而不是 OnceCell：切换库 / 恢复备份后要能把缓存清掉。
static ENCRYPT_CACHE: once_cell::sync::Lazy<std::sync::RwLock<Option<bool>>> =
    once_cell::sync::Lazy::new(|| std::sync::RwLock::new(None));

fn encrypt_flag() -> bool {
    if let Ok(guard) = ENCRYPT_CACHE.read() {
        if let Some(value) = *guard {
            return value;
        }
    }
    let value = open()
        .ok()
        .and_then(|conn| {
            conn.query_row("SELECT value FROM meta WHERE key='encrypt_local'", [], |r| {
                r.get::<_, String>(0)
            })
            .ok()
        })
        // 没有这条设置（老库）时按默认开启。
        .map(|v| v != "0")
        .unwrap_or(true);
    if let Ok(mut guard) = ENCRYPT_CACHE.write() {
        *guard = Some(value);
    }
    value
}

pub fn encryption_enabled() -> bool {
    encrypt_flag()
}

/// 当前库的 schema 版本（诊断报告用）。
pub fn schema_version() -> i32 {
    open()
        .ok()
        .and_then(|c| c.query_row("PRAGMA user_version", [], |r| r.get(0)).ok())
        .unwrap_or(0)
}

pub fn set_encryption_enabled(enabled: bool) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('encrypt_local', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![if enabled { "1" } else { "0" }],
    )
    .map_err(|e| format!("保存加密设置失败: {e}"))?;
    if let Ok(mut guard) = ENCRYPT_CACHE.write() {
        *guard = Some(enabled);
    }
    Ok(())
}

/// 缓存命中时返回 (html, text, 附件 JSON, 是否已经取过正文)。
pub fn get_body(
    account_id: &str,
    folder: &str,
    uid: u32,
) -> Result<Option<(Option<String>, String, String, bool)>, String> {
    let conn = open()?;
    let raw: Option<(Option<String>, String, String, bool)> = conn
        .query_row(
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
        .map_err(|e| e.to_string())?;

    let Some((html, text, attachments, fetched)) = raw else {
        return Ok(None);
    };

    // 解密。历史明文（没有 enc:v1: 前缀）原样返回，老数据不需要迁移。
    // 解密失败（换机器、密钥丢失）时给一句人话，而不是把 base64 甩到界面上。
    let unseal = |value: &str| -> String {
        match super::crypto::decrypt(account_id, value) {
            Ok(Some(plain)) => plain,
            Ok(None) => value.to_string(),
            Err(_) => "⚠ 这封邮件的内容无法解密（加密密钥已变更或数据来自另一台设备）".to_string(),
        }
    };

    let html = html.map(|value| unseal(&value)).filter(|value| !value.is_empty());
    Ok(Some((html, unseal(&text), unseal(&attachments), fetched)))
}

/// 只取正文纯文本（规则匹配用）。
///
/// 规则只看 `text`，但 `get_body` 会连 HTML 和附件清单一起读、一起解密：
/// 对几万封已有邮件跑规则时，多解出来的那一大坨 HTML 才是真正的瓶颈。
pub fn get_body_text(account_id: &str, folder: &str, uid: u32) -> Result<String, String> {
    let conn = open()?;
    let text: Option<String> = conn
        .query_row(
            "SELECT text FROM message_bodies WHERE account_id=?1 AND folder=?2 AND uid=?3",
            params![account_id, folder, uid],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(text) = text else {
        return Ok(String::new());
    };
    match super::crypto::decrypt(account_id, &text) {
        Ok(Some(plain)) => Ok(plain),
        Ok(None) => Ok(text),
        Err(_) => Ok(String::new()),
    }
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

/// 一个词的 LIKE 片段（多列之间 OR），同时给出对应的参数。
///
/// 用户不会手抄完整邮箱：`张三`、`zhangsan`、`zhang`、`zhangsan@x.com` 都应该命中。
/// FTS 的 `from_addr` 列只有地址、且按 token 匹配，中文名与「地址前缀」都会落空，
/// 所以发件人 / 收件人 / 主题改用 LIKE 子串（多词之间是 AND，每词在任一列出现即可）。
/// 代价是全表扫描，但这几列都很短、本地邮件量在万到十万级，实测可接受；
/// 正文仍然走 FTS（正文可能有几十 MB，LIKE 才是真的慢）。
fn like_terms(cols: &[&str], text: &str) -> (Vec<String>, Vec<String>) {
    let mut fragments = Vec::new();
    let mut patterns = Vec::new();
    for term in text.split_whitespace() {
        let pattern = format!("%{}%", escape_like(term));
        let ors = cols
            .iter()
            .map(|col| format!("{col} LIKE ? ESCAPE '\\'"))
            .collect::<Vec<_>>()
            .join(" OR ");
        fragments.push(format!("({ors})"));
        for _ in cols {
            patterns.push(pattern.clone());
        }
    }
    (fragments, patterns)
}

/// 「主搜索词 或 通讯录展开出来的地址」：任一组命中即可，组内多个词仍是 AND。
fn push_like_with_alternatives(
    sql: &mut String,
    args: &mut Vec<Box<dyn rusqlite::ToSql>>,
    cols: &[&str],
    text: &str,
    alternatives: &[String],
) {
    if cols.is_empty() {
        return;
    }
    let mut groups: Vec<String> = Vec::new();
    let mut patterns: Vec<String> = Vec::new();
    let (fragments, terms) = like_terms(cols, text);
    if !fragments.is_empty() {
        groups.push(fragments.join(" AND "));
        patterns.extend(terms);
    }
    for alt in alternatives {
        let (fragments, terms) = like_terms(cols, alt.trim());
        if fragments.is_empty() {
            continue;
        }
        groups.push(fragments.join(" AND "));
        patterns.extend(terms);
    }
    if groups.is_empty() {
        return;
    }
    sql.push_str(&format!(" AND ({})", groups.join(" OR ")));
    for pattern in patterns {
        args.push(Box::new(pattern));
    }
}

/// FTS5 查询串：用户输入先用 jieba 切词（与写入侧同一套分词），
/// 每个词加双引号（避免 `AND`/`OR`/`*` 被当成语法），词间 AND。
/// 最后一个纯 ASCII 词加 `*`，让 `invo` 能命中 `invoice`。
fn fts_query(text: &str) -> String {
    fts_query_in(text, None)
}

/// 带列过滤的 FTS 查询：`body:"词"`。FTS5 的列过滤是写在 MATCH 表达式里的，
/// 不能写成 `messages_fts.body MATCH ?`（那会报 `no such column`）。
fn fts_query_in(text: &str, column: Option<&str>) -> String {
    let segmented = segment(text);
    let terms: Vec<&str> = segmented.split_whitespace().collect();
    if terms.is_empty() {
        return String::new();
    }
    let prefix = column.map(|name| format!("{name}:")).unwrap_or_default();
    let last = terms.len() - 1;
    terms
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let escaped = w.replace('"', "\"\"");
            let ascii_word = w.chars().all(|c| c.is_ascii_alphanumeric());
            if i == last && ascii_word && w.len() >= 2 {
                format!("{prefix}\"{escaped}\"*")
            } else {
                format!("{prefix}\"{escaped}\"")
            }
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

pub fn search(query: &SearchQuery) -> Result<Vec<MessageSummary>, String> {
    let limit = query.limit.unwrap_or(200).min(2000);
    let offset = query.offset.unwrap_or(0);
    let field = query.field.as_deref().unwrap_or("all");
    let text = query.text.trim();
    // 发件人 / 收件人 / 主题按子串匹配（见 push_field_like），其余走 FTS。
    let field_like = match field {
        "from" => Some(&["m.from_addr", "m.from_name"][..]),
        "to" => Some(&["m.to_json", "m.cc_json"][..]),
        "subject" => Some(&["m.subject"][..]),
        _ => None,
    };
    let use_fts = fts_available() && !text.is_empty() && field_like.is_none();

    let mut sql = String::from("SELECT ");
    for (i, col) in MSG_COLS.split(',').map(|s| s.trim()).enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("m.{col}"));
    }
    sql.push_str(" FROM messages m");
    // 不再 JOIN messages_fts：MATCH 只能出现在「直接对着 FTS 表」的 WHERE 里，
    // 一旦和别的条件用 OR 连起来（例如通讯录候选地址那条 LIKE 分支）就会报
    // `unable to use function MATCH in the requested context`。放到子查询里过滤即可。
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
            // 按「角色」跨文件夹筛选：收件箱 / 已发送 / 草稿 / 垃圾箱 / 存档…
            // 一个账户可能有多个同角色文件夹（例如自定义的「存档/2024」），
            // 所以按 role 子查询而不是写死名字。
            // 例外：`sent` 与 `drafts` 都要求「这是我自己发出的」，
            // 否则归档里被误标成已发送的邮件也会混进来。
            "inbox" | "sent" | "drafts" | "trash" | "junk" | "archive" => {
                sql.push_str(
                    " AND m.folder IN (SELECT name FROM folders WHERE role = ?)",
                );
                args.push(Box::new(scope.to_string()));
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
        // FTS 命中词，外加「通讯录里同名联系人对应的邮箱地址」——
        // 前端把候选地址放在 textAlternatives 里传进来（见 mailStore.loadMessages）。
        let expr = fts_query(&query.text);
        let mut groups: Vec<String> = Vec::new();
        let mut patterns: Vec<String> = Vec::new();
        if !expr.is_empty() {
            // 按正文搜时用列过滤语法 `body:"词"`；其余字段走整表 FTS。
            // 匹配条件写进子查询：这样 OR 的另一半（候选地址的 LIKE）不会把
            // MATCH 拖进「不支持的上下文」。
            let needle = if field == "body" {
                fts_query_in(&query.text, Some("body"))
            } else {
                expr
            };
            groups.push(
                "m.rowid IN (SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?)"
                    .to_string(),
            );
            patterns.push(needle);
        }
        if field != "body" {
            let alt_cols = ["m.subject", "m.from_addr", "m.from_name", "m.to_json"];
            let mut alt_groups: Vec<String> = Vec::new();
            let mut alt_patterns: Vec<String> = Vec::new();
            for alt in &query.text_alternatives {
                let (fragments, terms) = like_terms(&alt_cols, alt.trim());
                if fragments.is_empty() {
                    continue;
                }
                alt_groups.push(fragments.join(" AND "));
                alt_patterns.extend(terms);
            }
            if !alt_groups.is_empty() {
                groups.push(format!("({})", alt_groups.join(" OR ")));
                patterns.extend(alt_patterns);
            }
        }
        if !groups.is_empty() {
            sql.push_str(&format!(" AND ({})", groups.join(" OR ")));
            for pattern in patterns {
                args.push(Box::new(pattern));
            }
        }
    } else if let Some(cols) = field_like {
        push_like_with_alternatives(&mut sql, &mut args, cols, text, &query.text_alternatives);
    } else if !text.is_empty() {
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

// ---------------- 收/发件人名单 ----------------

pub fn list_sender_rules() -> Result<Vec<SenderRule>, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare("SELECT id, kind, pattern, note, created_at FROM sender_rules ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SenderRule {
                id: row.get(0)?,
                kind: row.get(1)?,
                pattern: row.get(2)?,
                note: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn upsert_sender_rule(rule: &SenderRule) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "INSERT INTO sender_rules (id, kind, pattern, note, created_at) VALUES (?1,?2,?3,?4,?5)
         ON CONFLICT(id) DO UPDATE SET kind=excluded.kind, pattern=excluded.pattern, note=excluded.note",
        params![
            rule.id,
            rule.kind,
            rule.pattern,
            rule.note,
            if rule.created_at == 0 {
                chrono::Utc::now().timestamp()
            } else {
                rule.created_at
            }
        ],
    )
    .map_err(|e| format!("保存名单失败: {e}"))?;
    Ok(())
}

pub fn delete_sender_rule(id: &str) -> Result<(), String> {
    let conn = open()?;
    conn.execute("DELETE FROM sender_rules WHERE id=?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------- 模板 ----------------

pub fn list_templates() -> Result<Vec<MailTemplate>, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare("SELECT id, name, subject, html, text, created_at FROM templates ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(MailTemplate {
                id: row.get(0)?,
                name: row.get(1)?,
                subject: row.get(2)?,
                html: row.get(3)?,
                text: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn upsert_template(item: &MailTemplate) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "INSERT INTO templates (id, name, subject, html, text, created_at) VALUES (?1,?2,?3,?4,?5,?6)
         ON CONFLICT(id) DO UPDATE SET name=excluded.name, subject=excluded.subject,
              html=excluded.html, text=excluded.text",
        params![
            item.id,
            item.name,
            item.subject,
            item.html,
            item.text,
            if item.created_at == 0 {
                chrono::Utc::now().timestamp()
            } else {
                item.created_at
            }
        ],
    )
    .map_err(|e| format!("保存模板失败: {e}"))?;
    Ok(())
}

pub fn delete_template(id: &str) -> Result<(), String> {
    let conn = open()?;
    conn.execute("DELETE FROM templates WHERE id=?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------- 待发队列 ----------------

pub fn enqueue_outbox(item: &OutboxItem, payload_json: &str) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "INSERT OR REPLACE INTO outbox (id, account_id, subject, to_addrs, payload_json, due_at, created_at, sent, staged)
         VALUES (?1,?2,?3,?4,?5,?6,?7,0,0)",
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
        .prepare(
            "SELECT id, account_id, subject, to_addrs, due_at, created_at, attempts,
                    next_attempt_at, last_error, staged
             FROM outbox WHERE sent=0 ORDER BY due_at ASC",
        )
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
                attempts: row.get(6)?,
                next_attempt_at: row.get(7)?,
                last_error: row.get(8)?,
                staged: row.get::<_, i32>(9)? != 0,
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
                 WHERE sent=0 AND staged=0 AND due_at<=?1 AND claimed_at<=?2
                   AND next_attempt_at<=?1
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

/// 释放认领：发送失败后立刻允许重试（新代码走 `note_outbox_failure` 的退避，
/// 这个保留给「用户手动点重试」的语义）。
#[allow(dead_code)]
pub fn release_outbox(id: &str) -> Result<(), String> {
    let conn = open()?;
    conn.execute("UPDATE outbox SET claimed_at=0 WHERE id=?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 退避间隔（秒）：1 分钟起，每次翻倍，封顶 1 小时。
///
/// 之前失败后把 `claimed_at` 归零，下一轮轮询（90 秒）立刻重撞一次。
/// 网络断了的时候这会变成每 90 秒一次的固定尖峰，而且服务器把同一个连接
/// 反复打回，IP 还可能被临时封禁。指数退避是 SMTP 客户端的基本礼貌。
pub fn backoff_seconds(attempts: u32) -> i64 {
    // 60s 起步，每次翻倍，上限 1 小时。
    (60i64 << attempts.min(6)).min(3600)
}

/// 记录一次发送失败：次数 +1，并把下次可尝试时间推到退避之后。
pub fn note_outbox_failure(id: &str, error: &str) -> Result<i64, String> {
    let conn = open()?;
    let attempts: i64 = conn
        .query_row("SELECT attempts FROM outbox WHERE id=?1", params![id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|e| e.to_string())?
        .unwrap_or(0);
    let next_attempts = attempts + 1;
    let delay = backoff_seconds(next_attempts as u32);
    let next_at = chrono::Utc::now().timestamp() + delay;
    conn.execute(
        "UPDATE outbox SET claimed_at=0, attempts=?1, next_attempt_at=?2, last_error=?3 WHERE id=?4",
        params![next_attempts, next_at, error, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(delay)
}

/// 暂存一条始终发不出去的邮件：不再自动重试，改由发件箱展示给用户处置。
pub fn stage_outbox(id: &str, error: &str) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "UPDATE outbox SET staged=1, claimed_at=0, last_error=?1, attempts=attempts+1 WHERE id=?2",
        params![error, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 重新入队（发件箱里点「重试」）。退避与计数一起清零，否则用户刚点重试又要等一小时。
pub fn requeue_outbox(id: &str) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        "UPDATE outbox SET staged=0, claimed_at=0, attempts=0, next_attempt_at=0 WHERE id=?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 一条待发邮件的原始请求（发件箱里点开查看 / 撤回时用）。
pub fn outbox_payload(id: &str) -> Result<Option<String>, String> {
    let conn = open()?;
    conn.query_row(
        "SELECT payload_json FROM outbox WHERE id=?1",
        params![id],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| e.to_string())
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
    let mut settings: MailSettings = raw
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    // 一次性把老默认字号 15 抬到 16（真实反馈：正文偏小）。
    // 只改动「仍是老默认值」的设置——用户自己调过的字号保持原样。
    let migrated: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key='settings_reading_size_v1'",
            [],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten();
    if migrated.is_none() {
        if settings.reading_size == 15 {
            settings.reading_size = 16;
            if let Ok(json) = serde_json::to_string(&settings) {
                let _ = conn.execute(
                    "INSERT INTO meta (key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                    params![META_SETTINGS_KEY, json],
                );
            }
        }
        let _ = conn.execute(
            "INSERT INTO meta (key, value) VALUES ('settings_reading_size_v1', '1')
             ON CONFLICT(key) DO UPDATE SET value='1'",
            [],
        );
    }
    settings
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
    fn fts_query_in_prefixes_every_term_with_the_column() {
        // 正文搜索用列过滤语法：每个词都要带上列名，否则会退化成整表匹配。
        assert_eq!(fts_query_in("invo", Some("body")), "body:\"invo\"*");
        assert_eq!(
            fts_query_in("invoice 报告", Some("body")),
            "body:\"invoice\" AND body:\"报告\""
        );
        // 不传列名时与 fts_query 一致。
        assert_eq!(fts_query_in("invo", None), fts_query("invo"));
    }

    #[test]
    fn escape_like_escapes_wildcards() {
        assert_eq!(escape_like("50%_off"), "50\\%\\_off");
    }
}


