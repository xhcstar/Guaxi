use grep_regex::RegexMatcher;
use grep_searcher::{sinks, Searcher, SearcherBuilder};
use grep_matcher::Matcher;
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::ffi::OsStr;

#[derive(Debug, Serialize)]
pub struct Match {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub path: String,
    pub file_name: String,
    pub preview: Option<String>,
    pub line_number: Option<u64>,
    pub matches: Vec<Match>,
}

// 判断是否为二进制文件扩展名
fn is_binary_extension(ext: &OsStr) -> bool {
    let ext = ext.to_string_lossy().to_lowercase();
    matches!(ext.as_str(), 
        "gz" | "bz2" | "zip" | "lzma" | "xz" | "zst" | 
        "pack" | "idx" | "rev" | "png" | "jpg" | "jpeg" | 
        "gif" | "pdf" | "doc" | "docx" | "xls" | "xlsx" |
        "ppt" | "pptx" | "bin" | "exe" | "dll" | "so" |
        "class" | "jar" | "war" | "tar" | "rar" | "7z"
    )
}

// 判断是否应该跳过的路径
fn should_skip_path(path: &Path) -> bool {
    // 跳过.git目录
    path.components().any(|c| c.as_os_str() == ".git") ||
    // 跳过常见二进制文件扩展名
    path.extension().map_or(false, is_binary_extension)
}

#[tauri::command]
pub async fn fd_search(pattern: String, path: String) -> Result<Vec<SearchResult>, String> {
    let path = Path::new(&path);
    if !path.exists() {
        return Err("路径不存在".into());
    }

    let mut results = Vec::new();

    for entry in WalkBuilder::new(path)
        .hidden(false)
        .add_custom_ignore_filename(".ignore") // 添加自定义忽略文件
        .build()
        .filter_map(Result::ok)
    {
        // 跳过不需要搜索的文件和目录
        if should_skip_path(entry.path()) {
            continue;
        }

        if let Some(file_name) = entry.path().file_name() {
            let file_name = file_name.to_string_lossy();
            if file_name.contains(&pattern) {
                results.push(SearchResult {
                    path: entry.path().to_string_lossy().to_string(),
                    file_name: file_name.into_owned(),
                    preview: None,
                    line_number: None,
                    matches: vec![],
                });
            }
        }
    }

    Ok(results)
}

/// 将字节区间转换为字符区间（用于高亮索引准确性）
/// 返回 (char_start, char_end)
fn byte_to_char_indices(line: &str, start: usize, end: usize) -> (usize, usize) {
    let mut char_start = None;
    let mut char_end = None;
    for (i, (bidx, _)) in line.char_indices().enumerate() {
        if bidx == start {
            char_start = Some(i);
        }
        if bidx == end {
            char_end = Some(i);
        }
    }
    // 处理 end 恰好在行尾的情况
    let total_chars = line.chars().count();
    (
        char_start.unwrap_or_else(|| {
            if start == line.len() { total_chars } else { 0 }
        }),
        char_end.unwrap_or_else(|| {
            if end == line.len() { total_chars } else { 0 }
        }),
    )
}

#[tauri::command]
pub async fn rg_search(pattern: String, path: String) -> Result<Vec<SearchResult>, String> {
    let path = Path::new(&path);
    if !path.exists() {
        return Err("路径不存在".into());
    }

    let matcher = RegexMatcher::new(&pattern).map_err(|e| format!("搜索模式无效: {}", e))?;
    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .binary_detection(grep_searcher::BinaryDetection::quit(b'\x00')) // 二进制检测
        .build();

    let mut results = Vec::new();

    for entry in WalkBuilder::new(path)
        .hidden(false)
        .add_custom_ignore_filename(".ignore") // 添加自定义忽略文件
        .build()
        .filter_map(Result::ok)
    {
        // 跳过不需要搜索的文件和目录
        if should_skip_path(entry.path()) {
            continue;
        }

        if entry.file_type().map_or(false, |ft| ft.is_file()) {
            let path_str = entry.path().to_string_lossy().to_string();
            let file_name = entry
                .path()
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();

            // 使用更安全的搜索方式
            match searcher.search_path(
                &matcher,
                entry.path(),
                sinks::UTF8(|lineno, line| {
                    // 检查是否为有效的UTF-8文本
                    if std::str::from_utf8(line.as_bytes()).is_err() {
                        println!(
                            "[rg_search][skip] 非UTF-8文件: {}",
                            path_str
                        );
                        return Ok(true); // 跳过非UTF-8文件
                    }

                    let mut matches = vec![];
                    matcher.find_iter(line.as_bytes(), |m| {
                        // 字节索引转字符索引，保证前端高亮准确
                        let (char_start, char_end) = byte_to_char_indices(line, m.start(), m.end());
                        if char_start != m.start() || char_end != m.end() {
                            println!(
                                "[rg_search][debug] 字节索引({}-{}) => 字符索引({}-{})，内容: {}",
                                m.start(),
                                m.end(),
                                char_start,
                                char_end,
                                &line[m.start()..m.end().min(line.len())]
                            );
                        }
                        matches.push(Match {
                            start: char_start,
                            end: char_end,
                        });
                        true
                    })?;

                    if !matches.is_empty() {
                        println!(
                            "[rg_search][match] 文件: {}, 行: {}, 匹配数: {}, 内容: {}",
                            path_str,
                            lineno,
                            matches.len(),
                            line
                        );
                    }

                    results.push(SearchResult {
                        path: path_str.clone(),
                        file_name: file_name.clone(),
                        preview: Some(line.to_string()),
                        line_number: Some(lineno),
                        matches,
                    });

                    Ok(true)
                }),
            ) {
                Ok(_) => (),
                Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                    // 跳过二进制文件错误
                    println!(
                        "[rg_search][skip] 二进制文件或无效数据: {}",
                        path_str
                    );
                    continue;
                }
                Err(e) => {
                    eprintln!("搜索文件错误: {} - {}", entry.path().display(), e);
                }
            }
        }
    }

    Ok(results)
}