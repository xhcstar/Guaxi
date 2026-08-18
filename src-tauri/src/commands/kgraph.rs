//! 知识图谱索引层(kgraph)
//!
//! 把分散在 .meta JSON 中的知识库结构与关系“固化”进一个独立的 SQLite 图索引库
//! (repos/kg_index.db),并在内存中以 petgraph 维护工作图,支撑分层 LOD、
//! 邻居查询、最短路、中心度、社区检测与向量检索。
//!
//! 分层职责:
//!   .md(真相源) → .meta JSON(可移植元数据) → kg_index.db(固化索引)
//!     → petgraph 内存图 + HNSW 向量索引(工作记忆) → kg_layout 缓存坐标 → 前端 3D

use once_cell::sync::Lazy;
use regex::Regex;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};

/// 图索引 schema 版本。结构变更时 +1,触发重建。
pub const KG_SCHEMA_VERSION: i64 = 1;

/// 解析 kg_index.db 路径(与 knowledge_bases.db 同目录 repos/)。
pub fn kg_db_path() -> Result<PathBuf, String> {
    let exe_path = env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "无法获取可执行文件所在目录".to_string())?
        .to_path_buf();

    let repo_dir = exe_dir.join("repos");
    if !repo_dir.exists() {
        fs::create_dir_all(&repo_dir).map_err(|e| e.to_string())?;
    }
    Ok(repo_dir.join("kg_index.db"))
}

/// 打开图索引连接并应用性能相关 PRAGMA。
pub fn open_kg_db() -> Result<Connection, String> {
    let path = kg_db_path()?;
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;
    // WAL 提升并发读写;NORMAL 同步在崩溃安全与吞吐间取平衡;较大 cache 提升 10 万级批量写入。
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;\
         PRAGMA synchronous = NORMAL;\
         PRAGMA temp_store = MEMORY;\
         PRAGMA cache_size = -65536;\
         PRAGMA foreign_keys = OFF;",
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

/// 建表(幂等)。所有表以 kb_name 维度区分多知识库。
pub fn init_kg_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS kg_build_state (
    kb_name         TEXT PRIMARY KEY,
    schema_version  INTEGER NOT NULL,
    built_at        TEXT,
    node_count      INTEGER NOT NULL DEFAULT 0,
    edge_count      INTEGER NOT NULL DEFAULT 0,
    embedding_model TEXT,
    embedding_dim   INTEGER NOT NULL DEFAULT 0,
    embedded_count  INTEGER NOT NULL DEFAULT 0,
    layout_version  INTEGER NOT NULL DEFAULT 0,
    community_level INTEGER NOT NULL DEFAULT 0,
    dirty           INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS kg_nodes (
    kb_name      TEXT NOT NULL,
    node_id      TEXT NOT NULL,
    node_type    TEXT NOT NULL,          -- knowledge-base|directory|file|section|paragraph|concept
    name         TEXT NOT NULL,
    rel_path     TEXT,
    abs_path     TEXT,
    file_path    TEXT,
    stable_id    TEXT,
    parent_id    TEXT,
    depth        INTEGER NOT NULL DEFAULT 0,
    start_line   INTEGER,
    end_line     INTEGER,
    level        INTEGER,
    content_hash TEXT,                   -- 内容指纹,用于 embedding 失效重算
    degree       INTEGER NOT NULL DEFAULT 0,
    pagerank     REAL NOT NULL DEFAULT 0,
    community_id INTEGER,
    created_at   TEXT,
    modified_at  TEXT,
    PRIMARY KEY (kb_name, node_id)
);
CREATE INDEX IF NOT EXISTS idx_kg_nodes_type      ON kg_nodes(kb_name, node_type);
CREATE INDEX IF NOT EXISTS idx_kg_nodes_stable    ON kg_nodes(kb_name, stable_id);
CREATE INDEX IF NOT EXISTS idx_kg_nodes_parent    ON kg_nodes(kb_name, parent_id);
CREATE INDEX IF NOT EXISTS idx_kg_nodes_rel       ON kg_nodes(kb_name, rel_path);
CREATE INDEX IF NOT EXISTS idx_kg_nodes_community ON kg_nodes(kb_name, community_id);

CREATE TABLE IF NOT EXISTS kg_edges (
    edge_id   INTEGER PRIMARY KEY AUTOINCREMENT,
    kb_name   TEXT NOT NULL,
    from_id   TEXT NOT NULL,
    to_id     TEXT NOT NULL,
    edge_type TEXT NOT NULL,             -- contains|next|link|mention|tag|related|prerequisite|...
    strength  REAL NOT NULL DEFAULT 0.5,
    source    TEXT,                      -- structural|wikilink|mdlink|tag|semantic|llm|manual
    src_file  TEXT,                      -- 产生该边的源文件 rel_path,便于增量清理
    UNIQUE (kb_name, from_id, to_id, edge_type)
);
CREATE INDEX IF NOT EXISTS idx_kg_edges_from     ON kg_edges(kb_name, from_id);
CREATE INDEX IF NOT EXISTS idx_kg_edges_to       ON kg_edges(kb_name, to_id);
CREATE INDEX IF NOT EXISTS idx_kg_edges_srcfile  ON kg_edges(kb_name, src_file);
CREATE INDEX IF NOT EXISTS idx_kg_edges_type     ON kg_edges(kb_name, edge_type);

CREATE TABLE IF NOT EXISTS kg_node_tags (
    kb_name TEXT NOT NULL,
    node_id TEXT NOT NULL,
    tag     TEXT NOT NULL,
    source  TEXT,                        -- manual|zeroshot|llm
    score   REAL NOT NULL DEFAULT 1.0,
    PRIMARY KEY (kb_name, node_id, tag)
);
CREATE INDEX IF NOT EXISTS idx_kg_tags_tag ON kg_node_tags(kb_name, tag);

CREATE TABLE IF NOT EXISTS kg_embeddings (
    kb_name      TEXT NOT NULL,
    node_id      TEXT NOT NULL,
    content_hash TEXT,
    dim          INTEGER NOT NULL DEFAULT 0,
    vec          BLOB,                    -- f32 小端序连续存储
    PRIMARY KEY (kb_name, node_id)
);

CREATE TABLE IF NOT EXISTS kg_layout (
    kb_name TEXT NOT NULL,
    node_id TEXT NOT NULL,
    view    TEXT NOT NULL DEFAULT 'relation',  -- relation|tag|time|importance
    x       REAL NOT NULL DEFAULT 0,
    y       REAL NOT NULL DEFAULT 0,
    z       REAL NOT NULL DEFAULT 0,
    PRIMARY KEY (kb_name, node_id, view)
);

CREATE TABLE IF NOT EXISTS kg_communities (
    kb_name       TEXT NOT NULL,
    community_id  INTEGER NOT NULL,
    level         INTEGER NOT NULL DEFAULT 0,
    label         TEXT,
    size          INTEGER NOT NULL DEFAULT 0,
    centroid_node TEXT,
    color         TEXT,
    PRIMARY KEY (kb_name, community_id, level)
);

CREATE TABLE IF NOT EXISTS kg_settings (
    key   TEXT PRIMARY KEY,
    value TEXT
);
"#,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 确保 schema 就绪并返回连接。
pub fn ensure_kg_db() -> Result<Connection, String> {
    let conn = open_kg_db()?;
    init_kg_schema(&conn)?;
    Ok(conn)
}

// ───────────────────────────── 关系抽取器 ─────────────────────────────
//
// 从 markdown 正文抽取三类显式关系信号:
//   1. [[wikilink|alias]] / [[file#heading]] / [[#heading]]  → link 边
//   2. [text](path.md) / [text](path.md#anchor)             → link 边(mdlink 来源)
//   3. #tag(行内,非标题)                                    → tag 分类维度
//
// 抽取阶段只产出“原始引用”(目标名/路径 + 锚点),不解析到具体 node_id。
// 因为解析需要全库节点表(文件名→节点、标题→节点),放在索引构建阶段(T3)统一完成。

pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    Wikilink,
    MdLink,
}

#[derive(Debug, Clone)]
pub struct RawRef {
    pub kind: RefKind,
    /// 链接目标:文件名/页面名 或 相对路径(去掉 '#' 之后的部分)。空表示同文件内锚点。
    pub target: String,
    /// '#' 之后的标题锚点文本(若有)。
    pub anchor: Option<String>,
    /// 引用所在的最近上级标题 stable_id(用于把边挂到 section 粒度)。
    pub from_section: Option<String>,
    /// 引用所在源行号。保留作行级关系归因(暂未消费)。
    #[allow(dead_code)]
    pub line: u32,
}

#[derive(Debug, Clone)]
pub struct TagHit {
    pub tag: String,
    pub from_section: Option<String>,
    /// 标签所在源行号。保留作行级归因(暂未消费)。
    #[allow(dead_code)]
    pub line: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ExtractedRefs {
    pub refs: Vec<RawRef>,
    pub tags: Vec<TagHit>,
}

static MD_LINK_RE: Lazy<Regex> =
    // 捕获:1=可选前导 '!'(图片),2=链接文字,3=链接目标
    Lazy::new(|| Regex::new(r"(!?)\[([^\]]*)\]\(([^)]+)\)").unwrap());

fn is_external_target(t: &str) -> bool {
    let lt = t.trim().to_ascii_lowercase();
    lt.starts_with("http://")
        || lt.starts_with("https://")
        || lt.starts_with("mailto:")
        || lt.starts_with("ftp://")
        || lt.starts_with("tel:")
        || lt.starts_with("data:")
}

/// 目标是否应被视为“笔记链接”:无扩展名,或 .md/.markdown。其余(图片/pdf 等资源)忽略。
fn target_is_note_like(target: &str) -> bool {
    if target.is_empty() {
        return true; // 同文件锚点
    }
    let last = target.rsplit('/').next().unwrap_or(target);
    match last.rsplit_once('.') {
        None => true,
        Some((_, ext)) => {
            let e = ext.to_ascii_lowercase();
            e == "md" || e == "markdown"
        }
    }
}

/// 把行内代码 `code` 段替换为等长空格,避免误抓代码里的 # 与链接。
fn mask_inline_code(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for ch in line.chars() {
        if ch == '`' {
            in_code = !in_code;
            out.push(' ');
        } else if in_code {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

fn split_target_anchor(raw: &str) -> (String, Option<String>) {
    let raw = raw.trim().trim_start_matches('<').trim_end_matches('>').trim();
    match raw.split_once('#') {
        Some((t, a)) => {
            let anchor = a.trim();
            (
                t.trim().to_string(),
                if anchor.is_empty() { None } else { Some(anchor.to_string()) },
            )
        }
        None => (raw.to_string(), None),
    }
}

fn extract_wikilinks(masked: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let bytes = masked.as_bytes();
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(rel_end) = masked[i + 2..].find("]]") {
                let inner = &masked[i + 2..i + 2 + rel_end];
                // [[target|alias]] → 取 '|' 之前;支持 [[target#anchor|alias]]
                let target_part = inner.split('|').next().unwrap_or("").trim();
                if !target_part.is_empty() {
                    let (t, a) = split_target_anchor(target_part);
                    out.push((t, a));
                }
                i = i + 2 + rel_end + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn extract_md_links(masked: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    for cap in MD_LINK_RE.captures_iter(masked) {
        if cap.get(1).map(|m| m.as_str() == "!").unwrap_or(false) {
            continue; // 图片
        }
        let url = cap.get(3).map(|m| m.as_str()).unwrap_or("").trim();
        if url.is_empty() || is_external_target(url) {
            continue;
        }
        let (t, a) = split_target_anchor(url);
        if !target_is_note_like(&t) {
            continue;
        }
        out.push((t, a));
    }
    out
}

fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || c == '/'
}

fn extract_tags_in_line(masked: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = masked.chars().collect();
    let n = chars.len();
    let mut i = 0usize;
    while i < n {
        if chars[i] == '#' {
            let prev_ok = i == 0 || chars[i - 1].is_whitespace();
            let next = chars.get(i + 1).copied();
            if prev_ok && next.map(is_tag_char).unwrap_or(false) {
                let mut j = i + 1;
                while j < n && is_tag_char(chars[j]) {
                    j += 1;
                }
                let tag: String = chars[i + 1..j].iter().collect();
                // 跳过纯数字标签(如 #123),避免误抓
                if !tag.is_empty() && !tag.chars().all(|c| c.is_ascii_digit()) {
                    out.push(tag);
                }
                i = j;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// 从 markdown 内容抽取显式关系信号。`file_rel` 用于复现与 kbmeta 一致的 section stable_id。
pub fn extract_refs(file_rel: &str, content: &str) -> ExtractedRefs {
    let mut result = ExtractedRefs::default();
    let mut in_fence = false;
    let mut cur_section: Option<String> = None;

    for (idx, line) in content.lines().enumerate() {
        let line_num = (idx + 1) as u32;
        let lt = line.trim_start();

        if lt.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }

        // 标题行:更新当前 section,并跳过(不在标题里抓 tag)。
        let hashes = lt.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) {
            let rest = lt[hashes..].trim();
            if !rest.is_empty() {
                let sid = sha256_hex(&format!("{}\nH{}\nL{}\n{}", file_rel, hashes, line_num, rest));
                cur_section = Some(sid);
                continue;
            }
        }

        let masked = mask_inline_code(line);

        for (target, anchor) in extract_wikilinks(&masked) {
            result.refs.push(RawRef {
                kind: RefKind::Wikilink,
                target,
                anchor,
                from_section: cur_section.clone(),
                line: line_num,
            });
        }
        for (target, anchor) in extract_md_links(&masked) {
            result.refs.push(RawRef {
                kind: RefKind::MdLink,
                target,
                anchor,
                from_section: cur_section.clone(),
                line: line_num,
            });
        }
        for tag in extract_tags_in_line(&masked) {
            result.tags.push(TagHit {
                tag,
                from_section: cur_section.clone(),
                line: line_num,
            });
        }
    }

    result
}

/// 索引状态(供前端判断是否需要构建/重建)。
#[derive(serde::Serialize, Debug, Clone, Default)]
pub struct KgIndexStatus {
    pub kb_name: String,
    pub exists: bool,
    pub schema_version: i64,
    pub current_schema_version: i64,
    pub built_at: Option<String>,
    pub node_count: i64,
    pub edge_count: i64,
    pub embedding_model: Option<String>,
    pub embedding_dim: i64,
    pub embedded_count: i64,
    pub layout_version: i64,
    pub community_level: i64,
    pub stale: bool,
}

/// 读取 kg 全局设置(key/value 存于 kg_index.db)。
pub fn get_kg_setting(key: &str) -> Option<String> {
    let conn = ensure_kg_db().ok()?;
    conn.query_row("SELECT value FROM kg_settings WHERE key = ?1", [key], |r| r.get::<_, String>(0))
        .ok()
}

/// 写入 kg 全局设置。
pub fn set_kg_setting(key: &str, value: &str) -> Result<(), String> {
    let conn = ensure_kg_db()?;
    conn.execute(
        "INSERT INTO kg_settings (key,value) VALUES (?1,?2) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 命令:设置激活的 embedding 模型(key:multilingual-e5-small | bge-m3)。
/// 切换模型会使所有已存向量维度失效,需重算——此处仅记录选择,重算在 kg_embed_kb 触发。
#[tauri::command]
pub fn kg_set_embedding_model(model_key: String) -> Result<(), String> {
    set_kg_setting("embedding_model", model_key.trim())
}

/// 读取某知识库的图索引状态。
#[tauri::command]
pub fn kg_index_status(kb_name: String) -> Result<KgIndexStatus, String> {
    let conn = ensure_kg_db()?;
    let mut status = KgIndexStatus {
        kb_name: kb_name.clone(),
        current_schema_version: KG_SCHEMA_VERSION,
        ..Default::default()
    };

    let row = conn.query_row(
        "SELECT schema_version, built_at, node_count, edge_count, embedding_model, \
                embedding_dim, embedded_count, layout_version, community_level \
         FROM kg_build_state WHERE kb_name = ?1",
        [&kb_name],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, i64>(8)?,
            ))
        },
    );

    if let Ok((sv, built_at, nc, ec, model, dim, emb, lv, cl)) = row {
        status.exists = true;
        status.schema_version = sv;
        status.built_at = built_at;
        status.node_count = nc;
        status.edge_count = ec;
        status.embedding_model = model;
        status.embedding_dim = dim;
        status.embedded_count = emb;
        status.layout_version = lv;
        status.community_level = cl;
        status.stale = sv != KG_SCHEMA_VERSION;
    } else {
        status.stale = true;
    }

    Ok(status)
}

// ───────────────────────────── 索引构建器 ─────────────────────────────

use chrono::Local;
use std::collections::HashMap;
use std::path::Path;

const MAX_MD_BYTES: u64 = 4 * 1024 * 1024;

fn kb_root_dir() -> PathBuf {
    PathBuf::from(crate::commands::settingview::get_knowledge_bases_root())
}

fn kg_is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

fn kg_is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
        .unwrap_or(false)
}

fn kg_rel_path(root: &Path, abs: &Path) -> String {
    abs.strip_prefix(root)
        .unwrap_or(abs)
        .to_string_lossy()
        .replace('\\', "/")
}


fn kg_excerpt_one_line(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    let mut count = 0usize;
    let mut last_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if out.is_empty() || last_space {
                continue;
            }
            out.push(' ');
            count += 1;
            last_space = true;
        } else {
            out.push(ch);
            count += 1;
            last_space = false;
        }
        if count >= max_chars {
            break;
        }
    }
    out.trim().to_string()
}

fn node_id_dir(rel: &str) -> String {
    format!("dir_{}", sha256_hex(rel))
}
fn node_id_file(rel: &str) -> String {
    format!("file_{}", sha256_hex(rel))
}
fn node_id_section(stable_id: &str) -> String {
    format!("section_{stable_id}")
}
fn node_id_para(stable_id: &str) -> String {
    format!("para_{stable_id}")
}

struct KgSection {
    level: u8,
    title: String,
    start_line: u32,
    stable_id: String,
}
struct KgParagraph {
    start_line: u32,
    end_line: u32,
    excerpt: String,
    text_hash: String,
    stable_id: String,
    parent_section: Option<String>,
}

/// 解析 markdown 结构,stable_id 公式与 kbmeta/knowledgebase 保持一致。
fn parse_structure(file_rel: &str, content: &str) -> (Vec<KgSection>, Vec<KgParagraph>) {
    let mut sections = Vec::new();
    let mut paragraphs = Vec::new();
    let mut in_fence = false;
    let mut para_lines: Vec<String> = Vec::new();
    let mut para_start_line: u32 = 0;
    let mut last_section: Option<String> = None;

    let flush = |end_line: u32,
                     para_lines: &mut Vec<String>,
                     para_start_line: &mut u32,
                     last_section: &Option<String>,
                     paragraphs: &mut Vec<KgParagraph>| {
        if para_lines.is_empty() {
            return;
        }
        let text = para_lines.join("\n").trim().to_string();
        para_lines.clear();
        if text.is_empty() {
            return;
        }
        let excerpt = kg_excerpt_one_line(&text, 80);
        if excerpt.is_empty() {
            return;
        }
        let stable_id = sha256_hex(&format!("{}\nP\nL{}\n{}", file_rel, *para_start_line, text));
        paragraphs.push(KgParagraph {
            start_line: *para_start_line,
            end_line,
            excerpt,
            text_hash: sha256_hex(&text),
            stable_id,
            parent_section: last_section.clone(),
        });
        *para_start_line = 0;
    };

    for (idx, line) in content.lines().enumerate() {
        let line_num = (idx + 1) as u32;
        let l = line.trim_end();
        let lt = l.trim_start();

        if lt.starts_with("```") {
            flush(line_num.saturating_sub(1), &mut para_lines, &mut para_start_line, &last_section, &mut paragraphs);
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }

        let hashes = lt.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) {
            let rest = lt[hashes..].trim();
            if !rest.is_empty() {
                flush(line_num.saturating_sub(1), &mut para_lines, &mut para_start_line, &last_section, &mut paragraphs);
                let stable_id = sha256_hex(&format!("{}\nH{}\nL{}\n{}", file_rel, hashes, line_num, rest));
                last_section = Some(stable_id.clone());
                sections.push(KgSection {
                    level: hashes as u8,
                    title: rest.to_string(),
                    start_line: line_num,
                    stable_id,
                });
                continue;
            }
        }

        if lt.is_empty() {
            flush(line_num.saturating_sub(1), &mut para_lines, &mut para_start_line, &last_section, &mut paragraphs);
            continue;
        }
        if para_lines.is_empty() {
            para_start_line = line_num;
        }
        para_lines.push(l.to_string());
    }
    flush(content.lines().count() as u32, &mut para_lines, &mut para_start_line, &last_section, &mut paragraphs);

    (sections, paragraphs)
}

#[derive(Default)]
struct NodeRow {
    node_id: String,
    node_type: String,
    name: String,
    rel_path: Option<String>,
    abs_path: Option<String>,
    file_path: Option<String>,
    stable_id: Option<String>,
    parent_id: Option<String>,
    depth: i64,
    start_line: Option<i64>,
    end_line: Option<i64>,
    level: Option<i64>,
    content_hash: Option<String>,
}

struct EdgeRow {
    from_id: String,
    to_id: String,
    edge_type: String,
    strength: f64,
    source: String,
    src_file: Option<String>,
}

struct TagRow {
    node_id: String,
    tag: String,
    source: String,
}

/// 一个文件抽取出的引用,带源文件节点上下文,留待全库解析。
struct FileRefs {
    src_file_rel: String,
    src_file_node: String,
    refs: ExtractedRefs,
}

#[derive(Default)]
struct BuildAccum {
    nodes: Vec<NodeRow>,
    edges: Vec<EdgeRow>,
    tags: Vec<TagRow>,
    file_refs: Vec<FileRefs>,
    // 解析索引
    file_by_relpath: HashMap<String, String>, // 小写 rel(含/不含扩展名) → file node_id
    file_by_stem: HashMap<String, String>,     // 小写文件名(stem) → file node_id(首个)
    section_in_file: HashMap<(String, String), String>, // (file_node, 小写标题) → section node_id
    stable_to_node: HashMap<String, String>,         // stable_id → 节点 id(section/para)
}

#[allow(clippy::too_many_arguments)]
fn walk_build(
    base: &Path,
    cur: &Path,
    parent_id: &str,
    parent_depth: i64,
    acc: &mut BuildAccum,
) -> Result<(), String> {
    let mut entries: Vec<PathBuf> = fs::read_dir(cur)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|x| x.path()))
        .collect();
    entries.sort_by(|a, b| a.to_string_lossy().to_lowercase().cmp(&b.to_string_lossy().to_lowercase()));

    for path in entries {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.is_empty() || kg_is_hidden(name) || name == ".meta" {
            continue;
        }
        let rel = kg_rel_path(base, &path);
        let abs_str = path.to_string_lossy().to_string();

        if path.is_dir() {
            let id = node_id_dir(&rel);
            acc.nodes.push(NodeRow {
                node_id: id.clone(),
                node_type: "directory".into(),
                name: name.into(),
                rel_path: Some(rel.clone()),
                abs_path: Some(abs_str),
                depth: parent_depth + 1,
                ..Default::default()
            });
            acc.edges.push(EdgeRow {
                from_id: parent_id.into(),
                to_id: id.clone(),
                edge_type: "contains".into(),
                strength: 0.7,
                source: "structural".into(),
                src_file: None,
            });
            walk_build(base, &path, &id, parent_depth + 1, acc)?;
            continue;
        }

        // 文件节点
        let file_node = node_id_file(&rel);
        acc.nodes.push(NodeRow {
            node_id: file_node.clone(),
            node_type: "file".into(),
            name: name.into(),
            rel_path: Some(rel.clone()),
            abs_path: Some(abs_str.clone()),
            file_path: Some(abs_str.clone()),
            depth: parent_depth + 1,
            ..Default::default()
        });
        acc.edges.push(EdgeRow {
            from_id: parent_id.into(),
            to_id: file_node.clone(),
            edge_type: "contains".into(),
            strength: 0.5,
            source: "structural".into(),
            src_file: Some(rel.clone()),
        });

        // 解析索引:rel(含/不含扩展名)与 stem
        let rel_lower = rel.to_lowercase();
        acc.file_by_relpath.insert(rel_lower.clone(), file_node.clone());
        if let Some(stripped) = rel_lower.strip_suffix(".md").or_else(|| rel_lower.strip_suffix(".markdown")) {
            acc.file_by_relpath.insert(stripped.to_string(), file_node.clone());
        }
        let stem = Path::new(&rel)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(name)
            .to_lowercase();
        acc.file_by_stem.entry(stem).or_insert_with(|| file_node.clone());

        if !kg_is_markdown(&path) {
            continue;
        }
        let too_large = fs::metadata(&path).ok().map(|m| m.len() > MAX_MD_BYTES).unwrap_or(false);
        if too_large {
            continue;
        }
        let content = match fs::read_to_string(&path) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let (sections, paragraphs) = parse_structure(&rel, &content);

        // section 节点
        for sec in &sections {
            let sec_id = node_id_section(&sec.stable_id);
            acc.stable_to_node.insert(sec.stable_id.clone(), sec_id.clone());
            acc.section_in_file.insert((file_node.clone(), sec.title.to_lowercase()), sec_id.clone());
            acc.nodes.push(NodeRow {
                node_id: sec_id.clone(),
                node_type: "section".into(),
                name: sec.title.clone(),
                rel_path: Some(rel.clone()),
                abs_path: Some(abs_str.clone()),
                file_path: Some(abs_str.clone()),
                stable_id: Some(sec.stable_id.clone()),
                parent_id: Some(file_node.clone()),
                depth: parent_depth + 2,
                start_line: Some(sec.start_line as i64),
                level: Some(sec.level as i64),
                content_hash: Some(sha256_hex(&sec.title)),
                ..Default::default()
            });
            acc.edges.push(EdgeRow {
                from_id: file_node.clone(),
                to_id: sec_id.clone(),
                edge_type: "contains".into(),
                strength: 0.45,
                source: "structural".into(),
                src_file: Some(rel.clone()),
            });
        }

        // paragraph 节点 + contains + next
        let mut prev_para: Option<String> = None;
        for p in &paragraphs {
            let p_id = node_id_para(&p.stable_id);
            acc.stable_to_node.insert(p.stable_id.clone(), p_id.clone());
            let parent = p
                .parent_section
                .as_ref()
                .map(|sid| node_id_section(sid))
                .unwrap_or_else(|| file_node.clone());
            acc.nodes.push(NodeRow {
                node_id: p_id.clone(),
                node_type: "paragraph".into(),
                name: p.excerpt.clone(),
                rel_path: Some(rel.clone()),
                abs_path: Some(abs_str.clone()),
                file_path: Some(abs_str.clone()),
                stable_id: Some(p.stable_id.clone()),
                parent_id: Some(parent.clone()),
                depth: parent_depth + 3,
                start_line: Some(p.start_line as i64),
                end_line: Some(p.end_line as i64),
                content_hash: Some(p.text_hash.clone()),
                ..Default::default()
            });
            acc.edges.push(EdgeRow {
                from_id: parent,
                to_id: p_id.clone(),
                edge_type: "contains".into(),
                strength: 0.28,
                source: "structural".into(),
                src_file: Some(rel.clone()),
            });
            if let Some(prev) = prev_para.take() {
                acc.edges.push(EdgeRow {
                    from_id: prev,
                    to_id: p_id.clone(),
                    edge_type: "next".into(),
                    strength: 0.12,
                    source: "structural".into(),
                    src_file: Some(rel.clone()),
                });
            }
            prev_para = Some(p_id);
        }

        // 抽取显式引用与行内标签(留待全库解析)
        let refs = extract_refs(&rel, &content);
        // 行内标签直接落到“来源 section 或文件”节点
        for th in &refs.tags {
            let node = th
                .from_section
                .as_ref()
                .and_then(|sid| acc.stable_to_node.get(&node_id_section_key(sid)).cloned())
                .unwrap_or_else(|| file_node.clone());
            acc.tags.push(TagRow { node_id: node, tag: th.tag.clone(), source: "inline".into() });
        }
        acc.file_refs.push(FileRefs {
            src_file_rel: rel.clone(),
            src_file_node: file_node.clone(),
            refs,
        });
    }
    Ok(())
}

/// from_section 存的是 section 的 stable_id,这里换算为 stable_to_node 的查询键。
fn node_id_section_key(stable_id: &str) -> String {
    stable_id.to_string()
}

/// 解析所有文件引用为 link 边,并把 .meta related 关系补成 related 边。
fn resolve_refs_to_edges(acc: &mut BuildAccum) {
    let mut link_edges: Vec<EdgeRow> = Vec::new();

    for fr in &acc.file_refs {
        for r in &fr.refs.refs {
            // from 节点:优先 section,否则文件
            let from_id = r
                .from_section
                .as_ref()
                .and_then(|sid| acc.stable_to_node.get(sid).cloned())
                .unwrap_or_else(|| fr.src_file_node.clone());

            // 解析目标文件节点
            let target_file_node: Option<String> = if r.target.is_empty() {
                // 同文件锚点
                Some(fr.src_file_node.clone())
            } else {
                let t = r.target.replace('\\', "/");
                let t = t.strip_prefix("./").unwrap_or(&t).to_lowercase();
                acc.file_by_relpath
                    .get(&t)
                    .cloned()
                    .or_else(|| acc.file_by_relpath.get(&format!("{t}.md")).cloned())
                    .or_else(|| {
                        let stem = Path::new(&t).file_stem().and_then(|s| s.to_str()).unwrap_or(&t).to_string();
                        acc.file_by_stem.get(&stem).cloned()
                    })
            };

            let Some(file_node) = target_file_node else { continue };

            // 若有锚点,尝试解析到目标文件内的 section
            let to_id = if let Some(anchor) = &r.anchor {
                acc.section_in_file
                    .get(&(file_node.clone(), anchor.to_lowercase()))
                    .cloned()
                    .unwrap_or(file_node)
            } else {
                file_node
            };

            if from_id == to_id {
                continue;
            }
            let source = if r.kind == RefKind::Wikilink { "wikilink" } else { "mdlink" };
            link_edges.push(EdgeRow {
                from_id,
                to_id,
                edge_type: "link".into(),
                strength: 0.4,
                source: source.into(),
                src_file: Some(fr.src_file_rel.clone()),
            });
        }
    }
    acc.edges.append(&mut link_edges);
}

/// 全量构建某知识库的图索引,物化进 SQLite。返回 (节点数, 边数)。
pub fn build_kb_index(kb_name: &str) -> Result<(usize, usize), String> {
    let root = kb_root_dir();
    let kb_dir = root.join(kb_name);
    if !kb_dir.exists() {
        return Err(format!("知识库不存在: {kb_name}"));
    }

    let mut conn = ensure_kg_db()?;
    let r = build_into_conn(&mut conn, kb_name, &kb_dir)?;
    MEM_CACHE.write().remove(kb_name);
    Ok(r)
}

/// 构建核心:把图物化进给定连接(便于注入内存库做测试)。调用方需保证 schema 已就绪。
fn build_into_conn(
    conn: &mut Connection,
    kb_name: &str,
    kb_dir: &Path,
) -> Result<(usize, usize), String> {
    let mut acc = BuildAccum::default();
    // kb 根节点
    acc.nodes.push(NodeRow {
        node_id: "kb_root".into(),
        node_type: "knowledge-base".into(),
        name: kb_name.into(),
        rel_path: Some(String::new()),
        abs_path: Some(kb_dir.to_string_lossy().to_string()),
        depth: 0,
        ..Default::default()
    });

    walk_build(kb_dir, kb_dir, "kb_root", 0, &mut acc)?;
    resolve_refs_to_edges(&mut acc);

    // 去重边(同 from/to/type 取首个)并统计度
    let mut seen: std::collections::HashSet<(String, String, String)> = std::collections::HashSet::new();
    let mut degree: HashMap<String, i64> = HashMap::new();
    let mut edges: Vec<EdgeRow> = Vec::with_capacity(acc.edges.len());
    for e in acc.edges.drain(..) {
        let key = (e.from_id.clone(), e.to_id.clone(), e.edge_type.clone());
        if !seen.insert(key) {
            continue;
        }
        *degree.entry(e.from_id.clone()).or_default() += 1;
        *degree.entry(e.to_id.clone()).or_default() += 1;
        edges.push(e);
    }

    let node_count = acc.nodes.len();
    let edge_count = edges.len();
    let now = Local::now().to_rfc3339();
    let mut node_id_set: HashSet<String> = acc.nodes.iter().map(|n| n.node_id.clone()).collect();

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // 概念节点不是文件派生的(由 LLM 抽取),重建时需保留。
    let preserved_concepts: Vec<(String, String, Option<String>)> = {
        let mut stmt = tx
            .prepare(
                "SELECT node_id,name,content_hash FROM kg_nodes \
                 WHERE kb_name=?1 AND node_type='concept'",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([kb_name], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?))
            })
            .map_err(|e| e.to_string())?;
        let mut v = Vec::new();
        for row in rows {
            v.push(row.map_err(|e| e.to_string())?);
        }
        v
    };
    // 概念节点 id 计入存活集合,使其 mention/relation 边在回填时不被当作悬挂丢弃。
    for (id, _, _) in &preserved_concepts {
        node_id_set.insert(id.clone());
    }

    // 重建前保留“非结构性”的用户/AI 产物(语义/LLM 边、zeroshot/llm 标签),
    // 仅当其端点在新节点集合中仍存在时回填,避免悬挂。
    let preserved_edges: Vec<(String, String, String, f64, String)> = {
        let mut stmt = tx
            .prepare(
                "SELECT from_id,to_id,edge_type,strength,source FROM kg_edges \
                 WHERE kb_name=?1 AND source IN ('semantic','llm','manual')",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([kb_name], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, f64>(3)?, r.get::<_, String>(4)?))
            })
            .map_err(|e| e.to_string())?;
        let mut v = Vec::new();
        for row in rows {
            v.push(row.map_err(|e| e.to_string())?);
        }
        v
    };
    let preserved_tags: Vec<(String, String, String, f64)> = {
        let mut stmt = tx
            .prepare(
                "SELECT node_id,tag,source,score FROM kg_node_tags \
                 WHERE kb_name=?1 AND source IN ('manual','zeroshot','llm')",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([kb_name], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, f64>(3)?))
            })
            .map_err(|e| e.to_string())?;
        let mut v = Vec::new();
        for row in rows {
            v.push(row.map_err(|e| e.to_string())?);
        }
        v
    };

    // 全量重建:清空派生数据(embeddings/layout/communities 由各自阶段维护)
    tx.execute("DELETE FROM kg_nodes WHERE kb_name = ?1", [kb_name]).map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM kg_edges WHERE kb_name = ?1", [kb_name]).map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM kg_node_tags WHERE kb_name = ?1", [kb_name]).map_err(|e| e.to_string())?;

    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO kg_nodes (kb_name,node_id,node_type,name,rel_path,abs_path,file_path,\
                 stable_id,parent_id,depth,start_line,end_line,level,content_hash,degree,created_at,modified_at) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
            )
            .map_err(|e| e.to_string())?;
        for n in &acc.nodes {
            stmt.execute(rusqlite::params![
                kb_name,
                n.node_id,
                n.node_type,
                n.name,
                n.rel_path,
                n.abs_path,
                n.file_path,
                n.stable_id,
                n.parent_id,
                n.depth,
                n.start_line,
                n.end_line,
                n.level,
                n.content_hash,
                degree.get(&n.node_id).copied().unwrap_or(0),
                now,
                now,
            ])
            .map_err(|e| e.to_string())?;
        }
    }
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO kg_edges (kb_name,from_id,to_id,edge_type,strength,source,src_file) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7)",
            )
            .map_err(|e| e.to_string())?;
        for e in &edges {
            stmt.execute(rusqlite::params![
                kb_name, e.from_id, e.to_id, e.edge_type, e.strength, e.source, e.src_file
            ])
            .map_err(|e| e.to_string())?;
        }
    }
    {
        let mut stmt = tx
            .prepare(
                "INSERT OR IGNORE INTO kg_node_tags (kb_name,node_id,tag,source,score) \
                 VALUES (?1,?2,?3,?4,1.0)",
            )
            .map_err(|e| e.to_string())?;
        for t in &acc.tags {
            stmt.execute(rusqlite::params![kb_name, t.node_id, t.tag, t.source])
                .map_err(|e| e.to_string())?;
        }
    }

    // 回填保留的概念节点
    {
        let mut stmt = tx
            .prepare(
                "INSERT OR IGNORE INTO kg_nodes (kb_name,node_id,node_type,name,depth,content_hash,degree,created_at,modified_at) \
                 VALUES (?1,?2,'concept',?3,1,?4,0,?5,?5)",
            )
            .map_err(|e| e.to_string())?;
        for (id, name, chash) in &preserved_concepts {
            stmt.execute(rusqlite::params![kb_name, id, name, chash, now]).map_err(|e| e.to_string())?;
        }
    }

    // 回填保留的非结构性边/标签(端点存在才插入)
    {
        let mut stmt = tx
            .prepare(
                "INSERT OR IGNORE INTO kg_edges (kb_name,from_id,to_id,edge_type,strength,source) \
                 VALUES (?1,?2,?3,?4,?5,?6)",
            )
            .map_err(|e| e.to_string())?;
        for (from_id, to_id, etype, strength, source) in &preserved_edges {
            if node_id_set.contains(from_id) && node_id_set.contains(to_id) {
                stmt.execute(rusqlite::params![kb_name, from_id, to_id, etype, strength, source])
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    {
        let mut stmt = tx
            .prepare(
                "INSERT OR IGNORE INTO kg_node_tags (kb_name,node_id,tag,source,score) \
                 VALUES (?1,?2,?3,?4,?5)",
            )
            .map_err(|e| e.to_string())?;
        for (node_id, tag, source, score) in &preserved_tags {
            if node_id_set.contains(node_id) {
                stmt.execute(rusqlite::params![kb_name, node_id, tag, source, score])
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    tx.execute(
        "INSERT INTO kg_build_state (kb_name,schema_version,built_at,node_count,edge_count,dirty) \
         VALUES (?1,?2,?3,?4,?5,0) \
         ON CONFLICT(kb_name) DO UPDATE SET \
           schema_version=excluded.schema_version, built_at=excluded.built_at, \
           node_count=excluded.node_count, edge_count=excluded.edge_count, dirty=0",
        rusqlite::params![kb_name, KG_SCHEMA_VERSION, now, node_count as i64, edge_count as i64],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok((node_count, edge_count))
}

/// 清空某知识库的内存图与向量缓存(写入图/向量后调用,使下次查询重载)。
pub fn invalidate_kb_caches(kb_name: &str) {
    MEM_CACHE.write().remove(kb_name);
    VEC_CACHE.write().remove(kb_name);
}

/// 全局 AppHandle,供非命令上下文(CRUD 钩子)向所有窗口广播索引变更事件。
/// 由 main.rs 的 setup 注入一次。
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// 注入全局 AppHandle(幂等;重复调用忽略)。
pub fn init_app_handle(handle: AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

/// 广播"某知识库图索引已变更"给所有窗口(主窗口 Brain 视图、独立的知识库管理器窗口等)。
///
/// 事件只做通知,不触发任何重建 —— 重建仍由查询侧 `build_if_needed` 惰性完成,
/// 保存路径保持零阻塞。`reason`: `dirty`(增删改移) / `removed`(知识库整体删除)。
fn emit_index_changed(kb_name: &str, reason: &str) {
    let Some(app) = APP_HANDLE.get() else { return };
    if let Err(e) = app.emit(
        "kg_index_changed",
        serde_json::json!({ "kb_name": kb_name, "reason": reason }),
    ) {
        tracing::warn!(error = %e, kb_name, reason, "emit kg_index_changed failed");
    }
}

/// 把某知识库的图索引标记为脏(增量钩子调用,极轻量,绝不阻塞保存)。
/// 真正的重建在查询时按需触发(materialized-view 惰性刷新)。
pub fn mark_kb_dirty(kb_name: &str) {
    let _ = (|| -> Result<(), String> {
        let conn = ensure_kg_db()?;
        conn.execute(
            "INSERT INTO kg_build_state (kb_name,schema_version,dirty) VALUES (?1,?2,1) \
             ON CONFLICT(kb_name) DO UPDATE SET dirty=1",
            rusqlite::params![kb_name, KG_SCHEMA_VERSION],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })();
    emit_index_changed(kb_name, "dirty");
}

/// 删除某知识库的全部图索引数据(知识库被删除时调用)。
pub fn drop_kb_index(kb_name: &str) {
    let _ = (|| -> Result<(), String> {
        let conn = ensure_kg_db()?;
        for t in [
            "kg_nodes",
            "kg_edges",
            "kg_node_tags",
            "kg_embeddings",
            "kg_layout",
            "kg_communities",
            "kg_build_state",
        ] {
            conn.execute(&format!("DELETE FROM {t} WHERE kb_name = ?1"), [kb_name])
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    emit_index_changed(kb_name, "removed");
}

// ─────────────────────── 文件 CRUD 钩子(取代旧 kbmeta 钩子) ───────────────────────
//
// 文件增删改移后由 fileprocess / flipboard / knowledgebase 调用,负责维护图索引的
// 脏标记与生命周期。kgraph 自行解析 .md,不再依赖任何 .meta 侧车。

/// 从绝对路径解析其所属知识库名(知识库根下的顶层目录名);非库内路径返回 None。
fn kb_name_from_path(path: &Path) -> Option<String> {
    let rel = path.strip_prefix(kb_root_dir()).ok()?;
    let first = rel.components().next()?;
    let name = first.as_os_str().to_string_lossy().to_string();
    if name.is_empty() || name.starts_with('.') {
        None
    } else {
        Some(name)
    }
}

/// 路径是否为某知识库的根目录(parent 恰为知识库根)。
fn is_kb_root_path(path: &Path) -> bool {
    path.parent().map(|p| p == kb_root_dir()).unwrap_or(false)
}

pub fn after_write_file(path: &Path, _content: &str) -> Result<(), String> {
    if let Some(kb) = kb_name_from_path(path) {
        mark_kb_dirty(&kb);
    }
    Ok(())
}

pub fn after_create_path(path: &Path, _is_dir: bool, _content: Option<&str>) -> Result<(), String> {
    if let Some(kb) = kb_name_from_path(path) {
        mark_kb_dirty(&kb);
    }
    Ok(())
}

pub fn after_delete_path(path: &Path, _is_dir: bool) -> Result<(), String> {
    if let Some(kb) = kb_name_from_path(path) {
        if is_kb_root_path(path) {
            drop_kb_index(&kb);
        } else {
            mark_kb_dirty(&kb);
        }
    }
    Ok(())
}

pub fn after_move_path(src: &Path, dst: &Path) -> Result<(), String> {
    if let Some(kb) = kb_name_from_path(src) {
        if is_kb_root_path(src) {
            drop_kb_index(&kb);
        } else {
            mark_kb_dirty(&kb);
        }
    }
    if let Some(kb) = kb_name_from_path(dst) {
        mark_kb_dirty(&kb);
    }
    Ok(())
}

/// 查询前调用:若索引缺失 / schema 过期 / 被标记为脏,则触发一次全量重建。
pub fn build_if_needed(kb_name: &str) -> Result<(), String> {
    let conn = ensure_kg_db()?;
    let row: Option<(i64, i64, i64)> = conn
        .query_row(
            "SELECT schema_version, dirty, node_count FROM kg_build_state WHERE kb_name = ?1",
            [kb_name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();
    let need = match row {
        None => true,
        Some((sv, dirty, nc)) => sv != KG_SCHEMA_VERSION || dirty != 0 || nc == 0,
    };
    drop(conn);
    if need {
        build_kb_index(kb_name)?;
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct KgBuildResult {
    pub kb_name: String,
    pub node_count: usize,
    pub edge_count: usize,
}

/// 触发某知识库图索引的全量构建。
#[tauri::command]
pub fn kg_build_index(kb_name: String) -> Result<KgBuildResult, String> {
    let (node_count, edge_count) = build_kb_index(&kb_name)?;
    Ok(KgBuildResult { kb_name, node_count, edge_count })
}

// ─────────────────────────── 内存图 + 查询命令 ───────────────────────────

use parking_lot::RwLock;
use petgraph::graph::{NodeIndex, UnGraph};
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

#[derive(Clone, serde::Serialize)]
pub struct MemNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub name: String,
    pub rel_path: Option<String>,
    pub path: Option<String>,
    pub file_path: Option<String>,
    pub stable_id: Option<String>,
    pub parent_id: Option<String>,
    pub depth: i64,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub level: Option<i64>,
    pub degree: i64,
    pub pagerank: f64,
    pub community_id: Option<i64>,
    pub size: f64,
    pub tags: Vec<String>,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
}

#[derive(Clone, serde::Serialize)]
pub struct MemEdge {
    pub from: String,
    pub to: String,
    #[serde(rename = "type")]
    pub edge_type: String,
    pub strength: f64,
    pub source: String,
}

pub struct MemGraph {
    pub nodes: Vec<MemNode>,
    pub edges: Vec<MemEdge>,
    pub pos: HashMap<String, usize>,
    pub g: UnGraph<u32, u32>,
    pub nidx: HashMap<String, NodeIndex>,
    pub children: HashMap<String, Vec<String>>, // contains 层级:parent → children
    pub built_at: String,
}

static MEM_CACHE: Lazy<RwLock<HashMap<String, Arc<MemGraph>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

fn type_size(t: &str) -> f64 {
    match t {
        "knowledge-base" => 1.0,
        "directory" => 0.5,
        "file" => 0.3,
        "section" => 0.22,
        "concept" => 0.35,
        _ => 0.16,
    }
}

/// 无向图上的简易 PageRank(20 轮,阻尼 0.85),作为“重要度”中心度。
fn compute_pagerank(g: &UnGraph<u32, u32>) -> Vec<f64> {
    let n = g.node_count();
    if n == 0 {
        return Vec::new();
    }
    let nf = n as f64;
    let damping = 0.85;
    let mut pr = vec![1.0 / nf; n];
    let deg: Vec<usize> = g.node_indices().map(|i| g.neighbors(i).count()).collect();

    for _ in 0..20 {
        let mut next = vec![(1.0 - damping) / nf; n];
        let mut dangling = 0.0;
        for i in g.node_indices() {
            let idx = i.index();
            if deg[idx] == 0 {
                dangling += pr[idx];
            }
        }
        let dangling_share = damping * dangling / nf;
        for i in g.node_indices() {
            let idx = i.index();
            if deg[idx] == 0 {
                continue;
            }
            let share = damping * pr[idx] / deg[idx] as f64;
            for nb in g.neighbors(i) {
                next[nb.index()] += share;
            }
        }
        for v in next.iter_mut() {
            *v += dangling_share;
        }
        pr = next;
    }
    pr
}

/// 从 SQLite 载入内存图(带缓存,按 built_at 失效)。
fn load_mem_graph(kb_name: &str) -> Result<Arc<MemGraph>, String> {
    build_if_needed(kb_name)?;
    let conn = ensure_kg_db()?;

    let built_at: String = conn
        .query_row(
            "SELECT COALESCE(built_at,'') FROM kg_build_state WHERE kb_name = ?1",
            [kb_name],
            |r| r.get(0),
        )
        .unwrap_or_default();

    if let Some(cached) = MEM_CACHE.read().get(kb_name) {
        if cached.built_at == built_at {
            return Ok(cached.clone());
        }
    }

    // 标签:node_id → tags
    let mut tags_by_node: HashMap<String, Vec<String>> = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT node_id, tag FROM kg_node_tags WHERE kb_name = ?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([kb_name], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (nid, tag) = row.map_err(|e| e.to_string())?;
            tags_by_node.entry(nid).or_default().push(tag);
        }
    }

    let mut nodes: Vec<MemNode> = Vec::new();
    let mut pos: HashMap<String, usize> = HashMap::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT node_id,node_type,name,rel_path,abs_path,file_path,stable_id,parent_id,\
                        depth,start_line,end_line,level,degree,community_id,created_at,modified_at \
                 FROM kg_nodes WHERE kb_name = ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([kb_name], |r| {
                Ok(MemNode {
                    id: r.get(0)?,
                    node_type: r.get(1)?,
                    name: r.get(2)?,
                    rel_path: r.get(3)?,
                    path: r.get::<_, Option<String>>(4)?,
                    file_path: r.get(5)?,
                    stable_id: r.get(6)?,
                    parent_id: r.get(7)?,
                    depth: r.get(8)?,
                    start_line: r.get(9)?,
                    end_line: r.get(10)?,
                    level: r.get(11)?,
                    degree: r.get(12)?,
                    community_id: r.get(13)?,
                    created_at: r.get(14)?,
                    modified_at: r.get(15)?,
                    pagerank: 0.0,
                    size: 0.0,
                    tags: Vec::new(),
                })
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let mut n = row.map_err(|e| e.to_string())?;
            n.size = type_size(&n.node_type);
            n.tags = tags_by_node.remove(&n.id).unwrap_or_default();
            pos.insert(n.id.clone(), nodes.len());
            nodes.push(n);
        }
    }

    let mut edges: Vec<MemEdge> = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT from_id,to_id,edge_type,strength,source FROM kg_edges WHERE kb_name = ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([kb_name], |r| {
                Ok(MemEdge {
                    from: r.get(0)?,
                    to: r.get(1)?,
                    edge_type: r.get(2)?,
                    strength: r.get(3)?,
                    source: r.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            edges.push(row.map_err(|e| e.to_string())?);
        }
    }

    // 构图
    let mut g: UnGraph<u32, u32> = UnGraph::new_undirected();
    let mut nidx: HashMap<String, NodeIndex> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        let idx = g.add_node(i as u32);
        nidx.insert(n.id.clone(), idx);
    }
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    for (ei, e) in edges.iter().enumerate() {
        if let (Some(&a), Some(&b)) = (nidx.get(&e.from), nidx.get(&e.to)) {
            g.add_edge(a, b, ei as u32);
        }
        if e.edge_type == "contains" {
            children.entry(e.from.clone()).or_default().push(e.to.clone());
        }
    }

    // PageRank → 写回 nodes
    let pr = compute_pagerank(&g);
    for idx in g.node_indices() {
        let npos = g[idx] as usize;
        nodes[npos].pagerank = pr[idx.index()];
    }

    let mem = Arc::new(MemGraph {
        nodes,
        edges,
        pos,
        g,
        nidx,
        children,
        built_at,
    });
    MEM_CACHE.write().insert(kb_name.to_string(), mem.clone());
    Ok(mem)
}

#[derive(serde::Serialize)]
pub struct KgGraphData {
    pub nodes: Vec<MemNode>,
    pub edges: Vec<MemEdge>,
    pub total_nodes: usize,
    pub returned_nodes: usize,
    pub truncated: bool,
}

#[derive(serde::Deserialize, Default)]
pub struct KgQueryOpts {
    #[serde(default)]
    pub root_id: Option<String>,
    #[serde(default)]
    pub max_depth: Option<i64>,
    #[serde(default)]
    pub node_types: Option<Vec<String>>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub edge_types: Option<Vec<String>>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

fn assemble_subgraph(mem: &MemGraph, ids: &HashSet<String>, edge_types: &Option<Vec<String>>) -> (Vec<MemNode>, Vec<MemEdge>) {
    let out_nodes: Vec<MemNode> = ids
        .iter()
        .filter_map(|id| mem.pos.get(id).map(|&p| mem.nodes[p].clone()))
        .collect();
    let out_edges: Vec<MemEdge> = mem
        .edges
        .iter()
        .filter(|e| ids.contains(&e.from) && ids.contains(&e.to))
        .filter(|e| edge_types.as_ref().map(|ts| ts.iter().any(|t| t == &e.edge_type)).unwrap_or(true))
        .cloned()
        .collect();
    (out_nodes, out_edges)
}

/// 分层 LOD 查询:从 root_id 沿 contains 层级 BFS 到 max_depth,叠加类型/标签/搜索过滤与节点上限。
#[tauri::command]
pub fn kg_query_graph(kb_name: String, opts: Option<KgQueryOpts>) -> Result<KgGraphData, String> {
    let mem = load_mem_graph(&kb_name)?;
    let opts = opts.unwrap_or_default();
    let total_nodes = mem.nodes.len();

    let root = opts.root_id.clone().unwrap_or_else(|| "kb_root".to_string());
    let max_depth = opts.max_depth.unwrap_or(2).max(0);
    let limit = opts.limit.unwrap_or(3000);

    // 沿 contains 层级 BFS
    let mut selected: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, i64)> = VecDeque::new();
    if mem.pos.contains_key(&root) {
        queue.push_back((root.clone(), 0));
        seen.insert(root.clone());
    }
    while let Some((id, d)) = queue.pop_front() {
        selected.push(id.clone());
        if d >= max_depth {
            continue;
        }
        if let Some(ch) = mem.children.get(&id) {
            for c in ch {
                if seen.insert(c.clone()) {
                    queue.push_back((c.clone(), d + 1));
                }
            }
        }
    }

    // 过滤
    let search_lc = opts.search.as_ref().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty());
    let mut filtered: Vec<&MemNode> = selected
        .iter()
        .filter_map(|id| mem.pos.get(id).map(|&p| &mem.nodes[p]))
        .filter(|n| {
            opts.node_types.as_ref().map(|ts| ts.iter().any(|t| t == &n.node_type)).unwrap_or(true)
        })
        .filter(|n| match &opts.tags {
            Some(ts) if !ts.is_empty() => n.tags.iter().any(|tag| ts.iter().any(|t| t == tag)),
            _ => true,
        })
        .filter(|n| match &search_lc {
            Some(q) => n.name.to_lowercase().contains(q) || n.rel_path.as_deref().unwrap_or("").to_lowercase().contains(q),
            None => true,
        })
        .collect();

    // 始终保留根节点;超限时按 depth 浅、degree 高优先
    let truncated = filtered.len() > limit;
    if truncated {
        filtered.sort_by(|a, b| a.depth.cmp(&b.depth).then(b.degree.cmp(&a.degree)));
        filtered.truncate(limit);
    }

    let ids: HashSet<String> = filtered.iter().map(|n| n.id.clone()).collect();
    let (nodes, edges) = assemble_subgraph(&mem, &ids, &opts.edge_types);
    let returned_nodes = nodes.len();

    Ok(KgGraphData { nodes, edges, total_nodes, returned_nodes, truncated })
}

/// N 跳邻居(ego 网络):在无向图上从 node_id BFS 到 hops 跳,返回诱导子图。
#[tauri::command]
pub fn kg_node_neighbors(
    kb_name: String,
    node_id: String,
    hops: Option<i64>,
    limit: Option<usize>,
    edge_types: Option<Vec<String>>,
) -> Result<KgGraphData, String> {
    let mem = load_mem_graph(&kb_name)?;
    let total_nodes = mem.nodes.len();
    let hops = hops.unwrap_or(1).max(0);
    let limit = limit.unwrap_or(1500);

    let Some(&start) = mem.nidx.get(&node_id) else {
        return Err(format!("节点不存在: {node_id}"));
    };

    let mut ids: HashSet<String> = HashSet::new();
    let mut seen_idx: HashSet<usize> = HashSet::new();
    let mut queue: VecDeque<(NodeIndex, i64)> = VecDeque::new();
    queue.push_back((start, 0));
    seen_idx.insert(start.index());
    ids.insert(node_id.clone());

    while let Some((idx, d)) = queue.pop_front() {
        if d >= hops || ids.len() >= limit {
            continue;
        }
        for nb in mem.g.neighbors(idx) {
            let npos = mem.g[nb] as usize;
            let nid = &mem.nodes[npos].id;
            if seen_idx.insert(nb.index()) {
                ids.insert(nid.clone());
                queue.push_back((nb, d + 1));
            }
        }
    }

    let (nodes, edges) = assemble_subgraph(&mem, &ids, &edge_types);
    let returned_nodes = nodes.len();
    Ok(KgGraphData { nodes, edges, total_nodes, returned_nodes, truncated: ids.len() >= limit })
}

#[derive(serde::Serialize)]
pub struct KgPathResult {
    pub found: bool,
    pub nodes: Vec<MemNode>,
    pub edges: Vec<MemEdge>,
    pub hops: usize,
}

/// 两节点间的最短路径(无向、无权 BFS),用于“这两块知识怎么连起来”。
#[tauri::command]
pub fn kg_shortest_path(kb_name: String, from_id: String, to_id: String) -> Result<KgPathResult, String> {
    let mem = load_mem_graph(&kb_name)?;
    let (Some(&s), Some(&t)) = (mem.nidx.get(&from_id), mem.nidx.get(&to_id)) else {
        return Err("起点或终点不存在".to_string());
    };

    let mut prev: HashMap<usize, usize> = HashMap::new();
    let mut visited: HashSet<usize> = HashSet::new();
    let mut queue: VecDeque<NodeIndex> = VecDeque::new();
    queue.push_back(s);
    visited.insert(s.index());
    let mut found = false;
    while let Some(idx) = queue.pop_front() {
        if idx == t {
            found = true;
            break;
        }
        for nb in mem.g.neighbors(idx) {
            if visited.insert(nb.index()) {
                prev.insert(nb.index(), idx.index());
                queue.push_back(nb);
            }
        }
    }

    if !found {
        return Ok(KgPathResult { found: false, nodes: vec![], edges: vec![], hops: 0 });
    }

    // 回溯路径(节点索引序列)
    let mut path_idx: Vec<usize> = vec![t.index()];
    let mut cur = t.index();
    while cur != s.index() {
        let p = prev[&cur];
        path_idx.push(p);
        cur = p;
    }
    path_idx.reverse();

    let path_ids: Vec<String> = path_idx
        .iter()
        .map(|&gi| mem.nodes[mem.g[NodeIndex::new(gi)] as usize].id.clone())
        .collect();
    let id_set: HashSet<String> = path_ids.iter().cloned().collect();

    let nodes: Vec<MemNode> = path_ids
        .iter()
        .map(|id| mem.nodes[mem.pos[id]].clone())
        .collect();
    // 仅保留路径上相邻节点之间的边
    let mut edges: Vec<MemEdge> = Vec::new();
    for w in path_ids.windows(2) {
        if let Some(e) = mem.edges.iter().find(|e| {
            (e.from == w[0] && e.to == w[1]) || (e.from == w[1] && e.to == w[0])
        }) {
            edges.push(e.clone());
        }
    }
    let _ = id_set;
    let hops = path_ids.len().saturating_sub(1);
    Ok(KgPathResult { found: true, nodes, edges, hops })
}

#[derive(serde::Serialize)]
pub struct KgCount {
    pub key: String,
    pub count: i64,
}

#[derive(serde::Serialize)]
pub struct KgStats {
    pub kb_name: String,
    pub total_nodes: usize,
    pub total_edges: usize,
    pub node_types: Vec<KgCount>,
    pub edge_types: Vec<KgCount>,
    pub tags: Vec<KgCount>,
    pub top_nodes: Vec<MemNode>,
}

/// 图统计:类型分布、边类型分布、标签计数、按 PageRank 的核心节点 Top-N。
#[tauri::command]
pub fn kg_stats(kb_name: String, top_n: Option<usize>) -> Result<KgStats, String> {
    let mem = load_mem_graph(&kb_name)?;
    let top_n = top_n.unwrap_or(20);

    let mut nt: HashMap<String, i64> = HashMap::new();
    let mut tag_count: HashMap<String, i64> = HashMap::new();
    for n in &mem.nodes {
        *nt.entry(n.node_type.clone()).or_default() += 1;
        for t in &n.tags {
            *tag_count.entry(t.clone()).or_default() += 1;
        }
    }
    let mut et: HashMap<String, i64> = HashMap::new();
    for e in &mem.edges {
        *et.entry(e.edge_type.clone()).or_default() += 1;
    }

    let to_sorted = |m: HashMap<String, i64>| {
        let mut v: Vec<KgCount> = m.into_iter().map(|(key, count)| KgCount { key, count }).collect();
        v.sort_by(|a, b| b.count.cmp(&a.count).then(a.key.cmp(&b.key)));
        v
    };

    let mut top_nodes = mem.nodes.clone();
    top_nodes.sort_by(|a, b| b.pagerank.partial_cmp(&a.pagerank).unwrap_or(std::cmp::Ordering::Equal));
    top_nodes.truncate(top_n);

    Ok(KgStats {
        kb_name,
        total_nodes: mem.nodes.len(),
        total_edges: mem.edges.len(),
        node_types: to_sorted(nt),
        edge_types: to_sorted(et),
        tags: to_sorted(tag_count),
        top_nodes,
    })
}

// ─────────────────────── 语义层(embedding + 向量检索)───────────────────────

use rayon::prelude::*;

fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

fn blob_to_vec(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// 读取某节点用于 embedding 的文本:段落读原文行,其余用 name。限长避免超大请求。
fn node_embed_text(
    node_type: &str,
    name: &str,
    file_path: Option<&str>,
    start_line: Option<i64>,
    end_line: Option<i64>,
    file_cache: &mut HashMap<String, Vec<String>>,
) -> String {
    const MAX: usize = 2000;
    if node_type == "paragraph" {
        if let (Some(fp), Some(s), Some(e)) = (file_path, start_line, end_line) {
            let lines = file_cache.entry(fp.to_string()).or_insert_with(|| {
                fs::read_to_string(fp)
                    .map(|c| c.lines().map(|l| l.to_string()).collect())
                    .unwrap_or_default()
            });
            let s = (s.max(1) as usize).saturating_sub(1);
            let e = (e.max(0) as usize).min(lines.len());
            if s < e {
                let text = lines[s..e].join("\n");
                let text = text.trim();
                if !text.is_empty() {
                    return text.chars().take(MAX).collect();
                }
            }
        }
    }
    name.chars().take(MAX).collect()
}

#[derive(serde::Serialize)]
pub struct KgEmbedResult {
    pub kb_name: String,
    pub model_key: String,
    pub dim: usize,
    pub embedded: usize,
    pub skipped: usize,
    pub total_eligible: usize,
}

/// 为知识库的 section/paragraph/file/concept 节点计算 embedding 并增量入库。
/// 模型变更或维度不一致时清空旧向量全量重算。需要 embedding 服务可用(缺模型自动下载)。
#[tauri::command]
pub async fn kg_embed_kb(kb_name: String) -> Result<KgEmbedResult, String> {
    build_if_needed(&kb_name)?;
    let model = crate::commands::embedding::active_model();

    // —— gather(同步,不跨 await 持有连接)——
    let (targets, clear_all, eligible_total) = {
        let conn = ensure_kg_db()?;
        let prev_model: String = conn
            .query_row(
                "SELECT COALESCE(embedding_model,'') FROM kg_build_state WHERE kb_name=?1",
                [&kb_name],
                |r| r.get(0),
            )
            .unwrap_or_default();
        let prev_dim: i64 = conn
            .query_row(
                "SELECT embedding_dim FROM kg_build_state WHERE kb_name=?1",
                [&kb_name],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let clear_all = prev_model != model.key || prev_dim as usize != model.dim;

        // 已有向量的 content_hash(用于跳过未变化节点)
        let mut existing: HashMap<String, String> = HashMap::new();
        if !clear_all {
            let mut stmt = conn
                .prepare("SELECT node_id, COALESCE(content_hash,'') FROM kg_embeddings WHERE kb_name=?1")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([&kb_name], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .map_err(|e| e.to_string())?;
            for row in rows {
                let (id, h) = row.map_err(|e| e.to_string())?;
                existing.insert(id, h);
            }
        }

        let mut stmt = conn
            .prepare(
                "SELECT node_id,node_type,name,file_path,start_line,end_line,content_hash \
                 FROM kg_nodes WHERE kb_name=?1 AND node_type IN ('section','paragraph','file','concept')",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([&kb_name], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        let mut file_cache: HashMap<String, Vec<String>> = HashMap::new();
        let mut targets: Vec<(String, String, String)> = Vec::new(); // (node_id, text, hash)
        let mut eligible_total = 0usize;
        for row in rows {
            let (id, ntype, name, fp, sl, el, chash) = row.map_err(|e| e.to_string())?;
            eligible_total += 1;
            let text = node_embed_text(&ntype, &name, fp.as_deref(), sl, el, &mut file_cache);
            if text.trim().is_empty() {
                continue;
            }
            let hash = chash.unwrap_or_else(|| sha256_hex(&text));
            if !clear_all {
                if let Some(prev) = existing.get(&id) {
                    if prev == &hash {
                        continue; // 未变化,跳过
                    }
                }
            }
            targets.push((id, text, hash));
        }
        (targets, clear_all, eligible_total)
    };

    let skipped = eligible_total.saturating_sub(targets.len());
    if targets.is_empty() && !clear_all {
        return Ok(KgEmbedResult {
            kb_name,
            model_key: model.key.into(),
            dim: model.dim,
            embedded: 0,
            skipped,
            total_eligible: eligible_total,
        });
    }

    // —— embed(异步)——
    let texts: Vec<String> = targets.iter().map(|t| t.1.clone()).collect();
    let vecs = crate::commands::embedding::embed_texts(&texts, false).await?;
    if vecs.len() != targets.len() {
        return Err(format!("embedding 数量不匹配: {} vs {}", vecs.len(), targets.len()));
    }
    let dim = vecs.first().map(|v| v.len()).unwrap_or(model.dim);

    // —— store(同步)——
    {
        let mut conn = ensure_kg_db()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        if clear_all {
            tx.execute("DELETE FROM kg_embeddings WHERE kb_name=?1", [&kb_name]).map_err(|e| e.to_string())?;
        }
        {
            let mut stmt = tx
                .prepare(
                    "INSERT OR REPLACE INTO kg_embeddings (kb_name,node_id,content_hash,dim,vec) \
                     VALUES (?1,?2,?3,?4,?5)",
                )
                .map_err(|e| e.to_string())?;
            for ((id, _text, hash), v) in targets.iter().zip(vecs.iter()) {
                stmt.execute(rusqlite::params![kb_name, id, hash, dim as i64, vec_to_blob(v)])
                    .map_err(|e| e.to_string())?;
            }
        }
        let embedded_count: i64 = tx
            .query_row("SELECT COUNT(*) FROM kg_embeddings WHERE kb_name=?1", [&kb_name], |r| r.get(0))
            .unwrap_or(0);
        tx.execute(
            "UPDATE kg_build_state SET embedding_model=?2, embedding_dim=?3, embedded_count=?4 WHERE kb_name=?1",
            rusqlite::params![kb_name, model.key, dim as i64, embedded_count],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    VEC_CACHE.write().remove(&kb_name);

    Ok(KgEmbedResult {
        kb_name,
        model_key: model.key.into(),
        dim,
        embedded: targets.len(),
        skipped,
        total_eligible: eligible_total,
    })
}

/// 内存向量库(扁平存储,点积≈余弦,因向量已 L2 归一化)。
pub struct VecStore {
    pub ids: Vec<String>,
    pub types: Vec<String>,
    pub data: Vec<f32>, // 扁平 len = ids.len() * dim
    pub dim: usize,
    pub pos: HashMap<String, usize>,
    pub model: String,
    pub built_at: String,
    pub emb_count: i64,
}

static VEC_CACHE: Lazy<RwLock<HashMap<String, Arc<VecStore>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

fn load_vectors(kb_name: &str) -> Result<Arc<VecStore>, String> {
    let conn = ensure_kg_db()?;
    let (model, built_at, emb_count): (String, String, i64) = conn
        .query_row(
            "SELECT COALESCE(embedding_model,''),COALESCE(built_at,''),embedded_count \
             FROM kg_build_state WHERE kb_name=?1",
            [kb_name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or_default();

    if let Some(c) = VEC_CACHE.read().get(kb_name) {
        if c.built_at == built_at && c.model == model && c.emb_count == emb_count {
            return Ok(c.clone());
        }
    }

    let mut ids = Vec::new();
    let mut types = Vec::new();
    let mut data: Vec<f32> = Vec::new();
    let mut pos = HashMap::new();
    let mut dim = 0usize;
    {
        let mut stmt = conn
            .prepare(
                "SELECT e.node_id, e.vec, COALESCE(n.node_type,'') \
                 FROM kg_embeddings e LEFT JOIN kg_nodes n \
                   ON n.kb_name=e.kb_name AND n.node_id=e.node_id \
                 WHERE e.kb_name=?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([kb_name], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?, r.get::<_, String>(2)?))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (id, blob, ntype) = row.map_err(|e| e.to_string())?;
            let v = blob_to_vec(&blob);
            if v.is_empty() {
                continue;
            }
            if dim == 0 {
                dim = v.len();
            }
            if v.len() != dim {
                continue; // 维度不一致(模型混用残留),跳过
            }
            pos.insert(id.clone(), ids.len());
            ids.push(id);
            types.push(ntype);
            data.extend_from_slice(&v);
        }
    }

    let store = Arc::new(VecStore { ids, types, data, dim, pos, model, built_at, emb_count });
    VEC_CACHE.write().insert(kb_name.to_string(), store.clone());
    Ok(store)
}

/// 在向量库中暴力检索 top-k(点积),可选类型过滤与排除集合。
fn vec_search(
    store: &VecStore,
    query: &[f32],
    top_k: usize,
    types: &Option<Vec<String>>,
    exclude: &HashSet<String>,
) -> Vec<(String, f32)> {
    if store.dim == 0 || query.len() != store.dim {
        return Vec::new();
    }
    let n = store.ids.len();
    let mut scored: Vec<(usize, f32)> = (0..n)
        .into_par_iter()
        .filter_map(|i| {
            if exclude.contains(&store.ids[i]) {
                return None;
            }
            if let Some(ts) = types {
                if !ts.iter().any(|t| t == &store.types[i]) {
                    return None;
                }
            }
            let base = i * store.dim;
            let row = &store.data[base..base + store.dim];
            let dot: f32 = row.iter().zip(query).map(|(a, b)| a * b).sum();
            Some((i, dot))
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k);
    scored.into_iter().map(|(i, s)| (store.ids[i].clone(), s)).collect()
}

#[derive(serde::Serialize)]
pub struct KgScoredNode {
    #[serde(flatten)]
    pub node: MemNode,
    pub score: f32,
}

/// 语义检索:对查询文本求向量,返回最相近的节点(带分数)。
#[tauri::command]
pub async fn kg_semantic_search(
    kb_name: String,
    query: String,
    top_k: Option<usize>,
    node_types: Option<Vec<String>>,
) -> Result<Vec<KgScoredNode>, String> {
    let top_k = top_k.unwrap_or(20);
    let qv = crate::commands::embedding::embed_texts(&[query], true).await?;
    let qv = qv.into_iter().next().ok_or_else(|| "查询向量为空".to_string())?;

    let store = load_vectors(&kb_name)?;
    let mem = load_mem_graph(&kb_name)?;
    let hits = vec_search(&store, &qv, top_k, &node_types, &HashSet::new());
    Ok(hits
        .into_iter()
        .filter_map(|(id, score)| mem.pos.get(&id).map(|&p| KgScoredNode { node: mem.nodes[p].clone(), score }))
        .collect())
}

#[derive(serde::Serialize)]
pub struct KgRelationSuggestion {
    pub from: String,
    pub to: String,
    pub to_name: String,
    pub to_type: String,
    pub score: f32,
}

/// 基于向量相似度推荐候选关系边:返回与 node_id 最相近、且尚未直接相连的节点。
/// 用户确认后通过 kg_confirm_relation 固化为 related 边。
#[tauri::command]
pub fn kg_suggest_relations(
    kb_name: String,
    node_id: String,
    top_k: Option<usize>,
    min_score: Option<f32>,
) -> Result<Vec<KgRelationSuggestion>, String> {
    let top_k = top_k.unwrap_or(10);
    let min_score = min_score.unwrap_or(0.6);
    let store = load_vectors(&kb_name)?;
    let mem = load_mem_graph(&kb_name)?;

    let Some(&p) = store.pos.get(&node_id) else {
        return Err("该节点尚无向量,请先运行 kg_embed_kb".to_string());
    };
    let base = p * store.dim;
    let qv = store.data[base..base + store.dim].to_vec();

    // 排除自身 + 已直接相连的节点
    let mut exclude: HashSet<String> = HashSet::new();
    exclude.insert(node_id.clone());
    if let Some(&idx) = mem.nidx.get(&node_id) {
        for nb in mem.g.neighbors(idx) {
            exclude.insert(mem.nodes[mem.g[nb] as usize].id.clone());
        }
    }

    let hits = vec_search(&store, &qv, top_k + exclude.len(), &None, &exclude);
    Ok(hits
        .into_iter()
        .filter(|(_, s)| *s >= min_score)
        .take(top_k)
        .filter_map(|(id, score)| {
            mem.pos.get(&id).map(|&q| KgRelationSuggestion {
                from: node_id.clone(),
                to: id.clone(),
                to_name: mem.nodes[q].name.clone(),
                to_type: mem.nodes[q].node_type.clone(),
                score,
            })
        })
        .collect())
}

#[derive(serde::Serialize)]
pub struct KgTagSuggestion {
    pub tag: String,
    pub score: f32,
}

/// 零样本打标签:用候选标签的向量与节点向量比余弦,推荐最贴合且节点尚未拥有的标签。
/// 候选标签默认取该库已有标签词表(把已有标签传播到相似的未标注节点)。
#[tauri::command]
pub async fn kg_suggest_tags(
    kb_name: String,
    node_id: String,
    candidate_tags: Option<Vec<String>>,
    top_k: Option<usize>,
    min_score: Option<f32>,
) -> Result<Vec<KgTagSuggestion>, String> {
    let top_k = top_k.unwrap_or(5);
    let min_score = min_score.unwrap_or(0.55);

    // 候选标签:显式传入,或取库内已有标签词表
    let candidates: Vec<String> = match candidate_tags {
        Some(t) if !t.is_empty() => t,
        _ => {
            let conn = ensure_kg_db()?;
            let mut stmt = conn
                .prepare("SELECT DISTINCT tag FROM kg_node_tags WHERE kb_name=?1")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([&kb_name], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            let mut v = Vec::new();
            for row in rows {
                v.push(row.map_err(|e| e.to_string())?);
            }
            v
        }
    };
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    // 节点向量
    let store = load_vectors(&kb_name)?;
    let Some(&p) = store.pos.get(&node_id) else {
        return Err("该节点尚无向量,请先运行 kg_embed_kb".to_string());
    };
    let base = p * store.dim;
    let nv = &store.data[base..base + store.dim];

    // 已有标签(排除)
    let existing: HashSet<String> = {
        let conn = ensure_kg_db()?;
        let mut stmt = conn
            .prepare("SELECT tag FROM kg_node_tags WHERE kb_name=?1 AND node_id=?2")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![kb_name, node_id], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let mut s = HashSet::new();
        for row in rows {
            s.insert(row.map_err(|e| e.to_string())?);
        }
        s
    };

    // 候选标签向量(作为 passage 编码)
    let tag_vecs = crate::commands::embedding::embed_texts(&candidates, false).await?;
    let mut scored: Vec<KgTagSuggestion> = candidates
        .iter()
        .zip(tag_vecs.iter())
        .filter(|(t, _)| !existing.contains(*t))
        .filter(|(_, v)| v.len() == store.dim)
        .map(|(t, v)| {
            let dot: f32 = v.iter().zip(nv).map(|(a, b)| a * b).sum();
            KgTagSuggestion { tag: t.clone(), score: dot }
        })
        .filter(|s| s.score >= min_score)
        .collect();
    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k);
    Ok(scored)
}

/// 固化一条用户确认的关系边(source=manual,默认 edge_type=related)。
#[tauri::command]
pub fn kg_confirm_relation(
    kb_name: String,
    from_id: String,
    to_id: String,
    relation: Option<String>,
    strength: Option<f64>,
) -> Result<(), String> {
    if from_id == to_id {
        return Err("不能连接节点到自身".to_string());
    }
    let etype = relation.unwrap_or_else(|| "related".to_string());
    let conn = ensure_kg_db()?;
    conn.execute(
        "INSERT OR IGNORE INTO kg_edges (kb_name,from_id,to_id,edge_type,strength,source) \
         VALUES (?1,?2,?3,?4,?5,'manual')",
        rusqlite::params![kb_name, from_id, to_id, etype, strength.unwrap_or(0.5)],
    )
    .map_err(|e| e.to_string())?;
    MEM_CACHE.write().remove(&kb_name);
    write_manual_snapshot(&kb_name);
    Ok(())
}

/// 固化一个用户确认/AI 建议的标签(source 默认 zeroshot)。
#[tauri::command]
pub fn kg_add_tag(
    kb_name: String,
    node_id: String,
    tag: String,
    source: Option<String>,
    score: Option<f64>,
) -> Result<(), String> {
    let tag = tag.trim().to_string();
    if tag.is_empty() {
        return Err("标签不能为空".to_string());
    }
    let conn = ensure_kg_db()?;
    conn.execute(
        "INSERT OR IGNORE INTO kg_node_tags (kb_name,node_id,tag,source,score) VALUES (?1,?2,?3,?4,?5)",
        rusqlite::params![kb_name, node_id, tag, source.unwrap_or_else(|| "zeroshot".into()), score.unwrap_or(1.0)],
    )
    .map_err(|e| e.to_string())?;
    MEM_CACHE.write().remove(&kb_name);
    write_manual_snapshot(&kb_name);
    Ok(())
}

/// 删除某节点上的一个标签(用户在图谱中手动移除)。
#[tauri::command]
pub fn kg_remove_tag(kb_name: String, node_id: String, tag: String) -> Result<(), String> {
    let tag = tag.trim().to_string();
    if tag.is_empty() {
        return Err("标签不能为空".to_string());
    }
    let conn = ensure_kg_db()?;
    conn.execute(
        "DELETE FROM kg_node_tags WHERE kb_name=?1 AND node_id=?2 AND tag=?3",
        rusqlite::params![kb_name, node_id, tag],
    )
    .map_err(|e| e.to_string())?;
    MEM_CACHE.write().remove(&kb_name);
    write_manual_snapshot(&kb_name);
    Ok(())
}

/// 删除一条关系边。relation 省略时删除该有向对之间的所有边类型;
/// 仅作用于用户/语义产生的边(source∈ manual/semantic/related/llm),结构性边(contains/next)不允许删除。
#[tauri::command]
pub fn kg_remove_relation(
    kb_name: String,
    from_id: String,
    to_id: String,
    relation: Option<String>,
) -> Result<usize, String> {
    let conn = ensure_kg_db()?;
    let protected = ["contains", "next"];
    let n = match relation {
        Some(rel) if !rel.trim().is_empty() => {
            let rel = rel.trim();
            if protected.contains(&rel) {
                return Err(format!("结构性关系 {rel} 不可手动删除"));
            }
            conn.execute(
                "DELETE FROM kg_edges WHERE kb_name=?1 AND from_id=?2 AND to_id=?3 AND edge_type=?4 \
                 AND edge_type NOT IN ('contains','next')",
                rusqlite::params![kb_name, from_id, to_id, rel],
            )
            .map_err(|e| e.to_string())?
        }
        _ => conn
            .execute(
                "DELETE FROM kg_edges WHERE kb_name=?1 AND from_id=?2 AND to_id=?3 \
                 AND edge_type NOT IN ('contains','next')",
                rusqlite::params![kb_name, from_id, to_id],
            )
            .map_err(|e| e.to_string())?,
    };
    MEM_CACHE.write().remove(&kb_name);
    write_manual_snapshot(&kb_name);
    Ok(n)
}

// ─────────────────────── 人工知识层导出 / 导入(可移植同步) ───────────────────────
//
// kg_index.db 是本地派生固化索引;其中“无法从 .md 重建”的人工 / LLM 产物
// (manual/llm/semantic 边、manual/zeroshot/llm 标签、concept 概念节点)是唯一需要
// 跨机同步的知识。这里把这层序列化为每个知识库一个 `manual.kgraph.json` 快照,随知识库
// 目录走现有文件同步(机制A);导入语义为“整层替换”(whole-layer LWW),天然携带删除。
// 所有节点标识(file_/dir_/section_/para_/concept_ + 哈希)均跨机一致,因此无需重映射。

const MANUAL_SNAPSHOT_FILE: &str = ".manual.kgraph.json";
const MANUAL_SNAPSHOT_VERSION: u32 = 1;

fn default_strength() -> f64 {
    0.5
}
fn default_score() -> f64 {
    1.0
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
pub struct ManualEdge {
    pub from_id: String,
    pub to_id: String,
    pub edge_type: String,
    #[serde(default = "default_strength")]
    pub strength: f64,
    pub source: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
pub struct ManualTag {
    pub node_id: String,
    pub tag: String,
    pub source: String,
    #[serde(default = "default_score")]
    pub score: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
pub struct ManualConcept {
    pub node_id: String,
    pub name: String,
    #[serde(default)]
    pub content_hash: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
pub struct ManualBundle {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub kb_name: String,
    #[serde(default)]
    pub exported_at: String,
    #[serde(default)]
    pub edges: Vec<ManualEdge>,
    #[serde(default)]
    pub tags: Vec<ManualTag>,
    #[serde(default)]
    pub concepts: Vec<ManualConcept>,
}

fn manual_snapshot_path(kb_name: &str) -> PathBuf {
    kb_root_dir().join(kb_name).join(MANUAL_SNAPSHOT_FILE)
}

/// 从 kg_index.db 读取某知识库的人工知识层(可移植快照)。
pub fn export_manual_bundle(kb_name: &str) -> Result<ManualBundle, String> {
    let conn = ensure_kg_db()?;

    let mut edges = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT from_id,to_id,edge_type,strength,source FROM kg_edges \
                 WHERE kb_name=?1 AND source IN ('manual','llm','semantic')",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([kb_name], |r| {
                Ok(ManualEdge {
                    from_id: r.get(0)?,
                    to_id: r.get(1)?,
                    edge_type: r.get(2)?,
                    strength: r.get(3)?,
                    source: r.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            edges.push(row.map_err(|e| e.to_string())?);
        }
    }

    let mut tags = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT node_id,tag,source,score FROM kg_node_tags \
                 WHERE kb_name=?1 AND source IN ('manual','zeroshot','llm')",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([kb_name], |r| {
                Ok(ManualTag {
                    node_id: r.get(0)?,
                    tag: r.get(1)?,
                    source: r.get(2)?,
                    score: r.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            tags.push(row.map_err(|e| e.to_string())?);
        }
    }

    let mut concepts = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT node_id,name,content_hash FROM kg_nodes \
                 WHERE kb_name=?1 AND node_type='concept'",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([kb_name], |r| {
                Ok(ManualConcept {
                    node_id: r.get(0)?,
                    name: r.get(1)?,
                    content_hash: r.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            concepts.push(row.map_err(|e| e.to_string())?);
        }
    }

    Ok(ManualBundle {
        version: MANUAL_SNAPSHOT_VERSION,
        kb_name: kb_name.to_string(),
        exported_at: Local::now().to_rfc3339(),
        edges,
        tags,
        concepts,
    })
}

/// 把当前 DB 的人工层快照写回知识库目录(随文件同步)。最佳努力,绝不阻断保存。
pub fn write_manual_snapshot(kb_name: &str) {
    let _ = (|| -> Result<(), String> {
        let kb_dir = kb_root_dir().join(kb_name);
        if !kb_dir.exists() {
            return Ok(());
        }
        let bundle = export_manual_bundle(kb_name)?;
        let p = manual_snapshot_path(kb_name);
        // 人工层为空且本来就没有快照文件 → 不创建空文件,减少同步噪声;
        // 若文件已存在则写入空快照,使“清空”能经由 LWW 传播到其它机器。
        if bundle.edges.is_empty()
            && bundle.tags.is_empty()
            && bundle.concepts.is_empty()
            && !p.exists()
        {
            return Ok(());
        }
        let json = serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())?;
        fs::write(&p, json).map_err(|e| e.to_string())?;
        Ok(())
    })();
}

/// 读取磁盘上的人工层快照(可能不存在)。
pub fn read_manual_snapshot(kb_name: &str) -> Result<Option<ManualBundle>, String> {
    let p = manual_snapshot_path(kb_name);
    if !p.exists() {
        return Ok(None);
    }
    let s = fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let b: ManualBundle = serde_json::from_str(&s).map_err(|e| e.to_string())?;
    Ok(Some(b))
}

/// 用快照整层替换某知识库的人工层(机制A:whole-layer LWW,天然携带删除),随后标脏待重建。
/// 导入的边/标签即使端点暂不存在也先落库;下次重建会保留端点存在者并丢弃悬挂项。
pub fn import_manual_bundle(kb_name: &str, bundle: &ManualBundle) -> Result<(), String> {
    let mut conn = ensure_kg_db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "DELETE FROM kg_edges WHERE kb_name=?1 AND source IN ('manual','llm','semantic')",
        [kb_name],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM kg_node_tags WHERE kb_name=?1 AND source IN ('manual','zeroshot','llm')",
        [kb_name],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM kg_nodes WHERE kb_name=?1 AND node_type='concept'",
        [kb_name],
    )
    .map_err(|e| e.to_string())?;

    let now = Local::now().to_rfc3339();
    {
        let mut stmt = tx
            .prepare(
                "INSERT OR IGNORE INTO kg_nodes (kb_name,node_id,node_type,name,depth,content_hash,degree,created_at,modified_at) \
                 VALUES (?1,?2,'concept',?3,1,?4,0,?5,?5)",
            )
            .map_err(|e| e.to_string())?;
        for c in &bundle.concepts {
            stmt.execute(rusqlite::params![kb_name, c.node_id, c.name, c.content_hash, now])
                .map_err(|e| e.to_string())?;
        }
    }
    {
        let mut stmt = tx
            .prepare(
                "INSERT OR IGNORE INTO kg_edges (kb_name,from_id,to_id,edge_type,strength,source) \
                 VALUES (?1,?2,?3,?4,?5,?6)",
            )
            .map_err(|e| e.to_string())?;
        for e in &bundle.edges {
            stmt.execute(rusqlite::params![
                kb_name, e.from_id, e.to_id, e.edge_type, e.strength, e.source
            ])
            .map_err(|e| e.to_string())?;
        }
    }
    {
        let mut stmt = tx
            .prepare(
                "INSERT OR IGNORE INTO kg_node_tags (kb_name,node_id,tag,source,score) \
                 VALUES (?1,?2,?3,?4,?5)",
            )
            .map_err(|e| e.to_string())?;
        for t in &bundle.tags {
            stmt.execute(rusqlite::params![kb_name, t.node_id, t.tag, t.source, t.score])
                .map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    mark_kb_dirty(kb_name);
    invalidate_kb_caches(kb_name);
    Ok(())
}

/// 导出某知识库的人工层(前端可用于查看 / 备份)。
#[tauri::command]
pub fn kg_export_manual(kb_name: String) -> Result<ManualBundle, String> {
    export_manual_bundle(&kb_name)
}

/// 导入人工层:bundle 省略时从磁盘 manual.kgraph.json 读取(机制A 同步收尾用)。
/// 返回是否实际执行了导入。
#[tauri::command]
pub fn kg_import_manual(kb_name: String, bundle: Option<ManualBundle>) -> Result<bool, String> {
    let b = match bundle {
        Some(b) => b,
        None => match read_manual_snapshot(&kb_name)? {
            Some(b) => b,
            None => return Ok(false),
        },
    };
    import_manual_bundle(&kb_name, &b)?;
    Ok(true)
}

/// 把当前 DB 人工层快照落盘(写命令已自动调用;此命令供前端显式触发)。
#[tauri::command]
pub fn kg_write_manual_snapshot(kb_name: String) -> Result<(), String> {
    write_manual_snapshot(&kb_name);
    Ok(())
}

/// 枚举知识库根下的所有知识库名(顶层目录,跳过隐藏与元数据目录)。
pub fn list_kb_names() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(kb_root_dir()) {
        for ent in rd.flatten() {
            if !ent.path().is_dir() {
                continue;
            }
            let name = ent.file_name().to_string_lossy().to_string();
            if name.is_empty() || name.starts_with('.') {
                continue;
            }
            out.push(name);
        }
    }
    out
}

#[derive(serde::Serialize)]
pub struct KgKbSummary {
    pub knowledge_base_name: String,
    pub knowledge_base_location: String,
    pub file_count: i64,
    pub dir_count: i64,
}

fn count_fs_entries(dir: &Path, files: &mut i64, dirs: &mut i64) {
    if let Ok(rd) = fs::read_dir(dir) {
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let p = ent.path();
            if p.is_dir() {
                *dirs += 1;
                count_fs_entries(&p, files, dirs);
            } else if p.is_file() {
                *files += 1;
            }
        }
    }
}

/// 轻量统计各知识库的文件 / 目录数(替代旧 kbmeta_sync_all,供统计面板使用)。
#[tauri::command]
pub fn kg_kb_summaries() -> Result<Vec<KgKbSummary>, String> {
    let root = kb_root_dir();
    let mut out = Vec::new();
    for name in list_kb_names() {
        let kb_dir = root.join(&name);
        let (mut files, mut dirs) = (0i64, 0i64);
        count_fs_entries(&kb_dir, &mut files, &mut dirs);
        out.push(KgKbSummary {
            knowledge_base_name: name,
            knowledge_base_location: kb_dir.to_string_lossy().to_string(),
            file_count: files,
            dir_count: dirs,
        });
    }
    Ok(out)
}

#[derive(serde::Serialize)]
pub struct KgTagCount {
    pub tag: String,
    pub count: i64,
}

/// 列出某知识库的全部标签及其出现次数(供搜索筛选器的标签选择器使用)。
#[tauri::command]
pub fn kg_all_tags(kb_name: String) -> Result<Vec<KgTagCount>, String> {
    build_if_needed(&kb_name)?;
    let conn = ensure_kg_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT tag, COUNT(*) AS c FROM kg_node_tags \
             WHERE kb_name=?1 GROUP BY tag ORDER BY c DESC, tag ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([&kb_name], |r| {
            Ok(KgTagCount { tag: r.get(0)?, count: r.get(1)? })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// 返回携带指定标签的文件绝对路径集合(文件节点本身、或其章节/段落子节点带标签均算)。
/// match_all=true 时要求文件同时满足所有标签;否则任一标签即可。
#[tauri::command]
pub fn kg_files_by_tags(
    kb_name: String,
    tags: Vec<String>,
    match_all: Option<bool>,
) -> Result<Vec<String>, String> {
    let tags: Vec<String> = tags.into_iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
    if tags.is_empty() {
        return Ok(Vec::new());
    }
    build_if_needed(&kb_name)?;
    let conn = ensure_kg_db()?;

    // tag → 该标签覆盖到的文件路径集合
    let mut per_tag: Vec<std::collections::HashSet<String>> = Vec::with_capacity(tags.len());
    let mut stmt = conn
        .prepare(
            "SELECT COALESCE(NULLIF(n.file_path,''), n.abs_path) AS fpath \
             FROM kg_node_tags t JOIN kg_nodes n \
               ON n.kb_name=t.kb_name AND n.node_id=t.node_id \
             WHERE t.kb_name=?1 AND t.tag=?2 \
               AND n.node_type IN ('file','section','paragraph') \
               AND fpath IS NOT NULL AND fpath<>''",
        )
        .map_err(|e| e.to_string())?;

    for tag in &tags {
        let rows = stmt
            .query_map(rusqlite::params![kb_name, tag], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let mut set = std::collections::HashSet::new();
        for row in rows {
            set.insert(row.map_err(|e| e.to_string())?);
        }
        per_tag.push(set);
    }

    let all = match_all.unwrap_or(false);
    let mut result: std::collections::HashSet<String> = per_tag.first().cloned().unwrap_or_default();
    for set in per_tag.iter().skip(1) {
        if all {
            result = result.intersection(set).cloned().collect();
        } else {
            for p in set {
                result.insert(p.clone());
            }
        }
    }

    Ok(result.into_iter().collect())
}

/// 机制B(可选,行级合并)开关:存于 kg_settings。
pub fn manual_sync_enabled() -> bool {
    get_kg_setting("p2p_manual_sync").as_deref() == Some("1")
}

#[tauri::command]
pub fn kg_set_manual_sync(enabled: bool) -> Result<(), String> {
    set_kg_setting("p2p_manual_sync", if enabled { "1" } else { "0" })
}

#[tauri::command]
pub fn kg_get_manual_sync() -> Result<bool, String> {
    Ok(manual_sync_enabled())
}

/// 机制B:按行合并对端人工层(INSERT OR IGNORE,无墓碑,删除不传播),随后标脏待重建。
/// 返回实际新增的行数。合并后刷新本机快照,使机制A/B 结果保持一致。
pub fn merge_manual_bundle(kb_name: &str, bundle: &ManualBundle) -> Result<usize, String> {
    let mut conn = ensure_kg_db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let now = Local::now().to_rfc3339();
    let mut merged = 0usize;
    {
        let mut stmt = tx
            .prepare(
                "INSERT OR IGNORE INTO kg_nodes (kb_name,node_id,node_type,name,depth,content_hash,degree,created_at,modified_at) \
                 VALUES (?1,?2,'concept',?3,1,?4,0,?5,?5)",
            )
            .map_err(|e| e.to_string())?;
        for c in &bundle.concepts {
            merged += stmt
                .execute(rusqlite::params![kb_name, c.node_id, c.name, c.content_hash, now])
                .map_err(|e| e.to_string())?;
        }
    }
    {
        let mut stmt = tx
            .prepare(
                "INSERT OR IGNORE INTO kg_edges (kb_name,from_id,to_id,edge_type,strength,source) \
                 VALUES (?1,?2,?3,?4,?5,?6)",
            )
            .map_err(|e| e.to_string())?;
        for e in &bundle.edges {
            merged += stmt
                .execute(rusqlite::params![kb_name, e.from_id, e.to_id, e.edge_type, e.strength, e.source])
                .map_err(|e| e.to_string())?;
        }
    }
    {
        let mut stmt = tx
            .prepare(
                "INSERT OR IGNORE INTO kg_node_tags (kb_name,node_id,tag,source,score) \
                 VALUES (?1,?2,?3,?4,?5)",
            )
            .map_err(|e| e.to_string())?;
        for t in &bundle.tags {
            merged += stmt
                .execute(rusqlite::params![kb_name, t.node_id, t.tag, t.source, t.score])
                .map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    mark_kb_dirty(kb_name);
    invalidate_kb_caches(kb_name);
    write_manual_snapshot(kb_name);
    Ok(merged)
}

// ─────────────────────── 社区检测(Louvain)───────────────────────
//
// 在加权无向图上做多层 Louvain 模块度优化,把强关联知识点聚成社区,
// 用于前端按社区着色 + “社区折叠 LOD”。结果固化到 kg_nodes.community_id 与 kg_communities。

fn compact_labels(labels: Vec<usize>) -> (Vec<usize>, usize) {
    let mut remap: HashMap<usize, usize> = HashMap::new();
    let mut out = Vec::with_capacity(labels.len());
    for l in labels {
        let next = remap.len();
        let id = *remap.entry(l).or_insert(next);
        out.push(id);
    }
    let k = remap.len();
    (out, k)
}

/// 单层局部移动。返回 (社区分配[0..ncomm), 是否改进, 社区数)。
fn louvain_one_level(n: usize, edges: &[(usize, usize, f64)], resolution: f64) -> (Vec<usize>, bool, usize) {
    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    let mut k = vec![0.0f64; n];
    let mut m2 = 0.0f64;
    for &(a, b, w) in edges {
        if a == b {
            k[a] += 2.0 * w;
            m2 += 2.0 * w;
        } else {
            adj[a].push((b, w));
            adj[b].push((a, w));
            k[a] += w;
            k[b] += w;
            m2 += 2.0 * w;
        }
    }
    if m2 == 0.0 {
        return ((0..n).collect(), false, n);
    }

    let mut comm: Vec<usize> = (0..n).collect();
    let mut sigma_tot = k.clone();
    let mut improved_any = false;
    let mut moved = true;
    let mut rounds = 0;
    while moved && rounds < 50 {
        moved = false;
        rounds += 1;
        for i in 0..n {
            let ci = comm[i];
            // i 到各邻居社区的权重(排序保证确定性)
            let mut w_to: HashMap<usize, f64> = HashMap::new();
            for &(j, w) in &adj[i] {
                if j == i {
                    continue;
                }
                *w_to.entry(comm[j]).or_default() += w;
            }
            sigma_tot[ci] -= k[i];

            let mut candidates: Vec<(usize, f64)> = w_to.into_iter().collect();
            candidates.sort_by(|a, b| a.0.cmp(&b.0));

            let mut best_c = ci;
            let stay_wic = candidates.iter().find(|(c, _)| *c == ci).map(|(_, w)| *w).unwrap_or(0.0);
            let mut best_gain = stay_wic - resolution * sigma_tot[ci] * k[i] / m2;
            for (c, wic) in &candidates {
                let gain = wic - resolution * sigma_tot[*c] * k[i] / m2;
                if gain > best_gain + 1e-12 {
                    best_gain = gain;
                    best_c = *c;
                }
            }
            sigma_tot[best_c] += k[i];
            if best_c != ci {
                comm[i] = best_c;
                moved = true;
                improved_any = true;
            }
        }
    }

    let (compacted, ncomm) = compact_labels(comm);
    (compacted, improved_any, ncomm)
}

fn louvain_aggregate(edges: &[(usize, usize, f64)], comm: &[usize]) -> Vec<(usize, usize, f64)> {
    let mut map: HashMap<(usize, usize), f64> = HashMap::new();
    for &(a, b, w) in edges {
        let (ca, cb) = (comm[a], comm[b]);
        let key = if ca <= cb { (ca, cb) } else { (cb, ca) };
        *map.entry(key).or_default() += w;
    }
    map.into_iter().map(|((a, b), w)| (a, b, w)).collect()
}

/// 多层 Louvain。返回每个原始节点的社区 id(0 起压缩)。
fn louvain(n: usize, edges: &[(usize, usize, f64)], resolution: f64) -> Vec<usize> {
    if n == 0 {
        return Vec::new();
    }
    let mut labels: Vec<usize> = (0..n).collect();
    let mut cur_n = n;
    let mut cur_edges: Vec<(usize, usize, f64)> = edges.to_vec();
    for _ in 0..20 {
        let (comm, improved, ncomm) = louvain_one_level(cur_n, &cur_edges, resolution);
        for l in labels.iter_mut() {
            *l = comm[*l];
        }
        if !improved || ncomm == cur_n {
            break;
        }
        cur_edges = louvain_aggregate(&cur_edges, &comm);
        cur_n = ncomm;
    }
    compact_labels(labels).0
}

/// HSL 调色:按社区索引生成区分度高的颜色(黄金角散布色相)。
fn community_color(idx: usize) -> String {
    let hue = (idx as f64 * 137.508) % 360.0;
    hsl_to_hex(hue, 0.62, 0.55)
}

fn hsl_to_hex(h: f64, s: f64, l: f64) -> String {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    format!("#{:02X}{:02X}{:02X}", to(r1), to(g1), to(b1))
}

#[derive(serde::Serialize)]
pub struct KgCommunityResult {
    pub kb_name: String,
    pub communities: usize,
    pub nodes: usize,
}

/// 命令:对知识库做社区检测并固化(kg_nodes.community_id + kg_communities)。
#[tauri::command]
pub fn kg_detect_communities(kb_name: String, resolution: Option<f64>) -> Result<KgCommunityResult, String> {
    let mem = load_mem_graph(&kb_name)?;
    let n = mem.nodes.len();
    let resolution = resolution.unwrap_or(1.0);

    // 用 mem.pos 索引构建加权无向边(语义/关系边权重更高,层级边较低)
    let edges: Vec<(usize, usize, f64)> = mem
        .edges
        .iter()
        .filter_map(|e| {
            let a = *mem.pos.get(&e.from)?;
            let b = *mem.pos.get(&e.to)?;
            if a == b {
                return None;
            }
            Some((a, b, e.strength.max(0.01)))
        })
        .collect();

    let comm = louvain(n, &edges, resolution);
    let ncomm = comm.iter().copied().max().map(|m| m + 1).unwrap_or(0);

    // 每社区:大小 + 代表节点(PageRank 最高)
    let mut size = vec![0i64; ncomm];
    let mut best_node: Vec<Option<usize>> = vec![None; ncomm];
    for (i, &c) in comm.iter().enumerate() {
        size[c] += 1;
        let better = match best_node[c] {
            None => true,
            Some(bi) => mem.nodes[i].pagerank > mem.nodes[bi].pagerank,
        };
        if better {
            best_node[c] = Some(i);
        }
    }

    let mut conn = ensure_kg_db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM kg_communities WHERE kb_name=?1", [&kb_name]).map_err(|e| e.to_string())?;
    {
        let mut upd = tx
            .prepare("UPDATE kg_nodes SET community_id=?3 WHERE kb_name=?1 AND node_id=?2")
            .map_err(|e| e.to_string())?;
        for (i, &c) in comm.iter().enumerate() {
            upd.execute(rusqlite::params![kb_name, mem.nodes[i].id, c as i64]).map_err(|e| e.to_string())?;
        }
        let mut ins = tx
            .prepare(
                "INSERT INTO kg_communities (kb_name,community_id,level,label,size,centroid_node,color) \
                 VALUES (?1,?2,0,?3,?4,?5,?6)",
            )
            .map_err(|e| e.to_string())?;
        for c in 0..ncomm {
            let (label, centroid) = match best_node[c] {
                Some(bi) => (mem.nodes[bi].name.clone(), mem.nodes[bi].id.clone()),
                None => (format!("社区 {c}"), String::new()),
            };
            ins.execute(rusqlite::params![kb_name, c as i64, label, size[c], centroid, community_color(c)])
                .map_err(|e| e.to_string())?;
        }
    }
    tx.execute(
        "UPDATE kg_build_state SET community_level=?2 WHERE kb_name=?1",
        rusqlite::params![kb_name, ncomm as i64],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;

    MEM_CACHE.write().remove(&kb_name);
    Ok(KgCommunityResult { kb_name, communities: ncomm, nodes: n })
}

#[derive(serde::Serialize)]
pub struct KgCommunityInfo {
    pub community_id: i64,
    pub label: String,
    pub size: i64,
    pub centroid_node: String,
    pub color: String,
}

/// 命令:读取社区列表(供前端着色与折叠 LOD)。
#[tauri::command]
pub fn kg_communities(kb_name: String) -> Result<Vec<KgCommunityInfo>, String> {
    let conn = ensure_kg_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT community_id,COALESCE(label,''),size,COALESCE(centroid_node,''),COALESCE(color,'') \
             FROM kg_communities WHERE kb_name=?1 AND level=0 ORDER BY size DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([&kb_name], |r| {
            Ok(KgCommunityInfo {
                community_id: r.get(0)?,
                label: r.get(1)?,
                size: r.get(2)?,
                centroid_node: r.get(3)?,
                color: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

// ─────────────────────── 布局坐标固化(kg_layout)───────────────────────
//
// 前端用 d3-force-3d 算好坐标后回存,下次秒加载、稳定可复现,支撑平滑转场。
// 按 view(relation/tag/time/importance)分别缓存。

#[derive(serde::Deserialize)]
pub struct LayoutPoint {
    pub node_id: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// 命令:保存某视角的布局坐标(覆盖式 upsert)。
#[tauri::command]
pub fn kg_save_layout(kb_name: String, view: String, points: Vec<LayoutPoint>) -> Result<usize, String> {
    let view = if view.trim().is_empty() { "relation".to_string() } else { view };
    let mut conn = ensure_kg_db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO kg_layout (kb_name,node_id,view,x,y,z) VALUES (?1,?2,?3,?4,?5,?6) \
                 ON CONFLICT(kb_name,node_id,view) DO UPDATE SET x=excluded.x,y=excluded.y,z=excluded.z",
            )
            .map_err(|e| e.to_string())?;
        for p in &points {
            stmt.execute(rusqlite::params![kb_name, p.node_id, view, p.x, p.y, p.z])
                .map_err(|e| e.to_string())?;
        }
    }
    tx.execute(
        "UPDATE kg_build_state SET layout_version = layout_version + 1 WHERE kb_name=?1",
        [&kb_name],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(points.len())
}

#[derive(serde::Serialize)]
pub struct LayoutOut {
    pub node_id: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// 在内存图上做 3D 力导向布局(Fruchterman-Reingold 变体,rayon 并行斥力)。
/// 把最重的布局计算放到 Rust 后台,前端零开销直接取坐标。结果写入 kg_layout 缓存。
fn force_layout_3d(mem: &MemGraph, iterations: usize) -> Vec<(String, f64, f64, f64)> {
    let n = mem.nodes.len();
    if n == 0 {
        return Vec::new();
    }
    // 初始位置:黄金角球面散布(确定性,避免 Math.random;同库可复现)
    let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    let r0 = 10.0 + (n as f64).sqrt() * 6.0;
    let mut px: Vec<f64> = Vec::with_capacity(n);
    let mut py: Vec<f64> = Vec::with_capacity(n);
    let mut pz: Vec<f64> = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f64 / n as f64;
        let theta = i as f64 * golden;
        let phi = (1.0 - 2.0 * t).acos();
        px.push(r0 * phi.sin() * theta.cos());
        py.push(r0 * (1.0 - 2.0 * t));
        pz.push(r0 * phi.sin() * theta.sin());
    }

    // 边表(索引化)
    let mut elinks: Vec<(usize, usize, f64)> = Vec::new();
    for e in &mem.edges {
        if let (Some(&a), Some(&b)) = (mem.pos.get(&e.from), mem.pos.get(&e.to)) {
            if a != b {
                elinks.push((a, b, e.strength.max(0.05)));
            }
        }
    }

    // FR 参数:理想边长 k,斥力 k^2/d,引力 d^2/k
    let area = (n as f64) * 800.0;
    let k = (area / n as f64).cbrt().max(12.0);
    let mut temp = k * 2.0;
    let cool = temp / (iterations as f64 + 1.0);

    for _ in 0..iterations {
        // 斥力(并行):每个节点对其余节点求斥力位移。O(n^2),但在 Rust+rayon 下数千节点可接受。
        let disp: Vec<(f64, f64, f64)> = (0..n)
            .into_par_iter()
            .map(|i| {
                let (xi, yi, zi) = (px[i], py[i], pz[i]);
                let (mut dx, mut dy, mut dz) = (0.0, 0.0, 0.0);
                for j in 0..n {
                    if i == j {
                        continue;
                    }
                    let mut ddx = xi - px[j];
                    let mut ddy = yi - py[j];
                    let mut ddz = zi - pz[j];
                    let mut dist2 = ddx * ddx + ddy * ddy + ddz * ddz;
                    if dist2 < 0.01 {
                        // 重合时给一个确定性微扰
                        ddx = ((i * 13 + 7) % 17) as f64 - 8.0;
                        ddy = ((i * 7 + 3) % 17) as f64 - 8.0;
                        ddz = ((i * 11 + 5) % 17) as f64 - 8.0;
                        dist2 = ddx * ddx + ddy * ddy + ddz * ddz + 0.01;
                    }
                    let dist = dist2.sqrt();
                    let force = (k * k) / dist; // 斥力
                    dx += (ddx / dist) * force;
                    dy += (ddy / dist) * force;
                    dz += (ddz / dist) * force;
                }
                (dx, dy, dz)
            })
            .collect();

        let mut dxv: Vec<f64> = disp.iter().map(|d| d.0).collect();
        let mut dyv: Vec<f64> = disp.iter().map(|d| d.1).collect();
        let mut dzv: Vec<f64> = disp.iter().map(|d| d.2).collect();

        // 引力(沿边,串行累加避免数据竞争)
        for &(a, b, s) in &elinks {
            let ddx = px[a] - px[b];
            let ddy = py[a] - py[b];
            let ddz = pz[a] - pz[b];
            let dist = (ddx * ddx + ddy * ddy + ddz * ddz).sqrt().max(0.01);
            let force = (dist * dist) / k * s; // 引力(按边强度加权)
            let fx = (ddx / dist) * force;
            let fy = (ddy / dist) * force;
            let fz = (ddz / dist) * force;
            dxv[a] -= fx;
            dyv[a] -= fy;
            dzv[a] -= fz;
            dxv[b] += fx;
            dyv[b] += fy;
            dzv[b] += fz;
        }

        // 限幅(温度)并更新位置
        for i in 0..n {
            let d = (dxv[i] * dxv[i] + dyv[i] * dyv[i] + dzv[i] * dzv[i]).sqrt();
            if d > 1e-6 {
                let lim = d.min(temp);
                px[i] += (dxv[i] / d) * lim;
                py[i] += (dyv[i] / d) * lim;
                pz[i] += (dzv[i] / d) * lim;
            }
        }
        temp = (temp - cool).max(k * 0.05);
    }

    (0..n)
        .map(|i| (mem.nodes[i].id.clone(), px[i], py[i], pz[i]))
        .collect()
}

/// 补点时与既有节点保持的最小间距(节点最大半径 11,留出肉眼可辨的空隙)。
const LAYOUT_MIN_SEP: f64 = 14.0;
/// 判定"几乎完全重合"的阈值,用于修复历史缓存里叠在一起的节点。
const LAYOUT_OVERLAP_EPS: f64 = 1.5;

/// 稳定哈希(FNV-1a):由 node_id 决定落点相位。
///
/// 关键:相位**不能**用"本次缺失列表里的序号" —— 每轮增量补点都从 0 开始,
/// 会让每轮新增的第一个同类节点落在完全相同的点上(新目录精确重叠在上一次新增的目录里)。
/// 用 node_id 哈希则同一节点每次落点一致、不同节点天然错开。
fn stable_hash_u64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 空间网格:O(1) 邻近查询,用于补点时避免与已有节点重合。
struct PointGrid {
    cell: f64,
    map: HashMap<(i64, i64, i64), Vec<(f64, f64, f64)>>,
}

impl PointGrid {
    fn new(cell: f64) -> Self {
        Self { cell: cell.max(1.0), map: HashMap::new() }
    }

    fn key(&self, x: f64, y: f64, z: f64) -> (i64, i64, i64) {
        (
            (x / self.cell).floor() as i64,
            (y / self.cell).floor() as i64,
            (z / self.cell).floor() as i64,
        )
    }

    fn insert(&mut self, x: f64, y: f64, z: f64) {
        self.map.entry(self.key(x, y, z)).or_default().push((x, y, z));
    }

    /// 半径 r 内是否已有点(只扫 27 个邻格;要求 r <= cell)
    fn has_within(&self, x: f64, y: f64, z: f64, r: f64) -> bool {
        let (kx, ky, kz) = self.key(x, y, z);
        let r2 = r * r;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(pts) = self.map.get(&(kx + dx, ky + dy, kz + dz)) {
                        for (px, py, pz) in pts {
                            let d2 = (x - px).powi(2) + (y - py).powi(2) + (z - pz).powi(2);
                            if d2 < r2 {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }
}

/// 在 anchor 附近找一个与既有节点不重合的落点。相位来自 node_id 哈希(确定性),
/// 撞上已有节点就换相位并略微外扩,最多 24 次后接受当前候选。
fn free_spot_near(
    node_id: &str,
    anchor: (f64, f64, f64),
    base_spread: f64,
    grid: &PointGrid,
) -> (f64, f64, f64) {
    let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    let h = stable_hash_u64(node_id);
    let t = (((h >> 17) % 64) as f64 + 0.5) / 64.0;
    let phi = (1.0 - 2.0 * t).acos();
    let mut theta = golden * ((h % 100_003) as f64);
    let mut spread = base_spread.max(LAYOUT_MIN_SEP);
    let mut cand = (0.0, 0.0, 0.0);
    for _ in 0..24 {
        cand = (
            anchor.0 + spread * phi.sin() * theta.cos(),
            anchor.1 + spread * (1.0 - 2.0 * t) * 0.6,
            anchor.2 + spread * phi.sin() * theta.sin(),
        );
        if !grid.has_within(cand.0, cand.1, cand.2, LAYOUT_MIN_SEP) {
            return cand;
        }
        theta += golden;
        spread += LAYOUT_MIN_SEP * 0.35;
    }
    cand
}

/// 修复历史缓存里几乎完全重合的节点(早期补点算法留下的):按稳定顺序保留先出现者,
/// 把后来者挪到附近的空位。返回被移动的点,供调用方写回缓存。
fn dedupe_overlapping_layout(
    mem: &MemGraph,
    have: &mut HashMap<String, (f64, f64, f64)>,
) -> Vec<LayoutPoint> {
    let mut ordered: Vec<&MemNode> = mem.nodes.iter().filter(|n| have.contains_key(&n.id)).collect();
    // 稳定顺序:浅层优先(父节点先占位),同层按 id
    ordered.sort_by(|a, b| a.depth.cmp(&b.depth).then(a.id.cmp(&b.id)));

    let mut grid = PointGrid::new(LAYOUT_MIN_SEP);
    let mut moved = Vec::new();
    for n in ordered {
        let (x, y, z) = have[&n.id];
        if grid.has_within(x, y, z, LAYOUT_OVERLAP_EPS) {
            let p = free_spot_near(&n.id, (x, y, z), LAYOUT_MIN_SEP, &grid);
            have.insert(n.id.clone(), p);
            grid.insert(p.0, p.1, p.2);
            moved.push(LayoutPoint { node_id: n.id.clone(), x: p.0, y: p.1, z: p.2 });
        } else {
            grid.insert(x, y, z);
        }
    }
    moved
}

/// 缓存布局命中后的**增量补点**:只给缓存里没有坐标的新节点定位,已有节点坐标一动不动。
///
/// 为什么需要:文件新增后图里多了 file/section 节点,但 kg_layout 里没有它们的坐标。
/// 若直接返回缓存,前端会把这些节点全部落到 (0,0,0) —— 相互重叠、被大球吞掉、连边被跳过,
/// 表现为"新节点根本看不到"。这里让新节点贴着已定位的邻居(父目录/所属文件)落位,
/// 确定性、无随机,既能立刻看见又不会把整张图重排。
///
/// 返回新补的坐标(供调用方写回缓存),同时就地写入 `have`。
fn seed_missing_layout(
    mem: &MemGraph,
    have: &mut HashMap<String, (f64, f64, f64)>,
) -> Vec<LayoutPoint> {
    let mut missing: Vec<&MemNode> = mem.nodes.iter().filter(|n| !have.contains_key(&n.id)).collect();
    if missing.is_empty() {
        return Vec::new();
    }

    // 双向邻接(含 contains 层级),用于找"已定位的邻居"作为锚点
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in &mem.edges {
        adj.entry(e.from.as_str()).or_default().push(e.to.as_str());
        adj.entry(e.to.as_str()).or_default().push(e.from.as_str());
    }

    // 全局质心与外径:节点一个已定位邻居都没有时的兜底锚点(放到球壳上,不堆在中心)
    let (mut cx, mut cy, mut cz) = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut r_max = 0.0_f64;
    if !have.is_empty() {
        for (x, y, z) in have.values() {
            cx += *x;
            cy += *y;
            cz += *z;
        }
        let cnt = have.len() as f64;
        cx /= cnt;
        cy /= cnt;
        cz /= cnt;
        for (x, y, z) in have.values() {
            let d = ((x - cx).powi(2) + (y - cy).powi(2) + (z - cz).powi(2)).sqrt();
            if d > r_max {
                r_max = d;
            }
        }
    }

    // 浅层先放:父目录先拿到坐标,子文件/章节才能贴着父节点落位
    missing.sort_by(|a, b| a.depth.cmp(&b.depth).then(a.id.cmp(&b.id)));

    // 已定位节点进网格,补点时据此避让,保证新节点不会叠在任何既有节点上
    let mut grid = PointGrid::new(LAYOUT_MIN_SEP);
    for (x, y, z) in have.values() {
        grid.insert(*x, *y, *z);
    }

    let mut placed = Vec::with_capacity(missing.len());
    for n in missing.iter() {
        let (mut ax, mut ay, mut az, mut cnt) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
        if let Some(nb) = adj.get(n.id.as_str()) {
            for id in nb {
                if let Some(&(x, y, z)) = have.get(*id) {
                    ax += x;
                    ay += y;
                    az += z;
                    cnt += 1.0;
                }
            }
        }
        // 有邻居:贴着邻居质心,按类型给一个体面的间距;没邻居:落到全图外壳
        let (anchor, spread) = if cnt > 0.0 {
            (
                (ax / cnt, ay / cnt, az / cnt),
                match n.node_type.as_str() {
                    "directory" => 46.0,
                    "file" => 28.0,
                    "concept" => 24.0,
                    _ => 18.0,
                },
            )
        } else {
            ((cx, cy, cz), (r_max * 0.8).max(60.0))
        };
        let (x, y, z) = free_spot_near(&n.id, anchor, spread, &grid);
        grid.insert(x, y, z);
        have.insert(n.id.clone(), (x, y, z));
        placed.push(LayoutPoint { node_id: n.id.clone(), x, y, z });
    }
    placed
}

/// 命令:后端计算 relation 视角力导向布局并写入缓存,返回坐标。
/// force=false 时优先复用缓存:缓存覆盖全部节点则直接返回;只差少量新节点则增量补点;
/// 缺口太大(批量导入等)才全量重算。
#[tauri::command]
pub async fn kg_compute_layout(
    kb_name: String,
    view: Option<String>,
    force: Option<bool>,
) -> Result<Vec<LayoutOut>, String> {
    let view = view.unwrap_or_else(|| "relation".to_string());
    let view = if view.trim().is_empty() { "relation".to_string() } else { view };

    // 非 relation 视角后端暂不接管(径向/时间/聚类很轻,前端算),返回空让前端回退
    if view != "relation" {
        return Ok(Vec::new());
    }

    // 已有缓存且非强制 → 尽量复用(必要时增量补点)
    if !force.unwrap_or(false) {
        let cached = kg_load_layout(kb_name.clone(), view.clone())?;
        if !cached.is_empty() {
            let mem = load_mem_graph(&kb_name)?;
            let mut have: HashMap<String, (f64, f64, f64)> =
                cached.into_iter().map(|p| (p.node_id, (p.x, p.y, p.z))).collect();
            let missing = mem.nodes.iter().filter(|n| !have.contains_key(&n.id)).count();
            // 少量新增(新建/移动几个文件,最常见)→ 增量补点,已有布局保持稳定;
            // 缺口过大(批量导入、首次扩容)→ 落到下面全量重算,布局质量更好。
            let incremental_cap = std::cmp::max(64, mem.nodes.len() / 5);
            if missing == 0 || missing <= incremental_cap {
                if missing > 0 {
                    let added = seed_missing_layout(&mem, &mut have);
                    let added_count = added.len();
                    let _ = kg_save_layout(kb_name.clone(), view.clone(), added);
                    tracing::info!(
                        kb_name = %kb_name,
                        added = added_count,
                        total = mem.nodes.len(),
                        "kg_compute_layout: 复用缓存并为新节点增量补点"
                    );
                }
                // 修复历史缓存里几乎完全重合的节点(早期补点算法会让每轮新增的第一个节点叠在一起)
                let moved = dedupe_overlapping_layout(&mem, &mut have);
                if !moved.is_empty() {
                    let moved_count = moved.len();
                    let _ = kg_save_layout(kb_name.clone(), view.clone(), moved);
                    tracing::info!(
                        kb_name = %kb_name,
                        moved = moved_count,
                        "kg_compute_layout: 已拆开重合节点"
                    );
                }
                // 只返回当前图里仍存在的节点(缓存可能残留已删除节点的坐标)
                return Ok(mem
                    .nodes
                    .iter()
                    .filter_map(|n| {
                        have.get(&n.id).map(|&(x, y, z)| LayoutOut {
                            node_id: n.id.clone(),
                            x,
                            y,
                            z,
                        })
                    })
                    .collect());
            }
            tracing::info!(
                kb_name = %kb_name,
                missing,
                total = mem.nodes.len(),
                "kg_compute_layout: 缓存缺口过大,全量重算布局"
            );
        }
    }

    let mem = load_mem_graph(&kb_name)?;
    let n = mem.nodes.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    // 规模自适应迭代次数(节点越多迭代略减,控制总耗时)
    let iterations = if n > 4000 {
        80
    } else if n > 1500 {
        120
    } else {
        180
    };

    let coords = force_layout_3d(&mem, iterations);
    let points: Vec<LayoutPoint> = coords
        .iter()
        .map(|(id, x, y, z)| LayoutPoint { node_id: id.clone(), x: *x, y: *y, z: *z })
        .collect();
    // 写入缓存(复用 kg_save_layout 逻辑)
    let _ = kg_save_layout(kb_name.clone(), view.clone(), points);

    Ok(coords
        .into_iter()
        .map(|(node_id, x, y, z)| LayoutOut { node_id, x, y, z })
        .collect())
}

/// 命令:读取某视角已缓存的布局坐标。
#[tauri::command]
pub fn kg_load_layout(kb_name: String, view: String) -> Result<Vec<LayoutOut>, String> {
    let view = if view.trim().is_empty() { "relation".to_string() } else { view };
    let conn = ensure_kg_db()?;
    let mut stmt = conn
        .prepare("SELECT node_id,x,y,z FROM kg_layout WHERE kb_name=?1 AND view=?2")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![kb_name, view], |r| {
            Ok(LayoutOut { node_id: r.get(0)?, x: r.get(1)?, y: r.get(2)?, z: r.get(3)? })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[cfg(test)]
mod layout_seed_tests {
    use super::*;

    /// 回归:两个不同节点即使锚点、间距完全相同(同一个父目录下的两个新目录),
    /// 也必须落在互不重合的位置 —— 早期实现用"缺失列表序号"当相位,
    /// 导致每轮新增的第一个节点精确叠在上一轮那个之上,新目录整个藏进旧目录里。
    #[test]
    fn seeded_siblings_never_collide() {
        let anchor = (10.0, 20.0, 30.0);
        let mut grid = PointGrid::new(LAYOUT_MIN_SEP);
        grid.insert(anchor.0, anchor.1, anchor.2); // 父节点本身

        let a = free_spot_near("dir_tte", anchor, 46.0, &grid);
        grid.insert(a.0, a.1, a.2);
        let b = free_spot_near("dir_yui", anchor, 46.0, &grid);

        let d = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)).sqrt();
        assert!(d >= LAYOUT_MIN_SEP, "两个新目录距离 {d} 小于最小间距 {LAYOUT_MIN_SEP}");
    }

    /// 同一节点每次落点必须一致(布局可复现,切换视角/重载不会乱跳)。
    #[test]
    fn seeding_is_deterministic() {
        let anchor = (0.0, 0.0, 0.0);
        let grid = PointGrid::new(LAYOUT_MIN_SEP);
        let p1 = free_spot_near("file_abc", anchor, 28.0, &grid);
        let p2 = free_spot_near("file_abc", anchor, 28.0, &grid);
        assert_eq!(p1, p2);
    }

    /// 网格邻近查询要能跨格命中(点落在格子边界附近时不能漏判)。
    #[test]
    fn grid_detects_neighbor_across_cells() {
        let mut grid = PointGrid::new(LAYOUT_MIN_SEP);
        grid.insert(13.9, 0.0, 0.0); // 紧贴 cell 边界
        assert!(grid.has_within(14.1, 0.0, 0.0, LAYOUT_MIN_SEP));
        assert!(!grid.has_within(100.0, 0.0, 0.0, LAYOUT_MIN_SEP));
    }
}

#[cfg(test)]
mod extract_tests {
    use super::*;

    #[test]
    fn wikilinks_basic_alias_anchor() {
        let r = extract_refs("a.md", "see [[Other Note]] and [[Page|别名]] and [[Doc#Heading]]");
        let ts: Vec<_> = r.refs.iter().map(|x| (x.kind, x.target.clone(), x.anchor.clone())).collect();
        assert!(ts.contains(&(RefKind::Wikilink, "Other Note".to_string(), None)));
        assert!(ts.contains(&(RefKind::Wikilink, "Page".to_string(), None)));
        assert!(ts.contains(&(RefKind::Wikilink, "Doc".to_string(), Some("Heading".to_string()))));
    }

    #[test]
    fn md_links_skip_images_and_external() {
        let r = extract_refs("a.md", "![img](pic.png) [note](sub/b.md#sec) [ext](https://x.com) [asset](f.pdf)");
        let targets: Vec<_> = r.refs.iter().map(|x| x.target.clone()).collect();
        assert_eq!(targets, vec!["sub/b.md".to_string()]);
        assert_eq!(r.refs[0].anchor, Some("sec".to_string()));
    }

    #[test]
    fn tags_skip_headings_code_numeric() {
        let content = "# Heading not a tag\nbody #ml #深度学习/cnn here\n`#incode` end\n#123 not";
        let r = extract_refs("a.md", content);
        let tags: Vec<_> = r.tags.iter().map(|t| t.tag.clone()).collect();
        assert!(tags.contains(&"ml".to_string()));
        assert!(tags.contains(&"深度学习/cnn".to_string()));
        assert!(!tags.iter().any(|t| t == "incode"));
        assert!(!tags.iter().any(|t| t == "123"));
        assert!(!tags.iter().any(|t| t.starts_with("Heading")));
    }

    #[test]
    fn from_section_attribution() {
        let content = "## Section A\ntext [[Target]]\n";
        let r = extract_refs("a.md", content);
        assert_eq!(r.refs.len(), 1);
        assert!(r.refs[0].from_section.is_some());
        let expect = sha256_hex("a.md\nH2\nL1\nSection A");
        assert_eq!(r.refs[0].from_section.as_deref(), Some(expect.as_str()));
    }

    #[test]
    fn resolver_wikilink_by_stem_and_anchor() {
        let mut acc = BuildAccum::default();
        acc.file_by_stem.insert("noteb".into(), "file_B".into());
        acc.file_by_relpath.insert("noteb.md".into(), "file_B".into());
        acc.section_in_file.insert(("file_B".into(), "intro".into()), "section_x".into());

        // A 链接到 B(按 stem),以及 B#Intro(锚点解析到 section)
        acc.file_refs.push(FileRefs {
            src_file_rel: "a.md".into(),
            src_file_node: "file_A".into(),
            refs: ExtractedRefs {
                refs: vec![
                    RawRef { kind: RefKind::Wikilink, target: "NoteB".into(), anchor: None, from_section: None, line: 1 },
                    RawRef { kind: RefKind::Wikilink, target: "NoteB".into(), anchor: Some("Intro".into()), from_section: None, line: 2 },
                ],
                tags: vec![],
            },
        });

        resolve_refs_to_edges(&mut acc);
        let links: Vec<(String, String)> = acc
            .edges
            .iter()
            .filter(|e| e.edge_type == "link")
            .map(|e| (e.from_id.clone(), e.to_id.clone()))
            .collect();
        assert!(links.contains(&("file_A".into(), "file_B".into())));
        assert!(links.contains(&("file_A".into(), "section_x".into())));
    }

    #[test]
    fn end_to_end_build_into_memory_db() {
        let dir = tempfile::tempdir().unwrap();
        let kb = dir.path();
        fs::write(
            kb.join("a.md"),
            "# Topic A\n\nlinks to [[b]] and #ml here.\n\n## Sub\n\nbody text\n",
        )
        .unwrap();
        fs::write(kb.join("b.md"), "# Topic B\n\nsome content about b\n").unwrap();

        let mut conn = Connection::open_in_memory().unwrap();
        init_kg_schema(&conn).unwrap();
        let (nc, ec) = build_into_conn(&mut conn, "TestKB", kb).unwrap();
        assert!(nc >= 6, "expected kb+2 files+2 sections+paras, got {nc}");
        assert!(ec >= 4, "expected contains+link edges, got {ec}");

        // 校验 a.md → b.md 的 link 边存在
        let link_cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM kg_edges WHERE kb_name='TestKB' AND edge_type='link'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(link_cnt >= 1, "wikilink [[b]] 应生成 1 条 link 边");

        // 校验 #ml 标签落库
        let tag_cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM kg_node_tags WHERE kb_name='TestKB' AND tag='ml'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tag_cnt, 1, "#ml 应作为标签落库");

        // 校验 contains 层级:kb_root → file
        let contains_cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM kg_edges WHERE kb_name='TestKB' AND edge_type='contains' AND from_id='kb_root'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(contains_cnt, 2, "kb_root 应 contains 两个文件");
    }

    #[test]
    fn blob_roundtrip_and_search() {
        let v = vec![0.1f32, -0.2, 0.3, 0.9];
        let b = vec_to_blob(&v);
        assert_eq!(blob_to_vec(&b), v);

        // 构造一个小向量库:q 与 n2 最接近
        let store = VecStore {
            ids: vec!["n1".into(), "n2".into(), "n3".into()],
            types: vec!["paragraph".into(), "paragraph".into(), "section".into()],
            data: vec![
                1.0, 0.0, 0.0, // n1
                0.0, 1.0, 0.0, // n2
                0.0, 0.0, 1.0, // n3
            ],
            dim: 3,
            pos: HashMap::new(),
            model: "m".into(),
            built_at: "t".into(),
            emb_count: 3,
        };
        let q = vec![0.0f32, 0.9, 0.1];
        let hits = vec_search(&store, &q, 2, &None, &HashSet::new());
        assert_eq!(hits[0].0, "n2", "最相近应为 n2");

        // 类型过滤:只要 section
        let only_sec = vec_search(&store, &q, 5, &Some(vec!["section".into()]), &HashSet::new());
        assert!(only_sec.iter().all(|(id, _)| id == "n3"));

        // 排除集合
        let excl: HashSet<String> = ["n2".to_string()].into_iter().collect();
        let no_n2 = vec_search(&store, &q, 5, &None, &excl);
        assert!(no_n2.iter().all(|(id, _)| id != "n2"));
    }

    #[test]
    fn louvain_finds_two_clusters() {
        // 两个三角形(0-1-2)与(3-4-5),只有一条弱桥 2-3 连接 → 应分成 2 个社区
        let edges = vec![
            (0, 1, 1.0), (1, 2, 1.0), (0, 2, 1.0),
            (3, 4, 1.0), (4, 5, 1.0), (3, 5, 1.0),
            (2, 3, 0.05),
        ];
        let comm = louvain(6, &edges, 1.0);
        assert_eq!(comm[0], comm[1]);
        assert_eq!(comm[1], comm[2]);
        assert_eq!(comm[3], comm[4]);
        assert_eq!(comm[4], comm[5]);
        assert_ne!(comm[2], comm[3], "两个三角形应属不同社区");
    }
}
