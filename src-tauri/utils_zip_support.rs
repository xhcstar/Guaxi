#![allow(dead_code)]

// Shared build/install bundle support for the large `src-tauri/utils` runtime.
//
// Design goals:
// - Debug/dev mode must be able to run directly from the source-tree utils dir.
// - Release builds must avoid embedding multi-GB resources directly in NSIS.
// - Bundle creation and extraction must be deterministic, recoverable and fast.
// - The on-disk structure after extraction must exactly match the original
//   `utils/...` layout so existing runtime commands can keep using
//   `ensure_relative_dir("utils/...")` without special cases.

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;
use tar::{Archive, Builder, EntryType};
use zstd::stream::{Decoder, Encoder};

const ZSTD_COMPRESSION_LEVEL: i32 = 3;
const ZSTD_ENCODER_WRITE_BUFFER_SIZE: usize = 8 * 1024 * 1024;
const GENERATED_BUNDLES_DIR: &str = "__bundles";
const BUNDLE_EXTENSION: &str = ".tar.zst";

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

#[derive(Debug, Clone)]
struct BundleEntrySpec {
    relative: PathBuf,
    source: PathBuf,
    kind: BundleEntryKind,
}

#[derive(Debug, Clone, Copy)]
enum BundleEntryKind {
    File { size: u64 },
    EmptyDir,
}

#[derive(Debug, Clone, Copy, Default)]
struct EntryStats {
    file_count: usize,
    empty_dir_count: usize,
    total_bytes: u64,
}

#[derive(Debug, Clone)]
struct BundleSpec {
    bundle_path: PathBuf,
    entries: Vec<BundleEntrySpec>,
    stats: EntryStats,
}

#[derive(Debug, Clone)]
pub struct PrepareResult {
    pub bundle_dir: PathBuf,
    pub strategy: BundleStrategy,
    pub reused_existing: bool,
    pub elapsed_ms: u128,
    pub bundle_count: usize,
    pub file_count: usize,
    pub empty_dir_count: usize,
    pub total_bytes: u64,
}

// Skip legacy generated artifacts, temporary extraction directories and resource
// trees that are intentionally excluded from release payloads.
pub fn should_skip(relative: &str) -> bool {
    let normalized = relative.replace('\\', "/");
    LEGACY_BUNDLES.contains(&normalized.as_str())
        || SKIPPED_DIRECTORIES.iter().any(|dir| {
            normalized == *dir || normalized.starts_with(&format!("{dir}/"))
        })
        // 语音模型与原生库的下载归档只是中间产物，解压完就没用了。
        // 一旦误留在 utils 下，就会被整包打进 release 白增几百 MB。
        || normalized.to_ascii_lowercase().ends_with(".tar.bz2")
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

    // Reusing previously generated bundles is important for large release
    // payloads: repacking multi-GB resources can take many minutes. An
    // interrupted run leaves bundle temp files behind, so reuse only when the
    // directory holds exactly the expected bundle set.
    if reuse_existing && bundles_are_complete(source_dir, bundle_dir, include_extension)? {
        let bundle_count = count_existing_bundles(bundle_dir);
        return Ok(PrepareResult {
            bundle_dir: bundle_dir.to_path_buf(),
            strategy: BundleStrategy::RustTarZstd,
            reused_existing: true,
            elapsed_ms: started_at.elapsed().as_millis(),
            bundle_count,
            file_count: 0,
            empty_dir_count: 0,
            total_bytes: 0,
        });
    }

    reset_bundle_dir(bundle_dir)?;
    let bundles = collect_bundle_specs(source_dir, bundle_dir, include_extension)?;
    let aggregate_stats = summarize_bundle_specs(&bundles);

    // Each top-level payload entry becomes one bundle file. Compressing those
    // bundles in parallel is the main coarse-grained speedup in this pipeline.
    bundles.par_iter().try_for_each(prepare_single_bundle)?;

    Ok(PrepareResult {
        bundle_dir: bundle_dir.to_path_buf(),
        strategy: BundleStrategy::RustTarZstd,
        reused_existing: false,
        elapsed_ms: started_at.elapsed().as_millis(),
        bundle_count: bundles.len(),
        file_count: aggregate_stats.file_count,
        empty_dir_count: aggregate_stats.empty_dir_count,
        total_bytes: aggregate_stats.total_bytes,
    })
}

pub fn extract_bundle_archives_to_destination(
    bundle_root: &Path,
    destination: &Path,
) -> Result<usize, String> {
    let bundles = bundle_files(bundle_root)?;
    if bundles.is_empty() {
        return Err(format!(
            "未找到 utils 资源分包: {}",
            bundle_root.to_string_lossy()
        ));
    }

    fs::create_dir_all(destination)
        .map_err(|e| format!("创建目标目录失败: {} ({e})", destination.to_string_lossy()))?;

    // Bundles target disjoint top-level paths, so they can be expanded in
    // parallel without changing the final directory structure.
    bundles
        .par_iter()
        .try_for_each(|bundle| extract_single_bundle(bundle, destination))?;

    Ok(bundles.len())
}

/// True when `bundle_dir` holds exactly the bundles the source directory
/// describes. Interrupted runs leave `.tmp` files and only a subset of the
/// renamed bundles, which must not be mistaken for a complete payload.
fn bundles_are_complete(
    source_dir: &Path,
    bundle_dir: &Path,
    include_extension: Option<&str>,
) -> Result<bool, String> {
    let specs = collect_bundle_specs(source_dir, bundle_dir, include_extension)?;
    let existing = bundle_files(bundle_dir)?;
    Ok(!specs.is_empty()
        && existing.len() == specs.len()
        && specs.iter().all(|bundle| bundle.bundle_path.is_file()))
}

fn count_existing_bundles(bundle_dir: &Path) -> usize {
    bundle_files(bundle_dir).map(|files| files.len()).unwrap_or(0)
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

fn reset_bundle_dir(bundle_dir: &Path) -> Result<(), String> {
    if bundle_dir.exists() {
        fs::remove_dir_all(bundle_dir)
            .map_err(|e| format!("failed to remove old {}: {e}", bundle_dir.display()))?;
    }
    fs::create_dir_all(bundle_dir)
        .map_err(|e| format!("failed to create {}: {e}", bundle_dir.display()))
}

fn prepare_single_bundle(bundle: &BundleSpec) -> Result<(), String> {
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
    let mut encoder = Encoder::new(writer, ZSTD_COMPRESSION_LEVEL)
        .map_err(|e| format!("failed to start zstd encoder {}: {e}", temp_bundle_path.display()))?;
    let thread_count = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1);
    if thread_count > 1 {
        encoder
            .multithread(thread_count as u32)
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
            }
            BundleEntryKind::File { .. } => {
                archive
                    .append_path_with_name(&entry.source, &entry.relative)
                    .map_err(|e| {
                        format!(
                            "failed to add file {} to archive {}: {e}",
                            entry.source.display(),
                            bundle.bundle_path.display()
                        )
                    })?;
            }
        }
    }

    let encoder = archive
        .into_inner()
        .map_err(|e| format!("failed to finalize tar {}: {e}", temp_bundle_path.display()))?;
    let mut writer = encoder
        .finish()
        .map_err(|e| format!("failed to finalize zstd {}: {e}", temp_bundle_path.display()))?;
    writer
        .flush()
        .map_err(|e| format!("failed to flush {}: {e}", temp_bundle_path.display()))?;
    drop(writer);

    // Atomic replace makes the final bundle appear only after it has been fully
    // written and flushed.
    fs::rename(&temp_bundle_path, &bundle.bundle_path).map_err(|e| {
        format!(
            "failed to replace bundle {} with {}: {e}",
            temp_bundle_path.display(),
            bundle.bundle_path.display()
        )
    })?;
    Ok(())
}

fn extract_single_bundle(bundle: &Path, destination: &Path) -> Result<(), String> {
    let file = File::open(bundle)
        .map_err(|e| format!("打开资源分包失败: {} ({e})", bundle.to_string_lossy()))?;
    let decoder = Decoder::new(file)
        .map_err(|e| format!("读取资源分包失败: {} ({e})", bundle.to_string_lossy()))?;
    let mut archive = Archive::new(decoder);
    let mut entries = archive
        .entries()
        .map_err(|e| format!("读取资源分包条目失败: {} ({e})", bundle.to_string_lossy()))?;

    while let Some(next_entry) = entries.next() {
        let mut entry = next_entry
            .map_err(|e| format!("读取资源分包条目失败: {} ({e})", bundle.to_string_lossy()))?;
        let entry_path = entry
            .path()
            .map_err(|e| format!("读取资源分包路径失败: {} ({e})", bundle.to_string_lossy()))?;
        // Never trust archive paths directly. Reject absolute paths and parent
        // traversals so a malformed bundle cannot escape the destination root.
        let relative = sanitize_relative_path(entry_path.as_ref()).ok_or_else(|| {
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
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "创建资源文件父目录失败: {} ({e})",
                        parent.to_string_lossy()
                    )
                })?;
            }
            entry.unpack(&out_path).map_err(|e| {
                format!(
                    "写入资源目标文件失败: {} ({e})",
                    out_path.to_string_lossy()
                )
            })?;
            continue;
        }

        return Err(format!(
            "资源分包包含不支持的条目类型: {} ({:?})",
            relative.to_string_lossy(),
            entry_type
        ));
    }

    Ok(())
}

fn sanitize_relative_path(path: &Path) -> Option<PathBuf> {
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

fn collect_bundle_specs(
    source_dir: &Path,
    bundle_dir: &Path,
    include_extension: Option<&str>,
) -> Result<Vec<BundleSpec>, String> {
    let mut entries = fs::read_dir(source_dir)
        .map_err(|e| format!("failed to read dir {}: {e}", source_dir.display()))?
        .flatten()
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());

    let mut bundles = Vec::new();
    for entry in entries {
        let path = entry.path();

        // Only filter top-level files: payload directories keep their whole tree.
        if let Some(extension) = include_extension {
            let matches = path
                .extension()
                .and_then(OsStr::to_str)
                .map(|value| value.eq_ignore_ascii_case(extension))
                .unwrap_or(false);
            if path.is_file() && !matches {
                continue;
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

        // We bundle by top-level payload entry so installation can publish,
        // replace and troubleshoot large runtime chunks independently.
        let bundle_entries = if path.is_dir() {
            let nested = collect_entries(source_dir, &path)?;
            if nested.is_empty() {
                vec![BundleEntrySpec {
                    relative: relative.clone(),
                    source: path.clone(),
                    kind: BundleEntryKind::EmptyDir,
                }]
            } else {
                nested
            }
        } else {
            let size = entry
                .metadata()
                .map_err(|e| format!("failed to read metadata {}: {e}", path.display()))?
                .len();
            vec![BundleEntrySpec {
                relative: relative.clone(),
                source: path.clone(),
                kind: BundleEntryKind::File { size },
            }]
        };

        let bundle_name = format!("{}{}", relative.file_name().unwrap().to_string_lossy(), BUNDLE_EXTENSION);
        bundles.push(BundleSpec {
            bundle_path: bundle_dir.join(bundle_name),
            stats: summarize_entries(&bundle_entries),
            entries: bundle_entries,
        });
    }

    Ok(bundles)
}

fn collect_entries(root_dir: &Path, current_dir: &Path) -> Result<Vec<BundleEntrySpec>, String> {
    let mut entries = fs::read_dir(current_dir)
        .map_err(|e| format!("failed to read dir {}: {e}", current_dir.display()))?
        .flatten()
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());

    let mut collected = Vec::new();
    for entry in entries {
        let path = entry.path();
        let relative = path
            .strip_prefix(root_dir)
            .map_err(|e| {
                format!(
                    "failed to strip prefix {} from {}: {e}",
                    root_dir.display(),
                    path.display()
                )
            })?
            .to_path_buf();

        if should_skip(&normalize_path(&relative)) {
            continue;
        }

        if path.is_dir() {
            let nested_entries = collect_entries(root_dir, &path)?;
            if nested_entries.is_empty() {
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

        let size = entry
            .metadata()
            .map_err(|e| format!("failed to read metadata {}: {e}", path.display()))?
            .len();
        collected.push(BundleEntrySpec {
            relative,
            source: path,
            kind: BundleEntryKind::File { size },
        });
    }

    Ok(collected)
}

fn summarize_entries(entries: &[BundleEntrySpec]) -> EntryStats {
    let mut stats = EntryStats::default();
    for entry in entries {
        match entry.kind {
            BundleEntryKind::File { size } => {
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

fn summarize_bundle_specs(bundles: &[BundleSpec]) -> EntryStats {
    let mut stats = EntryStats::default();
    for bundle in bundles {
        stats.file_count += bundle.stats.file_count;
        stats.empty_dir_count += bundle.stats.empty_dir_count;
        stats.total_bytes += bundle.stats.total_bytes;
    }
    stats
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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
