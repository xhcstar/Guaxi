//! 终端调试工坊 —— SSH 连接与会话录制（批3）。
//!
//! 采用纯 Rust 的 russh 自带 SSH 栈（不依赖系统 ssh 客户端，符合“release 离线自足”）。
//! 每个会话在 tokio 任务里持有 russh 通道：服务端输出实时推前端 xterm 并原样落盘录制
//! （B2），前端输入/尺寸变化经 mpsc 下发给通道。连接档案（非机密）存 JSON（A4），
//! 密码/密钥口令经系统 keychain 存取（A5）。录制文件可直接喂进日志分析管道回放（B4）。

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use russh::client::{self};
use russh::keys::{load_secret_key, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh::{ChannelMsg, Disconnect};
use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Emitter, Manager};
use tokio::sync::mpsc;
use uuid::Uuid;

const SSH_OUTPUT_EVENT: &str = "ssh-session-output";
const SSH_EXIT_EVENT: &str = "ssh-session-exit";
const KEYRING_SERVICE: &str = "guaxi-ssh";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub user: String,
    /// "key" | "password" | "agent"（agent 现为“尝试默认 ~/.ssh 密钥”）
    #[serde(default = "default_auth")]
    pub auth: String,
    #[serde(default)]
    pub key_path: Option<String>,
    /// 连接类型："ssh" | "serial" | "telnet"（旧档案无此字段，默认 ssh）。
    #[serde(default = "default_kind")]
    pub kind: String,
    /// 文件夹分组（Xshell/MobaXterm 式分类）。
    #[serde(default)]
    pub group: Option<String>,
    /// 串口专用。
    #[serde(default)]
    pub serial_path: Option<String>,
    #[serde(default)]
    pub serial_baud: Option<u32>,
}

fn default_port() -> u16 {
    22
}
fn default_auth() -> String {
    "agent".to_string()
}
fn default_kind() -> String {
    "ssh".to_string()
}

#[derive(Debug, Clone, Serialize)]
pub struct SshSessionSnapshot {
    pub session_id: String,
    pub profile_id: String,
    pub host: String,
    pub recording_path: String,
}

#[derive(Serialize)]
pub struct RecordingInfo {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub modified_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
struct SshOutputEvent {
    session_id: String,
    data: String,
}

#[derive(Debug, Clone, Serialize)]
struct SshExitEvent {
    session_id: String,
}

enum SessionCmd {
    Data(Vec<u8>),
    Resize { cols: u32, rows: u32 },
    Close,
}

struct SshSession {
    cmd_tx: mpsc::UnboundedSender<SessionCmd>,
}

type SessionMap = HashMap<String, Arc<SshSession>>;
static SSH_SESSIONS: OnceLock<Mutex<SessionMap>> = OnceLock::new();

fn session_store() -> &'static Mutex<SessionMap> {
    SSH_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 关闭软件时请求结束所有 ssh 会话。
pub fn shutdown_all_sessions() {
    if let Ok(mut g) = session_store().lock() {
        for (_id, s) in g.drain() {
            let _ = s.cmd_tx.send(SessionCmd::Close);
        }
    }
}

// -------- russh 客户端事件处理 --------

pub(crate) struct ClientHandler;

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(&mut self, _key: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        // 面向“连接自有设备”的调试工具，接受服务端指纹。已知主机校验可后续增强。
        Ok(true)
    }
}

/// 供 GDB/MI 模块复用：按档案连接并认证，返回可开通道的 client handle。
pub(crate) async fn connect_authenticated(
    app: &AppHandle,
    profile_id: &str,
) -> Result<client::Handle<ClientHandler>, String> {
    let profile = read_profiles(app)?
        .into_iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| "连接档案不存在".to_string())?;
    let secret = get_secret(&profile.id);
    connect_with_profile(&profile, secret).await
}

/// 按显式参数连接并认证。
///
/// 命令下发工具复用：既支持库内 SSH 档案，也支持直接吃配置文件里内联的凭据
/// （参考工具的 config.yaml 把 ip/username/password 写在 global 段）。
pub(crate) async fn connect_explicit(
    host: &str,
    port: u16,
    user: &str,
    auth: &str,
    secret: Option<String>,
    key_path: Option<&str>,
) -> Result<client::Handle<ClientHandler>, String> {
    let profile = SshProfile {
        id: String::new(),
        name: String::new(),
        host: host.to_string(),
        port,
        user: user.to_string(),
        auth: auth.to_string(),
        key_path: key_path.map(|s| s.to_string()),
        kind: "ssh".to_string(),
        group: None,
        serial_path: None,
        serial_baud: None,
    };
    connect_with_profile(&profile, secret).await
}

/// 连接 + 认证的公共实现（两条入口共用）。
pub(crate) async fn connect_with_profile(
    profile: &SshProfile,
    secret: Option<String>,
) -> Result<client::Handle<ClientHandler>, String> {
    let config = Arc::new(client::Config { inactivity_timeout: None, ..Default::default() });
    let mut handle = client::connect(config, (profile.host.clone(), profile.port), ClientHandler)
        .await
        .map_err(|e| format!("SSH 连接失败: {e}"))?;
    if !authenticate(&mut handle, profile, secret).await? {
        return Err("SSH 认证失败".to_string());
    }
    Ok(handle)
}

// -------- 存储路径 --------

fn base_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {e}"))?
        .join("remote_terminal");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {e}"))?;
    Ok(dir)
}

fn profiles_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(base_dir(app)?.join("ssh_profiles.json"))
}

fn recordings_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = base_dir(app)?.join("recordings");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建录制目录失败: {e}"))?;
    Ok(dir)
}

fn read_profiles(app: &AppHandle) -> Result<Vec<SshProfile>, String> {
    let path = profiles_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = std::fs::read_to_string(&path).map_err(|e| format!("读取连接档案失败: {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("解析连接档案失败: {e}"))
}

fn write_profiles(app: &AppHandle, profiles: &[SshProfile]) -> Result<(), String> {
    let path = profiles_path(app)?;
    let json = serde_json::to_string_pretty(profiles).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("写入连接档案失败: {e}"))
}

fn get_secret(profile_id: &str) -> Option<String> {
    keyring::Entry::new(KEYRING_SERVICE, profile_id)
        .ok()
        .and_then(|e| e.get_password().ok())
}

fn default_key_paths() -> Vec<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    if home.is_empty() {
        return Vec::new();
    }
    let ssh = PathBuf::from(home).join(".ssh");
    ["id_ed25519", "id_rsa", "id_ecdsa"]
        .iter()
        .map(|n| ssh.join(n))
        .collect()
}

// -------- 连接档案 CRUD（A4） --------

#[command]
pub fn ssh_list_profiles(app: AppHandle) -> Result<Vec<SshProfile>, String> {
    read_profiles(&app)
}

#[command]
pub fn ssh_save_profile(app: AppHandle, mut profile: SshProfile) -> Result<SshProfile, String> {
    let mut profiles = read_profiles(&app)?;
    if profile.id.trim().is_empty() {
        profile.id = Uuid::new_v4().to_string();
    }
    match profiles.iter_mut().find(|p| p.id == profile.id) {
        Some(existing) => *existing = profile.clone(),
        None => profiles.push(profile.clone()),
    }
    write_profiles(&app, &profiles)?;
    Ok(profile)
}

#[command]
pub fn ssh_delete_profile(app: AppHandle, id: String) -> Result<(), String> {
    let mut profiles = read_profiles(&app)?;
    profiles.retain(|p| p.id != id);
    write_profiles(&app, &profiles)?;
    let _ = keyring::Entry::new(KEYRING_SERVICE, &id).and_then(|e| e.delete_credential());
    Ok(())
}

// -------- 凭据（A5，系统 keychain） --------

#[command]
pub fn ssh_set_secret(profile_id: String, secret: String) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, &profile_id).map_err(|e| e.to_string())?;
    entry.set_password(&secret).map_err(|e| format!("写入 keychain 失败: {e}"))
}

#[command]
pub fn ssh_delete_secret(profile_id: String) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, &profile_id).map_err(|e| e.to_string())?;
    match entry.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("删除 keychain 失败: {e}")),
    }
}

#[command]
pub fn ssh_has_secret(profile_id: String) -> Result<bool, String> {
    Ok(get_secret(&profile_id).is_some())
}

// -------- enable 密码（命令下发用：网络设备 CLI 的 `enable` 口令）--------
//
// 与登录密码分开存：同一档案可能登录用密钥、enable 用口令。仍然只进系统 keychain，不明文落盘。

const KEYRING_ENABLE_SERVICE: &str = "guaxi-ssh-enable";

fn get_enable_secret(profile_id: &str) -> Option<String> {
    keyring::Entry::new(KEYRING_ENABLE_SERVICE, profile_id)
        .ok()
        .and_then(|e| e.get_password().ok())
}

#[command]
pub fn ssh_set_enable_secret(profile_id: String, secret: String) -> Result<(), String> {
    let entry =
        keyring::Entry::new(KEYRING_ENABLE_SERVICE, &profile_id).map_err(|e| e.to_string())?;
    entry
        .set_password(&secret)
        .map_err(|e| format!("写入 keychain 失败: {e}"))
}

#[command]
pub fn ssh_delete_enable_secret(profile_id: String) -> Result<(), String> {
    let entry =
        keyring::Entry::new(KEYRING_ENABLE_SERVICE, &profile_id).map_err(|e| e.to_string())?;
    match entry.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("删除 keychain 失败: {e}")),
    }
}

#[command]
pub fn ssh_has_enable_secret(profile_id: String) -> Result<bool, String> {
    Ok(get_enable_secret(&profile_id).is_some())
}

/// 供命令下发模块复用：取该档案的 enable 密码。
pub(crate) fn enable_secret(profile_id: &str) -> Option<String> {
    get_enable_secret(profile_id)
}

// -------- 会话（A1，russh） --------

async fn authenticate(
    handle: &mut client::Handle<ClientHandler>,
    profile: &SshProfile,
    secret: Option<String>,
) -> Result<bool, String> {
    match profile.auth.as_str() {
        "password" => {
            let pw = secret.ok_or_else(|| "该档案未设置密码，请先在表单里填写并保存".to_string())?;
            let res = handle
                .authenticate_password(profile.user.clone(), pw)
                .await
                .map_err(|e| format!("密码认证失败: {e}"))?;
            Ok(res.success())
        }
        "key" => {
            let kp = profile
                .key_path
                .as_ref()
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| "密钥认证需要指定私钥路径".to_string())?;
            let key = load_secret_key(kp, secret.as_deref())
                .map_err(|e| format!("加载私钥失败: {e}"))?;
            let hash = handle
                .best_supported_rsa_hash()
                .await
                .map_err(|e| format!("协商 RSA hash 失败: {e}"))?
                .flatten();
            let res = handle
                .authenticate_publickey(
                    profile.user.clone(),
                    PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                )
                .await
                .map_err(|e| format!("密钥认证失败: {e}"))?;
            Ok(res.success())
        }
        _ => {
            // 默认：依次尝试 ~/.ssh 常见私钥；都不行且有存储口令则退回密码。
            for p in default_key_paths() {
                if !p.exists() {
                    continue;
                }
                if let Ok(key) = load_secret_key(&p, secret.as_deref()) {
                    let hash = handle.best_supported_rsa_hash().await.ok().flatten().flatten();
                    if let Ok(res) = handle
                        .authenticate_publickey(
                            profile.user.clone(),
                            PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                        )
                        .await
                    {
                        if res.success() {
                            return Ok(true);
                        }
                    }
                }
            }
            if let Some(pw) = secret {
                let res = handle
                    .authenticate_password(profile.user.clone(), pw)
                    .await
                    .map_err(|e| format!("密码认证失败: {e}"))?;
                return Ok(res.success());
            }
            Ok(false)
        }
    }
}

#[command]
pub async fn ssh_create_session(
    app: AppHandle,
    profile_id: String,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<SshSessionSnapshot, String> {
    let profile = read_profiles(&app)?
        .into_iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| "连接档案不存在".to_string())?;
    let secret = get_secret(&profile.id);
    let rec_dir = recordings_dir(&app)?;

    let config = Arc::new(client::Config {
        inactivity_timeout: None,
        ..Default::default()
    });
    let mut handle = client::connect(config, (profile.host.clone(), profile.port), ClientHandler)
        .await
        .map_err(|e| format!("SSH 连接失败: {e}"))?;

    if !authenticate(&mut handle, &profile, secret).await? {
        return Err("SSH 认证失败".to_string());
    }

    let mut channel = handle
        .channel_open_session()
        .await
        .map_err(|e| format!("打开会话通道失败: {e}"))?;
    let cols = cols.unwrap_or(120).max(20) as u32;
    let rows = rows.unwrap_or(32).max(10) as u32;
    channel
        .request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
        .await
        .map_err(|e| format!("请求 PTY 失败: {e}"))?;
    channel
        .request_shell(true)
        .await
        .map_err(|e| format!("请求 shell 失败: {e}"))?;

    // 录制文件
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let safe: String = profile
        .name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let rec_path = rec_dir.join(format!("{safe}-{ts}.log"));
    let mut recorder: Option<BufWriter<File>> = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&rec_path)
        .ok()
        .map(BufWriter::new);

    let session_id = Uuid::new_v4().to_string();
    let (tx, mut rx) = mpsc::unbounded_channel::<SessionCmd>();
    session_store()
        .lock()
        .map_err(|_| "会话锁失败".to_string())?
        .insert(session_id.clone(), Arc::new(SshSession { cmd_tx: tx }));

    let snapshot = SshSessionSnapshot {
        session_id: session_id.clone(),
        profile_id: profile.id.clone(),
        host: profile.host.clone(),
        recording_path: rec_path.to_string_lossy().to_string(),
    };

    // 会话事件循环：服务端输出 <-> 前端输入/尺寸。
    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = channel.wait() => {
                    match msg {
                        Some(ChannelMsg::Data { ref data })
                        | Some(ChannelMsg::ExtendedData { ref data, .. }) => {
                            if let Some(w) = recorder.as_mut() {
                                let _ = w.write_all(data);
                                let _ = w.flush();
                            }
                            let text = String::from_utf8_lossy(data).to_string();
                            let _ = app.emit(
                                SSH_OUTPUT_EVENT,
                                SshOutputEvent { session_id: session_id.clone(), data: text },
                            );
                        }
                        Some(ChannelMsg::ExitStatus { .. }) | None => break,
                        _ => {}
                    }
                }
                cmd = rx.recv() => {
                    match cmd {
                        Some(SessionCmd::Data(bytes)) => {
                            let _ = channel.data(&bytes[..]).await;
                        }
                        Some(SessionCmd::Resize { cols, rows }) => {
                            let _ = channel.window_change(cols, rows, 0, 0).await;
                        }
                        Some(SessionCmd::Close) | None => break,
                    }
                }
            }
        }
        if let Ok(mut sessions) = session_store().lock() {
            sessions.remove(&session_id);
        }
        let _ = app.emit(SSH_EXIT_EVENT, SshExitEvent { session_id: session_id.clone() });
        let _ = channel.eof().await;
        let _ = handle.disconnect(Disconnect::ByApplication, "", "English").await;
    });

    Ok(snapshot)
}

fn send_cmd(session_id: &str, cmd: SessionCmd) -> Result<(), String> {
    let session = {
        let sessions = session_store().lock().map_err(|_| "会话锁失败".to_string())?;
        sessions.get(session_id.trim()).cloned()
    };
    let session = session.ok_or_else(|| "会话不存在".to_string())?;
    session.cmd_tx.send(cmd).map_err(|_| "会话已关闭".to_string())
}

#[command]
pub fn ssh_write_session(session_id: String, data: String) -> Result<(), String> {
    send_cmd(&session_id, SessionCmd::Data(data.into_bytes()))
}

#[command]
pub fn ssh_resize_session(session_id: String, cols: u16, rows: u16) -> Result<(), String> {
    send_cmd(
        &session_id,
        SessionCmd::Resize { cols: cols.max(2) as u32, rows: rows.max(2) as u32 },
    )
}

#[command]
pub fn ssh_close_session(session_id: String) -> Result<(), String> {
    // 忽略“会话不存在”：可能已因远端断开而自行清理。
    let _ = send_cmd(&session_id, SessionCmd::Close);
    Ok(())
}

// -------- 录制列表（B4：可直接喂进日志分析管道回放） --------

#[command]
pub fn ssh_list_recordings(app: AppHandle) -> Result<Vec<RecordingInfo>, String> {
    let dir = recordings_dir(&app)?;
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64);
        out.push(RecordingInfo {
            path: path.to_string_lossy().to_string(),
            name: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            size: meta.len(),
            modified_ms,
        });
    }
    out.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    Ok(out)
}
