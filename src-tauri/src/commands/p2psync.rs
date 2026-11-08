use crate::commands::settingview::{
    get_device_id, get_image_bed_root, get_knowledge_bases_root, get_libraries_root,
    get_resource_repo_sync_config, get_slidev_repo_root, get_template_repo_path,
};
use base64::Engine as _;
use blake3::Hasher;
use chrono::{DateTime, Duration, Utc};
use copia::{Delta, Signature, Sync, SyncBuilder};
use futures_util::future::join_all;
use hex;
use iroh::{
    endpoint::{
        presets, Connection, ConnectOptions, IdleTimeout, QuicTransportConfig, RecvStream,
        SendStream, VarInt,
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
// 3: PushFile 增加接收偏移握手（断点续传），与旧版协议不兼容。
const P2P_ALPN: &[u8] = b"guaxi/p2p-sync/3";
const P2P_ONLINE_TIMEOUT_SECS: u64 = 30;
const P2P_FRAME_LIMIT: usize = 1024 * 1024;
/// 数据库记录 bundle 可能远大于普通控制帧，用更大的读取上限（64MB）。
const P2P_BUNDLE_FRAME_LIMIT: usize = 64 * 1024 * 1024;
/// 清单可能包含上万个文件条目，单帧上限放宽到 256MiB。
const P2P_MANIFEST_FRAME_LIMIT: usize = 256 * 1024 * 1024;
/// 块级 signature/delta 帧上限，单文件签名/差异不应超过此值。
const P2P_DELTA_FRAME_LIMIT: usize = 256 * 1024 * 1024;
/// 文件体按固定块流式收发，避免把整文件读进内存。
const P2P_CHUNK_SIZE: usize = 2 * 1024 * 1024;
/// 单次磁盘/流 I/O 的看门狗：120 秒没有任何进展就当作连接卡死，触发重连续传。
const P2P_IO_TIMEOUT_SECS: u64 = 120;
/// 单个文件传输失败后的最大重试次数（含重连）。
const P2P_TRANSFER_MAX_RETRIES: u32 = 8;
/// 低于此大小的文件不做块级 delta，直接整文件续传（delta 开销不划算）。
const P2P_DELTA_MIN_SIZE: u64 = 256 * 1024;
/// 超过 2GB 的文件不再做块级 delta：签名/差异构建的内存与 CPU 开销不划算，
/// 直接走整文件断点续传更稳。
const P2P_DELTA_MAX_SIZE: u64 = 2 * 1024 * 1024 * 1024;
/// 超过 512MB 的下载拆成 4 条区间流并行拉取；每个区间完成后落一个 sidecar，
/// 断线重试只补未完成的区间。
const PARALLEL_PULL_THRESHOLD: u64 = 512 * 1024 * 1024;
const PARALLEL_PULL_RANGES: usize = 4;
/// 上传同样拆 4 个区间；每个区间独立落盘，完成后统一合并。
const PARALLEL_PUSH_THRESHOLD: u64 = 512 * 1024 * 1024;
const PARALLEL_PUSH_RANGES: usize = 4;
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
    /// 是否开放联邦浏览(允许可信设备浏览本机库清单/按需取文件)。缺省视为开放。
    #[serde(default = "default_federation_enabled")]
    federation_enabled: Option<bool>,
}

fn default_federation_enabled() -> Option<bool> {
    Some(true)
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
        /// 只发送该长度（0 表示到文件末尾），用于大文件并行区间拉取。
        #[serde(default)]
        length: u64,
        /// 客户端清单里已有的哈希；非空时服务端不再为每个请求重读整文件。
        #[serde(default)]
        known_hash: String,
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
        #[serde(default)]
        range_index: u32,
        #[serde(default)]
        range_start: u64,
        #[serde(default)]
        range_length: u64,
        #[serde(default)]
        range_count: u32,
        /// 临时传输的稳定标识；同步目标为空。接收端用它把多个区间映射到同一个唯一目标名。
        #[serde(default)]
        transfer_id: String,
    },
    /// 所有推送区间完成后，通知对端合并并校验。
    PushFileComplete {
        target: String,
        rel: String,
        size: u64,
        blake3: String,
        mtime_ms: i64,
        range_count: u32,
        #[serde(default)]
        transfer_id: String,
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
    /// 联邦浏览:请求对端返回可浏览的库清单(仅名称/数量,不落地文件)。
    /// 对端未开放联邦浏览时返回 ok:false。用于"发现其他设备的库"。
    ListLibraries {},
    /// 联邦检索:在对端某个库上跑正文检索(对端本地 tantivy),返回命中(不落地文件)。
    SearchLibrary {
        target: String,
        query: String,
        #[serde(default)]
        offset: usize,
        #[serde(default)]
        limit: usize,
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

/// 联邦库清单里的单个库(对端某个同步目标)。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PLibInfo {
    target: String,
    name: String,
    count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PLibrariesResponse {
    ok: bool,
    message: String,
    #[serde(default)]
    libraries: Vec<P2PLibInfo>,
}

/// 联邦检索单条命中。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PSearchHit {
    rel: String,
    name: String,
    #[serde(default)]
    preview: Option<String>,
    #[serde(default)]
    mtime_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PSearchResponse {
    ok: bool,
    message: String,
    #[serde(default)]
    hits: Vec<P2PSearchHit>,
    #[serde(default)]
    has_more: bool,
}

/// 供 library_site 联邦检索使用的命中(带来源与色带)。
#[derive(Debug, Clone, Serialize)]
pub struct PeerSearchHit {
    pub rel: String,
    pub name: String,
    pub snippet: Option<String>,
    pub mtime_ms: i64,
}

/// 供 library_site 联邦视图使用的对端库(带来源与色带)。
#[derive(Debug, Clone, Serialize)]
pub struct PeerLibrary {
    pub id: String,       // 形如 peer:<peer_id>:<target>
    pub name: String,
    pub peer: String,     // 对端设备名
    pub peer_id: String,
    pub count: u64,
    pub band: String,
}

/// 供 library_site 联邦视图使用的对端文件条目。
#[derive(Debug, Clone, Serialize)]
pub struct PeerFileEntry {
    pub rel: String,
    pub size: u64,
    pub mtime_ms: i64,
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
    /// 服务端已接收的字节数；推送方据此断点续传。
    #[serde(default)]
    received: u64,
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

/// 单个文件传输期间的字节级进度。多个并行区间共享同一个 bytes_done，
/// 这样 UI 不会在几个区间的百分比之间来回跳。
#[derive(Clone)]
struct FileProgress {
    app: Option<AppHandle>,
    run_id: String,
    target: P2PSyncTarget,
    peer: String,
    rel: String,
    index: u64,
    total_files: u64,
    bytes_total: u64,
    bytes_done: Arc<AtomicU64>,
    last_emit: Arc<AtomicU64>,
}

impl FileProgress {
    fn new(
        app: Option<AppHandle>,
        run_id: String,
        target: P2PSyncTarget,
        peer: String,
        rel: String,
        index: u64,
        total_files: u64,
        bytes_total: u64,
    ) -> Self {
        Self {
            app,
            run_id,
            target,
            peer,
            rel,
            index,
            total_files,
            bytes_total,
            bytes_done: Arc::new(AtomicU64::new(0)),
            last_emit: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 续传时把已完成的字节计入进度，避免进度条从 0 重新开始。
    fn seed(&self, value: u64) {
        self.bytes_done.fetch_max(value, Ordering::Relaxed);
        self.last_emit.store(value, Ordering::Relaxed);
    }

    fn add(&self, delta: u64) {
        let current = self.bytes_done.fetch_add(delta, Ordering::Relaxed) + delta;
        let last = self.last_emit.load(Ordering::Relaxed);
        if current.saturating_sub(last) >= 16 * 1024 * 1024 || current >= self.bytes_total {
            self.last_emit.store(current, Ordering::Relaxed);
            emit_p2p_progress(
                self.app.as_ref(),
                &self.run_id,
                &self.target,
                "transfer",
                "running",
                format!("与 {} 传输 {}", self.peer, self.rel),
                self.index,
                self.total_files,
                current,
                self.bytes_total,
                self.rel.clone(),
            );
        }
    }
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

/// P2P endpoint 是否在运行。
/// 它用的是 QUIC（中继 + 打洞），没有固定监听端口，所以设置页不能用 TCP connect 探测，
/// 只能问 iroh 自己。
pub async fn p2p_endpoint_running() -> bool {
    let guard = endpoint_runtime().lock().await;
    guard
        .as_ref()
        .map(|runtime| !runtime.endpoint.is_closed())
        .unwrap_or(false)
}

/// 关闭 P2P endpoint（设置页「服务器」用）。
/// 会把 slot 取空，所以之后调用 `ensure_p2p_endpoint` 会重新建一个（见 `p2p_start_endpoint`）。
/// 正在进行的同步会因连接中断而失败。
pub async fn p2p_stop_endpoint() -> Result<(), String> {
    let runtime = {
        let mut guard = endpoint_runtime().lock().await;
        guard.take()
    };
    match runtime {
        Some(runtime) => {
            runtime.endpoint.close().await;
            info!("[P2PSync] endpoint closed");
            Ok(())
        }
        None => Ok(()),
    }
}

/// 启动 P2P endpoint（供设置页手动启动/重启）。
pub async fn p2p_start_endpoint() -> Result<(), String> {
    ensure_p2p_endpoint().await.map(|_| ())
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
    // 大文件同步至少给 60 秒空闲窗口：10GB 传输期间即使短暂卡顿也不至于整条连接被回收。
    let idle = StdDuration::from_secs(p2p_idle_timeout_secs().max(120));
    let mut builder = QuicTransportConfig::builder();
    if let Ok(idle_timeout) = IdleTimeout::try_from(idle) {
        builder = builder.max_idle_timeout(Some(idle_timeout));
        builder = builder.default_path_max_idle_timeout(idle);
    }
    builder = builder
        .keep_alive_interval(StdDuration::from_secs(3))
        // 多文件并行 + 单文件分片都需要足够的双向流与接收窗口。
        .max_concurrent_bidi_streams(VarInt::from_u32(512))
        .stream_receive_window(VarInt::from_u32(16 * 1024 * 1024))
        .receive_window(VarInt::from_u32(64 * 1024 * 1024));
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
    progress: Option<&FileProgress>,
) -> Result<(), String> {
    send_file_body_range(send, path, size, offset, 0, progress).await
}

/// `length=0` 表示从 offset 发到文件末尾；大于 0 时只发指定区间。
async fn send_file_body_range(
    send: &mut SendStream,
    path: &Path,
    size: u64,
    offset: u64,
    length: u64,
    progress: Option<&FileProgress>,
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
    let available = size.saturating_sub(offset);
    let mut remaining = if length == 0 {
        available
    } else {
        length.min(available)
    };
    let mut buf = vec![0_u8; P2P_CHUNK_SIZE];
    while remaining > 0 {
        if let Some(progress) = progress {
            if is_sync_cancelled(&progress.run_id) {
                return Err("同步已取消".to_string());
            }
        }
        let want = remaining.min(buf.len() as u64) as usize;
        timeout(
            StdDuration::from_secs(P2P_IO_TIMEOUT_SECS),
            f.read_exact(&mut buf[..want]),
        )
        .await
        .map_err(|_| format!("读取待发送文件超时 {}: {want} 字节", path.display()))?
        .map_err(|e| format!("读取待发送文件失败 {}: {e}", path.display()))?;
        timeout(
            StdDuration::from_secs(P2P_IO_TIMEOUT_SECS),
            send.write_all(&buf[..want]),
        )
        .await
        .map_err(|_| "发送文件体超时".to_string())?
        .map_err(|e| format!("发送文件体失败 {}: {e}", path.display()))?;
        if let Some(progress) = progress {
            progress.add(want as u64);
        }
        remaining -= want as u64;
    }
    Ok(())
}

/// 从流按块读取文件体写入文件，O(1) 内存，异步落盘。
async fn recv_file_body(
    recv: &mut RecvStream,
    out: &mut tokio::fs::File,
    size: u64,
    progress: Option<&FileProgress>,
) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    let mut remaining = size;
    let mut buf = vec![0_u8; P2P_CHUNK_SIZE];
    while remaining > 0 {
        if let Some(progress) = progress {
            if is_sync_cancelled(&progress.run_id) {
                return Err("同步已取消".to_string());
            }
        }
        let want = remaining.min(buf.len() as u64) as usize;
        timeout(
            StdDuration::from_secs(P2P_IO_TIMEOUT_SECS),
            recv.read_exact(&mut buf[..want]),
        )
        .await
        .map_err(|_| format!("接收文件体超时: {want} 字节"))?
        .map_err(|e| format!("接收文件体失败: {e}"))?;
        timeout(
            StdDuration::from_secs(P2P_IO_TIMEOUT_SECS),
            out.write_all(&buf[..want]),
        )
        .await
        .map_err(|_| "写入临时文件超时".to_string())?
        .map_err(|e| format!("写入临时文件失败: {e}"))?;
        if let Some(progress) = progress {
            progress.add(want as u64);
        }
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
        P2PWireRequest::ListLibraries {} => {
            let response = if federation_enabled() {
                P2PLibrariesResponse {
                    ok: true,
                    message: "ok".to_string(),
                    libraries: enumerate_browsable_libraries(),
                }
            } else {
                P2PLibrariesResponse {
                    ok: false,
                    message: "对方未开放联邦浏览".to_string(),
                    libraries: Vec::new(),
                }
            };
            write_control(&mut send, &response, "P2P 联邦库清单响应").await?;
        }
        P2PWireRequest::SearchLibrary {
            target,
            query,
            offset,
            limit,
        } => {
            let response = serve_library_search(&target, &query, offset, limit).await;
            write_control(&mut send, &response, "P2P 联邦检索响应").await?;
        }
        P2PWireRequest::PullFile {
            target,
            rel,
            offset,
            length,
            known_hash,
        } => match serve_pull(&target, &rel, &known_hash).await {
            Ok((path, size, blake3, mtime_ms)) => {
                // 续传偏移越界（对端文件已变）时退回从 0 发送整文件。
                let start = if offset <= size { offset } else { 0 };
                let header = P2PFileHeader {
                    ok: true,
                    message: "ok".to_string(),
                    size,
                    blake3,
                    mtime_ms,
                    received: 0,
                };
                write_control(&mut send, &header, "P2P 文件头").await?;
                send_file_body_range(&mut send, &path, size, start, length, None).await?;
            }
            Err(e) => {
                let header = P2PFileHeader {
                    ok: false,
                    message: e,
                    size: 0,
                    blake3: String::new(),
                    mtime_ms: 0,
                    received: 0,
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
            range_index,
            range_start,
            range_length,
            range_count,
            transfer_id,
        } => {
            let ack = match accept_push(
                &target,
                &rel,
                &mut send,
                &mut recv,
                size,
                &blake3,
                mtime_ms,
                range_index,
                range_start,
                range_length,
                range_count,
                &transfer_id,
            )
            .await
            {
                Ok(message) => P2PAck { ok: true, message },
                Err(e) => P2PAck {
                    ok: false,
                    message: e,
                },
            };
            write_control(&mut send, &ack, "P2P 推送响应").await?;
        }
        P2PWireRequest::PushFileComplete {
            target,
            rel,
            size,
            blake3,
            mtime_ms,
            range_count,
            transfer_id,
        } => {
            let ack = match assemble_push_ranges(
                &target,
                &rel,
                size,
                &blake3,
                mtime_ms,
                range_count,
                &transfer_id,
            )
            .await
            {
                Ok(message) => P2PAck { ok: true, message },
                Err(e) => P2PAck {
                    ok: false,
                    message: e,
                },
            };
            write_control(&mut send, &ack, "P2P 推送完成响应").await?;
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
    // 先按上次记录的地址拨号；失败后再只用 peer_id 走发现/中继。
    // 设备换网、DHCP 续租后 address_hint 里的旧 IP 会失效，单靠它会导致长时间连不上。
    let mut addresses = Vec::new();
    if let Ok(addr) = decode_endpoint_addr(&peer.address_hint) {
        addresses.push(addr);
    }
    if let Ok(id) = peer.peer_id.parse::<iroh::EndpointId>() {
        addresses.push(EndpointAddr::from(id));
    }
    if addresses.is_empty() {
        return Err(format!("设备 {} 没有可用地址", peer.device_name));
    }

    let mut last_error = String::new();
    for addr in addresses {
        let options = ConnectOptions::new().with_transport_config(build_transport_config());
        let connecting = match timeout(
            StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS),
            endpoint.connect_with_opts(addr, P2P_ALPN, options),
        )
        .await
        {
            Ok(Ok(connecting)) => connecting,
            Ok(Err(error)) => {
                last_error = format!("连接设备 {} 失败: {error}", peer.device_name);
                continue;
            }
            Err(_) => {
                last_error = format!("连接设备 {} 超时", peer.device_name);
                continue;
            }
        };
        match timeout(StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS), connecting).await {
            Ok(Ok(connection)) => return Ok(connection),
            Ok(Err(error)) => {
                last_error = format!("与设备 {} 握手失败: {error}", peer.device_name);
            }
            Err(_) => {
                last_error = format!("等待设备 {} 握手超时", peer.device_name);
            }
        }
    }
    Err(last_error)
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

// —— 联邦浏览（供 library_site 联邦视图调用）——

/// 本机是否开放联邦浏览。缺省视为开放。
fn federation_enabled() -> bool {
    read_state_file().ok().and_then(|s| s.federation_enabled).unwrap_or(true)
}

/// 本机开放联邦浏览的库（知识库/文件库），带文件数。
fn enumerate_browsable_libraries() -> Vec<P2PLibInfo> {
    let mut out = Vec::new();
    for key in ["knowledge_base", "libraries_root"] {
        if let Ok(target) = target_from_key(key) {
            if !target.root.is_dir() {
                continue;
            }
            let count = scan_manifest(&target).map(|m| m.files.len() as u64).unwrap_or(0);
            out.push(P2PLibInfo { target: key.to_string(), name: target.label.clone(), count });
        }
    }
    out
}

/// 稳定色带：按 peer_id 哈希取一个调色板颜色，让不同设备来源在前端一眼区分。
fn peer_band(peer_id: &str) -> String {
    const PALETTE: [&str; 6] = ["#22863a", "#a626a4", "#e45649", "#0969da", "#986801", "#007c89"];
    let h = blake3::hash(peer_id.as_bytes());
    let idx = (h.as_bytes()[0] as usize) % PALETTE.len();
    PALETTE[idx].to_string()
}

fn parse_peer_lib_id(id: &str) -> Option<(String, String)> {
    let rest = id.strip_prefix("peer:")?;
    let (peer_id, target) = rest.split_once(':')?;
    if peer_id.is_empty() || target.is_empty() {
        return None;
    }
    Some((peer_id.to_string(), target.to_string()))
}

async fn connect_trusted(endpoint: &Endpoint, peer_id: &str) -> Result<Connection, String> {
    let peer = read_state_file()?
        .trusted_peers
        .into_iter()
        .find(|p| p.peer_id == peer_id)
        .ok_or_else(|| "未找到该可信设备".to_string())?;
    connect_peer(endpoint, &peer).await
}

/// 发现页是手动触发的交互，单台离线设备不能拖住整个列表：
/// 各设备并发探测，每台最多等这么久（connect_peer 自身的 30s 超时在这里被外层截断）。
const FEDERATION_DISCOVERY_PEER_TIMEOUT_SECS: u64 = 5;

/// 遍历可信设备，取其开放的库清单，合并返回（带来源与色带）。单个设备失败/超时不影响其余。
pub async fn federation_list_peer_libraries() -> Result<Vec<PeerLibrary>, String> {
    let endpoint = ensure_p2p_endpoint().await?;
    let peers = read_state_file()?.trusted_peers;
    let probes = peers.into_iter().map(|peer| {
        let endpoint = endpoint.clone();
        async move {
            let band = peer_band(&peer.peer_id);
            let attempt = async {
                let conn = connect_peer(&endpoint, &peer).await?;
                let resp: P2PLibrariesResponse = request_response(
                    &conn,
                    &P2PWireRequest::ListLibraries {},
                    P2P_FRAME_LIMIT,
                )
                .await?;
                Ok::<_, String>((peer, band, resp))
            };
            match timeout(
                StdDuration::from_secs(FEDERATION_DISCOVERY_PEER_TIMEOUT_SECS),
                attempt,
            )
            .await
            {
                Ok(Ok((peer, band, resp))) if resp.ok => resp
                    .libraries
                    .into_iter()
                    .map(|lib| PeerLibrary {
                        id: format!("peer:{}:{}", peer.peer_id, lib.target),
                        name: lib.name,
                        peer: peer.device_name.clone(),
                        peer_id: peer.peer_id.clone(),
                        count: lib.count,
                        band: band.clone(),
                    })
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            }
        }
    });
    let mut out = Vec::new();
    for mut part in join_all(probes).await {
        out.append(&mut part);
    }
    Ok(out)
}

/// 取某个联邦库（peer:<peer_id>:<target>）的文件清单。
pub async fn federation_list_files(peer_lib_id: &str) -> Result<Vec<PeerFileEntry>, String> {
    let (peer_id, target) = parse_peer_lib_id(peer_lib_id).ok_or_else(|| "非法联邦库 id".to_string())?;
    let endpoint = ensure_p2p_endpoint().await?;
    let conn = connect_trusted(&endpoint, &peer_id).await?;
    let resp: P2PManifestResponse =
        request_response(&conn, &P2PWireRequest::Manifest { target }, P2P_FRAME_LIMIT).await?;
    let manifest = resp.manifest.ok_or(resp.message)?;
    let mut out: Vec<PeerFileEntry> = manifest
        .files
        .into_iter()
        .map(|(rel, e)| PeerFileEntry { rel, size: e.size, mtime_ms: e.modified_unix_ms })
        .collect();
    out.sort_by(|a, b| b.mtime_ms.cmp(&a.mtime_ms));
    Ok(out)
}

/// 按需从联邦库拉取单个文件到内存（预览/下载），不落地、不触发全量同步。
pub async fn federation_pull_file(peer_lib_id: &str, rel: &str) -> Result<Vec<u8>, String> {
    let (peer_id, target) = parse_peer_lib_id(peer_lib_id).ok_or_else(|| "非法联邦库 id".to_string())?;
    let endpoint = ensure_p2p_endpoint().await?;
    let conn = connect_trusted(&endpoint, &peer_id).await?;
    let (mut send, mut recv) = timeout(
        StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS),
        conn.open_bi(),
    )
    .await
    .map_err(|_| "打开传输通道超时".to_string())?
    .map_err(|e| format!("打开传输通道失败: {e}"))?;
    write_control(
        &mut send,
        &P2PWireRequest::PullFile {
            target,
            rel: rel.to_string(),
            offset: 0,
            length: 0,
            known_hash: String::new(),
        },
        "P2P 拉取请求",
    )
    .await?;
    send.finish().map_err(|e| format!("请求发送失败: {e}"))?;
    let header: P2PFileHeader = read_control(&mut recv, P2P_FRAME_LIMIT, "P2P 文件头").await?;
    if !header.ok {
        return Err(header.message);
    }
    let mut buf = vec![0u8; header.size as usize];
    recv.read_exact(&mut buf)
        .await
        .map_err(|e| format!("接收文件体失败: {e}"))?;
    Ok(buf)
}

/// 对端处理联邦检索：用全局 AppHandle 在本机某库上跑 fulltext（tantivy + jieba）。
async fn serve_library_search(target: &str, query: &str, offset: usize, limit: usize) -> P2PSearchResponse {
    if !federation_enabled() {
        return P2PSearchResponse { ok: false, message: "对方未开放联邦浏览".to_string(), hits: Vec::new(), has_more: false };
    }
    let app = match crate::cognition::runtime::app_handle() {
        Some(a) => a,
        None => {
            return P2PSearchResponse { ok: false, message: "对端 AppHandle 未就绪".to_string(), hits: Vec::new(), has_more: false };
        }
    };
    let root = match target_from_key(target) {
        Ok(t) => t.root,
        Err(e) => return P2PSearchResponse { ok: false, message: e, hits: Vec::new(), has_more: false },
    };
    let root_str = root.to_string_lossy().to_string();
    let lim = if limit == 0 { 50 } else { limit.min(200) };
    match crate::commands::fulltext::fulltext_search(
        app, query.to_string(), root_str, Some(lim), Some(offset), None, None, None, None, None,
    )
    .await
    {
        Ok((hits, has_more)) => {
            let mapped = hits
                .into_iter()
                .map(|h| {
                    let abs = PathBuf::from(&h.path);
                    let rel = abs.strip_prefix(&root).unwrap_or(&abs).to_string_lossy().replace('\\', "/");
                    P2PSearchHit { rel, name: h.file_name, preview: h.preview, mtime_ms: h.modified_ms.unwrap_or(0) }
                })
                .collect();
            P2PSearchResponse { ok: true, message: "ok".to_string(), hits: mapped, has_more }
        }
        Err(e) => P2PSearchResponse { ok: false, message: e, hits: Vec::new(), has_more: false },
    }
}

/// 客户端：在某联邦库上跑跨设备正文检索。
pub async fn federation_search(
    peer_lib_id: &str,
    query: &str,
    offset: usize,
    limit: usize,
) -> Result<(Vec<PeerSearchHit>, bool), String> {
    let (peer_id, target) = parse_peer_lib_id(peer_lib_id).ok_or_else(|| "非法联邦库 id".to_string())?;
    let endpoint = ensure_p2p_endpoint().await?;
    let conn = connect_trusted(&endpoint, &peer_id).await?;
    let req = P2PWireRequest::SearchLibrary { target, query: query.to_string(), offset, limit };
    let resp: P2PSearchResponse = request_response(&conn, &req, P2P_FRAME_LIMIT).await?;
    if !resp.ok {
        return Err(resp.message);
    }
    let hits = resp
        .hits
        .into_iter()
        .map(|h| PeerSearchHit { rel: h.rel, name: h.name, snippet: h.preview, mtime_ms: h.mtime_ms })
        .collect();
    Ok((hits, resp.has_more))
}

/// 前端：开关本机的联邦浏览开放。
#[tauri::command]
pub fn p2p_federation_set_enabled(enabled: bool) -> Result<bool, String> {
    let mut state = read_state_file()?;
    state.federation_enabled = Some(enabled);
    write_state_file(&state)?;
    Ok(enabled)
}

/// 前端：查询本机是否开放联邦浏览。
#[tauri::command]
pub fn p2p_federation_status() -> bool {
    federation_enabled()
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
            // 临时文件直传：与 HTTP 接收目录分开，避免 P2P 元数据混进普通收件箱。
            "transfers" => (
                "临时传输".to_string(),
                PathBuf::from(get_libraries_root())
                    .join("incoming_transfers")
                    .join("p2p")
                    .to_string_lossy()
                    .to_string(),
            ),
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
    // 10GB 级文件用 mmap + 多核 AVX 路径，比 64KB 循环读快一个量级；
    // 网络盘/不支持 mmap 的环境回退到 1MB 缓冲。
    let mut hasher = Hasher::new();
    if hasher.update_mmap(path).is_ok() {
        return Ok(hasher.finalize().to_hex().to_string());
    }
    let mut f = fs::File::open(path).map_err(|e| format!("打开文件失败 {}: {e}", path.display()))?;
    let mut hasher = Hasher::new();
    let mut buf = vec![0_u8; 1024 * 1024];
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

    let previous = read_manifest(target).ok().flatten();
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
        let mtime = file_mtime_ms(path)?;
        // 大小 + 修改时间没变就复用上次清单里的哈希：10GB 级目录重复同步时不用再读一遍全盘。
        let hash = match previous
            .as_ref()
            .and_then(|manifest| manifest.files.get(&rel))
        {
            Some(entry) if entry.size == meta.len() && entry.modified_unix_ms == mtime => {
                entry.blake3.clone()
            }
            _ => hash_file(path)?,
        };
        scanned_files += 1;
        scanned_bytes = scanned_bytes.saturating_add(meta.len());
        files.insert(
            rel.clone(),
            P2PFileEntry {
                size: meta.len(),
                modified_unix_ms: mtime,
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
    // 临时传输不预先算哈希（大文件算一遍太慢），空哈希表示“只做大小/落盘校验”。
    if !expected_hash.is_empty() {
        let actual = hash_file(tmp)?;
        if actual != expected_hash {
            let _ = fs::remove_file(tmp);
            return Err(format!(
                "P2P 文件校验失败：{} 期望 {} 实际 {}",
                rel, expected_hash, actual
            ));
        }
    }

    let dest = safe_join(&target.root, rel)?;
    if !expected_hash.is_empty() && dest.exists() {
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
async fn serve_pull(
    target_key: &str,
    rel: &str,
    known_hash: &str,
) -> Result<(PathBuf, u64, String, i64), String> {
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
    // 清单已经带哈希时直接用；否则才读整文件计算（旧客户端/联邦拉取）。
    let hash = if known_hash.trim().is_empty() {
        hash_file_async(&path).await?
    } else {
        known_hash.to_string()
    };
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
fn unique_transfer_rel(root: &Path, rel: &str) -> Result<String, String> {
    let base = safe_join(root, rel)?;
    if !base.exists() {
        return Ok(rel.to_string());
    }
    let path = Path::new(rel);
    let parent = path
        .parent()
        .map(|value| value.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    let ext = path.extension().and_then(|value| value.to_str()).unwrap_or("");
    for index in 1..10_000 {
        let name = if ext.is_empty() {
            format!("{} ({})", stem, index)
        } else {
            format!("{} ({}).{}", stem, index, ext)
        };
        let candidate = if parent.is_empty() {
            name
        } else {
            format!("{}/{}", parent, name)
        };
        if !root.join(&candidate).exists() {
            return Ok(candidate);
        }
    }
    Err(format!("无法为 {rel} 生成不冲突的文件名"))
}

async fn accept_push(
    target_key: &str,
    rel: &str,
    send: &mut SendStream,
    recv: &mut RecvStream,
    size: u64,
    expected_hash: &str,
    mtime_ms: i64,
    range_index: u32,
    range_start: u64,
    range_length: u64,
    range_count: u32,
    transfer_id: &str,
) -> Result<String, String> {
    use tokio::io::{AsyncSeekExt, AsyncWriteExt};
    let target = target_from_key(target_key)?;
    if should_skip(&target, rel) {
        return Err(format!("该文件不在同步范围内: {rel}"));
    }
    let rel = if target.key == "transfers" {
        unique_transfer_rel(&target.root, rel)?
    } else {
        rel.to_string()
    };
    // 提前校验路径合法性。
    let _ = safe_join(&target.root, &rel)?;

    let tmp_dir = target.root.join(P2P_META_DIR).join(P2P_TMP_DIR);
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("创建 P2P 临时目录失败: {e}"))?;

    if range_count > 0 {
        return accept_push_range(
            &target,
            &rel,
            send,
            recv,
            size,
            expected_hash,
            mtime_ms,
            range_index,
            range_start,
            range_length,
            range_count,
            transfer_id,
        )
        .await;
    }

    // 同步目标使用稳定 part 文件，断线重连后从已接收字节继续；
    // 临时传输不续传，避免同名文件在重试时被误当成同一个目标。
    let (tmp, resume) = if target.key == "transfers" {
        (tmp_dir.join(format!("{}.tmp", Uuid::new_v4())), 0u64)
    } else {
        let part = push_part_path(&target, &rel);
        let mut resume = fs::metadata(&part).map(|meta| meta.len()).unwrap_or(0);
        if resume > size {
            let _ = fs::remove_file(&part);
            resume = 0;
        }
        (part, resume)
    };

    // 先告诉发送端从哪个偏移继续，再接收剩余字节。
    let header = P2PFileHeader {
        ok: true,
        message: "ok".to_string(),
        size,
        blake3: expected_hash.to_string(),
        mtime_ms,
        received: resume,
    };
    write_control(send, &header, "P2P 推送文件头").await?;

    {
        let mut out = tokio::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&tmp)
            .await
            .map_err(|e| format!("创建 P2P 临时文件失败 {}: {e}", tmp.display()))?;
        if resume == 0 {
            out.set_len(0)
                .await
                .map_err(|e| format!("重置 P2P 临时文件失败: {e}"))?;
        } else {
            out.seek(SeekFrom::Start(resume))
                .await
                .map_err(|e| format!("定位 P2P 临时文件失败: {e}"))?;
        }
        recv_file_body(recv, &mut out, size.saturating_sub(resume), None).await?;
        out.flush()
            .await
            .map_err(|e| format!("刷新 P2P 临时文件失败 {}: {e}", tmp.display()))?;
    }

    finalize_incoming_file_async(&target, &rel, &tmp, expected_hash).await?;
    info!(target = %target.key, rel, size, resumed = resume, "[P2PSync] accepted pushed file");
    Ok(format!("已接收 {rel}"))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PushRangeMeta {
    size: u64,
    blake3: String,
    count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PushTransferMeta {
    rel: String,
    final_rel: String,
    size: u64,
    count: u32,
}

fn transfer_meta_path(target: &P2PSyncTarget, transfer_id: &str) -> PathBuf {
    let key = blake3::hash(format!("transfer-meta:{transfer_id}").as_bytes())
        .to_hex()
        .to_string();
    target
        .root
        .join(P2P_META_DIR)
        .join(P2P_TMP_DIR)
        .join(format!("{key}.json"))
}

fn transfer_part_path(target: &P2PSyncTarget, transfer_id: &str) -> PathBuf {
    let key = blake3::hash(format!("transfer-part:{transfer_id}").as_bytes())
        .to_hex()
        .to_string();
    target
        .root
        .join(P2P_META_DIR)
        .join(P2P_TMP_DIR)
        .join(format!("{key}.part"))
}

fn transfer_range_marker(target: &P2PSyncTarget, transfer_id: &str, index: u32) -> PathBuf {
    let key = blake3::hash(format!("transfer-done:{transfer_id}:{index}").as_bytes())
        .to_hex()
        .to_string();
    target
        .root
        .join(P2P_META_DIR)
        .join(P2P_TMP_DIR)
        .join(format!("{key}.done"))
}

fn push_range_marker(target: &P2PSyncTarget, rel: &str, index: u32) -> PathBuf {
    let key = blake3::hash(format!("push-range-done:{rel}:{index}").as_bytes())
        .to_hex()
        .to_string();
    target
        .root
        .join(P2P_META_DIR)
        .join(P2P_TMP_DIR)
        .join(format!("{key}.done"))
}

fn push_range_meta_path(target: &P2PSyncTarget, rel: &str) -> PathBuf {
    let key = blake3::hash(format!("push-range-meta:{rel}").as_bytes())
        .to_hex()
        .to_string();
    target
        .root
        .join(P2P_META_DIR)
        .join(P2P_TMP_DIR)
        .join(format!("{key}.json"))
}

fn push_range_bounds(size: u64, count: u32, index: u32) -> (u64, u64) {
    let range_size = (size + count as u64 - 1) / count as u64;
    let start = range_size * index as u64;
    (start, range_size.min(size.saturating_sub(start)))
}

async fn accept_push_range(
    target: &P2PSyncTarget,
    rel: &str,
    send: &mut SendStream,
    recv: &mut RecvStream,
    size: u64,
    expected_hash: &str,
    _mtime_ms: i64,
    range_index: u32,
    range_start: u64,
    range_length: u64,
    range_count: u32,
    transfer_id: &str,
) -> Result<String, String> {
    use tokio::io::{AsyncSeekExt, AsyncWriteExt};
    if target.key == "transfers" && transfer_id.trim().is_empty() {
        return Err("临时传输并行区间缺少 transfer_id".to_string());
    }
    let (expected_start, expected_length) = push_range_bounds(size, range_count, range_index);
    if range_start != expected_start || range_length != expected_length {
        return Err(format!(
            "推送区间参数不一致：start {} != {} 或 length {} != {}",
            range_start, expected_start, range_length, expected_length
        ));
    }
    let is_transfer = target.key == "transfers";
    let meta_path;
    let part;
    let marker;
    if is_transfer {
        meta_path = transfer_meta_path(target, transfer_id);
        part = transfer_part_path(target, transfer_id);
        let same_file = fs::read_to_string(&meta_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<PushTransferMeta>(&raw).ok())
            .map(|old| old.rel == rel && old.size == size && old.count == range_count)
            .unwrap_or(false);
        if !same_file {
            let _ = fs::remove_file(&part);
            for index in 0..range_count {
                let _ = fs::remove_file(transfer_range_marker(target, transfer_id, index));
            }
            let meta = PushTransferMeta {
                rel: rel.to_string(),
                final_rel: unique_transfer_rel(&target.root, rel)?,
                size,
                count: range_count,
            };
            fs::write(
                &meta_path,
                serde_json::to_vec(&meta).map_err(|e| e.to_string())?,
            )
            .map_err(|e| format!("写入临时传输元数据失败: {e}"))?;
            let file = tokio::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&part)
                .await
                .map_err(|e| format!("创建临时传输文件失败: {e}"))?;
            file.set_len(size)
                .await
                .map_err(|e| format!("预分配临时传输文件失败: {e}"))?;
        }
        marker = transfer_range_marker(target, transfer_id, range_index);
    } else {
        meta_path = push_range_meta_path(target, rel);
        let meta = PushRangeMeta {
            size,
            blake3: expected_hash.to_string(),
            count: range_count,
        };
        let same_file = fs::read_to_string(&meta_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<PushRangeMeta>(&raw).ok())
            .map(|old| old == meta)
            .unwrap_or(false);
        part = push_part_path(target, rel);
        if !same_file {
            let _ = fs::remove_file(&part);
            for index in 0..range_count {
                let _ = fs::remove_file(push_range_marker(target, rel, index));
            }
            fs::write(
                &meta_path,
                serde_json::to_vec(&meta).map_err(|e| e.to_string())?,
            )
            .map_err(|e| format!("写入区间元数据失败: {e}"))?;
            let file = tokio::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&part)
                .await
                .map_err(|e| format!("创建并行上传临时文件失败: {e}"))?;
            file.set_len(size)
                .await
                .map_err(|e| format!("预分配并行上传临时文件失败: {e}"))?;
        }
        marker = push_range_marker(target, rel, range_index);
    }
    let received = if marker.exists() { expected_length } else { 0 };
    let header = P2PFileHeader {
        ok: true,
        message: "ok".to_string(),
        size,
        blake3: expected_hash.to_string(),
        mtime_ms: _mtime_ms,
        received,
    };
    write_control(send, &header, "P2P 区间文件头").await?;

    if received < expected_length {
        let mut out = tokio::fs::OpenOptions::new()
            .write(true)
            .open(&part)
            .await
            .map_err(|e| format!("打开并行上传临时文件失败: {e}"))?;
        out.seek(SeekFrom::Start(range_start))
            .await
            .map_err(|e| format!("定位并行上传区间失败: {e}"))?;
        recv_file_body(recv, &mut out, expected_length, None).await?;
        out.flush()
            .await
            .map_err(|e| format!("刷新并行上传区间失败: {e}"))?;
        fs::write(&marker, b"done").map_err(|e| format!("写入区间完成标记失败: {e}"))?;
    }
    Ok(format!(
        "已接收区间 {}/{}（{}+{}）",
        range_index + 1,
        range_count,
        range_start,
        expected_length
    ))
}

async fn assemble_push_ranges(
    target_key: &str,
    rel: &str,
    size: u64,
    expected_hash: &str,
    _mtime_ms: i64,
    range_count: u32,
    transfer_id: &str,
) -> Result<String, String> {
    if range_count == 0 {
        return Err("区间数量不能为空".to_string());
    }
    let target = target_from_key(target_key)?;
    if should_skip(&target, rel) {
        return Err(format!("该文件不在同步范围内: {rel}"));
    }
    let is_transfer = target.key == "transfers";
    let (meta_path, part, final_rel) = if is_transfer {
        if transfer_id.trim().is_empty() {
            return Err("临时传输并行区间缺少 transfer_id".to_string());
        }
        let meta_path = transfer_meta_path(&target, transfer_id);
        let meta: PushTransferMeta = fs::read_to_string(&meta_path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .ok_or_else(|| "临时传输元数据不存在，请重新发送".to_string())?;
        if meta.rel != rel || meta.size != size || meta.count != range_count {
            return Err("临时传输元数据与当前文件不一致，请重新发送".to_string());
        }
        (
            meta_path,
            transfer_part_path(&target, transfer_id),
            meta.final_rel,
        )
    } else {
        let _ = safe_join(&target.root, rel)?;
        let meta_path = push_range_meta_path(&target, rel);
        let meta = PushRangeMeta {
            size,
            blake3: expected_hash.to_string(),
            count: range_count,
        };
        let same_file = fs::read_to_string(&meta_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<PushRangeMeta>(&raw).ok())
            .map(|old| old == meta)
            .unwrap_or(false);
        if !same_file {
            return Err("区间元数据与当前文件不一致，请重新上传".to_string());
        }
        (meta_path, push_part_path(&target, rel), rel.to_string())
    };

    let marker_for = |index: u32| {
        if is_transfer {
            transfer_range_marker(&target, transfer_id, index)
        } else {
            push_range_marker(&target, rel, index)
        }
    };
    for index in 0..range_count {
        if !marker_for(index).exists() {
            return Err(format!("区间 {} 未完成", index));
        }
    }
    let size_ok = fs::metadata(&part)
        .map(|meta| meta.len() == size)
        .unwrap_or(false);
    if !size_ok {
        let _ = fs::remove_file(&part);
        for index in 0..range_count {
            let _ = fs::remove_file(marker_for(index));
        }
        return Err("并行上传临时文件大小不符".to_string());
    }

    if let Err(error) = finalize_incoming_file_async(&target, &final_rel, &part, expected_hash).await {
        let _ = fs::remove_file(&part);
        for index in 0..range_count {
            let _ = fs::remove_file(marker_for(index));
        }
        return Err(error);
    }
    for index in 0..range_count {
        let _ = fs::remove_file(marker_for(index));
    }
    let _ = fs::remove_file(&meta_path);
    info!(target = %target.key, rel = %final_rel, size, range_count, "[P2PSync] assembled pushed ranges");
    Ok(format!("已接收 {final_rel}"))
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
fn push_part_path(target: &P2PSyncTarget, rel: &str) -> PathBuf {
    let key = blake3::hash(format!("push:{rel}").as_bytes()).to_hex().to_string();
    target
        .root
        .join(P2P_META_DIR)
        .join(P2P_TMP_DIR)
        .join(format!("{key}.part"))
}

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
fn parallel_marker(part: &Path) -> PathBuf {
    PathBuf::from(format!("{}.parallel", part.to_string_lossy()))
}

fn range_marker(part: &Path, index: usize) -> PathBuf {
    PathBuf::from(format!("{}.r{}.done", part.to_string_lossy(), index))
}

async fn pull_file_parallel(
    connection: &Connection,
    target: &P2PSyncTarget,
    rel: &str,
    expected: &P2PFileEntry,
    progress: &FileProgress,
) -> Result<u64, String> {
    use futures_util::stream::{self, StreamExt};
    use tokio::io::{AsyncSeekExt, AsyncWriteExt};

    let tmp_dir = target.root.join(P2P_META_DIR).join(P2P_TMP_DIR);
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("创建 P2P 临时目录失败: {e}"))?;
    let part = part_path(target, rel);
    let marker = parallel_marker(&part);
    fs::write(&marker, b"parallel").map_err(|e| format!("创建并行拉取标记失败: {e}"))?;

    let same_size = fs::metadata(&part)
        .map(|meta| meta.len() == expected.size)
        .unwrap_or(false);
    if !same_size {
        for index in 0..PARALLEL_PULL_RANGES {
            let _ = fs::remove_file(range_marker(&part, index));
        }
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&part)
            .await
            .map_err(|e| format!("创建 P2P 临时文件失败 {}: {e}", part.display()))?;
        file.set_len(expected.size)
            .await
            .map_err(|e| format!("预分配 P2P 临时文件失败: {e}"))?;
    }

    let range_size = (expected.size + PARALLEL_PULL_RANGES as u64 - 1)
        / PARALLEL_PULL_RANGES as u64;
    let completed: u64 = (0..PARALLEL_PULL_RANGES)
        .filter(|index| range_marker(&part, *index).exists())
        .map(|index| {
            let start = index as u64 * range_size;
            range_size.min(expected.size.saturating_sub(start))
        })
        .sum();
    progress.seed(completed);
    let tasks = (0..PARALLEL_PULL_RANGES).map(|index| {
        let connection = connection.clone();
        let target = target.clone();
        let rel = rel.to_string();
        let expected = (*expected).clone();
        let part = part.clone();
        let progress = (*progress).clone();
        async move {
            let done = range_marker(&part, index);
            if done.exists() {
                return Ok::<(), String>(());
            }
            let start = index as u64 * range_size;
            if start >= expected.size {
                return Ok(());
            }
            let length = range_size.min(expected.size - start);
            let (mut send, mut recv) = timeout(
                StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS),
                connection.open_bi(),
            )
            .await
            .map_err(|_| "打开并行拉取通道超时".to_string())?
            .map_err(|e| format!("打开并行拉取通道失败: {e}"))?;
            write_control(
                &mut send,
                &P2PWireRequest::PullFile {
                    target: target.key.clone(),
                    rel: rel.clone(),
                    offset: start,
                    length,
                    known_hash: expected.blake3.clone(),
                },
                "P2P 区间拉取请求",
            )
            .await?;
            send.finish()
                .map_err(|e| format!("区间拉取请求完成发送失败: {e}"))?;
            let header: P2PFileHeader =
                read_control(&mut recv, P2P_FRAME_LIMIT, "P2P 区间文件头").await?;
            if !header.ok {
                return Err(header.message);
            }
            if header.size != expected.size {
                return Err(format!("对端文件大小已变化：{} != {}", header.size, expected.size));
            }
            let mut out = tokio::fs::OpenOptions::new()
                .write(true)
                .open(&part)
                .await
                .map_err(|e| format!("打开 P2P 临时文件失败: {e}"))?;
            out.seek(SeekFrom::Start(start))
                .await
                .map_err(|e| format!("定位并行拉取区间失败: {e}"))?;
            recv_file_body(&mut recv, &mut out, length, Some(&progress)).await?;
            out.flush()
                .await
                .map_err(|e| format!("刷新并行拉取区间失败: {e}"))?;
            fs::write(&done, b"done").map_err(|e| format!("写入区间完成标记失败: {e}"))?;
            Ok(())
        }
    });

    let results = stream::iter(tasks)
        .buffer_unordered(PARALLEL_PULL_RANGES)
        .collect::<Vec<_>>()
        .await;
    if let Some(Err(error)) = results.into_iter().find(|result| result.is_err()) {
        return Err(error);
    }
    for index in 0..PARALLEL_PULL_RANGES {
        if expected.size > index as u64 * range_size && !range_marker(&part, index).exists() {
            return Err(format!("并行拉取区间 {} 未完成", index));
        }
    }

    finalize_incoming_file_async(target, rel, &part, &expected.blake3).await?;
    for index in 0..PARALLEL_PULL_RANGES {
        let _ = fs::remove_file(range_marker(&part, index));
    }
    let _ = fs::remove_file(&marker);
    info!(
        target = %target.key,
        rel,
        size = expected.size,
        ranges = PARALLEL_PULL_RANGES,
        "[P2PSync] pulled file with parallel ranges"
    );
    Ok(expected.size)
}

async fn pull_file_full(
    connection: &Connection,
    target: &P2PSyncTarget,
    rel: &str,
    expected: &P2PFileEntry,
    progress: &FileProgress,
) -> Result<u64, String> {
    use tokio::io::AsyncWriteExt;

    let tmp_dir = target.root.join(P2P_META_DIR).join(P2P_TMP_DIR);
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("创建 P2P 临时目录失败: {e}"))?;
    let part = part_path(target, rel);

    if expected.size >= PARALLEL_PULL_THRESHOLD
        && (parallel_marker(&part).exists() || !part.exists())
    {
        return pull_file_parallel(connection, target, rel, expected, progress).await;
    }

    // 已有断点：取其当前长度作为续传偏移；若超过期望大小说明对端文件已变，丢弃重来。
    let mut offset = match fs::metadata(&part) {
        Ok(m) => m.len(),
        Err(_) => 0,
    };
    if offset > expected.size {
        let _ = fs::remove_file(&part);
        offset = 0;
    }
    progress.seed(offset);

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
            length: 0,
            known_hash: expected.blake3.clone(),
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
        recv_file_body(&mut recv, &mut out, body_len, Some(progress)).await?;
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
    progress: &FileProgress,
) -> Result<u64, String> {
    let basis = safe_join(&target.root, rel)?;
    let can_delta = p2p_block_delta_enabled()
        && basis.is_file()
        && expected.size >= P2P_DELTA_MIN_SIZE
        && expected.size <= P2P_DELTA_MAX_SIZE;
    if can_delta {
        match pull_file_delta(connection, target, rel).await {
            Ok(n) => return Ok(n),
            Err(e) => {
                info!(target = %target.key, rel, error = e, "[P2PSync] delta pull failed, falling back to full");
            }
        }
    }
    pull_file_full(connection, target, rel, expected, progress).await
}

/// 客户端：把本地单个文件推送给对端。复用清单中已算好的哈希，避免重复 blake3。
async fn push_file_parallel(
    connection: &Connection,
    target: &P2PSyncTarget,
    rel: &str,
    entry: &P2PFileEntry,
    progress: &FileProgress,
) -> Result<u64, String> {
    use futures_util::stream::{self, StreamExt};
    let size = entry.size;
    let count = PARALLEL_PUSH_RANGES as u32;
    let path = safe_join(&target.root, rel)?;

    let tasks = (0..count).map(|index| {
        let connection = connection.clone();
        let target = target.clone();
        let rel = rel.to_string();
        let entry = entry.clone();
        let progress = (*progress).clone();
        let path = path.clone();
        async move {
            let (start, length) = push_range_bounds(size, count, index);
            if length == 0 {
                return Ok::<(), String>(());
            }
            let (mut send, mut recv) = timeout(
                StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS),
                connection.open_bi(),
            )
            .await
            .map_err(|_| "打开上传区间通道超时".to_string())?
            .map_err(|e| format!("打开上传区间通道失败: {e}"))?;
            write_control(
                &mut send,
                &P2PWireRequest::PushFile {
                    target: target.key.clone(),
                    rel: rel.clone(),
                    size,
                    blake3: entry.blake3.clone(),
                    mtime_ms: entry.modified_unix_ms,
                    range_index: index,
                    range_start: start,
                    range_length: length,
                    range_count: count,
                    transfer_id: String::new(),
                },
                "P2P 区间推送请求",
            )
            .await?;
            let header: P2PFileHeader =
                read_control(&mut recv, P2P_FRAME_LIMIT, "P2P 区间文件头").await?;
            if !header.ok {
                return Err(header.message);
            }
            let received = header.received.min(length);
            if received < length {
                send_file_body_range(
                    &mut send,
                    &path,
                    size,
                    start + received,
                    length - received,
                    Some(&progress),
                )
                .await?;
            }
            send.finish()
                .map_err(|e| format!("区间推送完成发送失败: {e}"))?;
            let ack: P2PAck =
                read_control(&mut recv, P2P_FRAME_LIMIT, "P2P 区间推送响应").await?;
            if !ack.ok {
                return Err(ack.message);
            }
            Ok(())
        }
    });

    let results = stream::iter(tasks)
        .buffer_unordered(PARALLEL_PUSH_RANGES)
        .collect::<Vec<_>>()
        .await;
    if let Some(Err(error)) = results.into_iter().find(|result| result.is_err()) {
        return Err(error);
    }

    let (mut send, mut recv) = timeout(
        StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS),
        connection.open_bi(),
    )
    .await
    .map_err(|_| "打开上传完成通道超时".to_string())?
    .map_err(|e| format!("打开上传完成通道失败: {e}"))?;
    write_control(
        &mut send,
        &P2PWireRequest::PushFileComplete {
            target: target.key.clone(),
            rel: rel.to_string(),
            size,
            blake3: entry.blake3.clone(),
            mtime_ms: entry.modified_unix_ms,
            range_count: count,
            transfer_id: String::new(),
        },
        "P2P 区间完成请求",
    )
    .await?;
    send.finish()
        .map_err(|e| format!("区间完成请求发送失败: {e}"))?;
    let ack: P2PAck = read_control(&mut recv, P2P_FRAME_LIMIT, "P2P 区间完成响应").await?;
    if !ack.ok {
        return Err(ack.message);
    }
    info!(target = %target.key, rel, size, count, "[P2PSync] pushed file with parallel ranges");
    Ok(size)
}

async fn push_file(
    connection: &Connection,
    target: &P2PSyncTarget,
    rel: &str,
    entry: &P2PFileEntry,
    progress: &FileProgress,
) -> Result<u64, String> {
    if entry.size >= PARALLEL_PUSH_THRESHOLD && target.key != "transfers" {
        return push_file_parallel(connection, target, rel, entry, progress).await;
    }
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
            range_index: 0,
            range_start: 0,
            range_length: 0,
            range_count: 0,
            transfer_id: String::new(),
        },
        "P2P 推送请求",
    )
    .await?;
    let header: P2PFileHeader = read_control(&mut recv, P2P_FRAME_LIMIT, "P2P 推送文件头").await?;
    if !header.ok {
        return Err(header.message);
    }
    let start = header.received.min(size);
    progress.seed(start);
    send_file_body(&mut send, &path, size, start, Some(progress)).await?;
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
                let backoff = 300u64
                    .saturating_mul(1u64 << attempt.min(4))
                    .min(5_000);
                // 并发文件的多个重试加抖动，避免同时重连把刚恢复的连接再次打满。
                let jitter = rand::random::<u64>() % 400;
                tokio::time::sleep(StdDuration::from_millis(backoff + jitter)).await;
                continue;
            }
        };
        match op(connection.clone()).await {
            Ok(n) => return Ok(n),
            Err(e) => {
                last_err = e.clone();
                if is_connection_error(&e) {
                    link.invalidate(&connection).await;
                    let backoff = 300u64
                        .saturating_mul(1u64 << attempt.min(4))
                        .min(5_000);
                    let jitter = rand::random::<u64>() % 400;
                    tokio::time::sleep(StdDuration::from_millis(backoff + jitter)).await;
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
                    let progress = FileProgress::new(
                        app_owned.clone(),
                        run_id.clone(),
                        target.clone(),
                        peer_name.clone(),
                        rel.clone(),
                        cur,
                        plan_total,
                        entry.size,
                    );
                    let progress_for_retry = progress.clone();
                    let res = transfer_with_retry(&link, |conn| {
                        let target = target.clone();
                        let rel = rel.clone();
                        let entry = entry.clone();
                        let progress = progress_for_retry.clone();
                        async move { pull_file(&conn, &target, &rel, &entry, &progress).await }
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
                    let progress = FileProgress::new(
                        app_owned.clone(),
                        run_id.clone(),
                        target.clone(),
                        peer_name.clone(),
                        rel.clone(),
                        cur,
                        plan_total,
                        entry.size,
                    );
                    let progress_for_retry = progress.clone();
                    let res = transfer_with_retry(&link, |conn| {
                        let target = target.clone();
                        let rel = rel.clone();
                        let entry = entry.clone();
                        let progress = progress_for_retry.clone();
                        async move { push_file(&conn, &target, &rel, &entry, &progress).await }
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

#[derive(Clone, Serialize)]
pub struct P2PFileProgress {
    pub peer_id: String,
    pub device_name: String,
    pub file: String,
    pub index: u64,
    pub total_files: u64,
    pub bytes_current: u64,
    pub bytes_total: u64,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct P2PSendFile {
    pub path: String,
    #[serde(default)]
    pub relative_path: String,
}

/// 通过已配对的 P2P 设备直传临时文件，落盘到对端的 incoming_transfers/p2p。
/// 不计算整文件哈希：临时传输优先速度，服务端只校验大小与落盘。
#[tauri::command]
pub async fn p2p_send_files(
    app: AppHandle,
    peer_id: String,
    files: Vec<P2PSendFile>,
) -> Result<String, String> {
    if files.is_empty() {
        return Err("没有要发送的文件".to_string());
    }
    let endpoint = ensure_p2p_endpoint().await?;
    let state = read_state_file()?;
    let peer = state
        .trusted_peers
        .iter()
        .find(|item| item.peer_id == peer_id)
        .cloned()
        .ok_or_else(|| "该设备还没有配对".to_string())?;
    let link = Arc::new(PeerLink::new(endpoint.clone(), peer.clone()));
    let total_files = files.len() as u64;
    let mut sent = 0u64;
    let mut errors = Vec::new();

    for (index, file) in files.iter().enumerate() {
        let path = PathBuf::from(file.path.trim());
        let meta = match tokio::fs::metadata(&path).await {
            Ok(value) if value.is_file() => value,
            _ => continue,
        };
        let relative = if file.relative_path.trim().is_empty() {
            path.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("file")
                .to_string()
        } else {
            file.relative_path.trim().replace('\\', "/")
        };
        // 传输 id 在整个文件的多区间/多次重试中保持不变，接收端据此续传与命名。
        let transfer_id = Uuid::new_v4().to_string();
        let result = transfer_with_retry(&link, |connection| {
            let app = app.clone();
            let peer = peer.clone();
            let path = path.clone();
            let relative = relative.clone();
            let transfer_id = transfer_id.clone();
            let size = meta.len();
            async move {
                push_transfer_file(
                    &app,
                    &connection,
                    &peer,
                    &path,
                    &relative,
                    size,
                    index as u64,
                    total_files,
                    &transfer_id,
                )
                .await
                .map(|_| size)
            }
        })
        .await;
        match result {
            Ok(_) => sent += 1,
            Err(error) => errors.push(format!("{}: {}", relative, error)),
        }
    }
    link.close().await;

    let message = if errors.is_empty() {
        format!("已向 {} 发送 {} 个文件", peer.device_name, sent)
    } else {
        format!(
            "已向 {} 发送 {} 个文件，失败 {} 个：{}",
            peer.device_name,
            sent,
            errors.len(),
            errors.first().cloned().unwrap_or_default()
        )
    };
    let _ = app.emit(
        "p2p-file-progress",
        P2PFileProgress {
            peer_id: peer.peer_id.clone(),
            device_name: peer.device_name.clone(),
            file: String::new(),
            index: total_files,
            total_files,
            bytes_current: 0,
            bytes_total: 0,
            status: "done".to_string(),
            message: message.clone(),
        },
    );
    Ok(message)
}

async fn push_transfer_file(
    app: &AppHandle,
    connection: &Connection,
    peer: &P2PTrustedPeer,
    source: &Path,
    relative: &str,
    size: u64,
    index: u64,
    total_files: u64,
    transfer_id: &str,
) -> Result<(), String> {
    if size >= PARALLEL_PUSH_THRESHOLD {
        return push_transfer_file_parallel(
            app,
            connection,
            peer,
            source,
            relative,
            size,
            index,
            total_files,
            transfer_id,
        )
        .await;
    }
    use tokio::io::AsyncReadExt;
    let (mut send, mut recv) = timeout(
        StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS),
        connection.open_bi(),
    )
    .await
    .map_err(|_| "打开 P2P 传输通道超时".to_string())?
    .map_err(|e| format!("打开 P2P 传输通道失败: {e}"))?;

    let mtime_ms = std::fs::metadata(source)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|value| value.as_millis() as i64)
        .unwrap_or(0);

    let request = P2PWireRequest::PushFile {
        target: "transfers".to_string(),
        rel: relative.to_string(),
        size,
        blake3: String::new(),
        mtime_ms,
        range_index: 0,
        range_start: 0,
        range_length: 0,
        range_count: 0,
        transfer_id: transfer_id.to_string(),
    };
    write_control(&mut send, &request, "P2P 文件推送请求").await?;
    let header: P2PFileHeader = read_control(&mut recv, P2P_FRAME_LIMIT, "P2P 推送文件头").await?;
    if !header.ok {
        return Err(header.message);
    }

    let mut file = tokio::fs::File::open(source)
        .await
        .map_err(|e| format!("打开待发送文件失败: {e}"))?;
    let start = header.received.min(size);
    let mut sent = start;
    if start > 0 {
        use tokio::io::AsyncSeekExt;
        file.seek(SeekFrom::Start(start))
            .await
            .map_err(|e| format!("定位待发送文件失败: {e}"))?;
    }
    let mut buffer = vec![0u8; P2P_CHUNK_SIZE];
    let mut last_emit = start;
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|e| format!("读取待发送文件失败: {e}"))?;
        if read == 0 {
            break;
        }
        send.write_all(&buffer[..read])
            .await
            .map_err(|e| format!("发送文件体失败: {e}"))?;
        sent += read as u64;
        if sent - last_emit >= 4 * 1024 * 1024 || sent == size {
            last_emit = sent;
            let _ = app.emit(
                "p2p-file-progress",
                P2PFileProgress {
                    peer_id: peer.peer_id.clone(),
                    device_name: peer.device_name.clone(),
                    file: relative.to_string(),
                    index,
                    total_files,
                    bytes_current: sent,
                    bytes_total: size,
                    status: "running".to_string(),
                    message: String::new(),
                },
            );
        }
    }
    send.finish()
        .map_err(|e| format!("P2P 推送完成发送失败: {e}"))?;
    let ack: P2PAck = read_control(&mut recv, P2P_FRAME_LIMIT, "P2P 推送响应").await?;
    if !ack.ok {
        return Err(ack.message);
    }
    info!(
        peer = %peer.device_name,
        relative,
        size,
        "[P2PSync] transfer file pushed"
    );
    Ok(())
}

async fn push_transfer_file_parallel(
    app: &AppHandle,
    connection: &Connection,
    peer: &P2PTrustedPeer,
    source: &Path,
    relative: &str,
    size: u64,
    index: u64,
    total_files: u64,
    transfer_id: &str,
) -> Result<(), String> {
    use futures_util::stream::{self, StreamExt};
    let count = PARALLEL_PUSH_RANGES as u32;
    let mtime_ms = std::fs::metadata(source)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|value| value.as_millis() as i64)
        .unwrap_or(0);
    let bytes_done = Arc::new(AtomicU64::new(0));
    let _ = app.emit(
        "p2p-file-progress",
        P2PFileProgress {
            peer_id: peer.peer_id.clone(),
            device_name: peer.device_name.clone(),
            file: relative.to_string(),
            index,
            total_files,
            bytes_current: 0,
            bytes_total: size,
            status: "running".to_string(),
            message: String::new(),
        },
    );

    let tasks = (0..count).map(|range_index| {
        let connection = connection.clone();
        let app = app.clone();
        let peer = peer.clone();
        let source = source.to_path_buf();
        let relative = relative.to_string();
        let transfer_id = transfer_id.to_string();
        let bytes_done = bytes_done.clone();
        async move {
            let (start, length) = push_range_bounds(size, count, range_index);
            if length == 0 {
                return Ok::<(), String>(());
            }
            let (mut send, mut recv) = timeout(
                StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS),
                connection.open_bi(),
            )
            .await
            .map_err(|_| "打开临时传输区间通道超时".to_string())?
            .map_err(|e| format!("打开临时传输区间通道失败: {e}"))?;
            write_control(
                &mut send,
                &P2PWireRequest::PushFile {
                    target: "transfers".to_string(),
                    rel: relative.clone(),
                    size,
                    blake3: String::new(),
                    mtime_ms,
                    range_index,
                    range_start: start,
                    range_length: length,
                    range_count: count,
                    transfer_id: transfer_id.clone(),
                },
                "P2P 临时传输区间请求",
            )
            .await?;
            let header: P2PFileHeader =
                read_control(&mut recv, P2P_FRAME_LIMIT, "P2P 临时传输区间文件头").await?;
            if !header.ok {
                return Err(header.message);
            }
            let received = header.received.min(length);
            if received < length {
                send_file_body_range(
                    &mut send,
                    &source,
                    size,
                    start + received,
                    length - received,
                    None,
                )
                .await?;
            }
            send.finish()
                .map_err(|e| format!("临时传输区间完成发送失败: {e}"))?;
            let ack: P2PAck =
                read_control(&mut recv, P2P_FRAME_LIMIT, "P2P 临时传输区间响应").await?;
            if !ack.ok {
                return Err(ack.message);
            }
            let current = bytes_done.fetch_add(length, Ordering::Relaxed) + length;
            let _ = app.emit(
                "p2p-file-progress",
                P2PFileProgress {
                    peer_id: peer.peer_id.clone(),
                    device_name: peer.device_name.clone(),
                    file: relative,
                    index,
                    total_files,
                    bytes_current: current.min(size),
                    bytes_total: size,
                    status: "running".to_string(),
                    message: String::new(),
                },
            );
            Ok(())
        }
    });

    let results = stream::iter(tasks)
        .buffer_unordered(PARALLEL_PUSH_RANGES)
        .collect::<Vec<_>>()
        .await;
    if let Some(Err(error)) = results.into_iter().find(|result| result.is_err()) {
        return Err(error);
    }

    let (mut send, mut recv) = timeout(
        StdDuration::from_secs(P2P_ONLINE_TIMEOUT_SECS),
        connection.open_bi(),
    )
    .await
    .map_err(|_| "打开临时传输完成通道超时".to_string())?
    .map_err(|e| format!("打开临时传输完成通道失败: {e}"))?;
    write_control(
        &mut send,
        &P2PWireRequest::PushFileComplete {
            target: "transfers".to_string(),
            rel: relative.to_string(),
            size,
            blake3: String::new(),
            mtime_ms,
            range_count: count,
            transfer_id: transfer_id.to_string(),
        },
        "P2P 临时传输完成请求",
    )
    .await?;
    send.finish()
        .map_err(|e| format!("临时传输完成请求发送失败: {e}"))?;
    let ack: P2PAck =
        read_control(&mut recv, P2P_FRAME_LIMIT, "P2P 临时传输完成响应").await?;
    if !ack.ok {
        return Err(ack.message);
    }
    info!(
        peer = %peer.device_name,
        relative,
        size,
        count,
        "[P2PSync] transfer file pushed with parallel ranges"
    );
    Ok(())
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

    #[test]
    fn unique_transfer_rel_keeps_directory_and_avoids_overwrite() {
        let root = env::temp_dir().join(format!("guaxi-transfer-test-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("docs")).expect("create docs");
        fs::write(root.join("docs/report.pdf"), b"old").expect("write old");

        assert_eq!(
            unique_transfer_rel(&root, "docs/new.pdf").expect("new rel"),
            "docs/new.pdf"
        );
        assert_eq!(
            unique_transfer_rel(&root, "docs/report.pdf").expect("unique rel"),
            "docs/report (1).pdf"
        );
        assert!(unique_transfer_rel(&root, "../escape.pdf").is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn push_and_pull_part_files_do_not_collide() {
        let target = P2PSyncTarget {
            key: "resource_repo".to_string(),
            label: "资源库".to_string(),
            root: PathBuf::from("root"),
        };
        let push = push_part_path(&target, "big.bin");
        let pull = part_path(&target, "big.bin");
        assert_ne!(push, pull, "push and pull temp files must be separate");
        assert_eq!(push, push_part_path(&target, "big.bin"));
    }

    #[test]
    fn transfer_chunk_is_large_enough_for_throughput() {
        assert!(
            P2P_CHUNK_SIZE >= 1024 * 1024,
            "10GB 级传输需要至少 1MB 的块，当前 {}",
            P2P_CHUNK_SIZE
        );
    }

    #[test]
    fn push_header_defaults_received_to_zero_for_old_peers() {
        let raw = r#"{"ok":true,"message":"ok","size":10,"blake3":"","mtime_ms":0}"#;
        let header: P2PFileHeader = serde_json::from_str(raw).expect("header");
        assert_eq!(header.received, 0);
    }

    #[test]
    fn parallel_pull_markers_are_distinct_and_stable() {
        let part = PathBuf::from("root/.guaxi-p2p/tmp/a.part");
        assert_ne!(parallel_marker(&part), part);
        assert_eq!(parallel_marker(&part), parallel_marker(&part));
        assert_ne!(range_marker(&part, 0), range_marker(&part, 1));
        assert!(range_marker(&part, 3)
            .to_string_lossy()
            .ends_with(".r3.done"));
    }

    #[test]
    fn delta_only_applies_to_bounded_file_sizes() {
        assert!(P2P_DELTA_MAX_SIZE > P2P_DELTA_MIN_SIZE);
        assert_eq!(PARALLEL_PULL_RANGES, 4);
        assert!(PARALLEL_PULL_THRESHOLD >= 256 * 1024 * 1024);
    }

    #[test]
    fn push_ranges_cover_the_whole_file_without_gaps() {
        for size in [0u64, 1, 511, 512 * 1024 * 1024, 10 * 1024 * 1024 * 1024] {
            let count = PARALLEL_PUSH_RANGES as u32;
            let mut covered = 0u64;
            for index in 0..count {
                let (start, length) = push_range_bounds(size, count, index);
                assert_eq!(start, covered);
                covered += length;
            }
            assert_eq!(covered, size);
        }
        assert!(PARALLEL_PUSH_THRESHOLD >= PARALLEL_PULL_THRESHOLD);
    }

    #[test]
    fn push_range_markers_are_per_range() {
        let target = P2PSyncTarget {
            key: "resource_repo".to_string(),
            label: "资源库".to_string(),
            root: PathBuf::from("root"),
        };
        assert_ne!(
            push_range_marker(&target, "big.bin", 0),
            push_range_marker(&target, "big.bin", 1)
        );
        assert_ne!(
            push_range_marker(&target, "big.bin", 0),
            push_range_meta_path(&target, "big.bin")
        );
    }

    #[test]
    fn push_request_deserializes_without_range_fields() {
        let raw = r#"{"PushFile":{"target":"resource_repo","rel":"a.bin","size":10,"blake3":"h","mtime_ms":0}}"#;
        let request: P2PWireRequest = serde_json::from_str(raw).expect("request");
        match request {
            P2PWireRequest::PushFile {
                range_count,
                range_length,
                transfer_id,
                ..
            } => {
                assert_eq!(range_count, 0);
                assert_eq!(range_length, 0);
                assert!(transfer_id.is_empty());
            }
            _ => panic!("expected PushFile"),
        }
    }

    #[test]
    fn transfer_id_paths_are_stable_and_isolated() {
        let target = P2PSyncTarget {
            key: "transfers".to_string(),
            label: "临时传输".to_string(),
            root: PathBuf::from("root"),
        };
        let id = "transfer-123";
        assert_eq!(transfer_meta_path(&target, id), transfer_meta_path(&target, id));
        assert_ne!(transfer_meta_path(&target, id), transfer_part_path(&target, id));
        assert_ne!(
            transfer_range_marker(&target, id, 0),
            transfer_range_marker(&target, id, 1)
        );
        assert_ne!(
            transfer_part_path(&target, id),
            transfer_part_path(&target, "other")
        );
    }

    #[test]
    fn transfer_meta_keeps_final_name_for_all_ranges() {
        let meta = PushTransferMeta {
            rel: "docs/report.pdf".to_string(),
            final_rel: "docs/report (1).pdf".to_string(),
            size: 10,
            count: 4,
        };
        let raw = serde_json::to_string(&meta).expect("serialize");
        let decoded: PushTransferMeta = serde_json::from_str(&raw).expect("deserialize");
        assert_eq!(decoded, meta);
    }

    #[test]
    fn pull_request_deserializes_without_new_optional_fields() {
        let raw = r#"{"PullFile":{"target":"resource_repo","rel":"a.bin","offset":0}}"#;
        let request: P2PWireRequest = serde_json::from_str(raw).expect("request");
        match request {
            P2PWireRequest::PullFile {
                length,
                known_hash,
                ..
            } => {
                assert_eq!(length, 0);
                assert!(known_hash.is_empty());
            }
            _ => panic!("expected PullFile"),
        }
    }
}
