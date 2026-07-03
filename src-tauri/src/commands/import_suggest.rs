//! 导入文件时的自动建议：标签 / 知识库 / 文件库。
//!
//! 思路（复用已集成的 bge-m3 embedding 服务）：
//! 1. 用文件名（+ 描述 + 可选内容预览）构造查询文本，求其向量；
//! 2. 分别对已存在的标签、知识库、文件库名称求向量；
//! 3. 用点积（向量已 L2 归一化，等价余弦相似度）排序，返回最相近的建议。
//!
//! 全部为“建议”，前端预填后用户可自由修改。embedding 服务不可用或无历史数据时，
//! 优雅降级：返回空建议（不阻塞导入流程）。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tracing::{info, warn};

use crate::commands::embedding::embed_texts;
use crate::commands::repoview::{
    get_knowledge_repos, search_imported_file_libraries, search_imported_files_tags,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestRequest {
    /// 原始文件名（含扩展名）。
    pub file_name: String,
    /// 可选：用户已填的描述，纳入语义信号。
    #[serde(default)]
    pub description: Option<String>,
    /// 可选：文件路径。用于对 .txt/.md 读取少量内容预览，增强语义。
    #[serde(default)]
    pub file_path: Option<String>,
    /// 每类返回的候选数（标签可多选，库/知识库前端只取第 1 个）。默认 3。
    #[serde(default)]
    pub top_k: Option<usize>,
    /// 相似度下限，低于此分不建议。默认 0.35。
    #[serde(default)]
    pub min_score: Option<f32>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScoredSuggestion {
    pub value: String,
    pub score: f32,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SuggestResult {
    /// 建议标签（按分数降序，可多选）。
    pub tags: Vec<ScoredSuggestion>,
    /// 建议知识库（第 1 个为最佳）。
    pub knowledge_repos: Vec<ScoredSuggestion>,
    /// 建议文件库（第 1 个为最佳）。
    pub libraries: Vec<ScoredSuggestion>,
    /// 是否因为 embedding 服务不可用而降级为空建议。
    pub degraded: bool,
    /// 降级/说明信息（可选，便于前端提示）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// 去掉扩展名并把常见分隔符换成空格，让文件名更“可读”利于语义匹配。
fn humanize_file_name(file_name: &str) -> String {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);
    stem.replace(['_', '-', '.', '（', '）', '(', ')', '[', ']', '【', '】'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// 对 .txt/.md 读取前若干字符作为内容预览（其它类型跳过，避免慢/无意义解析）。
fn read_preview(file_path: &Option<String>) -> Option<String> {
    let path = file_path.as_ref()?;
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "txt" | "md" | "markdown" | "csv") {
        return None;
    }
    match fs::read_to_string(path) {
        Ok(content) => {
            let preview: String = content.chars().take(1000).collect();
            let trimmed = preview.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        Err(_) => None,
    }
}

/// 对候选集合按与查询向量的相似度排序，返回 >= min_score 的前 top_k 项。
async fn rank_candidates(
    query_vec: &[f32],
    candidates: &[String],
    top_k: usize,
    min_score: f32,
) -> Result<Vec<ScoredSuggestion>, String> {
    // 去空、去重（保序）。
    let mut seen = std::collections::HashSet::new();
    let cleaned: Vec<String> = candidates
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && seen.insert(s.clone()))
        .collect();
    if cleaned.is_empty() {
        return Ok(Vec::new());
    }

    // 候选用 passage 前缀（is_query=false）。
    let vecs = embed_texts(&cleaned, false).await?;
    if vecs.len() != cleaned.len() {
        warn!(
            "候选向量数量不匹配: got {} expected {}",
            vecs.len(),
            cleaned.len()
        );
    }

    let mut scored: Vec<ScoredSuggestion> = cleaned
        .iter()
        .zip(vecs.iter())
        .map(|(name, v)| ScoredSuggestion {
            value: name.clone(),
            score: dot(query_vec, v),
        })
        .filter(|s| s.score >= min_score)
        .collect();

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k.max(1));
    Ok(scored)
}

/// 导入文件时自动建议 标签/知识库/文件库。失败不致命，降级为空建议。
#[tauri::command]
pub async fn suggest_import_metadata(request: SuggestRequest) -> Result<SuggestResult, String> {
    let top_k = request.top_k.unwrap_or(3).clamp(1, 10);
    let min_score = request.min_score.unwrap_or(0.35);

    // 组织查询文本：文件名（人性化）+ 描述 + 内容预览。
    let mut parts: Vec<String> = Vec::new();
    let readable = humanize_file_name(&request.file_name);
    if !readable.is_empty() {
        parts.push(readable);
    }
    if let Some(desc) = request.description.as_ref() {
        let d = desc.trim();
        if !d.is_empty() {
            parts.push(d.to_string());
        }
    }
    if let Some(preview) = read_preview(&request.file_path) {
        parts.push(preview);
    }
    let query_text = parts.join("。");
    if query_text.trim().is_empty() {
        return Ok(SuggestResult {
            degraded: true,
            note: Some("无可用文本用于建议".to_string()),
            ..Default::default()
        });
    }

    // 求查询向量。embedding 服务不可用时降级返回空建议。
    let query_vecs = match embed_texts(&[query_text.clone()], true).await {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "suggest_import_metadata: embedding 服务不可用，降级");
            return Ok(SuggestResult {
                degraded: true,
                note: Some(format!("嵌入服务不可用：{e}")),
                ..Default::default()
            });
        }
    };
    let query_vec = match query_vecs.into_iter().next() {
        Some(v) => v,
        None => {
            return Ok(SuggestResult {
                degraded: true,
                note: Some("嵌入结果为空".to_string()),
                ..Default::default()
            })
        }
    };

    // 拉取现有候选（空 query = 全部）。
    let tags = search_imported_files_tags(String::new()).unwrap_or_default();
    let repos = get_knowledge_repos().unwrap_or_default();
    let libs = search_imported_file_libraries(String::new()).unwrap_or_default();

    info!(
        file_name = request.file_name.as_str(),
        tag_count = tags.len(),
        repo_count = repos.len(),
        lib_count = libs.len(),
        "suggest_import_metadata candidates"
    );

    // 标签可给多个；库/知识库前端只用第 1 个，但也返回候选便于展示。
    let tag_suggestions = rank_candidates(&query_vec, &tags, top_k, min_score)
        .await
        .unwrap_or_default();
    let repo_suggestions = rank_candidates(&query_vec, &repos, top_k, min_score)
        .await
        .unwrap_or_default();
    let lib_suggestions = rank_candidates(&query_vec, &libs, top_k, min_score)
        .await
        .unwrap_or_default();

    Ok(SuggestResult {
        tags: tag_suggestions,
        knowledge_repos: repo_suggestions,
        libraries: lib_suggestions,
        degraded: false,
        note: None,
    })
}
