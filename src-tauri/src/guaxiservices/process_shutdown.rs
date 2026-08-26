//! 后台子进程的统一收尾工具。
//!
//! 设计说明（对应“关闭软件后 llama.cpp / python 等后台进程残留”）：
//! 1. 之前的收尾代码写在 `main()` 里 `tauri::Builder::run(...)` 之后。tao 的事件循环
//!    永不返回（内部直接 `process::exit`），那段代码是死代码，任何子进程都不会被回收。
//!    现在改为在 `RunEvent::Exit` 回调里收尾，并由本模块提供统一的杀进程能力。
//! 2. `std::process::Child::kill()` 在 Windows 上只结束直接子进程，llama-server /
//!    python worker / node 这类进程往往还有孙进程（pip、matplotlib、esbuild 等），
//!    因此优先用 `taskkill /T /F` 结束整棵进程树，再用 `kill()` 兜底。
//! 3. 对于拿不到 `Child` 句柄的场景（异步任务内的临时子进程、chromiumoxide 实例等），
//!    提供一个按 PID 登记的注册表：登记时记录用途标签，进程正常结束时反注册，
//!    退出时统一清理。反注册是为了尽量缩小 PID 复用导致误杀的窗口。

use once_cell::sync::Lazy;
use std::process::{Child, Command};
use std::sync::Mutex;
use tracing::{info, warn};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 收尾阶段调用的 taskkill / kill 本身不应该再弹出控制台窗口。
fn hide_console_window(command: &mut Command) {
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
}

#[derive(Clone, Debug)]
struct ManagedProcess {
    label: String,
    pid: u32,
}

static MANAGED_PROCESSES: Lazy<Mutex<Vec<ManagedProcess>>> = Lazy::new(|| Mutex::new(Vec::new()));

/// 登记一个需要随主程序一起结束的后台进程。
///
/// 只用于拿不到 `Child` 句柄的场景；能长期持有句柄的模块应当自己实现 `shutdown_*`，
/// 那样不依赖 PID，也就没有 PID 复用的风险。
pub fn register_managed_pid(label: &str, pid: u32) {
    if pid == 0 {
        return;
    }
    let mut guard = match MANAGED_PROCESSES.lock() {
        Ok(guard) => guard,
        Err(_) => {
            warn!(label, pid, "[ProcessShutdown] 托管进程表加锁失败，跳过登记");
            return;
        }
    };
    guard.retain(|item| item.pid != pid);
    guard.push(ManagedProcess {
        label: label.to_string(),
        pid,
    });
    info!(
        label,
        pid,
        total = guard.len(),
        "[ProcessShutdown] 已登记后台进程"
    );
}

/// 进程已经自然结束时反注册，避免退出时对着一个可能被复用的 PID 开枪。
pub fn unregister_managed_pid(pid: u32) {
    if pid == 0 {
        return;
    }
    if let Ok(mut guard) = MANAGED_PROCESSES.lock() {
        guard.retain(|item| item.pid != pid);
    }
}

/// 结束 `pid` 所在的整棵进程树。返回是否成功发出终止指令。
#[cfg(windows)]
pub fn terminate_process_tree(pid: u32, label: &str) -> bool {
    if pid == 0 {
        return false;
    }
    let mut command = Command::new("taskkill");
    command.args(["/PID", &pid.to_string(), "/T", "/F"]);
    hide_console_window(&mut command);
    match command.output() {
        Ok(output) if output.status.success() => {
            info!(label, pid, "[ProcessShutdown] 已结束进程树");
            true
        }
        Ok(output) => {
            // 进程可能已经自己退出了，这属于正常情况，只记 debug 级别的原因。
            warn!(
                label,
                pid,
                status = %output.status,
                stderr = %String::from_utf8_lossy(&output.stderr).trim(),
                "[ProcessShutdown] taskkill 未成功（进程可能已退出）"
            );
            false
        }
        Err(error) => {
            warn!(label, pid, error = %error, "[ProcessShutdown] 调用 taskkill 失败");
            false
        }
    }
}

/// 结束 `pid` 所在的整棵进程树。返回是否成功发出终止指令。
///
/// 类 Unix 平台没有 `taskkill /T`，用 `pkill -P` 先收孙进程，再结束父进程；
/// 先 TERM 给一次优雅退出的机会，再 KILL 兜底。
#[cfg(not(windows))]
pub fn terminate_process_tree(pid: u32, label: &str) -> bool {
    if pid == 0 {
        return false;
    }
    let pid_text = pid.to_string();

    let mut kill_children = Command::new("pkill");
    kill_children.args(["-TERM", "-P", &pid_text]);
    let _ = kill_children.output();

    let mut term = Command::new("kill");
    term.args(["-TERM", &pid_text]);
    let term_ok = term.output().map(|out| out.status.success()).unwrap_or(false);

    std::thread::sleep(std::time::Duration::from_millis(120));

    let mut force = Command::new("kill");
    force.args(["-KILL", &pid_text]);
    let force_ok = force.output().map(|out| out.status.success()).unwrap_or(false);

    let mut force_children = Command::new("pkill");
    force_children.args(["-KILL", "-P", &pid_text]);
    let _ = force_children.output();

    if term_ok || force_ok {
        info!(label, pid, "[ProcessShutdown] 已结束进程树");
        true
    } else {
        warn!(label, pid, "[ProcessShutdown] kill 未成功（进程可能已退出）");
        false
    }
}

/// 结束一个仍持有句柄的子进程：先整棵树，再用句柄兜底，最后回收僵尸进程。
pub fn terminate_child(label: &str, child: &mut Child) {
    let pid = child.id();
    terminate_process_tree(pid, label);
    let _ = child.kill();

    // 回收僵尸进程，但必须有上限：如果因为权限等原因没能真正杀掉，
    // 阻塞式 wait() 会把关闭流程（或 stop 命令）卡死，所以改成有限次轮询。
    for _ in 0..20 {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => break,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(25)),
        }
    }
    unregister_managed_pid(pid);
}

/// 结束所有登记在册的后台进程，返回实际处理的数量。
pub fn shutdown_registered(reason: &str) -> usize {
    let pending = match MANAGED_PROCESSES.lock() {
        Ok(mut guard) => std::mem::take(&mut *guard),
        Err(_) => {
            warn!(reason, "[ProcessShutdown] 托管进程表加锁失败，跳过统一清理");
            return 0;
        }
    };

    let total = pending.len();
    for item in pending {
        terminate_process_tree(item.pid, &item.label);
    }
    if total > 0 {
        info!(reason, total, "[ProcessShutdown] 已清理登记的后台进程");
    }
    total
}
