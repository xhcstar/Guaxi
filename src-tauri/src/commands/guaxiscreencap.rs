use std::env;
use std::path::Path;
use std::process::Command;

use tracing::info;

#[tauri::command]
pub async fn take_screenshot(path: String) -> Result<String, String> {
    info!("Taking screenshot to path: {}", path);
    // 获取当前程序的执行路径
    let current_dir =
        env::current_exe().map_err(|e| format!("Failed to get current executable path: {}", e))?;

    // 构造 utils 目录下的 ScreenShotTool.exe 的路径
    let utils_dir = current_dir
        .parent()
        .ok_or("Failed to get parent directory of the executable")?
        .join("utils")
        .join("ScreenShotTool.exe");
    info!("utils_dir: {}", utils_dir.display());

    // 检查 ScreenShotTool.exe 是否存在
    if !utils_dir.exists() {
        return Err(format!(
            "ScreenShotTool.exe not found at path: {}",
            utils_dir.display()
        ));
    }

    // 调用 ScreenShotTool.exe 并传递 path 参数
    let output = Command::new(utils_dir)
        .arg(&path)
        .output()
        .map_err(|e| format!("Failed to execute ScreenShotTool.exe: {}", e))?;

    // 检查命令执行结果
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
pub fn create_screenshot_dir(path: String) -> Result<(), String> {
    info!("Creating screenshot directory: {}", path);
    std::fs::create_dir_all(&path)
        .map_err(|e| format!("创建目录失败: {}", e))
}