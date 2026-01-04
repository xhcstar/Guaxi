use crate::commands::repoview::{add_resource, ResourceInput};
use crate::commands::settingview::get_image_bed_root;
use chrono::Local;
use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};
use urlencoding::encode;

#[derive(Debug, Serialize)]
pub struct ScreenshotMarkdownResult {
    pub relative_path: String,
    pub absolute_path: String,
    pub url: String,
    pub markdown: String,
}

#[tauri::command]
pub async fn trigger_screenshot_tool_once() -> Result<(), String> {
    info!("[screenshot] trigger_screenshot_tool_once called");

    let current_exe =
        env::current_exe().map_err(|e| format!("Failed to get current executable path: {}", e))?;
    let utils_path = current_exe
        .parent()
        .ok_or("Failed to get parent directory of the executable")?
        .join("utils")
        .join("ScreenShotTool.exe");

    info!(utils_path = %utils_path.display(), "[screenshot] ScreenShotTool path");

    if !utils_path.exists() {
        error!(utils_path = %utils_path.display(), "[screenshot] ScreenShotTool.exe not found");
        return Err(format!(
            "ScreenShotTool.exe not found at path: {}",
            utils_path.display()
        ));
    }

    info!("[screenshot] spawning ScreenShotTool with --once");
    let start = Instant::now();
    let output = Command::new(&utils_path)
        .arg("--once")
        .output()
        .map_err(|e| format!("Failed to execute ScreenShotTool.exe: {}", e))?;

    info!(
        status = ?output.status.code(),
        stdout_len = output.stdout.len(),
        stderr_len = output.stderr.len(),
        elapsed_ms = start.elapsed().as_millis(),
        "[screenshot] ScreenShotTool finished (--once)"
    );

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("ScreenShotTool.exe failed with error: {}", stderr))
    }
}

#[tauri::command]
pub async fn take_screenshot(path: String) -> Result<String, String> {
    info!("[screenshot] take_screenshot called");
    if path.trim().is_empty() {
        warn!("[screenshot] take_screenshot called with empty path");
    }
    info!(path = %path, "[screenshot] target path");

    let current_exe =
        env::current_exe().map_err(|e| format!("Failed to get current executable path: {}", e))?;
    info!(current_exe = %current_exe.display(), "[screenshot] current_exe");

    let utils_path = current_exe
        .parent()
        .ok_or("Failed to get parent directory of the executable")?
        .join("utils")
        .join("ScreenShotTool.exe");
    info!(utils_path = %utils_path.display(), "[screenshot] ScreenShotTool path");

    if !utils_path.exists() {
        error!(utils_path = %utils_path.display(), "[screenshot] ScreenShotTool.exe not found");
        return Err(format!(
            "ScreenShotTool.exe not found at path: {}",
            utils_path.display()
        ));
    }

    info!("[screenshot] spawning ScreenShotTool");
    let start = Instant::now();
    let output = Command::new(&utils_path)
        .arg(&path)
        .output()
        .map_err(|e| format!("Failed to execute ScreenShotTool.exe: {}", e))?;

    info!(
        status = ?output.status.code(),
        stdout_len = output.stdout.len(),
        stderr_len = output.stderr.len(),
        elapsed_ms = start.elapsed().as_millis(),
        "[screenshot] ScreenShotTool finished"
    );

    if output.status.success() {
        info!("[screenshot] screenshot tool returned success");
        Ok(format!("Screenshot saved to: {}", path))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!(
            status = ?output.status.code(),
            stderr = %stderr,
            "[screenshot] screenshot tool returned failure"
        );
        Err(format!("ScreenShotTool.exe failed with error: {}", stderr))
    }
}

#[allow(unused_assignments)]
#[tauri::command]
pub async fn take_screenshot_markdown(
    owner_file_path: String,
    alt: Option<String>,
) -> Result<ScreenshotMarkdownResult, String> {
    info!(
        owner_file_path = %owner_file_path,
        alt = ?alt,
        "[screenshot] take_screenshot_markdown called"
    );

    let now = Local::now();
    let root = PathBuf::from(get_image_bed_root());
    info!(root = %root.display(), "[screenshot] image bed root");

    let relative_path = format!(
        "images/{}/{}/{}/screenshot_{}.png",
        now.format("%Y"),
        now.format("%m"),
        now.format("%d"),
        now.format("%Y%m%d_%H%M%S_%3f")
    )
    .replace('\\', "/");

    let absolute_path = root.join(&relative_path);
    info!(
        relative_path = %relative_path,
        absolute_path = %absolute_path.display(),
        "[screenshot] generated paths"
    );

    if let Some(parent) = absolute_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
        info!(dir = %parent.display(), "[screenshot] ensured parent dir");
    }

    let absolute_str = absolute_path.to_string_lossy().to_string();

    info!("[screenshot] spawning screenshot tool (non-blocking)");
    let current_exe =
        env::current_exe().map_err(|e| format!("Failed to get current executable path: {}", e))?;
    let utils_path = current_exe
        .parent()
        .ok_or("Failed to get parent directory of the executable")?
        .join("utils")
        .join("ScreenShotTool.exe");

    info!(utils_path = %utils_path.display(), "[screenshot] ScreenShotTool path");

    if !utils_path.exists() {
        error!(utils_path = %utils_path.display(), "[screenshot] ScreenShotTool.exe not found");
        return Err(format!(
            "ScreenShotTool.exe not found at path: {}",
            utils_path.display()
        ));
    }

    let mut child = Command::new(&utils_path)
        .args(["--save", &absolute_str, "--once"])
        .spawn()
        .map_err(|e| format!("Failed to spawn ScreenShotTool.exe: {}", e))?;

    info!(pid = ?child.id(), "[screenshot] ScreenShotTool spawned");

    let poll_start = Instant::now();
    let max_wait = Duration::from_secs(60 * 30);
    let mut exists = false;
    let mut last_len: Option<u64> = None;

    loop {
        match std::fs::metadata(&absolute_path) {
            Ok(m) if m.is_file() && m.len() > 0 => {
                exists = true;
                last_len = Some(m.len());
                break;
            }
            Ok(m) => {
                last_len = Some(m.len());
            }
            Err(_) => {}
        }

        if let Ok(Some(status)) = child.try_wait() {
            warn!(
                status = ?status.code(),
                "[screenshot] ScreenShotTool exited before screenshot file was created"
            );
            return Err(format!(
                "ScreenShotTool exited before creating screenshot file: {} (status: {:?})",
                absolute_str,
                status.code()
            ));
        }

        if poll_start.elapsed() >= max_wait {
            warn!(
                elapsed_ms = poll_start.elapsed().as_millis(),
                "[screenshot] waiting for screenshot file timed out"
            );
            let _ = child.kill();
            return Err(format!(
                "Timed out waiting for screenshot file: {}",
                absolute_str
            ));
        }

        sleep(Duration::from_millis(200)).await;
    }

    info!(
        exists,
        last_len = ?last_len,
        elapsed_ms = poll_start.elapsed().as_millis(),
        "[screenshot] file existence poll finished"
    );

    if !exists {
        error!(absolute_path = %absolute_str, "[screenshot] screenshot file not created");
        return Err(format!("Screenshot file not created: {}", absolute_str));
    }

    let file_name = Path::new(&absolute_str)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid screenshot file name")?
        .to_string();

    let created_at = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let extra = Some(r#"{"source":"screenshot"}"#.to_string());

    info!(
        file_name = %file_name,
        created_at = %created_at,
        "[screenshot] preparing resource registration"
    );

    let input = ResourceInput::new(
        "image".to_string(),
        file_name.clone(),
        relative_path.clone(),
        absolute_str.clone(),
        owner_file_path,
        created_at,
        extra,
    );

    add_resource(input)?;
    info!("[screenshot] resource registered");

    let url = format!(
        "http://localhost:8888/markdown/api/bed/{}",
        encode(&relative_path)
    );

    let alt_text = alt.unwrap_or_else(|| "screenshot".to_string());
    let markdown = format!("![{}]({})", alt_text, url);

    info!(url = %url, "[screenshot] returning markdown result");

    Ok(ScreenshotMarkdownResult {
        relative_path,
        absolute_path: absolute_str,
        url,
        markdown,
    })
}

#[tauri::command]
pub fn create_screenshot_dir(path: String) -> Result<(), String> {
    info!("Creating screenshot directory: {}", path);
    std::fs::create_dir_all(&path).map_err(|e| format!("创建目录失败: {}", e))
}