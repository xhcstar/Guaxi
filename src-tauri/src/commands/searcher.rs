use grep_regex::RegexMatcher;
use grep_searcher::{sinks, Searcher, SearcherBuilder};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub path: String,
    pub preview: Option<String>,  // 仅用于内容搜索
    pub line_number: Option<u64>, // 仅用于内容搜索
}

// 文件名搜索（代替 fd-find）
#[tauri::command]
pub async fn fd_search(pattern: String, path: String) -> Result<Vec<SearchResult>, String> {
    let path = Path::new(&path);
    if !path.exists() {
        return Err("路径不存在".into());
    }

    let mut results = Vec::new();

    for result in WalkBuilder::new(path)
        .hidden(false) // 包括隐藏文件
        .build()
    {
        match result {
            Ok(entry) => {
                if let Some(file_name) = entry.path().file_name() {
                    let file_name = file_name.to_string_lossy();
                    if file_name.contains(&pattern) {
                        results.push(SearchResult {
                            path: entry.path().to_string_lossy().to_string(),
                            preview: None,
                            line_number: None,
                        });
                    }
                }
            }
            Err(e) => {
                eprintln!("目录遍历错误: {}", e);
                continue;
            }
        }
    }

    Ok(results)
}

// 内容搜索（代替 ripgrep）
#[tauri::command]
pub async fn rg_search(pattern: String, path: String) -> Result<Vec<SearchResult>, String> {
    let path = Path::new(&path);
    if !path.exists() {
        return Err("路径不存在".into());
    }

    let matcher =
        RegexMatcher::new_line_matcher(&pattern).map_err(|e| format!("搜索模式无效: {}", e))?;
    let mut searcher = SearcherBuilder::new().line_number(true).build();

    let mut results = Vec::new();

    for result in WalkBuilder::new(path)
        .hidden(false) // 包括隐藏文件
        .build()
    {
        match result {
            Ok(entry) => {
                if entry.file_type().map_or(false, |ft| ft.is_file()) {
                    let path_str = entry.path().to_string_lossy().to_string();

                    // 使用 sinks::UTF8，而不是 grep_searcher::Sink::new
                    if let Err(e) = searcher.search_path(
                        &matcher,
                        entry.path(),
                        sinks::UTF8(|lineno, line| {
                            results.push(SearchResult {
                                path: path_str.clone(),
                                preview: Some(line.to_string()), // 这里直接用 line.to_string()
                                line_number: Some(lineno),
                            });
                            Ok(true)
                        }),
                    ) {
                        eprintln!("搜索文件错误: {} - {}", entry.path().display(), e);
                    }
                }
            }
            Err(e) => {
                eprintln!("目录遍历错误: {}", e);
            }
        }
    }

    Ok(results)
}
