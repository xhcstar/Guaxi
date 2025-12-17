use crate::commands::repoview::{add_resource, ResourceInput};
use crate::commands::settingview::get_image_bed_root;
use chrono::Local;
use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;
use urlencoding::encode;

#[derive(Debug, Serialize)]
pub struct ScreenshotMarkdownResult {
    pub relative_path: String,
    pub absolute_path: String,
    pub url: String,
    pub markdown: String,
}

#[tauri::command]
pub async fn take_screenshot(path: String) -> Result<String, String> {
    info!("Taking screenshot to path: {}", path);
    let current_dir =
        env::current_exe().map_err(|e| format!("Failed to get current executable path: {}", e))?;

    let utils_dir = current_dir
        .parent()
        .ok_or("Failed to get parent directory of the executable")?
        .join("utils")
        .join("ScreenShotTool.exe");
    info!("utils_dir: {}", utils_dir.display());

    if !utils_dir.exists() {
        return Err(format!(
            "ScreenShotTool.exe not found at path: {}",
            utils_dir.display()
        ));
    }

    let output = Command::new(utils_dir)
        .arg(&path)
        .output()
        .map_err(|e| format!("Failed to execute ScreenShotTool.exe: {}", e))?;

    if output.status.success() {
        info!("Screenshot taken successfully");
        Ok(format!("Screenshot saved to: {}", path))
    } else {
        Err(format!(
            "ScreenShotTool.exe failed with error: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[tauri::command]
pub async fn take_screenshot_markdown(
    owner_file_path: String,
    alt: Option<String>,
) -> Result<ScreenshotMarkdownResult, String> {
    let now = Local::now();
    let root = PathBuf::from(get_image_bed_root());

    let relative_path = format!(
        "images/{}/{}/{}/screenshot_{}.png",
        now.format("%Y"),
        now.format("%m"),
        now.format("%d"),
        now.format("%Y%m%d_%H%M%S_%3f")
    )
    .replace('\\', "/");

    let absolute_path = root.join(&relative_path);

    if let Some(parent) = absolute_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
    }

    let absolute_str = absolute_path.to_string_lossy().to_string();
    take_screenshot(absolute_str.clone()).await?;

    let file_name = Path::new(&absolute_str)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid screenshot file name")?
        .to_string();

    let created_at = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let extra = Some(r#"{"source":"screenshot"}"#.to_string());

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

    let url = format!(
        "http://localhost:8888/markdown/api/bed/{}",
        encode(&relative_path)
    );

    let alt_text = alt.unwrap_or_else(|| "screenshot".to_string());
    let markdown = format!("![{}]({})", alt_text, url);

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