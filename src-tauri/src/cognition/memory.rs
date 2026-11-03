//! 记忆层：把活动、情节与摘录沉淀成**可检索、可编辑**的长期记忆。
//!
//! 三件事决定这一层的形状：
//! 1. **真相拆成两半**。`memory.md` 的手动块是用户可改可删的（与 `profile.md` 同一思路），
//!    自动块由巩固任务重建；DB 里的派生记忆只是索引，删库后可从 markdown 与来源重建。
//! 2. **巩固走两条腿**。规则抽取（读完的书、摘录）保证没有模型也能工作；模型抽取
//!    （事实/偏好/疑问/技能/目标）在预算允许时才跑，失败不影响规则那部分。
//! 3. **忘记要粘住**。被退休的记忆保留 text_hash 作墓碑——同一句话再被抽出来也不会复活，
//!    除非用户自己把它写回 memory.md 的手动块。
//!
//! 召回是第五路：memory 通道按向量取活跃记忆，与语义/图/感知/情节一起 RRF 融合。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use tracing::{debug, info};

use crate::cognition::store::{blob_to_vec, ensure_db, hash_hex, now_iso, vec_to_blob};

/// 允许的记忆种类。与设置页的下拉、markdown 的中文标签一一对应。
pub const MEMORY_KINDS: [&str; 5] = ["fact", "preference", "question", "skill", "goal"];

const KIND_LABELS: [(&str, &str); 5] = [
    ("fact", "事实"),
    ("preference", "偏好"),
    ("question", "疑问"),
    ("skill", "技能"),
    ("goal", "目标"),
];

/// 合并的向量相似度闸门：同一件事换个说法，不该占两条记忆。
const MERGE_SIM: f32 = 0.92;
/// 召回通道的最低相似度。
const RECALL_SIM_MIN: f32 = 0.18;
/// 单条记忆的文本上限。
const MAX_TEXT_CHARS: usize = 1000;
/// 巩固输入的上限，避免把整天原文塞进提示词。
const MAX_DIGEST_CHARS: usize = 6000;
/// 未再次出现多久后退休低置信度的派生记忆。
const STALE_DAYS: i64 = 180;

const MANUAL_START: &str = "<!-- guaxi:manual:start -->";
const MANUAL_END: &str = "<!-- guaxi:manual:end -->";
const AUTO_START: &str = "<!-- guaxi:auto:start -->";
const AUTO_END: &str = "<!-- guaxi:auto:end -->";

const FILE_TEMPLATE: &str = "# 记忆\n\
\n\
> 这份文件是大脑对你的长期记忆。手动块里的条目你可以随时增删改——一行一条，\n\
> 可以写成 `- [偏好] …`（种类：事实 / 偏好 / 疑问 / 技能 / 目标），不写种类按「事实」。\n\
> 自动块由巩固任务重建，不要手改；删除自动块里的条目请用「忘记」，否则下次会被重建。\n\
\n\
<!-- guaxi:manual:start -->\n\
<!-- guaxi:manual:end -->\n\
\n\
<!-- guaxi:auto:start -->\n\
<!-- guaxi:auto:end -->\n";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Memory {
    pub id: String,
    pub kind: String,
    pub text: String,
    pub confidence: f64,
    pub status: String,
    /// manual | derived | note | read | episode
    pub source: String,
    pub topics: Vec<String>,
    pub refs: Value,
    pub hits: i64,
    pub created_at: String,
    pub updated_at: String,
    pub last_seen_at: Option<String>,
    pub embedded: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConsolidateReport {
    pub window_start: String,
    pub candidates: usize,
    pub created: usize,
    pub merged: usize,
    pub retired: usize,
    pub used_llm: bool,
    pub note: String,
}

#[derive(Debug, Clone)]
struct Candidate {
    kind: String,
    text: String,
    confidence: f64,
    source: String,
    topics: Vec<String>,
    refs: Value,
}

fn kind_label(kind: &str) -> &'static str {
    KIND_LABELS
        .iter()
        .find(|(k, _)| *k == kind)
        .map(|(_, label)| *label)
        .unwrap_or("事实")
}

fn normalize_kind(kind: &str) -> String {
    let k = kind.trim().to_lowercase();
    if MEMORY_KINDS.contains(&k.as_str()) {
        k
    } else {
        // 也接受中文标签。
        KIND_LABELS
            .iter()
            .find(|(_, label)| *label == kind.trim())
            .map(|(k, _)| (*k).to_string())
            .unwrap_or_else(|| "fact".to_string())
    }
}

fn normalize_text(raw: &str) -> String {
    let collapsed = raw
        .replace("\r\n", "\n")
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    collapsed.chars().take(MAX_TEXT_CHARS).collect()
}

fn text_hash(text: &str) -> String {
    hash_hex(&normalize_text(text))
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0f32, 0f32, 0f32);
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

fn row_to_memory(r: &rusqlite::Row<'_>) -> rusqlite::Result<Memory> {
    let topics_raw: String = r.get(6)?;
    let refs_raw: String = r.get(7)?;
    let vec_blob: Option<Vec<u8>> = r.get(11)?;
    Ok(Memory {
        id: r.get(0)?,
        kind: r.get(1)?,
        text: r.get(2)?,
        confidence: r.get(3)?,
        status: r.get(4)?,
        source: r.get(5)?,
        topics: serde_json::from_str(&topics_raw).unwrap_or_default(),
        refs: serde_json::from_str(&refs_raw).unwrap_or_else(|_| json!([])),
        hits: r.get(8)?,
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
        last_seen_at: r.get(12)?,
        embedded: vec_blob.map(|b| !b.is_empty()).unwrap_or(false),
    })
}

const SELECT_COLUMNS: &str =
    "id,kind,text,confidence,status,source,topics,refs,hits,created_at,updated_at,vec,last_seen_at";

/// 列出记忆。默认只看活跃的。
pub fn list(kind: Option<&str>, status: Option<&str>, limit: Option<usize>) -> Result<Vec<Memory>, String> {
    let conn = ensure_db()?;
    let limit = limit.unwrap_or(100).clamp(1, 500) as i64;
    let mut sql = format!("SELECT {SELECT_COLUMNS} FROM memories WHERE 1=1");
    let mut values: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(s) = status.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if s != "all" {
            sql.push_str(" AND status=?");
            values.push(rusqlite::types::Value::Text(s.to_string()));
        }
    } else {
        sql.push_str(" AND status='active'");
    }
    if let Some(k) = kind.map(normalize_kind).filter(|k| !k.is_empty()) {
        sql.push_str(" AND kind=?");
        values.push(rusqlite::types::Value::Text(k));
    }
    sql.push_str(" ORDER BY updated_at DESC LIMIT ?");
    values.push(rusqlite::types::Value::Integer(limit));
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(values), row_to_memory)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn fetch_by_id(id: &str) -> Result<Memory, String> {
    let conn = ensure_db()?;
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM memories WHERE id=?1"),
        params![id],
        row_to_memory,
    )
    .map_err(|e| e.to_string())
}

/// 手动添加一条记忆（会同步进 memory.md 的手动块）。
pub fn add_manual(kind: &str, text: &str, confidence: Option<f64>) -> Result<Memory, String> {
    let text = normalize_text(text);
    if text.chars().count() < 2 {
        return Err("记忆内容太短".to_string());
    }
    let kind = normalize_kind(kind);
    let hash = text_hash(&text);
    let confidence = confidence.unwrap_or(0.95).clamp(0.1, 1.0);
    let conn = ensure_db()?;
    let existing: Option<(String, String)> = conn
        .query_row(
            "SELECT id,status FROM memories WHERE text_hash=?1",
            params![hash],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let id = match existing {
        Some((id, _)) => {
            conn.execute(
                "UPDATE memories SET kind=?2,status='active',source='manual',confidence=?3,\
                 updated_at=?4,last_seen_at=?4,text=?5 WHERE id=?1",
                params![id, kind, confidence, now_iso(), text],
            )
            .map_err(|e| e.to_string())?;
            id
        }
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO memories(id,kind,text,text_hash,confidence,status,source,topics,refs,\
                 hits,created_at,updated_at,last_seen_at) \
                 VALUES(?1,?2,?3,?4,?5,'active','manual','[]','[]',0,?6,?6,?6)",
                params![id, kind, text, hash, confidence, now_iso()],
            )
            .map_err(|e| e.to_string())?;
            id
        }
    };
    drop(conn);
    // 手动块要真的出现在 markdown 里，用户下次打开文件能看见自己写了什么。
    let _ = append_manual_line(&kind, &text);
    let _ = sync_to_markdown();
    fetch_by_id(&id)
}

/// 忘记一条记忆。手动记忆同时从 markdown 手动块里删掉该行，避免被同步复活。
pub fn forget(id: &str) -> Result<(), String> {
    let conn = ensure_db()?;
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT source,text FROM memories WHERE id=?1",
            params![id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((source, text)) = row else {
        return Err("记忆不存在".to_string());
    };
    conn.execute(
        "UPDATE memories SET status='retired',updated_at=?2 WHERE id=?1",
        params![id, now_iso()],
    )
    .map_err(|e| e.to_string())?;
    drop(conn);
    if source == "manual" {
        let _ = remove_manual_line(&text);
    }
    let _ = sync_to_markdown();
    Ok(())
}

/// 为没有向量的活跃记忆补 embedding（后台循环调用）。
pub async fn embed_pending(limit: usize) -> Result<usize, String> {
    let rows: Vec<(String, String)> = {
        let conn = ensure_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT id,text FROM memories WHERE status='active' AND vec IS NULL \
                 ORDER BY updated_at DESC LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let it = stmt
            .query_map(params![limit.clamp(1, 64) as i64], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        it.filter_map(|x| x.ok()).collect()
    };
    if rows.is_empty() {
        return Ok(0);
    }
    let texts: Vec<String> = rows.iter().map(|(_, t)| t.clone()).collect();
    let vecs = crate::commands::embedding::embed_texts(&texts, false).await?;
    let conn = ensure_db()?;
    let mut written = 0usize;
    for (idx, (id, _)) in rows.iter().enumerate() {
        let Some(vec) = vecs.get(idx) else { continue };
        conn.execute(
            "UPDATE memories SET vec=?2 WHERE id=?1",
            params![id, vec_to_blob(vec)],
        )
        .map_err(|e| e.to_string())?;
        written += 1;
    }
    Ok(written)
}

/// 按查询向量取活跃记忆（召回第五路）。
pub fn topk(query: &[f32], k: usize) -> Vec<(Memory, f32)> {
    let Ok(conn) = ensure_db() else {
        return Vec::new();
    };
    let mut stmt = match conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM memories WHERE status='active' AND vec IS NOT NULL \
         ORDER BY updated_at DESC LIMIT 5000"
    )) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map([], |r| {
        Ok((
            row_to_memory(r)?,
            r.get::<_, Option<Vec<u8>>>(11)?,
        ))
    }) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };
    let mut scored: Vec<(Memory, f32)> = Vec::new();
    for row in rows.flatten() {
        let (memory, blob) = row;
        let Some(blob) = blob else { continue };
        let vec = blob_to_vec(&blob);
        let sim = cosine(query, &vec);
        if sim >= RECALL_SIM_MIN {
            scored.push((memory, sim));
        }
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(k.clamp(1, 20));
    scored
}

// ───────────────────────── memory.md ─────────────────────────

pub fn memory_file_path() -> Result<PathBuf, String> {
    Ok(crate::cognition::store::cog_data_dir()?.join("memory.md"))
}

fn read_or_init() -> Result<String, String> {
    let path = memory_file_path()?;
    if !path.exists() {
        fs::write(&path, FILE_TEMPLATE).map_err(|e| format!("写入 memory.md 失败: {e}"))?;
    }
    fs::read_to_string(&path).map_err(|e| format!("读取 memory.md 失败: {e}"))
}

fn extract_block(text: &str, start: &str, end: &str) -> Option<String> {
    let s = text.find(start)? + start.len();
    let e = text[s..].find(end)? + s;
    Some(text[s..e].to_string())
}

fn replace_block(text: &str, start: &str, end: &str, body: &str) -> String {
    if let (Some(s), Some(e)) = (text.find(start), text.find(end)) {
        if e > s {
            let mut out = String::new();
            out.push_str(&text[..s + start.len()]);
            out.push('\n');
            out.push_str(body.trim_end());
            out.push('\n');
            out.push_str(&text[e..]);
            return out;
        }
    }
    format!("{text}\n{start}\n{}\n{end}\n", body.trim_end())
}

fn parse_manual_block(block: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in block.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("- ") else {
            continue;
        };
        let rest = rest.trim();
        if rest.is_empty() {
            continue;
        }
        if let Some(after) = rest.strip_prefix('[') {
            if let Some(close) = after.find(']') {
                let label = &after[..close];
                let text = after[close + 1..].trim();
                if !text.is_empty() {
                    out.push((normalize_kind(label), normalize_text(text)));
                }
                continue;
            }
        }
        out.push(("fact".to_string(), normalize_text(rest)));
    }
    out
}

fn append_manual_line(kind: &str, text: &str) -> Result<(), String> {
    let content = read_or_init()?;
    let block = extract_block(&content, MANUAL_START, MANUAL_END).unwrap_or_default();
    if parse_manual_block(&block).iter().any(|(_, t)| t == text) {
        return Ok(());
    }
    let line = format!("- [{}] {}", kind_label(kind), text);
    let next = if block.trim().is_empty() {
        format!("\n{line}\n")
    } else {
        format!("{block}\n{line}\n")
    };
    let updated = replace_block(&content, MANUAL_START, MANUAL_END, &next);
    let path = memory_file_path()?;
    fs::write(&path, updated).map_err(|e| e.to_string())
}

fn remove_manual_line(text: &str) -> Result<(), String> {
    let content = read_or_init()?;
    let Some(block) = extract_block(&content, MANUAL_START, MANUAL_END) else {
        return Ok(());
    };
    let target = normalize_text(text);
    let kept: Vec<String> = block
        .lines()
        .filter(|line| {
            let parsed = parse_manual_block(line);
            !parsed.iter().any(|(_, t)| *t == target)
        })
        .map(|l| l.to_string())
        .collect();
    let updated = replace_block(&content, MANUAL_START, MANUAL_END, &kept.join("\n"));
    let path = memory_file_path()?;
    fs::write(&path, updated).map_err(|e| e.to_string())
}

/// 把 memory.md 手动块的编辑同步进 DB：新增/改类型 → active，被删掉的手动记忆 → retired。
pub fn sync_manual_from_markdown() -> Result<usize, String> {
    let content = read_or_init()?;
    let block = extract_block(&content, MANUAL_START, MANUAL_END).unwrap_or_default();
    let parsed = parse_manual_block(&block);
    let conn = ensure_db()?;
    let mut keep: HashSet<String> = HashSet::new();
    let mut changed = 0usize;
    for (kind, text) in &parsed {
        let hash = text_hash(text);
        keep.insert(hash.clone());
        let existing: Option<(String, String)> = conn
            .query_row(
                "SELECT id,status FROM memories WHERE text_hash=?1",
                params![hash],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some((id, status)) = existing {
            conn.execute(
                "UPDATE memories SET kind=?2,status='active',source='manual',updated_at=?3,\
                 last_seen_at=?3 WHERE id=?1",
                params![id, kind, now_iso()],
            )
            .map_err(|e| e.to_string())?;
            if status != "active" {
                changed += 1;
            }
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO memories(id,kind,text,text_hash,confidence,status,source,topics,refs,\
                 hits,created_at,updated_at,last_seen_at) \
                 VALUES(?1,?2,?3,?4,0.95,'active','manual','[]','[]',0,?5,?5,?5)",
                params![id, kind, text, hash, now_iso()],
            )
            .map_err(|e| e.to_string())?;
            changed += 1;
        }
    }
    // 文件里已经删掉的手动条目 → 退休。
    let manual: Vec<(String, String)> = {
        let mut stmt = conn
            .prepare("SELECT id,text_hash FROM memories WHERE source='manual' AND status='active'")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };
    for (id, hash) in manual {
        if !keep.contains(&hash) {
            conn.execute(
                "UPDATE memories SET status='retired',updated_at=?2 WHERE id=?1",
                params![id, now_iso()],
            )
            .ok();
            changed += 1;
        }
    }
    Ok(changed)
}

/// 重建 memory.md 的自动块（手动块原样保留）。
pub fn sync_to_markdown() -> Result<(), String> {
    let content = read_or_init()?;
    let conn = ensure_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT kind,text FROM memories WHERE status='active' AND source<>'manual' \
             ORDER BY updated_at DESC LIMIT 300",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    let mut by_kind: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows.flatten() {
        by_kind.entry(row.0).or_default().push(row.1);
    }
    let mut body = String::new();
    for (kind, _) in KIND_LABELS {
        let Some(items) = by_kind.get(kind) else { continue };
        if items.is_empty() {
            continue;
        }
        body.push_str(&format!("### {}\n", kind_label(kind)));
        for text in items {
            body.push_str(&format!("- {}\n", text.trim()));
        }
        body.push('\n');
    }
    if body.trim().is_empty() {
        body.push_str("_还没有自动记忆。读完一本书、摘一段、或者让巩固任务跑一次就会长出来。_\n");
    }
    let updated = replace_block(&content, AUTO_START, AUTO_END, &body);
    let path = memory_file_path()?;
    fs::write(&path, updated).map_err(|e| e.to_string())
}

// ───────────────────────── 巩固 ─────────────────────────

fn truncate(text: &str, max: usize) -> String {
    let one = text.split_whitespace().collect::<Vec<_>>().join(" ");
    one.chars().take(max).collect()
}

/// 收集自上次巩固以来的材料，压成一段给模型看的摘要。
fn build_digest(since: &str) -> String {
    let mut digest = String::new();
    let Ok(conn) = ensure_db() else { return digest };

    // 情节：读完的书 + 最近的问答。
    if let Ok(mut stmt) = conn.prepare(
        "SELECT kind,COALESCE(question,''),COALESCE(answer,'') FROM episodes \
         WHERE started_at>=?1 AND COALESCE(question,'')<>'' ORDER BY started_at DESC LIMIT 40",
    ) {
        if let Ok(rows) = stmt.query_map(params![since], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        }) {
            for (kind, question, answer) in rows.flatten() {
                if kind == "read" {
                    digest.push_str(&format!("读完：{}\n", truncate(&question, 120)));
                } else if !answer.trim().is_empty() {
                    digest.push_str(&format!(
                        "问：{}\n答：{}\n",
                        truncate(&question, 200),
                        truncate(&answer, 400)
                    ));
                }
            }
        }
    }

    // 活动：只在摘要里保留"做过什么"，不堆路径。
    if let Ok(mut stmt) = conn.prepare(
        "SELECT app,kind,COALESCE(target_id,''),COALESCE(meta,'') FROM activity \
         WHERE at>=?1 AND kind IN ('note','search','open','edit') ORDER BY id DESC LIMIT 120",
    ) {
        if let Ok(rows) = stmt.query_map(params![since], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        }) {
            let mut searches: Vec<String> = Vec::new();
            let mut notes: Vec<String> = Vec::new();
            let mut opens: Vec<String> = Vec::new();
            let mut edits: Vec<String> = Vec::new();
            for (app, kind, target, meta) in rows.flatten() {
                let name = serde_json::from_str::<Value>(&meta)
                    .ok()
                    .and_then(|m| {
                        m.get("fileName")
                            .or_else(|| m.get("importFileName"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| {
                        target
                            .replace('\\', "/")
                            .rsplit('/')
                            .next()
                            .unwrap_or("")
                            .to_string()
                    });
                match kind.as_str() {
                    "search" if !target.trim().is_empty() => searches.push(truncate(&target, 60)),
                    "note" => notes.push(truncate(&name, 80)),
                    "open" if app == "repo" => opens.push(truncate(&name, 80)),
                    "edit" => edits.push(truncate(&name, 80)),
                    _ => {}
                }
            }
            let uniq = |v: Vec<String>| -> Vec<String> {
                let mut seen = HashSet::new();
                v.into_iter().filter(|x| seen.insert(x.clone())).take(20).collect()
            };
            for s in uniq(searches) {
                digest.push_str(&format!("搜索：{}\n", s));
            }
            for s in uniq(opens) {
                digest.push_str(&format!("打开：{}\n", s));
            }
            for s in uniq(notes) {
                digest.push_str(&format!("摘录：{}\n", s));
            }
            for s in uniq(edits) {
                digest.push_str(&format!("编辑：{}\n", s));
            }
        }
    }

    if digest.chars().count() > MAX_DIGEST_CHARS {
        digest = digest.chars().take(MAX_DIGEST_CHARS).collect();
    }
    digest
}

/// 规则抽取：完全不依赖模型的那部分。读完的书与摘录就是最稳的记忆来源。
fn rule_candidates(since: &str) -> Vec<Candidate> {
    let mut out = Vec::new();
    let Ok(conn) = ensure_db() else { return out };

    if let Ok(mut stmt) = conn.prepare(
        "SELECT question FROM episodes WHERE kind='read' AND outcome='ok' \
         AND started_at>=?1 AND COALESCE(question,'')<>'' ORDER BY started_at DESC LIMIT 20",
    ) {
        if let Ok(rows) = stmt.query_map(params![since], |r| r.get::<_, String>(0)) {
            for title in rows.flatten() {
                let title = title.trim();
                if title.is_empty() {
                    continue;
                }
                out.push(Candidate {
                    kind: "fact".into(),
                    text: format!("读过《{}》", title),
                    confidence: 0.7,
                    source: "read".into(),
                    topics: Vec::new(),
                    refs: json!([]),
                });
            }
        }
    }

    if let Ok(mut stmt) = conn.prepare(
        "SELECT id,COALESCE(title,''),text FROM percepts WHERE source_kind='note' \
         AND state<>'rejected' AND captured_at>=?1 ORDER BY captured_at DESC LIMIT 12",
    ) {
        if let Ok(rows) = stmt.query_map(params![since], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        }) {
            for (id, title, text) in rows.flatten() {
                let body = truncate(&text, 160);
                if body.chars().count() < 8 {
                    continue;
                }
                let label = if title.trim().is_empty() {
                    "摘录".to_string()
                } else {
                    title
                };
                out.push(Candidate {
                    kind: "fact".into(),
                    text: format!("{}：{}", truncate(&label, 60), body),
                    confidence: 0.8,
                    source: "note".into(),
                    topics: Vec::new(),
                    refs: json!([{ "ref_kind": "percept", "ref_id": id }]),
                });
            }
        }
    }
    out
}

fn parse_llm_candidates(answer: &str) -> Vec<Candidate> {
    let Some(start) = answer.find('[') else {
        return Vec::new();
    };
    let Some(end) = answer.rfind(']') else {
        return Vec::new();
    };
    if end <= start {
        return Vec::new();
    }
    let json_slice = &answer[start..=end];
    let Ok(items) = serde_json::from_str::<Vec<Value>>(json_slice) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items.into_iter().take(8) {
        let text = item
            .get("text")
            .and_then(|v| v.as_str())
            .map(normalize_text)
            .unwrap_or_default();
        if text.chars().count() < 4 {
            continue;
        }
        let kind = normalize_kind(
            item.get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("fact"),
        );
        let confidence = item
            .get("confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.6)
            .clamp(0.3, 0.95);
        let topics: Vec<String> = item
            .get("topics")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t.as_str())
                    .map(|t| truncate(t, 24))
                    .filter(|t| !t.is_empty())
                    .take(6)
                    .collect()
            })
            .unwrap_or_default();
        out.push(Candidate {
            kind,
            text,
            confidence,
            source: "derived".into(),
            topics,
            refs: json!([]),
        });
    }
    out
}

async fn llm_candidates(digest: &str) -> Vec<Candidate> {
    if digest.trim().chars().count() < 40 {
        return Vec::new();
    }
    let budget_ok = crate::cognition::store::tokens_spent_today(None)
        < crate::cognition::store::get_i64("daily_token_budget", 60_000);
    if !budget_ok {
        debug!("[cognition] 记忆巩固跳过模型抽取：今日 token 预算已用完");
        return Vec::new();
    }
    let modelid = crate::cognition::store::get_i64("deliberate_modelid", 6) as i32;
    let prompt = format!(
        "从下面一天内的活动与问答记录里，抽取值得**长期记住**的用户信息。\n\
         只写稳定的东西：身份/事实、偏好、悬而未决的问题、正在练的技能、明确的目标。\n\
         不要写一次性的阅读进度、搜索词本身、寒暄与过程描述。\n\
         输出 JSON 数组（不要代码块、不要解释），元素形如：\n\
         {{\"kind\":\"fact|preference|question|skill|goal\",\"text\":\"一句中文，第一人称或'用户……'\",\"topics\":[\"主题\"],\"confidence\":0.0}}\n\
         最多 8 条；没有值得记的就输出 []。\n\n记录：\n{digest}"
    );
    match crate::commands::chatview::kg_llm_complete(modelid, &prompt, "cog-memory").await {
        Ok((answer, _)) => {
            crate::cognition::store::record_tokens(
                "memory",
                (answer.chars().count() / 3) as i64,
            );
            parse_llm_candidates(&answer)
        }
        Err(e) => {
            debug!("[cognition] 记忆巩固模型抽取失败: {}", e);
            Vec::new()
        }
    }
}

fn retire_stale() -> usize {
    let cutoff = (chrono::Local::now() - chrono::Duration::days(STALE_DAYS)).to_rfc3339();
    let Ok(conn) = ensure_db() else { return 0 };
    conn.execute(
        "UPDATE memories SET status='retired',updated_at=?2 \
         WHERE status='active' AND source<>'manual' AND confidence<0.85 \
         AND COALESCE(last_seen_at,updated_at) < ?1",
        params![cutoff, now_iso()],
    )
    .unwrap_or(0)
}

/// 合并候选：先按文本去重，再按向量相似度合并到已有记忆。
async fn merge_candidates(candidates: Vec<Candidate>) -> (usize, usize) {
    if candidates.is_empty() {
        return (0, 0);
    }
    let mut unique: Vec<Candidate> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for c in candidates {
        let hash = text_hash(&c.text);
        if seen.insert(hash) {
            unique.push(c);
        }
    }

    // 一次批量求向量；服务不可用时退化为纯文本去重（记忆仍然能落库，召回等补向量后再生效）。
    let texts: Vec<String> = unique.iter().map(|c| c.text.clone()).collect();
    let vecs = crate::commands::embedding::embed_texts(&texts, false)
        .await
        .unwrap_or_default();

    let Ok(conn) = ensure_db() else { return (0, 0) };

    // 活跃记忆的向量索引：既用于相似合并，也随插入增量维护。
    let mut existing: Vec<(String, Vec<f32>)> = {
        let mut stmt = match conn.prepare(
            "SELECT id,vec FROM memories WHERE status='active' AND vec IS NOT NULL",
        ) {
            Ok(s) => s,
            Err(_) => return (0, 0),
        };
        let rows = match stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<Vec<u8>>>(1)?))
        }) {
            Ok(rows) => rows,
            Err(_) => return (0, 0),
        };
        rows.flatten()
            .filter_map(|(id, blob)| blob.map(|b| (id, blob_to_vec(&b))))
            .collect()
    };

    let mut created = 0usize;
    let mut merged = 0usize;
    for (idx, candidate) in unique.iter().enumerate() {
        let hash = text_hash(&candidate.text);
        let existing_row: Option<(String, String)> = conn
            .query_row(
                "SELECT id,status FROM memories WHERE text_hash=?1",
                params![hash],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()
            .ok()
            .flatten();
        if let Some((id, status)) = existing_row {
            if status == "active" {
                let _ = conn.execute(
                    "UPDATE memories SET hits=hits+1,confidence=MAX(confidence,?2),\
                     last_seen_at=?3,updated_at=?3 WHERE id=?1",
                    params![id, candidate.confidence, now_iso()],
                );
                merged += 1;
            }
            // retired 是墓碑：用户忘记过的东西不再复活。
            continue;
        }

        let vec_owned: Option<Vec<f32>> = vecs
            .get(idx)
            .filter(|v| !v.is_empty())
            .cloned();
        let vec = vec_owned.as_deref();
        let mut merge_into: Option<String> = None;
        if let Some(vec) = vec {
            let mut best: Option<(String, f32)> = None;
            for (id, other) in existing.iter() {
                let sim = cosine(vec, other);
                if best.as_ref().map(|(_, s)| sim > *s).unwrap_or(true) {
                    best = Some((id.clone(), sim));
                }
            }
            if let Some((id, sim)) = best {
                if sim >= MERGE_SIM {
                    merge_into = Some(id);
                }
            }
        }
        if let Some(id) = merge_into {
            let _ = conn.execute(
                "UPDATE memories SET hits=hits+1,confidence=MAX(confidence,?2),\
                 last_seen_at=?3,updated_at=?3 WHERE id=?1",
                params![id, candidate.confidence, now_iso()],
            );
            merged += 1;
            continue;
        }

        let id = uuid::Uuid::new_v4().to_string();
        let topics = serde_json::to_string(&candidate.topics).unwrap_or_else(|_| "[]".into());
        let refs = candidate.refs.to_string();
        let result = conn.execute(
            "INSERT INTO memories(id,kind,text,text_hash,confidence,status,source,topics,refs,\
             hits,created_at,updated_at,last_seen_at,vec) \
             VALUES(?1,?2,?3,?4,?5,'active',?6,?7,?8,0,?9,?9,?9,?10)",
            params![
                id,
                candidate.kind,
                candidate.text,
                hash,
                candidate.confidence,
                candidate.source,
                topics,
                refs,
                now_iso(),
                vec_owned.as_ref().map(|v| vec_to_blob(v))
            ],
        );
        if result.is_ok() {
            created += 1;
            if let Some(vec) = vec_owned {
                existing.push((id, vec));
            }
        }
    }
    (created, merged)
}

/// 立即巩固一次：手动块同步 → 规则抽取 → 模型抽取（预算允许）→ 合并 → 重建镜像。
pub async fn consolidate_now() -> Result<ConsolidateReport, String> {
    let window_start = crate::cognition::store::get_setting("memory_consolidated_at")
        .unwrap_or_else(|| {
            (chrono::Local::now() - chrono::Duration::days(1)).to_rfc3339()
        });

    let manual_changed = sync_manual_from_markdown().unwrap_or(0);

    let digest = build_digest(&window_start);
    let mut candidates = rule_candidates(&window_start);
    let llm = llm_candidates(&digest).await;
    let used_llm = !llm.is_empty();
    candidates.extend(llm);

    let candidate_count = candidates.len();
    let (created, merged) = merge_candidates(candidates).await;
    let retired = retire_stale();

    let _ = crate::cognition::store::set_setting("memory_consolidated_at", &now_iso());
    let sync_err = sync_to_markdown().err();

    let mut note = String::new();
    if manual_changed > 0 {
        note.push_str(&format!("手动块同步 {manual_changed} 条；"));
    }
    if !used_llm && !digest.trim().is_empty() {
        note.push_str("模型抽取未产生结果；");
    }
    if let Some(e) = sync_err {
        note.push_str(&format!("镜像写入失败：{e}；"));
    }

    let report = ConsolidateReport {
        window_start,
        candidates: candidate_count,
        created,
        merged,
        retired,
        used_llm,
        note,
    };
    info!(
        "[cognition] 记忆巩固：候选 {}，新增 {}，合并 {}，退休 {}，模型={}",
        report.candidates, report.created, report.merged, report.retired, report.used_llm
    );
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_block_parsing_handles_labels() {
        let block = "- [偏好] 喜欢先看结论\n- 普通一条\n";
        let parsed = parse_manual_block(block);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, "preference");
        assert_eq!(parsed[0].1, "喜欢先看结论");
        assert_eq!(parsed[1].0, "fact");
    }

    #[test]
    fn llm_json_extraction_tolerates_fences() {
        let answer = "```json\n[{\"kind\":\"preference\",\"text\":\"喜欢本地模型\",\"confidence\":0.8}]\n```";
        let items = parse_llm_candidates(answer);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "preference");
    }

    #[test]
    fn block_replace_keeps_manual_content() {
        let text = format!("{MANUAL_START}\n- [事实] 保留我\n{MANUAL_END}\n\n{AUTO_START}\n旧\n{AUTO_END}\n");
        let replaced = replace_block(&text, AUTO_START, AUTO_END, "新");
        assert!(replaced.contains("保留我"));
        assert!(replaced.contains("新"));
        assert!(!replaced.contains("旧"));
    }
}
