//! 远程工具台：通过库内 SSH 档案在 Linux 机器上执行用户自备的工具
//! （如自行编译的 Tongsuo/openssl），并支持本机 ↔ 远端文件传输。
//!
//! 这样 SM2/PQC、客户端/服务端联调这类依赖 Tongsuo 的能力不必在本机原生编译：
//! 用户只需要在目标机准备好工具，填工具路径即可。

use std::time::{Duration, Instant};

use russh::ChannelMsg;
use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle};

use crate::commands::ssh_terminal::connect_authenticated;

fn shell_quote(s: &str) -> String {
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
    if command.trim().is_empty() {
        return Err("命令为空".into());
    }
    let o = opts.unwrap_or_default();
    let final_cmd = build_command(
        &command,
        o.tool_path.as_deref().unwrap_or(""),
        o.workdir.as_deref().unwrap_or(""),
        o.env.as_deref().unwrap_or(""),
    );
    let timeout = Duration::from_secs(o.timeout_secs.unwrap_or(120).clamp(1, 3600));

    let handle = connect_authenticated(&app, &profile_id).await?;
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
