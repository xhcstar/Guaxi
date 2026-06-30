use std::env;
use std::path::PathBuf;
use std::process::{Child, Command};
use tauri::Emitter;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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

/// Windows：通过隐藏的 cmd 启动器执行 `start /WAIT`。
/// 设计思路：
/// 1. 前端仍然通过同一个 `neovim_open_path` 命令传入文件或目录，业务行为不变。
/// 2. 终端版 nvim 需要可见的控制台窗口，所以不能把 nvim.exe 本身用 `CREATE_NO_WINDOW` 直接启动。
/// 3. `CREATE_NO_WINDOW` 只隐藏中间的 cmd 启动器，`start /WAIT` 打开的 Neovim 编辑窗口仍然可见。
/// 4. Rust 侧继续 wait 这个 cmd 启动器；因为 `/WAIT` 会等 Neovim 退出，所以 `neovim-exited`
///    的关闭事件处理链保持不变。
#[cfg(target_os = "windows")]
fn spawn_neovim(executable: &str, target: &PathBuf) -> Result<Child, String> {
    let target_arg = target.to_string_lossy().to_string();
    let work_dir = resolve_working_dir(target);
    println!(
        "[NeovimBridge] launching on Windows executable={} target={} work_dir={:?}",
        executable,
        target_arg,
        work_dir
    );

    let mut command = Command::new("cmd");
    if let Some(dir) = work_dir.as_ref() {
        command.current_dir(dir);
    }
    // 这里隐藏的是中间 cmd 启动器，不是 Neovim 窗口；/WAIT 保证关闭事件仍然可被后续 wait 捕获。
    command
        .creation_flags(CREATE_NO_WINDOW)
        .arg("/C")
        .arg("start")
        .arg("/WAIT")
        .arg("")
        .arg(executable)
        .arg(&target_arg);

    let child = command
        .spawn()
        .map_err(|err| format!("启动 Neovim 失败: {}", err))?;
    println!(
        "[NeovimBridge] Windows launcher pid={} is waiting for Neovim target={}",
        child.id(),
        target_arg
    );
    Ok(child)
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
