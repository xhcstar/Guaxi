//! 认知子系统持久层：`repos/cognition.db` 的连接、schema 与迁移。
//!
//! 设计约定（与 kgraph 保持一致）：
//! - 数据库放在 exe 同级 `repos/`，与 `kg_index.db` / `knowledge_bases.db` 同目录。
//! - WAL + NORMAL，读写并发下不阻塞 UI。
//! - **真相源仍在文件系统**：percept 的原文、profile 的正文、brief 的成品都落 markdown；
//!   本库只存派生索引、状态机与账本，删库后可由 `cognition_rebuild` 重建大部分内容。
//! - 迁移用 `cog_settings.schema_version` 单调推进，每一步幂等。

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

/// schema 版本。新增表/列时 +1 并在 `migrate` 里补一段。
pub const COG_SCHEMA_VERSION: i64 = 7;

/// 建库串行化：多个后台循环同时首次开库时避免并发建表冲突。
static INIT_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
static INITIALIZED: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));

/// 解析 cognition.db 路径。
pub fn cog_db_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe
        .parent()
        .ok_or_else(|| "无法获取可执行文件所在目录".to_string())?
        .to_path_buf();
    let repos = dir.join("repos");
    if !repos.exists() {
        fs::create_dir_all(&repos).map_err(|e| e.to_string())?;
    }
    Ok(repos.join("cognition.db"))
}

/// 认知子系统的数据目录（profile.md / 简报 / 审计导出）。
pub fn cog_data_dir() -> Result<PathBuf, String> {
    let cfg = crate::commands::settingview::get_config_path();
    let base = cfg
        .parent()
        .ok_or_else(|| "无法定位 config 目录".to_string())?
        .join("cognition");
    if !base.exists() {
        fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    }
    Ok(base)
}

/// 打开连接并套用 PRAGMA。不建表——建表走 `ensure_db`。
pub fn open_db() -> Result<Connection, String> {
    let path = cog_db_path()?;
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;\
         PRAGMA synchronous = NORMAL;\
         PRAGMA temp_store = MEMORY;\
         PRAGMA busy_timeout = 5000;\
         PRAGMA cache_size = -32768;\
         PRAGMA foreign_keys = OFF;",
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

/// 打开连接并保证 schema 就位（进程内只真正建表一次，之后走快路径）。
pub fn ensure_db() -> Result<Connection, String> {
    let conn = open_db()?;
    {
        let mut done = INITIALIZED.lock();
        if *done {
            return Ok(conn);
        }
        let _guard = INIT_LOCK.lock();
        init_schema(&conn)?;
        migrate(&conn)?;
        *done = true;
    }
    Ok(conn)
}

/// 读取一个设置项。
pub fn get_setting(key: &str) -> Option<String> {
    let conn = ensure_db().ok()?;
    conn.query_row(
        "SELECT value FROM cog_settings WHERE key=?1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
}

/// 写入一个设置项。
pub fn set_setting(key: &str, value: &str) -> Result<(), String> {
    let conn = ensure_db()?;
    conn.execute(
        "INSERT INTO cog_settings(key,value) VALUES(?1,?2) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 按前缀批量读取设置，一次查询。
///
/// 给「一次要用到很多个设置」的调用方用（例如工具等级授权）。逐项 `get_setting`
/// 每次都新开连接并重跑 PRAGMA，几十项就是几十次连接开销。
pub fn settings_with_prefix(prefix: &str) -> Vec<(String, String)> {
    let Ok(conn) = ensure_db() else {
        return Vec::new();
    };
    let Ok(mut stmt) = conn.prepare("SELECT key, value FROM cog_settings WHERE key LIKE ?1") else {
        return Vec::new();
    };
    let pattern = format!("{prefix}%");
    let Ok(rows) = stmt.query_map(params![pattern], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    }) else {
        return Vec::new();
    };
    rows.filter_map(|x| x.ok()).collect()
}

/// 读取布尔设置（缺省值由调用方给）。
pub fn get_bool(key: &str, default: bool) -> bool {
    match get_setting(key).as_deref() {
        Some("1") | Some("true") => true,
        Some("0") | Some("false") => false,
        _ => default,
    }
}

/// 读取整数设置。
pub fn get_i64(key: &str, default: i64) -> i64 {
    get_setting(key)
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(default)
}

/// 读取浮点设置。
pub fn get_f64(key: &str, default: f64) -> f64 {
    get_setting(key)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(default)
}

/// f32 向量 → 小端 BLOB（与 kgraph.kg_embeddings 的编码保持一致，便于互算）。
pub fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

/// 小端 BLOB → f32 向量。长度不是 4 的倍数时返回空（视为损坏，调用方跳过）。
pub fn blob_to_vec(b: &[u8]) -> Vec<f32> {
    if b.is_empty() || b.len() % 4 != 0 {
        return Vec::new();
    }
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// 当前时刻的 RFC3339 本地时间字符串（全库统一时间格式）。
pub fn now_iso() -> String {
    chrono::Local::now().to_rfc3339()
}

/// 今天的 `YYYY-MM-DD`（预算账本按日聚合）。
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS cog_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS percepts (
    id            TEXT PRIMARY KEY,
    source_kind   TEXT NOT NULL,
    source_id     TEXT,
    uri           TEXT,
    title         TEXT,
    text          TEXT NOT NULL,
    text_hash     TEXT NOT NULL,
    captured_at   TEXT NOT NULL,
    novelty       REAL NOT NULL DEFAULT -1,
    salience      REAL NOT NULL DEFAULT 0,
    trust         REAL NOT NULL DEFAULT 0.5,
    kb_name       TEXT,
    anchor_node_id TEXT,
    anchor_score  REAL NOT NULL DEFAULT 0,
    state         TEXT NOT NULL DEFAULT 'new',
    topics        TEXT,
    meta          TEXT,
    vec           BLOB
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_percepts_hash ON percepts(text_hash);
CREATE INDEX IF NOT EXISTS idx_percepts_state ON percepts(state, captured_at DESC);
CREATE INDEX IF NOT EXISTS idx_percepts_kind ON percepts(source_kind, captured_at DESC);
"#,
    )
    .map_err(|e| format!("建 percepts 表失败: {e}"))?;

    conn.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS episodes (
    id           TEXT PRIMARY KEY,
    kind         TEXT NOT NULL,
    session_id   TEXT,
    started_at   TEXT NOT NULL,
    ended_at     TEXT,
    model        TEXT,
    question     TEXT,
    answer       TEXT,
    kb_name      TEXT,
    recall_used  INTEGER NOT NULL DEFAULT 0,
    outcome      TEXT,
    tokens_out   INTEGER NOT NULL DEFAULT 0,
    meta         TEXT,
    vec          BLOB
);
CREATE INDEX IF NOT EXISTS idx_episodes_time ON episodes(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_episodes_kind ON episodes(kind, started_at DESC);

CREATE TABLE IF NOT EXISTS episode_steps (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    episode_id  TEXT NOT NULL,
    seq         INTEGER NOT NULL,
    step_type   TEXT NOT NULL,
    name        TEXT,
    payload     TEXT,
    at          TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_steps_ep ON episode_steps(episode_id, seq);

CREATE TABLE IF NOT EXISTS citations (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    episode_id  TEXT NOT NULL,
    ref_kind    TEXT NOT NULL,
    ref_id      TEXT NOT NULL,
    kb_name     TEXT,
    label       TEXT,
    channel     TEXT NOT NULL,
    rank        INTEGER NOT NULL,
    score       REAL NOT NULL,
    used        INTEGER NOT NULL DEFAULT 0,
    clicked     INTEGER NOT NULL DEFAULT 0,
    at          TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_citations_ep ON citations(episode_id, rank);
CREATE INDEX IF NOT EXISTS idx_citations_ref ON citations(ref_kind, ref_id);
"#,
    )
    .map_err(|e| format!("建 episodes 表失败: {e}"))?;

    conn.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS beliefs (
    id              TEXT PRIMARY KEY,
    kb_name         TEXT NOT NULL,
    statement       TEXT NOT NULL,
    statement_hash  TEXT NOT NULL,
    confidence      REAL NOT NULL DEFAULT 0.5,
    source_node_id  TEXT,
    source_rel_path TEXT,
    source_line     INTEGER NOT NULL DEFAULT 0,
    topics          TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'active',
    vec             BLOB,
    meta            TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_beliefs_hash ON beliefs(kb_name, statement_hash);
CREATE INDEX IF NOT EXISTS idx_beliefs_kb ON beliefs(kb_name, status);

CREATE TABLE IF NOT EXISTS belief_evidence (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    belief_id  TEXT NOT NULL,
    ref_kind   TEXT NOT NULL,
    ref_id     TEXT NOT NULL,
    stance     TEXT NOT NULL,
    score      REAL NOT NULL,
    note       TEXT,
    at         TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_evidence_uniq
    ON belief_evidence(belief_id, ref_kind, ref_id, stance);
CREATE INDEX IF NOT EXISTS idx_evidence_belief ON belief_evidence(belief_id);

CREATE TABLE IF NOT EXISTS interests (
    topic     TEXT PRIMARY KEY,
    weight    REAL NOT NULL DEFAULT 0,
    source    TEXT NOT NULL DEFAULT 'derived',
    hits      INTEGER NOT NULL DEFAULT 0,
    last_seen TEXT,
    vec       BLOB
);
"#,
    )
    .map_err(|e| format!("建 beliefs 表失败: {e}"))?;

    conn.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS nudges (
    id           TEXT PRIMARY KEY,
    kind         TEXT NOT NULL,
    title        TEXT NOT NULL,
    body         TEXT NOT NULL,
    why          TEXT NOT NULL,
    refs         TEXT NOT NULL,
    kb_name      TEXT,
    topic        TEXT,
    utility      REAL NOT NULL,
    cost         REAL NOT NULL DEFAULT 1,
    channel      TEXT NOT NULL DEFAULT 'board',
    created_at   TEXT NOT NULL,
    valid_until  TEXT,
    state        TEXT NOT NULL DEFAULT 'pending',
    delivered_at TEXT,
    decided_at   TEXT,
    dedup_key    TEXT NOT NULL,
    features     TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_nudges_dedup ON nudges(dedup_key);
CREATE INDEX IF NOT EXISTS idx_nudges_state ON nudges(state, utility DESC);
CREATE INDEX IF NOT EXISTS idx_nudges_created ON nudges(created_at DESC);

CREATE TABLE IF NOT EXISTS nudge_feedback (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    nudge_id TEXT NOT NULL,
    kind     TEXT NOT NULL,
    label    INTEGER NOT NULL,
    features TEXT NOT NULL,
    at       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_feedback_at ON nudge_feedback(at DESC);

CREATE TABLE IF NOT EXISTS policy (
    name       TEXT PRIMARY KEY,
    version    INTEGER NOT NULL DEFAULT 1,
    weights    TEXT NOT NULL,
    bias       REAL NOT NULL DEFAULT 0,
    trained_at TEXT,
    samples    INTEGER NOT NULL DEFAULT 0,
    metric     REAL NOT NULL DEFAULT 0
);
"#,
    )
    .map_err(|e| format!("建 nudges 表失败: {e}"))?;

    conn.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS delivery_log (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    channel  TEXT NOT NULL,
    at       TEXT NOT NULL,
    nudge_id TEXT
);
CREATE INDEX IF NOT EXISTS idx_delivery_time ON delivery_log(channel, at DESC);

CREATE TABLE IF NOT EXISTS token_ledger (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    day     TEXT NOT NULL,
    purpose TEXT NOT NULL,
    tokens  INTEGER NOT NULL,
    at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ledger_day ON token_ledger(day, purpose);

CREATE TABLE IF NOT EXISTS tool_audit (
    id     INTEGER PRIMARY KEY AUTOINCREMENT,
    at     TEXT NOT NULL,
    caller TEXT NOT NULL,
    tool   TEXT NOT NULL,
    tier   TEXT NOT NULL,
    args   TEXT,
    ok     INTEGER NOT NULL,
    ms     INTEGER NOT NULL,
    error  TEXT
);
CREATE INDEX IF NOT EXISTS idx_audit_at ON tool_audit(at DESC);

CREATE TABLE IF NOT EXISTS briefs (
    id         TEXT PRIMARY KEY,
    day        TEXT NOT NULL,
    kind       TEXT NOT NULL,
    path       TEXT,
    markdown   TEXT NOT NULL,
    created_at TEXT NOT NULL,
    opened     INTEGER NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_briefs_day ON briefs(day, kind);

CREATE TABLE IF NOT EXISTS review_state (
    node_id       TEXT PRIMARY KEY,
    kb_name       TEXT NOT NULL,
    label         TEXT NOT NULL,
    rel_path      TEXT,
    line          INTEGER NOT NULL DEFAULT 0,
    ease          REAL NOT NULL DEFAULT 2.5,
    interval_days INTEGER NOT NULL DEFAULT 1,
    due_date      TEXT NOT NULL,
    reps          INTEGER NOT NULL DEFAULT 0,
    lapses        INTEGER NOT NULL DEFAULT 0,
    last_review   TEXT
);
CREATE INDEX IF NOT EXISTS idx_review_due ON review_state(due_date, kb_name);
"#,
    )
    .map_err(|e| format!("建账本表失败: {e}"))?;

    // 评测闩：自调参数（检索权重 / 打断阈值 / 提示词）必须在冻结样本上验过才允许留下。
    conn.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS eval_items (
    id         TEXT PRIMARY KEY,
    kb_name    TEXT NOT NULL,
    question   TEXT NOT NULL,
    gold_ref   TEXT NOT NULL,
    gold_label TEXT,
    created_at TEXT NOT NULL,
    frozen     INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS idx_eval_kb ON eval_items(kb_name);

CREATE TABLE IF NOT EXISTS eval_runs (
    id         TEXT PRIMARY KEY,
    at         TEXT NOT NULL,
    target     TEXT NOT NULL,
    version    INTEGER NOT NULL DEFAULT 0,
    params     TEXT NOT NULL,
    n          INTEGER NOT NULL DEFAULT 0,
    hit_at_1   REAL NOT NULL DEFAULT 0,
    hit_at_5   REAL NOT NULL DEFAULT 0,
    mrr        REAL NOT NULL DEFAULT 0,
    accepted   INTEGER NOT NULL DEFAULT 0,
    note       TEXT
);
CREATE INDEX IF NOT EXISTS idx_eval_runs_at ON eval_runs(target, at DESC);
"#,
    )
    .map_err(|e| format!("建 eval 表失败: {e}"))?;
    Ok(())
}

/// 迁移：按 `cog_settings.schema_version` 推进，每步幂等。
fn migrate(conn: &Connection) -> Result<(), String> {
    let current: i64 = conn
        .query_row(
            "SELECT value FROM cog_settings WHERE key='schema_version'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(0);

    if current >= COG_SCHEMA_VERSION {
        return Ok(());
    }
    info!(
        "[cognition] 迁移 schema {} → {}",
        current, COG_SCHEMA_VERSION
    );

    // v1 → v2：percepts 增加 lang / 归档原因；episodes 增加 latency。
    if current < 2 {
        add_column(conn, "percepts", "lang", "TEXT");
        add_column(conn, "percepts", "archive_reason", "TEXT");
        add_column(conn, "episodes", "latency_ms", "INTEGER NOT NULL DEFAULT 0");
    }
    // v2 → v3：nudges 增加投递时的策略分与模型版本，便于事后归因。
    if current < 3 {
        add_column(conn, "nudges", "policy_score", "REAL NOT NULL DEFAULT 0");
        add_column(conn, "nudges", "policy_version", "INTEGER NOT NULL DEFAULT 0");
    }
    // v3 → v4：活动总线。只新增表与索引，幂等；旧库首次启动时自动补上。
    if current < 4 {
        conn.execute_batch(
            r#"
CREATE TABLE IF NOT EXISTS activity (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    at          TEXT NOT NULL,
    app         TEXT NOT NULL,
    kind        TEXT NOT NULL,
    target_kind TEXT,
    target_id   TEXT,
    kb_name     TEXT,
    path        TEXT,
    line        INTEGER,
    duration_ms INTEGER,
    progress    REAL,
    session_id  TEXT,
    meta        TEXT
);
CREATE INDEX IF NOT EXISTS idx_activity_at ON activity(at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_activity_app ON activity(app, kind, at DESC);
CREATE INDEX IF NOT EXISTS idx_activity_target ON activity(target_id, at DESC);
"#,
        )
        .map_err(|e| format!("建 activity 表失败: {e}"))?;
    }
    // v4 → v5：长期记忆。手动块（memory.md）是用户可以改的那一半，本表是派生索引与墓碑。
    if current < 5 {
        conn.execute_batch(
            r#"
CREATE TABLE IF NOT EXISTS memories (
    id            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL,
    text          TEXT NOT NULL,
    text_hash     TEXT NOT NULL,
    confidence    REAL NOT NULL DEFAULT 0.6,
    status        TEXT NOT NULL DEFAULT 'active',
    source        TEXT NOT NULL DEFAULT 'derived',
    topics        TEXT NOT NULL DEFAULT '[]',
    refs          TEXT NOT NULL DEFAULT '[]',
    hits          INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    last_seen_at  TEXT,
    vec           BLOB
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_memories_hash ON memories(text_hash);
CREATE INDEX IF NOT EXISTS idx_memories_status ON memories(status, kind, updated_at DESC);
"#,
        )
        .map_err(|e| format!("建 memories 表失败: {e}"))?;
    }
    // v5 → v6：FSRS 排期字段 + 主题掌握度。旧 SM-2 队列保留，首次反馈时按新卡接管。
    if current < 6 {
        add_column(conn, "review_state", "stability", "REAL");
        add_column(conn, "review_state", "difficulty", "REAL");
        add_column(conn, "review_state", "fsrs_state", "TEXT");
        add_column(conn, "review_state", "elapsed_days", "INTEGER NOT NULL DEFAULT 0");
        add_column(conn, "review_state", "scheduled_days", "INTEGER NOT NULL DEFAULT 1");
        conn.execute_batch(
            r#"
CREATE TABLE IF NOT EXISTS topic_mastery (
    topic_key      TEXT PRIMARY KEY,
    topic          TEXT NOT NULL,
    source         TEXT NOT NULL,
    kb_name        TEXT,
    ref_id         TEXT,
    coverage       REAL NOT NULL DEFAULT 0,
    retention      REAL NOT NULL DEFAULT 0,
    engagement     REAL NOT NULL DEFAULT 0,
    mastery        REAL NOT NULL DEFAULT 0,
    confidence     REAL NOT NULL DEFAULT 0,
    sessions       INTEGER NOT NULL DEFAULT 0,
    finished       INTEGER NOT NULL DEFAULT 0,
    reps           INTEGER NOT NULL DEFAULT 0,
    lapses         INTEGER NOT NULL DEFAULT 0,
    last_touched_at TEXT,
    updated_at     TEXT NOT NULL,
    extra          TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_mastery_rank ON topic_mastery(mastery ASC, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_mastery_source ON topic_mastery(source, kb_name);
"#,
        )
        .map_err(|e| format!("建 topic_mastery 表失败: {e}"))?;
    }
    // v6 → v7：引证排序特征。召回时把特征快照写进引证，训练器才有样本可用。
    if current < 7 {
        add_column(conn, "citations", "features", "TEXT");
    }

    conn.execute(
        "INSERT INTO cog_settings(key,value) VALUES('schema_version',?1) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![COG_SCHEMA_VERSION.to_string()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 幂等加列：已存在时 SQLite 报 duplicate column，吞掉即可。
fn add_column(conn: &Connection, table: &str, column: &str, decl: &str) {
    let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {decl}");
    if let Err(e) = conn.execute(&sql, []) {
        let msg = e.to_string();
        if !msg.contains("duplicate column") {
            warn!("[cognition] 加列失败 {table}.{column}: {msg}");
        }
    }
}

/// 记一笔 token 消耗（预算护栏的账本）。
pub fn record_tokens(purpose: &str, tokens: i64) {
    if tokens <= 0 {
        return;
    }
    if let Ok(conn) = ensure_db() {
        let _ = conn.execute(
            "INSERT INTO token_ledger(day,purpose,tokens,at) VALUES(?1,?2,?3,?4)",
            params![today(), purpose, tokens, now_iso()],
        );
    }
}

/// 今日已花的 token（可按用途过滤）。
pub fn tokens_spent_today(purpose: Option<&str>) -> i64 {
    let Ok(conn) = ensure_db() else { return 0 };
    let res = match purpose {
        Some(p) => conn.query_row(
            "SELECT COALESCE(SUM(tokens),0) FROM token_ledger WHERE day=?1 AND purpose=?2",
            params![today(), p],
            |r| r.get::<_, i64>(0),
        ),
        None => conn.query_row(
            "SELECT COALESCE(SUM(tokens),0) FROM token_ledger WHERE day=?1",
            params![today()],
            |r| r.get::<_, i64>(0),
        ),
    };
    res.unwrap_or(0)
}

/// 记一条工具调用审计。所有经 MCP / Agent 触达应用能力的调用都必须落这里。
pub fn audit_tool(
    caller: &str,
    tool: &str,
    tier: &str,
    args: &str,
    ok: bool,
    ms: u128,
    error: Option<&str>,
) {
    if let Ok(conn) = ensure_db() {
        // args 只留前 2000 字符，避免把整份文件内容写进审计表。
        let args_trimmed: String = args.chars().take(2000).collect();
        let _ = conn.execute(
            "INSERT INTO tool_audit(at,caller,tool,tier,args,ok,ms,error) \
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                now_iso(),
                caller,
                tool,
                tier,
                args_trimmed,
                if ok { 1i64 } else { 0i64 },
                ms as i64,
                error
            ],
        );
    }
}

/// SHA256 十六进制（去重键统一走它）。
pub fn hash_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    format!("{:x}", h.finalize())
}
