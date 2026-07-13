use grep_matcher::Matcher;
use grep_regex::RegexMatcher;
use grep_searcher::{sinks, SearcherBuilder};
use ignore::WalkBuilder;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use rayon::prelude::*;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde::Serialize;
use std::collections::VecDeque;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};

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
        let mut searches = self.active_searches.write();
        searches.retain(|active_id| active_id != id);
        searches.push(id.to_string());
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
    /// 文件最后修改时间(unix 毫秒),用于时间段筛选与展示。
    #[serde(default)]
    pub modified_ms: Option<i64>,
}

/// 对搜索结果执行统一分页。
///
/// 设计说明：
/// 1. `limit = None` 保持原有的 50 条默认值，供搜索框实时预览使用。
/// 2. `limit = Some(0)` 明确表示“返回 offset 之后的全部结果”，供用户正式执行搜索时使用。
/// 3. 完整结果模式返回 `has_more = false`，避免前端误以为还有未展示数据。
fn paginate_results(
    results: &[SearchResult],
    limit: Option<usize>,
    offset: Option<usize>,
) -> (Vec<SearchResult>, bool) {
    let start = offset.unwrap_or(0).min(results.len());
    if matches!(limit, Some(0)) {
        return (results[start..].to_vec(), false);
    }

    let page_size = limit.unwrap_or(50);
    let end = start.saturating_add(page_size).min(results.len());
    (results[start..end].to_vec(), end < results.len())
}

/// 将不同文件的内容命中结果交错排列，避免单个高频文件垄断结果前部。
///
/// 每个文件内部保留单行相关度排序；文件之间每轮最多取一条，
/// 因而第一轮结束前不会展示任一文件的第二条结果。
fn interleave_content_results(
    mut file_groups: Vec<(u32, Vec<SearchResult>)>,
) -> Vec<SearchResult> {
    file_groups.sort_by(|a, b| {
        let a_path = a.1.first().map(|result| result.path.as_str()).unwrap_or_default();
        let b_path = b.1.first().map(|result| result.path.as_str()).unwrap_or_default();
        b.0.cmp(&a.0).then_with(|| a_path.cmp(b_path))
    });

    let total_results = file_groups.iter().map(|(_, results)| results.len()).sum();
    let mut queues: Vec<VecDeque<SearchResult>> = file_groups
        .into_iter()
        .map(|(_, results)| VecDeque::from(results))
        .collect();
    let mut ordered = Vec::with_capacity(total_results);

    while ordered.len() < total_results {
        for queue in &mut queues {
            if let Some(result) = queue.pop_front() {
                ordered.push(result);
            }
        }
    }
    ordered
}

/// 读取文件最后修改时间(unix 毫秒)。
fn file_mtime_ms(path: &Path) -> Option<i64> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let dur = mtime.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(dur.as_millis() as i64)
}

/// 按扩展名集合 + 修改时间区间过滤(空集合/None 表示不限制)。
fn passes_meta_filter(
    path: &Path,
    exts: &Option<Vec<String>>,
    after: Option<i64>,
    before: Option<i64>,
) -> bool {
    if let Some(list) = exts {
        if !list.is_empty() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase());
            match ext {
                Some(e) if list.iter().any(|x| x.trim().to_lowercase() == e) => {}
                _ => return false,
            }
        }
    }
    if after.is_some() || before.is_some() {
        match file_mtime_ms(path) {
            Some(ms) => {
                if let Some(a) = after {
                    if ms < a {
                        return false;
                    }
                }
                if let Some(b) = before {
                    if ms > b {
                        return false;
                    }
                }
            }
            None => return false,
        }
    }
    true
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
    extensions: Option<Vec<String>>,
    modified_after: Option<i64>,
    modified_before: Option<i64>,
) -> Result<(Vec<SearchResult>, bool), String> {
    info!("fd_search called with pattern: {}, path: {}", pattern, path);
    let _start_time = Instant::now();
    let path = Path::new(&path);

    if !path.exists() {
        return Err("路径不存在".into());
    }

    info!("fd_search path exists");
    if let Some(id) = &search_id {
        SEARCH_MANAGER.register_search(id);
    }
    info!("fd_search start: {:?}", search_id);

    let file_paths: Vec<_> = WalkBuilder::new(path)
        .hidden(false)
        .add_custom_ignore_filename(".ignore")
        .filter_entry(|e| !has_ignored_dir(e.path()))
        .build()
        .filter_map(|result| match result {
            Ok(entry) => Some(entry),
            Err(error) => {
                // 未忽略但无法遍历的路径不能静默消失，记录错误供诊断。
                warn!("fd_search 遍历路径失败: {}", error);
                None
            }
        })
        .filter(|entry| {
            entry.file_type().map_or(false, |ft| ft.is_file())
                && !should_skip_path(entry.path())
                && passes_meta_filter(entry.path(), &extensions, modified_after, modified_before)
        })
        .collect();

    info!(
        "fd_search 扫描完成: root={}, candidate_files={}, pattern={}",
        path.display(), file_paths.len(), pattern
    );

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
                modified_ms: file_mtime_ms(entry.path()),
            })
        })
        .filter_map(|x| x)
        .collect();

    results.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.file_name.cmp(&b.file_name)));

    // 直接基于当前磁盘状态返回，不使用跨请求全局缓存，避免重复查询得到陈旧结果。
    let total_results = results.len();
    let (paginated_results, has_more) = paginate_results(&results, limit, offset);

    if let Some(id) = search_id {
        SEARCH_MANAGER.cancel_search(&id);
    }

    info!(
        "fd_search 返回结果: total={}, returned={}, has_more={}, limit={:?}, offset={:?}",
        total_results, paginated_results.len(), has_more, limit, offset
    );
    Ok((paginated_results, has_more))
}

#[tauri::command]
pub async fn rg_search(
    pattern: String,
    path: String,
    limit: Option<usize>,
    offset: Option<usize>,
    search_id: Option<String>,
    extensions: Option<Vec<String>>,
    modified_after: Option<i64>,
    modified_before: Option<i64>,
) -> Result<(Vec<SearchResult>, bool), String> {
    let start_time = Instant::now();
    let path = Path::new(&path);

    if !path.exists() {
        return Err("路径不存在".into());
    }

    if let Some(id) = &search_id {
        SEARCH_MANAGER.register_search(id);
    }

    let matcher = RegexMatcher::new(&pattern).map_err(|e| format!("搜索模式无效: {}", e))?;

    let file_paths: Vec<_> = WalkBuilder::new(path)
        .hidden(false)
        .add_custom_ignore_filename(".ignore")
        .filter_entry(|e| !has_ignored_dir(e.path()))
        .build()
        .filter_map(|result| match result {
            Ok(entry) => Some(entry),
            Err(error) => {
                // 未忽略但无法遍历的路径不能静默消失，记录错误供诊断。
                warn!("rg_search 遍历路径失败: {}", error);
                None
            }
        })
        .filter(|entry| {
            entry.file_type().map_or(false, |ft| ft.is_file())
                && !should_skip_path(entry.path())
                && passes_meta_filter(entry.path(), &extensions, modified_after, modified_before)
        })
        .collect();

    info!(
        "rg_search 扫描完成: root={}, candidate_files={}, pattern={}",
        path.display(), file_paths.len(), pattern
    );

    // 使用rayon的par_iter()进行并行处理
    let all_results: Vec<(u32, Vec<SearchResult>)> = file_paths
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
            let mtime_ms = file_mtime_ms(entry.path());

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
                            modified_ms: mtime_ms,
                        });
                    }

                    Ok(true)
                }),
            ) {
                // 文件读取或 UTF-8 解码失败必须保留路径和原因，避免静默漏搜。
                warn!(
                    "rg_search 读取文件失败: path={}, kind={:?}, error={}",
                    entry.path().display(), e.kind(), e
                );
                return None;
            }

            file_results.sort_by(|a, b| b.score.cmp(&a.score));

            if !file_results.is_empty() {
                Some((file_score, file_results))
            } else {
                None
            }
        })
        .collect();

    // 跨文件公平交错，且不再把文件总分重复叠加到每个命中行。
    let sorted_results = interleave_content_results(all_results);
    let total_results = sorted_results.len();
    let unique_files = sorted_results
        .iter()
        .map(|result| result.path.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len();
    let (paginated_results, has_more) = paginate_results(&sorted_results, limit, offset);

    if let Some(id) = search_id {
        SEARCH_MANAGER.cancel_search(&id);
    }

    info!(
        "rg_search 返回结果: total={}, unique_files={}, returned={}, has_more={}, limit={:?}, offset={:?}, elapsed={:?}",
        total_results, unique_files, paginated_results.len(), has_more, limit, offset, start_time.elapsed()
    );
    Ok((paginated_results, has_more))
}

#[tauri::command]
pub async fn cancel_search(search_id: String) -> Result<(), String> {
    SEARCH_MANAGER.cancel_search(&search_id);
    Ok(())
}
