use grep_matcher::Matcher;
use grep_regex::RegexMatcher;
use grep_searcher::{sinks, SearcherBuilder};
use ignore::WalkBuilder;
use lazy_static::lazy_static;
use lru::LruCache;
use parking_lot::RwLock;
use rayon::prelude::*;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde::Serialize;
use std::ffi::OsStr;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

// 辅助函数：字节索引转字符索引
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
    let total_chars = line.chars().count();
    (
        char_start.unwrap_or_else(|| if start == line.len() { total_chars } else { 0 }),
        char_end.unwrap_or_else(|| if end == line.len() { total_chars } else { 0 }),
    )
}

// 使用NonZeroUsize创建LRU缓存
lazy_static! {
    static ref SEARCH_CACHE: RwLock<LruCache<String, Vec<SearchResult>>> =
        RwLock::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
}

struct SearchManager {
    active_searches: Arc<RwLock<Vec<String>>>,
}

impl SearchManager {
    fn new() -> Self {
        Self {
            active_searches: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn register_search(&self, id: &str) {
        self.active_searches.write().push(id.to_string());
    }

    fn cancel_search(&self, id: &str) {
        self.active_searches.write().retain(|x| x != id);
    }

    fn is_cancelled(&self, id: &str) -> bool {
        !self.active_searches.read().contains(&id.to_string())
    }
}

lazy_static! {
    static ref SEARCH_MANAGER: SearchManager = SearchManager::new();
}

#[derive(Debug, Serialize, Clone)]
pub struct Match {
    pub start: usize,
    pub end: usize,
    pub score: u8,
}

#[derive(Debug, Serialize, Clone)]
pub struct SearchResult {
    pub path: String,
    pub file_name: String,
    pub preview: Option<String>,
    pub line_number: Option<u64>,
    pub matches: Vec<Match>,
    pub score: u32,
}

fn is_binary_extension(ext: &OsStr) -> bool {
    let ext = ext.to_string_lossy().to_lowercase();
    matches!(
        ext.as_str(),
        "gz" | "bz2"
            | "zip"
            | "lzma"
            | "xz"
            | "zst"
            | "pack"
            | "idx"
            | "rev"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "pdf"
            | "doc"
            | "docx"
            | "xls"
            | "xlsx"
            | "ppt"
            | "pptx"
            | "bin"
            | "exe"
            | "dll"
            | "so"
            | "class"
            | "jar"
            | "war"
            | "tar"
            | "rar"
            | "7z"
    )
}

fn is_ignored_dir_component(c: &OsStr) -> bool {
    c == OsStr::new(".git") || c == OsStr::new(".meta") || c == OsStr::new(".trash")
}

fn has_ignored_dir(path: &Path) -> bool {
    path.components()
        .any(|c| is_ignored_dir_component(c.as_os_str()))
}

fn is_cellmeta_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase().ends_with(".cellmeta"))
        .unwrap_or(false)
}

fn should_skip_path(path: &Path) -> bool {
    // 设计说明：
    // 1. `.cellmeta` 是 Notebook 的侧车索引文件，供编辑器内部能力使用，并不是用户希望直接检索/打开的正文。
    // 2. 之前把它纳入全文搜索后，右侧“搜索关键字”面板会出现这些技术文件，点击后自然无法精准跳回真实笔记内容。
    // 3. 这里在搜索源头统一排除 `.cellmeta`，让文件搜索和内容搜索都只面向用户文档本身。
    has_ignored_dir(path)
        || is_cellmeta_file(path)
        || path.extension().map_or(false, is_binary_extension)
}

#[tauri::command]
pub async fn fd_search(
    pattern: String,
    path: String,
    limit: Option<usize>,
    offset: Option<usize>,
    search_id: Option<String>,
) -> Result<(Vec<SearchResult>, bool), String> {
    info!("fd_search called with pattern: {}, path: {}", pattern, path);
    let _start_time = Instant::now();
    let path = Path::new(&path);

    if !path.exists() {
        return Err("路径不存在".into());
    }

    info!("fd_search path exists");
    if let Some(id) = &search_id {
        if SEARCH_MANAGER.is_cancelled(id) {
            return Err("搜索已取消".into());
        }
    }
    info!("fd_search start: {:?}", search_id);

    // 修改1：在缓存键前添加"fd:"前缀，防止与rg_search的缓存冲突
    let cache_key = format!("fd:{}:{}", pattern, path.display());

    // 检查缓存（使用读锁和peek方法）
    {
        let cache = SEARCH_CACHE.read();
        if let Some(cached) = cache.peek(&cache_key) {
            // 修改2：增加缓存结果验证，确保是文件名搜索结果
            if cached.iter().all(|r| r.preview.is_none() && r.matches.is_empty()) {
                let has_more = cached.len() > offset.unwrap_or(0) + limit.unwrap_or(50);
                let results = cached
                    .iter()
                    .skip(offset.unwrap_or(0))
                    .take(limit.unwrap_or(50))
                    .cloned()
                    .collect();
                info!("Cache key: {}", cache_key);
                info!("Cache size: {}", cache.len());
                info!("Returning: {:?} ", results);
                return Ok((results, has_more));
            }
        }
    }

    let file_paths: Vec<_> = WalkBuilder::new(path)
        .hidden(false)
        .add_custom_ignore_filename(".ignore")
        .filter_entry(|e| !has_ignored_dir(e.path()))
        .build()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().map_or(false, |ft| ft.is_file()) && !should_skip_path(entry.path())
        })
        .collect();

    info!("Total files: {:?}", file_paths);

    let mut results: Vec<SearchResult> = file_paths
        .par_iter()
        .map_init(|| SkimMatcherV2::default(), |matcher, entry| {
            if let Some(id) = &search_id {
                if SEARCH_MANAGER.is_cancelled(id) {
                    return None;
                }
            }

            let file_name = entry.path().file_name()?.to_string_lossy().to_string();
            let score = matcher.fuzzy_match(&file_name, &pattern)?;

            Some(SearchResult {
                path: entry.path().to_string_lossy().to_string(),
                file_name,
                preview: None,
                line_number: None,
                matches: vec![],
                score: score as u32,
            })
        })
        .filter_map(|x| x)
        .collect();

    results.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.file_name.cmp(&b.file_name)));

    // 更新缓存（使用写锁）
    {
        let mut cache = SEARCH_CACHE.write();
        cache.put(cache_key, results.clone());
    }

    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    let has_more = results.len() > offset + limit;
    let paginated_results = results.into_iter().skip(offset).take(limit).collect();

    info!("文件搜索完成: {:?}", paginated_results);
    Ok((paginated_results, has_more))
}

#[tauri::command]
pub async fn rg_search(
    pattern: String,
    path: String,
    limit: Option<usize>,
    offset: Option<usize>,
    search_id: Option<String>,
) -> Result<(Vec<SearchResult>, bool), String> {
    let start_time = Instant::now();
    let path = Path::new(&path);

    if !path.exists() {
        return Err("路径不存在".into());
    }

    if let Some(id) = &search_id {
        if SEARCH_MANAGER.is_cancelled(id) {
            return Err("搜索已取消".into());
        }
        SEARCH_MANAGER.register_search(id);
    }

    // 修改4：在缓存键前添加"rg:"前缀，防止与fd_search的缓存冲突
    let cache_key = format!("rg:{}:{}", pattern, path.display());

    // 检查缓存（使用读锁和peek方法）
    {
        let cache = SEARCH_CACHE.read();
        if let Some(cached) = cache.peek(&cache_key) {
            // 修改5：增加缓存结果验证，确保是内容搜索结果
            if cached.iter().any(|r| r.preview.is_some() || !r.matches.is_empty()) {
                let has_more = cached.len() > offset.unwrap_or(0) + limit.unwrap_or(50);
                let results = cached
                    .iter()
                    .skip(offset.unwrap_or(0))
                    .take(limit.unwrap_or(50))
                    .cloned()
                    .collect();
                return Ok((results, has_more));
            }
        }
    }

    let matcher = RegexMatcher::new(&pattern).map_err(|e| format!("搜索模式无效: {}", e))?;

    let file_paths: Vec<_> = WalkBuilder::new(path)
        .hidden(false)
        .add_custom_ignore_filename(".ignore")
        .filter_entry(|e| !has_ignored_dir(e.path()))
        .build()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().map_or(false, |ft| ft.is_file()) && !should_skip_path(entry.path())
        })
        .collect();

    // 使用rayon的par_iter()进行并行处理
    let all_results: Vec<SearchResult> = file_paths
        .par_iter()
        .filter_map(|entry| {
            if let Some(id) = &search_id {
                if SEARCH_MANAGER.is_cancelled(id) {
                    return None;
                }
            }

            let path_str = entry.path().to_string_lossy().to_string();
            let file_name = entry
                .path()
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();

            let mut searcher = SearcherBuilder::new()
                .line_number(true)
                .binary_detection(grep_searcher::BinaryDetection::quit(b'\x00'))
                .build();

            let mut file_results = Vec::new();
            let mut file_score = 0;

            if let Err(e) = searcher.search_path(
                &matcher,
                entry.path(),
                sinks::UTF8(|lineno, line| {
                    let mut matches = vec![];
                    let mut line_score = 0;

                    matcher.find_iter(line.as_bytes(), |m| {
                        let (char_start, char_end) = byte_to_char_indices(line, m.start(), m.end());

                        let match_len = char_end - char_start;
                        let match_score = match_len as u8 + if m.start() == 0 { 5 } else { 0 };

                        matches.push(Match {
                            start: char_start,
                            end: char_end,
                            score: match_score,
                        });

                        line_score += match_score as u32;
                        true
                    })?;

                    if !matches.is_empty() {
                        file_score += line_score;
                        file_results.push(SearchResult {
                            path: path_str.clone(),
                            file_name: file_name.clone(),
                            preview: Some(line.to_string()),
                            line_number: Some(lineno),
                            matches,
                            score: line_score,
                        });
                    }

                    Ok(true)
                }),
            ) {
                if e.kind() != std::io::ErrorKind::InvalidData {
                    eprintln!("搜索文件错误: {} - {}", entry.path().display(), e);
                }
                return None;
            }

            file_results.sort_by(|a, b| b.score.cmp(&a.score));

            if !file_results.is_empty() {
                Some((file_score, file_results))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
        .flat_map(|(file_score, mut results)| {
            for result in &mut results {
                result.score += file_score;
            }
            results
        })
        .collect();

    let mut sorted_results = all_results;
    sorted_results.sort_by(|a, b| b.score.cmp(&a.score));

    // 更新缓存（使用写锁）
    {
        let mut cache = SEARCH_CACHE.write();
        cache.put(cache_key, sorted_results.clone());
    }

    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    let has_more = sorted_results.len() > offset + limit;
    let paginated_results = sorted_results
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect();

    if let Some(id) = search_id {
        SEARCH_MANAGER.cancel_search(&id);
    }

    println!("内容搜索完成，耗时: {:?}", start_time.elapsed());
    Ok((paginated_results, has_more))
}

#[tauri::command]
pub async fn cancel_search(search_id: String) -> Result<(), String> {
    SEARCH_MANAGER.cancel_search(&search_id);
    Ok(())
}
