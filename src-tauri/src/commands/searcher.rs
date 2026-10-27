use grep_matcher::Matcher;
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::{sinks, SearcherBuilder};
use ignore::WalkBuilder;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use rayon::prelude::*;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tantivy::tokenizer::{TokenStream, Tokenizer};
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

/// 取消登记表对外的三个口子。索引搜索和目录搜索必须共用同一张表，
/// 否则前端发出的 `cancel_search` 只能取消其中一路。
///
/// 注意语义：`is_cancelled` = "这个 id 不在活跃表里"，所以两条并发命令
/// 绝不能共用一个 id——先完成的那条会把 id 摘掉，后一条立刻自认为被取消。
pub(crate) fn register_search_id(id: &str) {
    SEARCH_MANAGER.register_search(id);
}

pub(crate) fn release_search_id(id: &str) {
    SEARCH_MANAGER.cancel_search(id);
}

pub(crate) fn search_id_cancelled(id: &str) -> bool {
    SEARCH_MANAGER.is_cancelled(id)
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
    /// 命中落在文件名上的字符区间。`matches` 描述的是 `preview` 里的位置，
    /// 文件名搜索没有 preview，高亮信息必须单独一个字段，否则前端无从分辨坐标系。
    #[serde(default)]
    pub name_matches: Vec<Match>,
    /// 相对搜索根的路径（正斜杠）。前端展示目录、也是路径式查询的匹配对象。
    #[serde(default)]
    pub rel_path: Option<String>,
}

/// 对搜索结果执行统一分页。
///
/// 设计说明：
/// 1. `limit = None` 保持原有的 50 条默认值，供搜索框实时预览使用。
/// 2. `limit = Some(0)` 明确表示“返回 offset 之后的全部结果”，供用户正式执行搜索时使用。
/// 3. 完整结果模式返回 `has_more = false`，避免前端误以为还有未展示数据。
pub(crate) fn paginate_results(
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
pub(crate) fn file_mtime_ms(path: &Path) -> Option<i64> {
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

pub(crate) fn has_ignored_dir(path: &Path) -> bool {
    path.components()
        .any(|c| is_ignored_dir_component(c.as_os_str()))
}

fn is_cellmeta_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase().ends_with(".cellmeta"))
        .unwrap_or(false)
}

pub(crate) fn should_skip_path(path: &Path) -> bool {
    // 设计说明：
    // 1. `.cellmeta` 是 Notebook 的侧车索引文件，供编辑器内部能力使用，并不是用户希望直接检索/打开的正文。
    // 2. 之前把它纳入全文搜索后，右侧“搜索关键字”面板会出现这些技术文件，点击后自然无法精准跳回真实笔记内容。
    // 3. 这里在搜索源头统一排除 `.cellmeta`，让文件搜索和内容搜索都只面向用户文档本身。
    has_ignored_dir(path)
        || is_cellmeta_file(path)
        || path.extension().map_or(false, is_binary_extension)
}

/// 匹配方式开关，文件名搜索与内容搜索共用。
///
/// 默认字面量而不是正则：用户在搜索框敲 `foo(bar)`、`a.b`、`C++`、`std::vec` 时
/// 想的是"找这串字符"。按正则解析要么直接报"模式无效"，要么静默匹配到别的东西——
/// 两种都是难以自查的坏体验。真要用正则的人会自己去开开关。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchOptions {
    /// true = pattern 按正则解析；默认(false/None)按字面量。
    #[serde(default)]
    pub regex: Option<bool>,
    /// None = 智能大小写（查询里出现大写才区分）；Some 则强制。
    #[serde(default)]
    pub case_sensitive: Option<bool>,
    /// 只匹配完整单词。中文照样有效，词边界由 jieba 分词给出，见 [`SearchMatcher`]。
    #[serde(default)]
    pub whole_word: Option<bool>,
}

impl MatchOptions {
    pub(crate) fn is_regex(&self) -> bool {
        self.regex.unwrap_or(false)
    }

    pub(crate) fn is_whole_word(&self) -> bool {
        self.whole_word.unwrap_or(false)
    }
}

/// 词边界判定时的字符分类。
///
/// 分三类而不是照搬正则的 `\w`/`\W`：Unicode 语义下汉字也是 word char，于是
/// "参数"和"高效"之间不存在 `\b`，`\b参数\b` 在"参数高效微调"里永远匹配不上——
/// 这就是全词匹配对中文失效的根因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharClass {
    /// 字母、数字、下划线，语义和正则的 `\w` 对齐
    Word,
    /// 汉字与日文假名：连着写、不靠空格分词，边界只能问分词器
    Cjk,
    /// 空白与标点，天然的边界
    Other,
}

fn char_class(ch: char) -> CharClass {
    // 韩文用空格分词，归到 Word 让它继续沿用正则那套语义
    let cjk = matches!(ch as u32,
        0x3040..=0x30FF          // 平假名、片假名
        | 0x3400..=0x4DBF        // 汉字扩展 A
        | 0x4E00..=0x9FFF        // 基本汉字
        | 0xF900..=0xFAFF        // 兼容汉字
        | 0x20000..=0x3134F      // 汉字扩展 B 及以后
    );
    if cjk {
        CharClass::Cjk
    } else if ch.is_alphanumeric() || ch == '_' {
        CharClass::Word
    } else {
        CharClass::Other
    }
}

/// 一段文本里所有 jieba 词的起止位置（byte offset），升序去重。
///
/// 复用 `tantivy_jieba` 而不是自己 new 一个 jieba：它内部是全局共享实例，
/// 不会再多加载一份词典，而且用的就是全文索引那套分词器——全词匹配认的"词"
/// 和索引认的"词"因此天然一致。
///
/// 取 search 模式是故意的：它会把长词再切出子词（"生命科学"里还能拿到"生命"），
/// 搜索框宁可多给一条结果，也不该让用户觉得"明明有这个词却搜不到"。
fn word_bounds(text: &str) -> Vec<usize> {
    let mut tokenizer = tantivy_jieba::JiebaTokenizer::new();
    let mut stream = tokenizer.token_stream(text);
    let mut bounds = Vec::new();
    while stream.advance() {
        let token = stream.token();
        bounds.push(token.offset_from);
        bounds.push(token.offset_to);
    }
    bounds.sort_unstable();
    bounds.dedup();
    bounds
}

/// grep matcher 加一层词边界策略。
///
/// 为什么不用 grep-regex 自带的 `.word(true)`：那个开关等价于给模式两头加 `\b`，
/// 而 Unicode 语义下汉字是 word char，一串汉字内部根本不存在 `\b`——
/// 开着全词匹配搜中文，结果永远是空的。
///
/// VSCode 也不是靠 `\b` 做这件事。它先用 `wordSeparators` 逐字符判断左右是否成词，
/// 这一层遇到 CJK 同样判不出来（一串汉字里没有任何分隔符），所以它又叠了一层
/// `Intl.Segmenter(granularity: 'word')`：拿分词结果去校验命中的两端有没有落在
/// 词的边界上。这里做的是同一件事，分词器换成 jieba。
pub struct SearchMatcher {
    inner: RegexMatcher,
    /// true 时每条命中都要过一遍词边界校验
    whole_word: bool,
}

impl SearchMatcher {
    /// 给 grep searcher 做逐行扫描用的原始 matcher。
    ///
    /// 它只负责挑出"这一行可能有命中"，真正的全词校验在 [`SearchMatcher::find_iter`]：
    /// 校验要看整行上下文，而 matcher 只看得到模式本身。
    pub(crate) fn raw(&self) -> &RegexMatcher {
        &self.inner
    }

    /// 遍历一段文本里通过词边界校验的命中，位置是 byte offset。
    pub(crate) fn find_iter<F>(&self, haystack: &str, mut on_match: F) -> std::io::Result<()>
    where
        F: FnMut(grep_matcher::Match) -> bool,
    {
        // 分词按需算、且一段文本只算一次：没有汉字相邻的命中根本用不上它
        let mut bounds: Option<Vec<usize>> = None;
        self.inner
            .find_iter(haystack.as_bytes(), |m| {
                if self.whole_word && !self.is_whole_word_match(haystack, &m, &mut bounds) {
                    return true; // 这条不算，继续找下一条
                }
                on_match(m)
            })
            .map_err(std::io::Error::from)
    }

    /// 是否至少有一条通过校验的命中。
    pub fn is_match(&self, haystack: &str) -> bool {
        if !self.whole_word {
            return self.inner.is_match(haystack.as_bytes()).unwrap_or(false);
        }
        let mut hit = false;
        let _ = self.find_iter(haystack, |_| {
            hit = true;
            false // 找到一条就够，不用扫完
        });
        hit
    }

    /// 命中的两端是否都落在词边界上。
    fn is_whole_word_match(
        &self,
        haystack: &str,
        m: &grep_matcher::Match,
        bounds: &mut Option<Vec<usize>>,
    ) -> bool {
        self.is_boundary(haystack, m.start(), bounds)
            && self.is_boundary(haystack, m.end(), bounds)
    }

    /// `at`（byte offset）处是不是词边界。
    fn is_boundary(&self, haystack: &str, at: usize, bounds: &mut Option<Vec<usize>>) -> bool {
        let before = haystack
            .get(..at)
            .and_then(|s| s.chars().next_back())
            .map(char_class);
        let after = haystack
            .get(at..)
            .and_then(|s| s.chars().next())
            .map(char_class);
        match (before, after) {
            // 文本头尾
            (None, _) | (_, None) => true,
            // 有一侧是空白或标点
            (Some(CharClass::Other), _) | (_, Some(CharClass::Other)) => true,
            // 字母数字连成一片：和 `\b` 一致，`foo` 不该命中 `foobar`
            (Some(CharClass::Word), Some(CharClass::Word)) => false,
            // 汉字挨着字母数字：文字体系换了，就是边界。
            // 这条让"使用LoRA的"里的 LoRA 也能被全词匹配找到——正则 `\b` 同样漏这一种
            (Some(CharClass::Word), Some(CharClass::Cjk))
            | (Some(CharClass::Cjk), Some(CharClass::Word)) => true,
            // 汉字挨着汉字：字符本身给不出答案，问分词器
            (Some(CharClass::Cjk), Some(CharClass::Cjk)) => bounds
                .get_or_insert_with(|| word_bounds(haystack))
                .binary_search(&at)
                .is_ok(),
        }
    }
}

/// 统一构建 grep matcher，让文件名搜索和内容搜索的语义完全一致。
///
/// `fixed_strings` 交给 grep-regex 自己转义，比手写 `regex::escape` 稳：
/// 它在开了智能大小写时会走"转义后再解析"的路径，大小写分析仍然生效。
pub fn build_matcher(pattern: &str, opts: &MatchOptions) -> Result<SearchMatcher, String> {
    let mut builder = RegexMatcherBuilder::new();
    builder
        .fixed_strings(!opts.is_regex())
        // 这里故意不开 `.word()`：词边界一律交给 SearchMatcher 判。
        // 否则汉字相邻的命中会先被正则的 `\b` 吃掉，轮不到分词器说话
        .line_terminator(Some(b'\n'));
    match opts.case_sensitive {
        // 用户点了开关就该立刻生效，不再让智能大小写插手
        Some(true) => {
            builder.case_insensitive(false).case_smart(false);
        }
        Some(false) => {
            builder.case_insensitive(true).case_smart(false);
        }
        // 没指定则智能大小写：全小写查询忽略大小写，敲了大写就当他是故意的
        None => {
            builder.case_insensitive(false).case_smart(true);
        }
    }
    let inner = builder
        .build(pattern)
        .map_err(|e| format!("搜索模式无效: {}", e))?;
    Ok(SearchMatcher {
        inner,
        whole_word: opts.is_whole_word(),
    })
}

/// 相对搜索根的路径，分隔符统一成 `/`。
///
/// Windows 上真实路径是 `\`，但用户敲的是 `src/components/foo`；
/// 不统一的话路径式查询在 Windows 上永远匹配不到。
pub(crate) fn rel_display_path(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let s = rel.to_string_lossy().replace('\\', "/");
    if s.is_empty() {
        path.to_string_lossy().replace('\\', "/")
    } else {
        s
    }
}

/// 把一串递增的字符下标压成连续区间，供前端做高亮。
pub(crate) fn char_indices_to_ranges(indices: &[usize]) -> Vec<Match> {
    let mut ranges: Vec<Match> = Vec::new();
    for &i in indices {
        match ranges.last_mut() {
            Some(last) if last.end == i => last.end = i + 1,
            _ => ranges.push(Match {
                start: i,
                end: i + 1,
                score: 1,
            }),
        }
    }
    ranges
}

/// 文件名命中比目录命中值钱：同样的模糊分数，命中在文件名上的排前面。
const FILENAME_MATCH_BONUS: u32 = 64;

/// 文件名搜索的打分。
///
/// 旧实现只拿 `file_name` 去模糊匹配，于是 `src/foo`、`components/header` 这类
/// 带分隔符的查询永远是零结果——文件名里根本不可能出现 `/`。这里改成：
/// 先按文件名匹配（加权），不中再退回整条相对路径，让目录名也能参与检索。
fn score_name_or_path(
    matcher: &SkimMatcherV2,
    file_name: &str,
    rel: &str,
    pattern: &str,
) -> Option<(u32, Vec<Match>)> {
    // 查询里带分隔符，说明用户在描述路径，只有整条相对路径才可能匹配
    let looks_like_path = pattern.contains('/') || pattern.contains('\\');
    let query = if looks_like_path {
        pattern.replace('\\', "/")
    } else {
        pattern.to_string()
    };

    if !looks_like_path {
        if let Some((score, indices)) = matcher.fuzzy_indices(file_name, &query) {
            return Some((
                score.max(0) as u32 + FILENAME_MATCH_BONUS,
                char_indices_to_ranges(&indices),
            ));
        }
    }

    let (score, indices) = matcher.fuzzy_indices(rel, &query)?;
    // 落在文件名区段内的下标折算成文件名坐标，落在目录段的没法高亮就丢掉
    let name_offset = rel.chars().count().saturating_sub(file_name.chars().count());
    let in_name: Vec<usize> = indices
        .iter()
        .filter(|&&i| i >= name_offset)
        .map(|&i| i - name_offset)
        .collect();
    Some((score.max(0) as u32, char_indices_to_ranges(&in_name)))
}

/// 正则模式下的文件名/路径匹配，复用内容搜索那套 matcher，语义保持一致。
pub(crate) fn regex_match_name_or_path(
    matcher: &SearchMatcher,
    file_name: &str,
    rel: &str,
) -> Option<(u32, Vec<Match>)> {
    let mut name_ranges: Vec<Match> = Vec::new();
    let _ = matcher.find_iter(file_name, |m| {
        let (s, e) = byte_to_char_indices(file_name, m.start(), m.end());
        name_ranges.push(Match {
            start: s,
            end: e,
            score: 1,
        });
        true
    });
    if !name_ranges.is_empty() {
        return Some((FILENAME_MATCH_BONUS, name_ranges));
    }
    // 文件名没中就看整条相对路径，目录名也算命中（只是不加权）
    if matcher.is_match(rel) {
        Some((1, Vec::new()))
    } else {
        None
    }
}

/// 路径深度，用于同分时让浅层文件排前面（更可能是用户要的那个）。
pub(crate) fn path_depth(rel: &str) -> usize {
    rel.bytes().filter(|&b| b == b'/').count()
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
    options: Option<MatchOptions>,
) -> Result<(Vec<SearchResult>, bool), String> {
    info!("fd_search called with pattern: {}, path: {}", pattern, path);
    let _start_time = Instant::now();
    let root = Path::new(&path);

    if !root.exists() {
        return Err("路径不存在".into());
    }

    let opts = options.unwrap_or_default();
    let query = pattern.trim().to_string();
    // 空查询不是错误：它表示"先给我看看这个目录里都有什么"，按最近修改排。
    let list_mode = query.is_empty();
    let regex_matcher = if !list_mode && opts.is_regex() {
        Some(build_matcher(&query, &opts)?)
    } else {
        None
    };

    info!("fd_search path exists");
    if let Some(id) = &search_id {
        SEARCH_MANAGER.register_search(id);
    }
    info!("fd_search start: {:?}", search_id);

    let file_paths: Vec<_> = WalkBuilder::new(root)
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
        root.display(), file_paths.len(), pattern
    );

    let make_fuzzy = || match opts.case_sensitive {
        Some(true) => SkimMatcherV2::default().respect_case(),
        Some(false) => SkimMatcherV2::default().ignore_case(),
        None => SkimMatcherV2::default(),
    };

    let mut results: Vec<SearchResult> = file_paths
        .par_iter()
        .map_init(make_fuzzy, |fuzzy, entry| {
            if let Some(id) = &search_id {
                if SEARCH_MANAGER.is_cancelled(id) {
                    return None;
                }
            }

            let file_name = entry.path().file_name()?.to_string_lossy().to_string();
            let rel = rel_display_path(root, entry.path());

            let (score, name_matches) = if list_mode {
                (0, Vec::new())
            } else if let Some(rx) = &regex_matcher {
                regex_match_name_or_path(rx, &file_name, &rel)?
            } else {
                score_name_or_path(fuzzy, &file_name, &rel, &query)?
            };

            Some(SearchResult {
                path: entry.path().to_string_lossy().to_string(),
                file_name,
                preview: None,
                line_number: None,
                matches: vec![],
                score,
                modified_ms: file_mtime_ms(entry.path()),
                name_matches,
                rel_path: Some(rel),
            })
        })
        .filter_map(|x| x)
        .collect();

    if list_mode {
        // 没有查询词时分数全为 0，按最近修改排才有意义
        results.sort_by(|a, b| {
            b.modified_ms
                .cmp(&a.modified_ms)
                .then_with(|| a.file_name.cmp(&b.file_name))
        });
    } else {
        results.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                // 同分优先浅层路径：越靠近根目录越可能是用户想找的那个
                .then_with(|| {
                    path_depth(a.rel_path.as_deref().unwrap_or_default())
                        .cmp(&path_depth(b.rel_path.as_deref().unwrap_or_default()))
                })
                .then_with(|| a.file_name.len().cmp(&b.file_name.len()))
                .then_with(|| a.file_name.cmp(&b.file_name))
        });
    }

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

/// 在单个文件里逐行找命中，返回 (文件总分, 每行一条的结果)。
///
/// 抽成独立函数是因为全文索引那一路也要用：tantivy 只回答"哪些文件相关"，
/// 行号和预览仍得靠这同一套 grep 现读一遍——否则两条路给出的预览会不一样，
/// 而用户分不清自己看的是哪一路的结果。
///
/// `max_lines` 到量就让 grep 提前收工。全文索引只需要前几行做预览，
/// 没必要把一个几万行的日志整个扫完。
pub fn grep_file_matches(
    matcher: &SearchMatcher,
    path: &Path,
    rel: &str,
    mtime_ms: Option<i64>,
    max_lines: Option<usize>,
) -> Option<(u32, Vec<SearchResult>)> {
    let path_str = path.to_string_lossy().to_string();
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .binary_detection(grep_searcher::BinaryDetection::quit(b'\x00'))
        .build();

    let mut file_results: Vec<SearchResult> = Vec::new();
    let mut file_score = 0u32;

    if let Err(e) = searcher.search_path(
        matcher.raw(),
        path,
        sinks::UTF8(|lineno, line| {
            let mut matches = vec![];
            let mut line_score = 0;

            matcher.find_iter(line, |m| {
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
                    name_matches: Vec::new(),
                    rel_path: Some(rel.to_string()),
                });
            }

            // 够数了就别再往下读
            Ok(max_lines.map_or(true, |cap| file_results.len() < cap))
        }),
    ) {
        // 文件读取或 UTF-8 解码失败必须保留路径和原因，避免静默漏搜。
        warn!(
            "grep 读取文件失败: path={}, kind={:?}, error={}",
            path.display(),
            e.kind(),
            e
        );
        return None;
    }

    file_results.sort_by(|a, b| b.score.cmp(&a.score));

    if file_results.is_empty() {
        None
    } else {
        Some((file_score, file_results))
    }
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
    options: Option<MatchOptions>,
) -> Result<(Vec<SearchResult>, bool), String> {
    let start_time = Instant::now();
    let root = Path::new(&path);

    if !root.exists() {
        return Err("路径不存在".into());
    }

    // 空模式在字面量模式下会编译成"匹配一切"，一个大目录就能刷出几百万行。
    // 直接返回空集：调用方通常是搜索框在用户还没敲完时先发的那一枪。
    if pattern.trim().is_empty() {
        return Ok((vec![], false));
    }

    if let Some(id) = &search_id {
        SEARCH_MANAGER.register_search(id);
    }

    let opts = options.unwrap_or_default();
    let matcher = match build_matcher(&pattern, &opts) {
        Ok(m) => m,
        Err(e) => {
            // 模式非法时也要摘掉登记，否则这个 id 会一直挂在活跃表里
            if let Some(id) = &search_id {
                SEARCH_MANAGER.cancel_search(id);
            }
            return Err(e);
        }
    };

    let file_paths: Vec<_> = WalkBuilder::new(root)
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
        root.display(), file_paths.len(), pattern
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

            let rel = rel_display_path(root, entry.path());
            let mtime_ms = file_mtime_ms(entry.path());

            grep_file_matches(&matcher, entry.path(), &rel, mtime_ms, None)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(regex: Option<bool>, case_sensitive: Option<bool>, whole_word: Option<bool>) -> MatchOptions {
        MatchOptions {
            regex,
            case_sensitive,
            whole_word,
        }
    }

    fn hits(pattern: &str, opts: &MatchOptions, haystack: &str) -> bool {
        build_matcher(pattern, opts).unwrap().is_match(haystack)
    }

    #[test]
    fn literal_by_default_does_not_interpret_metacharacters() {
        let o = opts(None, None, None);
        assert!(hits("a.b", &o, "xxa.byy"));
        assert!(!hits("a.b", &o, "axb"), "字面量模式下的 . 不该当通配符");
        // 这些在旧实现里会直接抛"搜索模式无效"
        assert!(hits("foo(bar", &o, "call foo(bar)"));
        assert!(hits("C++", &o, "写点 C++ 代码"));
        assert!(hits("a[b", &o, "arr a[b]"));
    }

    #[test]
    fn regex_mode_opts_in_explicitly() {
        let o = opts(Some(true), None, None);
        assert!(hits("a.b", &o, "axb"));
        assert!(hits(r"fo+", &o, "fooo"));
        assert!(build_matcher("a[b", &o).is_err(), "正则模式下非法模式应报错");
    }

    #[test]
    fn smart_case_is_the_default() {
        // 全小写查询忽略大小写
        assert!(hits("foo", &opts(None, None, None), "FooBar"));
        // 查询里出现大写就当他是故意的
        assert!(!hits("Foo", &opts(None, None, None), "foobar"));
    }

    #[test]
    fn explicit_case_overrides_smart_case() {
        assert!(!hits("foo", &opts(None, Some(true), None), "FooBar"));
        assert!(hits("Foo", &opts(None, Some(false), None), "foobar"));
    }

    #[test]
    fn whole_word_requires_boundaries() {
        let o = opts(None, None, Some(true));
        assert!(hits("foo", &o, "a foo b"));
        assert!(!hits("foo", &o, "foobar"));
        assert!(!hits("foo", &o, "barfoo"));
        // 下划线在正则里算 word char，这里也保持一致
        assert!(!hits("foo", &o, "foo_bar"));
    }

    #[test]
    fn whole_word_works_for_chinese() {
        let o = opts(None, None, Some(true));
        // 汉字之间没有 `\b`，靠 jieba 切出来的词边界判断
        assert!(hits("参数", &o, "参数高效微调"), "整词应该能命中");
        assert!(hits("微调", &o, "参数高效微调"));
        assert!(!hits("数高", &o, "参数高效微调"), "跨词的片段不算整词");
        assert!(hits("笔记", &o, "这是我的笔记，随手记的"));
    }

    #[test]
    fn whole_word_matches_latin_inside_chinese() {
        let o = opts(None, None, Some(true));
        // 正则 `\b` 在这里也失效：汉字和字母都是 word char，中间判不出边界
        assert!(hits("LoRA", &o, "使用LoRA的参数高效微调"));
        assert!(!hits("oRA", &o, "使用LoRA的参数高效微调"));
    }

    #[test]
    fn whole_word_off_still_matches_substrings() {
        // 没开全词匹配时不该受分词影响，子串照样命中
        let o = opts(None, None, None);
        assert!(hits("数高", &o, "参数高效微调"));
        assert!(hits("foo", &o, "foobar"));
    }

    #[test]
    fn rel_display_path_uses_forward_slashes() {
        let root = Path::new("/tmp/kb");
        assert_eq!(rel_display_path(root, Path::new("/tmp/kb/a/b.md")), "a/b.md");
        // 不在根下时退回原路径，至少不会变成空串
        assert!(!rel_display_path(root, Path::new("/other/c.md")).is_empty());
    }

    #[test]
    fn char_ranges_merge_consecutive_indices() {
        let r = char_indices_to_ranges(&[0, 1, 2, 5, 6, 9]);
        let pairs: Vec<(usize, usize)> = r.iter().map(|m| (m.start, m.end)).collect();
        assert_eq!(pairs, vec![(0, 3), (5, 7), (9, 10)]);
        assert!(char_indices_to_ranges(&[]).is_empty());
    }

    #[test]
    fn path_query_matches_directory_segments() {
        let fuzzy = SkimMatcherV2::default();
        // 旧实现只匹配文件名，这种查询必定零结果
        assert!(score_name_or_path(&fuzzy, "header.vue", "src/components/header.vue", "components/header").is_some());
        assert!(score_name_or_path(&fuzzy, "header.vue", "src/components/header.vue", "src/head").is_some());
        assert!(score_name_or_path(&fuzzy, "header.vue", "src/components/header.vue", "zzz/head").is_none());
    }

    #[test]
    fn filename_hit_outranks_directory_only_hit() {
        let fuzzy = SkimMatcherV2::default();
        let (name_hit, ranges) = score_name_or_path(&fuzzy, "search.rs", "src/search.rs", "search").unwrap();
        let (dir_hit, _) = score_name_or_path(&fuzzy, "mod.rs", "src/search/mod.rs", "search").unwrap();
        assert!(name_hit > dir_hit, "文件名命中应排在只有目录命中的前面");
        assert!(!ranges.is_empty(), "文件名命中要带出高亮区间");
    }

    #[test]
    fn directory_hit_highlights_only_the_filename_part() {
        let fuzzy = SkimMatcherV2::default();
        let (_, ranges) = score_name_or_path(&fuzzy, "mod.rs", "src/search/mod.rs", "search/mod").unwrap();
        // 命中横跨目录和文件名，只有文件名那一段能落到 name_matches 上
        assert!(ranges.iter().all(|m| m.end <= "mod.rs".chars().count()));
    }

    #[test]
    fn regex_name_match_prefers_filename() {
        let m = build_matcher(r"search", &opts(Some(true), None, None)).unwrap();
        let (name_score, ranges) = regex_match_name_or_path(&m, "search.rs", "src/search.rs").unwrap();
        let (dir_score, dir_ranges) = regex_match_name_or_path(&m, "mod.rs", "src/search/mod.rs").unwrap();
        assert!(name_score > dir_score);
        assert_eq!(ranges.len(), 1);
        assert!(dir_ranges.is_empty());
        assert!(regex_match_name_or_path(&m, "mod.rs", "src/other/mod.rs").is_none());
    }

    #[test]
    fn path_depth_counts_separators() {
        assert_eq!(path_depth("a.md"), 0);
        assert_eq!(path_depth("a/b/c.md"), 2);
    }

    #[tokio::test]
    async fn rg_search_returns_empty_for_blank_pattern() {
        let dir = std::env::temp_dir();
        let (results, has_more) = rg_search(
            "   ".to_string(),
            dir.to_string_lossy().to_string(),
            Some(10),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert!(results.is_empty());
        assert!(!has_more);
    }
}
