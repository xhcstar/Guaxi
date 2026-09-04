//! 正文全文索引：tantivy 建倒排索引，jieba 负责中文分词。
//!
//! 为什么不是"再跑一遍 grep"：`rg_search` 每次调用都要把全库文件从头读一遍。
//! 库大了以后逐字符预览根本跟不上，而且 grep 只认字面子串——
//! 「增强 搜索」这两个词分散在同一篇文章里，它永远找不到。
//!
//! 四个设计取舍：
//!
//! 1. **tantivy 只回答"哪些文件相关、谁更相关"。** 行号和预览仍由
//!    `searcher::grep_file_matches` 去现读那几个命中文件。把行号也塞进倒排索引
//!    会让索引大一个量级，而且预览逻辑有两份的话迟早会不一致。
//! 2. **分词是 jieba 的 search 模式 + 小写化。** search 模式会额外切出较短的
//!    子词，召回优先——搜索框要的是"先看见"，不是"切得漂亮"。
//! 3. **查询的最后一个词按前缀放宽。** 用户是逐字符敲的，「搜索增」在词典里
//!    并不是一个词；不做前缀匹配的话打字到一半会突然一条结果都没有。
//! 4. **只给预览用，正式搜索仍走 `rg_search`。** 索引落后于磁盘的窗口再小也存在，
//!    按下回车要的那份结果必须是当下磁盘的真实状态。
use ignore::WalkBuilder;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::RwLock;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, BoostQuery, Occur, Query, QueryParser, RegexQuery, TermQuery};
use tantivy::schema::{
    Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value, STORED, STRING,
};
use tantivy::tokenizer::{LowerCaser, TextAnalyzer, TokenStream};
use tantivy::{Index, IndexReader, ReloadPolicy, TantivyDocument, Term};
use tauri::{AppHandle, Emitter, Manager};
use tracing::{info, warn};

use super::file_index::path_key;
use super::searcher::{
    build_matcher, grep_file_matches, has_ignored_dir, register_search_id, rel_display_path,
    release_search_id, search_id_cancelled, should_skip_path, MatchOptions, SearchResult,
};

/// 索引结构变了就得整个重建，靠这个版本号判断磁盘上那份还能不能用。
const SCHEMA_VERSION: u32 = 1;
/// 分词器在 tantivy 里的注册名。schema 里按名字引用，打开旧索引时必须重新注册同名的一份。
const TOKENIZER: &str = "jieba_lc";
/// 单文件正文上限。超过这个大小的不进索引——它们几乎都是日志和数据导出，
/// 出现在全文搜索里只会淹没笔记，真要找还有回车那一路的 grep 兜着。
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
/// 写入器的内存预算，tantivy 会按线程均分。
const WRITER_HEAP: usize = 64 * 1024 * 1024;
/// 增量落一两个文件时用的小预算。tantivy 的下限是 15MB，低于它会直接报错。
const SMALL_HEAP: usize = 16 * 1024 * 1024;
/// 每个命中文件最多读几行做预览。命中行取这几行里最相关的那一条。
const PREVIEW_LINES: usize = 4;
/// 文件名字段的加权。标题命中比正文里提一句更值得排前面。
const NAME_BOOST: f32 = 3.0;
/// 事件安静这么久就落一批改动。比文件名索引长一些——重建正文要读文件内容，
/// 编辑器连续保存时不值得每次都重读。
const DEBOUNCE: Duration = Duration::from_millis(500);
/// 事件不停时的强制落地上限。
const MAX_DEBOUNCE: Duration = Duration::from_secs(5);
/// 待处理路径攒到这个数就改为整体增量扫描，省得逐个处理几千条事件。
const PENDING_CAP: usize = 2048;

/// 认得的正文扩展名。白名单而不是黑名单：进索引意味着要把整个文件读进内存分词，
/// 拿不准的类型宁可漏掉，也不能把一个 200MB 的二进制文件塞进去。
const TEXT_EXTS: &[&str] = &[
    "md", "markdown", "mdx", "txt", "text", "rst", "org", "adoc", "asciidoc", "tex", "typ", "bib",
    "json", "jsonc", "json5", "yaml", "yml", "toml", "ini", "cfg", "conf", "properties", "env",
    "csv", "tsv", "log", "xml", "svg", "html", "htm", "xhtml", "css", "scss", "sass", "less",
    "js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx", "vue", "svelte", "astro",
    "py", "pyi", "rs", "go", "java", "kt", "kts", "scala", "clj", "ex", "exs", "erl", "hs",
    "c", "h", "cc", "cpp", "cxx", "hpp", "hh", "hxx", "cs", "rb", "php", "pl", "pm", "lua",
    "sh", "bash", "zsh", "fish", "ps1", "psm1", "bat", "cmd", "sql", "graphql", "gql", "proto",
    "r", "jl", "dart", "swift", "m", "mm", "vim", "ipynb", "srt", "vtt", "ass", "diff", "patch",
];

/// 这个扩展名的文件值不值得进正文索引。
fn is_text_ext(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => {
            let lower = ext.to_ascii_lowercase();
            TEXT_EXTS.iter().any(|known| *known == lower)
        }
        None => false,
    }
}

/// 索引里的字段句柄。`Field` 是个 u32，整组复制比每次按名字查便宜。
#[derive(Debug, Clone, Copy)]
struct Fields {
    /// 绝对路径。按 raw 分词（STRING）才能当主键用 `delete_term` 精确删除。
    path: Field,
    rel: Field,
    name: Field,
    body: Field,
    mtime: Field,
    ext: Field,
}

/// 建 schema。正文只索引不存储——预览要现读文件，存一份正文纯属浪费磁盘。
fn build_schema() -> Schema {
    let indexing = TextFieldIndexing::default()
        .set_tokenizer(TOKENIZER)
        // 位置信息是短语查询（"完整这句话"）的前提，索引大一点换语法完整
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let analyzed = TextOptions::default().set_indexing_options(indexing);

    let mut builder = Schema::builder();
    builder.add_text_field("path", STRING | STORED);
    builder.add_text_field("rel", STORED);
    builder.add_text_field("name", analyzed.clone());
    builder.add_text_field("body", analyzed);
    builder.add_i64_field("mtime", STORED);
    builder.add_text_field("ext", STORED);
    builder.build()
}

/// 按名字取字段句柄。少一个字段就说明磁盘上那份不是当前 schema 建的。
fn fields_from(schema: &Schema) -> Option<Fields> {
    Some(Fields {
        path: schema.get_field("path").ok()?,
        rel: schema.get_field("rel").ok()?,
        name: schema.get_field("name").ok()?,
        body: schema.get_field("body").ok()?,
        mtime: schema.get_field("mtime").ok()?,
        ext: schema.get_field("ext").ok()?,
    })
}

/// 磁盘上跟索引放在一起的清单：记着每个文件上次进索引时的 mtime。
/// 有它才能做增量——重建时只读改过的那些文件。清单丢了就等于全量重建，不会出错。
#[derive(Debug, Default, Serialize, Deserialize)]
struct Manifest {
    version: u32,
    root: String,
    /// `path_key` -> 上次入库时的路径与 mtime。键做过大小写折叠，
    /// 值里的 `path` 是索引里真正那一份，删除时要拿它去 `delete_term`。
    files: HashMap<String, ManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestEntry {
    path: String,
    mtime: i64,
}

fn manifest_path(dir: &Path) -> PathBuf {
    dir.join("manifest.json")
}

fn load_manifest(dir: &Path) -> Option<Manifest> {
    let raw = std::fs::read_to_string(manifest_path(dir)).ok()?;
    serde_json::from_str::<Manifest>(&raw).ok()
}

/// 先写临时文件再改名。清单写坏了会让增量判断全错，宁可整份丢掉重建。
fn save_manifest(dir: &Path, manifest: &Manifest) {
    let tmp = dir.join("manifest.json.tmp");
    let body = match serde_json::to_vec(manifest) {
        Ok(body) => body,
        Err(err) => {
            warn!("fulltext 清单序列化失败: {}", err);
            return;
        }
    };
    if let Err(err) = std::fs::write(&tmp, &body) {
        warn!("fulltext 清单写入失败: {}", err);
        return;
    }
    if let Err(err) = std::fs::rename(&tmp, manifest_path(dir)) {
        warn!("fulltext 清单改名失败: {}", err);
    }
}

/// 索引目录：`<appData>/fulltext/<根路径哈希>`。
///
/// 用哈希而不是把路径原样嵌进目录名，是因为路径里可能有冒号、空格和中文，
/// 而且长度没上限——Windows 的 260 字符限制很容易被一个深目录顶穿。
fn index_dir(app: &AppHandle, root: &Path) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("取应用数据目录失败: {}", e))?;
    let hex = blake3::hash(path_key(&root.to_string_lossy()).as_bytes()).to_hex();
    Ok(base.join("fulltext").join(&hex[..16]))
}

/// 常驻状态。`Index` 和 `IndexReader` 内部都是 `Arc`，克隆很便宜——
/// 搜索时先克隆出来再放锁，几十毫秒的查询不会挡住维护线程。
struct FulltextState {
    root: PathBuf,
    root_key: String,
    dir: PathBuf,
    index: Index,
    reader: IndexReader,
    fields: Fields,
    manifest: Manifest,
    /// false 表示首轮同步还没跑完，此时查询要退回 grep。
    ready: bool,
    generation: u64,
    _watcher: Option<RecommendedWatcher>,
}

static INDEX: RwLock<Option<FulltextState>> = RwLock::new(None);
/// 同一时刻只允许一轮写入：tantivy 的 `IndexWriter` 会独占目录锁。
static BUILDING: AtomicU64 = AtomicU64::new(0);
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// 构建期间又来的请求记在这里，当前这轮结束后补做。
static PENDING_ROOT: RwLock<Option<PathBuf>> = RwLock::new(None);

#[derive(Debug, Clone, Serialize)]
pub struct FulltextStatus {
    pub root: String,
    pub file_count: usize,
    pub ready: bool,
    pub building: bool,
}

impl FulltextStatus {
    fn empty() -> Self {
        Self {
            root: String::new(),
            file_count: 0,
            ready: false,
            building: BUILDING.load(Ordering::SeqCst) != 0,
        }
    }
}

fn current_status() -> FulltextStatus {
    let building = BUILDING.load(Ordering::SeqCst) != 0;
    match INDEX.read().as_ref() {
        Some(state) => FulltextStatus {
            root: state.root.to_string_lossy().to_string(),
            file_count: state.manifest.files.len(),
            ready: state.ready,
            building,
        },
        None => FulltextStatus::empty(),
    }
}

/// jieba + 小写化。jieba 本身只切词不改大小写，不接 `LowerCaser` 的话
/// 搜 `readme` 找不到 `README`。
fn analyzer() -> TextAnalyzer {
    TextAnalyzer::builder(tantivy_jieba::JiebaTokenizer::new())
        .filter(LowerCaser)
        .build()
}

/// 打开或新建索引。
///
/// 打不开就整份丢掉重建，而不是想办法修：这份索引是纯派生数据，
/// 重建的代价只是一次扫盘，而带着半坏的索引跑下去会给出莫名其妙的搜索结果。
fn open_or_create(dir: &Path, root: &Path, force: bool) -> Result<(Index, Fields, Manifest), String> {
    let root_key = path_key(&root.to_string_lossy());

    let stale = match load_manifest(dir) {
        Some(m) => m.version != SCHEMA_VERSION || path_key(&m.root) != root_key,
        None => true,
    };

    if (stale || force) && dir.exists() {
        // 只删我们自己那层哈希目录，父目录名必须是 fulltext——
        // 万一 index_dir 的拼法以后变了，也不至于把别的目录整个删掉
        let sane = dir.parent().and_then(|p| p.file_name()) == Some(std::ffi::OsStr::new("fulltext"));
        if sane {
            if let Err(err) = std::fs::remove_dir_all(dir) {
                warn!("fulltext 清理旧索引失败: {}", err);
            }
        } else {
            return Err(format!("索引目录不在预期位置，拒绝清理: {}", dir.display()));
        }
    }

    std::fs::create_dir_all(dir).map_err(|e| format!("创建索引目录失败: {}", e))?;

    let index = match Index::open_in_dir(dir) {
        Ok(index) => index,
        Err(_) => Index::create_in_dir(dir, build_schema())
            .map_err(|e| format!("创建正文索引失败: {}", e))?,
    };
    // 分词器是按名字挂在 schema 上的，每次拿到 Index 都得重新注册一遍
    index.tokenizers().register(TOKENIZER, analyzer());

    // 字段句柄从"实际打开的那份 schema"里取，而不是用刚建的 builder 返回值。
    // 万一磁盘上是旧版本留下的索引，字段编号对不上会静默写错字段。
    let fields = fields_from(&index.schema())
        .ok_or_else(|| "索引字段与当前版本不符".to_string())?;

    let manifest = match load_manifest(dir) {
        Some(m) if m.version == SCHEMA_VERSION && path_key(&m.root) == root_key && !force => m,
        _ => Manifest {
            version: SCHEMA_VERSION,
            root: root.to_string_lossy().to_string(),
            files: HashMap::new(),
        },
    };

    Ok((index, fields, manifest))
}

/// 一轮同步的统计，只用来写日志。
#[derive(Debug, Default)]
struct SyncStats {
    indexed: usize,
    removed: usize,
    unchanged: usize,
    failed: usize,
}

fn meta_mtime_ms(meta: &std::fs::Metadata) -> Option<i64> {
    let dur = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(dur.as_millis() as i64)
}

/// 走一遍目录，挑出值得进正文索引的文件，顺手带上 mtime。
///
/// mtime 从 `DirEntry::metadata` 拿而不是再 `stat` 一次——目录遍历本来就读到了
/// 这份元数据，白拿的东西没理由再花一次系统调用。
fn collect_candidates(root: &Path) -> Vec<(PathBuf, i64)> {
    WalkBuilder::new(root)
        .hidden(false)
        .add_custom_ignore_filename(".ignore")
        .filter_entry(|e| !has_ignored_dir(e.path()))
        .build()
        .filter_map(|result| match result {
            Ok(entry) => Some(entry),
            Err(err) => {
                warn!("fulltext 遍历路径失败: {}", err);
                None
            }
        })
        .filter(|entry| entry.file_type().map_or(false, |ft| ft.is_file()))
        .filter(|entry| !should_skip_path(entry.path()) && is_text_ext(entry.path()))
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            if meta.len() > MAX_FILE_BYTES {
                return None;
            }
            Some((entry.path().to_path_buf(), meta_mtime_ms(&meta).unwrap_or(0)))
        })
        .collect()
}

/// 这一轮同步管多大范围。
enum Scope<'a> {
    /// 走一遍全库并对账：清单里有、磁盘上没了的一并摘掉。首轮和目录级变动用这个。
    Full,
    /// 只处理监听线程报上来的这些路径。保存一次文件不该换来一次全库遍历。
    Paths(&'a [PathBuf]),
}

/// 把一个键连同它对应的文档从索引里摘掉。
fn remove_key(
    writer: &tantivy::IndexWriter,
    fields: &Fields,
    manifest: &mut Manifest,
    key: &str,
    stats: &mut SyncStats,
) {
    if let Some(entry) = manifest.files.remove(key) {
        writer.delete_term(Term::from_field_text(fields.path, &entry.path));
        stats.removed += 1;
    }
}

/// 把这个键、以及它名下的所有条目都摘掉。目录被删时 notify 只报目录本身，
/// 里面的文件得靠前缀自己找出来。
fn remove_subtree(
    writer: &tantivy::IndexWriter,
    fields: &Fields,
    manifest: &mut Manifest,
    key: &str,
    stats: &mut SyncStats,
) {
    let prefix = format!("{}/", key);
    let doomed: Vec<String> = manifest
        .files
        .keys()
        .filter(|k| {
            let k: &str = k;
            k == key || k.starts_with(&prefix)
        })
        .cloned()
        .collect();
    for k in doomed {
        remove_key(writer, fields, manifest, &k, stats);
    }
}

/// 挑出本轮要重读的文件，同时把已经消失的条目摘掉。
fn plan_sync(
    writer: &tantivy::IndexWriter,
    fields: &Fields,
    root: &Path,
    manifest: &mut Manifest,
    scope: &Scope<'_>,
    stats: &mut SyncStats,
) -> Vec<(PathBuf, String, i64)> {
    let mut todo: Vec<(PathBuf, String, i64)> = Vec::new();

    match scope {
        Scope::Full => {
            let candidates = collect_candidates(root);
            let mut seen: HashSet<String> = HashSet::with_capacity(candidates.len());
            for (path, mtime) in candidates {
                let key = path_key(&path.to_string_lossy());
                match manifest.files.get(&key) {
                    Some(entry) if entry.mtime == mtime => stats.unchanged += 1,
                    _ => todo.push((path, key.clone(), mtime)),
                }
                seen.insert(key);
            }
            let gone: Vec<String> = manifest
                .files
                .keys()
                .filter(|k| !seen.contains(*k))
                .cloned()
                .collect();
            for key in gone {
                remove_key(writer, fields, manifest, &key, stats);
            }
        }
        Scope::Paths(paths) => {
            for path in paths.iter() {
                if !path.starts_with(root) {
                    continue;
                }
                let key = path_key(&path.to_string_lossy());
                let qualifies = path.is_file()
                    && !has_ignored_dir(path)
                    && !should_skip_path(path)
                    && is_text_ext(path);
                if !qualifies {
                    // 删了、改名了、或者换成了不该进索引的类型——都按"没了"处理
                    remove_subtree(writer, fields, manifest, &key, stats);
                    continue;
                }
                let Ok(meta) = std::fs::metadata(path) else {
                    continue;
                };
                if meta.len() > MAX_FILE_BYTES {
                    remove_subtree(writer, fields, manifest, &key, stats);
                    continue;
                }
                let mtime = meta_mtime_ms(&meta).unwrap_or(0);
                match manifest.files.get(&key) {
                    Some(entry) if entry.mtime == mtime => stats.unchanged += 1,
                    _ => todo.push((path.clone(), key, mtime)),
                }
            }
        }
    }

    todo
}

/// 真正把改动写进索引。
///
/// `add_document` 只要 `&self`，所以读文件和灌文档可以整个并行——
/// 这一步的瓶颈是磁盘和分词，串行做会把首轮建索引拖成分钟级。
fn sync(
    index: &Index,
    reader: &IndexReader,
    fields: &Fields,
    root: &Path,
    manifest: &mut Manifest,
    scope: Scope<'_>,
    app: Option<&AppHandle>,
    heap: usize,
) -> Result<SyncStats, String> {
    let mut writer = index
        .writer::<TantivyDocument>(heap)
        .map_err(|e| format!("打开正文索引写入器失败: {}", e))?;

    let mut stats = SyncStats::default();
    let todo = plan_sync(&writer, fields, root, manifest, &scope, &mut stats);

    let total = todo.len();
    let done = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    let indexed: Vec<(String, ManifestEntry)> = todo
        .par_iter()
        .filter_map(|(path, key, mtime)| {
            let bytes = match std::fs::read(path) {
                Ok(bytes) => bytes,
                Err(err) => {
                    warn!("fulltext 读取文件失败: path={}, error={}", path.display(), err);
                    failed.fetch_add(1, Ordering::Relaxed);
                    return None;
                }
            };
            // 扩展名白名单挡不住"后缀对但内容是二进制"的文件，NUL 字节是最省的判据
            if bytes.iter().take(8192).any(|b| *b == 0) {
                return None;
            }

            let path_str = path.to_string_lossy().to_string();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();

            let mut doc = TantivyDocument::new();
            doc.add_text(fields.path, &path_str);
            doc.add_text(fields.rel, rel_display_path(root, path));
            doc.add_text(fields.name, &name);
            doc.add_text(fields.body, String::from_utf8_lossy(&bytes).as_ref());
            doc.add_i64(fields.mtime, *mtime);
            doc.add_text(fields.ext, &ext);

            // 同一路径的旧文档必须先删，否则文件每改一次就多出一条重复命中。
            // delete 的 opstamp 一定小于紧跟其后的 add，所以不会把新文档也删掉。
            writer.delete_term(Term::from_field_text(fields.path, &path_str));
            if let Err(err) = writer.add_document(doc) {
                warn!("fulltext 写入文档失败: path={}, error={}", path.display(), err);
                failed.fetch_add(1, Ordering::Relaxed);
                return None;
            }

            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            if n % 500 == 0 {
                if let Some(app) = app {
                    let _ = app.emit("fulltext_progress", (n, total));
                }
            }
            Some((
                key.clone(),
                ManifestEntry {
                    path: path_str,
                    mtime: *mtime,
                },
            ))
        })
        .collect();

    for (key, entry) in indexed {
        manifest.files.insert(key, entry);
        stats.indexed += 1;
    }
    stats.failed = failed.load(Ordering::Relaxed);

    writer
        .commit()
        .map_err(|e| format!("提交正文索引失败: {}", e))?;
    // reader 用的是 Manual 策略，提交完必须自己喊一声，否则查的还是旧段
    if let Err(err) = reader.reload() {
        warn!("fulltext 刷新读取器失败: {}", err);
    }

    Ok(stats)
}

fn is_current_root(root_key: &str) -> bool {
    INDEX
        .read()
        .as_ref()
        .map_or(false, |state| state.root_key == root_key)
}

enum ApplyOutcome {
    Applied,
    /// 现在落不下去（索引没就绪，或者有别的写入在跑），改动攥着等下一轮。
    Deferred,
    /// 增量搞不定，需要整体重扫。
    NeedsRescan,
}

/// 把监听线程攒下的这批路径增量落进索引。
fn apply_paths(root_key: &str, paths: &[PathBuf]) -> ApplyOutcome {
    // 新建或改名一个目录时 notify 只报目录本身，里面的文件一个都拿不到，只能重扫
    if paths.iter().any(|p| p.is_dir()) {
        return ApplyOutcome::NeedsRescan;
    }

    let (index, reader, fields, root, dir, generation) = {
        let guard = INDEX.read();
        let Some(state) = guard.as_ref() else {
            return ApplyOutcome::NeedsRescan;
        };
        if state.root_key != root_key {
            return ApplyOutcome::NeedsRescan;
        }
        if !state.ready {
            return ApplyOutcome::Deferred;
        }
        (
            state.index.clone(),
            state.reader.clone(),
            state.fields,
            state.root.clone(),
            state.dir.clone(),
            state.generation,
        )
    };

    // 写入器要独占目录锁，所以增量和全量共用同一道闸门
    if BUILDING.swap(1, Ordering::SeqCst) != 0 {
        return ApplyOutcome::Deferred;
    }

    // 清单借出来在锁外面改。借出期间 file_count 会显示 0，
    // 但增量只有几个文件，这个窗口是毫秒级的。
    let mut manifest = {
        let mut guard = INDEX.write();
        match guard.as_mut() {
            Some(state) if state.generation == generation => std::mem::take(&mut state.manifest),
            _ => {
                BUILDING.store(0, Ordering::SeqCst);
                return ApplyOutcome::NeedsRescan;
            }
        }
    };

    let result = sync(
        &index,
        &reader,
        &fields,
        &root,
        &mut manifest,
        Scope::Paths(paths),
        None,
        SMALL_HEAP,
    );
    save_manifest(&dir, &manifest);

    let mut restored = false;
    if let Some(state) = INDEX.write().as_mut() {
        if state.generation == generation {
            state.manifest = manifest;
            restored = true;
        }
    }
    BUILDING.store(0, Ordering::SeqCst);

    match result {
        Ok(stats) if restored => {
            info!(
                "fulltext 增量落地: 新增={}, 移除={}, 未变={}, 失败={}",
                stats.indexed, stats.removed, stats.unchanged, stats.failed
            );
            ApplyOutcome::Applied
        }
        Ok(_) => ApplyOutcome::NeedsRescan,
        Err(err) => {
            warn!("fulltext 增量落地失败: {}", err);
            ApplyOutcome::NeedsRescan
        }
    }
}

/// 这条事件路径值不值得排队。
///
/// 存在、不是目录、又不是正文类型的直接扔掉——图片和二进制文件的写入是最吵的
/// 一类事件，留着它们只会让去抖白转一圈。不存在的路径一律留下：
/// 那可能是刚被删掉的文件或目录，删除必须落到索引里。
fn worth_queueing(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(meta) => meta.is_dir() || is_text_ext(path),
        Err(_) => true,
    }
}

fn spawn_watcher(root: PathBuf, app: AppHandle) -> Option<RecommendedWatcher> {
    let root_key = path_key(&root.to_string_lossy());
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(err) => {
            warn!("fulltext 创建监听失败，正文索引将不再自动刷新: {}", err);
            return None;
        }
    };
    if let Err(err) = watcher.watch(&root, RecursiveMode::Recursive) {
        warn!("fulltext 监听 {} 失败: {}", root.display(), err);
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
                            if has_ignored_dir(&p) || !worth_queueing(&p) {
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
                    first_seen.map_or(false, |t| t.elapsed() >= MAX_DEBOUNCE)
                }
                Ok(Err(err)) => {
                    warn!("fulltext 监听事件出错: {}", err);
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
                start_build(root.clone(), app.clone(), BuildMode::Resync);
                continue;
            }
            // 同一个文件在一轮里可能被报好几次，去重省得重复读盘
            pending.sort();
            pending.dedup();
            match apply_paths(&root_key, &pending) {
                ApplyOutcome::Applied => {
                    pending.clear();
                    first_seen = None;
                }
                // 攥着不动，等下一轮再落
                ApplyOutcome::Deferred => {}
                ApplyOutcome::NeedsRescan => {
                    pending.clear();
                    first_seen = None;
                    start_build(root.clone(), app.clone(), BuildMode::Resync);
                }
            }
        }
        info!("fulltext 监听线程退出: {}", root.display());
    });

    Some(watcher)
}

/// 这一轮构建要做到哪一步。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuildMode {
    /// 已经就绪就什么都不做。挂载、换根、以及搜索时顺手催一把用这个。
    IfNeeded,
    /// 重新对账一遍。索引留着，只补差异——目录级变动之后走这条。
    Resync,
    /// 删掉磁盘上那份从头建。schema 版本变了或用户明确要求重建时用。
    Rebuild,
}

/// 起一轮构建。同一时刻只允许一轮，重复请求折叠成"结束后再来一次"。
fn start_build(root: PathBuf, app: AppHandle, mode: BuildMode) {
    let root_key = path_key(&root.to_string_lossy());
    if mode == BuildMode::IfNeeded {
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
        build_now(root, app.clone(), mode);
        // 先放开闸门再看队列：宁可多建一次，也不能把请求漏掉
        BUILDING.store(0, Ordering::SeqCst);
        let next = PENDING_ROOT.write().take();
        if let Some(next) = next {
            start_build(next, app, BuildMode::Resync);
        }
    });
}

/// 真正的构建流程。顺序是刻意的：先把"新的根"写进去（ready=false）并装上监听，
/// 再去扫盘。这样扫描期间发生的改动会被监听线程攥在手里，等索引就绪后补上，
/// 而不是因为"根还没换过来"被直接丢掉。
fn build_now(root: PathBuf, app: AppHandle, mode: BuildMode) {
    let root_key = path_key(&root.to_string_lossy());
    let dir = match index_dir(&app, &root) {
        Ok(dir) => dir,
        Err(err) => {
            warn!("fulltext 定位索引目录失败: {}", err);
            return;
        }
    };
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    let (index, fields, mut manifest) =
        match open_or_create(&dir, &root, mode == BuildMode::Rebuild) {
            Ok(parts) => parts,
            Err(err) => {
                warn!("fulltext 打开索引失败: {}", err);
                return;
            }
        };
    let reader: IndexReader = match index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
    {
        Ok(reader) => reader,
        Err(err) => {
            warn!("fulltext 打开读取器失败: {}", err);
            return;
        }
    };

    *INDEX.write() = Some(FulltextState {
        root: root.clone(),
        root_key: root_key.clone(),
        dir: dir.clone(),
        index: index.clone(),
        reader: reader.clone(),
        fields,
        // 真清单在下面同步完才装进来，这里先放个空的：ready=false 期间没人会用它
        manifest: Manifest::default(),
        ready: false,
        generation,
        _watcher: None,
    });

    let watcher = spawn_watcher(root.clone(), app.clone());

    let started = Instant::now();
    let stats = match sync(
        &index,
        &reader,
        &fields,
        &root,
        &mut manifest,
        Scope::Full,
        Some(&app),
        WRITER_HEAP,
    ) {
        Ok(stats) => stats,
        Err(err) => {
            warn!("fulltext 同步失败: {}", err);
            return;
        }
    };
    save_manifest(&dir, &manifest);
    let count = manifest.files.len();

    {
        let mut guard = INDEX.write();
        match guard.as_mut() {
            Some(state) if state.generation == generation => {
                state.manifest = manifest;
                state.ready = true;
                state._watcher = watcher;
            }
            // 期间被换根/释放了，这份结果作废（watcher 随本函数返回一起丢掉，监听自动停止）
            _ => {
                info!("fulltext 构建结果已过期，丢弃: {}", root.display());
                return;
            }
        }
    }

    if let Err(err) = app.emit("fulltext_ready", &current_status()) {
        warn!("fulltext 发送就绪事件失败: {}", err);
    }
    info!(
        "fulltext 就绪: root={}, files={}, 新增={}, 移除={}, 未变={}, 失败={}, 用时={:?}",
        root.display(),
        count,
        stats.indexed,
        stats.removed,
        stats.unchanged,
        stats.failed,
        started.elapsed()
    );
}

/// 转义正则元字符。前缀模式是我们自己拼的，用户敲进来的 `.`、`*` 必须当字面量。
/// 只转义 ASCII 标点——正则语法不允许转义非 ASCII 字符。
fn escape_regex(text: &str) -> String {
    const SPECIAL: &[char] = &[
        '.', '+', '*', '?', '(', ')', '|', '[', ']', '{', '}', '^', '$', '\\', '&', '~', '-', '#',
        '"', '/',
    ];
    let mut out = String::with_capacity(text.len() + 8);
    for ch in text.chars() {
        if SPECIAL.contains(&ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// 用索引里那套分词器切查询串。查询和正文必须走同一套切法，
/// 否则用户搜的词根本对不上索引里的词。
fn query_terms(raw: &str) -> Vec<String> {
    let mut analyzer = analyzer();
    let mut seen: HashSet<String> = HashSet::new();
    let mut terms: Vec<String> = Vec::new();
    let mut stream = analyzer.token_stream(raw);
    while let Some(token) = stream.next() {
        // jieba 会把空格和标点也切成 token。索引里同样存着它们，所以匹配得上，
        // 但把「,」当成必须命中的词毫无意义，还会让预览高亮满屏的逗号。
        if !token.text.chars().any(char::is_alphanumeric) {
            continue;
        }
        if !seen.insert(token.text.clone()) {
            continue;
        }
        terms.push(token.text.clone());
    }
    terms
}

/// 查询串看起来像不像 tantivy 的查询表达式。
///
/// 只是个启发式判断：猜错了也只是多试一次 `QueryParser`，解析不过就退回分词匹配。
fn looks_like_expression(raw: &str) -> bool {
    raw.contains('"')
        || raw.contains(':')
        || raw.contains('(')
        || raw.contains('+')
        || raw.contains(" -")
        || raw.starts_with('-')
        || raw.contains(" AND ")
        || raw.contains(" OR ")
}

/// 一个词在 name / body 两个字段上的精确查询。文件名命中更重要，单独加权。
fn term_queries(fields: &Fields, text: &str) -> Vec<Box<dyn Query>> {
    let name = TermQuery::new(
        Term::from_field_text(fields.name, text),
        IndexRecordOption::WithFreqs,
    );
    let body = TermQuery::new(
        Term::from_field_text(fields.body, text),
        IndexRecordOption::WithFreqs,
    );
    vec![
        Box::new(BoostQuery::new(Box::new(name), NAME_BOOST)),
        Box::new(body),
    ]
}

/// 把最后一个词放宽成前缀。
///
/// 用户是一个字一个字敲的，敲到「搜索增」时「增」还不是个词，
/// 严格匹配会在半途给出 0 结果——搜索框一闪一闪的很难用。
///
/// 单个 ASCII 字符不放宽：`a.*` 会把词典里一大半词都拖进来，代价最高收益最低。
fn prefix_queries(fields: &Fields, text: &str) -> Vec<Box<dyn Query>> {
    let mut subs = term_queries(fields, text);

    if text.chars().count() == 1 && text.is_ascii() {
        return subs;
    }

    let pattern = format!("{}.*", escape_regex(text));
    match RegexQuery::from_pattern(&pattern, fields.name) {
        Ok(query) => subs.push(Box::new(BoostQuery::new(Box::new(query), NAME_BOOST))),
        Err(err) => warn!("fulltext 构造文件名前缀查询失败: {}", err),
    }
    match RegexQuery::from_pattern(&pattern, fields.body) {
        Ok(query) => subs.push(Box::new(query)),
        Err(err) => warn!("fulltext 构造正文前缀查询失败: {}", err),
    }

    subs
}

/// 组装查询，并把用于高亮的词一起带出去。
///
/// 两条路：查询串里带了 `"`、`:`、`+`、`-` 这类记号就交给 tantivy 的 `QueryParser`
/// （用户明显在写表达式，`title:配置 -草稿` 这种得让它按语义走）；
/// 否则自己拼 —— 词之间取交集（都得出现），每个词在 name/body 上取并集，
/// 最后一个词额外放宽成前缀。
fn build_query(
    index: &Index,
    fields: &Fields,
    raw: &str,
) -> Result<(Box<dyn Query>, Vec<String>), String> {
    let terms = query_terms(raw);
    if terms.is_empty() {
        return Err("查询里没有可检索的词".to_string());
    }

    if looks_like_expression(raw) {
        let mut parser = QueryParser::for_index(index, vec![fields.name, fields.body]);
        // 默认取交集：搜「配置 加载」是想找同时提到两者的文件，不是任意一个
        parser.set_conjunction_by_default();
        parser.set_field_boost(fields.name, NAME_BOOST);
        match parser.parse_query(raw) {
            Ok(query) => return Ok((query, terms)),
            // 语法没写对就当普通词处理。搜索框不该因为一个减号就报错
            Err(err) => info!("fulltext 表达式解析失败，退回分词匹配: {}", err),
        }
    }

    let last = terms.len() - 1;
    let clauses: Vec<(Occur, Box<dyn Query>)> = terms
        .iter()
        .enumerate()
        .map(|(i, term)| {
            let subs = if i == last {
                prefix_queries(fields, term)
            } else {
                term_queries(fields, term)
            };
            let clause: Box<dyn Query> = Box::new(BooleanQuery::union(subs));
            (Occur::Must, clause)
        })
        .collect();

    Ok((Box::new(BooleanQuery::new(clauses)), terms))
}

/// 扩展名与修改时间过滤。
///
/// 这两条不做成 tantivy 的查询子句，而是拿回结果后在 Rust 里筛：
/// 存储字段本来就要读出来（路径、预览都要），顺手判一下几乎不要钱。
struct Filters {
    exts: Vec<String>,
    after: Option<i64>,
    before: Option<i64>,
}

impl Filters {
    fn new(exts: Option<Vec<String>>, after: Option<i64>, before: Option<i64>) -> Self {
        let exts = exts
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|e| !e.is_empty())
            .collect();
        Self {
            exts,
            after,
            before,
        }
    }

    fn is_noop(&self) -> bool {
        self.exts.is_empty() && self.after.is_none() && self.before.is_none()
    }

    fn accepts(&self, ext: &str, mtime: Option<i64>) -> bool {
        if !self.exts.is_empty() && !self.exts.iter().any(|e| e == ext) {
            return false;
        }
        // 索引里没记下时间的文件，一旦用户按时间筛就只能放弃它——
        // 宁可漏掉，也别把不满足条件的塞进结果里
        match (self.after, self.before) {
            (None, None) => true,
            _ => match mtime {
                Some(ms) => {
                    self.after.map_or(true, |a| ms >= a) && self.before.map_or(true, |b| ms <= b)
                }
                None => false,
            },
        }
    }
}

/// 一条文件级命中。BM25 只排到文件粒度，行号和预览随后由 grep 补。
struct Hit {
    path: String,
    rel: String,
    mtime: Option<i64>,
    score: u32,
}

/// 借出索引句柄。`Index`/`IndexReader` 内部都是 Arc，克隆很便宜，
/// 拿到手就能松锁——检索期间不该占着全局锁不放。
fn take_handle(root_key: &str) -> Option<(Index, IndexReader, Fields)> {
    let guard = INDEX.read();
    let state = guard.as_ref()?;
    if state.root_key != root_key || !state.ready {
        return None;
    }
    Some((state.index.clone(), state.reader.clone(), state.fields))
}

/// 查询当前索引状态。前端用它决定要不要显示"正在建索引"。
#[tauri::command]
pub async fn fulltext_status() -> Result<FulltextStatus, String> {
    Ok(current_status())
}

/// 确保某个根目录的正文索引可用。`force` 为真时推倒重建。
#[tauri::command]
pub async fn fulltext_ensure(app: AppHandle, path: String, force: Option<bool>) -> Result<FulltextStatus, String> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(format!("路径不是目录: {}", path));
    }
    let mode = if force.unwrap_or(false) {
        BuildMode::Rebuild
    } else {
        BuildMode::IfNeeded
    };
    start_build(root, app, mode);
    Ok(current_status())
}

/// 释放索引句柄（关闭工作区时调用）。落盘的索引留着，下次开同一个目录能直接复用。
#[tauri::command]
pub async fn fulltext_release() -> Result<(), String> {
    *PENDING_ROOT.write() = None;
    // watcher 挂在 state 上，state 一 drop 监听线程的 channel 就断，线程自己退出
    *INDEX.write() = None;
    info!("fulltext 已释放索引句柄");
    Ok(())
}

/// 正文全文检索。参数与 `rg_search` 一一对应，前端可以原地替换。
///
/// 索引不可用（还没建好、根目录换了、或者开了正则/全词）时透明退回 `rg_search`：
/// 用户不需要知道刚才走的是哪条路，只需要拿到结果。
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn fulltext_search(
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
    let started = Instant::now();
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err("路径不存在".into());
    }
    if pattern.trim().is_empty() {
        return Ok((Vec::new(), false));
    }

    let opts = options.clone().unwrap_or_default();
    // 正则和全词都是"按字符形状匹配"，倒排索引是"按词匹配"，语义对不上。
    // 与其给一份看着像其实不一样的结果，不如老实退回去 grep 一遍。
    let indexable = !opts.is_regex() && !opts.is_whole_word();
    let handle = if indexable {
        take_handle(&path_key(&path))
    } else {
        None
    };

    let Some((index, reader, fields)) = handle else {
        if indexable {
            // 顺手把索引挂上，下一次按键就能走快路
            start_build(root, app, BuildMode::IfNeeded);
        }
        return super::searcher::rg_search(
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

    if let Some(id) = &search_id {
        register_search_id(id);
    }

    let (query, terms) = match build_query(&index, &fields, &pattern) {
        Ok(parts) => parts,
        Err(err) => {
            if let Some(id) = &search_id {
                release_search_id(id);
            }
            return Err(err);
        }
    };

    let filters = Filters::new(extensions, modified_after, modified_before);
    let page_offset = offset.unwrap_or(0);
    let want_all = matches!(limit, Some(0));
    let page_size = match limit {
        Some(0) | None => 50,
        Some(n) => n,
    };

    // 过滤是拿回结果之后做的，所以要多取一些垫着，否则被筛掉的会白占名额。
    // 上限写死是为了兜住"搜一个 the 命中十万个文件"——预览这一路不承诺绝对完整。
    let top_k = if want_all {
        2000
    } else {
        let base = page_offset.saturating_add(page_size).max(50);
        if filters.is_noop() {
            base.saturating_add(1).min(2000)
        } else {
            base.saturating_mul(4).saturating_add(200).min(4000)
        }
    };

    let searcher = reader.searcher();
    // TopDocs 本身只是个"取前 K 条"的描述，order_by_score 才把它变成按 BM25 排序的收集器
    let top = match searcher.search(&query, &TopDocs::with_limit(top_k).order_by_score()) {
        Ok(top) => top,
        Err(err) => {
            if let Some(id) = &search_id {
                release_search_id(id);
            }
            return Err(format!("正文检索失败: {}", err));
        }
    };

    let mut hits: Vec<Hit> = Vec::with_capacity(top.len());
    for (score, addr) in top {
        let doc: TantivyDocument = match searcher.doc(addr) {
            Ok(doc) => doc,
            // 单条存储字段读坏了不该让整次搜索失败
            Err(err) => {
                warn!("fulltext 读取文档失败: {}", err);
                continue;
            }
        };
        let Some(hit_path) = doc.get_first(fields.path).and_then(|v| v.as_str()) else {
            continue;
        };
        let ext = doc
            .get_first(fields.ext)
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let mtime = doc.get_first(fields.mtime).and_then(|v| v.as_i64());
        if !filters.accepts(ext, mtime) {
            continue;
        }
        // 索引是异步跟文件系统对齐的，中间有窗口。已经删掉的文件不该出现在结果里
        let full = PathBuf::from(hit_path);
        if !full.is_file() {
            continue;
        }
        let rel = doc
            .get_first(fields.rel)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| rel_display_path(&root, &full));
        hits.push(Hit {
            path: hit_path.to_string(),
            rel,
            mtime,
            // BM25 是浮点分，前端那套排序和阈值都按整数走，放大一百倍取整
            score: (score * 100.0).max(0.0) as u32,
        });
    }

    let total = hits.len();
    let start = page_offset.min(total);
    let end = if want_all {
        total
    } else {
        start.saturating_add(page_size).min(total)
    };
    let has_more = !want_all && end < total;

    if search_id.as_deref().is_some_and(search_id_cancelled) {
        if let Some(id) = &search_id {
            release_search_id(id);
        }
        return Ok((Vec::new(), false));
    }

    // 索引只回答"哪些文件相关"，行号和高亮位置还得现读一遍。
    // 用 grep 而不是 tantivy 的 snippet：这样预览和回车走 rg_search 时长得一模一样，
    // 用户不会觉得"刚才看到的那一行怎么不见了"。
    let alternation = terms
        .iter()
        .map(|t| escape_regex(t))
        .collect::<Vec<_>>()
        .join("|");
    let preview_opts = MatchOptions {
        regex: Some(true),
        // 词已经被小写化过了，大小写必须放开才对得上原文
        case_sensitive: Some(false),
        whole_word: Some(false),
    };
    let matcher = build_matcher(&format!("(?:{})", alternation), &preview_opts).ok();

    let results: Vec<SearchResult> = hits[start..end]
        .par_iter()
        .map(|hit| {
            let full = Path::new(&hit.path);
            let best = matcher.as_ref().and_then(|m| {
                if search_id.as_deref().is_some_and(search_id_cancelled) {
                    return None;
                }
                grep_file_matches(m, full, &hit.rel, hit.mtime, Some(PREVIEW_LINES))
                    .and_then(|(_, mut rows)| (!rows.is_empty()).then(|| rows.remove(0)))
            });

            let mut row = best.unwrap_or_else(|| SearchResult {
                // 命中也可能只落在文件名上，或者 jieba 的切词和 grep 的字面匹配对不上。
                // 这种文件仍然要列出来——它确实相关，只是没有可高亮的那一行。
                path: hit.path.clone(),
                file_name: full
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                preview: None,
                line_number: None,
                matches: Vec::new(),
                score: 0,
                modified_ms: hit.mtime,
                name_matches: Vec::new(),
                rel_path: Some(hit.rel.clone()),
            });
            // 排序权归 BM25，行内得分只用来挑"这个文件里哪一行最像"
            row.score = hit.score;
            row
        })
        .collect();

    if let Some(id) = &search_id {
        release_search_id(id);
    }

    info!(
        "fulltext 检索完成: pattern={}, 词={:?}, 候选={}, 返回={}, 用时={:?}",
        pattern,
        terms,
        total,
        results.len(),
        started.elapsed()
    );

    Ok((results, has_more))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 建一个内存索引塞几篇文档。测的是"分词 + 查询构造"这一层，
    /// 不碰磁盘也不碰 AppHandle——那两样在单测里既慢又难摆。
    fn index_with(docs: &[(&str, &str)]) -> (Index, Fields) {
        let index = Index::create_in_ram(build_schema());
        index.tokenizers().register(TOKENIZER, analyzer());
        let fields = fields_from(&index.schema()).expect("schema 应当含有全部字段");

        let mut writer = index
            .writer::<TantivyDocument>(SMALL_HEAP)
            .expect("创建 writer");
        for (name, body) in docs {
            let mut doc = TantivyDocument::new();
            doc.add_text(fields.path, name);
            doc.add_text(fields.rel, name);
            doc.add_text(fields.name, name);
            doc.add_text(fields.body, body);
            doc.add_i64(fields.mtime, 0);
            doc.add_text(fields.ext, "md");
            writer.add_document(doc).expect("写入文档");
        }
        writer.commit().expect("提交");

        (index, fields)
    }

    /// 走一遍真实查询路径，只把命中的文件名按排名列出来。
    ///
    /// 读的是 `rel` 而不是 `name`：`name` 只建索引不落库（搜索结果里的文件名
    /// 是从 `path` 现算的），`get_first(name)` 永远是空。
    fn hits(index: &Index, fields: &Fields, raw: &str) -> Vec<String> {
        let (query, _) = build_query(index, fields, raw).expect("构造查询");
        let reader = index.reader().expect("创建 reader");
        let searcher = reader.searcher();
        searcher
            .search(&query, &TopDocs::with_limit(10).order_by_score())
            .expect("检索")
            .into_iter()
            .map(|(_, addr)| {
                let doc: TantivyDocument = searcher.doc(addr).expect("读取文档");
                doc.get_first(fields.rel)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn scattered_words_match_the_same_document() {
        // 这正是 grep 做不到的事：「增强」和「搜索」在原文里并不相邻
        let (index, fields) = index_with(&[
            ("plan.md", "这次要增强的是文件搜索能力"),
            ("diary.md", "今天天气不错，出门走了走"),
        ]);
        assert_eq!(hits(&index, &fields, "增强 搜索"), vec!["plan.md"]);
    }

    #[test]
    fn multiple_words_are_required_together() {
        // 默认取交集：只提到其中一个词的文件不该出现
        let (index, fields) = index_with(&[
            ("both.md", "既讲配置也讲加载"),
            ("one.md", "只讲配置"),
        ]);
        assert_eq!(hits(&index, &fields, "配置 加载"), vec!["both.md"]);
    }

    #[test]
    fn partial_last_word_still_matches() {
        // 敲到「搜索增」时「增」还不是个词，靠前缀放宽兜住，否则打字到一半会突然清空
        let (index, fields) = index_with(&[("plan.md", "搜索增强方案")]);
        assert_eq!(hits(&index, &fields, "搜索 增"), vec!["plan.md"]);
    }

    #[test]
    fn single_ascii_letter_is_not_widened_but_two_are() {
        // 「a.*」几乎等于把整个词典扫一遍，代价最高收益最低，所以单个 ASCII 字符不放宽
        let (index, fields) = index_with(&[("alpha.md", "alpha beta")]);
        assert!(hits(&index, &fields, "a").is_empty());
        assert_eq!(hits(&index, &fields, "al"), vec!["alpha.md"]);
    }

    #[test]
    fn file_name_hit_outranks_body_hit() {
        // 文件叫这个名字，比正文里顺口提过一句更像用户要找的
        let (index, fields) = index_with(&[
            ("index.md", "这里提到了笔记两个字"),
            ("笔记.md", "随手记的一些东西"),
        ]);
        assert_eq!(hits(&index, &fields, "笔记")[0], "笔记.md");
    }

    #[test]
    fn expression_syntax_reaches_the_query_parser() {
        // 用户明确写了 `-排除`，就该按排除处理，而不是当成一个普通词
        let (index, fields) = index_with(&[
            ("keep.md", "配置说明"),
            ("skip.md", "配置说明 草稿"),
        ]);
        assert_eq!(hits(&index, &fields, "配置 -草稿"), vec!["keep.md"]);
    }

    #[test]
    fn broken_expression_falls_back_instead_of_erroring() {
        // 字段名不存在，QueryParser 会报错。搜索框不该因此变成一片红，
        // 退回分词匹配至少还能给出结果——代价是 `nosuchfield` 这时候
        // 变成了一个必须命中的普通词
        let (index, fields) = index_with(&[("note.md", "nosuchfield 配置说明")]);
        assert_eq!(hits(&index, &fields, "nosuchfield:配置"), vec!["note.md"]);
    }

    #[test]
    fn query_terms_drops_punctuation_and_duplicates() {
        let terms = query_terms("配置, 配置 加载!");
        assert_eq!(terms, vec!["配置".to_string(), "加载".to_string()]);
    }

    #[test]
    fn query_terms_lowercases_like_the_index_does() {
        // 索引侧挂了 LowerCaser，查询侧必须同样小写化，否则大写字母永远搜不到
        assert_eq!(query_terms("README"), vec!["readme".to_string()]);
    }

    #[test]
    fn escape_regex_keeps_metacharacters_literal() {
        assert_eq!(escape_regex("a.b*c"), r"a\.b\*c");
        // 非 ASCII 不能加反斜杠——正则不允许转义汉字
        assert_eq!(escape_regex("配置"), "配置");
    }

    #[test]
    fn expression_detection_ignores_ordinary_hyphens() {
        // `utf-8` 里的连字符是词的一部分，不是排除操作符
        assert!(!looks_like_expression("utf-8 编码"));
        assert!(looks_like_expression("配置 -草稿"));
        assert!(looks_like_expression("title:配置"));
        assert!(looks_like_expression("\"完整短语\""));
    }

    #[test]
    fn filters_match_extension_case_insensitively() {
        let f = Filters::new(Some(vec![".MD".into(), "txt".into()]), None, None);
        assert!(f.accepts("md", Some(0)));
        assert!(f.accepts("txt", Some(0)));
        assert!(!f.accepts("rs", Some(0)));
        assert!(!f.is_noop());
    }

    #[test]
    fn filters_bound_modified_time_on_both_ends() {
        let f = Filters::new(None, Some(100), Some(200));
        assert!(f.accepts("md", Some(150)));
        assert!(f.accepts("md", Some(100)));
        assert!(f.accepts("md", Some(200)));
        assert!(!f.accepts("md", Some(99)));
        assert!(!f.accepts("md", Some(201)));
    }

    #[test]
    fn filters_reject_files_without_mtime_only_when_time_matters() {
        // 索引里没记下时间的文件，一旦按时间筛就只能放弃——
        // 宁可漏掉，也别把可能不满足条件的塞进结果
        assert!(Filters::new(None, None, None).accepts("md", None));
        assert!(!Filters::new(None, Some(1), None).accepts("md", None));
    }

    #[test]
    fn text_extension_whitelist_covers_notes_and_code_but_not_binaries() {
        assert!(is_text_ext(Path::new("a/b/note.md")));
        assert!(is_text_ext(Path::new("main.rs")));
        assert!(is_text_ext(Path::new("A/B/UPPER.TXT")));
        assert!(!is_text_ext(Path::new("photo.png")));
        assert!(!is_text_ext(Path::new("archive.zip")));
        // 没有扩展名的文件一律不进索引：可执行文件和锁文件多半长这样
        assert!(!is_text_ext(Path::new("LICENSE")));
    }
}
