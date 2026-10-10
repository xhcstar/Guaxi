//! 网络/终端工具箱 —— SFTP 文件传输（在 SSH 档案上开 sftp 子系统）。
//!
//! 复用 ssh_terminal 的连接认证,单开一条 SSH 连接跑 sftp,和交互终端解耦。
//! 提供列目录 / 上传 / 下载 / 新建目录 / 删除 / 重命名。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use serde::Serialize;
use tauri::{command, AppHandle};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

use crate::commands::ssh_terminal::{connect_authenticated, ClientHandler};

struct SftpEntry {
    session: SftpSession,
    // 保持 SSH 连接存活到 sftp 会话结束。
    _handle: russh::client::Handle<ClientHandler>,
}

type SftpMap = HashMap<String, Arc<AsyncMutex<SftpEntry>>>;
static SFTP_SESSIONS: OnceLock<Mutex<SftpMap>> = OnceLock::new();
fn store() -> &'static Mutex<SftpMap> {
    SFTP_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_entry(id: &str) -> Result<Arc<AsyncMutex<SftpEntry>>, String> {
    store()
        .lock()
        .map_err(|_| "会话锁失败".to_string())?
        .get(id)
        .cloned()
        .ok_or_else(|| "SFTP 会话不存在".to_string())
}

#[derive(Serialize)]
pub struct SftpConnectResult {
    pub session_id: String,
    pub home: String,
}

#[command]
pub async fn sftp_connect(app: AppHandle, profile_id: String) -> Result<SftpConnectResult, String> {
    let handle = connect_authenticated(&app, &profile_id).await?;
    let channel = handle
        .channel_open_session()
        .await
        .map_err(|e| format!("打开通道失败: {e}"))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| format!("请求 sftp 子系统失败: {e}"))?;
    let session = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| format!("建立 sftp 会话失败: {e}"))?;
    let home = session.canonicalize(".").await.unwrap_or_else(|_| "/".to_string());

    let session_id = Uuid::new_v4().to_string();
    store()
        .lock()
        .map_err(|_| "会话锁失败".to_string())?
        .insert(session_id.clone(), Arc::new(AsyncMutex::new(SftpEntry { session, _handle: handle })));
    Ok(SftpConnectResult { session_id, home })
}

#[derive(Serialize)]
pub struct SftpEntryInfo {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

#[command]
pub async fn sftp_list(session_id: String, path: String) -> Result<Vec<SftpEntryInfo>, String> {
    let entry = get_entry(&session_id)?;
    let guard = entry.lock().await;
    let p = if path.trim().is_empty() { ".".to_string() } else { path };
    let rd = guard.session.read_dir(p).await.map_err(|e| format!("列目录失败: {e}"))?;
    let mut out: Vec<SftpEntryInfo> = rd
        .map(|de| {
            let m = de.metadata();
            SftpEntryInfo { name: de.file_name(), is_dir: m.is_dir(), size: m.len() }
        })
        .collect();
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

/// 解析远端 "." / 相对路径为绝对路径。
#[command]
pub async fn sftp_canonicalize(session_id: String, path: String) -> Result<String, String> {
    let entry = get_entry(&session_id)?;
    let guard = entry.lock().await;
    guard.session.canonicalize(path).await.map_err(|e| format!("解析路径失败: {e}"))
}

/// 下载：远端 remote_path → 本地 local_path。
#[command]
pub async fn sftp_download(session_id: String, remote_path: String, local_path: String) -> Result<u64, String> {
    let entry = get_entry(&session_id)?;
    let guard = entry.lock().await;
    let data = guard.session.read(remote_path).await.map_err(|e| format!("读取远端失败: {e}"))?;
    let len = data.len() as u64;
    std::fs::write(&local_path, &data).map_err(|e| format!("写本地失败: {e}"))?;
    Ok(len)
}

/// 上传：本地 local_path → 远端 remote_path。
///
/// 不能用 `session.write()`——它只以 `OpenFlags::WRITE` 打开，目标不存在时远端返回
/// SSH_FX_NO_SUCH_FILE（“写远端失败: No such file”）。这里显式带 CREATE|TRUNCATE 打开，
/// 写完后 `shutdown()` 关句柄刷盘。
#[command]
pub async fn sftp_upload(session_id: String, local_path: String, remote_path: String) -> Result<u64, String> {
    let entry = get_entry(&session_id)?;
    let data = std::fs::read(&local_path).map_err(|e| format!("读本地失败: {e}"))?;
    let len = data.len() as u64;
    let guard = entry.lock().await;
    let mut file = guard
        .session
        .open_with_flags(
            remote_path,
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
        )
        .await
        .map_err(|e| format!("打开远端文件失败: {e}"))?;
    file.write_all(&data).await.map_err(|e| format!("写远端失败: {e}"))?;
    file.shutdown().await.map_err(|e| format!("关闭远端文件失败: {e}"))?;
    Ok(len)
}

#[command]
pub async fn sftp_mkdir(session_id: String, path: String) -> Result<(), String> {
    let entry = get_entry(&session_id)?;
    let guard = entry.lock().await;
    guard.session.create_dir(path).await.map_err(|e| format!("新建目录失败: {e}"))
}

#[command]
pub async fn sftp_remove(session_id: String, path: String, is_dir: bool) -> Result<(), String> {
    let entry = get_entry(&session_id)?;
    let guard = entry.lock().await;
    if is_dir {
        guard.session.remove_dir(path).await.map_err(|e| format!("删除目录失败: {e}"))
    } else {
        guard.session.remove_file(path).await.map_err(|e| format!("删除文件失败: {e}"))
    }
}

#[command]
pub async fn sftp_rename(session_id: String, from: String, to: String) -> Result<(), String> {
    let entry = get_entry(&session_id)?;
    let guard = entry.lock().await;
    guard.session.rename(from, to).await.map_err(|e| format!("重命名失败: {e}"))
}

#[command]
pub fn sftp_close(session_id: String) -> Result<(), String> {
    store().lock().map_err(|_| "会话锁失败".to_string())?.remove(&session_id);
    Ok(())
}
