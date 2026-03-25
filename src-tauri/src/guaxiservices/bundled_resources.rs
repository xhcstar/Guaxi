use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{info, warn};

static EXTRACT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

const EXTRACT_MARKER: &str = ".guaxi_extract_complete";
const PAYLOAD_DIR: &str = "__payload";
const FILE_OP_RETRY_TIMES: usize = 40;
const FILE_OP_RETRY_DELAY_MS: u64 = 250;

#[derive(Debug, Clone, Serialize)]
pub struct StartupPrewarmReport {
    pub total: usize,
    pub ready: usize,
    pub recovered: Vec<String>,
    pub extracted: Vec<String>,
    pub failed: Vec<String>,
    pub skipped: Vec<String>,
    pub finished: bool,
    pub summary: String,
}

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

fn zip_path_for_dir(dir: &Path) -> PathBuf {
    dir.with_extension("zip")
}

fn temp_dir_prefix(target_dir: &Path) -> Result<String, String> {
    Ok(format!(
        "{}.__extracting_",
        target_dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("无法解析目录名: {}", target_dir.to_string_lossy()))?
    ))
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

fn cleanup_or_recover_stale_temp_dirs(target_dir: &Path) -> Result<bool, String> {
    let parent = target_dir.parent().ok_or_else(|| {
        format!(
            "无法解析资源目标目录的父目录: {}",
            target_dir.to_string_lossy()
        )
    })?;
    let prefix = temp_dir_prefix(target_dir)?;
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
        if !name.starts_with(&prefix) {
            continue;
        }

        let payload = path.join(PAYLOAD_DIR);
        let marker = path.join(EXTRACT_MARKER);
        if !target_dir.exists() && payload.is_dir() && marker.is_file() {
            info!(
                "[BundledResource] recovering stale temp dir: src={} dst={}",
                payload.to_string_lossy(),
                target_dir.to_string_lossy()
            );
            rename_with_retry(&payload, target_dir)?;
            recovered = true;
        }

        if path.exists() {
            let _ = remove_dir_all_with_retry(&path);
        }
    }

    Ok(recovered)
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
    let payload_dir = temp_dir.join(PAYLOAD_DIR);

    if temp_dir.exists() {
        remove_dir_all_with_retry(&temp_dir)?;
    }
    std::fs::create_dir_all(&payload_dir).map_err(|e| {
        format!(
            "创建资源临时目录失败: {} ({e})",
            payload_dir.to_string_lossy()
        )
    })?;

    {
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
            let out_path = payload_dir.join(enclosed);

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
    }

    std::fs::write(temp_dir.join(EXTRACT_MARKER), b"ok").map_err(|e| {
        format!(
            "写入解压完成标记失败: {} ({e})",
            temp_dir.join(EXTRACT_MARKER).to_string_lossy()
        )
    })?;

    if target_dir.exists() {
        if target_dir.is_dir() {
            remove_dir_all_with_retry(target_dir)?;
        } else {
            return Err(format!(
                "资源目标路径已存在且不是目录: {}",
                target_dir.to_string_lossy()
            ));
        }
    }

    rename_with_retry(&payload_dir, target_dir)?;
    let _ = remove_dir_all_with_retry(&temp_dir);
    Ok(())
}

fn ensure_relative_dir_internal(relative_dir: &str) -> Result<(PathBuf, EnsureDirOutcome), String> {
    let target_dir = exe_dir()?.join(relative_dir);
    let mut outcome = EnsureDirOutcome::default();

    if cleanup_or_recover_stale_temp_dirs(&target_dir)? {
        outcome.recovered = true;
    }

    if target_dir.is_dir() {
        return Ok((target_dir, outcome));
    }
    if target_dir.exists() {
        return Err(format!(
            "资源目标路径已存在但不是目录: {}",
            target_dir.to_string_lossy()
        ));
    }

    let bundle = zip_path_for_dir(&target_dir);
    if !bundle.is_file() {
        outcome.skipped = true;
        return Ok((target_dir, outcome));
    }

    let _guard = extract_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if cleanup_or_recover_stale_temp_dirs(&target_dir)? {
        outcome.recovered = true;
    }
    if target_dir.is_dir() {
        return Ok((target_dir, outcome));
    }

    info!(
        "[BundledResource] extracting zip={} dst={}",
        bundle.to_string_lossy(),
        target_dir.to_string_lossy()
    );
    extract_dir_from_zip(&bundle, &target_dir)?;
    outcome.extracted = true;
    Ok((target_dir, outcome))
}

pub fn ensure_relative_dir(relative_dir: &str) -> Result<PathBuf, String> {
    ensure_relative_dir_internal(relative_dir).map(|(path, _)| path)
}

pub fn prewarm_startup_bundles() -> StartupPrewarmReport {
    let targets = [
        "utils/sherpa-onnx",
        "utils/Models/tts",
        "utils/whisper",
        "utils/Models/whisper_models",
        "utils/llama",
        "utils/python_libs",
        "utils/SlidevEnv",
    ];

    let mut ready = 0usize;
    let mut recovered = Vec::new();
    let mut extracted = Vec::new();
    let mut failed = Vec::new();
    let mut skipped = Vec::new();

    for relative_dir in targets {
        let start = Instant::now();
        match ensure_relative_dir_internal(relative_dir) {
            Ok((path, outcome)) => {
                ready += 1;
                if outcome.recovered {
                    recovered.push(relative_dir.to_string());
                }
                if outcome.extracted {
                    extracted.push(relative_dir.to_string());
                }
                if outcome.skipped {
                    skipped.push(relative_dir.to_string());
                }
                info!(
                    "[BundledResource] startup prewarm ready: dir={} path={} elapsedMs={} recovered={} extracted={} skipped={}",
                    relative_dir,
                    path.to_string_lossy(),
                    start.elapsed().as_millis(),
                    outcome.recovered,
                    outcome.extracted,
                    outcome.skipped
                );
            }
            Err(e) => {
                failed.push(format!("{relative_dir}: {e}"));
                warn!(
                    "[BundledResource] startup prewarm failed: dir={} elapsedMs={} error={}",
                    relative_dir,
                    start.elapsed().as_millis(),
                    e
                );
            }
        }
    }

    let summary = if !failed.is_empty() {
        format!(
            "启动时已尝试继续准备运行资源，成功 {ready}/{}，失败 {} 项。",
            targets.len(),
            failed.len()
        )
    } else if !recovered.is_empty() {
        format!(
            "已恢复上次未完成的资源解压，并完成运行环境准备（{} 项恢复）。",
            recovered.len()
        )
    } else if !extracted.is_empty() {
        format!("首次启动已在后台完成运行环境准备（{} 项）。", extracted.len())
    } else {
        "运行环境已就绪。".to_string()
    };

    let finished = failed.is_empty();
    StartupPrewarmReport {
        total: targets.len(),
        ready,
        recovered,
        extracted,
        failed,
        skipped,
        finished,
        summary,
    }
}
