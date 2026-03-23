use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use tracing::{info, warn};

static EXTRACT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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

fn zip_path_for_dir(dir: &Path) -> PathBuf {
    dir.with_extension("zip")
}

fn extract_dir_from_zip(bundle: &Path, target_dir: &Path) -> Result<(), String> {
    let parent = target_dir.parent().ok_or_else(|| {
        format!(
            "无法解析资源目标目录的父目录: {}",
            target_dir.to_string_lossy()
        )
    })?;
    let temp_dir = parent.join(format!(
        "{}.__extracting_{}",
        target_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("resource"),
        std::process::id()
    ));

    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).map_err(|e| {
            format!(
                "清理旧的资源临时目录失败: {} ({e})",
                temp_dir.to_string_lossy()
            )
        })?;
    }
    std::fs::create_dir_all(&temp_dir).map_err(|e| {
        format!(
            "创建资源临时目录失败: {} ({e})",
            temp_dir.to_string_lossy()
        )
    })?;

    let file = std::fs::File::open(bundle)
        .map_err(|e| format!("打开资源压缩包失败: {} ({e})", bundle.to_string_lossy()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("读取资源压缩包失败: {} ({e})", bundle.to_string_lossy()))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("读取资源压缩条目失败: {e}"))?;
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| format!("资源压缩包包含非法路径: {}", entry.name()))?
            .to_path_buf();
        let out_path = temp_dir.join(enclosed);

        if entry.name().ends_with('/') {
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

    let nested = temp_dir.join(
        target_dir
            .file_name()
            .ok_or_else(|| format!("无法解析目录名: {}", target_dir.to_string_lossy()))?,
    );
    let extracted_root = if nested.is_dir() { nested } else { temp_dir.clone() };

    if target_dir.exists() {
        if target_dir.is_dir() {
            std::fs::remove_dir_all(target_dir).map_err(|e| {
                format!(
                    "清理旧的资源目录失败: {} ({e})",
                    target_dir.to_string_lossy()
                )
            })?;
        } else {
            return Err(format!(
                "资源目标路径已存在且不是目录: {}",
                target_dir.to_string_lossy()
            ));
        }
    }

    if extracted_root == temp_dir {
        std::fs::rename(&temp_dir, target_dir).map_err(|e| {
            format!(
                "落地资源目录失败: {} -> {} ({e})",
                temp_dir.to_string_lossy(),
                target_dir.to_string_lossy()
            )
        })?;
    } else {
        std::fs::rename(&extracted_root, target_dir).map_err(|e| {
            format!(
                "落地资源目录失败: {} -> {} ({e})",
                extracted_root.to_string_lossy(),
                target_dir.to_string_lossy()
            )
        })?;
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    Ok(())
}

pub fn ensure_relative_dir(relative_dir: &str) -> Result<PathBuf, String> {
    let target_dir = exe_dir()?.join(relative_dir);
    if target_dir.is_dir() {
        return Ok(target_dir);
    }
    if target_dir.exists() {
        return Err(format!(
            "资源目标路径已存在但不是目录: {}",
            target_dir.to_string_lossy()
        ));
    }

    let bundle = zip_path_for_dir(&target_dir);
    if !bundle.is_file() {
        return Ok(target_dir);
    }

    let _guard = extract_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if target_dir.is_dir() {
        return Ok(target_dir);
    }

    info!(
        "[BundledResource] extracting zip={} dst={}",
        bundle.to_string_lossy(),
        target_dir.to_string_lossy()
    );
    extract_dir_from_zip(&bundle, &target_dir)?;
    Ok(target_dir)
}

pub fn prewarm_startup_bundles() {
    for relative_dir in [
        "utils/sherpa-onnx",
        "utils/Models/tts",
        "utils/whisper",
        "utils/Models/whisper_models",
        "utils/llama",
        "utils/python_libs",
        "utils/SlidevEnv",
    ] {
        let start = Instant::now();
        match ensure_relative_dir(relative_dir) {
            Ok(path) => {
                info!(
                    "[BundledResource] startup prewarm ready: dir={} path={} elapsedMs={}",
                    relative_dir,
                    path.to_string_lossy(),
                    start.elapsed().as_millis()
                );
            }
            Err(e) => {
                warn!(
                    "[BundledResource] startup prewarm failed: dir={} elapsedMs={} error={}",
                    relative_dir,
                    start.elapsed().as_millis(),
                    e
                );
            }
        }
    }
}
