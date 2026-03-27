use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const ZIP_READ_BUFFER_SIZE: usize = 1024 * 1024;
const ZIP_WRITE_BUFFER_SIZE: usize = 8 * 1024 * 1024;
const LARGE_FILE_STORE_THRESHOLD: u64 = 8 * 1024 * 1024;
const GENERATED_BUNDLES_DIR: &str = "__bundles";

const LEGACY_BUNDLES: &[&str] = &[
    "utils.zip",
    "SlidevEnv.zip",
    "sherpa-onnx.zip",
    "whisper.zip",
    "llama.zip",
    "python_libs.zip",
    "Models/tts.zip",
    "Models/whisper_models.zip",
];

const SKIPPED_DIRECTORIES: &[&str] = &[
    "MarkdownConvertor",
    "Models/surya_models",
    GENERATED_BUNDLES_DIR,
];

const STORED_EXTENSIONS: &[&str] = &[
    "7z", "apk", "avif", "bin", "br", "bz2", "class", "deb", "dll", "dylib", "egg", "exe",
    "gif", "gz", "ico", "jar", "jpeg", "jpg", "mp3", "mp4", "msi", "nupkg", "onnx", "otf",
    "pdf", "png", "pyc", "pyd", "pyo", "rpm", "safetensors", "so", "svgz", "tar", "tgz", "ttf",
    "wav", "webm", "webp", "whl", "woff", "woff2", "xz", "zip",
];

const COMPRESSIBLE_EXTENSIONS: &[&str] = &[
    "c", "cc", "cfg", "conf", "cpp", "css", "csv", "go", "h", "hpp", "html", "ini", "java",
    "js", "json", "jsx", "lua", "md", "mjs", "py", "rb", "rs", "scss", "sh", "sql", "svg",
    "toml", "ts", "tsx", "txt", "vue", "xml", "yaml", "yml",
];

#[derive(Debug, Clone, Copy)]
pub enum ZipStrategy {
    SevenZip,
    ZipCli,
    RustFallback,
}

impl ZipStrategy {
    pub fn label(self) -> &'static str {
        match self {
            ZipStrategy::SevenZip => "7z",
            ZipStrategy::ZipCli => "zip",
            ZipStrategy::RustFallback => "RustFallback",
        }
    }
}

#[derive(Debug, Clone)]
struct ZipEntrySpec {
    relative: PathBuf,
    kind: ZipEntryKind,
}

#[derive(Debug, Clone, Copy)]
enum ZipEntryKind {
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
    top_level_relative: PathBuf,
    entries: Vec<ZipEntrySpec>,
    stats: EntryStats,
}

#[derive(Debug, Clone)]
pub struct PrepareResult {
    pub bundle_dir: PathBuf,
    pub strategy: ZipStrategy,
    pub reused_existing: bool,
    pub elapsed_ms: u128,
    pub bundle_count: usize,
    pub file_count: usize,
    pub empty_dir_count: usize,
    pub total_bytes: u64,
}

pub fn should_skip(relative: &str) -> bool {
    let normalized = relative.replace('\\', "/");
    LEGACY_BUNDLES.contains(&normalized.as_str())
        || SKIPPED_DIRECTORIES.iter().any(|dir| {
            normalized == *dir || normalized.starts_with(&format!("{dir}/"))
        })
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
    let started_at = Instant::now();
    let utils_dir = manifest_dir.join("utils");
    let bundle_dir = utils_dir.join(GENERATED_BUNDLES_DIR);

    if !utils_dir.is_dir() {
        return Err(format!("utils dir not found: {}", utils_dir.display()));
    }

    if reuse_existing && has_existing_bundles(&bundle_dir) {
        let bundle_count = count_existing_bundles(&bundle_dir);
        return Ok(PrepareResult {
            bundle_dir,
            strategy: ZipStrategy::RustFallback,
            reused_existing: true,
            elapsed_ms: started_at.elapsed().as_millis(),
            bundle_count,
            file_count: 0,
            empty_dir_count: 0,
            total_bytes: 0,
        });
    }

    reset_bundle_dir(&bundle_dir)?;

    let bundles = collect_bundle_specs(&utils_dir, &bundle_dir)?;
    let aggregate_stats = summarize_bundle_specs(&bundles);
    let strategy = detect_strategy(&utils_dir);

    for bundle in &bundles {
        prepare_single_bundle(&utils_dir, bundle, strategy)?;
    }

    Ok(PrepareResult {
        bundle_dir,
        strategy,
        reused_existing: false,
        elapsed_ms: started_at.elapsed().as_millis(),
        bundle_count: bundles.len(),
        file_count: aggregate_stats.file_count,
        empty_dir_count: aggregate_stats.empty_dir_count,
        total_bytes: aggregate_stats.total_bytes,
    })
}

fn has_existing_bundles(bundle_dir: &Path) -> bool {
    std::fs::read_dir(bundle_dir)
        .ok()
        .map(|entries| {
            entries.flatten().any(|entry| {
                entry.path().is_file()
                    && entry
                        .path()
                        .extension()
                        .and_then(OsStr::to_str)
                        .map(|ext| ext.eq_ignore_ascii_case("zip"))
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn count_existing_bundles(bundle_dir: &Path) -> usize {
    std::fs::read_dir(bundle_dir)
        .ok()
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    entry.path().is_file()
                        && entry
                            .path()
                            .extension()
                            .and_then(OsStr::to_str)
                            .map(|ext| ext.eq_ignore_ascii_case("zip"))
                            .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

fn reset_bundle_dir(bundle_dir: &Path) -> Result<(), String> {
    if bundle_dir.exists() {
        fs::remove_dir_all(bundle_dir)
            .map_err(|e| format!("failed to remove old {}: {e}", bundle_dir.display()))?;
    }
    fs::create_dir_all(bundle_dir)
        .map_err(|e| format!("failed to create {}: {e}", bundle_dir.display()))
}

fn detect_strategy(utils_dir: &Path) -> ZipStrategy {
    if ["7z", "7zz", "7za"]
        .into_iter()
        .any(|candidate| has_command(candidate, utils_dir))
    {
        return ZipStrategy::SevenZip;
    }

    if cfg!(not(windows)) && has_command("zip", utils_dir) {
        return ZipStrategy::ZipCli;
    }

    ZipStrategy::RustFallback
}

fn prepare_single_bundle(
    utils_dir: &Path,
    bundle: &BundleSpec,
    strategy: ZipStrategy,
) -> Result<(), String> {
    match strategy {
        ZipStrategy::SevenZip => prepare_single_bundle_with_7z(utils_dir, bundle),
        ZipStrategy::ZipCli => prepare_single_bundle_with_zip(utils_dir, bundle),
        ZipStrategy::RustFallback => prepare_single_bundle_with_rust(utils_dir, bundle),
    }
}

fn prepare_single_bundle_with_7z(utils_dir: &Path, bundle: &BundleSpec) -> Result<(), String> {
    let program = ["7z", "7zz", "7za"]
        .into_iter()
        .find(|candidate| has_command(candidate, utils_dir))
        .ok_or_else(|| "7z not found".to_string())?;

    let listfile = write_listfile(utils_dir, &bundle.entries)?;
    let status = Command::new(program)
        .current_dir(utils_dir)
        .arg("a")
        .arg("-tzip")
        .arg("-mx=1")
        .arg("-mmt=on")
        .arg(&bundle.bundle_path)
        .arg(format!("@{}", listfile.file_name().unwrap().to_string_lossy()))
        .status();

    let _ = fs::remove_file(&listfile);

    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => {
            let _ = fs::remove_file(&bundle.bundle_path);
            Err(format!("7z failed for {}", bundle.bundle_path.display()))
        }
        Err(e) => Err(format!("failed to run {program}: {e}")),
    }
}

fn prepare_single_bundle_with_zip(utils_dir: &Path, bundle: &BundleSpec) -> Result<(), String> {
    let target_name = bundle
        .bundle_path
        .strip_prefix(utils_dir)
        .unwrap_or(&bundle.bundle_path)
        .to_path_buf();
    let top_level_name = normalize_path(&bundle.top_level_relative);
    let is_dir = utils_dir.join(&bundle.top_level_relative).is_dir();
    let mut command = Command::new("zip");
    command.current_dir(utils_dir).arg("-q").arg("-1");

    if is_dir {
        command.arg("-r");
    }

    command
        .arg(target_name)
        .arg(&top_level_name)
        .arg("-x")
        .arg(format!("{GENERATED_BUNDLES_DIR}/*"))
        .arg(format!("{GENERATED_BUNDLES_DIR}/**"))
        .arg("utils.zip")
        .arg("SlidevEnv.zip")
        .arg("sherpa-onnx.zip")
        .arg("whisper.zip")
        .arg("llama.zip")
        .arg("python_libs.zip")
        .arg("Models/tts.zip")
        .arg("Models/whisper_models.zip")
        .arg("MarkdownConvertor/*")
        .arg("MarkdownConvertor/**")
        .arg("Models/surya_models/*")
        .arg("Models/surya_models/**")
        .arg("*.guaxi_extract_complete")
        .arg("*.guaxi_utils_extract_complete")
        .arg("*.__extracting_*");

    match command.status() {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => {
            let _ = fs::remove_file(&bundle.bundle_path);
            Err(format!("zip failed for {}", bundle.bundle_path.display()))
        }
        Err(e) => Err(format!("failed to run zip: {e}")),
    }
}

fn prepare_single_bundle_with_rust(utils_dir: &Path, bundle: &BundleSpec) -> Result<(), String> {
    let file = File::create(&bundle.bundle_path)
        .map_err(|e| format!("failed to create {}: {e}", bundle.bundle_path.display()))?;
    let writer = BufWriter::with_capacity(ZIP_WRITE_BUFFER_SIZE, file);
    let mut zip = ZipWriter::new(writer);

    for entry in &bundle.entries {
        let relative_text = normalize_path(&entry.relative);
        match entry.kind {
            ZipEntryKind::EmptyDir => {
                zip.add_directory(format!("{relative_text}/"), directory_file_options())
                    .map_err(|e| {
                        format!("failed to add dir {relative_text} to zip: {e}")
                    })?;
            }
            ZipEntryKind::File { size } => {
                let full_path = utils_dir.join(&entry.relative);
                let mut reader = BufReader::with_capacity(
                    ZIP_READ_BUFFER_SIZE,
                    File::open(&full_path)
                        .map_err(|e| format!("failed to open {}: {e}", full_path.display()))?,
                );
                zip.start_file(relative_text.clone(), file_options_for_path(&entry.relative, size))
                    .map_err(|e| {
                        format!("failed to start file {relative_text} in zip: {e}")
                    })?;
                io::copy(&mut reader, &mut zip)
                    .map_err(|e| format!("failed to write {relative_text} to zip: {e}"))?;
            }
        }
    }

    zip.finish()
        .map_err(|e| format!("failed to finish {}: {e}", bundle.bundle_path.display()))?;
    Ok(())
}

fn collect_bundle_specs(utils_dir: &Path, bundle_dir: &Path) -> Result<Vec<BundleSpec>, String> {
    let mut entries = fs::read_dir(utils_dir)
        .map_err(|e| format!("failed to read dir {}: {e}", utils_dir.display()))?
        .flatten()
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());

    let mut bundles = Vec::new();
    for entry in entries {
        let path = entry.path();
        let relative = path
            .strip_prefix(utils_dir)
            .map_err(|e| {
                format!(
                    "failed to strip prefix {} from {}: {e}",
                    utils_dir.display(),
                    path.display()
                )
            })?
            .to_path_buf();

        if should_skip(&normalize_path(&relative)) {
            continue;
        }

        let entries = if path.is_dir() {
            let nested = collect_entries(utils_dir, &path)?;
            if nested.is_empty() {
                vec![ZipEntrySpec {
                    relative: relative.clone(),
                    kind: ZipEntryKind::EmptyDir,
                }]
            } else {
                nested
            }
        } else {
            let size = entry
                .metadata()
                .map_err(|e| format!("failed to read metadata {}: {e}", path.display()))?
                .len();
            vec![ZipEntrySpec {
                relative: relative.clone(),
                kind: ZipEntryKind::File { size },
            }]
        };

        let bundle_name = format!("{}.zip", relative.file_name().unwrap().to_string_lossy());
        bundles.push(BundleSpec {
            bundle_path: bundle_dir.join(bundle_name),
            top_level_relative: relative,
            stats: summarize_entries(&entries),
            entries,
        });
    }

    Ok(bundles)
}

fn collect_entries(root_dir: &Path, current_dir: &Path) -> Result<Vec<ZipEntrySpec>, String> {
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
                collected.push(ZipEntrySpec {
                    relative,
                    kind: ZipEntryKind::EmptyDir,
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
        collected.push(ZipEntrySpec {
            relative,
            kind: ZipEntryKind::File { size },
        });
    }

    Ok(collected)
}

fn write_listfile(utils_dir: &Path, entries: &[ZipEntrySpec]) -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let listfile = utils_dir.join(format!(".utils_zip_list_{nanos}.txt"));
    let mut file = BufWriter::new(
        File::create(&listfile)
            .map_err(|e| format!("failed to create {}: {e}", listfile.display()))?,
    );

    for entry in entries {
        #[cfg(windows)]
        let line = entry.relative.as_os_str().to_string_lossy().replace('/', "\\");
        #[cfg(not(windows))]
        let line = entry.relative.as_os_str().to_string_lossy().replace('\\', "/");

        writeln!(file, "{line}")
            .map_err(|e| format!("failed to write {}: {e}", listfile.display()))?;
    }

    file.flush()
        .map_err(|e| format!("failed to flush {}: {e}", listfile.display()))?;
    Ok(listfile)
}

fn summarize_entries(entries: &[ZipEntrySpec]) -> EntryStats {
    let mut stats = EntryStats::default();
    for entry in entries {
        match entry.kind {
            ZipEntryKind::File { size } => {
                stats.file_count += 1;
                stats.total_bytes += size;
            }
            ZipEntryKind::EmptyDir => {
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

fn has_command(program: &str, workdir: &Path) -> bool {
    let status = if cfg!(windows) {
        Command::new("where")
            .current_dir(workdir)
            .arg(program)
            .status()
    } else {
        Command::new("sh")
            .current_dir(workdir)
            .arg("-lc")
            .arg(format!("command -v {} >/dev/null 2>&1", shell_escape(program)))
            .status()
    };

    matches!(status, Ok(status) if status.success())
}

fn shell_escape(value: &str) -> String {
    value.replace('\'', "'\\''")
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn directory_file_options() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
}

fn file_options_for_path(relative: &Path, size: u64) -> SimpleFileOptions {
    let extension = relative
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());
    let known_stored_extension = extension
        .as_deref()
        .map(|ext| STORED_EXTENSIONS.contains(&ext))
        .unwrap_or(false);
    let known_compressible_extension = extension
        .as_deref()
        .map(|ext| COMPRESSIBLE_EXTENSIONS.contains(&ext))
        .unwrap_or(false);
    let stored =
        known_stored_extension || (size >= LARGE_FILE_STORE_THRESHOLD && !known_compressible_extension);

    if stored {
        SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
    } else {
        SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(1))
    }
}
