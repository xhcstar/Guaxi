use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime};
use tracing::info;

#[path = "../../utils_zip_support.rs"]
mod utils_zip_support;

// Runtime side of the external utils bundle strategy.
//
// Debug/dev:
// - do not copy multi-GB runtime assets into target/debug
// - resolve `utils/...` back to the source tree when possible
//
// Release:
// - prefer an installed sibling `utils` directory
// - recover interrupted extractions
// - rebuild `utils` from external bundle parts when necessary
static EXTRACT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

const EXTRACT_MARKER: &str = ".guaxi_utils_extract_complete";
const TEMP_EXTRACT_MARKER: &str = ".guaxi_extract_complete";
const PAYLOAD_DIR: &str = "__payload";
const FILE_OP_RETRY_TIMES: usize = 40;
const FILE_OP_RETRY_DELAY_MS: u64 = 250;

#[derive(Debug, Default)]
struct EnsureDirOutcome {
    recovered: bool,
    extracted: bool,
    skipped: bool,
}

fn extract_lock() -> &'static Mutex<()> {
    EXTRACT_LOCK.get_or_init(|| Mutex::new(()))
}

fn exe_dir() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("无法获取 exe 路径: {e}"))?;
    exe_path
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "无法获取 exe 所在目录".to_string())
}

fn sleep_retry() {
    thread::sleep(Duration::from_millis(FILE_OP_RETRY_DELAY_MS));
}

pub(crate) fn remove_dir_all_with_retry(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    let mut last_err = None;
    for _ in 0..FILE_OP_RETRY_TIMES {
        match std::fs::remove_dir_all(path) {
            Ok(_) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                sleep_retry();
            }
        }
    }

    Err(format!(
        "清理目录失败: {} ({})",
        path.to_string_lossy(),
        last_err
            .map(|e| e.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    ))
}

fn remove_file_with_retry(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    let mut last_err = None;
    for _ in 0..FILE_OP_RETRY_TIMES {
        match std::fs::remove_file(path) {
            Ok(_) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                sleep_retry();
            }
        }
    }

    Err(format!(
        "删除文件失败: {} ({})",
        path.to_string_lossy(),
        last_err
            .map(|e| e.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    ))
}

fn rename_with_retry(src: &Path, dst: &Path) -> Result<(), String> {
    let mut last_err = None;
    for _ in 0..FILE_OP_RETRY_TIMES {
        match std::fs::rename(src, dst) {
            Ok(_) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                sleep_retry();
            }
        }
    }

    Err(format!(
        "重命名失败: {} -> {} ({})",
        src.to_string_lossy(),
        dst.to_string_lossy(),
        last_err
            .map(|e| e.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    ))
}

fn utils_root() -> Result<PathBuf, String> {
    Ok(exe_dir()?.join("utils"))
}

fn dir_has_entries(path: &Path) -> bool {
    std::fs::read_dir(path)
        .ok()
        .and_then(|mut entries| entries.next())
        .is_some()
}

fn dev_source_utils_root() -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }

    // During `tauri dev`, the executable lives under Cargo target directories.
    // Walking upward lets us find the crate root and reuse `src-tauri/utils`
    // directly instead of syncing a second full copy for every debug run.
    let exe_dir = exe_dir().ok()?;
    for ancestor in exe_dir.ancestors() {
        let cargo_toml = ancestor.join("Cargo.toml");
        let utils_dir = ancestor.join("utils");
        if cargo_toml.is_file() && utils_dir.is_dir() {
            return Some(utils_dir);
        }
    }

    None
}

fn bundles_root(target_root: &Path) -> PathBuf {
    target_root.join(utils_zip_support::bundle_dir_name())
}

fn marker_path(target_root: &Path) -> PathBuf {
    target_root
        .parent()
        .unwrap_or(target_root)
        .join(EXTRACT_MARKER)
}

fn legacy_marker_path(target_root: &Path) -> PathBuf {
    target_root.join(EXTRACT_MARKER)
}

fn temp_extract_prefix() -> &'static str {
    "utils.__extracting_"
}

fn path_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

fn bundle_files(bundle_root: &Path) -> Result<Vec<PathBuf>, String> {
    if !bundle_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut bundles = std::fs::read_dir(bundle_root)
        .map_err(|e| format!("读取资源包目录失败: {} ({e})", bundle_root.to_string_lossy()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.ends_with(utils_zip_support::bundle_extension()))
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    bundles.sort();
    Ok(bundles)
}

fn latest_bundle_mtime(bundle_root: &Path) -> Option<SystemTime> {
    bundle_files(bundle_root)
        .ok()
        .and_then(|bundles| bundles.into_iter().filter_map(|path| path_mtime(&path)).max())
}

fn dir_ready(target_root: &Path, bundle_root: &Path) -> bool {
    let marker = marker_path(target_root);
    if !marker.is_file() {
        return false;
    }
    match (latest_bundle_mtime(bundle_root), path_mtime(&marker)) {
        (Some(bundle_ts), Some(marker_ts)) => marker_ts >= bundle_ts,
        (None, _) => true,
        _ => false,
    }
}

fn move_dir_contents_with_retry(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("创建目标目录失败: {} ({e})", dst.to_string_lossy()))?;

    let mut entries = std::fs::read_dir(src)
        .map_err(|e| format!("读取目录失败: {} ({e})", src.to_string_lossy()))?
        .flatten()
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let src_path = entry.path();
        let name = entry.file_name();
        let dst_path = dst.join(name);

        if src_path.is_dir() {
            move_dir_contents_with_retry(&src_path, &dst_path)?;
            remove_dir_all_with_retry(&src_path)?;
            continue;
        }

        if dst_path.exists() {
            if dst_path.is_dir() {
                remove_dir_all_with_retry(&dst_path)?;
            } else {
                remove_file_with_retry(&dst_path)?;
            }
        }
        rename_with_retry(&src_path, &dst_path)?;
    }

    Ok(())
}

fn cleanup_or_recover_stale_temp_dirs(target_root: &Path) -> Result<bool, String> {
    let parent = target_root.parent().ok_or_else(|| {
        format!(
            "无法解析 utils 目标目录的父目录: {}",
            target_root.to_string_lossy()
        )
    })?;
    let mut recovered = false;

    let entries = std::fs::read_dir(parent).map_err(|e| {
        format!(
            "读取资源父目录失败: {} ({e})",
            parent.to_string_lossy()
        )
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with(temp_extract_prefix()) {
            continue;
        }

        let payload = path.join(PAYLOAD_DIR);
        let marker = path.join(TEMP_EXTRACT_MARKER);
        // If the temporary payload is complete, extraction itself already
        // succeeded and only the final move into `utils` was interrupted.
        if payload.is_dir() && marker.is_file() {
            info!(
                "[BundledResource] recovering stale temp dir: src={} dst={}",
                payload.to_string_lossy(),
                target_root.to_string_lossy()
            );
            move_dir_contents_with_retry(&payload, target_root)?;
            recovered = true;
        }

        if path.exists() {
            let _ = remove_dir_all_with_retry(&path);
        }
    }

    Ok(recovered)
}

fn extract_bundle_archives(bundle_root: &Path, target_root: &Path) -> Result<(), String> {
    let parent = target_root.parent().ok_or_else(|| {
        format!(
            "无法解析 utils 目标目录的父目录: {}",
            target_root.to_string_lossy()
        )
    })?;
    let temp_root = parent.join(format!("{}{}", temp_extract_prefix(), std::process::id()));
    let payload_dir = temp_root.join(PAYLOAD_DIR);

    if temp_root.exists() {
        remove_dir_all_with_retry(&temp_root)?;
    }
    std::fs::create_dir_all(&payload_dir).map_err(|e| {
        format!(
            "创建资源临时目录失败: {} ({e})",
            payload_dir.to_string_lossy()
        )
    })?;

    // Expand bundles into a private temp location first so the public `utils`
    // directory never becomes half-written during install or recovery.
    utils_zip_support::extract_bundle_archives_to_destination(bundle_root, &payload_dir)?;

    std::fs::write(temp_root.join(TEMP_EXTRACT_MARKER), b"ok").map_err(|e| {
        format!(
            "写入解压完成标记失败: {} ({e})",
            temp_root.join(TEMP_EXTRACT_MARKER).to_string_lossy()
        )
    })?;

    std::fs::create_dir_all(target_root)
        .map_err(|e| format!("创建 utils 目录失败: {} ({e})", target_root.to_string_lossy()))?;
    move_dir_contents_with_retry(&payload_dir, target_root)?;
    std::fs::write(marker_path(target_root), b"ok").map_err(|e| {
        format!(
            "写入 utils 完成标记失败: {} ({e})",
            marker_path(target_root).to_string_lossy()
        )
    })?;
    let _ = remove_dir_all_with_retry(&temp_root);
    Ok(())
}

fn ensure_utils_ready_internal() -> Result<(PathBuf, EnsureDirOutcome), String> {
    let target_root = utils_root()?;
    let mut outcome = EnsureDirOutcome::default();

    std::fs::create_dir_all(&target_root)
        .map_err(|e| format!("创建 utils 目录失败: {} ({e})", target_root.to_string_lossy()))?;
    let legacy_marker = legacy_marker_path(&target_root);
    if legacy_marker.is_file() {
        let _ = remove_file_with_retry(&legacy_marker);
    }

    if cleanup_or_recover_stale_temp_dirs(&target_root)? {
        outcome.recovered = true;
    }

    let bundle_root = bundles_root(&target_root);
    let bundles = bundle_files(&bundle_root)?;
    if bundles.is_empty() {
        // Debug/dev usually lands here because no external bundles are shipped.
        if dir_has_entries(&target_root) {
            outcome.skipped = true;
            return Ok((target_root, outcome));
        }
        if let Some(dev_utils_root) = dev_source_utils_root() {
            outcome.skipped = true;
            return Ok((dev_utils_root, outcome));
        }
        outcome.skipped = true;
        return Ok((target_root, outcome));
    }

    // Fast path for a healthy release installation.
    if dir_ready(&target_root, &bundle_root) {
        return Ok((target_root, outcome));
    }

    let _guard = extract_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if cleanup_or_recover_stale_temp_dirs(&target_root)? {
        outcome.recovered = true;
    }
    if dir_ready(&target_root, &bundle_root) {
        return Ok((target_root, outcome));
    }

    // Release fallback: rebuild `utils` from shipped external bundle parts.
    info!(
        "[BundledResource] extracting bundleRoot={} dst={}",
        bundle_root.to_string_lossy(),
        target_root.to_string_lossy()
    );
    extract_bundle_archives(&bundle_root, &target_root)?;
    outcome.extracted = true;
    Ok((target_root, outcome))
}

pub fn ensure_relative_dir(relative_dir: &str) -> Result<PathBuf, String> {
    let normalized = relative_dir.replace('\\', "/");
    // Centralize all `utils/...` path resolution here so runtime commands do
    // not need separate debug/release path logic.
    if normalized == "utils" || normalized.starts_with("utils/") {
        let (root, _) = ensure_utils_ready_internal()?;
        if normalized == "utils" {
            return Ok(root);
        }
        let suffix = normalized.trim_start_matches("utils/").replace('/', std::path::MAIN_SEPARATOR_STR);
        let resolved = root.join(&suffix);

        // Dev/调试回退：
        // - `target/debug/utils` 经常已经存在并含有运行期生成的子目录（python_runtime 等），
        //   于是 ensure_utils_ready_internal 会直接返回它、不再回退到源码树。
        // - 但 pandoc.exe / tectonic 这类随源码走的工具只存在于 `src-tauri/utils`，
        //   导致导出 docx/tex/epub 等报 “pandoc executable not found at .../target/debug/utils/...”。
        // - 因此当解析到的文件在 target 下并不存在时，回退到源码树的同名文件。
        #[cfg(debug_assertions)]
        {
            if !resolved.exists() {
                if let Some(dev_root) = dev_source_utils_root() {
                    let dev_resolved = dev_root.join(&suffix);
                    if dev_resolved.exists() {
                        return Ok(dev_resolved);
                    }
                }
            }
        }

        return Ok(resolved);
    }

    Ok(exe_dir()?.join(relative_dir))
}
