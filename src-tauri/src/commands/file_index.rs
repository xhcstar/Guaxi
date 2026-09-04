//! 文件名索引：把"每次敲键都重走一遍目录树"换成"常驻内存表 + nucleo 匹配"。
//!
//! 为什么需要它：`fd_search` 每次调用都要 `WalkBuilder` 走一遍全库，
//! 再对每个文件 `stat` 两次（过滤一次、取 mtime 一次）。库里几万个文件时
//! 单次就是几百毫秒到数秒，而搜索框是逐字符触发的——用户敲十个字符，
//! 磁盘就被翻十遍。索引把遍历和 stat 从热路径上彻底挪走。
//!
//! 三个设计取舍：
//!
//! 1. **只给预览用，正式搜索仍走 `fd_search`。** 索引靠 notify 维护，
//!    但监听在网络盘、权限受限目录上会漏事件。逐字符预览稍旧无所谓，
//!    用户按下回车要的那份结果必须是当下磁盘的真实状态。
//! 2. **匹配对象是整条相对路径，不是文件名。** nucleo 的 `match_paths`
//!    打分会给分隔符后的首字符加权，所以文件名命中天然排在目录命中前面，
//!    不需要再手工加 bonus，而 `components/header` 这种查询也能中。
//! 3. **高亮下标只给返回的那一页算。** 打分不需要下标，算下标要额外一趟
//!    回溯矩阵；几万个候选里只有几十个会被看见。
use ignore::WalkBuilder;
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::RwLock;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tracing::{info, warn};

use super::searcher::{
    build_matcher, char_indices_to_ranges, file_mtime_ms, has_ignored_dir, path_depth,
    regex_match_name_or_path, register_search_id, release_search_id, rel_display_path,
    search_id_cancelled, should_skip_path, Match, MatchOptions, SearchResult,
};

/// 事件安静这么久就落地一批改动。太短会在 git checkout 这类批量操作里反复重建。
const DEBOUNCE: Duration = Duration::from_millis(300);
/// 事件不停时的强制落地上限，避免持续写入把更新饿死。
const MAX_DEBOUNCE: Duration = Duration::from_secs(2);

/// 一个被索引的文件。字段都是"匹配和过滤当场要用"的，别的一概不存。
#[derive(Debug, Clone)]
pub struct IndexEntry {
    /// 绝对路径，保留系统原生分隔符——前端要拿它去打开文件。
    pub path: String,
    /// 相对索引根的路径，分隔符统一成 `/`，匹配和展示都用这一份。
    pub rel: String,
    /// `rel` 里文件名起始的字符下标。把路径坐标折回文件名坐标时要用，
    /// 存下来省得每次匹配都重数一遍字符。
    pub name_start: usize,
    pub modified_ms: Option<i64>,
    /// 小写扩展名，没有扩展名则空串。
    pub ext: String,
}

impl IndexEntry {
    fn new(root: &Path, path: &Path) -> Self {
        let rel = rel_display_path(root, path);
        // 文件名是 rel 最后一个 `/` 之后的部分，按字符数算偏移
        let name_start = match rel.rfind('/') {
            Some(byte_idx) => rel[..=byte_idx].chars().count(),
            None => 0,
        };
        Self {
            path: path.to_string_lossy().to_string(),
            rel,
            name_start,
            modified_ms: file_mtime_ms(path),
            ext: path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default(),
        }
    }

    fn file_name(&self) -> &str {
        match self.rel.rfind('/') {
            Some(i) => &self.rel[i + 1..],
            None => &self.rel,
        }
    }
}

/// 路径的比较键。Windows 上大小写不敏感，`D:\A\b.md` 和 `d:\a\B.md` 是同一个文件，
/// 用原样字符串当 HashMap 的键会让同一个文件在增量更新时留下两条。
pub(crate) fn path_key(path: &str) -> String {
    let unified = path.replace('\\', "/");
    if cfg!(windows) {
        unified.to_lowercase()
    } else {
        unified
    }
}

/// 索引本体。
///
/// `map` 负责增量更新（按路径定位），`snapshot` 负责匹配（一段连续内存，
/// rayon 可以直接切片并行）。两份数据的代价是一次 memcpy，换来的是
/// 匹配时完全不用持锁——搜索几万个候选要几毫秒到几十毫秒，
/// 这段时间里如果攥着读锁，watcher 线程就会被卡住。
struct IndexState {
    root: PathBuf,
    root_key: String,
    map: HashMap<String, IndexEntry>,
    snapshot: Arc<Vec<IndexEntry>>,
    /// `map` 改过但 `snapshot` 还没重建
    dirty: bool,
    ready: bool,
    /// 每次整体重建自增，用来判断一个后台构建任务的结果是否已经过期
    generation: u64,
    /// 监听句柄。丢掉它就等于停止监听，所以必须一直被持有。
    _watcher: Option<RecommendedWatcher>,
}

impl IndexState {
    /// 取一份可以脱锁使用的候选表。
    fn snapshot(&mut self) -> Arc<Vec<IndexEntry>> {
        if self.dirty {
            self.snapshot = Arc::new(self.map.values().cloned().collect());
            self.dirty = false;
        }
        Arc::clone(&self.snapshot)
    }
}

/// 全局只留一份索引：搜索框永远只针对当前库根，换根就整体重建。
/// 同时索引两个根的收益远小于内存和 watcher 句柄的代价。
static INDEX: RwLock<Option<IndexState>> = RwLock::new(None);
/// 正在构建的代号。非 0 表示有后台构建在跑，用来给并发的 ensure 去重。
static BUILDING: AtomicU64 = AtomicU64::new(0);
static GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize)]
pub struct IndexStatus {
    /// 索引的根（原样路径）。没有索引时是空串。
    pub root: String,
    pub file_count: usize,
    /// 可以用来搜索了
    pub ready: bool,
    /// 后台正在构建
    pub building: bool,
}

impl IndexStatus {
    fn empty() -> Self {
        Self {
            root: String::new(),
            file_count: 0,
            ready: false,
            building: BUILDING.load(Ordering::Relaxed) != 0,
        }
    }
}

fn current_status() -> IndexStatus {
    let building = BUILDING.load(Ordering::Relaxed) != 0;
    match &*INDEX.read() {
        Some(state) => IndexStatus {
            root: state.root.to_string_lossy().to_string(),
            file_count: state.map.len(),
            ready: state.ready,
            building,
        },
        None => IndexStatus::empty(),
    }
}

/// 走一遍目录树。忽略规则和 `fd_search` 完全一致，否则索引搜索和
/// 正式搜索会给出不同的候选集，用户会以为哪一边坏了。
fn scan_root(root: &Path) -> HashMap<String, IndexEntry> {
    let started = Instant::now();
    let paths: Vec<PathBuf> = WalkBuilder::new(root)
        .hidden(false)
        .add_custom_ignore_filename(".ignore")
        .filter_entry(|e| !has_ignored_dir(e.path()))
        .build()
        .filter_map(|r| match r {
            Ok(entry) => Some(entry),
            Err(err) => {
                warn!("file_index 遍历失败: {}", err);
                None
            }
        })
        .filter(|e| e.file_type().map_or(false, |ft| ft.is_file()) && !should_skip_path(e.path()))
        .map(|e| e.into_path())
        .collect();

    // mtime 要逐个 stat，这是建索引最贵的一步，并行摊掉
    let entries: Vec<(String, IndexEntry)> = paths
        .par_iter()
        .map(|p| {
            let entry = IndexEntry::new(root, p);
            (path_key(&entry.path), entry)
        })
        .collect();

    let map: HashMap<String, IndexEntry> = entries.into_iter().collect();
    info!(
        "file_index 建立完成: root={}, files={}, 耗时={:?}",
        root.display(),
        map.len(),
        started.elapsed()
    );
    map
}

/// 这个根还是不是当前索引的根。用户换了库之后旧的 watcher 线程要自己退场。
fn is_current_root(root_key: &str) -> bool {
    INDEX
        .read()
        .as_ref()
        .map_or(false, |s| s.root_key == root_key)
}

/// 把一批 notify 事件落到索引上的结果。
enum ApplyOutcome {
    Applied,
    /// 索引还没就绪，这批事件先留着——建索引那几百毫秒里发生的改动不能丢。
    Deferred,
    /// 出现了目录级变动，只能整体重扫。
    NeedsRescan,
}

fn apply_changes(root_key: &str, paths: &[PathBuf]) -> ApplyOutcome {
    let mut guard = INDEX.write();
    let Some(state) = guard.as_mut() else {
        return ApplyOutcome::Deferred;
    };
    if state.root_key != root_key || !state.ready {
        return ApplyOutcome::Deferred;
    }
    let root = state.root.clone();
    let mut changed = 0usize;
    for path in paths {
        let key = path_key(&path.to_string_lossy());
        match std::fs::metadata(path) {
            Ok(meta) if meta.is_file() => {
                if should_skip_path(path) {
                    changed += state.map.remove(&key).is_some() as usize;
                } else {
                    state.map.insert(key, IndexEntry::new(&root, path));
                    changed += 1;
                }
            }
            // 目录的新建/改名只能全量重扫：notify 只报目录本身，不报里面的文件
            Ok(_) => {
                // 这一批里前面几条已经改到 map 上了，快照必须跟着失效
                state.dirty |= changed > 0;
                return ApplyOutcome::NeedsRescan;
            }
            Err(_) => {
                // 没了。可能是文件，也可能是整个目录——目录就把它下面的全摘掉
                changed += state.map.remove(&key).is_some() as usize;
                let prefix = format!("{}/", key);
                let before = state.map.len();
                state.map.retain(|k, _| !k.starts_with(&prefix));
                changed += before - state.map.len();
            }
        }
    }
    if changed > 0 {
        state.dirty = true;
        info!("file_index 增量更新: {} 项", changed);
    }
    ApplyOutcome::Applied
}

/// 事件攒到这个数就不再逐条处理了，直接重扫——逐条比重扫还贵的时候没必要坚持增量。
const PENDING_CAP: usize = 4096;

/// 起一个监听，并用一条线程做去抖后落地。
///
/// 返回的 watcher 句柄必须被 `IndexState` 一直持有：丢掉它就等于停止监听。
fn spawn_watcher(root: PathBuf, app: Option<AppHandle>) -> Option<RecommendedWatcher> {
    let root_key = path_key(&root.to_string_lossy());
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(err) => {
            warn!("file_index 创建监听失败，索引将不再自动刷新: {}", err);
            return None;
        }
    };
    if let Err(err) = watcher.watch(&root, RecursiveMode::Recursive) {
        warn!("file_index 监听 {} 失败: {}", root.display(), err);
        return None;
    }

    std::thread::spawn(move || {
        let mut pending: Vec<PathBuf> = Vec::new();
        let mut needs_rescan = false;
        let mut first_seen: Option<Instant> = None;
        loop {
            let waited = match rx.recv_timeout(DEBOUNCE) {
                Ok(Ok(event)) => {
                    if matches!(
                        event.kind,
                        notify::EventKind::Create(_)
                            | notify::EventKind::Remove(_)
                            | notify::EventKind::Modify(_)
                    ) {
                        for p in event.paths {
                            if has_ignored_dir(&p) {
                                continue;
                            }
                            pending.push(p);
                        }
                        first_seen.get_or_insert_with(Instant::now);
                    }
                    if pending.len() > PENDING_CAP {
                        needs_rescan = true;
                        pending.clear();
                    }
                    // 还有事件在来，继续攒
                    first_seen.map_or(false, |t| t.elapsed() >= MAX_DEBOUNCE)
                }
                Ok(Err(err)) => {
                    warn!("file_index 监听事件出错: {}", err);
                    false
                }
                Err(RecvTimeoutError::Timeout) => true,
                // 发送端连同 watcher 一起被丢弃，说明索引已经换根或释放
                Err(RecvTimeoutError::Disconnected) => break,
            };

            if !waited || (pending.is_empty() && !needs_rescan) {
                continue;
            }
            if !is_current_root(&root_key) {
                break;
            }
            if needs_rescan {
                pending.clear();
                first_seen = None;
                needs_rescan = false;
                start_build(root.clone(), app.clone(), true);
                continue;
            }
            match apply_changes(&root_key, &pending) {
                ApplyOutcome::Applied => {
                    pending.clear();
                    first_seen = None;
                }
                // 攥着不动，等索引就绪后的下一轮再落
                ApplyOutcome::Deferred => {}
                ApplyOutcome::NeedsRescan => {
                    pending.clear();
                    first_seen = None;
                    start_build(root.clone(), app.clone(), true);
                }
            }
        }
        info!("file_index 监听线程退出: {}", root.display());
    });

    Some(watcher)
}

/// 构建期间又有新请求（换根或目录级变动）时记在这里，等当前这轮结束补做。
static PENDING_ROOT: RwLock<Option<PathBuf>> = RwLock::new(None);

/// 起一轮构建。同一时刻只允许一轮，重复请求会被折叠成"结束后再来一次"。
fn start_build(root: PathBuf, app: Option<AppHandle>, force: bool) {
    let root_key = path_key(&root.to_string_lossy());
    if !force {
        if let Some(state) = INDEX.read().as_ref() {
            if state.root_key == root_key && state.ready {
                return;
            }
        }
    }
    if BUILDING.swap(1, Ordering::SeqCst) != 0 {
        *PENDING_ROOT.write() = Some(root);
        return;
    }
    std::thread::spawn(move || {
        build_now(root, app.clone());
        // 先放开闸门再看队列：宁可多建一次，也不能把请求漏掉
        BUILDING.store(0, Ordering::SeqCst);
        let next = PENDING_ROOT.write().take();
        if let Some(next) = next {
            start_build(next, app, true);
        }
    });
}

/// 真正的构建流程。顺序是刻意的：先把"新的根"写进去（ready=false）并装上监听，
/// 再去扫盘。这样扫描那几百毫秒里发生的改动会被监听线程攥在手里，
/// 等索引落地后的下一轮去抖补上，不会凭空丢失。
fn build_now(root: PathBuf, app: Option<AppHandle>) {
    let root_key = path_key(&root.to_string_lossy());
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    *INDEX.write() = Some(IndexState {
        root: root.clone(),
        root_key: root_key.clone(),
        map: HashMap::new(),
        snapshot: Arc::new(Vec::new()),
        dirty: false,
        ready: false,
        generation,
        _watcher: None,
    });

    let watcher = spawn_watcher(root.clone(), app.clone());
    let map = scan_root(&root);
    let count = map.len();

    {
        let mut guard = INDEX.write();
        match guard.as_mut() {
            // 还是我们刚写进去的那份，填上结果
            Some(state) if state.generation == generation => {
                state.map = map;
                state.dirty = true;
                state.ready = true;
                state._watcher = watcher;
            }
            // 期间被换根/释放了，这份结果作废（watcher 随本函数返回一起丢掉，监听自动停止）
            _ => {
                info!("file_index 构建结果已过期，丢弃: {}", root.display());
                return;
            }
        }
    }

    if let Some(app) = app {
        let status = current_status();
        if let Err(err) = app.emit("file_index_ready", &status) {
            warn!("file_index 发送就绪事件失败: {}", err);
        }
    }
    info!("file_index 就绪: root={}, files={}", root.display(), count);
}

/// 扩展名 / 修改时间过滤。数据全部来自索引，不再 stat 磁盘——
/// 这也是索引比 `fd_search` 快的另一半原因：原来每个文件要 stat 两次。
struct Filters {
    exts: Vec<String>,
    after: Option<i64>,
    before: Option<i64>,
}

impl Filters {
    fn new(exts: Option<Vec<String>>, after: Option<i64>, before: Option<i64>) -> Self {
        Self {
            exts: exts
                .unwrap_or_default()
                .into_iter()
                .map(|e| e.trim().trim_start_matches('.').to_lowercase())
                .filter(|e| !e.is_empty())
                .collect(),
            after,
            before,
        }
    }

    fn accepts(&self, entry: &IndexEntry) -> bool {
        if !self.exts.is_empty() && !self.exts.iter().any(|e| *e == entry.ext) {
            return false;
        }
        if self.after.is_some() || self.before.is_some() {
            let Some(ms) = entry.modified_ms else {
                return false;
            };
            if self.after.map_or(false, |a| ms < a) || self.before.map_or(false, |b| ms > b) {
                return false;
            }
        }
        true
    }
}

fn nucleo_case(opts: &MatchOptions) -> CaseMatching {
    match opts.case_sensitive {
        Some(true) => CaseMatching::Respect,
        Some(false) => CaseMatching::Ignore,
        None => CaseMatching::Smart,
    }
}

/// 路径感知的打分配置：分隔符后的首字符有加权，所以文件名命中天然压过目录命中，
/// 不需要像 `fd_search` 那样手工加 bonus。
fn path_matcher() -> Matcher {
    Matcher::new(Config::DEFAULT.match_paths())
}

/// 打分阶段的产物：只记下标和分数，高亮下标留给分页之后再算。
struct Scored {
    idx: usize,
    score: u32,
}

/// 和 `searcher::paginate_results` 同一套语义（`None` = 50 条，`Some(0)` = 全部），
/// 只是作用在打分结果上：分页要发生在算高亮之前，才能少算几万条的下标。
fn paginate_scored(hits: &[Scored], limit: Option<usize>, offset: Option<usize>) -> (&[Scored], bool) {
    let start = offset.unwrap_or(0).min(hits.len());
    if matches!(limit, Some(0)) {
        return (&hits[start..], false);
    }
    let end = start
        .saturating_add(limit.unwrap_or(50))
        .min(hits.len());
    (&hits[start..end], end < hits.len())
}

/// 对候选表打分排序。返回的顺序就是最终展示顺序。
fn rank(
    entries: &[IndexEntry],
    query: &str,
    opts: &MatchOptions,
    filters: &Filters,
    search_id: Option<&str>,
) -> Result<Vec<Scored>, String> {
    let cancelled = |id: Option<&str>| id.map_or(false, search_id_cancelled);

    // 空查询 = "先看看这里有什么"，按最近修改排
    if query.is_empty() {
        let mut hits: Vec<Scored> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| filters.accepts(e))
            .map(|(idx, _)| Scored { idx, score: 0 })
            .collect();
        hits.sort_by(|a, b| {
            entries[b.idx]
                .modified_ms
                .cmp(&entries[a.idx].modified_ms)
                .then_with(|| entries[a.idx].file_name().cmp(entries[b.idx].file_name()))
        });
        return Ok(hits);
    }

    let mut hits: Vec<Scored> = if opts.is_regex() {
        let matcher = build_matcher(query, opts)?;
        entries
            .par_iter()
            .enumerate()
            .filter_map(|(idx, e)| {
                if cancelled(search_id) || !filters.accepts(e) {
                    return None;
                }
                let (score, _) = regex_match_name_or_path(&matcher, e.file_name(), &e.rel)?;
                Some(Scored { idx, score })
            })
            .collect()
    } else {
        let pattern = Pattern::parse(query, nucleo_case(opts), Normalization::Smart);
        entries
            .par_iter()
            .enumerate()
            .map_init(
                || (path_matcher(), Vec::<char>::new()),
                |(matcher, buf), (idx, e)| {
                    if cancelled(search_id) || !filters.accepts(e) {
                        return None;
                    }
                    let haystack = Utf32Str::new(&e.rel, buf);
                    let score = pattern.score(haystack, matcher)?;
                    Some(Scored { idx, score })
                },
            )
            .flatten()
            .collect()
    };

    hits.sort_by(|a, b| {
        let (ea, eb) = (&entries[a.idx], &entries[b.idx]);
        b.score
            .cmp(&a.score)
            // 同分优先浅层路径，再优先短文件名——和 fd_search 保持一致的手感
            .then_with(|| path_depth(&ea.rel).cmp(&path_depth(&eb.rel)))
            .then_with(|| ea.file_name().len().cmp(&eb.file_name().len()))
            .then_with(|| ea.file_name().cmp(eb.file_name()))
    });
    Ok(hits)
}

/// 只给最终要返回的那一页算高亮下标。
///
/// 算下标要多跑一趟回溯，几万个候选里只有几十条会被看见，
/// 打分阶段就为它们全体付这份钱毫无意义。
fn to_results(
    entries: &[IndexEntry],
    page: &[Scored],
    query: &str,
    opts: &MatchOptions,
) -> Result<Vec<SearchResult>, String> {
    let mut name_ranges: Vec<Vec<Match>> = Vec::with_capacity(page.len());

    if query.is_empty() {
        name_ranges.extend(page.iter().map(|_| Vec::new()));
    } else if opts.is_regex() {
        let matcher = build_matcher(query, opts)?;
        for s in page {
            let e = &entries[s.idx];
            let ranges = regex_match_name_or_path(&matcher, e.file_name(), &e.rel)
                .map(|(_, r)| r)
                .unwrap_or_default();
            name_ranges.push(ranges);
        }
    } else {
        let pattern = Pattern::parse(query, nucleo_case(opts), Normalization::Smart);
        let mut matcher = path_matcher();
        let mut buf: Vec<char> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        for s in page {
            let e = &entries[s.idx];
            indices.clear();
            let haystack = Utf32Str::new(&e.rel, &mut buf);
            if pattern.indices(haystack, &mut matcher, &mut indices).is_none() {
                name_ranges.push(Vec::new());
                continue;
            }
            // 多个原子的下标是各自追加进来的，要排序去重才能压成区间
            indices.sort_unstable();
            indices.dedup();
            // 落在目录段的下标没法在文件名上高亮，丢掉
            let in_name: Vec<usize> = indices
                .iter()
                .map(|&i| i as usize)
                .filter(|&i| i >= e.name_start)
                .map(|i| i - e.name_start)
                .collect();
            name_ranges.push(char_indices_to_ranges(&in_name));
        }
    }

    Ok(page
        .iter()
        .zip(name_ranges)
        .map(|(s, name_matches)| {
            let e = &entries[s.idx];
            SearchResult {
                path: e.path.clone(),
                file_name: e.file_name().to_string(),
                preview: None,
                line_number: None,
                matches: vec![],
                score: s.score,
                modified_ms: e.modified_ms,
                name_matches,
                rel_path: Some(e.rel.clone()),
            }
        })
        .collect())
}

/// 拿一份当前可用的候选表；返回 None 表示"索引不能用，请走 fd_search"。
fn take_snapshot(root_key: &str) -> Option<Arc<Vec<IndexEntry>>> {
    let mut guard = INDEX.write();
    let state = guard.as_mut()?;
    if state.root_key != root_key || !state.ready {
        return None;
    }
    Some(state.snapshot())
}

#[tauri::command]
pub async fn file_index_status() -> IndexStatus {
    current_status()
}

/// 让索引对上这个根。已经就绪就什么都不做；否则起一轮后台构建，
/// 构建完会发 `file_index_ready` 事件。
#[tauri::command]
pub async fn file_index_ensure(
    app: AppHandle,
    path: String,
    force: Option<bool>,
) -> Result<IndexStatus, String> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err("索引根不是有效目录".into());
    }
    start_build(root, Some(app), force.unwrap_or(false));
    Ok(current_status())
}

/// 放掉索引和监听。换库、关窗口时调用，不然一个再也用不到的目录会一直被监听着。
#[tauri::command]
pub async fn file_index_release() -> IndexStatus {
    *INDEX.write() = None;
    *PENDING_ROOT.write() = None;
    info!("file_index 已释放");
    IndexStatus::empty()
}

/// 走索引的文件名搜索。参数和 `fd_search` 一模一样，前端可以直接换命令名。
///
/// 索引不可用时（还没建好、换了根、或者用了索引表达不了的选项）会透明地
/// 退回 `fd_search`，所以调用方永远不需要判断"该用哪个"。
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn file_index_search(
    app: AppHandle,
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
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err("路径不存在".into());
    }
    let opts = options.clone().unwrap_or_default();
    let root_key = path_key(&path);

    // 全词匹配是 grep 的词边界语义，模糊匹配没有对应概念。
    // 与其给出一份"看着像但其实不一样"的结果，不如退回去老老实实走一遍。
    let usable = !opts.is_whole_word();
    let snapshot = if usable { take_snapshot(&root_key) } else { None };

    let Some(entries) = snapshot else {
        // 顺手把索引建起来，下一次敲键就能享受到
        start_build(root, Some(app), false);
        return super::searcher::fd_search(
            pattern,
            path,
            limit,
            offset,
            search_id,
            extensions,
            modified_after,
            modified_before,
            options,
        )
        .await;
    };

    let started = Instant::now();
    let query = pattern.trim();
    if let Some(id) = &search_id {
        register_search_id(id);
    }
    let filters = Filters::new(extensions, modified_after, modified_before);
    let ranked = rank(&entries, query, &opts, &filters, search_id.as_deref());
    let ranked = match ranked {
        Ok(v) => v,
        Err(err) => {
            // 模式非法也要摘掉登记，否则这个 id 会一直挂在活跃表里
            if let Some(id) = &search_id {
                release_search_id(id);
            }
            return Err(err);
        }
    };

    let total = ranked.len();
    let (page, has_more) = paginate_scored(&ranked, limit, offset);
    let results = to_results(&entries, page, query, &opts);
    if let Some(id) = &search_id {
        release_search_id(id);
    }
    let results = results?;

    info!(
        "file_index_search: query={:?}, 候选={}, 命中={}, 返回={}, 耗时={:?}",
        query,
        entries.len(),
        total,
        results.len(),
        started.elapsed()
    );
    Ok((results, has_more))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(rel: &str) -> IndexEntry {
        IndexEntry {
            path: format!("D:/lib/{}", rel).replace('/', "\\"),
            rel: rel.to_string(),
            name_start: match rel.rfind('/') {
                Some(i) => rel[..=i].chars().count(),
                None => 0,
            },
            modified_ms: Some(0),
            ext: rel
                .rsplit_once('.')
                .map(|(_, e)| e.to_lowercase())
                .unwrap_or_default(),
        }
    }

    fn ranked_rels(entries: &[IndexEntry], query: &str) -> Vec<String> {
        let opts = MatchOptions::default();
        let filters = Filters::new(None, None, None);
        rank(entries, query, &opts, &filters, None)
            .unwrap()
            .into_iter()
            .map(|s| entries[s.idx].rel.clone())
            .collect()
    }

    #[test]
    fn name_start_is_measured_in_chars_not_bytes() {
        // 中文目录名下的文件名偏移必须按字符算，按字节算会把高亮错位
        let e = entry("笔记/日记/今天.md");
        assert_eq!(e.name_start, "笔记/日记/".chars().count());
        assert_eq!(e.file_name(), "今天.md");
    }

    #[test]
    fn path_query_matches_directory_segments() {
        let entries = vec![entry("src/components/Header.vue"), entry("README.md")];
        let hits = ranked_rels(&entries, "components/header");
        assert_eq!(hits, vec!["src/components/Header.vue"]);
    }

    #[test]
    fn filename_hit_outranks_directory_only_hit() {
        // nucleo 的路径模式会给分隔符后的首字符加权，文件名命中应当自然排前面
        let entries = vec![entry("header/notes/random.md"), entry("src/Header.vue")];
        let hits = ranked_rels(&entries, "header");
        assert_eq!(hits[0], "src/Header.vue");
    }

    #[test]
    fn whitespace_splits_into_independent_atoms() {
        // "guaxi header" 应当能命中 GuaxiHeader.vue，顺序无关但都必须出现
        let entries = vec![
            entry("src/components/GuaxiHeader.vue"),
            entry("src/components/GuaxiFooter.vue"),
        ];
        let hits = ranked_rels(&entries, "guaxi header");
        assert_eq!(hits, vec!["src/components/GuaxiHeader.vue"]);
    }

    #[test]
    fn empty_query_lists_everything_by_mtime() {
        let mut old = entry("a.md");
        old.modified_ms = Some(1);
        let mut fresh = entry("b.md");
        fresh.modified_ms = Some(999);
        let entries = vec![old, fresh];
        assert_eq!(ranked_rels(&entries, ""), vec!["b.md", "a.md"]);
    }

    #[test]
    fn extension_filter_uses_indexed_metadata() {
        let entries = vec![entry("a.md"), entry("b.rs")];
        let filters = Filters::new(Some(vec![".MD".into()]), None, None);
        let hits = rank(&entries, "", &MatchOptions::default(), &filters, None).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(entries[hits[0].idx].rel, "a.md");
    }

    #[test]
    fn mtime_filter_excludes_out_of_range() {
        let mut e = entry("a.md");
        e.modified_ms = Some(500);
        let entries = vec![e];
        let after = Filters::new(None, Some(600), None);
        assert!(rank(&entries, "", &MatchOptions::default(), &after, None)
            .unwrap()
            .is_empty());
        let before = Filters::new(None, None, Some(400));
        assert!(rank(&entries, "", &MatchOptions::default(), &before, None)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn highlight_ranges_are_relative_to_the_file_name() {
        let entries = vec![entry("src/components/Header.vue")];
        let hits = ranked_rels(&entries, "header");
        assert_eq!(hits.len(), 1);
        let scored = rank(
            &entries,
            "header",
            &MatchOptions::default(),
            &Filters::new(None, None, None),
            None,
        )
        .unwrap();
        let results = to_results(&entries, &scored, "header", &MatchOptions::default()).unwrap();
        let ranges = &results[0].name_matches;
        assert_eq!(ranges.len(), 1);
        // "Header.vue" 的前 6 个字符
        assert_eq!((ranges[0].start, ranges[0].end), (0, 6));
    }

    #[test]
    fn regex_mode_matches_against_the_relative_path() {
        let entries = vec![entry("src/a1.md"), entry("src/bb.md")];
        let opts = MatchOptions {
            regex: Some(true),
            ..Default::default()
        };
        let hits = rank(
            &entries,
            r"a\d",
            &opts,
            &Filters::new(None, None, None),
            None,
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(entries[hits[0].idx].rel, "src/a1.md");
    }

    #[test]
    fn invalid_regex_reports_an_error() {
        let entries = vec![entry("a.md")];
        let opts = MatchOptions {
            regex: Some(true),
            ..Default::default()
        };
        assert!(rank(&entries, "(", &opts, &Filters::new(None, None, None), None).is_err());
    }

    #[test]
    fn pagination_keeps_the_same_semantics_as_fd_search() {
        let hits: Vec<Scored> = (0..10).map(|idx| Scored { idx, score: 0 }).collect();
        let (page, has_more) = paginate_scored(&hits, Some(3), Some(0));
        assert_eq!(page.len(), 3);
        assert!(has_more);
        // Some(0) = 全量，不再声称还有更多
        let (all, more) = paginate_scored(&hits, Some(0), Some(2));
        assert_eq!(all.len(), 8);
        assert!(!more);
        // offset 越界不能 panic
        let (none, more) = paginate_scored(&hits, Some(5), Some(99));
        assert!(none.is_empty());
        assert!(!more);
    }

    #[test]
    fn path_key_folds_case_and_separators_on_windows() {
        assert_eq!(path_key(r"D:\Lib\A.md"), path_key("D:/Lib/A.md"));
        if cfg!(windows) {
            assert_eq!(path_key(r"D:\lib\a.md"), path_key(r"D:\LIB\A.MD"));
        }
    }
}
