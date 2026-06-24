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

// 计算 Neovim 的工作目录：目录目标用其自身，文件目标用其所在目录。
// 这样无论从图标还是右键菜单打开，Neovim 的 cwd 都落在被打开的文件/目录所在路径，
// 而不是软件进程继承下来的安装目录。
fn resolve_working_dir(target: &PathBuf) -> Option<PathBuf> {
    if target.is_dir() {
        Some(target.clone())
    } else {
        target.parent().map(|parent| parent.to_path_buf())
    }
}

#[cfg(target_os = "windows")]
fn spawn_neovim(executable: &str, target: &PathBuf) -> Result<(), String> {
    let target_arg = target.to_string_lossy().to_string();
    let work_dir = resolve_working_dir(target);
    println!(
        "[NeovimBridge] launching on Windows executable={} target={} work_dir={:?}",
        executable, target_arg, work_dir
    );

    // 设计说明：保持与历史可用写法一致 `cmd /C start "" nvim <target>`，由 start 为 TUI 的
    // nvim 新建一个控制台窗口。工作目录只通过给 cmd 进程设置 current_dir 来传递——start 会让
    // 新进程继承调用方 cmd 的当前目录。之前尝试用 start 的 /D 选项反而因为路径是正斜杠而解析
    // 失败，导致 start 不启动 nvim（窗口不出现），故这里不再使用 /D。
    let mut command = Command::new("cmd");
    if let Some(dir) = work_dir.as_ref() {
        command.current_dir(dir);
    }
    command.arg("/C").arg("start").arg("").arg(executable).arg(&target_arg);

    command
        .spawn()
        .map_err(|err| format!("启动 Neovim 失败: {}", err))?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn spawn_neovim(executable: &str, target: &PathBuf) -> Result<(), String> {
    let work_dir = resolve_working_dir(target);
    println!(
        "[NeovimBridge] launching executable={} target={} work_dir={:?}",
        executable,
        target.display(),
        work_dir
    );

    let mut command = Command::new(executable);
    if let Some(dir) = work_dir.as_ref() {
        command.current_dir(dir);
    }
    command
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
