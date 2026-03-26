use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime};
use tracing::info;

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

fn remove_dir_all_with_retry(path: &Path) -> Result<(), String> {
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

fn utils_zip_path(target_root: &Path) -> PathBuf {
    target_root.join("utils.zip")
}

fn marker_path(target_root: &Path) -> PathBuf {
    target_root.join(EXTRACT_MARKER)
}

fn temp_extract_prefix() -> &'static str {
    "utils.__extracting_"
}

fn zip_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

fn dir_ready(target_root: &Path, bundle: &Path) -> bool {
    let marker = marker_path(target_root);
    if !marker.is_file() {
        return false;
    }
    match (zip_mtime(bundle), zip_mtime(&marker)) {
        (Some(bundle_ts), Some(marker_ts)) => marker_ts >= bundle_ts,
        _ => true,
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

fn extract_utils_zip(bundle: &Path, target_root: &Path) -> Result<(), String> {
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

    {
        let file = std::fs::File::open(bundle)
            .map_err(|e| format!("打开 utils.zip 失败: {} ({e})", bundle.to_string_lossy()))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("读取 utils.zip 失败: {} ({e})", bundle.to_string_lossy()))?;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("读取 utils.zip 条目失败: {e}"))?;
            let enclosed = entry
                .enclosed_name()
                .ok_or_else(|| format!("utils.zip 包含非法路径: {}", entry.name()))?
                .to_path_buf();
            let normalized = entry.name().replace('\\', "/");
            let out_path = payload_dir.join(&enclosed);
            let is_dir_entry =
                entry.is_dir() || normalized.ends_with('/') || enclosed.file_name().is_none();

            if is_dir_entry {
                std::fs::create_dir_all(&out_path).map_err(|e| {
                    format!("创建资源目录失败: {} ({e})", out_path.to_string_lossy())
                })?;
                continue;
            }

            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "创建资源文件父目录失败: {} ({e})",
                        parent.to_string_lossy()
                    )
                })?;
            }

            let mut out_file = std::fs::File::create(&out_path).map_err(|e| {
                format!(
                    "创建资源目标文件失败: {} ({e})",
                    out_path.to_string_lossy()
                )
            })?;
            std::io::copy(&mut entry, &mut out_file).map_err(|e| {
                format!(
                    "写入资源目标文件失败: {} ({e})",
                    out_path.to_string_lossy()
                )
            })?;
        }
    }

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

    if cleanup_or_recover_stale_temp_dirs(&target_root)? {
        outcome.recovered = true;
    }

    let bundle = utils_zip_path(&target_root);
    if !bundle.is_file() {
        outcome.skipped = true;
        return Ok((target_root, outcome));
    }

    if dir_ready(&target_root, &bundle) {
        return Ok((target_root, outcome));
    }

    let _guard = extract_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if cleanup_or_recover_stale_temp_dirs(&target_root)? {
        outcome.recovered = true;
    }
    if dir_ready(&target_root, &bundle) {
        return Ok((target_root, outcome));
    }

    info!(
        "[BundledResource] extracting zip={} dst={}",
        bundle.to_string_lossy(),
        target_root.to_string_lossy()
    );
    extract_utils_zip(&bundle, &target_root)?;
    outcome.extracted = true;
    Ok((target_root, outcome))
}

pub fn ensure_relative_dir(relative_dir: &str) -> Result<PathBuf, String> {
    let normalized = relative_dir.replace('\\', "/");
    if normalized == "utils" || normalized.starts_with("utils/") {
        let (root, _) = ensure_utils_ready_internal()?;
        if normalized == "utils" {
            return Ok(root);
        }
        let suffix = normalized.trim_start_matches("utils/").replace('/', std::path::MAIN_SEPARATOR_STR);
        return Ok(root.join(suffix));
    }

    Ok(exe_dir()?.join(relative_dir))
}
