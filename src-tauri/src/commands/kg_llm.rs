//! 知识图谱的 LLM 抽取层(概念层 + typed 关系)
//!
//! 用 LLM 从文档抽取“概念/实体”及它们之间的有类型关系,落成横切文件结构的概念层:
//!   - concept 节点(node_type='concept'),跨文件复用,id = concept_<sha256(name_lowercase)>
//!   - mention 边:文件/章节 → 概念
//!   - typed 关系边:概念 ↔ 概念(prerequisite / related / elaborates / contradicts / part-of …)
//!
//! 模型后端走 chatview::kg_llm_complete(本地优先 + 外部 API 预留 + 失败回退本地)。
//! 概念节点与 llm 边在图重建时被保留(见 kgraph::build_into_conn),不会因重扫文件丢失。

use crate::commands::kgraph::{ensure_kg_db, invalidate_kb_caches, sha256_hex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

const MAX_DOC_CHARS: usize = 8000;
const ALLOWED_RELATIONS: [&str; 6] =
    ["related", "prerequisite", "elaborates", "contradicts", "part-of", "similar"];

fn kb_root_dir() -> PathBuf {
    PathBuf::from(crate::commands::settingview::get_knowledge_bases_root())
}

fn concept_node_id(name: &str) -> String {
    format!("concept_{}", sha256_hex(&name.trim().to_lowercase()))
}
fn file_node_id(rel: &str) -> String {
    format!("file_{}", sha256_hex(rel))
}

fn build_extract_prompt(doc: &str) -> String {
    format!(
        "你是知识图谱抽取器。请从下面的笔记中抽取核心\"概念/实体\"以及概念之间的关系。\n\
         要求:\n\
         1. 概念应是名词性的知识点/术语/实体,规范化命名(中英文保持原文常用写法),去重。\n\
         2. 关系类型只能从这些里选:{relations}。\n\
         3. 只输出 JSON,不要任何解释。格式:\n\
         {{\"concepts\":[{{\"name\":\"概念名\",\"type\":\"term|entity|topic\"}}],\
         \"relations\":[{{\"from\":\"概念A\",\"to\":\"概念B\",\"relation\":\"related\"}}]}}\n\
         笔记内容:\n{doc}",
        relations = ALLOWED_RELATIONS.join(" / "),
        doc = doc.chars().take(MAX_DOC_CHARS).collect::<String>()
    )
}

fn extract_json_segment(raw: &str) -> Option<String> {
    let start = raw.find('{')?;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, ch) in raw[start..].char_indices() {
        match ch {
            '"' if !esc => in_str = !in_str,
            '\\' if in_str => {
                esc = !esc;
                continue;
            }
            '{' if !in_str => depth += 1,
            '}' if !in_str => {
                depth -= 1;
                if depth == 0 {
                    return Some(raw[start..start + i + 1].to_string());
                }
            }
            _ => {}
        }
        esc = false;
    }
    None
}

#[derive(Deserialize, Default)]
struct ExtractConcept {
    name: String,
    #[serde(default)]
    #[allow(dead_code)]
    r#type: String,
}
#[derive(Deserialize, Default)]
struct ExtractRelation {
    from: String,
    to: String,
    #[serde(default)]
    relation: String,
}
#[derive(Deserialize, Default)]
struct ExtractResult {
    #[serde(default)]
    concepts: Vec<ExtractConcept>,
    #[serde(default)]
    relations: Vec<ExtractRelation>,
}

fn normalize_relation(r: &str) -> String {
    let r = r.trim().to_lowercase();
    if ALLOWED_RELATIONS.contains(&r.as_str()) {
        r
    } else {
        "related".to_string()
    }
}

/// 把一个文件的抽取结果写入图(概念节点 + mention + typed 关系边)。
fn persist_extraction(
    kb_name: &str,
    file_rel: &str,
    result: &ExtractResult,
) -> Result<(usize, usize), String> {
    let file_node = file_node_id(file_rel);
    let mut conn = ensure_kg_db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let now = chrono::Local::now().to_rfc3339();

    let mut concept_ids: HashMap<String, String> = HashMap::new();
    let mut new_concepts = 0usize;
    {
        let mut node_stmt = tx
            .prepare(
                "INSERT OR IGNORE INTO kg_nodes (kb_name,node_id,node_type,name,depth,content_hash,degree,created_at,modified_at) \
                 VALUES (?1,?2,'concept',?3,1,?4,0,?5,?5)",
            )
            .map_err(|e| e.to_string())?;
        let mut mention_stmt = tx
            .prepare(
                "INSERT OR IGNORE INTO kg_edges (kb_name,from_id,to_id,edge_type,strength,source,src_file) \
                 VALUES (?1,?2,?3,'mention',0.3,'llm',?4)",
            )
            .map_err(|e| e.to_string())?;

        for c in &result.concepts {
            let name = c.name.trim();
            if name.is_empty() {
                continue;
            }
            let cid = concept_node_id(name);
            node_stmt
                .execute(rusqlite::params![kb_name, cid, name, sha256_hex(&name.to_lowercase()), now])
                .map_err(|e| e.to_string())?;
            mention_stmt
                .execute(rusqlite::params![kb_name, file_node, cid, file_rel])
                .map_err(|e| e.to_string())?;
            concept_ids.insert(name.to_lowercase(), cid);
            new_concepts += 1;
        }
    }

    let mut rel_edges = 0usize;
    {
        // 确保关系端点的概念节点存在(即使没出现在 concepts 列表里)
        let mut node_stmt = tx
            .prepare(
                "INSERT OR IGNORE INTO kg_nodes (kb_name,node_id,node_type,name,depth,content_hash,degree,created_at,modified_at) \
                 VALUES (?1,?2,'concept',?3,1,?4,0,?5,?5)",
            )
            .map_err(|e| e.to_string())?;
        let mut edge_stmt = tx
            .prepare(
                "INSERT OR IGNORE INTO kg_edges (kb_name,from_id,to_id,edge_type,strength,source,src_file) \
                 VALUES (?1,?2,?3,?4,0.4,'llm',?5)",
            )
            .map_err(|e| e.to_string())?;

        for r in &result.relations {
            let from = r.from.trim();
            let to = r.to.trim();
            if from.is_empty() || to.is_empty() || from.eq_ignore_ascii_case(to) {
                continue;
            }
            let from_id = concept_node_id(from);
            let to_id = concept_node_id(to);
            for (nm, id) in [(from, &from_id), (to, &to_id)] {
                node_stmt
                    .execute(rusqlite::params![kb_name, id, nm, sha256_hex(&nm.to_lowercase()), now])
                    .map_err(|e| e.to_string())?;
            }
            let etype = normalize_relation(&r.relation);
            edge_stmt
                .execute(rusqlite::params![kb_name, from_id, to_id, etype, file_rel])
                .map_err(|e| e.to_string())?;
            rel_edges += 1;
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok((new_concepts, rel_edges))
}

#[derive(Serialize)]
pub struct KgExtractResult {
    pub kb_name: String,
    pub files_processed: usize,
    pub concepts: usize,
    pub relations: usize,
    pub failures: usize,
}

async fn extract_one_file(
    kb_name: &str,
    file_rel: &str,
    abs_path: &str,
    modelid: i32,
    sessionid: &mut String,
) -> Result<(usize, usize), String> {
    let content = fs::read_to_string(abs_path).map_err(|e| e.to_string())?;
    if content.trim().is_empty() {
        return Ok((0, 0));
    }
    let prompt = build_extract_prompt(&content);
    let (answer, sid) = crate::commands::chatview::kg_llm_complete(modelid, &prompt, sessionid).await?;
    *sessionid = sid;

    let json = extract_json_segment(&answer).ok_or_else(|| "LLM 未返回可解析 JSON".to_string())?;
    let result: ExtractResult = serde_json::from_str(&json).map_err(|e| format!("解析抽取 JSON 失败: {e}"))?;
    persist_extraction(kb_name, file_rel, &result)
}

/// 命令:对单个文件或整个知识库做概念/关系抽取并写入图。
/// rel_path 为空 → 处理全库 markdown 文件(可用 max_files 限量,默认 50,避免一次跑太久)。
#[tauri::command]
pub async fn kg_extract_concepts(
    kb_name: String,
    rel_path: Option<String>,
    modelid: Option<i32>,
    max_files: Option<usize>,
) -> Result<KgExtractResult, String> {
    crate::commands::kgraph::build_if_needed(&kb_name)?;
    let modelid = modelid.unwrap_or(6); // 默认本地 DeepSeek
    let mut sessionid = String::new();

    // 待处理文件列表 (rel, abs)
    let files: Vec<(String, String)> = {
        let conn = ensure_kg_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT rel_path, file_path FROM kg_nodes \
                 WHERE kb_name=?1 AND node_type='file' AND file_path IS NOT NULL \
                   AND (lower(rel_path) LIKE '%.md' OR lower(rel_path) LIKE '%.markdown')",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([&kb_name], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        let mut v = Vec::new();
        for row in rows {
            let (rel, abs) = row.map_err(|e| e.to_string())?;
            v.push((rel, abs));
        }
        v
    };

    let targets: Vec<(String, String)> = match &rel_path {
        Some(rp) if !rp.trim().is_empty() => {
            let norm = rp.replace('\\', "/");
            files.into_iter().filter(|(rel, _)| rel == &norm).collect()
        }
        _ => {
            let cap = max_files.unwrap_or(50);
            files.into_iter().take(cap).collect()
        }
    };

    if targets.is_empty() {
        // 没命中索引文件时,尝试用 kb 目录直接定位单文件
        if let Some(rp) = &rel_path {
            let abs = kb_root_dir().join(&kb_name).join(rp.replace('\\', "/"));
            if abs.is_file() {
                let (c, r) = extract_one_file(&kb_name, rp, &abs.to_string_lossy(), modelid, &mut sessionid).await?;
                invalidate_kb_caches(&kb_name);
                return Ok(KgExtractResult { kb_name, files_processed: 1, concepts: c, relations: r, failures: 0 });
            }
        }
        return Ok(KgExtractResult { kb_name, files_processed: 0, concepts: 0, relations: 0, failures: 0 });
    }

    let mut concepts = 0usize;
    let mut relations = 0usize;
    let mut failures = 0usize;
    let mut processed = 0usize;
    for (rel, abs) in &targets {
        match extract_one_file(&kb_name, rel, abs, modelid, &mut sessionid).await {
            Ok((c, r)) => {
                concepts += c;
                relations += r;
                processed += 1;
            }
            Err(e) => {
                warn!("kg_extract_concepts 文件失败 {}: {}", rel, e);
                failures += 1;
            }
        }
    }

    invalidate_kb_caches(&kb_name);
    info!(
        "kg_extract_concepts done kb={} files={} concepts={} relations={} fail={}",
        kb_name, processed, concepts, relations, failures
    );
    Ok(KgExtractResult { kb_name, files_processed: processed, concepts, relations, failures })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_segment_extraction_from_noisy_output() {
        let raw = "好的,结果如下:\n```json\n{\"concepts\":[{\"name\":\"向量检索\"}],\"relations\":[{\"from\":\"a\",\"to\":\"b\",\"relation\":\"related\"}]}\n```\n完成";
        let seg = extract_json_segment(raw).unwrap();
        let r: ExtractResult = serde_json::from_str(&seg).unwrap();
        assert_eq!(r.concepts.len(), 1);
        assert_eq!(r.concepts[0].name, "向量检索");
        assert_eq!(r.relations.len(), 1);
    }

    #[test]
    fn json_segment_handles_braces_in_strings() {
        let raw = r#"{"concepts":[{"name":"a{b}c"}],"relations":[]}"#;
        let seg = extract_json_segment(raw).unwrap();
        assert_eq!(seg, raw);
    }

    #[test]
    fn relation_normalization() {
        assert_eq!(normalize_relation("Prerequisite"), "prerequisite");
        assert_eq!(normalize_relation("乱写的"), "related");
    }

    #[test]
    fn concept_id_stable_and_case_insensitive() {
        assert_eq!(concept_node_id("Vector Search"), concept_node_id("vector search"));
    }
}
