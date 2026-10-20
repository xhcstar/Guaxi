//! 远程工具台：通过库内 SSH 档案在 Linux 机器上执行用户自备的工具
//! （如自行编译的 Tongsuo/openssl），并支持本机 ↔ 远端文件传输。
//!
//! 这样 SM2/PQC、客户端/服务端联调这类依赖 Tongsuo 的能力不必在本机原生编译：
//! 用户只需要在目标机准备好工具，填工具路径即可。

use std::collections::HashMap;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use russh::ChannelMsg;
use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Manager};

use crate::commands::ssh_terminal::connect_authenticated;

pub(crate) fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteToolOpts {
    /// 远端工具路径（如 `/opt/tongsuo/bin/openssl`）。命令里可用 `{tool}` 引用。
    #[serde(default)]
    pub tool_path: Option<String>,
    /// 远端工作目录。
    #[serde(default)]
    pub workdir: Option<String>,
    /// 额外的环境变量，如 `LD_LIBRARY_PATH=/opt/tongsuo/lib`（多个用空格分隔）。
    #[serde(default)]
    pub env: Option<String>,
    /// 超时秒数，默认 120，范围 1..3600。
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteToolResult {
    /// 最终实际执行的命令（已代入工作目录/环境/工具路径）。
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    /// stdout + stderr 合并，便于直接展示。
    pub output: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
}

/// 组装最终命令：`cd <workdir> && env <env> <tool> <command>`。
///
/// 命令模板里的 `{tool}` 会被安全引用；没写占位符但填了工具路径时自动前置。
fn build_command(command: &str, tool_path: &str, workdir: &str, env: &str) -> String {
    let mut cmd = if command.contains("{tool}") {
        command.replace("{tool}", &shell_quote(tool_path))
    } else if !tool_path.trim().is_empty() {
        format!("{} {command}", shell_quote(tool_path))
    } else {
        command.to_string()
    };
    if !env.trim().is_empty() {
        // 在独立 sh 里执行，避免 env 赋值被当成命令的一部分。
        cmd = format!("env {env} sh -c {}", shell_quote(&cmd));
    }
    if !workdir.trim().is_empty() {
        cmd = format!("cd {} && {cmd}", shell_quote(workdir));
    }
    cmd
}

/// 在指定 SSH 档案上执行一条命令，返回 stdout/stderr/退出码/耗时。
#[command]
pub async fn remote_tool_exec(
    app: AppHandle,
    profile_id: String,
    command: String,
    opts: Option<RemoteToolOpts>,
) -> Result<RemoteToolResult, String> {
    let o = opts.unwrap_or_default();
    run_remote_command(
        &app,
        &profile_id,
        &command,
        o.tool_path.as_deref().unwrap_or(""),
        o.workdir.as_deref().unwrap_or(""),
        o.env.as_deref().unwrap_or(""),
        o.timeout_secs.unwrap_or(120),
    )
    .await
}

/// 供其它命令复用的远程执行入口（不做 Tauri 参数转换）。
pub(crate) async fn run_remote_command(
    app: &AppHandle,
    profile_id: &str,
    command: &str,
    tool_path: &str,
    workdir: &str,
    env: &str,
    timeout_secs: u64,
) -> Result<RemoteToolResult, String> {
    if command.trim().is_empty() {
        return Err("命令为空".into());
    }
    let final_cmd = build_command(command, tool_path, workdir, env);
    let timeout = Duration::from_secs(timeout_secs.clamp(1, 3600));

    let handle = connect_authenticated(app, profile_id).await?;
    let mut channel = handle
        .channel_open_session()
        .await
        .map_err(|e| format!("打开通道失败: {e}"))?;
    channel
        .exec(true, final_cmd.as_bytes())
        .await
        .map_err(|e| format!("执行命令失败: {e}"))?;

    let started = Instant::now();
    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let mut exit_code: Option<i32> = None;

    loop {
        let remain = timeout.saturating_sub(started.elapsed());
        if remain.is_zero() {
            return Err(format!(
                "远程命令超时（{} 秒）：{final_cmd}",
                timeout.as_secs()
            ));
        }
        match tokio::time::timeout(remain, channel.wait()).await {
            Err(_) => {
                return Err(format!(
                    "远程命令超时（{} 秒）：{final_cmd}",
                    timeout.as_secs()
                ))
            }
            Ok(None) => break,
            Ok(Some(ChannelMsg::Data { data })) => stdout.extend_from_slice(&data),
            Ok(Some(ChannelMsg::ExtendedData { data, .. })) => {
                stderr.extend_from_slice(&data)
            }
            Ok(Some(ChannelMsg::ExitStatus { exit_status })) => {
                exit_code = Some(exit_status as i32)
            }
            Ok(Some(_)) => {}
        }
    }

    let out = String::from_utf8_lossy(&stdout).into_owned();
    let err = String::from_utf8_lossy(&stderr).into_owned();
    let output = if err.is_empty() {
        out.clone()
    } else if out.is_empty() {
        err.clone()
    } else {
        format!("{out}\n[stderr]\n{err}")
    };

    Ok(RemoteToolResult {
        command: final_cmd,
        stdout: out,
        stderr: err,
        output,
        exit_code,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTransferResult {
    pub bytes: u64,
    pub local_path: String,
    pub remote_path: String,
}

/// 本机 → 远端上传（SFTP）。
#[command]
pub async fn remote_tool_upload(
    app: AppHandle,
    profile_id: String,
    local_path: String,
    remote_path: String,
) -> Result<RemoteTransferResult, String> {
    if remote_path.trim().is_empty() {
        return Err("远端路径不能为空".into());
    }
    let session = crate::commands::sftp_fs::sftp_connect(app, profile_id).await?;
    let res = crate::commands::sftp_fs::sftp_upload(
        session.session_id.clone(),
        local_path.clone(),
        remote_path.clone(),
    )
    .await;
    let _ = crate::commands::sftp_fs::sftp_close(session.session_id);
    Ok(RemoteTransferResult {
        bytes: res?,
        local_path,
        remote_path,
    })
}

/// 远端 → 本机下载（SFTP）。
#[command]
pub async fn remote_tool_download(
    app: AppHandle,
    profile_id: String,
    remote_path: String,
    local_path: String,
) -> Result<RemoteTransferResult, String> {
    if local_path.trim().is_empty() {
        return Err("本地路径不能为空".into());
    }
    let session = crate::commands::sftp_fs::sftp_connect(app, profile_id).await?;
    let res = crate::commands::sftp_fs::sftp_download(
        session.session_id.clone(),
        remote_path.clone(),
        local_path.clone(),
    )
    .await;
    let _ = crate::commands::sftp_fs::sftp_close(session.session_id);
    Ok(RemoteTransferResult {
        bytes: res?,
        local_path,
        remote_path,
    })
}

// ============================ 远端文件系统挂载（SSHFS） ============================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteMountStatus {
    pub available: bool,
    pub binary: String,
    pub hint: String,
}

fn sshfs_binary() -> Option<&'static str> {
    ["sshfs", "sshfs.exe"]
        .into_iter()
        .find(|b| Command::new(b).arg("--version").output().is_ok())
}

#[command]
pub fn remote_mount_status() -> RemoteMountStatus {
    match sshfs_binary() {
        Some(b) => RemoteMountStatus {
            available: true,
            binary: b.to_string(),
            hint: String::new(),
        },
        None => RemoteMountStatus {
            available: false,
            binary: String::new(),
            hint: "未找到 sshfs。Windows 请安装 WinFsp + SSHFS-Win；Linux 安装 sshfs 包。安装后即可把远端目录挂成本机路径，用系统文件浏览器直接操作。".into(),
        },
    }
}

static MOUNTS: OnceLock<Mutex<HashMap<String, Child>>> = OnceLock::new();
fn mounts() -> &'static Mutex<HashMap<String, Child>> {
    MOUNTS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteMountResult {
    pub mount_point: String,
    pub profile_id: String,
}

#[command]
pub fn remote_mounts() -> Vec<String> {
    mounts()
        .lock()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

fn safe_file_name(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

/// 用 SSHFS 把远端根目录挂到本机（Windows 需 WinFsp + SSHFS-Win，Linux 需 sshfs）。
///
/// 挂载后远端就是一个本机目录：系统文件浏览器、上传/下载、工具路径都能直接选。
#[command]
pub async fn remote_mount(
    app: AppHandle,
    profile_id: String,
    mount_point: Option<String>,
) -> Result<RemoteMountResult, String> {
    let bin = sshfs_binary().ok_or_else(|| {
        "未找到 sshfs。Windows 请安装 WinFsp + SSHFS-Win；Linux 安装 sshfs 包".to_string()
    })?;
    let (profile, secret) = crate::commands::ssh_terminal::profile_credentials(&app, &profile_id)?;
    if profile.kind != "ssh" {
        return Err("只有 SSH 档案可以挂载".into());
    }
    if profile.host.trim().is_empty() || profile.user.trim().is_empty() {
        return Err("档案缺少主机或用户名".into());
    }

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("取应用数据目录失败: {e}"))?
        .join("remote_mounts");
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("建目录失败: {e}"))?;

    let mount_dir = match mount_point.filter(|s| !s.trim().is_empty()) {
        Some(p) => std::path::PathBuf::from(p),
        None => data_dir.join(format!(
            "{}-{}",
            safe_file_name(&profile.name),
            &profile.id.chars().take(8).collect::<String>()
        )),
    };
    std::fs::create_dir_all(&mount_dir).map_err(|e| format!("创建挂载点失败: {e}"))?;
    let mount_str = mount_dir.to_string_lossy().to_string();

    if mounts()
        .lock()
        .map(|m| m.contains_key(&mount_str))
        .unwrap_or(false)
    {
        return Ok(RemoteMountResult { mount_point: mount_str, profile_id });
    }

    let log_path = data_dir.join(format!("sshfs-{}.log", safe_file_name(&profile.name)));
    let log = std::fs::File::create(&log_path).map_err(|e| format!("创建日志失败: {e}"))?;
    let log2 = log.try_clone().map_err(|e| format!("创建日志失败: {e}"))?;

    let password = if profile.auth == "password" { secret } else { None };
    let key_path = profile.key_path.clone().filter(|s| !s.trim().is_empty());

    let mut cmd = Command::new(bin);
    cmd.arg(format!("{}@{}:/", profile.user, profile.host))
        .arg(&mount_dir)
        .arg("-f")
        .arg("-o")
        .arg(format!("port={}", profile.port))
        .arg("-o")
        .arg("reconnect")
        .arg("-o")
        .arg("ServerAliveInterval=15")
        // 与 russh 的连接策略保持一致：首次接受新主机键，避免每次弹终端确认。
        .arg("-o")
        .arg("StrictHostKeyChecking=accept-new")
        .arg("-o")
        .arg("dir_cache=yes");
    if let Some(k) = &key_path {
        cmd.arg("-o").arg(format!("IdentityFile={k}"));
    }
    if password.is_some() {
        cmd.arg("-o").arg("password_stdin");
        cmd.stdin(Stdio::piped());
    } else {
        cmd.stdin(Stdio::null());
    }
    cmd.stdout(Stdio::from(log)).stderr(Stdio::from(log2));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let mut child = cmd.spawn().map_err(|e| format!("启动 sshfs 失败: {e}"))?;
    if let Some(pw) = password {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = writeln!(stdin, "{pw}");
        }
    }

    // sshfs 在前台运行（-f）；短暂等待，若立刻退出就把日志给用户看。
    tokio::time::sleep(Duration::from_millis(1200)).await;
    match child.try_wait() {
        Ok(Some(status)) => {
            let log = std::fs::read_to_string(&log_path).unwrap_or_default();
            return Err(format!(
                "挂载失败（退出码 {:?}）：{}",
                status.code(),
                log.trim()
            ));
        }
        Ok(None) => {}
        Err(e) => return Err(format!("检查 sshfs 状态失败: {e}")),
    }

    mounts()
        .lock()
        .map_err(|_| "挂载表锁失败".to_string())?
        .insert(mount_str.clone(), child);
    Ok(RemoteMountResult { mount_point: mount_str, profile_id })
}

#[command]
pub fn remote_unmount(mount_point: String) -> Result<(), String> {
    let child = mounts()
        .lock()
        .map_err(|_| "挂载表锁失败".to_string())?
        .remove(&mount_point);
    if let Some(mut c) = child {
        let _ = c.kill();
        let _ = c.wait();
    }
    // 进程结束通常已卸载；再做一次系统级兜底。
    #[cfg(unix)]
    {
        let _ = Command::new("fusermount").arg("-u").arg(&mount_point).status();
        let _ = Command::new("umount").arg(&mount_point).status();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("net")
            .args(["use", &mount_point, "/delete", "/y"])
            .status();
    }
    Ok(())
}
