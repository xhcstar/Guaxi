use std::env;
use std::path::PathBuf;
use std::process::{Child, Command};
use tauri::Emitter;

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

fn resolve_working_dir(target: &PathBuf) -> Option<PathBuf> {
    if target.is_dir() {
        Some(target.clone())
    } else {
        target.parent().map(|parent| parent.to_path_buf())
    }
}

/// Windows：通过 cmd /C start /WAIT 启动 nvim，这样 cmd 进程的生命周期与 nvim 绑定，
/// Rust 侧 wait 到 cmd 退出即表示 nvim 已关闭。
#[cfg(target_os = "windows")]
fn spawn_neovim(executable: &str, target: &PathBuf) -> Result<Child, String> {
    let target_arg = target.to_string_lossy().to_string();
    let work_dir = resolve_working_dir(target);
    println!(
        "[NeovimBridge] launching on Windows executable={} target={} work_dir={:?}",
        executable, target_arg, work_dir
    );

    let mut command = Command::new("cmd");
    if let Some(dir) = work_dir.as_ref() {
        command.current_dir(dir);
    }
    // /WAIT 让 start 等待 nvim 退出后才返回，从而使整个 cmd 进程与 nvim 同生命周期
    command
        .arg("/C")
        .arg("start")
        .arg("/WAIT")
        .arg("")
        .arg(executable)
        .arg(&target_arg);

    command
        .spawn()
        .map_err(|err| format!("启动 Neovim 失败: {}", err))
}

/// Linux / macOS：直接 spawn nvim，子进程句柄可直接 wait。
#[cfg(not(target_os = "windows"))]
fn spawn_neovim(executable: &str, target: &PathBuf) -> Result<Child, String> {
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
        .map_err(|err| format!("启动 Neovim 失败: {}", err))
}

#[tauri::command]
pub fn neovim_open_path(
    app_handle: tauri::AppHandle,
    path: String,
    is_directory: Option<bool>,
) -> Result<(), String> {
    let target = normalize_target_path(&path)?;
    let target_is_dir = target.is_dir();
    let expected_dir = is_directory.unwrap_or(target_is_dir);

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

    let mut child = spawn_neovim(&executable, &target)?;

    // 后台监控子进程退出，emit 事件通知前端
    let target_clone = path.clone();
    tauri::async_runtime::spawn(async move {
        match child.wait() {
            Ok(status) => {
                println!(
                    "[NeovimBridge] nvim exited with code {:?} for path={}",
                    status.code(),
                    target_clone
                );
            }
            Err(err) => {
                eprintln!(
                    "[NeovimBridge] nvim wait error for path={}: {}",
                    target_clone, err
                );
            }
        }
        // 无论正常退出还是异常，都通知前端
        let _ = app_handle.emit("neovim-exited", target_clone);
    });

    Ok(())
}
