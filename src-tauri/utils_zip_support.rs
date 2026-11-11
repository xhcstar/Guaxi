#![allow(dead_code)]

// Shared build/install bundle support for the large `src-tauri/utils` runtime.
//
// Design goals:
// - Debug/dev mode must be able to run directly from the source-tree utils dir.
// - Release builds must avoid embedding multi-GB resources directly in NSIS.
// - Bundle creation and extraction must be deterministic, recoverable and fast.
// - Unchanged payloads must never be recompressed or rewritten: builds reuse a
//   bundle when its source fingerprint matches, and installs skip bundles/files
//   already present with the same signature.
// - The on-disk structure after extraction must exactly match the original
//   `utils/...` layout so existing runtime commands can keep using
//   `ensure_relative_dir("utils/...")` without special cases.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tar::{Archive, Builder, EntryType, Header};
use zstd::stream::{Decoder, Encoder};

const ZSTD_COMPRESSION_LEVEL: i32 = 3;
const ZSTD_ENCODER_WRITE_BUFFER_SIZE: usize = 8 * 1024 * 1024;
const GENERATED_BUNDLES_DIR: &str = "__bundles";
const BUNDLE_EXTENSION: &str = ".tar.zst";
const BUNDLE_MANIFEST_FILE: &str = "bundle-manifest.json";
const BUNDLE_FORMAT_VERSION: u32 = 2;
const STATE_FILE_SCHEMA: u32 = 1;

// Directories above this size are split into their child entries so one changed
// model does not force a multi-GB recompression/rewrite. The child cap keeps
// node_modules-style trees as a single chunk instead of exploding into
// thousands of tiny bundles.
const SPLIT_THRESHOLD_BYTES: u64 = 256 * 1024 * 1024;
const MAX_SPLIT_CHILDREN: usize = 64;
const MAX_SPLIT_DEPTH: usize = 6;

// Filesystem timestamps round-trip exactly on NTFS; the tolerance only absorbs
// minor timestamp rounding on other filesystems.
const MTIME_TOLERANCE_NS: u64 = 2_000_000;

const LEGACY_BUNDLES: &[&str] = &[
    "utils.zip",
    "SlidevEnv.zip",
    "llama.zip",
    "python_libs.zip",
    "Models/tts.zip",
    "Models/whisper_models.zip",
];

const SKIPPED_DIRECTORIES: &[&str] = &[
    "MarkdownConvertor",
    "Models/surya_models",
    // MinerU 强引擎（可选组件，含大体积 venv 与模型），与 MarkdownConvertor 一致处理：
    // 不进入 tar.zst 自动打包，由独立机制随安装目录分发；缺失时运行时自动回退 marker。
    "MinerU",
    "Models/mineru_models",
    // 语音已经改成进程内调用 sherpa-onnx 库，不再需要这些外部可执行文件与随附运行库。
    "sherpa-onnx",
    "whisper",
    GENERATED_BUNDLES_DIR,
];

// ---------------------------------------------------------------------------
// 构建期进度输出
//
// 打包 utils / CUDA 运行库要扫描十几万文件、把数 GB 数据压成 tar.zst，纯计算阶段
// 一句日志都没有，很容易被当成构建卡死。这里提供统一的进度行，并按运行环境选择
// 输出去向：`cargo run --bin ...` 直接写 stdout；build.rs 里 stdout 会被 cargo 丢掉，
// 只有 `cargo:warning=` 能显示出来。
// ---------------------------------------------------------------------------

/// 进度行的去向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressMode {
    /// 按运行环境自动判断（默认）。
    Auto,
    /// 直接写 stdout。
    Stdout,
    /// 写成 `cargo:warning=`，供 build.rs 使用。
    CargoWarning,
    /// 完全静默。
    Silent,
}

static PROGRESS_MODE: AtomicU8 = AtomicU8::new(0);

/// 覆盖进度输出方式。build.rs 在调用打包函数前会显式设为 `CargoWarning`。
pub fn set_progress_mode(mode: ProgressMode) {
    let value = match mode {
        ProgressMode::Auto => 0,
        ProgressMode::Stdout => 1,
        ProgressMode::CargoWarning => 2,
        ProgressMode::Silent => 3,
    };
    PROGRESS_MODE.store(value, Ordering::Relaxed);
}

fn progress_mode() -> ProgressMode {
    match PROGRESS_MODE.load(Ordering::Relaxed) {
        1 => return ProgressMode::Stdout,
        2 => return ProgressMode::CargoWarning,
        3 => return ProgressMode::Silent,
        _ => {}
    }

    // `GUAXI_BUNDLE_LOG=quiet` 用于收敛日志（例如 CI 里只想看摘要）。
    let quiet = std::env::var("GUAXI_BUNDLE_LOG")
        .map(|value| value.eq_ignore_ascii_case("quiet"))
        .unwrap_or(false);
    if quiet {
        return ProgressMode::Silent;
    }

    // 不能靠 OUT_DIR 判断「是否在 build 脚本里」：`cargo run` 启动的普通进程同样
    // 带着 OUT_DIR。build.rs 会显式调用 set_progress_mode(CargoWarning)，这里只管默认。
    ProgressMode::Stdout
}

/// 输出一行进度。调用方负责保证单行（`cargo:warning=` 不认多行）。
fn progress_emit(message: &str) {
    match progress_mode() {
        // Auto 只作为「未设置」的取值存在，progress_mode() 不会把它返回。
        ProgressMode::Auto | ProgressMode::Stdout => println!("{message}"),
        ProgressMode::CargoWarning => println!("cargo:warning={message}"),
        ProgressMode::Silent => {}
    }
}

fn format_bytes(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = MB * 1024.0;
    let value = bytes as f64;
    if value >= GB {
        format!("{:.2} GB", value / GB)
    } else if value >= MB {
        format!("{:.0} MB", value / MB)
    } else {
        format!("{:.0} KB", value / 1024.0)
    }
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs_f64();
    if total_seconds < 10.0 {
        format!("{total_seconds:.1}s")
    } else if total_seconds < 60.0 {
        format!("{total_seconds:.0}s")
    } else {
        let minutes = (total_seconds / 60.0).floor();
        let seconds = (total_seconds % 60.0).round();
        format!("{minutes:.0}m{seconds:02.0}s")
    }
}

/// 一次打包任务的进度。多线程压缩时由各 worker 原子累加，后台线程定期打印一行，
/// 让终端不至于长时间空白。
struct BundleProgress {
    started: Instant,
    total_files: usize,
    total_bytes: u64,
    done_files: AtomicUsize,
    done_bytes: AtomicU64,
    stop: AtomicBool,
    ticker_enabled: bool,
}

impl BundleProgress {
    fn new(total_files: usize, total_bytes: u64) -> Arc<Self> {
        Arc::new(Self {
            started: Instant::now(),
            total_files,
            total_bytes,
            done_files: AtomicUsize::new(0),
            done_bytes: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            // 小体量分包几秒就完事，为它开线程只是噪音。
            ticker_enabled: progress_mode() != ProgressMode::Silent
                && total_bytes >= 64 * 1024 * 1024,
        })
    }

    fn advance(&self, bytes: u64) {
        self.done_files.fetch_add(1, Ordering::Relaxed);
        self.done_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    /// 边读边记：单个文件可能几百 MB，必须按字节推进，否则进度会长时间停在原地。
    fn add_bytes(&self, bytes: u64) {
        self.done_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    fn complete_entry(&self) {
        self.done_files.fetch_add(1, Ordering::Relaxed);
    }

    fn spawn_ticker(self: &Arc<Self>) -> Option<thread::JoinHandle<()>> {
        if !self.ticker_enabled {
            return None;
        }
        // build.rs 的每行都会变成一条 warning，间隔放大一点免得刷屏。
        let interval = match progress_mode() {
            ProgressMode::CargoWarning => Duration::from_secs(20),
            _ => Duration::from_secs(5),
        };
        let this = Arc::clone(self);
        Some(thread::spawn(move || {
            let mut waited = Duration::ZERO;
            while !this.stop.load(Ordering::Relaxed) {
                if waited >= interval {
                    this.emit_progress();
                    waited = Duration::ZERO;
                }
                thread::sleep(Duration::from_millis(250));
                waited += Duration::from_millis(250);
            }
        }))
    }

    fn emit_progress(&self) {
        let done_bytes = self.done_bytes.load(Ordering::Relaxed);
        let elapsed = self.started.elapsed();
        let elapsed_seconds = elapsed.as_secs_f64();
        let percent = if self.total_bytes == 0 {
            0.0
        } else {
            done_bytes as f64 / self.total_bytes as f64 * 100.0
        };
        let rate = if elapsed_seconds > 0.5 {
            done_bytes as f64 / elapsed_seconds
        } else {
            0.0
        };
        let remaining = if rate > 1.0 && self.total_bytes > done_bytes {
            format_duration(Duration::from_secs_f64(
                (self.total_bytes - done_bytes) as f64 / rate,
            ))
        } else {
            "?".to_string()
        };
        progress_emit(&format!(
            "[utils.bundle] 压缩中 {percent:.0}% · {}/{} 文件 · {} / {} · {} / 已用 {} · 预计还需 {}",
            self.done_files.load(Ordering::Relaxed),
            self.total_files,
            format_bytes(done_bytes),
            format_bytes(self.total_bytes),
            if rate > 1.0 {
                format!("{} /s", format_bytes(rate as u64))
            } else {
                "—".to_string()
            },
            format_duration(elapsed),
            remaining
        ));
    }

    fn finish(&self, bundle_count: usize, archived_bytes: u64) {
        self.stop.store(true, Ordering::Relaxed);
        progress_emit(&format!(
            "[utils.bundle] 压缩完成 · {} 个分包 · {} 原始 → {} 归档 · 用时 {}",
            bundle_count,
            format_bytes(self.total_bytes),
            format_bytes(archived_bytes),
            format_duration(self.started.elapsed())
        ));
    }
}

/// 扫描阶段的轻量计数器：目录树遍历是单线程的，只需要节流打印。
struct ScanProgress {
    started: Instant,
    entries: AtomicUsize,
    last_emit: Mutex<Instant>,
}

impl ScanProgress {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            entries: AtomicUsize::new(0),
            last_emit: Mutex::new(Instant::now()),
        }
    }

    fn record(&self, count: usize) {
        let total = self.entries.fetch_add(count, Ordering::Relaxed) + count;
        if progress_mode() == ProgressMode::Silent {
            return;
        }
        let mut last = match self.last_emit.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if last.elapsed() < Duration::from_secs(3) {
            return;
        }
        *last = Instant::now();
        drop(last);
        progress_emit(&format!(
            "[utils.bundle] 扫描中 · 已遍历 {total} 个条目 · 用时 {}",
            format_duration(self.started.elapsed())
        ));
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BundleStrategy {
    RustTarZstd,
}

impl BundleStrategy {
    pub fn label(self) -> &'static str {
        match self {
            BundleStrategy::RustTarZstd => "TarZstdRustMT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceKind {
    File,
    Dir,
}

impl SourceKind {
    fn as_str(self) -> &'static str {
        match self {
            SourceKind::File => "file",
            SourceKind::Dir => "dir",
        }
    }

    fn from_str(value: &str) -> Self {
        if value == "file" {
            SourceKind::File
        } else {
            SourceKind::Dir
        }
    }
}

#[derive(Debug, Clone)]
struct BundleEntrySpec {
    relative: PathBuf,
    source: PathBuf,
    kind: BundleEntryKind,
}

#[derive(Debug, Clone, Copy)]
enum BundleEntryKind {
    File { size: u64, mtime_ns: u64 },
    EmptyDir,
}

#[derive(Debug, Clone, Copy, Default)]
struct EntryStats {
    file_count: usize,
    empty_dir_count: usize,
    total_bytes: u64,
}

#[derive(Debug, Clone)]
struct BundleUnit {
    relative: PathBuf,
    source_kind: SourceKind,
    entries: Vec<BundleEntrySpec>,
    stats: EntryStats,
}

#[derive(Debug, Clone)]
struct BundleSpec {
    bundle_path: PathBuf,
    bundle_name: String,
    source_relative: PathBuf,
    source_kind: SourceKind,
    entries: Vec<BundleEntrySpec>,
    stats: EntryStats,
    digest: String,
}

#[derive(Debug, Clone)]
struct PreparedMeta {
    archive_bytes: u64,
    archive_blake3: String,
    rebuilt: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BundleFileEntry {
    path: String,
    size: u64,
    #[serde(default)]
    mtime_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BundleManifestEntry {
    name: String,
    source: String,
    #[serde(default = "default_source_kind")]
    source_kind: String,
    digest: String,
    archive_bytes: u64,
    #[serde(default)]
    archive_blake3: String,
    #[serde(default)]
    files: Vec<BundleFileEntry>,
    #[serde(default)]
    empty_dirs: Vec<String>,
}

fn default_source_kind() -> String {
    SourceKind::Dir.as_str().to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BundleManifest {
    schema: u32,
    #[serde(default)]
    bundles: Vec<BundleManifestEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct BundleState {
    #[serde(default = "default_state_schema")]
    schema: u32,
    #[serde(default)]
    bundles: BTreeMap<String, String>,
}

fn default_state_schema() -> u32 {
    STATE_FILE_SCHEMA
}

#[derive(Debug, Clone)]
pub struct PrepareResult {
    pub bundle_dir: PathBuf,
    pub strategy: BundleStrategy,
    pub reused_existing: bool,
    pub elapsed_ms: u128,
    pub bundle_count: usize,
    pub rebuilt_bundle_count: usize,
    pub reused_bundle_count: usize,
    pub file_count: usize,
    pub empty_dir_count: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ExtractResult {
    pub bundle_count: usize,
    pub extracted_bundle_count: usize,
    pub skipped_bundle_count: usize,
    pub written_file_count: usize,
    pub skipped_file_count: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct ExtractBundleStats {
    written_files: usize,
    skipped_files: usize,
}

// Skip legacy generated artifacts, temporary extraction directories and resource
// trees that are intentionally excluded from release payloads.
pub fn should_skip(relative: &str) -> bool {
    // 一次扫描要调用近 50 万次，这里刻意避免逐项分配：
    // 只有含反斜杠的路径才复制，目录前缀用 strip_prefix 判断而不是 format!。
    let normalized = if relative.contains('\\') {
        std::borrow::Cow::Owned(relative.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(relative)
    };
    let normalized: &str = normalized.as_ref();
    let bytes = normalized.as_bytes();
    LEGACY_BUNDLES.contains(&normalized)
        || SKIPPED_DIRECTORIES.iter().any(|dir| {
            normalized == *dir
                || normalized
                    .strip_prefix(dir)
                    .map(|rest| rest.starts_with('/'))
                    .unwrap_or(false)
        })
        // 语音模型与原生库的下载归档只是中间产物，解压完就没用了。
        // 一旦误留在 utils 下，就会被整包打进 release 白增几百 MB。
        || (bytes.len() >= 8 && bytes[bytes.len() - 8..].eq_ignore_ascii_case(b".tar.bz2"))
        || normalized.contains(".__extracting_")
        || normalized.ends_with("/.guaxi_extract_complete")
        || normalized.ends_with("/.guaxi_utils_extract_complete")
        || normalized == ".guaxi_extract_complete"
        || normalized == ".guaxi_utils_extract_complete"
}

pub fn prepare_utils_bundles(
    manifest_dir: &Path,
    reuse_existing: bool,
) -> Result<PrepareResult, String> {
    let utils_dir = manifest_dir.join("utils");
    if !utils_dir.is_dir() {
        return Err(format!("utils dir not found: {}", utils_dir.display()));
    }
    prepare_bundles_in_dir(
        &utils_dir,
        &utils_dir.join(GENERATED_BUNDLES_DIR),
        reuse_existing,
        None,
    )
}

/// Bundles an arbitrary payload directory with the same archive layout the
/// installer's extraction helper understands. Used for payloads that must land
/// in the install root but cannot fit inside the NSIS installer itself, such as
/// the CUDA runtime DLLs. `include_extension` keeps staging files (import libs,
/// .exp) out of the payload when only one file type should ship.
pub fn prepare_payload_bundles(
    source_dir: &Path,
    bundle_dir: &Path,
    reuse_existing: bool,
    include_extension: Option<&str>,
) -> Result<PrepareResult, String> {
    if !source_dir.is_dir() {
        return Err(format!(
            "payload source dir not found: {}",
            source_dir.display()
        ));
    }
    prepare_bundles_in_dir(source_dir, bundle_dir, reuse_existing, include_extension)
}

fn prepare_bundles_in_dir(
    source_dir: &Path,
    bundle_dir: &Path,
    reuse_existing: bool,
    include_extension: Option<&str>,
) -> Result<PrepareResult, String> {
    let started_at = Instant::now();
    fs::create_dir_all(bundle_dir)
        .map_err(|e| format!("failed to create {}: {e}", bundle_dir.display()))?;
    cleanup_temp_files(bundle_dir)?;

    let previous = match read_manifest(bundle_dir) {
        Ok(manifest) => manifest.filter(|manifest| manifest.schema == BUNDLE_FORMAT_VERSION),
        Err(error) => {
            eprintln!("[utils.bundle] ignoring unreadable manifest: {error}");
            None
        }
    };
    let previous_manifest_present = previous.is_some();

    // Explicit trust mode: the wrapper just prepared these bundles, so a valid
    // manifest with all archives present means there is nothing left to scan.
    if reuse_existing {
        if let Some(manifest) = &previous {
            if manifest_is_complete(bundle_dir, manifest)? {
                prune_stale_bundles(
                    bundle_dir,
                    manifest.bundles.iter().map(|bundle| bundle.name.clone()),
                )?;
                progress_emit(&format!(
                    "[utils.bundle] 复用现有清单 · {} 个分包均已就绪（无需重扫）· {}",
                    manifest.bundles.len(),
                    format_duration(started_at.elapsed())
                ));
                return Ok(PrepareResult {
                    bundle_dir: bundle_dir.to_path_buf(),
                    strategy: BundleStrategy::RustTarZstd,
                    reused_existing: true,
                    elapsed_ms: started_at.elapsed().as_millis(),
                    bundle_count: manifest.bundles.len(),
                    rebuilt_bundle_count: 0,
                    reused_bundle_count: manifest.bundles.len(),
                    file_count: 0,
                    empty_dir_count: 0,
                    total_bytes: 0,
                });
            }
        }
    }

    progress_emit(&format!(
        "[utils.bundle] 开始扫描 · source={} · bundleDir={}{}",
        source_dir.display(),
        bundle_dir.display(),
        if reuse_existing {
            " · 复用清单不可用，退回完整扫描"
        } else {
            ""
        }
    ));

    let scan_started = Instant::now();
    let specs = collect_bundle_specs(source_dir, bundle_dir, include_extension)?;
    let total_files: usize = specs
        .iter()
        .map(|spec| spec.stats.file_count + spec.stats.empty_dir_count)
        .sum();
    let total_bytes: u64 = specs.iter().map(|spec| spec.stats.total_bytes).sum();
    progress_emit(&format!(
        "[utils.bundle] 扫描完成 · {} 个分包 · 去重后 {} 个条目 · {} · 用时 {}",
        specs.len(),
        total_files,
        format_bytes(total_bytes),
        format_duration(scan_started.elapsed())
    ));

    // Drop bundles whose names are no longer part of the payload before
    // rebuilding. During the first split migration this frees the old
    // multi-GB top-level archives instead of keeping both generations at once.
    prune_stale_bundles(
        bundle_dir,
        specs.iter().map(|spec| spec.bundle_name.clone()),
    )?;

    let previous_bundles: HashMap<String, BundleManifestEntry> = previous
        .map(|manifest| {
            manifest
                .bundles
                .into_iter()
                .map(|bundle| (bundle.name.clone(), bundle))
                .collect()
        })
        .unwrap_or_default();

    let mut prepared: Vec<Option<PreparedMeta>> = vec![None; specs.len()];
    let mut rebuild_indices = Vec::new();

    for (index, spec) in specs.iter().enumerate() {
        let reusable = previous_bundles
            .get(&spec.bundle_name)
            .filter(|previous| {
                previous.digest == spec.digest && previous.archive_blake3.len() == 64
            })
            .and_then(|previous| {
                fs::metadata(&spec.bundle_path)
                    .ok()
                    .filter(|metadata| metadata.is_file() && metadata.len() == previous.archive_bytes)
                    .map(|_| previous)
            });

        if let Some(previous) = reusable {
            prepared[index] = Some(PreparedMeta {
                archive_bytes: previous.archive_bytes,
                archive_blake3: previous.archive_blake3.clone(),
                rebuilt: false,
            });
        } else {
            rebuild_indices.push(index);
        }
    }

    if !rebuild_indices.is_empty() {
        let cores = std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(1);
        let encoder_threads = (cores / rebuild_indices.len())
            .clamp(1, cores) as u32;
        let rebuild_bytes: u64 = rebuild_indices
            .iter()
            .map(|index| specs[*index].stats.total_bytes)
            .sum();
        progress_emit(&format!(
            "[utils.bundle] 开始压缩 · 复用 {} 个（摘要未变）· 重压 {} 个（{}）· 每包 {} 线程 / 共 {} 核",
            specs.len() - rebuild_indices.len(),
            rebuild_indices.len(),
            format_bytes(rebuild_bytes),
            encoder_threads,
            cores
        ));

        let progress = BundleProgress::new(
            rebuild_indices
                .iter()
                .map(|index| specs[*index].stats.file_count + specs[*index].stats.empty_dir_count)
                .sum(),
            rebuild_bytes,
        );
        let ticker = progress.spawn_ticker();
        let results: Vec<Result<PreparedMeta, String>> = rebuild_indices
            .par_iter()
            .map(|&index| prepare_single_bundle(&specs[index], encoder_threads, &progress))
            .collect();
        progress.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = ticker {
            let _ = handle.join();
        }
        for (index, result) in rebuild_indices.iter().zip(results) {
            prepared[*index] = Some(result?);
        }
        let archived_bytes = prepared
            .iter()
            .flatten()
            .filter(|meta| meta.rebuilt)
            .map(|meta| meta.archive_bytes)
            .sum();
        progress.finish(rebuild_indices.len(), archived_bytes);
    } else {
        progress_emit(&format!(
            "[utils.bundle] 全部 {} 个分包均可复用（源文件未变），本轮不压缩",
            specs.len()
        ));
    }

    // If every spec reused its previous archive and the manifest has no extra
    // entries, the existing manifest already describes this exact payload.
    // Skip building/serializing the multi-MB file list on no-change builds.
    let fully_reused = previous_manifest_present
        && rebuild_indices.is_empty()
        && specs.len() == previous_bundles.len()
        && specs.iter().all(|spec| {
            previous_bundles
                .get(&spec.bundle_name)
                .map(|previous| previous.digest == spec.digest)
                .unwrap_or(false)
        });

    let mut manifest_bundles = Vec::new();
    let mut rebuilt_bundle_count = 0;
    let mut reused_bundle_count = 0;
    let mut aggregate_stats = EntryStats::default();

    for (spec, meta) in specs.iter().zip(prepared.into_iter()) {
        let meta = meta.ok_or_else(|| {
            format!(
                "bundle {} was neither reused nor rebuilt",
                spec.bundle_name
            )
        })?;
        if meta.rebuilt {
            rebuilt_bundle_count += 1;
        } else {
            reused_bundle_count += 1;
        }
        aggregate_stats.file_count += spec.stats.file_count;
        aggregate_stats.empty_dir_count += spec.stats.empty_dir_count;
        aggregate_stats.total_bytes += spec.stats.total_bytes;
        if !fully_reused {
            manifest_bundles.push(manifest_entry_for(spec, &meta));
        }
    }

    prune_stale_bundles(
        bundle_dir,
        specs.iter().map(|spec| spec.bundle_name.clone()),
    )?;
    if !fully_reused {
        write_manifest_if_changed(
            bundle_dir,
            &BundleManifest {
                schema: BUNDLE_FORMAT_VERSION,
                bundles: manifest_bundles,
            },
        )?;
    }

    Ok(PrepareResult {
        bundle_dir: bundle_dir.to_path_buf(),
        strategy: BundleStrategy::RustTarZstd,
        reused_existing: rebuilt_bundle_count == 0,
        elapsed_ms: started_at.elapsed().as_millis(),
        bundle_count: specs.len(),
        rebuilt_bundle_count,
        reused_bundle_count,
        file_count: aggregate_stats.file_count,
        empty_dir_count: aggregate_stats.empty_dir_count,
        total_bytes: aggregate_stats.total_bytes,
    })
}

fn manifest_is_complete(bundle_dir: &Path, manifest: &BundleManifest) -> Result<bool, String> {
    if manifest.schema != BUNDLE_FORMAT_VERSION || manifest.bundles.is_empty() {
        return Ok(false);
    }
    let mut names = HashSet::new();
    for bundle in &manifest.bundles {
        if !names.insert(bundle.name.as_str()) || bundle.archive_blake3.len() != 64 {
            return Ok(false);
        }
        let path = bundle_dir.join(&bundle.name);
        match fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() && metadata.len() == bundle.archive_bytes => {}
            _ => return Ok(false),
        }
    }
    Ok(true)
}

fn cleanup_temp_files(bundle_dir: &Path) -> Result<(), String> {
    let entries = fs::read_dir(bundle_dir)
        .map_err(|e| format!("failed to read dir {}: {e}", bundle_dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(OsStr::to_str) else {
            continue;
        };
        if path.is_file() && (name.ends_with(".tmp") || name.contains(".guaxi-tmp-")) {
            fs::remove_file(&path)
                .map_err(|e| format!("failed to remove temp {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

fn prune_stale_bundles(
    bundle_dir: &Path,
    expected: impl IntoIterator<Item = String>,
) -> Result<(), String> {
    let expected: HashSet<String> = expected.into_iter().collect();
    let entries = fs::read_dir(bundle_dir)
        .map_err(|e| format!("failed to read dir {}: {e}", bundle_dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(OsStr::to_str) else {
            continue;
        };
        let stale = is_bundle_file(&path) && !expected.contains(name);
        if stale {
            fs::remove_file(&path)
                .map_err(|e| format!("failed to remove stale bundle {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

fn collect_bundle_specs(
    source_dir: &Path,
    bundle_dir: &Path,
    include_extension: Option<&str>,
) -> Result<Vec<BundleSpec>, String> {
    let mut entries = fs::read_dir(source_dir)
        .map_err(|e| format!("failed to read dir {}: {e}", source_dir.display()))?
        .flatten()
        .collect::<Vec<_>>();
    entries.sort_by_cached_key(|entry| entry.file_name());

    let mut top_entries = Vec::new();
    for entry in entries {
        let path = entry.path();

        // Only filter top-level files: payload directories keep their whole tree.
        if path.is_file() {
            if let Some(extension) = include_extension {
                let matches = path
                    .extension()
                    .and_then(OsStr::to_str)
                    .map(|value| value.eq_ignore_ascii_case(extension))
                    .unwrap_or(false);
                if !matches {
                    continue;
                }
            }
        }

        let relative = path
            .strip_prefix(source_dir)
            .map_err(|e| {
                format!(
                    "failed to strip prefix {} from {}: {e}",
                    source_dir.display(),
                    path.display()
                )
            })?
            .to_path_buf();

        if should_skip(&normalize_path(&relative)) {
            continue;
        }

        top_entries.push((path, relative));
    }

    // Metadata traversal stays sequential: on Windows the parallel version
    // thrashed the disk/antivirus and was slower than this single pass.
    // 十几万文件可能扫十几秒，带上计数器让终端每隔几秒有一行输出。
    let scan = ScanProgress::new();
    let mut units = Vec::new();
    for (path, relative) in top_entries {
        collect_units(&path, &relative, 0, &mut units, &scan)?;
    }

    let mut specs = Vec::with_capacity(units.len());
    let mut names = HashSet::new();
    for unit in units {
        let bundle_name = bundle_name_for(&unit.relative);
        if !names.insert(bundle_name.clone()) {
            return Err(format!(
                "bundle name collision for {} ({bundle_name})",
                unit.relative.display()
            ));
        }
        let digest = digest_for_unit(&unit);
        specs.push(BundleSpec {
            bundle_path: bundle_dir.join(&bundle_name),
            bundle_name,
            source_relative: unit.relative,
            source_kind: unit.source_kind,
            entries: unit.entries,
            stats: unit.stats,
            digest,
        });
    }
    Ok(specs)
}

fn collect_units(
    path: &Path,
    relative: &Path,
    depth: usize,
    units: &mut Vec<BundleUnit>,
    scan: &ScanProgress,
) -> Result<(), String> {
    if path.is_file() {
        let entry = file_entry(relative, path)?;
        scan.record(1);
        units.push(BundleUnit {
            relative: relative.to_path_buf(),
            source_kind: SourceKind::File,
            stats: summarize_entries(std::slice::from_ref(&entry)),
            entries: vec![entry],
        });
        return Ok(());
    }

    let entries = collect_entries(relative, path, scan)?;
    if entries.is_empty() {
        let empty = BundleEntrySpec {
            relative: relative.to_path_buf(),
            source: path.to_path_buf(),
            kind: BundleEntryKind::EmptyDir,
        };
        units.push(BundleUnit {
            relative: relative.to_path_buf(),
            source_kind: SourceKind::Dir,
            stats: summarize_entries(std::slice::from_ref(&empty)),
            entries: vec![empty],
        });
        return Ok(());
    }

    let stats = summarize_entries(&entries);
    let children = included_children(path, relative)?;
    // A single-child wrapper such as Models/asr/sense-voice must still be
    // descended into; otherwise one 1GB model bundle hides several independent
    // model files and every update recompresses all of them.
    let should_split = depth < MAX_SPLIT_DEPTH
        && stats.total_bytes > SPLIT_THRESHOLD_BYTES
        && !children.is_empty()
        && children.len() <= MAX_SPLIT_CHILDREN;

    if should_split {
        for (child_path, child_relative) in children {
            collect_units(&child_path, &child_relative, depth + 1, units, scan)?;
        }
        return Ok(());
    }

    units.push(BundleUnit {
        relative: relative.to_path_buf(),
        source_kind: SourceKind::Dir,
        entries,
        stats,
    });
    Ok(())
}

fn included_children(
    dir: &Path,
    relative: &Path,
) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let mut entries = fs::read_dir(dir)
        .map_err(|e| format!("failed to read dir {}: {e}", dir.display()))?
        .flatten()
        .collect::<Vec<_>>();
    entries.sort_by_cached_key(|entry| entry.file_name());

    let mut children = Vec::new();
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let child_relative = relative.join(&name);
        if should_skip(&normalize_path(&child_relative)) {
            continue;
        }
        children.push((path, child_relative));
    }
    Ok(children)
}

fn file_entry(relative: &Path, path: &Path) -> Result<BundleEntrySpec, String> {
    let metadata = fs::metadata(path)
        .map_err(|e| format!("failed to read metadata {}: {e}", path.display()))?;
    Ok(entry_spec(relative, path, &metadata))
}

fn entry_spec(relative: &Path, source: &Path, metadata: &fs::Metadata) -> BundleEntrySpec {
    BundleEntrySpec {
        relative: relative.to_path_buf(),
        source: source.to_path_buf(),
        kind: BundleEntryKind::File {
            size: metadata.len(),
            mtime_ns: metadata_mtime_ns(metadata),
        },
    }
}

fn collect_entries(
    root_relative: &Path,
    current_dir: &Path,
    scan: &ScanProgress,
) -> Result<Vec<BundleEntrySpec>, String> {
    let mut entries = fs::read_dir(current_dir)
        .map_err(|e| format!("failed to read dir {}: {e}", current_dir.display()))?
        .flatten()
        .collect::<Vec<_>>();
    entries.sort_by_cached_key(|entry| entry.file_name());

    let mut collected = Vec::new();
    for entry in entries {
        let path = entry.path();
        let relative = root_relative.join(entry.file_name());
        if should_skip(&normalize_path(&relative)) {
            continue;
        }

        // 普通文件的类型和元数据直接取自目录枚举结果，不必再逐项 stat；
        // 符号链接 / 目录联接必须回退到 fs::metadata：链接自身的 size=0、
        // mtime 也是链接的，拿它做清单会和安装后按目标内容写出的文件对不上。
        let file_type = entry
            .file_type()
            .map_err(|e| format!("failed to stat dir entry {}: {e}", path.display()))?;
        let link_target = if file_type.is_symlink() {
            Some(
                fs::metadata(&path)
                    .map_err(|e| format!("failed to read metadata {}: {e}", path.display()))?,
            )
        } else {
            None
        };
        let is_dir = link_target
            .as_ref()
            .map(|metadata| metadata.is_dir())
            .unwrap_or_else(|| file_type.is_dir());

        if is_dir {
            let nested_entries = collect_entries(&relative, &path, scan)?;
            if nested_entries.is_empty() {
                scan.record(1);
                collected.push(BundleEntrySpec {
                    relative,
                    source: path,
                    kind: BundleEntryKind::EmptyDir,
                });
            } else {
                collected.extend(nested_entries);
            }
            continue;
        }

        let metadata = match link_target {
            Some(metadata) => metadata,
            None => entry
                .metadata()
                .map_err(|e| format!("failed to read metadata {}: {e}", path.display()))?,
        };
        scan.record(1);
        collected.push(entry_spec(&relative, &path, &metadata));
    }

    Ok(collected)
}

fn summarize_entries(entries: &[BundleEntrySpec]) -> EntryStats {
    let mut stats = EntryStats::default();
    for entry in entries {
        match entry.kind {
            BundleEntryKind::File { size, .. } => {
                stats.file_count += 1;
                stats.total_bytes += size;
            }
            BundleEntryKind::EmptyDir => {
                stats.empty_dir_count += 1;
            }
        }
    }
    stats
}

fn digest_for_unit(unit: &BundleUnit) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"guaxi-bundle-source");
    hasher.update(&BUNDLE_FORMAT_VERSION.to_le_bytes());
    hasher.update(normalize_path(&unit.relative).as_bytes());
    hasher.update(unit.source_kind.as_str().as_bytes());
    for entry in &unit.entries {
        hasher.update(b"\0");
        hasher.update(normalize_path(&entry.relative).as_bytes());
        match entry.kind {
            BundleEntryKind::File { size, mtime_ns } => {
                hasher.update(b"F");
                hasher.update(&size.to_le_bytes());
                hasher.update(&mtime_ns.to_le_bytes());
            }
            BundleEntryKind::EmptyDir => {
                hasher.update(b"D");
            }
        }
    }
    hasher.finalize().to_hex().to_string()
}

fn bundle_name_for(relative: &Path) -> String {
    let normalized = normalize_path(relative);
    let mut escaped = String::with_capacity(normalized.len() + 8);
    for ch in normalized.chars() {
        match ch {
            '%' => escaped.push_str("%25"),
            '/' => escaped.push_str("%2F"),
            _ => escaped.push(ch),
        }
    }
    if escaped.len() > 180 {
        let digest = blake3::hash(normalized.as_bytes()).to_hex().to_string();
        return format!("bundle-{}{}", &digest[..16], BUNDLE_EXTENSION);
    }
    format!("{escaped}{BUNDLE_EXTENSION}")
}

fn manifest_entry_for(spec: &BundleSpec, meta: &PreparedMeta) -> BundleManifestEntry {
    let mut files = Vec::new();
    let mut empty_dirs = Vec::new();
    for entry in &spec.entries {
        match entry.kind {
            BundleEntryKind::File { size, mtime_ns } => files.push(BundleFileEntry {
                path: normalize_path(&entry.relative),
                size,
                mtime_ns,
            }),
            BundleEntryKind::EmptyDir => empty_dirs.push(normalize_path(&entry.relative)),
        }
    }
    BundleManifestEntry {
        name: spec.bundle_name.clone(),
        source: normalize_path(&spec.source_relative),
        source_kind: spec.source_kind.as_str().to_string(),
        digest: spec.digest.clone(),
        archive_bytes: meta.archive_bytes,
        archive_blake3: meta.archive_blake3.clone(),
        files,
        empty_dirs,
    }
}

struct HashingWriter<W> {
    inner: W,
    hasher: blake3::Hasher,
}

impl<W: Write> HashingWriter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            hasher: blake3::Hasher::new(),
        }
    }

    fn finish(mut self) -> io::Result<(W, String)> {
        self.inner.flush()?;
        let Self { inner, hasher } = self;
        Ok((inner, hasher.finalize().to_hex().to_string()))
    }
}

impl<W: Write> Write for HashingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.hasher.update(&buf[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// 读取源文件的同时把「已读字节数」记进进度，供压缩阶段的实时百分比与 ETA 使用。
struct ProgressReader<'a, R> {
    inner: R,
    progress: &'a BundleProgress,
}

impl<R: Read> Read for ProgressReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        if read > 0 {
            self.progress.add_bytes(read as u64);
        }
        Ok(read)
    }
}

fn prepare_single_bundle(
    bundle: &BundleSpec,
    encoder_threads: u32,
    progress: &BundleProgress,
) -> Result<PreparedMeta, String> {
    let started_at = Instant::now();
    // Write to a temporary file first so aborted builds never leave a partially
    // valid bundle behind that might later be mistaken for a complete artifact.
    let temp_bundle_path = bundle.bundle_path.with_file_name(format!(
        "{}.tmp",
        bundle
            .bundle_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    ));
    let file = File::create(&temp_bundle_path)
        .map_err(|e| format!("failed to create {}: {e}", temp_bundle_path.display()))?;
    let writer = BufWriter::with_capacity(ZSTD_ENCODER_WRITE_BUFFER_SIZE, file);
    let hashing_writer = HashingWriter::new(writer);
    let mut encoder = Encoder::new(hashing_writer, ZSTD_COMPRESSION_LEVEL)
        .map_err(|e| format!("failed to start zstd encoder {}: {e}", temp_bundle_path.display()))?;
    if encoder_threads > 1 {
        encoder
            .multithread(encoder_threads)
            .map_err(|e| format!("failed to enable zstd multithreading: {e}"))?;
    }

    // tar keeps the original relative paths intact, while zstd gives us much
    // faster compression/decompression than the previous zip-based flow.
    let mut archive = Builder::new(encoder);
    archive.mode(tar::HeaderMode::Deterministic);

    for entry in &bundle.entries {
        match entry.kind {
            BundleEntryKind::EmptyDir => {
                archive
                    .append_dir(&entry.relative, &entry.source)
                    .map_err(|e| {
                        format!(
                            "failed to add dir {} to archive {}: {e}",
                            entry.relative.display(),
                            bundle.bundle_path.display()
                        )
                    })?;
                progress.complete_entry();
            }
            BundleEntryKind::File { .. } => {
                // 这里刻意不用 append_path_with_name：它无法在读取文件时回报进度，
                // 一个 500 MB 的 DLL 会让进度条钉死几十秒。手写 header 后走 append_data，
                // 中间插入 ProgressReader，压缩比为 1:1 地反映真实读取量。
                let stat = fs::metadata(&entry.source).map_err(|e| {
                    format!("failed to read metadata {}: {e}", entry.source.display())
                })?;
                let file = File::open(&entry.source)
                    .map_err(|e| format!("failed to open {}: {e}", entry.source.display()))?;
                let mut header = Header::new_gnu();
                header.set_metadata_in_mode(&stat, tar::HeaderMode::Deterministic);
                let mut reader = ProgressReader {
                    inner: file,
                    progress,
                };
                archive
                    .append_data(&mut header, &entry.relative, &mut reader)
                    .map_err(|e| {
                        format!(
                            "failed to add file {} to archive {}: {e}",
                            entry.source.display(),
                            bundle.bundle_path.display()
                        )
                    })?;
                progress.complete_entry();
            }
        }
    }

    let encoder = archive
        .into_inner()
        .map_err(|e| format!("failed to finalize tar {}: {e}", temp_bundle_path.display()))?;
    let hashing_writer = encoder
        .finish()
        .map_err(|e| format!("failed to finalize zstd {}: {e}", temp_bundle_path.display()))?;
    let (writer, archive_blake3) = hashing_writer
        .finish()
        .map_err(|e| format!("failed to flush {}: {e}", temp_bundle_path.display()))?;
    // Close the file before renaming it; Windows refuses to replace a file
    // while another handle to it is still open.
    drop(writer);

    let archive_bytes = fs::metadata(&temp_bundle_path)
        .map_err(|e| format!("failed to read {}: {e}", temp_bundle_path.display()))?
        .len();

    // Atomic replace makes the final bundle appear only after it has been fully
    // written and flushed.
    fs::rename(&temp_bundle_path, &bundle.bundle_path).map_err(|e| {
        format!(
            "failed to replace bundle {} with {}: {e}",
            temp_bundle_path.display(),
            bundle.bundle_path.display()
        )
    })?;

    // 每个分包完成后打一行，出问题能立刻定位到是哪个包（含压缩比与耗时）。
    let ratio = if bundle.stats.total_bytes > 0 {
        archive_bytes as f64 / bundle.stats.total_bytes as f64 * 100.0
    } else {
        0.0
    };
    progress_emit(&format!(
        "[utils.bundle] 完成 {} · {} → {}（{ratio:.0}%）· {} 个条目 · {}",
        bundle.bundle_name,
        format_bytes(bundle.stats.total_bytes),
        format_bytes(archive_bytes),
        bundle.stats.file_count,
        format_duration(started_at.elapsed())
    ));

    Ok(PreparedMeta {
        archive_bytes,
        archive_blake3,
        rebuilt: true,
    })
}

fn read_manifest(bundle_dir: &Path) -> Result<Option<BundleManifest>, String> {
    let path = bundle_dir.join(BUNDLE_MANIFEST_FILE);
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<BundleManifest>(&bytes)
            .map(Some)
            .map_err(|e| format!("failed to parse {}: {e}", path.display())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("failed to read {}: {error}", path.display())),
    }
}

fn write_manifest_if_changed(bundle_dir: &Path, manifest: &BundleManifest) -> Result<bool, String> {
    let path = bundle_dir.join(BUNDLE_MANIFEST_FILE);
    let bytes = serde_json::to_vec(manifest)
        .map_err(|e| format!("failed to serialize manifest: {e}"))?;
    if let Ok(existing) = fs::read(&path) {
        if existing == bytes {
            return Ok(false);
        }
    }
    let temp_path = bundle_dir.join(format!("{BUNDLE_MANIFEST_FILE}.tmp"));
    fs::write(&temp_path, &bytes)
        .map_err(|e| format!("failed to write {}: {e}", temp_path.display()))?;
    fs::rename(&temp_path, &path).map_err(|e| {
        format!(
            "failed to replace manifest {} with {}: {e}",
            path.display(),
            temp_path.display()
        )
    })?;
    Ok(true)
}

fn bundle_files(bundle_dir: &Path) -> Result<Vec<PathBuf>, String> {
    if !bundle_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut bundles = fs::read_dir(bundle_dir)
        .map_err(|e| format!("failed to read dir {}: {e}", bundle_dir.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| is_bundle_file(path))
        .collect::<Vec<_>>();
    bundles.sort();
    Ok(bundles)
}

fn is_bundle_file(path: &Path) -> bool {
    path.is_file()
        && path
            .file_name()
            .and_then(OsStr::to_str)
            .map(|name| name.ends_with(BUNDLE_EXTENSION))
            .unwrap_or(false)
}

fn normalize_relative(path: &Path) -> Option<PathBuf> {
    let mut sanitized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => sanitized.push(part),
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => return None,
        }
    }
    if sanitized.as_os_str().is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn metadata_mtime_ns(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

pub fn extract_bundle_archives_to_destination(
    bundle_root: &Path,
    destination: &Path,
) -> Result<usize, String> {
    let result = run_extraction(bundle_root, destination, None, false)?;
    Ok(result.bundle_count)
}

/// Incremental installer path: uses `bundle-manifest.json` plus the recorded
/// installation state to avoid rewriting payloads that are already present.
pub fn sync_bundle_archives_to_destination(
    bundle_root: &Path,
    destination: &Path,
    state_file: Option<&Path>,
) -> Result<ExtractResult, String> {
    run_extraction(bundle_root, destination, state_file, true)
}

struct BundlePlan<'a> {
    name: String,
    path: PathBuf,
    manifest: Option<&'a BundleManifestEntry>,
}

fn bundle_state_key(entry: &BundleManifestEntry) -> String {
    if entry.archive_blake3.is_empty() {
        entry.digest.clone()
    } else {
        entry.archive_blake3.clone()
    }
}

fn run_extraction(
    bundle_root: &Path,
    destination: &Path,
    state_file: Option<&Path>,
    incremental: bool,
) -> Result<ExtractResult, String> {
    fs::create_dir_all(destination)
        .map_err(|e| format!("创建目标目录失败: {} ({e})", destination.display()))?;

    let manifest = match read_manifest(bundle_root) {
        Ok(Some(manifest)) if manifest.schema == BUNDLE_FORMAT_VERSION => Some(manifest),
        Ok(Some(_)) if incremental => {
            return Err(format!(
                "资源清单版本不兼容: {}",
                bundle_root.join(BUNDLE_MANIFEST_FILE).display()
            ));
        }
        Ok(_) => None,
        Err(error) if incremental => return Err(error),
        Err(error) => {
            eprintln!("[utils.bundle] ignoring unreadable manifest: {error}");
            None
        }
    };

    if incremental && manifest.is_none() {
        return Err(format!(
            "安装包缺少资源清单 {}，请使用完整的安装包",
            bundle_root.join(BUNDLE_MANIFEST_FILE).display()
        ));
    }

    let mut ignored_unlisted = 0usize;
    let plans: Vec<BundlePlan<'_>> = if let Some(manifest) = &manifest {
        if manifest.bundles.is_empty() {
            return Err("资源清单没有列出任何分包".to_string());
        }
        let listed: HashSet<&str> = manifest
            .bundles
            .iter()
            .map(|bundle| bundle.name.as_str())
            .collect();
        for path in bundle_files(bundle_root)? {
            let name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
            if !listed.contains(name) {
                ignored_unlisted += 1;
            }
        }
        let mut seen_names = HashSet::new();
        manifest
            .bundles
            .iter()
            .map(|bundle| {
                let mut components = Path::new(&bundle.name).components();
                let flat_name = matches!(components.next(), Some(Component::Normal(_)))
                    && components.next().is_none();
                if !flat_name {
                    return Err(format!("资源分包名非法: {}", bundle.name));
                }
                if !seen_names.insert(bundle.name.clone()) {
                    return Err(format!("资源清单包含重复分包: {}", bundle.name));
                }
                let path = bundle_root.join(&bundle.name);
                if !path.is_file() {
                    return Err(format!("缺少资源分包: {}", path.display()));
                }
                Ok(BundlePlan {
                    name: bundle.name.clone(),
                    path,
                    manifest: Some(bundle),
                })
            })
            .collect::<Result<Vec<_>, String>>()?
    } else {
        let files = bundle_files(bundle_root)?;
        if files.is_empty() {
            return Err(format!(
                "未找到 utils 资源分包: {}",
                bundle_root.to_string_lossy()
            ));
        }
        files
            .into_iter()
            .map(|path| BundlePlan {
                name: path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or_default()
                    .to_string(),
                path,
                manifest: None,
            })
            .collect()
    };

    if ignored_unlisted > 0 {
        eprintln!(
            "[utils.bundle] ignoring {ignored_unlisted} bundle(s) not listed in {BUNDLE_MANIFEST_FILE}"
        );
    }

    let mut state = if incremental {
        state_file.map(load_state).unwrap_or_default()
    } else {
        BundleState::default()
    };
    // The per-bundle checkpoint below may save the state before the final
    // prune pass, so stamp the schema up front instead of writing a state file
    // that the next run would reject as stale.
    state.schema = STATE_FILE_SCHEMA;

    let mut result = ExtractResult {
        bundle_count: plans.len(),
        ..ExtractResult::default()
    };
    let mut extract_indices = Vec::new();
    for (index, plan) in plans.iter().enumerate() {
        let Some(entry) = plan.manifest else {
            extract_indices.push(index);
            continue;
        };
        let skip = incremental
            && state
                .bundles
                .get(&plan.name)
                .map(|key| key == &bundle_state_key(entry))
                .unwrap_or(false)
            && source_present(destination, entry)
            && signatures_match(destination, entry);
        if skip {
            result.skipped_bundle_count += 1;
            result.skipped_file_count += entry.files.len();
        } else {
            extract_indices.push(index);
        }
    }

    // Preflight every archive that is about to be written before touching the
    // destination. A stale/mixed utils_bundles folder must fail loudly instead
    // of leaving a half-new installation behind.
    if incremental {
        extract_indices
            .par_iter()
            .try_for_each(|&index| -> Result<(), String> {
                let plan = &plans[index];
                if let Some(entry) = plan.manifest {
                    verify_archive(&plan.path, entry)?;
                }
                Ok(())
            })?;
    }

    let state_mutex = state_file.map(|_| Mutex::new(state));
    let extract_started = Instant::now();
    progress_emit(&format!(
        "[utils.bundle] 安装解包 · 共 {} 个分包 · 本次解压 {} 个 · 增量跳过 {} 个 · 目标 {}",
        plans.len(),
        extract_indices.len(),
        result.skipped_bundle_count,
        destination.display()
    ));
    let extract_results: Vec<Result<ExtractBundleStats, String>> = extract_indices
        .par_iter()
        .map(|&index| {
            let plan = &plans[index];
            let bundle_started = Instant::now();
            let stats = extract_single_bundle(&plan.path, destination, plan.manifest)?;
            if let Some(entry) = plan.manifest {
                verify_files_complete(destination, entry)?;
            }
            progress_emit(&format!(
                "[utils.bundle] 解包完成 {} · 写入 {} 个文件 · 跳过 {} 个 · {}",
                plan.name,
                stats.written_files,
                stats.skipped_files,
                format_duration(bundle_started.elapsed())
            ));
            Ok(stats)
        })
        .collect();

    for (index, stats) in extract_indices.iter().zip(extract_results) {
        let plan = &plans[*index];
        let stats = stats?;
        result.extracted_bundle_count += 1;
        result.written_file_count += stats.written_files;
        result.skipped_file_count += stats.skipped_files;

        if let (Some(path), Some(state_mutex), Some(entry)) =
            (state_file, state_mutex.as_ref(), plan.manifest)
        {
            let mut guard = state_mutex
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard
                .bundles
                .insert(plan.name.clone(), bundle_state_key(entry));
            if let Err(error) = save_state(path, &guard) {
                eprintln!("[utils.bundle] failed to save install state: {error}");
            }
        }
    }

    if let (Some(path), Some(state_mutex), Some(manifest)) =
        (state_file, state_mutex.as_ref(), manifest.as_ref())
    {
        let allowed: HashSet<&str> = manifest
            .bundles
            .iter()
            .map(|bundle| bundle.name.as_str())
            .collect();
        let mut guard = state_mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.bundles.retain(|name, _| allowed.contains(name.as_str()));
        guard.schema = STATE_FILE_SCHEMA;
        if let Err(error) = save_state(path, &guard) {
            eprintln!("[utils.bundle] failed to save install state: {error}");
        }
    }

    progress_emit(&format!(
        "[utils.bundle] 安装解包完成 · 解压 {} 个分包 · 写入 {} 个文件 · 跳过 {} 个文件 · 总用时 {}",
        result.extracted_bundle_count,
        result.written_file_count,
        result.skipped_file_count,
        format_duration(extract_started.elapsed())
    ));

    Ok(result)
}

fn load_state(path: &Path) -> BundleState {
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<BundleState>(&bytes) {
            Ok(state) if state.schema == STATE_FILE_SCHEMA => state,
            Ok(_) => BundleState::default(),
            Err(error) => {
                eprintln!(
                    "[utils.bundle] ignoring invalid install state {}: {error}",
                    path.display()
                );
                BundleState::default()
            }
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => BundleState::default(),
        Err(error) => {
            eprintln!(
                "[utils.bundle] failed to read install state {}: {error}",
                path.display()
            );
            BundleState::default()
        }
    }
}

fn save_state(path: &Path, state: &BundleState) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create state dir {}: {e}", parent.display()))?;
    }
    let bytes = serde_json::to_vec(state)
        .map_err(|e| format!("failed to serialize install state: {e}"))?;
    let temp_path = path.with_file_name(format!(
        ".{}.tmp",
        path.file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("guaxi-bundles.state")
    ));
    fs::write(&temp_path, &bytes)
        .map_err(|e| format!("failed to write {}: {e}", temp_path.display()))?;
    fs::rename(&temp_path, path).map_err(|e| {
        format!(
            "failed to replace state {} with {}: {e}",
            path.display(),
            temp_path.display()
        )
    })
}

fn verify_archive(bundle: &Path, entry: &BundleManifestEntry) -> Result<(), String> {
    let metadata = fs::metadata(bundle)
        .map_err(|e| format!("读取资源分包失败: {} ({e})", bundle.display()))?;
    if metadata.len() != entry.archive_bytes {
        return Err(format!(
            "资源分包大小与清单不一致: {} (实际 {} 字节，清单 {} 字节)",
            bundle.display(),
            metadata.len(),
            entry.archive_bytes
        ));
    }
    if entry.archive_blake3.is_empty() {
        return Ok(());
    }

    let mut file = File::open(bundle)
        .map_err(|e| format!("读取资源分包失败: {} ({e})", bundle.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| format!("读取资源分包失败: {} ({e})", bundle.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = hasher.finalize().to_hex().to_string();
    if actual != entry.archive_blake3 {
        return Err(format!(
            "资源分包内容与清单不一致，请确认 utils_bundles 与安装包来自同一版本: {}",
            bundle.display()
        ));
    }
    Ok(())
}

fn source_present(destination: &Path, entry: &BundleManifestEntry) -> bool {
    let Some(relative) = normalize_relative(Path::new(&entry.source)) else {
        return false;
    };
    let path = destination.join(relative);
    match SourceKind::from_str(&entry.source_kind) {
        SourceKind::File => path.is_file(),
        SourceKind::Dir => path.is_dir(),
    }
}

fn signatures_match(destination: &Path, entry: &BundleManifestEntry) -> bool {
    for file in &entry.files {
        let Some(relative) = normalize_relative(Path::new(&file.path)) else {
            return false;
        };
        if !file_signature_matches(&destination.join(relative), file) {
            return false;
        }
    }
    for dir in &entry.empty_dirs {
        let Some(relative) = normalize_relative(Path::new(dir)) else {
            return false;
        };
        if !destination.join(relative).is_dir() {
            return false;
        }
    }
    true
}

fn file_signature_matches(path: &Path, expected: &BundleFileEntry) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() || metadata.len() != expected.size {
        return false;
    }
    if expected.mtime_ns == 0 {
        return true;
    }
    metadata_mtime_ns(&metadata).abs_diff(expected.mtime_ns) <= MTIME_TOLERANCE_NS
}

fn verify_files_complete(destination: &Path, entry: &BundleManifestEntry) -> Result<(), String> {
    for file in &entry.files {
        let Some(relative) = normalize_relative(Path::new(&file.path)) else {
            return Err(format!("资源清单包含非法路径: {}", file.path));
        };
        let path = destination.join(relative);
        let metadata = fs::metadata(&path)
            .map_err(|e| format!("资源文件缺失: {} ({e})", path.display()))?;
        if !metadata.is_file() || metadata.len() != file.size {
            return Err(format!(
                "资源文件大小不一致: {} (实际 {} 字节，清单 {} 字节)",
                path.display(),
                metadata.len(),
                file.size
            ));
        }
    }
    for dir in &entry.empty_dirs {
        let Some(relative) = normalize_relative(Path::new(dir)) else {
            return Err(format!("资源清单包含非法目录: {dir}"));
        };
        let path = destination.join(relative);
        if !path.is_dir() {
            return Err(format!("资源目录缺失: {}", path.display()));
        }
    }
    Ok(())
}

fn extract_single_bundle(
    bundle: &Path,
    destination: &Path,
    manifest: Option<&BundleManifestEntry>,
) -> Result<ExtractBundleStats, String> {
    let file = File::open(bundle)
        .map_err(|e| format!("打开资源分包失败: {} ({e})", bundle.to_string_lossy()))?;
    let decoder = Decoder::new(file)
        .map_err(|e| format!("读取资源分包失败: {} ({e})", bundle.to_string_lossy()))?;
    let mut archive = Archive::new(decoder);
    let mut entries = archive
        .entries()
        .map_err(|e| format!("读取资源分包条目失败: {} ({e})", bundle.to_string_lossy()))?;

    let expected: HashMap<String, &BundleFileEntry> = manifest
        .map(|entry| {
            entry
                .files
                .iter()
                .map(|file| (normalize_path(Path::new(&file.path)), file))
                .collect()
        })
        .unwrap_or_default();
    let mut stats = ExtractBundleStats::default();

    while let Some(next_entry) = entries.next() {
        let mut entry = next_entry
            .map_err(|e| format!("读取资源分包条目失败: {} ({e})", bundle.to_string_lossy()))?;
        let entry_path = entry
            .path()
            .map_err(|e| format!("读取资源分包路径失败: {} ({e})", bundle.to_string_lossy()))?;
        // Never trust archive paths directly. Reject absolute paths and parent
        // traversals so a malformed bundle cannot escape the destination root.
        let relative = normalize_relative(entry_path.as_ref()).ok_or_else(|| {
            format!(
                "资源分包包含非法路径: {} ({})",
                bundle.to_string_lossy(),
                entry_path.display()
            )
        })?;
        let out_path = destination.join(&relative);
        let entry_type = entry.header().entry_type();

        if entry_type.is_dir() {
            fs::create_dir_all(&out_path).map_err(|e| {
                format!("创建资源目录失败: {} ({e})", out_path.to_string_lossy())
            })?;
            continue;
        }

        if entry_type == EntryType::Regular {
            let expected_file = expected.get(&normalize_path(&relative)).copied();
            if let Some(expected_file) = expected_file {
                if file_signature_matches(&out_path, expected_file) {
                    stats.skipped_files += 1;
                    continue;
                }
            }

            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "创建资源文件父目录失败: {} ({e})",
                        parent.to_string_lossy()
                    )
                })?;
            }

            let temp_path = temp_path_for(&out_path)?;
            if temp_path.exists() {
                let _ = fs::remove_file(&temp_path);
            }
            entry.unpack(&temp_path).map_err(|e| {
                format!(
                    "写入资源临时文件失败: {} ({e})",
                    temp_path.to_string_lossy()
                )
            })?;

            if let Some(expected_file) = expected_file {
                if expected_file.mtime_ns > 0 {
                    if let Err(error) = set_file_mtime(&temp_path, expected_file.mtime_ns) {
                        eprintln!(
                            "[utils.bundle] failed to restore mtime for {}: {error}",
                            out_path.display()
                        );
                    }
                }
            }

            replace_file(&temp_path, &out_path)?;
            stats.written_files += 1;
            continue;
        }

        return Err(format!(
            "资源分包包含不支持的条目类型: {} ({:?})",
            relative.to_string_lossy(),
            entry_type
        ));
    }

    Ok(stats)
}

fn temp_path_for(target: &Path) -> Result<PathBuf, String> {
    let name = target
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("无法解析资源文件名: {}", target.display()))?;
    Ok(target.with_file_name(format!(
        ".{name}.guaxi-tmp-{}",
        std::process::id()
    )))
}

fn set_file_mtime(path: &Path, mtime_ns: u64) -> Result<(), String> {
    clear_readonly(path);
    let file = OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|e| format!("打开文件失败: {} ({e})", path.display()))?;
    let time = UNIX_EPOCH + Duration::from_nanos(mtime_ns);
    file.set_modified(time)
        .map_err(|e| format!("设置文件时间失败: {} ({e})", path.display()))
}

fn clear_readonly(path: &Path) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    let mut permissions = metadata.permissions();
    if permissions.readonly() {
        permissions.set_readonly(false);
        let _ = fs::set_permissions(path, permissions);
    }
}

fn replace_file(temp_path: &Path, target: &Path) -> Result<(), String> {
    if target.is_dir() {
        return Err(format!(
            "资源目标路径是目录，无法替换: {}",
            target.display()
        ));
    }

    for attempt in 0..20 {
        match fs::rename(temp_path, target) {
            Ok(()) => return Ok(()),
            Err(error) => {
                clear_readonly(target);
                if attempt == 19 {
                    return Err(format!(
                        "替换资源文件失败: {} -> {} ({error})",
                        temp_path.display(),
                        target.display()
                    ));
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    Err(format!("替换资源文件失败: {}", target.display()))
}

pub fn bundle_extension() -> &'static str {
    BUNDLE_EXTENSION
}

pub fn bundle_dir_name() -> &'static str {
    GENERATED_BUNDLES_DIR
}

#[cfg(test)]
mod tests {
    use super::should_skip;

    /// 打包排除规则是「release 里别混进冗余」的唯一防线，钉住它。
    #[test]
    fn skips_download_archives_and_retired_speech_binaries() {
        // 模型与原生库的下载归档只是中间产物，解压后就没用了，不能进包。
        assert!(should_skip("Models/whisper_onnx/sherpa-onnx-whisper-base.en.tar.bz2"));
        assert!(should_skip("Models/tts/kokoro-multi-lang-v1_1.tar.bz2"));

        // 语音已改成进程内调用库，这两棵外部二进制树不该再进包。
        assert!(should_skip("sherpa-onnx"));
        assert!(should_skip("sherpa-onnx/sherpa-onnx-offline-tts.exe"));
        assert!(should_skip("whisper"));
        assert!(should_skip("whisper/whisper-cli.exe"));

        // 运行期真正要加载的模型必须保留。
        assert!(!should_skip("Models/tts/kokoro_multi_lang/model.onnx"));
        assert!(!should_skip("Models/asr/sense-voice/tokens.txt"));
        assert!(!should_skip("Models/vad/silero_vad.onnx"));
    }

}
