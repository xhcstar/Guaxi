use crate::commands::settingview::{
    get_device_id, get_image_bed_root, get_knowledge_bases_root, get_resource_repo_sync_config,
};
use base64::Engine as _;
use blake3::Hasher;
use chrono::{DateTime, Duration, Utc};
use copia::{Sync, SyncBuilder};
use hex;
use iroh::{
    endpoint::{presets, Connection, RecvStream, SendStream},
    Endpoint, EndpointAddr, SecretKey,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration as StdDuration;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio::time::timeout;
use tracing::info;
use uuid::Uuid;
use walkdir::WalkDir;

const P2P_META_DIR: &str = ".guaxi-p2p";
const P2P_STATE_FILE: &str = "state.json";
const P2P_MANIFEST_FILE: &str = "manifest.json";
const P2P_TMP_DIR: &str = "tmp";
const P2P_BLOCK_SIZE: usize = 64 * 1024;
const P2P_ALPN: &[u8] = b"guaxi/p2p-sync/1";
const P2P_ONLINE_TIMEOUT_SECS: u64 = 12;
const P2P_FRAME_LIMIT: usize = 1024 * 1024;
/// 清单可能包含上万个文件条目，单帧上限放宽到 256MiB。
const P2P_MANIFEST_FRAME_LIMIT: usize = 256 * 1024 * 1024;
/// 文件体按固定块流式收发，避免把整文件读进内存。
const P2P_CHUNK_SIZE: usize = 256 * 1024;
const RESOURCE_REPO_SYNC_PROGRESS_EVENT: &str = "resource_repo_sync_progress";

struct P2PEndpointRuntime {
    endpoint: Endpoint,
}

static P2P_ENDPOINT: OnceLock<Mutex<Option<P2PEndpointRuntime>>> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
enum P2PSyncTargetKind {
    ResourceRepo,
    KnowledgeBase,
}

#[derive(Debug, Clone)]
struct P2PSyncTarget {
    kind: P2PSyncTargetKind,
    key: &'static str,
    label: &'static str,
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceRepoSyncProgress {
    pub run_id: String,
    pub provider: String,
    pub target: String,
    pub phase: String,
    pub status: String,
    pub message: String,
    pub current: u64,
    pub total: u64,
    pub bytes_current: u64,
    pub bytes_total: u64,
    pub file_path: String,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct P2PTrustedPeer {
    pub peer_id: String,
    pub device_id: String,
    pub device_name: String,
    pub added_at: String,
    #[serde(default)]
    pub address_hint: String,
    #[serde(default)]
    pub online_status: String,
    #[serde(default)]
    pub status_detail: String,
    #[serde(default)]
    pub last_checked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PPendingTicket {
    ticket_id: String,
    ticket: String,
    created_at: String,
    expires_at: Option<String>,
    used: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct P2PStateFile {
    secret_key_hex: String,
    trusted_peers: Vec<P2PTrustedPeer>,
    pending_tickets: Vec<P2PPendingTicket>,
}

#[derive(Debug, Clone, Serialize)]
pub struct P2PStatus {
    pub enabled: bool,
    pub provider: String,
    pub device_id: String,
    pub device_name: String,
    pub peer_id: String,
    pub ticket_validity_mode: String,
    pub ticket_ttl_minutes: u64,
    pub auto_sync_resource_repo: bool,
    pub sync_interval_minutes: u64,
    pub trusted_peers: Vec<P2PTrustedPeer>,
    pub iroh_version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct P2PTicketResponse {
    pub ticket: String,
    pub ticket_id: String,
    pub peer_id: String,
    pub device_id: String,
    pub device_name: String,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PTicketPayload {
    version: u32,
    ticket_id: String,
    peer_id: String,
    device_id: String,
    device_name: String,
    created_at: String,
    expires_at: Option<String>,
    #[serde(default)]
    address_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum P2PWireRequest {
    Probe {
        device_id: String,
        device_name: String,
        peer_id: String,
        address_hint: String,
    },
    Pair {
        ticket_id: String,
        device_id: String,
        device_name: String,
        peer_id: String,
        address_hint: String,
    },
    /// 请求对端返回某个同步目标的清单（用于 diff）。
    Manifest {
        target: String,
    },
    /// 从对端拉取一个文件，响应为文件头 + 原始字节流。
    PullFile {
        target: String,
        rel: String,
    },
    /// 向对端推送一个文件，请求帧后紧跟原始字节流。
    PushFile {
        target: String,
        rel: String,
        size: u64,
        blake3: String,
        mtime_ms: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PWireResponse {
    ok: bool,
    message: String,
    device_id: String,
    device_name: String,
    peer_id: String,
    address_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PManifestResponse {
    ok: bool,
    message: String,
    manifest: Option<P2PManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PFileHeader {
    ok: bool,
    message: String,
    size: u64,
    blake3: String,
    mtime_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PAck {
    ok: bool,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PManifest {
    version: u32,
    target: String,
    device_id: String,
    updated_at: String,
    files: BTreeMap<String, P2PFileEntry>,
}

fn emit_sync_progress(app: Option<&AppHandle>, progress: ResourceRepoSyncProgress) {
    info!(
        run_id = progress.run_id,
        provider = progress.provider,
        target = progress.target,
        phase = progress.phase,
        status = progress.status,
        current = progress.current,
        total = progress.total,
        bytes_current = progress.bytes_current,
        bytes_total = progress.bytes_total,
        file_path = progress.file_path,
        message = progress.message,
        "[P2PSync][Progress]"
    );
    if let Some(app) = app {
        if let Err(e) = app.emit(RESOURCE_REPO_SYNC_PROGRESS_EVENT, progress) {
            info!("[P2PSync][Progress] emit failed: {}", e);
        }
    }
}

fn emit_p2p_progress(
    app: Option<&AppHandle>,
    run_id: &str,
    target: &P2PSyncTarget,
    phase: &str,
    status: &str,
    message: impl Into<String>,
    current: u64,
    total: u64,
    bytes_current: u64,
    bytes_total: u64,
    file_path: impl Into<String>,
) {
    emit_sync_progress(
        app,
        ResourceRepoSyncProgress {
            run_id: run_id.to_string(),
            provider: "p2p".to_string(),
            target: target.key.to_string(),
            phase: phase.to_string(),
            status: status.to_string(),
            message: message.into(),
            current,
            total,
            bytes_current,
            bytes_total,
            file_path: file_path.into(),
            at: Utc::now().to_rfc3339(),
        },
    );
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct P2PFileEntry {
    size: u64,
    modified_unix_ms: i64,
    blake3: String,
}

#[derive(Debug, Clone, Serialize, Default)]
struct P2PSyncStats {
    uploaded: u32,
    downloaded: u32,
    conflicts: u32,
    skipped: u32,
    verified: u32,
}

fn p2p_state_dir() -> Result<PathBuf, String> {
    let exe = env::current_exe().map_err(|e| format!("获取程序路径失败: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "无法获取程序所在目录".to_string())?
        .join(P2P_META_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("创建 P2P 状态目录失败: {e}"))?;
    Ok(dir)
}

fn p2p_state_path() -> Result<PathBuf, String> {
    Ok(p2p_state_dir()?.join(P2P_STATE_FILE))
}

fn read_state_file() -> Result<P2PStateFile, String> {
    let path = p2p_state_path()?;
    if !path.exists() {
        return Ok(P2PStateFile::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取 P2P 状态失败: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析 P2P 状态失败: {e}"))
}

fn write_state_file(state: &P2PStateFile) -> Result<(), String> {
    let path = p2p_state_path()?;
    let raw = serde_json::to_string_pretty(state).map_err(|e| format!("序列化 P2P 状态失败: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("写入 P2P 状态失败: {e}"))?;
    info!(path = %path.to_string_lossy(), "[P2PSync] state file saved");
    Ok(())
}

fn ensure_secret_key(state: &mut P2PStateFile) -> Result<SecretKey, String> {
    if state.secret_key_hex.trim().is_empty() {
        let key = SecretKey::generate();
        state.secret_key_hex = hex::encode(key.to_bytes());
        write_state_file(state)?;
        info!("[P2PSync] generated local iroh secret key");
        return Ok(key);
    }

    let bytes = hex::decode(state.secret_key_hex.trim())
        .map_err(|e| format!("解析 P2P SecretKey 失败: {e}"))?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "P2P SecretKey 长度不正确".to_string())?;
    Ok(SecretKey::from_bytes(&bytes))
}

fn p2p_device_name() -> String {
    let cfg = get_resource_repo_sync_config();
    let configured = cfg.p2p.device_name.trim();
    if !configured.is_empty() {
        return configured.to_string();
    }
    env::var("COMPUTERNAME")
        .or_else(|_| env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Guaxi Device".to_string())
}

fn p2p_config_values() -> (String, u64, bool, u64) {
    let cfg = get_resource_repo_sync_config();
    (
        cfg.p2p.ticket_validity_mode,
        cfg.p2p.ticket_ttl_minutes.max(1),
        cfg.p2p.auto_sync_resource_repo,
        cfg.p2p.sync_interval_minutes.max(30),
    )
}

fn encode_ticket(payload: &P2PTicketPayload) -> Result<String, String> {
    let raw = serde_json::to_vec(payload).map_err(|e| format!("序列化 P2P Ticket 失败: {e}"))?;
    Ok(format!(
        "guaxi-p2p:{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw)
    ))
}

fn decode_ticket(ticket: &str) -> Result<P2PTicketPayload, String> {
    let body = ticket
        .trim()
        .strip_prefix("guaxi-p2p:")
        .ok_or_else(|| "P2P Ticket 格式不正确".to_string())?;
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(body)
        .map_err(|e| format!("解码 P2P Ticket 失败: {e}"))?;
    serde_json::from_slice(&raw).map_err(|e| format!("解析 P2P Ticket 失败: {e}"))
}

fn encode_endpoint_addr(addr: &EndpointAddr) -> Result<String, String> {
    let bytes = serde_json::to_vec(addr).map_err(|e| format!("序列化 iroh 地址失败: {e}"))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

fn decode_endpoint_addr(address_hint: &str) -> Result<EndpointAddr, String> {
    let trimmed = address_hint.trim();
    if trimmed.is_empty() {
        return Err("缺少 iroh 可拨号地址；请在对端启动 P2P 服务后重新生成 Ticket".to_string());
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(trimmed)
        .map_err(|e| format!("解码 iroh 地址失败，请重新配对: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("解析 iroh 地址失败，请重新配对: {e}"))
}

fn endpoint_runtime() -> &'static Mutex<Option<P2PEndpointRuntime>> {
    P2P_ENDPOINT.get_or_init(|| Mutex::new(None))
}

fn upsert_trusted_peer(
    state: &mut P2PStateFile,
    peer_id: String,
    device_id: String,
    device_name: String,
    address_hint: String,
    online_status: String,
    status_detail: String,
) {
    let now = Utc::now().to_rfc3339();
    if let Some(peer) = state.trusted_peers.iter_mut().find(|p| p.peer_id == peer_id) {
        peer.device_id = device_id;
        peer.device_name = device_name;
        if !address_hint.trim().is_empty() {
            peer.address_hint = address_hint;
        }
        peer.online_status = online_status;
        peer.status_detail = status_detail;
        peer.last_checked_at = now;
    } else {
        state.trusted_peers.push(P2PTrustedPeer {
            peer_id,
            device_id,
            device_name,
            added_at: now.clone(),
            address_hint,
            online_status,
            status_detail,
            last_checked_at: now,
        });
    }
}

/// 写入一帧控制消息：4 字节小端长度前缀 + JSON 负载。不调用 finish，方便同一条
/// 流后续继续写文件体。
async fn write_control<T: Serialize>(
    send: &mut SendStream,
    value: &T,
    context: &str,
) -> Result<(), String> {
    let payload = serde_json::to_vec(value).map_err(|e| format!("{context} 序列化失败: {e}"))?;
    let len = payload.len() as u32;
    send.write_all(&len.to_le_bytes())
        .await
        .map_err(|e| format!("{context} 写入长度失败: {e}"))?;
    send.write_all(&payload)
        .await
        .map_err(|e| format!("{context} 写入失败: {e}"))?;
    Ok(())
}

/// 读取一帧控制消息（长度前缀 + JSON），不会越界读取后续的文件体。
async fn read_control<T: DeserializeOwned>(
    recv: &mut RecvStream,
    limit: usize,
    context: &str,
) -> Result<T, String> {
    let mut len_buf = [0_u8; 4];
    recv.read_exact(&mut len_buf)
        .await
        .map_err(|e| format!("{context} 读取长度失败: {e}"))?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > limit {
        return Err(format!("{context} 超过协议帧大小限制"));
    }
    let mut buf = vec![0_u8; len];
    recv.read_exact(&mut buf)
        .await
        .map_err(|e| format!("{context} 读取失败: {e}"))?;
    serde_json::from_slice(&buf).map_err(|e| format!("{context} 解析失败: {e}"))
}

/// 从磁盘按块把文件体写入流，O(1) 内存。
async fn send_file_body(send: &mut SendStream, path: &Path, size: u64) -> Result<(), String> {
    let mut f =
        fs::File::open(path).map_err(|e| format!("打开待发送文件失败 {}: {e}", path.display()))?;
    let mut remaining = size;
    let mut buf = vec![0_u8; P2P_CHUNK_SIZE];
    while remaining > 0 {
        let want = remaining.min(buf.len() as u64) as usize;
        f.read_exact(&mut buf[..want])
            .map_err(|e| format!("读取待发送文件失败 {}: {e}", path.display()))?;
        send.write_all(&buf[..want])
            .await
            .map_err(|e| format!("发送文件体失败 {}: {e}", path.display()))?;
        remaining -= want as u64;
    }
    Ok(())
}

/// 从流按块读取文件体写入临时文件，O(1) 内存。
async fn recv_file_body(recv: &mut RecvStream, out: &mut fs::File, size: u64) -> Result<(), String> {
    let mut remaining = size;
    let mut buf = vec![0_u8; P2P_CHUNK_SIZE];
    while remaining > 0 {
        let want = remaining.min(buf.len() as u64) as usize;
        recv.read_exact(&mut buf[..want])
            .await
            .map_err(|e| format!("接收文件体失败: {e}"))?;
        out.write_all(&buf[..want])
            .map_err(|e| format!("写入临时文件失败: {e}"))?;
        remaining -= want as u64;
    }
    Ok(())
}

/// 处理一条入站 bi 流。每条流以一个 P2PWireRequest 控制帧开头，按类型分派。
async fn handle_incoming_stream(
    endpoint: Endpoint,
    mut send: SendStream,
    mut recv: RecvStream,
) -> Result<(), String> {
    let request: P2PWireRequest = read_control(&mut recv, P2P_FRAME_LIMIT, "P2P 请求").await?;
    let self_address_hint = encode_endpoint_addr(&endpoint.addr()).unwrap_or_default();

    match request {
        P2PWireRequest::Probe {
            device_id,
            device_name,
            peer_id,
            address_hint,
        } => {
            info!(peer_id, device_id, device_name, "[P2PSync] received peer probe");
            let mut state = read_state_file()?;
            upsert_trusted_peer(
                &mut state,
                peer_id,
                device_id,
                device_name,
                address_hint,
                "online".to_string(),
                "在线检测握手成功".to_string(),
            );
            write_state_file(&state)?;
            let response = P2PWireResponse {
                ok: true,
                message: "在线检测成功".to_string(),
                device_id: get_device_id(),
                device_name: p2p_device_name(),
                peer_id: endpoint.id().to_string(),
                address_hint: self_address_hint,
            };
            write_control(&mut send, &response, "P2P 响应").await?;
        }
        P2PWireRequest::Pair {
            ticket_id,
            device_id,
            device_name,
            peer_id,
            address_hint,
        } => {
            info!(
                peer_id, device_id, device_name, ticket_id,
                "[P2PSync] received pairing request"
            );
            let mut state = read_state_file()?;
            upsert_trusted_peer(
                &mut state,
                peer_id,
                device_id,
                device_name,
                address_hint,
                "online".to_string(),
                "配对握手成功".to_string(),
            );
            for pending in &mut state.pending_tickets {
                if pending.ticket_id == ticket_id {
                    pending.used = true;
                }
            }
            write_state_file(&state)?;
            let response = P2PWireResponse {
                ok: true,
                message: "配对成功".to_string(),
                device_id: get_device_id(),
                device_name: p2p_device_name(),
                peer_id: endpoint.id().to_string(),
                address_hint: self_address_hint,
            };
            write_control(&mut send, &response, "P2P 响应").await?;
        }
        P2PWireRequest::Manifest { target } => {
            let response = match target_from_key(&target).and_then(|t| scan_manifest(&t)) {
                Ok(manifest) => P2PManifestResponse {
                    ok: true,
                    message: "ok".to_string(),
                    manifest: Some(manifest),
                },
                Err(e) => P2PManifestResponse {
                    ok: false,
                    message: e,
                    manifest: None,
                },
            };
            write_control(&mut send, &response, "P2P 清单响应").await?;
        }
        P2PWireRequest::PullFile { target, rel } => match serve_pull(&target, &rel) {
            Ok((path, size, blake3, mtime_ms)) => {
                let header = P2PFileHeader {
                    ok: true,
                    message: "ok".to_string(),
                    size,
                    blake3,
                    mtime_ms,
                };
                write_control(&mut send, &header, "P2P 文件头").await?;
                send_file_body(&mut send, &path, size).await?;
            }
            Err(e) => {
                let header = P2PFileHeader {
                    ok: false,
                    message: e,
                    size: 0,
                    blake3: String::new(),
                    mtime_ms: 0,
                };
                write_control(&mut send, &header, "P2P 文件头").await?;
            }
        },
        P2PWireRequest::PushFile {
            target,
            rel,
            size,
            blake3,
            mtime_ms,
        } => {
            let ack = match accept_push(&target, &rel, &mut recv, size, &blake3, mtime_ms).await {
                Ok(message) => P2PAck { ok: true, message },
                Err(e) => P2PAck {
                    ok: false,
                    message: e,
                },
            };
            write_control(&mut send, &ack, "P2P 推送响应").await?;
        }
    }

    send.finish()
        .map_err(|e| format!("P2P 响应完成发送失败: {e}"))?;
    Ok(())
}

fn spawn_p2p_accept_loop(endpoint: Endpoint) {
    tokio::spawn(async move {
        info!("[P2PSync] iroh accept loop started");
        while let Some(incoming) = endpoint.accept().await {
            let endpoint = endpoint.clone();
            tokio::spawn(async move {
                let connection = match incoming.await {
                    Ok(connection) => connection,
                    Err(err) => {
                        info!(error = %err, "[P2PSync] incoming connection failed");
                        return;
                    }
                };
                // 单个连接上可能并发多条 bi 流（清单 + 多个文件），循环 accept 直到连接关闭。
                loop {
                    match connection.accept_bi().await {
                        Ok((send, recv)) => {
                            let endpoint = endpoint.clone();
                            tokio::spawn(async move {
                                if let Err(err) =
                                    handle_incoming_stream(endpoint, send, recv).await
                                {
                                    info!(error = err, "[P2PSync] incoming stream failed");
                                }
                            });
                        }
                        Err(err) => {
                            info!(error = %err, "[P2PSync] connection closed");
                            break;
                        }
                    }
                }
            });
        }
        info!("[P2PSync] iroh accept loop stopped");
    });
}

async fn ensure_p2p_endpoint() -> Result<Endpoint, String> {
    let runtime = endpoint_runtime();
    {
        let guard = runtime.lock().await;
        if let Some(runtime) = guard.as_ref() {
            return Ok(runtime.endpoint.clone());
        }
    }

    let mut state = read_state_file()?;
    let key = ensure_secret_key(&mut state)?;
    write_state_file(&state)?;

    info!("[P2PSync] starting iroh endpoint");
    // presets::N0 配置了 n0 的中继服务器 + DNS 节点发现，既能在局域网内直连打洞，
    // 也能在公网通过中继回退，满足"局域网/互联网都能同步"的要求。
    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(key)
        .alpns(vec![P2P_ALPN.to_vec()])
        .bind()
        .await
        .map_err(|e| format!("启动 iroh P2P Endpoint 失败: {e}"))?;

    // 等待 endpoint 完成中继/直连地址发现，确保后续生成的 Ticket 携带可拨号地址。
    let _ = timeout(StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS), endpoint.online()).await;
    info!(peer_id = endpoint.id().to_string(), "[P2PSync] iroh endpoint ready");
    spawn_p2p_accept_loop(endpoint.clone());

    let mut guard = runtime.lock().await;
    *guard = Some(P2PEndpointRuntime {
        endpoint: endpoint.clone(),
    });
    Ok(endpoint)
}

async fn current_p2p_address_hint(endpoint: &Endpoint) -> Result<String, String> {
    encode_endpoint_addr(&endpoint.addr())
}

async fn connect_peer(endpoint: &Endpoint, peer: &P2PTrustedPeer) -> Result<Connection, String> {
    let addr = decode_endpoint_addr(&peer.address_hint)?;
    let connection = timeout(
        StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS),
        endpoint.connect(addr, P2P_ALPN),
    )
    .await
    .map_err(|_| format!("连接设备 {} 超时", peer.device_name))?
    .map_err(|e| format!("连接设备 {} 失败: {e}", peer.device_name))?;
    Ok(connection)
}

/// 在一条连接上做一次"请求 -> 响应"的控制消息往返。
async fn request_response<Req, Res>(
    connection: &Connection,
    request: &Req,
    response_limit: usize,
) -> Result<Res, String>
where
    Req: Serialize,
    Res: DeserializeOwned,
{
    let (mut send, mut recv) = timeout(
        StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS),
        connection.open_bi(),
    )
    .await
    .map_err(|_| "打开 P2P 传输通道超时".to_string())?
    .map_err(|e| format!("打开 P2P 传输通道失败: {e}"))?;

    write_control(&mut send, request, "P2P 请求").await?;
    send.finish()
        .map_err(|e| format!("P2P 请求完成发送失败: {e}"))?;

    timeout(
        StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS),
        read_control::<Res>(&mut recv, response_limit, "P2P 响应"),
    )
    .await
    .map_err(|_| "等待 P2P 响应超时".to_string())?
}

async fn send_wire_request(
    endpoint: &Endpoint,
    peer: &P2PTrustedPeer,
    request: &P2PWireRequest,
) -> Result<P2PWireResponse, String> {
    let connection = connect_peer(endpoint, peer).await?;
    request_response::<P2PWireRequest, P2PWireResponse>(&connection, request, P2P_FRAME_LIMIT).await
}

fn target_for(kind: P2PSyncTargetKind) -> Result<P2PSyncTarget, String> {
    match kind {
        P2PSyncTargetKind::ResourceRepo => {
            let root = PathBuf::from(get_image_bed_root());
            fs::create_dir_all(&root).map_err(|e| format!("创建资源库目录失败: {e}"))?;
            Ok(P2PSyncTarget {
                kind,
                key: "resource_repo",
                label: "资源库",
                root,
            })
        }
        P2PSyncTargetKind::KnowledgeBase => {
            let root = PathBuf::from(get_knowledge_bases_root());
            fs::create_dir_all(&root).map_err(|e| format!("创建知识库目录失败: {e}"))?;
            Ok(P2PSyncTarget {
                kind,
                key: "knowledge_base",
                label: "知识库",
                root,
            })
        }
    }
}

fn target_from_key(key: &str) -> Result<P2PSyncTarget, String> {
    match key {
        "resource_repo" => target_for(P2PSyncTargetKind::ResourceRepo),
        "knowledge_base" => target_for(P2PSyncTargetKind::KnowledgeBase),
        other => Err(format!("未知的同步目标: {other}")),
    }
}

/// 把相对路径安全地拼到根目录下，拒绝绝对路径与 `..` 穿越。
fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let normalized = rel.replace('\\', "/");
    if normalized.is_empty() {
        return Err("空的相对路径".to_string());
    }
    for comp in normalized.split('/') {
        if comp.is_empty() || comp == "." || comp == ".." {
            return Err(format!("非法的相对路径: {rel}"));
        }
    }
    // 拒绝盘符等绝对路径形式。
    if normalized.starts_with('/') || normalized.contains(':') {
        return Err(format!("非法的相对路径: {rel}"));
    }
    Ok(root.join(PathBuf::from(&normalized)))
}

fn manifest_path(target: &P2PSyncTarget) -> PathBuf {
    target.root.join(P2P_META_DIR).join(P2P_MANIFEST_FILE)
}

fn rel_path(root: &Path, path: &Path) -> Result<String, String> {
    let rel = path
        .strip_prefix(root)
        .map_err(|e| format!("计算相对路径失败: {e}"))?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

fn should_skip(target: &P2PSyncTarget, rel: &str) -> bool {
    let normalized = rel.replace('\\', "/");
    normalized == P2P_META_DIR
        || normalized.starts_with(&format!("{P2P_META_DIR}/"))
        || matches!(target.kind, P2PSyncTargetKind::KnowledgeBase)
            && (normalized == ".git" || normalized.starts_with(".git/"))
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| format!("打开文件失败 {}: {e}", path.display()))?;
    let mut hasher = Hasher::new();
    let mut buf = [0_u8; 64 * 1024];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("读取文件失败 {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn file_mtime_ms(path: &Path) -> Result<i64, String> {
    let modified = fs::metadata(path)
        .map_err(|e| format!("读取文件元数据失败 {}: {e}", path.display()))?
        .modified()
        .map_err(|e| format!("读取文件修改时间失败 {}: {e}", path.display()))?;
    let duration = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("文件修改时间早于 UNIX_EPOCH {}: {e}", path.display()))?;
    Ok(duration.as_millis() as i64)
}

fn scan_manifest(target: &P2PSyncTarget) -> Result<P2PManifest, String> {
    scan_manifest_with_progress(target, None, "", false)
}

fn scan_manifest_with_progress(
    target: &P2PSyncTarget,
    app: Option<&AppHandle>,
    run_id: &str,
    emit_progress: bool,
) -> Result<P2PManifest, String> {
    let device_id = get_device_id();
    let mut total_files = 0_u64;
    let mut total_bytes = 0_u64;

    for entry in WalkDir::new(&target.root).follow_links(false) {
        let entry = entry.map_err(|e| format!("统计目录失败: {e}"))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let rel = rel_path(&target.root, path)?;
        if should_skip(target, &rel) {
            continue;
        }
        let meta =
            fs::metadata(path).map_err(|e| format!("读取文件元数据失败 {}: {e}", path.display()))?;
        total_files += 1;
        total_bytes = total_bytes.saturating_add(meta.len());
    }

    if emit_progress {
        emit_p2p_progress(
            app,
            run_id,
            target,
            "scan",
            "running",
            format!(
                "开始扫描 {}：{} 个文件，{} 字节",
                target.label, total_files, total_bytes
            ),
            0,
            total_files,
            0,
            total_bytes,
            "",
        );
    }

    let mut files = BTreeMap::new();
    let mut scanned_files = 0_u64;
    let mut scanned_bytes = 0_u64;
    for entry in WalkDir::new(&target.root).follow_links(false) {
        let entry = entry.map_err(|e| format!("扫描目录失败: {e}"))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let rel = rel_path(&target.root, path)?;
        if should_skip(target, &rel) {
            continue;
        }
        let meta =
            fs::metadata(path).map_err(|e| format!("读取文件元数据失败 {}: {e}", path.display()))?;
        let hash = hash_file(path)?;
        scanned_files += 1;
        scanned_bytes = scanned_bytes.saturating_add(meta.len());
        files.insert(
            rel.clone(),
            P2PFileEntry {
                size: meta.len(),
                modified_unix_ms: file_mtime_ms(path)?,
                blake3: hash,
            },
        );

        if emit_progress
            && (scanned_files == total_files || scanned_files == 1 || scanned_files % 25 == 0)
        {
            emit_p2p_progress(
                app,
                run_id,
                target,
                "scan",
                "running",
                format!(
                    "正在扫描 {}：{}/{} 个文件",
                    target.label, scanned_files, total_files
                ),
                scanned_files,
                total_files,
                scanned_bytes,
                total_bytes,
                rel,
            );
        }
    }
    info!(
        target = target.key,
        root = %target.root.to_string_lossy(),
        count = files.len(),
        bytes_total = total_bytes,
        "[P2PSync] local manifest scanned"
    );
    if emit_progress {
        emit_p2p_progress(
            app,
            run_id,
            target,
            "scan",
            "success",
            format!("扫描完成：{} 个文件", files.len()),
            scanned_files,
            total_files,
            scanned_bytes,
            total_bytes,
            "",
        );
    }
    Ok(P2PManifest {
        version: 1,
        target: target.key.to_string(),
        device_id,
        updated_at: Utc::now().to_rfc3339(),
        files,
    })
}

fn read_manifest(target: &P2PSyncTarget) -> Result<Option<P2PManifest>, String> {
    let path = manifest_path(target);
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取 P2P manifest 失败: {e}"))?;
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|e| format!("解析 P2P manifest 失败: {e}"))
}

fn write_manifest(target: &P2PSyncTarget, manifest: &P2PManifest) -> Result<(), String> {
    let path = manifest_path(target);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 P2P manifest 目录失败: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("序列化 P2P manifest 失败: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("写入 P2P manifest 失败: {e}"))?;
    info!(target = target.key, path = %path.to_string_lossy(), "[P2PSync] manifest saved");
    Ok(())
}

fn conflict_path(original: &Path, device_name: &str) -> PathBuf {
    let ts = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let stem = original
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = original.extension().and_then(|s| s.to_str()).unwrap_or("");
    let name = if ext.is_empty() {
        format!("{stem}.conflict.{device_name}.{ts}")
    } else {
        format!("{stem}.conflict.{device_name}.{ts}.{ext}")
    };
    original.with_file_name(name)
}

/// 校验临时文件的 BLAKE3，再原子替换目标文件。若目标已存在且与新内容不同，
/// 先把本地副本另存为 `.conflict` 备份，避免覆盖时丢数据。
fn finalize_incoming_file(
    target: &P2PSyncTarget,
    rel: &str,
    tmp: &Path,
    expected_hash: &str,
) -> Result<(), String> {
    let actual = hash_file(tmp)?;
    if actual != expected_hash {
        let _ = fs::remove_file(tmp);
        return Err(format!(
            "P2P 文件校验失败：{} 期望 {} 实际 {}",
            rel, expected_hash, actual
        ));
    }

    let dest = safe_join(&target.root, rel)?;
    if dest.exists() {
        if let Ok(cur) = hash_file(&dest) {
            if cur != expected_hash {
                let backup = conflict_path(&dest, &p2p_device_name());
                match fs::copy(&dest, &backup) {
                    Ok(_) => info!(
                        target = target.key,
                        rel,
                        backup = %backup.to_string_lossy(),
                        "[P2PSync] kept local copy as conflict backup before overwrite"
                    ),
                    Err(e) => info!(
                        target = target.key,
                        rel,
                        error = e.to_string(),
                        "[P2PSync] failed to keep conflict backup"
                    ),
                }
            }
        }
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("创建目标目录失败 {}: {e}", parent.display()))?;
    }
    fs::rename(tmp, &dest).map_err(|e| format!("原子替换目标文件失败 {}: {e}", dest.display()))?;
    Ok(())
}

/// 服务端：定位待拉取文件，返回 (路径, 大小, 哈希, mtime)。
fn serve_pull(target_key: &str, rel: &str) -> Result<(PathBuf, u64, String, i64), String> {
    let target = target_from_key(target_key)?;
    if should_skip(&target, rel) {
        return Err(format!("该文件不在同步范围内: {rel}"));
    }
    let path = safe_join(&target.root, rel)?;
    if !path.is_file() {
        return Err(format!("文件不存在: {rel}"));
    }
    let meta =
        fs::metadata(&path).map_err(|e| format!("读取文件元数据失败 {}: {e}", path.display()))?;
    let hash = hash_file(&path)?;
    let mtime = file_mtime_ms(&path)?;
    Ok((path, meta.len(), hash, mtime))
}

/// 服务端：把对端推送的文件体收进临时文件并原子落盘。
async fn accept_push(
    target_key: &str,
    rel: &str,
    recv: &mut RecvStream,
    size: u64,
    expected_hash: &str,
    _mtime_ms: i64,
) -> Result<String, String> {
    let target = target_from_key(target_key)?;
    if should_skip(&target, rel) {
        return Err(format!("该文件不在同步范围内: {rel}"));
    }
    // 提前校验路径合法性。
    let _ = safe_join(&target.root, rel)?;

    let tmp_dir = target.root.join(P2P_META_DIR).join(P2P_TMP_DIR);
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("创建 P2P 临时目录失败: {e}"))?;
    let tmp = tmp_dir.join(format!("{}.tmp", Uuid::new_v4()));

    {
        let mut out = fs::File::create(&tmp)
            .map_err(|e| format!("创建 P2P 临时文件失败 {}: {e}", tmp.display()))?;
        recv_file_body(recv, &mut out, size).await?;
        out.flush()
            .map_err(|e| format!("刷新 P2P 临时文件失败 {}: {e}", tmp.display()))?;
    }

    finalize_incoming_file(&target, rel, &tmp, expected_hash)?;
    info!(target = target.key, rel, size, "[P2PSync] accepted pushed file");
    Ok(format!("已接收 {rel}"))
}

/// 客户端：请求对端清单。
async fn fetch_remote_manifest(
    connection: &Connection,
    target_key: &str,
) -> Result<P2PManifest, String> {
    let resp: P2PManifestResponse = request_response(
        connection,
        &P2PWireRequest::Manifest {
            target: target_key.to_string(),
        },
        P2P_MANIFEST_FRAME_LIMIT,
    )
    .await?;
    if !resp.ok {
        return Err(resp.message);
    }
    resp.manifest.ok_or_else(|| "对端未返回清单".to_string())
}

/// 客户端：从对端拉取单个文件并原子落盘。
async fn pull_file(
    connection: &Connection,
    target: &P2PSyncTarget,
    rel: &str,
) -> Result<u64, String> {
    let (mut send, mut recv) = timeout(
        StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS),
        connection.open_bi(),
    )
    .await
    .map_err(|_| "打开拉取通道超时".to_string())?
    .map_err(|e| format!("打开拉取通道失败: {e}"))?;

    write_control(
        &mut send,
        &P2PWireRequest::PullFile {
            target: target.key.to_string(),
            rel: rel.to_string(),
        },
        "P2P 拉取请求",
    )
    .await?;
    send.finish()
        .map_err(|e| format!("拉取请求完成发送失败: {e}"))?;

    let header: P2PFileHeader = read_control(&mut recv, P2P_FRAME_LIMIT, "P2P 文件头").await?;
    if !header.ok {
        return Err(header.message);
    }

    let tmp_dir = target.root.join(P2P_META_DIR).join(P2P_TMP_DIR);
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("创建 P2P 临时目录失败: {e}"))?;
    let tmp = tmp_dir.join(format!("{}.tmp", Uuid::new_v4()));
    {
        let mut out = fs::File::create(&tmp)
            .map_err(|e| format!("创建 P2P 临时文件失败 {}: {e}", tmp.display()))?;
        recv_file_body(&mut recv, &mut out, header.size).await?;
        out.flush()
            .map_err(|e| format!("刷新 P2P 临时文件失败 {}: {e}", tmp.display()))?;
    }

    finalize_incoming_file(target, rel, &tmp, &header.blake3)?;
    info!(target = target.key, rel, size = header.size, "[P2PSync] pulled file");
    Ok(header.size)
}

/// 客户端：把本地单个文件推送给对端。
async fn push_file(
    connection: &Connection,
    target: &P2PSyncTarget,
    rel: &str,
) -> Result<u64, String> {
    let path = safe_join(&target.root, rel)?;
    let meta =
        fs::metadata(&path).map_err(|e| format!("读取文件元数据失败 {}: {e}", path.display()))?;
    let hash = hash_file(&path)?;
    let mtime = file_mtime_ms(&path)?;
    let size = meta.len();

    let (mut send, mut recv) = timeout(
        StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS),
        connection.open_bi(),
    )
    .await
    .map_err(|_| "打开推送通道超时".to_string())?
    .map_err(|e| format!("打开推送通道失败: {e}"))?;

    write_control(
        &mut send,
        &P2PWireRequest::PushFile {
            target: target.key.to_string(),
            rel: rel.to_string(),
            size,
            blake3: hash,
            mtime_ms: mtime,
        },
        "P2P 推送请求",
    )
    .await?;
    send_file_body(&mut send, &path, size).await?;
    send.finish()
        .map_err(|e| format!("推送完成发送失败: {e}"))?;

    let ack: P2PAck = read_control(&mut recv, P2P_FRAME_LIMIT, "P2P 推送响应").await?;
    if !ack.ok {
        return Err(ack.message);
    }
    info!(target = target.key, rel, size, "[P2PSync] pushed file");
    Ok(size)
}

fn run_copia_block_delta_self_check() -> Result<(), String> {
    let root = p2p_state_dir()?.join("self-check");
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(&root).map_err(|e| format!("创建 P2P 块级差异自检目录失败: {e}"))?;

    let basis = root.join("basis.txt");
    let source = root.join("source.txt");
    fs::write(&basis, b"guaxi p2p old block\nsame tail")
        .map_err(|e| format!("写入 P2P basis 自检文件失败: {e}"))?;
    fs::write(&source, b"guaxi p2p new block\nsame tail")
        .map_err(|e| format!("写入 P2P source 自检文件失败: {e}"))?;

    // 用 copia 做一次本地块级 signature/delta/patch 往返，确认差异引擎可用。
    let sync = SyncBuilder::new().block_size(P2P_BLOCK_SIZE).build();
    let basis_bytes = fs::read(&basis).map_err(|e| format!("读取 P2P basis 自检文件失败: {e}"))?;
    let source_bytes = fs::read(&source).map_err(|e| format!("读取 P2P source 自检文件失败: {e}"))?;
    let signature = sync
        .signature(Cursor::new(basis_bytes.as_slice()))
        .map_err(|e| format!("生成 copia 块签名失败: {e}"))?;
    let delta = sync
        .delta(Cursor::new(source_bytes.as_slice()), &signature)
        .map_err(|e| format!("生成 copia delta 失败: {e}"))?;
    let mut patched = Vec::with_capacity(source_bytes.len());
    sync.patch(Cursor::new(basis_bytes.as_slice()), &delta, &mut patched)
        .map_err(|e| format!("应用 copia patch 失败: {e}"))?;
    if patched != source_bytes {
        return Err("P2P copia 块级差异自检结果不一致".to_string());
    }

    let _ = fs::remove_dir_all(&root);
    info!("[P2PSync] copia block-delta self check ok");
    Ok(())
}

fn verify_target(target: P2PSyncTarget) -> Result<String, String> {
    let manifest = scan_manifest(&target)?;
    let old = read_manifest(&target)?;
    let mut stats = P2PSyncStats::default();
    match old {
        Some(old) => {
            let keys: BTreeSet<String> = manifest
                .files
                .keys()
                .chain(old.files.keys())
                .cloned()
                .collect();
            for key in keys {
                match (manifest.files.get(&key), old.files.get(&key)) {
                    (Some(a), Some(b)) if a.blake3 == b.blake3 => stats.verified += 1,
                    _ => stats.conflicts += 1,
                }
            }
        }
        None => {
            stats.verified = manifest.files.len() as u32;
        }
    }
    write_manifest(&target, &manifest)?;
    Ok(format!(
        "{} P2P 校验完成：一致 {}，差异 {}",
        target.label, stats.verified, stats.conflicts
    ))
}

/// 更新某个可信设备的在线状态并持久化。
fn update_peer_status(peer_id: &str, online_status: &str, status_detail: &str) {
    if let Ok(mut state) = read_state_file() {
        if let Some(peer) = state.trusted_peers.iter_mut().find(|p| p.peer_id == peer_id) {
            peer.online_status = online_status.to_string();
            peer.status_detail = status_detail.to_string();
            peer.last_checked_at = Utc::now().to_rfc3339();
            let _ = write_state_file(&state);
        }
    }
}

/// 真正执行一次跨设备双向同步：扫描本地清单，对每个可信设备拉取远端清单做 diff，
/// 按 mtime 后写优先把变化的文件拉/推过去；冲突文件保留 `.conflict` 备份。
/// 局域网走直连、互联网走中继，均由 iroh 自动选择。
async fn run_p2p_sync(
    app: Option<&AppHandle>,
    run_id: &str,
    target: P2PSyncTarget,
) -> Result<String, String> {
    emit_p2p_progress(
        app,
        run_id,
        &target,
        "connect",
        "running",
        "确认本机 P2P 身份与块级差异链路",
        0,
        0,
        0,
        0,
        "",
    );
    run_copia_block_delta_self_check()?;

    let endpoint = ensure_p2p_endpoint().await?;
    let _ = timeout(StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS), endpoint.online()).await;

    let peers = read_state_file()?.trusted_peers;
    if peers.is_empty() {
        emit_p2p_progress(
            app,
            run_id,
            &target,
            "complete",
            "waiting",
            "尚无可信设备，请先完成配对",
            0,
            0,
            0,
            0,
            "",
        );
        let manifest = scan_manifest(&target)?;
        write_manifest(&target, &manifest)?;
        return Ok(format!("{} 暂无可信设备，未发生跨设备传输。", target.label));
    }

    let local = scan_manifest_with_progress(&target, app, run_id, true)?;

    let mut stats = P2PSyncStats::default();
    let mut online = 0_u64;
    let mut errors: Vec<String> = Vec::new();

    for peer in &peers {
        emit_p2p_progress(
            app,
            run_id,
            &target,
            "connect",
            "running",
            format!("正在连接 {}", peer.device_name),
            online,
            peers.len() as u64,
            0,
            0,
            "",
        );

        let connection = match connect_peer(&endpoint, peer).await {
            Ok(c) => c,
            Err(e) => {
                update_peer_status(&peer.peer_id, "offline", &e);
                errors.push(format!("{}: {}", peer.device_name, e));
                continue;
            }
        };
        online += 1;
        update_peer_status(&peer.peer_id, "online", "同步连接成功");

        let remote = match fetch_remote_manifest(&connection, target.key).await {
            Ok(m) => m,
            Err(e) => {
                errors.push(format!("{} 获取清单失败: {}", peer.device_name, e));
                connection.close(0u32.into(), b"manifest failed");
                continue;
            }
        };

        // 计算 diff：以 mtime 较新者为准（last-writer-wins）。哈希相同直接跳过。
        let keys: BTreeSet<String> = local
            .files
            .keys()
            .chain(remote.files.keys())
            .cloned()
            .collect();
        let mut pulls: Vec<String> = Vec::new();
        let mut pushes: Vec<String> = Vec::new();
        for key in keys {
            match (local.files.get(&key), remote.files.get(&key)) {
                (Some(l), Some(r)) if l.blake3 == r.blake3 => stats.skipped += 1,
                (Some(l), Some(r)) => {
                    if r.modified_unix_ms > l.modified_unix_ms {
                        pulls.push(key);
                    } else {
                        pushes.push(key);
                    }
                }
                (None, Some(_)) => pulls.push(key),
                (Some(_), None) => pushes.push(key),
                (None, None) => {}
            }
        }

        let plan_total = (pulls.len() + pushes.len()) as u64;
        emit_p2p_progress(
            app,
            run_id,
            &target,
            "plan",
            "success",
            format!(
                "与 {} 的同步计划：拉取 {} 个，推送 {} 个",
                peer.device_name,
                pulls.len(),
                pushes.len()
            ),
            0,
            plan_total,
            0,
            0,
            "",
        );

        let mut done = 0_u64;
        for rel in &pulls {
            emit_p2p_progress(
                app,
                run_id,
                &target,
                "transfer",
                "running",
                format!("从 {} 拉取 {}", peer.device_name, rel),
                done,
                plan_total,
                0,
                0,
                rel.clone(),
            );
            match pull_file(&connection, &target, rel).await {
                Ok(_) => stats.downloaded += 1,
                Err(e) => errors.push(format!("拉取 {} 失败: {}", rel, e)),
            }
            done += 1;
        }
        for rel in &pushes {
            emit_p2p_progress(
                app,
                run_id,
                &target,
                "transfer",
                "running",
                format!("向 {} 推送 {}", peer.device_name, rel),
                done,
                plan_total,
                0,
                0,
                rel.clone(),
            );
            match push_file(&connection, &target, rel).await {
                Ok(_) => stats.uploaded += 1,
                Err(e) => errors.push(format!("推送 {} 失败: {}", rel, e)),
            }
            done += 1;
        }

        // 主动关闭连接，触发对端 accept_bi 退出。
        connection.close(0u32.into(), b"sync done");
    }

    // 同步后重扫一次并把清单作为新的对比基准持久化。
    let final_manifest = scan_manifest(&target)?;
    write_manifest(&target, &final_manifest)?;

    let status = if online == 0 {
        "error"
    } else if errors.is_empty() {
        "success"
    } else {
        "skipped"
    };
    emit_p2p_progress(
        app,
        run_id,
        &target,
        "complete",
        status,
        format!(
            "{} 同步完成：下载 {}，上传 {}，跳过 {}，在线设备 {}/{}",
            target.label,
            stats.downloaded,
            stats.uploaded,
            stats.skipped,
            online,
            peers.len()
        ),
        stats.downloaded as u64 + stats.uploaded as u64,
        stats.downloaded as u64 + stats.uploaded as u64,
        0,
        0,
        "",
    );

    if online == 0 {
        return Err(format!(
            "{} 同步失败：没有任何可信设备在线。{}",
            target.label,
            errors.first().cloned().unwrap_or_default()
        ));
    }

    let mut summary = format!(
        "{} P2P 同步完成：下载 {}，上传 {}，跳过 {}，在线设备 {}/{}。",
        target.label,
        stats.downloaded,
        stats.uploaded,
        stats.skipped,
        online,
        peers.len()
    );
    if !errors.is_empty() {
        summary.push_str(&format!("部分项未完成：{}", errors.join("；")));
    }
    Ok(summary)
}

#[tauri::command]
pub async fn p2p_sync_status() -> Result<P2PStatus, String> {
    let cfg = get_resource_repo_sync_config();
    let mut state = read_state_file()?;
    let key = ensure_secret_key(&mut state)?;
    let peer_id = key.public().to_string();
    let (ticket_validity_mode, ticket_ttl_minutes, auto_sync_resource_repo, sync_interval_minutes) =
        p2p_config_values();
    Ok(P2PStatus {
        enabled: cfg.provider.trim().eq_ignore_ascii_case("p2p"),
        provider: cfg.provider,
        device_id: get_device_id(),
        device_name: p2p_device_name(),
        peer_id,
        ticket_validity_mode,
        ticket_ttl_minutes,
        auto_sync_resource_repo,
        sync_interval_minutes,
        trusted_peers: state.trusted_peers,
        iroh_version: "0.98.2".to_string(),
    })
}

#[tauri::command]
pub async fn p2p_sync_test_connection() -> Result<String, String> {
    let mut state = read_state_file()?;
    ensure_secret_key(&mut state)?;
    write_state_file(&state)?;
    let endpoint = ensure_p2p_endpoint().await?;
    let address_hint = current_p2p_address_hint(&endpoint).await?;
    run_copia_block_delta_self_check()?;
    let peer_id = endpoint.id().to_string();
    let peers = state.trusted_peers.len();
    info!(
        peer_id,
        peers,
        address_hint_len = address_hint.len(),
        "[P2PSync] iroh endpoint and copia delta ready"
    );
    Ok(format!(
        "P2P Endpoint 可用：Peer ID {}，可信设备 {} 个，地址提示 {} 字节",
        peer_id,
        peers,
        address_hint.len()
    ))
}

#[tauri::command]
pub async fn p2p_sync_create_ticket() -> Result<P2PTicketResponse, String> {
    let mut state = read_state_file()?;
    let endpoint = ensure_p2p_endpoint().await?;
    let peer_id = endpoint.id().to_string();
    let address_hint = current_p2p_address_hint(&endpoint).await?;
    let (mode, ttl, _, _) = p2p_config_values();
    let created_at = Utc::now();
    let expires_at = if mode.trim().eq_ignore_ascii_case("permanent") {
        None
    } else {
        Some((created_at + Duration::minutes(ttl as i64)).to_rfc3339())
    };

    let payload = P2PTicketPayload {
        version: 1,
        ticket_id: Uuid::new_v4().to_string(),
        peer_id: peer_id.clone(),
        device_id: get_device_id(),
        device_name: p2p_device_name(),
        created_at: created_at.to_rfc3339(),
        expires_at: expires_at.clone(),
        address_hint,
    };
    let ticket = encode_ticket(&payload)?;
    state.pending_tickets.push(P2PPendingTicket {
        ticket_id: payload.ticket_id.clone(),
        ticket: ticket.clone(),
        created_at: payload.created_at.clone(),
        expires_at: expires_at.clone(),
        used: false,
    });
    write_state_file(&state)?;
    info!(
        peer_id,
        ticket_id = payload.ticket_id,
        expires_at = expires_at.clone().unwrap_or_else(|| "permanent".to_string()),
        address_hint_len = payload.address_hint.len(),
        "[P2PSync] created pairing ticket"
    );
    Ok(P2PTicketResponse {
        ticket,
        ticket_id: payload.ticket_id,
        peer_id,
        device_id: payload.device_id,
        device_name: payload.device_name,
        created_at: payload.created_at,
        expires_at,
    })
}

#[tauri::command]
pub async fn p2p_sync_join_ticket(ticket: String) -> Result<String, String> {
    let payload = decode_ticket(&ticket)?;
    if let Some(exp) = &payload.expires_at {
        let exp = DateTime::parse_from_rfc3339(exp)
            .map_err(|e| format!("解析 Ticket 过期时间失败: {e}"))?
            .with_timezone(&Utc);
        if Utc::now() > exp {
            return Err("P2P Ticket 已过期".to_string());
        }
    }
    if payload.address_hint.trim().is_empty() {
        return Err("P2P Ticket 缺少 iroh 可拨号地址；请在对端升级后重新生成 Ticket".to_string());
    }

    let mut state = read_state_file()?;
    let endpoint = ensure_p2p_endpoint().await?;
    let local_address_hint = current_p2p_address_hint(&endpoint).await?;
    let peer = P2PTrustedPeer {
        peer_id: payload.peer_id.clone(),
        device_id: payload.device_id.clone(),
        device_name: payload.device_name.clone(),
        added_at: Utc::now().to_rfc3339(),
        address_hint: payload.address_hint.clone(),
        online_status: "unknown".to_string(),
        status_detail: "配对握手中".to_string(),
        last_checked_at: String::new(),
    };
    let response = send_wire_request(
        &endpoint,
        &peer,
        &P2PWireRequest::Pair {
            ticket_id: payload.ticket_id.clone(),
            device_id: get_device_id(),
            device_name: p2p_device_name(),
            peer_id: endpoint.id().to_string(),
            address_hint: local_address_hint,
        },
    )
    .await?;
    if !response.ok {
        return Err(format!("P2P 配对被对端拒绝: {}", response.message));
    }

    upsert_trusted_peer(
        &mut state,
        response.peer_id.clone(),
        response.device_id.clone(),
        response.device_name.clone(),
        response.address_hint.clone(),
        "online".to_string(),
        "配对握手成功".to_string(),
    );
    write_state_file(&state)?;
    info!(
        peer_id = response.peer_id,
        device_id = response.device_id,
        device_name = response.device_name,
        ticket_id = payload.ticket_id,
        "[P2PSync] trusted peer added after live pairing"
    );
    Ok(format!("已连接并信任设备：{}", payload.device_name))
}

#[tauri::command]
pub async fn p2p_sync_list_peers() -> Result<Vec<P2PTrustedPeer>, String> {
    Ok(read_state_file()?.trusted_peers)
}

#[tauri::command]
pub async fn p2p_sync_probe_peers() -> Result<Vec<P2PTrustedPeer>, String> {
    let mut state = read_state_file()?;
    let mut changed = false;
    let checked_at = Utc::now().to_rfc3339();

    // 先确认本机 P2P 基础能力正常，否则远端检测没有意义。
    let endpoint = ensure_p2p_endpoint().await?;
    let local_address_hint = current_p2p_address_hint(&endpoint).await?;
    run_copia_block_delta_self_check()?;

    for peer in &mut state.trusted_peers {
        peer.last_checked_at = checked_at.clone();
        if peer.address_hint.trim().is_empty() {
            peer.online_status = "unknown".to_string();
            peer.status_detail =
                "缺少 iroh 可拨号地址；请使用新版 Ticket 重新配对后再检测在线状态".to_string();
            changed = true;
            info!(
                peer_id = peer.peer_id,
                device_name = peer.device_name,
                "[P2PSync] peer probe skipped because address hint is empty"
            );
            continue;
        }

        let request = P2PWireRequest::Probe {
            device_id: get_device_id(),
            device_name: p2p_device_name(),
            peer_id: endpoint.id().to_string(),
            address_hint: local_address_hint.clone(),
        };
        match send_wire_request(&endpoint, peer, &request).await {
            Ok(response) if response.ok => {
                peer.peer_id = response.peer_id;
                peer.device_id = response.device_id;
                peer.device_name = response.device_name;
                if !response.address_hint.trim().is_empty() {
                    peer.address_hint = response.address_hint;
                }
                peer.online_status = "online".to_string();
                peer.status_detail = "在线检测握手成功".to_string();
                info!(
                    peer_id = peer.peer_id,
                    device_name = peer.device_name,
                    "[P2PSync] peer probe online"
                );
            }
            Ok(response) => {
                peer.online_status = "offline".to_string();
                peer.status_detail = format!("在线检测被对端拒绝: {}", response.message);
                info!(
                    peer_id = peer.peer_id,
                    device_name = peer.device_name,
                    detail = peer.status_detail,
                    "[P2PSync] peer probe rejected"
                );
            }
            Err(err) => {
                peer.online_status = "offline".to_string();
                peer.status_detail = err;
                info!(
                    peer_id = peer.peer_id,
                    device_name = peer.device_name,
                    detail = peer.status_detail,
                    "[P2PSync] peer probe offline"
                );
            }
        }
        changed = true;
    }

    if changed {
        write_state_file(&state)?;
    }
    Ok(state.trusted_peers)
}

#[tauri::command]
pub async fn p2p_sync_remove_peer(peer_id: String) -> Result<String, String> {
    let mut state = read_state_file()?;
    let before = state.trusted_peers.len();
    state.trusted_peers.retain(|p| p.peer_id != peer_id);
    write_state_file(&state)?;
    Ok(format!(
        "已移除 {} 个设备",
        before.saturating_sub(state.trusted_peers.len())
    ))
}

#[tauri::command]
pub async fn p2p_sync_resource_repo_now(app: AppHandle) -> Result<String, String> {
    let run_id = Uuid::new_v4().to_string();
    let target = target_for(P2PSyncTargetKind::ResourceRepo)?;
    info!(run_id, target = target.key, "[P2PSync] resource repo sync requested");
    run_p2p_sync(Some(&app), &run_id, target).await
}

#[tauri::command]
pub async fn p2p_sync_verify_resource_repo() -> Result<String, String> {
    verify_target(target_for(P2PSyncTargetKind::ResourceRepo)?)
}

#[tauri::command]
pub async fn p2p_sync_knowledge_base_now(app: AppHandle) -> Result<String, String> {
    let run_id = Uuid::new_v4().to_string();
    let target = target_for(P2PSyncTargetKind::KnowledgeBase)?;
    info!(run_id, target = target.key, "[P2PSync] knowledge base emergency sync requested");
    run_p2p_sync(Some(&app), &run_id, target).await
}

#[tauri::command]
pub async fn p2p_sync_verify_knowledge_base() -> Result<String, String> {
    verify_target(target_for(P2PSyncTargetKind::KnowledgeBase)?)
}

/// 应用启动时调用：若同步后端是 P2P，则提前绑定 endpoint 并启动 accept loop，
/// 使本机即便没有打开设置页也能接收对端的配对/探测/同步请求。
pub async fn p2p_autostart_if_enabled() {
    let cfg = get_resource_repo_sync_config();
    if !cfg.provider.trim().eq_ignore_ascii_case("p2p") {
        return;
    }
    match ensure_p2p_endpoint().await {
        Ok(endpoint) => info!(
            peer_id = endpoint.id().to_string(),
            "[P2PSync] endpoint auto-started at app launch"
        ),
        Err(e) => info!(error = e, "[P2PSync] endpoint auto-start failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_roundtrip_preserves_expiry_policy() {
        let payload = P2PTicketPayload {
            version: 1,
            ticket_id: "ticket".to_string(),
            peer_id: "peer".to_string(),
            device_id: "device".to_string(),
            device_name: "Laptop".to_string(),
            created_at: "2026-06-05T00:00:00Z".to_string(),
            expires_at: None,
            address_hint: String::new(),
        };
        let encoded = encode_ticket(&payload).expect("encode ticket");
        let decoded = decode_ticket(&encoded).expect("decode ticket");
        assert_eq!(decoded.expires_at, None);
        assert_eq!(decoded.device_name, "Laptop");
    }

    #[test]
    fn skip_knowledge_git_and_p2p_meta() {
        let target = P2PSyncTarget {
            kind: P2PSyncTargetKind::KnowledgeBase,
            key: "knowledge_base",
            label: "知识库",
            root: PathBuf::from("x"),
        };
        assert!(should_skip(&target, ".git/config"));
        assert!(should_skip(&target, ".guaxi-p2p/manifest.json"));
        assert!(!should_skip(&target, "notes/a.md"));
    }

    #[test]
    fn safe_join_rejects_traversal() {
        let root = PathBuf::from("root");
        assert!(safe_join(&root, "a/b.txt").is_ok());
        assert!(safe_join(&root, "../escape").is_err());
        assert!(safe_join(&root, "/abs").is_err());
        assert!(safe_join(&root, "a/../b").is_err());
        assert!(safe_join(&root, "C:/x").is_err());
    }

    #[test]
    fn finalize_incoming_file_replaces_atomically_and_verifies() {
        let root = env::temp_dir().join(format!("guaxi-p2p-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create root");
        let target = P2PSyncTarget {
            kind: P2PSyncTargetKind::ResourceRepo,
            key: "resource_repo",
            label: "资源库",
            root: root.clone(),
        };
        let tmp = root.join("incoming.tmp");
        fs::write(&tmp, b"hello new block\nsame tail").expect("write tmp");
        let expected = hash_file(&tmp).expect("hash tmp");

        finalize_incoming_file(&target, "a.txt", &tmp, &expected).expect("finalize");

        let content = fs::read(root.join("a.txt")).expect("read dest");
        assert_eq!(content, b"hello new block\nsame tail");
        assert!(!tmp.exists(), "tmp should be renamed away");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn copia_self_check_passes() {
        run_copia_block_delta_self_check().expect("copia self check");
    }
}
