use crate::commands::settingview::{
    get_device_id, get_image_bed_root, get_knowledge_bases_root, get_libraries_root,
    get_resource_repo_sync_config, get_slidev_repo_root, get_template_repo_path,
};
use base64::Engine as _;
use blake3::Hasher;
use chrono::{DateTime, Duration, Utc};
use copia::{Delta, Signature, Sync, SyncBuilder};
use hex;
use iroh::{
    endpoint::{
        presets, Connection, ConnectOptions, IdleTimeout, QuicTransportConfig, RecvStream,
        SendStream,
    },
    Endpoint, EndpointAddr, RelayMode, RelayUrl, SecretKey,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{Cursor, Read, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration as StdDuration, Instant};
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
const P2P_ALPN: &[u8] = b"guaxi/p2p-sync/2";
const P2P_ONLINE_TIMEOUT_SECS: u64 = 12;
const P2P_FRAME_LIMIT: usize = 1024 * 1024;
/// 数据库记录 bundle 可能远大于普通控制帧，用更大的读取上限（64MB）。
const P2P_BUNDLE_FRAME_LIMIT: usize = 64 * 1024 * 1024;
/// 清单可能包含上万个文件条目，单帧上限放宽到 256MiB。
const P2P_MANIFEST_FRAME_LIMIT: usize = 256 * 1024 * 1024;
/// 块级 signature/delta 帧上限，单文件签名/差异不应超过此值。
const P2P_DELTA_FRAME_LIMIT: usize = 256 * 1024 * 1024;
/// 文件体按固定块流式收发，避免把整文件读进内存。
const P2P_CHUNK_SIZE: usize = 256 * 1024;
/// 单个文件传输失败后的最大重试次数（含重连）。
const P2P_TRANSFER_MAX_RETRIES: u32 = 3;
/// 低于此大小的文件不做块级 delta，直接整文件续传（delta 开销不划算）。
const P2P_DELTA_MIN_SIZE: u64 = 256 * 1024;
const RESOURCE_REPO_SYNC_PROGRESS_EVENT: &str = "resource_repo_sync_progress";

struct P2PEndpointRuntime {
    endpoint: Endpoint,
}

static P2P_ENDPOINT: OnceLock<Mutex<Option<P2PEndpointRuntime>>> = OnceLock::new();

/// 正在被请求取消的同步 run_id 集合。同步循环在检查点轮询该集合实现"终止"。
static SYNC_CANCEL: OnceLock<std::sync::Mutex<HashSet<String>>> = OnceLock::new();

fn sync_cancel_set() -> &'static std::sync::Mutex<HashSet<String>> {
    SYNC_CANCEL.get_or_init(|| std::sync::Mutex::new(HashSet::new()))
}

/// 请求取消指定 run_id 的同步。
fn request_sync_cancel(run_id: &str) {
    if let Ok(mut set) = sync_cancel_set().lock() {
        set.insert(run_id.to_string());
    }
}

/// 查询某 run_id 是否被请求取消。
fn is_sync_cancelled(run_id: &str) -> bool {
    sync_cancel_set()
        .lock()
        .map(|set| set.contains(run_id))
        .unwrap_or(false)
}

/// 同步结束时清理取消标记。
fn clear_sync_cancel(run_id: &str) {
    if let Ok(mut set) = sync_cancel_set().lock() {
        set.remove(run_id);
    }
}

#[derive(Debug, Clone)]
struct P2PSyncTarget {
    key: String,
    label: String,
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

/// 用于全网状信任传播的精简设备条目：随握手/同步响应携带，让各节点自动互认。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct P2PPeerHint {
    peer_id: String,
    device_id: String,
    device_name: String,
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
        #[serde(default)]
        known_peers: Vec<P2PPeerHint>,
    },
    Pair {
        ticket_id: String,
        device_id: String,
        device_name: String,
        peer_id: String,
        address_hint: String,
        #[serde(default)]
        known_peers: Vec<P2PPeerHint>,
    },
    /// 请求对端返回某个同步目标的清单（用于 diff）。
    Manifest {
        target: String,
    },
    /// 从对端拉取一个文件，响应为文件头 + 原始字节流。`offset` 支持断点续传：
    /// 对端从该偏移开始发送剩余字节（0 表示整文件）。
    PullFile {
        target: String,
        rel: String,
        #[serde(default)]
        offset: u64,
    },
    /// 块级增量：请求对端基于客户端提供的 basis 签名，返回 delta（仅变化块）。
    PullDelta {
        target: String,
        rel: String,
        signature: Signature,
    },
    /// 向对端推送一个文件，请求帧后紧跟原始字节流。
    PushFile {
        target: String,
        rel: String,
        size: u64,
        blake3: String,
        mtime_ms: i64,
    },
    /// 请求对端导出某 knowledge_repo 的数据库记录 bundle（JSON）。
    RepoBundle {
        knowledge_repo: String,
    },
    /// 把本机的数据库记录 bundle 推送给对端合并。
    PushRepoBundle {
        knowledge_repo: String,
        bundle_json: String,
    },
    /// 请求对端导出某 kgraph 知识库的人工知识层 bundle(JSON)。
    KgManualBundle {
        kb_name: String,
    },
    /// 把本机的 kgraph 人工层 bundle 推送给对端按行合并(机制B,无墓碑)。
    PushKgManualBundle {
        kb_name: String,
        bundle_json: String,
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
    #[serde(default)]
    known_peers: Vec<P2PPeerHint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PManifestResponse {
    ok: bool,
    message: String,
    manifest: Option<P2PManifest>,
}

/// 数据库记录 bundle 响应：bundle 以 JSON 字符串内嵌，避免跨模块暴露结构体。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PRepoBundleResponse {
    ok: bool,
    message: String,
    bundle_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PFileHeader {
    ok: bool,
    message: String,
    size: u64,
    blake3: String,
    mtime_ms: i64,
}

/// 块级增量响应头：携带目标文件的完整大小/哈希与序列化后的 delta。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PDeltaResponse {
    ok: bool,
    message: String,
    size: u64,
    blake3: String,
    mtime_ms: i64,
    delta: Option<Delta>,
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

/// 读取用户配置的传输并发度（每条连接同时打开的流数），下限 1，上限 32。
fn p2p_transfer_concurrency() -> usize {
    let cfg = get_resource_repo_sync_config();
    (cfg.p2p.transfer_concurrency as usize).clamp(1, 32)
}

/// 读取用户配置的 QUIC idle 超时（秒），下限 15s。iroh 默认仅 15s，长传输时
/// 容易在阻塞 I/O 抖动下被判死；这里默认放宽到 30s。
fn p2p_idle_timeout_secs() -> u64 {
    let cfg = get_resource_repo_sync_config();
    cfg.p2p.idle_timeout_secs.max(15)
}

fn p2p_block_delta_enabled() -> bool {
    get_resource_repo_sync_config().p2p.enable_block_delta
}

fn p2p_auto_propagate_trust() -> bool {
    get_resource_repo_sync_config().p2p.auto_propagate_trust
}

/// 根据用户配置解析中继模式：
/// - "lan_only" → 禁用中继（仅局域网直连/打洞）
/// - "custom"   → 使用用户填写的自部署中继（解析失败的逐个跳过；全部无效则回退默认）
/// - 其它       → None，表示沿用 presets::N0 的默认 n0 中继
fn p2p_relay_mode_override() -> Option<RelayMode> {
    let cfg = get_resource_repo_sync_config();
    let mode = cfg.p2p.relay_mode.trim().to_lowercase();
    match mode.as_str() {
        "lan_only" => {
            info!("[P2PSync] relay mode: LAN only (relays disabled)");
            Some(RelayMode::Disabled)
        }
        "custom" => {
            let urls: Vec<RelayUrl> = cfg
                .p2p
                .custom_relay_urls
                .split(|c| c == '\n' || c == ',' || c == ';')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .filter_map(|s| match s.parse::<RelayUrl>() {
                    Ok(u) => Some(u),
                    Err(e) => {
                        warn_p2p_relay(s, &format!("{e}"));
                        None
                    }
                })
                .collect();
            if urls.is_empty() {
                info!("[P2PSync] relay mode: custom but no valid URL, falling back to default n0 relays");
                None
            } else {
                info!(count = urls.len(), "[P2PSync] relay mode: custom relays");
                Some(RelayMode::custom(urls))
            }
        }
        _ => None,
    }
}

fn warn_p2p_relay(url: &str, err: &str) {
    info!(url, error = err, "[P2PSync] invalid custom relay URL, skipped");
}

/// 构造统一的 QUIC 传输参数：拉长 idle 超时、保持 5s 心跳、在 Windows 上关闭
/// UDP GSO 分段（规避 `sendmsg 10040` WSAEMSGSIZE）。连接两端需使用一致配置。
fn build_transport_config() -> QuicTransportConfig {
    let idle = StdDuration::from_secs(p2p_idle_timeout_secs());
    let mut builder = QuicTransportConfig::builder();
    if let Ok(idle_timeout) = IdleTimeout::try_from(idle) {
        builder = builder.max_idle_timeout(Some(idle_timeout));
        builder = builder.default_path_max_idle_timeout(idle);
    }
    builder = builder.keep_alive_interval(StdDuration::from_secs(5));
    #[cfg(windows)]
    {
        builder = builder.enable_segmentation_offload(false);
    }
    builder.build()
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

/// 收集本机已知的可信设备，作为信任传播（gossip）的负载。排除某个 peer_id
/// （通常是消息对端自己）以及空地址条目。
fn collect_peer_hints(state: &P2PStateFile, exclude_peer_id: &str) -> Vec<P2PPeerHint> {
    if !p2p_auto_propagate_trust() {
        return Vec::new();
    }
    state
        .trusted_peers
        .iter()
        .filter(|p| p.peer_id != exclude_peer_id && !p.address_hint.trim().is_empty())
        .map(|p| P2PPeerHint {
            peer_id: p.peer_id.clone(),
            device_id: p.device_id.clone(),
            device_name: p.device_name.clone(),
            address_hint: p.address_hint.clone(),
        })
        .collect()
}

/// 把对端传播过来的设备提示合并进本地可信列表。跳过本机自身与无地址条目；
/// 已存在的设备仅在本地缺地址时补全，不覆盖在线状态。返回新增的设备数。
fn merge_peer_hints(state: &mut P2PStateFile, hints: &[P2PPeerHint], self_peer_id: &str) -> usize {
    if !p2p_auto_propagate_trust() {
        return 0;
    }
    let now = Utc::now().to_rfc3339();
    let mut added = 0;
    for hint in hints {
        if hint.peer_id.trim().is_empty()
            || hint.peer_id == self_peer_id
            || hint.address_hint.trim().is_empty()
        {
            continue;
        }
        if let Some(existing) = state
            .trusted_peers
            .iter_mut()
            .find(|p| p.peer_id == hint.peer_id)
        {
            if existing.address_hint.trim().is_empty() {
                existing.address_hint = hint.address_hint.clone();
            }
            if existing.device_name.trim().is_empty() {
                existing.device_name = hint.device_name.clone();
            }
            if existing.device_id.trim().is_empty() {
                existing.device_id = hint.device_id.clone();
            }
        } else {
            state.trusted_peers.push(P2PTrustedPeer {
                peer_id: hint.peer_id.clone(),
                device_id: hint.device_id.clone(),
                device_name: hint.device_name.clone(),
                added_at: now.clone(),
                address_hint: hint.address_hint.clone(),
                online_status: "unknown".to_string(),
                status_detail: "经其他设备传播自动发现".to_string(),
                last_checked_at: String::new(),
            });
            added += 1;
        }
    }
    added
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

/// 从磁盘按块把文件体写入流，O(1) 内存。`offset` 支持断点续传：从该字节偏移
/// 开始发送，发送 `size - offset` 字节。磁盘读取走 tokio 异步文件 I/O，不阻塞
/// 运行时线程（旧实现用同步 read 会饿死 QUIC 心跳，导致 connection lost）。
async fn send_file_body(
    send: &mut SendStream,
    path: &Path,
    size: u64,
    offset: u64,
) -> Result<(), String> {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    let mut f = tokio::fs::File::open(path)
        .await
        .map_err(|e| format!("打开待发送文件失败 {}: {e}", path.display()))?;
    if offset > 0 {
        f.seek(SeekFrom::Start(offset))
            .await
            .map_err(|e| format!("定位待发送文件失败 {}: {e}", path.display()))?;
    }
    let mut remaining = size.saturating_sub(offset);
    let mut buf = vec![0_u8; P2P_CHUNK_SIZE];
    while remaining > 0 {
        let want = remaining.min(buf.len() as u64) as usize;
        f.read_exact(&mut buf[..want])
            .await
            .map_err(|e| format!("读取待发送文件失败 {}: {e}", path.display()))?;
        send.write_all(&buf[..want])
            .await
            .map_err(|e| format!("发送文件体失败 {}: {e}", path.display()))?;
        remaining -= want as u64;
    }
    Ok(())
}

/// 从流按块读取文件体写入文件，O(1) 内存，异步落盘。
async fn recv_file_body(
    recv: &mut RecvStream,
    out: &mut tokio::fs::File,
    size: u64,
) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    let mut remaining = size;
    let mut buf = vec![0_u8; P2P_CHUNK_SIZE];
    while remaining > 0 {
        let want = remaining.min(buf.len() as u64) as usize;
        recv.read_exact(&mut buf[..want])
            .await
            .map_err(|e| format!("接收文件体失败: {e}"))?;
        out.write_all(&buf[..want])
            .await
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
    let self_peer_id = endpoint.id().to_string();

    match request {
        P2PWireRequest::Probe {
            device_id,
            device_name,
            peer_id,
            address_hint,
            known_peers,
        } => {
            info!(peer_id, device_id, device_name, "[P2PSync] received peer probe");
            let mut state = read_state_file()?;
            upsert_trusted_peer(
                &mut state,
                peer_id.clone(),
                device_id,
                device_name,
                address_hint,
                "online".to_string(),
                "在线检测握手成功".to_string(),
            );
            let added = merge_peer_hints(&mut state, &known_peers, &self_peer_id);
            if added > 0 {
                info!(added, "[P2PSync] learned peers via probe propagation");
            }
            let hints = collect_peer_hints(&state, &peer_id);
            write_state_file(&state)?;
            let response = P2PWireResponse {
                ok: true,
                message: "在线检测成功".to_string(),
                device_id: get_device_id(),
                device_name: p2p_device_name(),
                peer_id: self_peer_id.clone(),
                address_hint: self_address_hint,
                known_peers: hints,
            };
            write_control(&mut send, &response, "P2P 响应").await?;
        }
        P2PWireRequest::Pair {
            ticket_id,
            device_id,
            device_name,
            peer_id,
            address_hint,
            known_peers,
        } => {
            info!(
                peer_id, device_id, device_name, ticket_id,
                "[P2PSync] received pairing request"
            );
            let mut state = read_state_file()?;
            upsert_trusted_peer(
                &mut state,
                peer_id.clone(),
                device_id,
                device_name,
                address_hint,
                "online".to_string(),
                "配对握手成功".to_string(),
            );
            merge_peer_hints(&mut state, &known_peers, &self_peer_id);
            for pending in &mut state.pending_tickets {
                if pending.ticket_id == ticket_id {
                    pending.used = true;
                }
            }
            let hints = collect_peer_hints(&state, &peer_id);
            write_state_file(&state)?;
            let response = P2PWireResponse {
                ok: true,
                message: "配对成功".to_string(),
                device_id: get_device_id(),
                device_name: p2p_device_name(),
                peer_id: self_peer_id.clone(),
                address_hint: self_address_hint,
                known_peers: hints,
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
        P2PWireRequest::PullFile {
            target,
            rel,
            offset,
        } => match serve_pull(&target, &rel).await {
            Ok((path, size, blake3, mtime_ms)) => {
                // 续传偏移越界（对端文件已变）时退回从 0 发送整文件。
                let start = if offset <= size { offset } else { 0 };
                let header = P2PFileHeader {
                    ok: true,
                    message: "ok".to_string(),
                    size,
                    blake3,
                    mtime_ms,
                };
                write_control(&mut send, &header, "P2P 文件头").await?;
                send_file_body(&mut send, &path, size, start).await?;
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
        P2PWireRequest::PullDelta {
            target,
            rel,
            signature,
        } => {
            let response = serve_pull_delta(&target, &rel, &signature).await;
            write_control(&mut send, &response, "P2P 增量响应").await?;
        }
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
        P2PWireRequest::RepoBundle { knowledge_repo } => {
            let response = match collect_repo_bundle_json(&knowledge_repo) {
                Ok(json) => P2PRepoBundleResponse {
                    ok: true,
                    message: "ok".to_string(),
                    bundle_json: Some(json),
                },
                Err(e) => P2PRepoBundleResponse {
                    ok: false,
                    message: e,
                    bundle_json: None,
                },
            };
            write_control(&mut send, &response, "P2P 记录响应").await?;
        }
        P2PWireRequest::PushRepoBundle {
            knowledge_repo,
            bundle_json,
        } => {
            let ack = match apply_repo_bundle_json(&bundle_json) {
                Ok(n) => P2PAck {
                    ok: true,
                    message: format!("已合并 {knowledge_repo} 的 {n} 条记录"),
                },
                Err(e) => P2PAck {
                    ok: false,
                    message: e,
                },
            };
            write_control(&mut send, &ack, "P2P 记录推送响应").await?;
        }
        P2PWireRequest::KgManualBundle { kb_name } => {
            let response = match kg_manual_bundle_json(&kb_name) {
                Ok(json) => P2PRepoBundleResponse {
                    ok: true,
                    message: "ok".to_string(),
                    bundle_json: Some(json),
                },
                Err(e) => P2PRepoBundleResponse {
                    ok: false,
                    message: e,
                    bundle_json: None,
                },
            };
            write_control(&mut send, &response, "P2P 人工层响应").await?;
        }
        P2PWireRequest::PushKgManualBundle { kb_name, bundle_json } => {
            let ack = match kg_manual_merge_json(&kb_name, &bundle_json) {
                Ok(n) => P2PAck {
                    ok: true,
                    message: format!("已合并 {kb_name} 人工层 {n} 行"),
                },
                Err(e) => P2PAck {
                    ok: false,
                    message: e,
                },
            };
            write_control(&mut send, &ack, "P2P 人工层推送响应").await?;
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
    let mut builder = Endpoint::builder(presets::N0)
        .secret_key(key)
        .alpns(vec![P2P_ALPN.to_vec()])
        .transport_config(build_transport_config());
    // 按用户配置覆盖中继模式（仅局域网 / 自定义中继）；默认沿用 n0 公共中继。
    if let Some(relay_mode) = p2p_relay_mode_override() {
        builder = builder.relay_mode(relay_mode);
    }
    let endpoint = builder
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
    let options = ConnectOptions::new().with_transport_config(build_transport_config());
    let connecting = timeout(
        StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS),
        endpoint.connect_with_opts(addr, P2P_ALPN, options),
    )
    .await
    .map_err(|_| format!("连接设备 {} 超时", peer.device_name))?
    .map_err(|e| format!("连接设备 {} 失败: {e}", peer.device_name))?;
    let connection = timeout(StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS), connecting)
        .await
        .map_err(|_| format!("等待设备 {} 握手超时", peer.device_name))?
        .map_err(|e| format!("与设备 {} 握手失败: {e}", peer.device_name))?;
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

/// 采集某 knowledge_repo 的数据库记录 bundle 并序列化为 JSON（复用 backup 模块逻辑）。
fn collect_repo_bundle_json(knowledge_repo: &str) -> Result<String, String> {
    let bundle = crate::commands::backup::collect_repo_bundle(knowledge_repo)?;
    crate::commands::backup::repo_bundle_to_json(&bundle)
}

/// 把对端传来的 bundle JSON 合并进本地数据库，返回合并的 imported_files 条数。
fn apply_repo_bundle_json(json: &str) -> Result<usize, String> {
    let bundle = crate::commands::backup::repo_bundle_from_json(json)?;
    crate::commands::backup::apply_repo_bundle(&bundle)
}

/// 与单个对端双向交换某 knowledge_repo 的数据库记录：
/// 1) 拉取对端 bundle 并合并到本地；2) 把本机 bundle 推给对端合并。
/// 返回 (本地合并条数, 推送给对端条数提示文本)。
async fn exchange_repo_bundle(
    connection: &Connection,
    knowledge_repo: &str,
) -> Result<(usize, String), String> {
    // 1) 拉取对端记录
    let pull_req = P2PWireRequest::RepoBundle {
        knowledge_repo: knowledge_repo.to_string(),
    };
    let pull_resp: P2PRepoBundleResponse =
        request_response(connection, &pull_req, P2P_BUNDLE_FRAME_LIMIT).await?;
    let mut merged = 0_usize;
    if pull_resp.ok {
        if let Some(json) = pull_resp.bundle_json {
            merged = apply_repo_bundle_json(&json)?;
        }
    } else {
        return Err(format!("对端导出记录失败: {}", pull_resp.message));
    }

    // 2) 推送本机记录给对端
    let local_json = collect_repo_bundle_json(knowledge_repo)?;
    let push_req = P2PWireRequest::PushRepoBundle {
        knowledge_repo: knowledge_repo.to_string(),
        bundle_json: local_json,
    };
    let push_ack: P2PAck =
        request_response(connection, &push_req, P2P_BUNDLE_FRAME_LIMIT).await?;
    if !push_ack.ok {
        return Err(format!("推送记录到对端失败: {}", push_ack.message));
    }
    Ok((merged, push_ack.message))
}

fn kg_manual_bundle_json(kb_name: &str) -> Result<String, String> {
    let bundle = crate::commands::kgraph::export_manual_bundle(kb_name)?;
    serde_json::to_string(&bundle).map_err(|e| e.to_string())
}

fn kg_manual_merge_json(kb_name: &str, json: &str) -> Result<usize, String> {
    let bundle: crate::commands::kgraph::ManualBundle =
        serde_json::from_str(json).map_err(|e| e.to_string())?;
    crate::commands::kgraph::merge_manual_bundle(kb_name, &bundle)
}

/// 机制B:与单个对端双向按行合并某 kgraph 知识库的人工知识层(无墓碑)。
/// 返回本地新增的行数。
async fn exchange_kg_manual_bundle(
    connection: &Connection,
    kb_name: &str,
) -> Result<usize, String> {
    let pull_req = P2PWireRequest::KgManualBundle {
        kb_name: kb_name.to_string(),
    };
    let pull_resp: P2PRepoBundleResponse =
        request_response(connection, &pull_req, P2P_BUNDLE_FRAME_LIMIT).await?;
    let mut merged = 0_usize;
    if pull_resp.ok {
        if let Some(json) = pull_resp.bundle_json {
            merged = kg_manual_merge_json(kb_name, &json)?;
        }
    } else {
        return Err(format!("对端导出人工层失败: {}", pull_resp.message));
    }

    let local_json = kg_manual_bundle_json(kb_name)?;
    let push_req = P2PWireRequest::PushKgManualBundle {
        kb_name: kb_name.to_string(),
        bundle_json: local_json,
    };
    let push_ack: P2PAck =
        request_response(connection, &push_req, P2P_BUNDLE_FRAME_LIMIT).await?;
    if !push_ack.ok {
        return Err(format!("推送人工层到对端失败: {}", push_ack.message));
    }
    Ok(merged)
}

/// 统一构造一个同步目标：确保根目录存在后返回 target。
fn make_target(key: &str, label: &str, root: String) -> Result<P2PSyncTarget, String> {
    let root = PathBuf::from(root);
    fs::create_dir_all(&root).map_err(|e| format!("创建「{label}」目录失败: {e}"))?;
    Ok(P2PSyncTarget {
        key: key.to_string(),
        label: label.to_string(),
        root,
    })
}

/// 在 P2P 同步配置中按 id 查找自定义目录。
fn custom_dir_root(id: &str) -> Result<(String, String), String> {
    let cfg = get_resource_repo_sync_config();
    cfg.p2p
        .custom_sync_dirs
        .iter()
        .find(|d| d.id == id)
        .filter(|d| !d.path.trim().is_empty())
        .map(|d| {
            let label = if d.label.trim().is_empty() {
                d.id.clone()
            } else {
                d.label.clone()
            };
            (label, d.path.clone())
        })
        .ok_or_else(|| format!("未找到自定义同步目录: {id}"))
}

/// 同步目标 (label, root 路径) 的短时缓存。一次同步会对每个文件解析一次 target，
/// 而解析要走 load_config（读盘 + keyring 解密），数百文件叠加开销显著。缓存 TTL 内
/// 复用解析结果，避免热路径反复 load_config。配置变更最多延迟 TTL 生效，对同步无碍。
const TARGET_RESOLVE_TTL: StdDuration = StdDuration::from_secs(10);
static TARGET_RESOLVE_CACHE: OnceLock<std::sync::Mutex<HashMap<String, (String, String, Instant)>>> =
    OnceLock::new();

fn target_resolve_cache() -> &'static std::sync::Mutex<HashMap<String, (String, String, Instant)>> {
    TARGET_RESOLVE_CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// 解析 key 对应的 (label, root 路径)，命中 TTL 内缓存则直接返回，否则读配置并写缓存。
fn resolve_target_label_root(key: &str) -> Result<(String, String), String> {
    if let Ok(cache) = target_resolve_cache().lock() {
        if let Some((label, root, at)) = cache.get(key) {
            if at.elapsed() < TARGET_RESOLVE_TTL {
                return Ok((label.clone(), root.clone()));
            }
        }
    }

    let (label, root) = if let Some(id) = key.strip_prefix("custom:") {
        let (label, path) = custom_dir_root(id)?;
        (label, path)
    } else {
        match key {
            "resource_repo" => ("资源库".to_string(), get_image_bed_root()),
            "knowledge_base" => ("知识库".to_string(), get_knowledge_bases_root()),
            // 文件库的实际文件存放在 <libraries_root>/Libraries 下（与 backup.rs、repoview.rs
            // 等一致）。同步根必须锚定到该 Libraries 子目录，否则会把 libraries_root 下的其它
            // 内容也纳入同步，且相对路径会带上各机不同的父级目录名，导致对端写入错位、产生残留目录。
            "libraries_root" => (
                "文件库".to_string(),
                PathBuf::from(get_libraries_root())
                    .join("Libraries")
                    .to_string_lossy()
                    .to_string(),
            ),
            "template" => ("模板库".to_string(), get_template_repo_path()),
            "slidev" => ("Slidev".to_string(), get_slidev_repo_root()),
            other => return Err(format!("未知的同步目标: {other}")),
        }
    };

    if let Ok(mut cache) = target_resolve_cache().lock() {
        cache.insert(key.to_string(), (label.clone(), root.clone(), Instant::now()));
    }
    Ok((label, root))
}

/// 同步目标的唯一解析入口。支持固定 key 与 `custom:<id>` 形式的自定义目录。
fn target_from_key(key: &str) -> Result<P2PSyncTarget, String> {
    let (label, root) = resolve_target_label_root(key)?;
    make_target(key, &label, root)
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
        || target.key == "knowledge_base"
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

/// 在阻塞线程池里对整文件做 blake3，避免在异步线程上长时间占用 CPU 把 QUIC
/// 心跳饿死。大文件（如 mp4）尤其需要。
async fn hash_file_async(path: &Path) -> Result<String, String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || hash_file(&path))
        .await
        .map_err(|e| format!("哈希任务执行失败: {e}"))?
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
        target = %target.key,
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
    info!(target = %target.key, path = %path.to_string_lossy(), "[P2PSync] manifest saved");
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
                        target = %target.key,
                        rel,
                        backup = %backup.to_string_lossy(),
                        "[P2PSync] kept local copy as conflict backup before overwrite"
                    ),
                    Err(e) => info!(
                        target = %target.key,
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

/// `finalize_incoming_file` 的异步封装：整个落盘+校验在阻塞线程池执行，避免
/// 在异步线程上做大文件哈希。
async fn finalize_incoming_file_async(
    target: &P2PSyncTarget,
    rel: &str,
    tmp: &Path,
    expected_hash: &str,
) -> Result<(), String> {
    let target = target.clone();
    let rel = rel.to_string();
    let tmp = tmp.to_path_buf();
    let expected = expected_hash.to_string();
    tokio::task::spawn_blocking(move || finalize_incoming_file(&target, &rel, &tmp, &expected))
        .await
        .map_err(|e| format!("落盘任务执行失败: {e}"))?
}

/// 服务端：定位待拉取文件，返回 (路径, 大小, 哈希, mtime)。
async fn serve_pull(target_key: &str, rel: &str) -> Result<(PathBuf, u64, String, i64), String> {
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
    let hash = hash_file_async(&path).await?;
    let mtime = file_mtime_ms(&path)?;
    Ok((path, meta.len(), hash, mtime))
}

/// 服务端：基于客户端提供的 basis 签名，计算并返回块级 delta（仅变化块）。
/// 在阻塞线程池里跑 copia 的 signature 比对与 delta 生成，避免阻塞运行时。
async fn serve_pull_delta(target_key: &str, rel: &str, signature: &Signature) -> P2PDeltaResponse {
    let fail = |msg: String| P2PDeltaResponse {
        ok: false,
        message: msg,
        size: 0,
        blake3: String::new(),
        mtime_ms: 0,
        delta: None,
    };

    let target = match target_from_key(target_key) {
        Ok(t) => t,
        Err(e) => return fail(e),
    };
    if should_skip(&target, rel) {
        return fail(format!("该文件不在同步范围内: {rel}"));
    }
    let path = match safe_join(&target.root, rel) {
        Ok(p) => p,
        Err(e) => return fail(e),
    };
    if !path.is_file() {
        return fail(format!("文件不存在: {rel}"));
    }
    let blake3 = match hash_file_async(&path).await {
        Ok(h) => h,
        Err(e) => return fail(e),
    };
    let mtime_ms = match file_mtime_ms(&path) {
        Ok(m) => m,
        Err(e) => return fail(e),
    };
    let size = match fs::metadata(&path) {
        Ok(m) => m.len(),
        Err(e) => return fail(format!("读取文件元数据失败: {e}")),
    };

    let signature = signature.clone();
    let path_for_delta = path.clone();
    let delta = tokio::task::spawn_blocking(move || {
        let sync = SyncBuilder::new().block_size(P2P_BLOCK_SIZE).build();
        let source = fs::read(&path_for_delta)
            .map_err(|e| format!("读取待发送文件失败 {}: {e}", path_for_delta.display()))?;
        sync.delta(Cursor::new(source.as_slice()), &signature)
            .map_err(|e| format!("生成 copia delta 失败: {e}"))
    })
    .await;

    match delta {
        Ok(Ok(delta)) => P2PDeltaResponse {
            ok: true,
            message: "ok".to_string(),
            size,
            blake3,
            mtime_ms,
            delta: Some(delta),
        },
        Ok(Err(e)) => fail(e),
        Err(e) => fail(format!("delta 任务执行失败: {e}")),
    }
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
        let mut out = tokio::fs::File::create(&tmp)
            .await
            .map_err(|e| format!("创建 P2P 临时文件失败 {}: {e}", tmp.display()))?;
        recv_file_body(recv, &mut out, size).await?;
        use tokio::io::AsyncWriteExt;
        out.flush()
            .await
            .map_err(|e| format!("刷新 P2P 临时文件失败 {}: {e}", tmp.display()))?;
    }

    finalize_incoming_file_async(&target, rel, &tmp, expected_hash).await?;
    info!(target = %target.key, rel, size, "[P2PSync] accepted pushed file");
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

/// 断点续传用的稳定临时文件路径：按 rel 的 blake3 命名，跨多次同步保持一致，
/// 中断后重连能从已写字节继续。
fn part_path(target: &P2PSyncTarget, rel: &str) -> PathBuf {
    let key = blake3::hash(rel.as_bytes()).to_hex().to_string();
    target
        .root
        .join(P2P_META_DIR)
        .join(P2P_TMP_DIR)
        .join(format!("{key}.part"))
}

/// 客户端：从对端拉取单个文件并原子落盘，支持断点续传。`expected` 为远端清单
/// 中记录的条目（大小/哈希），用于校验与续传判断。
async fn pull_file_full(
    connection: &Connection,
    target: &P2PSyncTarget,
    rel: &str,
    expected: &P2PFileEntry,
) -> Result<u64, String> {
    use tokio::io::AsyncWriteExt;

    let tmp_dir = target.root.join(P2P_META_DIR).join(P2P_TMP_DIR);
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("创建 P2P 临时目录失败: {e}"))?;
    let part = part_path(target, rel);

    // 已有断点：取其当前长度作为续传偏移；若超过期望大小说明对端文件已变，丢弃重来。
    let mut offset = match fs::metadata(&part) {
        Ok(m) => m.len(),
        Err(_) => 0,
    };
    if offset > expected.size {
        let _ = fs::remove_file(&part);
        offset = 0;
    }

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
            offset,
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
    // 对端大小与续传偏移不一致（对端文件已变）时，从头重写。
    let resume = offset > 0 && header.size >= offset;
    let body_len = if resume {
        header.size - offset
    } else {
        offset = 0;
        header.size
    };

    {
        let mut out = if resume {
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(&part)
                .await
                .map_err(|e| format!("打开续传临时文件失败 {}: {e}", part.display()))?
        } else {
            tokio::fs::File::create(&part)
                .await
                .map_err(|e| format!("创建 P2P 临时文件失败 {}: {e}", part.display()))?
        };
        recv_file_body(&mut recv, &mut out, body_len).await?;
        out.flush()
            .await
            .map_err(|e| format!("刷新 P2P 临时文件失败 {}: {e}", part.display()))?;
    }

    finalize_incoming_file_async(target, rel, &part, &header.blake3).await?;
    info!(
        target = %target.key,
        rel,
        size = header.size,
        resumed_from = offset,
        "[P2PSync] pulled file"
    );
    Ok(header.size)
}

/// 客户端：块级增量拉取。以本地现有文件为 basis 计算签名，对端只回传变化块，
/// 本地 patch 重建后校验落盘。仅在本地存在旧版本且开启 delta 时使用。
async fn pull_file_delta(
    connection: &Connection,
    target: &P2PSyncTarget,
    rel: &str,
) -> Result<u64, String> {
    let basis = safe_join(&target.root, rel)?;
    if !basis.is_file() {
        return Err("本地无 basis，无法块级增量".to_string());
    }

    // 在阻塞线程池计算 basis 签名。
    let basis_for_sig = basis.clone();
    let signature = tokio::task::spawn_blocking(move || {
        let sync = SyncBuilder::new().block_size(P2P_BLOCK_SIZE).build();
        let bytes = fs::read(&basis_for_sig)
            .map_err(|e| format!("读取 basis 失败 {}: {e}", basis_for_sig.display()))?;
        sync.signature(Cursor::new(bytes.as_slice()))
            .map_err(|e| format!("生成 copia 签名失败: {e}"))
    })
    .await
    .map_err(|e| format!("签名任务执行失败: {e}"))??;

    let resp: P2PDeltaResponse = request_response(
        connection,
        &P2PWireRequest::PullDelta {
            target: target.key.to_string(),
            rel: rel.to_string(),
            signature,
        },
        P2P_DELTA_FRAME_LIMIT,
    )
    .await?;
    if !resp.ok {
        return Err(resp.message);
    }
    let delta = resp.delta.ok_or_else(|| "对端未返回 delta".to_string())?;
    let size = resp.size;

    let tmp_dir = target.root.join(P2P_META_DIR).join(P2P_TMP_DIR);
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("创建 P2P 临时目录失败: {e}"))?;
    let tmp = tmp_dir.join(format!("{}.delta.tmp", Uuid::new_v4()));

    // 在阻塞线程池 patch：basis + delta -> 新文件。
    let basis_for_patch = basis.clone();
    let tmp_for_patch = tmp.clone();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let sync = SyncBuilder::new().block_size(P2P_BLOCK_SIZE).build();
        let basis_file = fs::File::open(&basis_for_patch)
            .map_err(|e| format!("打开 basis 失败 {}: {e}", basis_for_patch.display()))?;
        let out = fs::File::create(&tmp_for_patch)
            .map_err(|e| format!("创建 delta 临时文件失败 {}: {e}", tmp_for_patch.display()))?;
        sync.patch(basis_file, &delta, out)
            .map_err(|e| format!("应用 copia patch 失败: {e}"))
    })
    .await
    .map_err(|e| format!("patch 任务执行失败: {e}"))??;

    finalize_incoming_file_async(target, rel, &tmp, &resp.blake3).await?;
    info!(target = %target.key, rel, size, "[P2PSync] pulled file via block delta");
    Ok(size)
}

/// 客户端：拉取一个文件。优先块级增量（本地有旧版本时），失败回退整文件续传。
async fn pull_file(
    connection: &Connection,
    target: &P2PSyncTarget,
    rel: &str,
    expected: &P2PFileEntry,
) -> Result<u64, String> {
    let basis = safe_join(&target.root, rel)?;
    let can_delta = p2p_block_delta_enabled() && basis.is_file() && expected.size >= P2P_DELTA_MIN_SIZE;
    if can_delta {
        match pull_file_delta(connection, target, rel).await {
            Ok(n) => return Ok(n),
            Err(e) => {
                info!(target = %target.key, rel, error = e, "[P2PSync] delta pull failed, falling back to full");
            }
        }
    }
    pull_file_full(connection, target, rel, expected).await
}

/// 客户端：把本地单个文件推送给对端。复用清单中已算好的哈希，避免重复 blake3。
async fn push_file(
    connection: &Connection,
    target: &P2PSyncTarget,
    rel: &str,
    entry: &P2PFileEntry,
) -> Result<u64, String> {
    let path = safe_join(&target.root, rel)?;
    let size = entry.size;

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
            blake3: entry.blake3.clone(),
            mtime_ms: entry.modified_unix_ms,
        },
        "P2P 推送请求",
    )
    .await?;
    send_file_body(&mut send, &path, size, 0).await?;
    send.finish()
        .map_err(|e| format!("推送完成发送失败: {e}"))?;

    let ack: P2PAck = read_control(&mut recv, P2P_FRAME_LIMIT, "P2P 推送响应").await?;
    if !ack.ok {
        return Err(ack.message);
    }
    info!(target = %target.key, rel, size, "[P2PSync] pushed file");
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

/// 一个对端的连接句柄：缓存当前 Connection，断线后按需重连。多个并发文件传输
/// 共享同一条连接（QUIC 多路复用），任意一方探测到连接已死即触发一次重连。
struct PeerLink {
    endpoint: Endpoint,
    peer: P2PTrustedPeer,
    conn: Mutex<Option<Connection>>,
}

impl PeerLink {
    fn new(endpoint: Endpoint, peer: P2PTrustedPeer) -> Self {
        Self {
            endpoint,
            peer,
            conn: Mutex::new(None),
        }
    }

    /// 取当前可用连接；若不存在或已关闭则重连。
    async fn current(&self) -> Result<Connection, String> {
        let mut guard = self.conn.lock().await;
        if let Some(c) = guard.as_ref() {
            if c.close_reason().is_none() {
                return Ok(c.clone());
            }
        }
        let c = connect_peer(&self.endpoint, &self.peer).await?;
        *guard = Some(c.clone());
        Ok(c)
    }

    /// 标记某条具体连接已死（仅当缓存的就是它时才清空，避免误删新连接）。
    async fn invalidate(&self, dead: &Connection) {
        let mut guard = self.conn.lock().await;
        if guard
            .as_ref()
            .map(|c| c.stable_id() == dead.stable_id())
            .unwrap_or(false)
        {
            *guard = None;
        }
    }

    async fn close(&self) {
        let mut guard = self.conn.lock().await;
        if let Some(c) = guard.take() {
            c.close(0u32.into(), b"sync done");
        }
    }
}

/// 判断错误是否属于"连接级"失败（需要重连），而非业务级失败（文件不存在等）。
fn is_connection_error(msg: &str) -> bool {
    let m = msg;
    m.contains("connection lost")
        || m.contains("connection closed")
        || m.contains("closed")
        || m.contains("timed out")
        || m.contains("超时")
        || m.contains("通道")
        || m.contains("reset")
        || m.contains("aborted")
}

/// 单个文件传输（拉/推）带重连重试。`op` 接收当前连接执行一次传输，失败若属
/// 连接级错误则使缓存连接失效并退避重试。
async fn transfer_with_retry<F, Fut>(link: &PeerLink, op: F) -> Result<u64, String>
where
    F: Fn(Connection) -> Fut,
    Fut: std::future::Future<Output = Result<u64, String>>,
{
    let mut last_err = String::new();
    for attempt in 0..P2P_TRANSFER_MAX_RETRIES {
        let connection = match link.current().await {
            Ok(c) => c,
            Err(e) => {
                last_err = e;
                tokio::time::sleep(StdDuration::from_millis(300 * (attempt as u64 + 1))).await;
                continue;
            }
        };
        match op(connection.clone()).await {
            Ok(n) => return Ok(n),
            Err(e) => {
                last_err = e.clone();
                if is_connection_error(&e) {
                    link.invalidate(&connection).await;
                    tokio::time::sleep(StdDuration::from_millis(300 * (attempt as u64 + 1))).await;
                } else {
                    // 业务级错误重试也无益，直接返回。
                    return Err(e);
                }
            }
        }
    }
    Err(last_err)
}

/// 同步过程中顺带交换可信设备列表，推动全网状自动组网。失败不影响数据同步。
async fn exchange_peer_gossip(endpoint: &Endpoint, connection: &Connection, remote_peer_id: &str) {
    if !p2p_auto_propagate_trust() {
        return;
    }
    let state = match read_state_file() {
        Ok(s) => s,
        Err(_) => return,
    };
    let self_peer_id = endpoint.id().to_string();
    let request = P2PWireRequest::Probe {
        device_id: get_device_id(),
        device_name: p2p_device_name(),
        peer_id: self_peer_id.clone(),
        address_hint: encode_endpoint_addr(&endpoint.addr()).unwrap_or_default(),
        known_peers: collect_peer_hints(&state, remote_peer_id),
    };
    match request_response::<P2PWireRequest, P2PWireResponse>(connection, &request, P2P_FRAME_LIMIT)
        .await
    {
        Ok(resp) if resp.ok => {
            if let Ok(mut st) = read_state_file() {
                let learned = merge_peer_hints(&mut st, &resp.known_peers, &self_peer_id);
                if learned > 0 {
                    let _ = write_state_file(&st);
                    info!(learned, "[P2PSync] learned peers during sync gossip");
                }
            }
        }
        _ => {}
    }
}

/// 真正执行一次跨设备双向同步：扫描本地清单，对每个可信设备拉取远端清单做 diff，
/// 按 mtime 后写优先把变化的文件拉/推过去；冲突文件保留 `.conflict` 备份。
/// 单连接多流并发传输、断线自动重连续传、块级增量；全网状自动组网。
/// 局域网走直连、互联网走中继，均由 iroh 自动选择。
async fn run_p2p_sync(
    app: Option<&AppHandle>,
    run_id: &str,
    target: P2PSyncTarget,
    knowledge_repo: Option<String>,
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
    let concurrency = p2p_transfer_concurrency();

    let mut stats = P2PSyncStats::default();
    let mut online = 0_u64;
    let mut errors: Vec<String> = Vec::new();
    let mut records_merged = 0_usize;

    // 注意：同步循环里会因 gossip 学到新设备并写回状态文件，但本轮仍按起始快照
    // 的 peers 同步；新设备在下一轮（或下次同步）纳入，符合最终一致。
    for peer in &peers {
        if is_sync_cancelled(run_id) {
            break;
        }
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

        let link = PeerLink::new(endpoint.clone(), peer.clone());
        let connection = match link.current().await {
            Ok(c) => c,
            Err(e) => {
                update_peer_status(&peer.peer_id, "offline", &e);
                errors.push(format!("{}: {}", peer.device_name, e));
                continue;
            }
        };
        online += 1;
        update_peer_status(&peer.peer_id, "online", "同步连接成功");

        // 顺带交换可信设备列表，推动全网状组网。
        exchange_peer_gossip(&endpoint, &connection, &peer.peer_id).await;

        let remote = match fetch_remote_manifest(&connection, &target.key).await {
            Ok(m) => m,
            Err(e) => {
                errors.push(format!("{} 获取清单失败: {}", peer.device_name, e));
                link.close().await;
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

        // 并发执行传输：单连接多流，失败自动重连重试。进度用原子计数。
        let link = Arc::new(link);
        let done = Arc::new(AtomicU64::new(0));
        let downloaded = Arc::new(AtomicU64::new(0));
        let uploaded = Arc::new(AtomicU64::new(0));
        let transfer_errors = Arc::new(Mutex::new(Vec::<String>::new()));

        use futures_util::future::FutureExt;
        use futures_util::stream::{self, StreamExt};

        // 拉取任务。
        let pull_futs = pulls.iter().filter_map(|rel| {
            let entry = remote.files.get(rel)?.clone();
            let rel = rel.clone();
            let link = link.clone();
            let target = target.clone();
            let done = done.clone();
            let downloaded = downloaded.clone();
            let transfer_errors = transfer_errors.clone();
            let app_owned = app.cloned();
            let run_id = run_id.to_string();
            let peer_name = peer.device_name.clone();
            let plan_total = plan_total;
            Some(
                async move {
                    // 已请求终止：跳过本文件传输（保留已完成文件与半截 .part 供续传）。
                    if is_sync_cancelled(&run_id) {
                        done.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                    let cur = done.load(Ordering::Relaxed);
                    emit_p2p_progress(
                        app_owned.as_ref(),
                        &run_id,
                        &target,
                        "transfer",
                        "running",
                        format!("从 {} 拉取 {}", peer_name, rel),
                        cur,
                        plan_total,
                        0,
                        0,
                        rel.clone(),
                    );
                    let res = transfer_with_retry(&link, |conn| {
                        let target = target.clone();
                        let rel = rel.clone();
                        let entry = entry.clone();
                        async move { pull_file(&conn, &target, &rel, &entry).await }
                    })
                    .await;
                    match res {
                        Ok(_) => {
                            downloaded.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(e) => {
                            transfer_errors
                                .lock()
                                .await
                                .push(format!("拉取 {} 失败: {}", rel, e));
                        }
                    }
                    done.fetch_add(1, Ordering::Relaxed);
                }
                .boxed(),
            )
        });

        // 推送任务。
        let push_futs = pushes.iter().filter_map(|rel| {
            let entry = local.files.get(rel)?.clone();
            let rel = rel.clone();
            let link = link.clone();
            let target = target.clone();
            let done = done.clone();
            let uploaded = uploaded.clone();
            let transfer_errors = transfer_errors.clone();
            let app_owned = app.cloned();
            let run_id = run_id.to_string();
            let peer_name = peer.device_name.clone();
            let plan_total = plan_total;
            Some(
                async move {
                    // 已请求终止：跳过本文件传输。
                    if is_sync_cancelled(&run_id) {
                        done.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                    let cur = done.load(Ordering::Relaxed);
                    emit_p2p_progress(
                        app_owned.as_ref(),
                        &run_id,
                        &target,
                        "transfer",
                        "running",
                        format!("向 {} 推送 {}", peer_name, rel),
                        cur,
                        plan_total,
                        0,
                        0,
                        rel.clone(),
                    );
                    let res = transfer_with_retry(&link, |conn| {
                        let target = target.clone();
                        let rel = rel.clone();
                        let entry = entry.clone();
                        async move { push_file(&conn, &target, &rel, &entry).await }
                    })
                    .await;
                    match res {
                        Ok(_) => {
                            uploaded.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(e) => {
                            transfer_errors
                                .lock()
                                .await
                                .push(format!("推送 {} 失败: {}", rel, e));
                        }
                    }
                    done.fetch_add(1, Ordering::Relaxed);
                }
                .boxed(),
            )
        });

        let all_futs: Vec<_> = pull_futs.chain(push_futs).collect();
        stream::iter(all_futs)
            .buffer_unordered(concurrency)
            .collect::<Vec<()>>()
            .await;

        stats.downloaded += downloaded.load(Ordering::Relaxed) as u32;
        stats.uploaded += uploaded.load(Ordering::Relaxed) as u32;
        {
            let mut te = transfer_errors.lock().await;
            errors.append(&mut te);
        }

        // 文件传输完成后，若需要则双向交换数据库记录：
        // 资源库传当前 knowledge_repo，文件库根目录传 "ALL"（所有库）。已请求终止则跳过。
        if let Some(ref kr) = knowledge_repo {
            if !kr.trim().is_empty() && !is_sync_cancelled(run_id) {
                emit_p2p_progress(
                    app,
                    run_id,
                    &target,
                    "records",
                    "running",
                    format!("与 {} 交换数据库记录…", peer.device_name),
                    0,
                    0,
                    0,
                    0,
                    "",
                );
                match link.current().await {
                    Ok(conn) => match exchange_repo_bundle(&conn, kr).await {
                        Ok((merged, _push_msg)) => {
                            records_merged += merged;
                        }
                        Err(e) => {
                            errors.push(format!("{} 记录同步失败: {}", peer.device_name, e));
                        }
                    },
                    Err(e) => {
                        errors.push(format!("{} 记录同步连接失败: {}", peer.device_name, e));
                    }
                }
            }
        }

        // 机制B(可选):知识库目录同步后,双向按行合并各 KB 的 kgraph 人工知识层。
        if target.key == "knowledge_base"
            && crate::commands::kgraph::manual_sync_enabled()
            && !is_sync_cancelled(run_id)
        {
            match link.current().await {
                Ok(conn) => {
                    for kb in crate::commands::kgraph::list_kb_names() {
                        match exchange_kg_manual_bundle(&conn, &kb).await {
                            Ok(n) => {
                                records_merged += n;
                            }
                            Err(e) => {
                                errors.push(format!(
                                    "{} 人工层同步失败({}): {}",
                                    peer.device_name, kb, e
                                ));
                            }
                        }
                    }
                }
                Err(e) => {
                    errors.push(format!("{} 人工层同步连接失败: {}", peer.device_name, e));
                }
            }
        }

        // 主动关闭连接，触发对端 accept_bi 退出。
        link.close().await;
    }

    // 是否被用户终止（在清理标记前读取）。
    let cancelled = is_sync_cancelled(run_id);
    clear_sync_cancel(run_id);

    // 同步后重扫一次并把清单作为新的对比基准持久化（即便被终止，也保留已完成部分的快照）。
    let final_manifest = scan_manifest(&target)?;
    write_manifest(&target, &final_manifest)?;

    let status = if cancelled {
        "cancelled"
    } else if online == 0 {
        "error"
    } else if errors.is_empty() {
        "success"
    } else {
        "skipped"
    };
    let complete_message = if cancelled {
        format!(
            "{} 同步已终止：已完成下载 {}，上传 {}（已完成部分已保留，可再次同步续传）",
            target.label, stats.downloaded, stats.uploaded
        )
    } else {
        format!(
            "{} 同步完成：下载 {}，上传 {}，跳过 {}，在线设备 {}/{}",
            target.label,
            stats.downloaded,
            stats.uploaded,
            stats.skipped,
            online,
            peers.len()
        )
    };
    emit_p2p_progress(
        app,
        run_id,
        &target,
        "complete",
        status,
        complete_message,
        stats.downloaded as u64 + stats.uploaded as u64,
        stats.downloaded as u64 + stats.uploaded as u64,
        0,
        0,
        "",
    );

    if cancelled {
        return Ok(format!(
            "{} 同步已终止：下载 {}，上传 {}。已完成部分已保留，可再次同步续传。",
            target.label, stats.downloaded, stats.uploaded
        ));
    }

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
    if knowledge_repo.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false) {
        summary.push_str(&format!("合并数据库记录 {} 条。", records_merged));
    }
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
    let known_peers = collect_peer_hints(&state, &payload.peer_id);
    let response = send_wire_request(
        &endpoint,
        &peer,
        &P2PWireRequest::Pair {
            ticket_id: payload.ticket_id.clone(),
            device_id: get_device_id(),
            device_name: p2p_device_name(),
            peer_id: endpoint.id().to_string(),
            address_hint: local_address_hint,
            known_peers,
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
    let self_peer_id = endpoint.id().to_string();
    let learned = merge_peer_hints(&mut state, &response.known_peers, &self_peer_id);
    if learned > 0 {
        info!(learned, "[P2PSync] learned peers from pairing response");
    }
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
    let self_peer_id = endpoint.id().to_string();
    run_copia_block_delta_self_check()?;

    // 探测前固定一份待传播的可信列表快照，避免与循环中的可变借用冲突。
    let hints_snapshot = collect_peer_hints(&state, "");
    let mut learned_hints: Vec<P2PPeerHint> = Vec::new();

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

        let known_peers: Vec<P2PPeerHint> = hints_snapshot
            .iter()
            .filter(|h| h.peer_id != peer.peer_id)
            .cloned()
            .collect();
        let request = P2PWireRequest::Probe {
            device_id: get_device_id(),
            device_name: p2p_device_name(),
            peer_id: self_peer_id.clone(),
            address_hint: local_address_hint.clone(),
            known_peers,
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
                learned_hints.extend(response.known_peers);
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

    // 把探测过程中从各对端学到的设备合并进来（全网状自动组网）。
    if !learned_hints.is_empty() {
        let learned = merge_peer_hints(&mut state, &learned_hints, &self_peer_id);
        if learned > 0 {
            changed = true;
            info!(learned, "[P2PSync] learned peers via probe propagation round");
        }
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
pub async fn p2p_sync_resource_repo_now(
    app: AppHandle,
    knowledge_repo: Option<String>,
) -> Result<String, String> {
    let run_id = Uuid::new_v4().to_string();
    let target = target_from_key("resource_repo")?;
    info!(run_id, target = %target.key, knowledge_repo = ?knowledge_repo, "[P2PSync] resource repo sync requested");
    run_p2p_sync(Some(&app), &run_id, target, knowledge_repo).await
}

#[tauri::command]
pub async fn p2p_sync_verify_resource_repo() -> Result<String, String> {
    verify_target(target_from_key("resource_repo")?)
}

#[tauri::command]
pub async fn p2p_sync_knowledge_base_now(app: AppHandle) -> Result<String, String> {
    let run_id = Uuid::new_v4().to_string();
    let target = target_from_key("knowledge_base")?;
    info!(run_id, target = %target.key, "[P2PSync] knowledge base emergency sync requested");
    run_p2p_sync(Some(&app), &run_id, target, None).await
}

#[tauri::command]
pub async fn p2p_sync_verify_knowledge_base() -> Result<String, String> {
    verify_target(target_from_key("knowledge_base")?)
}

#[tauri::command]
pub async fn p2p_sync_libraries_root_now(app: AppHandle) -> Result<String, String> {
    let run_id = Uuid::new_v4().to_string();
    let target = target_from_key("libraries_root")?;
    info!(run_id, target = %target.key, "[P2PSync] libraries root sync requested (all libraries + records)");
    // 文件库根目录承载所有库的文件，记录用 "ALL" 导出所有库的数据库记录，实现"同步所有库的数据和文件"。
    run_p2p_sync(Some(&app), &run_id, target, Some("ALL".to_string())).await
}

#[tauri::command]
pub async fn p2p_sync_verify_libraries_root() -> Result<String, String> {
    verify_target(target_from_key("libraries_root")?)
}

#[tauri::command]
pub async fn p2p_sync_template_now(app: AppHandle) -> Result<String, String> {
    let run_id = Uuid::new_v4().to_string();
    let target = target_from_key("template")?;
    info!(run_id, target = %target.key, "[P2PSync] template sync requested");
    run_p2p_sync(Some(&app), &run_id, target, None).await
}

#[tauri::command]
pub async fn p2p_sync_verify_template() -> Result<String, String> {
    verify_target(target_from_key("template")?)
}

#[tauri::command]
pub async fn p2p_sync_slidev_now(app: AppHandle) -> Result<String, String> {
    let run_id = Uuid::new_v4().to_string();
    let target = target_from_key("slidev")?;
    info!(run_id, target = %target.key, "[P2PSync] slidev sync requested");
    run_p2p_sync(Some(&app), &run_id, target, None).await
}

#[tauri::command]
pub async fn p2p_sync_verify_slidev() -> Result<String, String> {
    verify_target(target_from_key("slidev")?)
}

#[tauri::command]
pub async fn p2p_sync_custom_now(app: AppHandle, key: String) -> Result<String, String> {
    let run_id = Uuid::new_v4().to_string();
    let target = target_from_key(&key)?;
    info!(run_id, target = %target.key, "[P2PSync] custom dir sync requested");
    run_p2p_sync(Some(&app), &run_id, target, None).await
}

#[tauri::command]
pub async fn p2p_sync_verify_custom(key: String) -> Result<String, String> {
    verify_target(target_from_key(&key)?)
}

/// 终止正在进行的同步（按 run_id）。同步循环会在下一个检查点干净地停止，
/// 已完成的文件保留、半截文件保留 .part 以便下次续传。
#[tauri::command]
pub fn p2p_sync_cancel(run_id: String) -> Result<(), String> {
    info!(run_id, "[P2PSync] cancel requested");
    request_sync_cancel(&run_id);
    Ok(())
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
            key: "knowledge_base".to_string(),
            label: "知识库".to_string(),
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
            key: "resource_repo".to_string(),
            label: "资源库".to_string(),
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

    #[test]
    fn part_path_is_stable_per_rel() {
        let target = P2PSyncTarget {
            key: "resource_repo".to_string(),
            label: "资源库".to_string(),
            root: PathBuf::from("root"),
        };
        let a = part_path(&target, "images/2026/a.png");
        let b = part_path(&target, "images/2026/a.png");
        let c = part_path(&target, "images/2026/b.png");
        assert_eq!(a, b, "same rel must map to the same resume part file");
        assert_ne!(a, c, "different rel must map to different part files");
        assert!(a.to_string_lossy().ends_with(".part"));
    }

    #[test]
    fn connection_errors_are_classified_for_retry() {
        assert!(is_connection_error("接收文件体失败: connection lost"));
        assert!(is_connection_error("打开拉取通道失败: timed out"));
        assert!(is_connection_error("打开推送通道超时"));
        // 业务级错误不应触发重连重试。
        assert!(!is_connection_error("文件不存在: a.png"));
        assert!(!is_connection_error("该文件不在同步范围内: x"));
    }

    #[test]
    fn merge_peer_hints_dedups_and_excludes_self() {
        // 直接构造状态，绕过配置门控验证去重/排除自身逻辑。
        let mut state = P2PStateFile::default();
        state.trusted_peers.push(P2PTrustedPeer {
            peer_id: "peerA".to_string(),
            device_id: "devA".to_string(),
            device_name: "A".to_string(),
            added_at: "t".to_string(),
            address_hint: String::new(),
            online_status: "online".to_string(),
            status_detail: String::new(),
            last_checked_at: String::new(),
        });
        let hints = vec![
            // 已存在但本地缺地址 -> 补地址，不新增。
            P2PPeerHint {
                peer_id: "peerA".to_string(),
                device_id: "devA".to_string(),
                device_name: "A".to_string(),
                address_hint: "addrA".to_string(),
            },
            // 新设备 -> 新增。
            P2PPeerHint {
                peer_id: "peerB".to_string(),
                device_id: "devB".to_string(),
                device_name: "B".to_string(),
                address_hint: "addrB".to_string(),
            },
            // 自身 -> 跳过。
            P2PPeerHint {
                peer_id: "self".to_string(),
                device_id: "devS".to_string(),
                device_name: "S".to_string(),
                address_hint: "addrS".to_string(),
            },
            // 空地址 -> 跳过。
            P2PPeerHint {
                peer_id: "peerC".to_string(),
                device_id: "devC".to_string(),
                device_name: "C".to_string(),
                address_hint: "  ".to_string(),
            },
        ];
        // 仅在开启传播时断言新增数量，避免依赖磁盘配置导致测试不稳定。
        if p2p_auto_propagate_trust() {
            let added = merge_peer_hints(&mut state, &hints, "self");
            assert_eq!(added, 1, "only peerB should be newly added");
            let a = state
                .trusted_peers
                .iter()
                .find(|p| p.peer_id == "peerA")
                .unwrap();
            assert_eq!(a.address_hint, "addrA", "existing peer gets address filled");
            assert!(state.trusted_peers.iter().all(|p| p.peer_id != "self"));
            assert!(state.trusted_peers.iter().all(|p| p.peer_id != "peerC"));
        }
    }

    #[test]
    fn block_delta_wire_roundtrip_reconstructs_source() {
        // 验证 Signature/Delta 经 serde 序列化后仍能 patch 还原 source。
        let sync = SyncBuilder::new().block_size(P2P_BLOCK_SIZE).build();
        let basis = b"guaxi block one\nguaxi block two\nshared tail".to_vec();
        let source = b"guaxi block one CHANGED\nguaxi block two\nshared tail".to_vec();

        let signature = sync.signature(Cursor::new(basis.as_slice())).expect("sig");
        let sig_bytes = serde_json::to_vec(&signature).expect("ser sig");
        let signature: Signature = serde_json::from_slice(&sig_bytes).expect("de sig");

        let delta = sync
            .delta(Cursor::new(source.as_slice()), &signature)
            .expect("delta");
        let delta_bytes = serde_json::to_vec(&delta).expect("ser delta");
        let delta: Delta = serde_json::from_slice(&delta_bytes).expect("de delta");

        let mut patched = Vec::new();
        sync.patch(Cursor::new(basis.as_slice()), &delta, &mut patched)
            .expect("patch");
        assert_eq!(patched, source, "delta patch must reconstruct source");
    }
}
