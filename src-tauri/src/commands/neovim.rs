use std::env;
use std::path::PathBuf;
use std::process::Command;

fn normalize_target_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Neovim 打开失败：路径为空".to_string());
    }

    let target = PathBuf::from(trimmed);
    if !target.exists() {
        return Err(format!("Neovim 打开失败：路径不存在 {}", trimmed));
    }

    Ok(target)
}

#[cfg(target_os = "windows")]
fn spawn_neovim(executable: &str, target: &PathBuf) -> Result<(), String> {
    let target_arg = target.to_string_lossy().to_string();
    println!(
        "[NeovimBridge] launching on Windows executable={} target={}",
        executable, target_arg
    );

    Command::new("cmd")
        .args(["/C", "start", "", executable, &target_arg])
        .spawn()
        .map_err(|err| format!("启动 Neovim 失败: {}", err))?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn spawn_neovim(executable: &str, target: &PathBuf) -> Result<(), String> {
    println!(
        "[NeovimBridge] launching executable={} target={}",
        executable,
        target.display()
    );

    Command::new(executable)
        .arg(target)
        .spawn()
        .map_err(|err| format!("启动 Neovim 失败: {}", err))?;

    Ok(())
}

#[tauri::command]
pub fn neovim_open_path(path: String, is_directory: Option<bool>) -> Result<(), String> {
    let target = normalize_target_path(&path)?;
    let target_is_dir = target.is_dir();
    let expected_dir = is_directory.unwrap_or(target_is_dir);

    // 设计思路：
    // 1. Guaxi 只负责唤醒外部 Neovim，并在前端 store 中切换文件写权；具体编辑体验交给用户已有 Neovim 配置。
    // 2. 文件和目录都直接作为 nvim 参数传入，目录由用户的 netrw/oil/telescope 等配置处理。
    // 3. 使用 GUAXI_NVIM 允许用户把启动器换成 neovide、wt 包装脚本或自己的 nvim wrapper。
    println!(
        "[NeovimBridge] open request path={} is_directory={} target_is_dir={}",
        target.display(),
        expected_dir,
        target_is_dir
    );

    let executable = env::var("GUAXI_NVIM")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "nvim".to_string());

    spawn_neovim(&executable, &target)
}
