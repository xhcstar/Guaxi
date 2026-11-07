use crate::commands::settingview::{get_config_path, get_libraries_root, get_webrtc_signal_token};
use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use chrono::Utc;
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{Mutex, Notify};
use tracing::{error, info, warn};
use uuid::Uuid;

use base64::engine::general_purpose;
use base64::Engine as _;

use actix::{Actor, ActorContext, AsyncContext, Handler, Message, StreamHandler};
use actix_web_actors::ws;
use parking_lot::Mutex as ParkingMutex;

const EVENT_TASK_UPDATE: &str = "filetransfer_task_update";
const EVENT_PEERS_UPDATE: &str = "filetransfer_peers_update";

const DEFAULT_RECEIVER_PORT: u16 = 39190;
const DEFAULT_DISCOVERY_PORT: u16 = 39191;
const DEFAULT_SHARE_PORT: u16 = 39200;

const CHUNK_SIZE: usize = 2 * 1024 * 1024;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiverStatus {
    pub running: bool,
    pub bind_addr: String,
    pub port: u16,
    pub save_dir: String,
    pub base_url: String,
    pub requires_token: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpShareStatus {
    pub running: bool,
    pub bind_addr: String,
    pub port: u16,
    pub root_dir: String,
    pub base_url: String,
    pub read_only: bool,
    pub requires_token: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalIpInfo {
    pub name: String,
    pub address: String,
    pub family: String,
    pub is_loopback: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTransferStatus {
    pub receiver: Option<ReceiverStatus>,
    pub http_share: Option<HttpShareStatus>,
    pub discovery_running: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub peer_id: String,
    pub name: String,
    pub addr: String,
    pub port: u16,
    pub last_seen_ms: i64,
    pub requires_token: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferTask {
    pub id: String,
    pub direction: String,
    pub status: String,
    pub file_name: String,
    pub file_path: String,
    pub peer: String,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub error: Option<String>,
    /// 发送时使用的配对码；重试时复用，不要求用户再填一次。
    #[serde(default)]
    pub access_token: Option<String>,
}

struct TaskControl {
    paused: bool,
    canceled: bool,
    notify: Arc<Notify>,
}

impl TaskControl {
    fn new() -> Self {
        Self {
            paused: false,
            canceled: false,
            notify: Arc::new(Notify::new()),
        }
    }
}

static PEER_ID: once_cell::sync::Lazy<String> =
    once_cell::sync::Lazy::new(|| Uuid::new_v4().to_string());

static TASKS: once_cell::sync::Lazy<Mutex<HashMap<String, TransferTask>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

static TASK_CONTROLS: once_cell::sync::Lazy<Mutex<HashMap<String, Arc<Mutex<TaskControl>>>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

static TASKS_LOADED: once_cell::sync::Lazy<std::sync::atomic::AtomicBool> =
    once_cell::sync::Lazy::new(|| std::sync::atomic::AtomicBool::new(false));
static TASKS_LOAD_LOCK: once_cell::sync::Lazy<Mutex<()>> =
    once_cell::sync::Lazy::new(|| Mutex::new(()));
static LAST_PERSIST_MS: once_cell::sync::Lazy<std::sync::atomic::AtomicI64> =
    once_cell::sync::Lazy::new(|| std::sync::atomic::AtomicI64::new(0));

static PEERS: once_cell::sync::Lazy<Mutex<HashMap<String, PeerInfo>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

struct ReceiverRuntime {
    bind_addr: String,
    port: u16,
    save_dir: PathBuf,
    handle: actix_web::dev::ServerHandle,
}

static RECEIVER: once_cell::sync::Lazy<Mutex<Option<ReceiverRuntime>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

static RECEIVER_TOKEN: once_cell::sync::Lazy<ParkingMutex<String>> =
    once_cell::sync::Lazy::new(|| ParkingMutex::new(String::new()));

static SHARE_TOKEN: once_cell::sync::Lazy<ParkingMutex<String>> =
    once_cell::sync::Lazy::new(|| ParkingMutex::new(String::new()));
static SHARE_READ_ONLY: once_cell::sync::Lazy<std::sync::atomic::AtomicBool> =
    once_cell::sync::Lazy::new(|| std::sync::atomic::AtomicBool::new(true));

fn share_allowed(req: &HttpRequest) -> bool {
    let expected = SHARE_TOKEN.lock().clone();
    if expected.is_empty() {
        return true;
    }
    let header = req
        .headers()
        .get("x-guaxi-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if header == expected {
        return true;
    }
    req.query_string()
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == "token")
        .map(|(_, value)| {
            urlencoding::decode(value)
                .map(|decoded| decoded == expected)
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn receiver_token() -> String {
    RECEIVER_TOKEN.lock().clone()
}

fn token_allowed(req: &HttpRequest) -> bool {
    let expected = receiver_token();
    if expected.is_empty() {
        return true;
    }
    req.headers()
        .get("x-guaxi-token")
        .and_then(|value| value.to_str().ok())
        .map(|value| value == expected)
        .unwrap_or(false)
}

fn unauthorized() -> HttpResponse {
    HttpResponse::Unauthorized().json(serde_json::json!({"error":"配对码不正确"}))
}

fn query_param(req: &HttpRequest, key: &str) -> String {
    req.query_string()
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(name, _)| *name == key)
        .map(|(_, value)| {
            urlencoding::decode(value)
                .map(|decoded| decoded.into_owned())
                .unwrap_or_else(|_| value.to_string())
        })
        .unwrap_or_default()
}

fn signal_token_allowed(req: &HttpRequest) -> bool {
    let expected = get_webrtc_signal_token();
    if expected.trim().is_empty() {
        return true;
    }
    query_param(req, "token") == expected
}

struct ShareRuntime {
    bind_addr: String,
    port: u16,
    root_dir: PathBuf,
    handle: actix_web::dev::ServerHandle,
}

static SHARE: once_cell::sync::Lazy<Mutex<Option<ShareRuntime>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

struct DiscoveryRuntime {
    stop: tokio::sync::oneshot::Sender<()>,
}

static DISCOVERY: once_cell::sync::Lazy<Mutex<Option<DiscoveryRuntime>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStatResp {
    pub file_name: String,
    pub total_bytes: u64,
    pub mtime_ms: i64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebRtcRecvInitResp {
    pub upload_id: String,
    pub received_bytes: u64,
    pub save_dir: String,
    pub final_name: String,
    pub task_id: String,
}

#[derive(Clone)]
struct WebRtcUploadCtx {
    save_dir: PathBuf,
    file_name: String,
    file_size: u64,
    final_name: String,
    task_id: String,
}

static WEBRTC_UPLOADS: once_cell::sync::Lazy<Mutex<HashMap<String, WebRtcUploadCtx>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

static WEBRTC_LOCKS: once_cell::sync::Lazy<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

fn default_incoming_dir() -> PathBuf {
    PathBuf::from(get_libraries_root()).join("incoming_transfers")
}

fn normalize_base_url(peer_url: &str) -> String {
    let s = peer_url.trim().trim_end_matches('/').to_string();
    if s.starts_with("http://") || s.starts_with("https://") {
        return s;
    }
    format!("http://{}", s)
}

fn safe_join(root: &Path, tail: &str) -> Result<PathBuf, String> {
    let t = tail.trim_start_matches('/').trim();
    let p = Path::new(t);

    if p.is_absolute() {
        return Err("路径不允许为绝对路径".to_string());
    }

    let mut out = root.to_path_buf();
    for c in p.components() {
        match c {
            Component::Normal(x) => out.push(x),
            Component::CurDir => {}
            _ => return Err("路径包含非法片段（.. 或前缀）".to_string()),
        }
    }
    Ok(out)
}

fn sha256_short(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let out = h.finalize();
    hex::encode(out)
}

/// 只接受普通路径段，拒绝 `..`、绝对路径和盘符，防止接收端写到保存目录之外。
fn sanitize_relative_path(raw: &str) -> Result<String, String> {
    let mut parts = Vec::new();
    for component in Path::new(raw.trim()).components() {
        match component {
            Component::Normal(value) => {
                let name = value.to_string_lossy().trim().to_string();
                if !name.is_empty() {
                    parts.push(name);
                }
            }
            Component::CurDir => {}
            _ => return Err("文件路径包含非法片段".to_string()),
        }
    }
    if parts.is_empty() {
        return Err("文件路径不能为空".to_string());
    }
    Ok(parts.join("/"))
}

/// 在目标目录内生成不冲突的相对路径；目录不存在时直接使用原路径。
fn unique_relative_path(root: &Path, raw: &str) -> Result<String, String> {
    let rel = sanitize_relative_path(raw)?;
    if !root.join(&rel).exists() {
        return Ok(rel);
    }
    let path = Path::new(&rel);
    let parent = path.parent().map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
    let name = path.file_name().and_then(|v| v.to_str()).unwrap_or("file");
    let unique = unique_name_in_dir(&root.join(&parent), name);
    if parent.is_empty() {
        Ok(unique)
    } else {
        Ok(format!("{}/{}", parent.trim_matches('/'), unique))
    }
}

fn unique_name_in_dir(dir: &Path, name: &str) -> String {
    let base = name.trim();
    if base.is_empty() {
        return format!("file-{}", Uuid::new_v4());
    }
    let candidate = base.to_string();
    if !dir.join(&candidate).exists() {
        return candidate;
    }
    let stem = Path::new(base)
        .file_stem()
        .and_then(|x| x.to_str())
        .unwrap_or(base);
    let ext = Path::new(base)
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("");
    for i in 1..10_000 {
        let v = if ext.is_empty() {
            format!("{} ({})", stem, i)
        } else {
            format!("{} ({}){}.{}", stem, i, "", ext)
        };
        if !dir.join(&v).exists() {
            return v;
        }
    }
    format!("{}-{}", stem, Uuid::new_v4())
}

#[derive(Clone)]
struct UploadStore {
    save_dir: PathBuf,
    locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

#[derive(Serialize, Deserialize)]
struct UploadInitReq {
    upload_id: String,
    file_name: String,
    file_size: u64,
    mtime_ms: i64,
    /// 文件夹发送时携带相对路径（含文件名）；单文件为空。
    #[serde(default)]
    relative_path: String,
}

#[derive(Serialize, Deserialize)]
struct UploadInitResp {
    upload_id: String,
    received_bytes: u64,
}

#[derive(Deserialize)]
struct UploadChunkQuery {
    upload_id: String,
    offset: u64,
}

#[derive(Serialize, Deserialize)]
struct UploadCompleteReq {
    upload_id: String,
}

fn upload_meta_path(save_dir: &Path, upload_id: &str) -> PathBuf {
    save_dir
        .join(".guaxi_transfer_tmp")
        .join(format!("{}.meta.json", upload_id))
}

fn upload_partial_path(save_dir: &Path, upload_id: &str) -> PathBuf {
    save_dir
        .join(".guaxi_transfer_tmp")
        .join(format!("{}.partial", upload_id))
}

#[derive(Serialize, Deserialize)]
struct UploadMeta {
    upload_id: String,
    file_name: String,
    file_size: u64,
    mtime_ms: i64,
    final_name: String,
    #[serde(default)]
    task_id: String,
}

/// upload_id -> 接收任务 id，用于分片阶段更新进度。
static UPLOAD_TASK_MAP: once_cell::sync::Lazy<Mutex<HashMap<String, String>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

async fn get_lock(store: &UploadStore, upload_id: &str) -> Arc<Mutex<()>> {
    let mut locks = store.locks.lock().await;
    if let Some(l) = locks.get(upload_id) {
        return l.clone();
    }
    let l = Arc::new(Mutex::new(()));
    locks.insert(upload_id.to_string(), l.clone());
    l
}

async fn ft_ping() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "peer_id": *PEER_ID,
        "ts": now_ms()
    }))
}

#[derive(Default)]
struct SignalState {
    rooms: HashMap<String, HashMap<String, actix::Recipient<SignalServerPush>>>,
}

static SIGNAL_STATE: once_cell::sync::Lazy<Arc<ParkingMutex<SignalState>>> =
    once_cell::sync::Lazy::new(|| Arc::new(ParkingMutex::new(SignalState::default())));

#[derive(Message, Clone)]
#[rtype(result = "()")] 
struct SignalServerPush(String);

struct SignalWsSession {
    id: String,
    room: Option<String>,
    state: Arc<ParkingMutex<SignalState>>,
}

impl SignalWsSession {
    fn new(state: Arc<ParkingMutex<SignalState>>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            room: None,
            state,
        }
    }

    fn send_json(ctx: &mut ws::WebsocketContext<Self>, v: serde_json::Value) {
        if let Ok(s) = serde_json::to_string(&v) {
            ctx.text(s);
        }
    }

    fn leave_room_inner(&mut self) {
        let room = match self.room.take() {
            Some(r) => r,
            None => return,
        };

        let mut st = self.state.lock();
        if let Some(m) = st.rooms.get_mut(&room) {
            m.remove(&self.id);
            let peers: Vec<(String, actix::Recipient<SignalServerPush>)> =
                m.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

            for (_pid, recp) in peers {
                let _ = recp.do_send(SignalServerPush(
                    serde_json::json!({"t":"peer_left","clientId":self.id}).to_string(),
                ));
            }

            if m.is_empty() {
                st.rooms.remove(&room);
            }
        }

        info!("[FileTransfer][Signal] leave room={} client={}", room, self.id);
    }
}

impl Actor for SignalWsSession {
    type Context = ws::WebsocketContext<Self>;

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        self.leave_room_inner();
    }
}

impl Handler<SignalServerPush> for SignalWsSession {
    type Result = ();

    fn handle(&mut self, msg: SignalServerPush, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for SignalWsSession {
    fn handle(&mut self, item: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        let msg = match item {
            Ok(v) => v,
            Err(e) => {
                warn!("[FileTransfer][Signal] ws protocol error: {}", e);
                ctx.stop();
                return;
            }
        };

        match msg {
            ws::Message::Text(text) => {
                let v: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("[FileTransfer][Signal] invalid json: {}", e);
                        return;
                    }
                };

                let t = v.get("t").and_then(|x| x.as_str()).unwrap_or("");

                if t == "join" {
                    let room = v
                        .get("room")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    if room.is_empty() {
                        Self::send_json(ctx, serde_json::json!({"t":"error","error":"room 不能为空"}));
                        return;
                    }

                    self.leave_room_inner();

                    let (peers, others): (Vec<String>, Vec<actix::Recipient<SignalServerPush>>) = {
                        let mut st = self.state.lock();
                        let m = st.rooms.entry(room.clone()).or_default();
                        let peers = m.keys().cloned().collect::<Vec<_>>();
                        let others = m.values().cloned().collect::<Vec<_>>();
                        m.insert(self.id.clone(), ctx.address().recipient::<SignalServerPush>());
                        (peers, others)
                    };

                    self.room = Some(room.clone());

                    Self::send_json(
                        ctx,
                        serde_json::json!({"t":"joined","room":room,"clientId":self.id,"peers":peers}),
                    );

                    for recp in others {
                        let _ = recp.do_send(SignalServerPush(
                            serde_json::json!({"t":"peer_joined","clientId":self.id}).to_string(),
                        ));
                    }

                    info!("[FileTransfer][Signal] join room={} client={}", room, self.id);
                    return;
                }

                if t == "signal" {
                    let room = v
                        .get("room")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    if room.is_empty() {
                        return;
                    }

                    let to = v.get("to").and_then(|x| x.as_str()).map(|s| s.to_string());
                    let data = v.get("data").cloned().unwrap_or(serde_json::Value::Null);

                    let targets: Vec<actix::Recipient<SignalServerPush>> = {
                        let st = self.state.lock();
                        let Some(m) = st.rooms.get(&room) else {
                            return;
                        };

                        if let Some(to_id) = &to {
                            m.get(to_id).cloned().into_iter().collect()
                        } else {
                            m.iter()
                                .filter(|(pid, _)| *pid != &self.id)
                                .map(|(_, r)| r.clone())
                                .collect()
                        }
                    };

                    let payload = serde_json::json!({"t":"signal","from":self.id,"data":data}).to_string();
                    for recp in targets {
                        let _ = recp.do_send(SignalServerPush(payload.clone()));
                    }

                    return;
                }

                if t == "leave" {
                    self.leave_room_inner();
                    Self::send_json(ctx, serde_json::json!({"t":"left","clientId":self.id}));
                    return;
                }
            }
            ws::Message::Ping(v) => ctx.pong(&v),
            ws::Message::Pong(_) => {}
            ws::Message::Close(_) => ctx.stop(),
            _ => {}
        }
    }
}

async fn ft_signal_ws(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, actix_web::Error> {
    if !signal_token_allowed(&req) {
        return Ok(HttpResponse::Unauthorized().body("信令配对码不正确"));
    }
    ws::start(SignalWsSession::new(SIGNAL_STATE.clone()), &req, stream)
}

async fn ft_upload_init(
    store: web::Data<UploadStore>,
    app: web::Data<AppHandle>,
    http: HttpRequest,
    req: web::Json<UploadInitReq>,
) -> impl Responder {
    if !token_allowed(&http) {
        return unauthorized();
    }
    let upload_id = req.upload_id.trim().to_string();
    if upload_id.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({"error":"upload_id 不能为空"}));
    }

    let save_dir = store.save_dir.clone();
    if let Err(e) = tokio::fs::create_dir_all(save_dir.join(".guaxi_transfer_tmp")).await {
        error!("[FileTransfer] create tmp dir failed: {}", e);
        return HttpResponse::InternalServerError()
            .json(serde_json::json!({"error":"创建临时目录失败"}));
    }
    if let Err(e) = tokio::fs::create_dir_all(&save_dir).await {
        error!("[FileTransfer] create save dir failed: {}", e);
        return HttpResponse::InternalServerError()
            .json(serde_json::json!({"error":"创建保存目录失败"}));
    }

    let meta_path = upload_meta_path(&save_dir, &upload_id);
    let partial_path = upload_partial_path(&save_dir, &upload_id);

    let requested = if req.relative_path.trim().is_empty() {
        req.file_name.clone()
    } else {
        req.relative_path.clone()
    };
    let mut final_name = match unique_relative_path(&save_dir, &requested) {
        Ok(value) => value,
        Err(error) => {
            return HttpResponse::BadRequest().json(serde_json::json!({"error": error}));
        }
    };
    let mut received_bytes = 0u64;
    let mut task_id = String::new();

    if meta_path.exists() {
        if let Ok(raw) = tokio::fs::read_to_string(&meta_path).await {
            if let Ok(meta) = serde_json::from_str::<UploadMeta>(&raw) {
                if meta.file_size == req.file_size && meta.mtime_ms == req.mtime_ms {
                    final_name = meta.final_name;
                    task_id = meta.task_id;
                    if let Ok(m) = tokio::fs::metadata(&partial_path).await {
                        received_bytes = m.len();
                    }
                } else {
                    warn!(
                        "[FileTransfer] meta mismatch, reset upload_id={} old_size={} new_size={} old_mtime={} new_mtime={}",
                        upload_id, meta.file_size, req.file_size, meta.mtime_ms, req.mtime_ms
                    );
                }
            }
        }
    }

    if task_id.is_empty() {
        task_id = Uuid::new_v4().to_string();
    }
    let meta = UploadMeta {
        upload_id: upload_id.clone(),
        file_name: req.file_name.clone(),
        file_size: req.file_size,
        mtime_ms: req.mtime_ms,
        final_name: final_name.clone(),
        task_id: task_id.clone(),
    };
    if let Ok(s) = serde_json::to_string(&meta) {
        if let Err(e) = tokio::fs::write(&meta_path, s).await {
            error!("[FileTransfer] write meta failed: {}", e);
        }
    }

    let peer = http
        .connection_info()
        .peer_addr()
        .unwrap_or("")
        .to_string();
    let task = {
        let tasks = TASKS.lock().await;
        let mut task = tasks.get(&task_id).cloned().unwrap_or_else(|| TransferTask {
            id: task_id.clone(),
            direction: "recv".to_string(),
            status: "running".to_string(),
            file_name: final_name.clone(),
            file_path: save_dir.to_string_lossy().to_string(),
            peer: peer.clone(),
            total_bytes: req.file_size,
            transferred_bytes: 0,
            created_at_ms: now_ms(),
            updated_at_ms: now_ms(),
            error: None,
            access_token: None,
        });
        task.status = "running".to_string();
        task.transferred_bytes = received_bytes;
        task.updated_at_ms = now_ms();
        task
    };
    update_task(app.get_ref(), task).await;
    UPLOAD_TASK_MAP
        .lock()
        .await
        .insert(upload_id.clone(), task_id);

    HttpResponse::Ok().json(UploadInitResp {
        upload_id,
        received_bytes,
    })
}

async fn ft_upload_chunk(
    store: web::Data<UploadStore>,
    app: web::Data<AppHandle>,
    q: web::Query<UploadChunkQuery>,
    http: HttpRequest,
    body: web::Bytes,
) -> impl Responder {
    if !token_allowed(&http) {
        return unauthorized();
    }
    let upload_id = q.upload_id.trim().to_string();
    if upload_id.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({"error":"upload_id 不能为空"}));
    }
    let offset = q.offset;

    let lock = get_lock(&store, &upload_id).await;
    let _g = lock.lock().await;

    let partial_path = upload_partial_path(&store.save_dir, &upload_id);

    let mut f = match tokio::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&partial_path)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            error!("[FileTransfer] open partial failed: {}", e);
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({"error":"打开临时文件失败"}));
        }
    };

    if let Err(e) = f.seek(std::io::SeekFrom::Start(offset)).await {
        error!("[FileTransfer] seek failed: {}", e);
        return HttpResponse::InternalServerError()
            .json(serde_json::json!({"error":"写入定位失败"}));
    }
    if let Err(e) = f.write_all(&body).await {
        error!("[FileTransfer] write failed: {}", e);
        return HttpResponse::InternalServerError().json(serde_json::json!({"error":"写入失败"}));
    }

    let task_id = UPLOAD_TASK_MAP
        .lock()
        .await
        .get(&upload_id)
        .cloned()
        .unwrap_or_default();
    if !task_id.is_empty() {
        let current = TASKS.lock().await.get(&task_id).cloned();
        if let Some(mut task) = current {
            task.transferred_bytes = offset.saturating_add(body.len() as u64);
            task.status = "running".to_string();
            task.updated_at_ms = now_ms();
            update_task(app.get_ref(), task).await;
        }
    }

    HttpResponse::Ok().json(serde_json::json!({"ok":true, "bytes": body.len()}))
}

async fn ft_upload_complete(
    store: web::Data<UploadStore>,
    app: web::Data<AppHandle>,
    http: HttpRequest,
    req: web::Json<UploadCompleteReq>,
) -> impl Responder {
    if !token_allowed(&http) {
        return unauthorized();
    }
    let upload_id = req.upload_id.trim().to_string();
    if upload_id.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({"error":"upload_id 不能为空"}));
    }

    let meta_path = upload_meta_path(&store.save_dir, &upload_id);
    let partial_path = upload_partial_path(&store.save_dir, &upload_id);

    let raw = match tokio::fs::read_to_string(&meta_path).await {
        Ok(v) => v,
        Err(_) => {
            return HttpResponse::NotFound().json(serde_json::json!({"error":"meta 不存在"}));
        }
    };
    let meta: UploadMeta = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({"error":"meta 解析失败"}));
        }
    };

    let final_path = match safe_join(&store.save_dir, &meta.final_name) {
        Ok(value) => value,
        Err(error) => {
            return HttpResponse::BadRequest().json(serde_json::json!({"error": error}));
        }
    };
    if let Some(parent) = final_path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            error!("[FileTransfer] create final dir failed: {}", e);
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({"error":"创建目标目录失败"}));
        }
    }
    if let Err(e) = tokio::fs::rename(&partial_path, &final_path).await {
        error!("[FileTransfer] finalize rename failed: {}", e);
        return HttpResponse::InternalServerError()
            .json(serde_json::json!({"error":"完成写入失败"}));
    }

    let _ = tokio::fs::remove_file(&meta_path).await;
    let task_id = {
        let mapped = UPLOAD_TASK_MAP.lock().await.remove(&upload_id);
        mapped.filter(|value| !value.is_empty()).unwrap_or_else(|| meta.task_id.clone())
    };
    if !task_id.is_empty() {
        let task = {
            let tasks = TASKS.lock().await;
            let mut task = tasks.get(&task_id).cloned().unwrap_or_else(|| TransferTask {
                id: task_id.clone(),
                direction: "recv".to_string(),
                status: "done".to_string(),
                file_name: meta.final_name.clone(),
                file_path: final_path.to_string_lossy().to_string(),
                peer: String::new(),
                total_bytes: meta.file_size,
                transferred_bytes: meta.file_size,
                created_at_ms: now_ms(),
                updated_at_ms: now_ms(),
                error: None,
                access_token: None,
            });
            task.status = "done".to_string();
            task.transferred_bytes = meta.file_size;
            task.file_name = meta.final_name.clone();
            task.file_path = final_path.to_string_lossy().to_string();
            task.error = None;
            task.updated_at_ms = now_ms();
            task
        };
        update_task(app.get_ref(), task).await;
    }
    HttpResponse::Ok().json(serde_json::json!({"ok":true, "saved": final_path.to_string_lossy()}))
}

fn guess_mime(path: &Path) -> String {
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_string()
}

async fn share_get(req: HttpRequest, root: web::Data<PathBuf>) -> HttpResponse {
    // 下载保持公开：分享本质是“只读目录”。配对码只保护上传，避免把浏览器里的
    // 目录链接全部变成需要手动补 token 的地址。
    let tail = req.match_info().query("tail");
    let decoded = match urlencoding::decode(tail) {
        Ok(v) => v.to_string(),
        Err(_) => tail.to_string(),
    };

    let full = match safe_join(root.get_ref(), &decoded) {
        Ok(v) => v,
        Err(e) => return HttpResponse::BadRequest().body(e),
    };

    let meta = match tokio::fs::metadata(&full).await {
        Ok(v) => v,
        Err(_) => return HttpResponse::NotFound().body("not found"),
    };

    if meta.is_dir() {
        let full2 = full.clone();
        let decoded2 = decoded.clone();
        let listing = tokio::task::spawn_blocking(move || {
            fn html_escape(s: &str) -> String {
                let mut out = String::with_capacity(s.len());
                for ch in s.chars() {
                    match ch {
                        '&' => out.push_str("&amp;"),
                        '<' => out.push_str("&lt;"),
                        '>' => out.push_str("&gt;"),
                        '"' => out.push_str("&quot;"),
                        '\'' => out.push_str("&#39;"),
                        _ => out.push(ch),
                    }
                }
                out
            }

            fn encode_path_for_url(path: &str) -> String {
                let trimmed = path.trim_matches('/');
                if trimmed.is_empty() {
                    return String::new();
                }
                trimmed
                    .split('/')
                    .filter(|x| !x.is_empty())
                    .map(|seg| urlencoding::encode(seg).into_owned())
                    .collect::<Vec<_>>()
                    .join("/")
            }

            let current_raw = decoded2.trim_end_matches('/');

            let parent_raw = current_raw.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
            let parent_enc = encode_path_for_url(parent_raw);

            fn format_bytes(bytes: u64) -> String {
                const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
                let mut v = bytes as f64;
                let mut idx: usize = 0;
                while v >= 1024.0 && idx + 1 < UNITS.len() {
                    v /= 1024.0;
                    idx += 1;
                }

                if idx == 0 {
                    format!("{} {}", bytes, UNITS[idx])
                } else if v >= 10.0 {
                    format!("{:.0} {}", v, UNITS[idx])
                } else {
                    format!("{:.1} {}", v, UNITS[idx])
                }
            }

            fn format_mtime(meta: &std::fs::Metadata) -> String {
                match meta.modified() {
                    Ok(st) => {
                        let dt: chrono::DateTime<chrono::Local> = st.into();
                        dt.format("%Y-%m-%d %H:%M:%S").to_string()
                    }
                    Err(_) => "-".to_string(),
                }
            }

            struct Item {
                name: String,
                is_dir: bool,
                size: u64,
                modified: String,
            }

            let mut items: Vec<Item> = Vec::new();
            if let Ok(rd) = std::fs::read_dir(&full2) {
                for e in rd.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    let meta = match e.metadata() {
                        Ok(m) => m,
                        Err(_) => continue,
                    };
                    items.push(Item {
                        name,
                        is_dir: meta.is_dir(),
                        size: meta.len(),
                        modified: format_mtime(&meta),
                    });
                }
            }

            items.sort_by(|a, b| {
                match (a.is_dir, b.is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                }
            });

            let mut html = String::new();
            html.push_str("<!doctype html><html><head><meta charset=\"utf-8\"/>");
            html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"/>");
            html.push_str("<title>Guaxi HTTP Share</title>");
            html.push_str(
                "<style>\
                :root{\
                  color-scheme:light;\
                  --bg:#f7f8fa;\
                  --fg:#0f172a;\
                  --muted:#475569;\
                  --link:#0369a1;\
                  --card:#ffffff;\
                  --border:#e2e8f0;\
                  --btn-bg:#f1f5f9;\
                  --btn-border:#cbd5e1;\
                  --btn-hover:#e2e8f0;\
                  --btn-primary:#2563eb;\
                  --btn-primary-hover:#1d4ed8;\
                  --input-bg:#ffffff;\
                  --table-border:#e2e8f0;\
                  --toast-bg:#0f172a;\
                  --toast-border:#334155;\
                  --toast-fg:#f8fafc;\
                }\
                @media (prefers-color-scheme: dark){\
                  :root{\
                    color-scheme:dark;\
                    --bg:#0b0f17;\
                    --fg:#e6edf3;\
                    --muted:#9fb0c0;\
                    --link:#7dd3fc;\
                    --card:#121826;\
                    --border:#1f2a44;\
                    --btn-bg:#16233a;\
                    --btn-border:#2a3a5e;\
                    --btn-hover:#1a2a46;\
                    --btn-primary:#1d4ed8;\
                    --btn-primary-hover:#1e40af;\
                    --input-bg:#0f172a;\
                    --table-border:#1f2a44;\
                    --toast-bg:#111827;\
                    --toast-border:#24314f;\
                    --toast-fg:#e6edf3;\
                  }\
                }\
                :root[data-theme='light']{color-scheme:light;}\
                :root[data-theme='dark']{color-scheme:dark;}\
                :root[data-theme='light']{\
                  --bg:#f7f8fa;--fg:#0f172a;--muted:#475569;--link:#0369a1;--card:#ffffff;--border:#e2e8f0;\
                  --btn-bg:#f1f5f9;--btn-border:#cbd5e1;--btn-hover:#e2e8f0;--btn-primary:#2563eb;--btn-primary-hover:#1d4ed8;\
                  --input-bg:#ffffff;--table-border:#e2e8f0;--toast-bg:#0f172a;--toast-border:#334155;--toast-fg:#f8fafc;\
                }\
                :root[data-theme='dark']{\
                  --bg:#0b0f17;--fg:#e6edf3;--muted:#9fb0c0;--link:#7dd3fc;--card:#121826;--border:#1f2a44;\
                  --btn-bg:#16233a;--btn-border:#2a3a5e;--btn-hover:#1a2a46;--btn-primary:#1d4ed8;--btn-primary-hover:#1e40af;\
                  --input-bg:#0f172a;--table-border:#1f2a44;--toast-bg:#111827;--toast-border:#24314f;--toast-fg:#e6edf3;\
                }\
                body{font-family:system-ui,-apple-system,Segoe UI,Roboto,Helvetica,Arial; margin:0; background:var(--bg); color:var(--fg);}\
                a{color:var(--link); text-decoration:none;} a:hover{text-decoration:underline;}\
                .wrap{max-width:980px; margin:0 auto; padding:18px 16px 40px;}\
                .card{background:var(--card); border:1px solid var(--border); border-radius:14px; padding:14px 14px; margin-top:12px;}\
                .title{font-size:18px; font-weight:700;}\
                .sub{margin-top:6px; color:var(--muted); font-size:13px; line-height:1.5;}\
                .row{display:flex; gap:10px; flex-wrap:wrap; align-items:center;}\
                .btn{appearance:none; border:1px solid var(--btn-border); background:var(--btn-bg); color:var(--fg); border-radius:10px; padding:8px 12px; cursor:pointer;}\
                .btn:hover{background:var(--btn-hover);}\
                .btn.primary{border-color:var(--btn-primary); background:var(--btn-primary); color:#fff;}\
                .btn.primary:hover{background:var(--btn-primary-hover);}\
                .input{border:1px solid var(--btn-border); background:var(--input-bg); color:var(--fg); border-radius:10px; padding:8px 10px; min-width:240px;}\
                table{width:100%; border-collapse:collapse;}\
                th,td{padding:10px 8px; border-bottom:1px solid var(--table-border); font-size:13px;}\
                th{color:var(--muted); font-weight:600; text-align:left;}\
                .mono{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;}\
                .tag{display:inline-block; font-size:12px; padding:2px 8px; border:1px solid var(--btn-border); border-radius:999px; color:var(--muted);}\
                .right{display:flex; justify-content:flex-end; gap:8px; flex-wrap:wrap;}\
                .hint{margin-top:8px; color:var(--muted); font-size:12px;}\
                .toast{position:fixed; left:50%; bottom:16px; transform:translateX(-50%); background:var(--toast-bg); border:1px solid var(--toast-border); color:var(--toast-fg); padding:10px 12px; border-radius:999px; opacity:0; transition:opacity .18s;}\
                .toast.show{opacity:1;}\
                </style>",
            );
            html.push_str("</head><body><div class=\"wrap\">");

            html.push_str("<div class=\"card\"><div class=\"title\">Guaxi 目录分享</div>");
            html.push_str("<div class=\"sub\">当前路径：<span class=\"mono\">/");
            html.push_str(&html_escape(current_raw));
            html.push_str("</span></div>");
            html.push_str("<div class=\"row\" style=\"margin-top:10px\"> ");
            if !current_raw.is_empty() {
                html.push_str(&format!(
                    "<a class=\"btn\" href=\"/{}\">返回上级</a>",
                    parent_enc
                ));
            } else {
                html.push_str("<span class=\"tag\">根目录</span>");
            }
            html.push_str("<a class=\"btn\" href=\"/\">回到根目录</a>");
            html.push_str("<button id=\"themeToggle\" class=\"btn\" type=\"button\">切换主题</button>");
            html.push_str("</div></div>");

            html.push_str("<div class=\"card\">\
                <div class=\"row\">\
                  <span class=\"tag\">上传</span>\
                  <input id=\"file\" type=\"file\"/>\
                  <input id=\"target\" class=\"input mono\" placeholder=\"保存为（相对当前目录），例如：foo.txt 或 sub/bar.zip\"/>\
                  <button id=\"upload\" class=\"btn primary\">上传</button>\
                </div>\
                <div class=\"hint\">说明：上传会通过 HTTP PUT 写入到当前目录；若填写 sub/xxx 会自动创建子目录。</div>\
              </div>");

            html.push_str("<div class=\"card\">");
            html.push_str("<div class=\"row\" style=\"justify-content:space-between\">\
                <div><span class=\"tag\">文件列表</span></div>\
                <div class=\"hint\">点击文件名下载；点击目录名进入</div>\
              </div>");

            html.push_str("<table><thead><tr><th>名称</th><th style=\"width:120px\">大小</th><th style=\"width:170px\">修改时间</th><th style=\"width:220px\">操作</th></tr></thead><tbody>");

            for it in items {
                let name_esc = html_escape(&it.name);
                let mut child_raw = current_raw.to_string();
                if !child_raw.is_empty() {
                    child_raw.push('/');
                }
                child_raw.push_str(&it.name);
                if it.is_dir {
                    child_raw.push('/');
                }

                let child_enc = encode_path_for_url(&child_raw);

                let size_text = if it.is_dir {
                    "-".to_string()
                } else {
                    format_bytes(it.size)
                };
                let modified_esc = html_escape(&it.modified);

                html.push_str("<tr>");
                html.push_str(&format!(
                    "<td><a class=\"mono\" href=\"/{}\">{}</a></td>",
                    child_enc, name_esc
                ));
                html.push_str(&format!("<td class=\"mono\">{}</td>", size_text));
                html.push_str(&format!("<td class=\"mono\">{}</td>", modified_esc));

                if it.is_dir {
                    html.push_str("<td><div class=\"right\"><span class=\"tag\">目录</span></div></td>");
                } else {
                    html.push_str(&format!(
                        "<td><div class=\"right\">\
                          <a class=\"btn\" href=\"/{}\" download>下载</a>\
                          <button class=\"btn\" data-copy=\"/{}\">复制链接</button>\
                        </div></td>",
                        child_enc, child_enc
                    ));
                }

                html.push_str("</tr>");
            }

            html.push_str("</tbody></table></div>");

            html.push_str("<div id=\"toast\" class=\"toast\"></div>");
            html.push_str("<script>\
              const toastEl = document.getElementById('toast');\
              const toast = (s) => {\
                toastEl.textContent = s;\
                toastEl.classList.add('show');\
                setTimeout(()=>toastEl.classList.remove('show'), 1200);\
              };\
              const THEME_KEY = 'guaxi_http_share_theme';\
              const systemTheme = () => (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) ? 'dark' : 'light';\
              const themeFromQuery = () => {\
                try {\
                  const u = new URL(window.location.href);\
                  const t = String(u.searchParams.get('theme') || '').toLowerCase();\
                  if (t === 'dark' || t === 'light') return t;\
                } catch (e) {\
                  console.warn('themeFromQuery failed', e);\
                }\
                return '';\
              };\
              const applyTheme = (t) => {\
                const root = document.documentElement;\
                if (t === 'dark' || t === 'light') root.dataset.theme = t; else delete root.dataset.theme;\
                const btn = document.getElementById('themeToggle');\
                if (btn) {\
                  const cur = root.dataset.theme || systemTheme();\
                  btn.textContent = cur === 'dark' ? '切换到亮色' : '切换到暗色';\
                }\
              };\
              let theme = '';\
              try {\
                theme = themeFromQuery() || localStorage.getItem(THEME_KEY) || '';\
              } catch (e) {\
                console.warn('read theme failed', e);\
              }\
              applyTheme(theme);\
              const themeBtn = document.getElementById('themeToggle');\
              if (themeBtn) {\
                themeBtn.addEventListener('click', () => {\
                  const root = document.documentElement;\
                  const cur = root.dataset.theme || systemTheme();\
                  const next = cur === 'dark' ? 'light' : 'dark';\
                  root.dataset.theme = next;\
                  try { localStorage.setItem(THEME_KEY, next); } catch (e) { console.warn('save theme failed', e); }\
                  applyTheme(next);\
                });\
              }\
              if (window.matchMedia) {\
                const mq = window.matchMedia('(prefers-color-scheme: dark)');\
                const onChange = () => {\
                  const root = document.documentElement;\
                  if (!root.dataset.theme) applyTheme('');\
                };\
                if (mq.addEventListener) mq.addEventListener('change', onChange);\
                else if (mq.addListener) mq.addListener(onChange);\
              }\
              const fileEl = document.getElementById('file');\
              const targetEl = document.getElementById('target');\
              const uploadBtn = document.getElementById('upload');\
              fileEl.addEventListener('change', () => {\
                const f = fileEl.files && fileEl.files[0];\
                if (f && !targetEl.value) targetEl.value = f.name;\
              });\
              const baseDir = () => {\
                let p = window.location.pathname || '/';\
                if (!p.endsWith('/')) p += '/';\
                return p;\
              };\
              uploadBtn.addEventListener('click', async () => {\
                const f = fileEl.files && fileEl.files[0];\
                const t = (targetEl.value || '').trim();\
                if (!f) return toast('请先选择文件');\
                if (!t) return toast('请填写保存路径');\
                uploadBtn.disabled = true;\
                try {\
                  const url = baseDir() + encodeURI(t).replace(/#/g, '%23');\
                  const buf = await f.arrayBuffer();\
                  const r = await fetch(url, { method: 'PUT', body: buf });\
                  if (!r.ok) {\
                    const txt = await r.text().catch(()=> '');\
                    throw new Error('上传失败: ' + r.status + ' ' + txt);\
                  }\
                  toast('上传成功');\
                  setTimeout(()=>window.location.reload(), 250);\
                } catch (e) {\
                  console.error(e);\
                  toast(String(e && e.message ? e.message : e));\
                } finally {\
                  uploadBtn.disabled = false;\
                }\
              });\
              const fallbackCopy = (text) => {\
                try {\
                  const ta = document.createElement('textarea');\
                  ta.value = text;\
                  ta.setAttribute('readonly', '');\
                  ta.style.position = 'fixed';\
                  ta.style.top = '0';\
                  ta.style.left = '0';\
                  ta.style.opacity = '0';\
                  document.body.appendChild(ta);\
                  ta.focus();\
                  ta.select();\
                  const ok = document.execCommand && document.execCommand('copy');\
                  document.body.removeChild(ta);\
                  return !!ok;\
                } catch (e) {\
                  console.warn('fallbackCopy failed', e);\
                  return false;\
                }\
              };\
              const copyText = async (text) => {\
                if (navigator.clipboard && window.isSecureContext) {\
                  await navigator.clipboard.writeText(text);\
                  return true;\
                }\
                return fallbackCopy(text);\
              };\
              document.querySelectorAll('button[data-copy]').forEach((btn) => {\
                btn.addEventListener('click', async () => {\
                  const p = btn.getAttribute('data-copy') || '';\
                  const full = new URL(p, window.location.href).toString();\
                  try {\
                    const ok = await copyText(full);\
                    if (ok) return toast('已复制');\
                    try {\
                      prompt('复制失败，请手动复制：', full);\
                    } catch {}\
                    toast('复制失败');\
                  } catch (e) {\
                    console.warn('copy failed', e);\
                    try {\
                      prompt('复制失败，请手动复制：', full);\
                    } catch {}\
                    toast('复制失败');\
                  }\
                });\
              });\
            </script>");

            html.push_str("</div></body></html>");
            html
        })
        .await
        .unwrap_or_else(|_| "<html><body>list failed</body></html>".to_string());

        return HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(listing);
    }

    let size = meta.len();
    let content_type = guess_mime(&full);

    let range = req
        .headers()
        .get("Range")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let (start, end, status) = if range.starts_with("bytes=") && range.contains('-') {
        let v = range.trim_start_matches("bytes=");
        let mut it = v.splitn(2, '-');
        let s = it.next().unwrap_or("").trim();
        let e = it.next().unwrap_or("").trim();
        let start = s.parse::<u64>().unwrap_or(0);
        let end = if e.is_empty() {
            size.saturating_sub(1)
        } else {
            e.parse::<u64>().unwrap_or(size.saturating_sub(1))
        };
        (start, end.min(size.saturating_sub(1)), 206u16)
    } else {
        (0, size.saturating_sub(1), 200u16)
    };

    let mut f = match tokio::fs::File::open(&full).await {
        Ok(v) => v,
        Err(_) => return HttpResponse::NotFound().body("not found"),
    };
    if f.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return HttpResponse::InternalServerError().body("seek failed");
    }

    let remain = end.saturating_sub(start).saturating_add(1);
    let stream = file_stream(f, remain);

    let status_code = actix_web::http::StatusCode::from_u16(status)
        .unwrap_or(actix_web::http::StatusCode::OK);

    let mut resp = HttpResponse::build(status_code);
    resp.insert_header(("Content-Type", content_type));
    resp.insert_header(("Accept-Ranges", "bytes"));

    if status == 206 {
        resp.insert_header(("Content-Range", format!("bytes {}-{}/{}", start, end, size)));
        resp.insert_header(("Content-Length", remain.to_string()));
    } else {
        resp.insert_header(("Content-Length", size.to_string()));
    }

    resp.streaming(stream)
}

fn file_stream(
    file: tokio::fs::File,
    remaining: u64,
) -> impl Stream<Item = Result<web::Bytes, actix_web::Error>> {
    futures_util::stream::unfold((file, remaining), |(mut file, remaining)| async move {
        if remaining == 0 {
            return None;
        }

        let take = std::cmp::min(remaining, 64 * 1024) as usize;
        let mut buf = vec![0u8; take];

        match file.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                let next_remaining = remaining.saturating_sub(n as u64);
                Some((Ok(web::Bytes::from(buf)), (file, next_remaining)))
            }
            Err(_) => Some((
                Err(actix_web::error::ErrorInternalServerError("read failed")),
                (file, 0),
            )),
        }
    })
}

async fn share_put(req: HttpRequest, root: web::Data<PathBuf>, body: web::Bytes) -> HttpResponse {
    use std::sync::atomic::Ordering;
    if !share_allowed(&req) {
        return HttpResponse::Unauthorized().body("需要配对码：在地址后追加 ?token=...");
    }
    if SHARE_READ_ONLY.load(Ordering::Relaxed) {
        return HttpResponse::Forbidden().body("当前分享为只读，请先允许上传");
    }
    let tail = req.match_info().query("tail");
    let decoded = match urlencoding::decode(tail) {
        Ok(v) => v.to_string(),
        Err(_) => tail.to_string(),
    };

    let full = match safe_join(root.get_ref(), &decoded) {
        Ok(v) => v,
        Err(e) => return HttpResponse::BadRequest().body(e),
    };

    if decoded.ends_with('/') {
        return HttpResponse::BadRequest().body("上传目标不能为目录（请指定文件路径）");
    }

    if let Some(parent) = full.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            error!("[HttpShare] create parent failed: {}", e);
            return HttpResponse::InternalServerError().body("create dir failed");
        }
    }

    match tokio::fs::write(&full, &body).await {
        Ok(_) => HttpResponse::Ok().body("ok"),
        Err(e) => {
            error!("[HttpShare] write failed: {}", e);
            HttpResponse::InternalServerError().body("write failed")
        }
    }
}

#[tauri::command]
pub async fn filetransfer_start_receiver(
    app: AppHandle,
    bind_addr: Option<String>,
    port: Option<u16>,
    save_dir: Option<String>,
    access_token: Option<String>,
) -> Result<ReceiverStatus, String> {
    let mut guard = RECEIVER.lock().await;
    if guard.is_some() {
        return Err("接收服务已在运行".to_string());
    }

    let bind = bind_addr.unwrap_or_else(|| "0.0.0.0".to_string());
    let p = port.unwrap_or(DEFAULT_RECEIVER_PORT);
    let dir = save_dir
        .map(|s| PathBuf::from(s))
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(default_incoming_dir);

    info!(
        "[FileTransfer] start receiver bind={} port={} dir={}",
        bind,
        p,
        dir.to_string_lossy()
    );

    *RECEIVER_TOKEN.lock() = access_token.unwrap_or_default().trim().to_string();

    let store = UploadStore {
        save_dir: dir.clone(),
        locks: Arc::new(Mutex::new(HashMap::new())),
    };
    let server_app = app.clone();

    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(store.clone()))
            .app_data(web::Data::new(server_app.clone()))
            .route("/ft/ping", web::get().to(ft_ping))
            .route("/ft/signal", web::get().to(ft_signal_ws))
            .route("/ft/upload/init", web::post().to(ft_upload_init))
            .route("/ft/upload/chunk", web::post().to(ft_upload_chunk))
            .route("/ft/upload/complete", web::post().to(ft_upload_complete))
    })
    .bind((bind.as_str(), p))
    .map_err(|e| format!("绑定端口失败: {}", e))?
    .run();

    let handle = server.handle();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = server.await {
            error!("[FileTransfer] receiver server exited: {}", e);
        }
    });

    let base_url = format!("http://{}:{}", bind, p);
    *guard = Some(ReceiverRuntime {
        bind_addr: bind.clone(),
        port: p,
        save_dir: dir.clone(),
        handle,
    });

    let st = ReceiverStatus {
        running: true,
        bind_addr: bind,
        port: p,
        save_dir: dir.to_string_lossy().to_string(),
        base_url,
        requires_token: !receiver_token().is_empty(),
    };

    app.emit(
        EVENT_TASK_UPDATE,
        serde_json::json!({"type":"receiver_started"}),
    )
    .ok();
    Ok(st)
}

#[tauri::command]
pub async fn filetransfer_stop_receiver() -> Result<(), String> {
    let mut guard = RECEIVER.lock().await;
    if let Some(rt) = guard.take() {
        info!("[FileTransfer] stop receiver");
        rt.handle.stop(true).await;
    }
    *RECEIVER_TOKEN.lock() = String::new();
    Ok(())
}

#[tauri::command]
pub async fn filetransfer_start_http_share(
    bind_addr: Option<String>,
    port: Option<u16>,
    root_dir: String,
    access_token: Option<String>,
    read_only: Option<bool>,
) -> Result<HttpShareStatus, String> {
    let mut guard = SHARE.lock().await;
    if guard.is_some() {
        return Err("HTTP 分享已在运行".to_string());
    }

    let bind = bind_addr.unwrap_or_else(|| "0.0.0.0".to_string());
    let p = port.unwrap_or(DEFAULT_SHARE_PORT);
    let root = PathBuf::from(root_dir.trim());
    if root.as_os_str().is_empty() {
        return Err("root_dir 不能为空".to_string());
    }
    if !root.is_dir() {
        return Err("root_dir 不是目录或不存在".to_string());
    }

    info!(
        "[HttpShare] start bind={} port={} root={}",
        bind,
        p,
        root.to_string_lossy()
    );

    *SHARE_TOKEN.lock() = access_token.unwrap_or_default().trim().to_string();
    let share_read_only = read_only.unwrap_or(true);
    SHARE_READ_ONLY.store(share_read_only, std::sync::atomic::Ordering::Relaxed);

    let root_data = web::Data::new(root.clone());
    let server = HttpServer::new(move || {
        App::new()
            .app_data(root_data.clone())
            .route("/{tail:.*}", web::get().to(share_get))
            .route("/{tail:.*}", web::put().to(share_put))
    })
    .bind((bind.as_str(), p))
    .map_err(|e| format!("绑定端口失败: {}", e))?
    .run();

    let handle = server.handle();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = server.await {
            error!("[HttpShare] server exited: {}", e);
        }
    });

    let base_url = format!("http://{}:{}", bind, p);
    *guard = Some(ShareRuntime {
        bind_addr: bind.clone(),
        port: p,
        root_dir: root.clone(),
        handle,
    });

    Ok(HttpShareStatus {
        running: true,
        bind_addr: bind,
        port: p,
        root_dir: root.to_string_lossy().to_string(),
        base_url,
        read_only: share_read_only,
        requires_token: !SHARE_TOKEN.lock().is_empty(),
    })
}

#[tauri::command]
pub async fn filetransfer_stop_http_share() -> Result<(), String> {
    let mut guard = SHARE.lock().await;
    if let Some(rt) = guard.take() {
        info!("[HttpShare] stop");
        rt.handle.stop(true).await;
    }
    *SHARE_TOKEN.lock() = String::new();
    SHARE_READ_ONLY.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

/// 列出本机可用于局域网访问的地址，供传输中心选择/复制。
/// 监听端仍用 0.0.0.0，这样回环、多个网卡都能访问；这里只负责展示可分享的 IP。
#[tauri::command]
pub fn filetransfer_list_local_ips() -> Result<Vec<LocalIpInfo>, String> {
    let interfaces = NetworkInterface::show().map_err(|e| format!("枚举本机网卡失败：{}", e))?;
    let mut out = Vec::new();
    for interface in interfaces {
        for addr in interface.addr {
            match addr {
                Addr::V4(v4) => {
                    let ip = v4.ip;
                    if ip.is_unspecified() {
                        continue;
                    }
                    out.push(LocalIpInfo {
                        name: interface.name.clone(),
                        address: ip.to_string(),
                        family: "ipv4".to_string(),
                        is_loopback: ip.is_loopback(),
                    });
                }
                Addr::V6(v6) => {
                    let ip = v6.ip;
                    if ip.is_unspecified() || ip.is_loopback() {
                        continue;
                    }
                    // 跳过 fe80::/10 链路本地地址，其它设备通常无法直接使用。
                    if (ip.segments()[0] & 0xffc0) == 0xfe80 {
                        continue;
                    }
                    out.push(LocalIpInfo {
                        name: interface.name.clone(),
                        address: ip.to_string(),
                        family: "ipv6".to_string(),
                        is_loopback: false,
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| {
        let rank = |item: &LocalIpInfo| {
            if item.is_loopback {
                3
            } else {
                match item.address.parse::<std::net::IpAddr>() {
                    Ok(std::net::IpAddr::V4(v4)) if v4.is_private() => 0,
                    Ok(std::net::IpAddr::V4(_)) => 1,
                    _ => 2,
                }
            }
        };
        rank(a)
            .cmp(&rank(b))
            // 先按地址排序，保证同一 IP 出现在多个网卡时能连续去重。
            .then_with(|| a.address.cmp(&b.address))
            .then_with(|| a.name.cmp(&b.name))
    });
    out.dedup_by(|a, b| a.address == b.address);
    if out.is_empty() {
        out.push(LocalIpInfo {
            name: "本机回环".to_string(),
            address: "127.0.0.1".to_string(),
            family: "ipv4".to_string(),
            is_loopback: true,
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn filetransfer_get_status() -> Result<FileTransferStatus, String> {
    let receiver = {
        let g = RECEIVER.lock().await;
        g.as_ref().map(|rt| ReceiverStatus {
            running: true,
            bind_addr: rt.bind_addr.clone(),
            port: rt.port,
            save_dir: rt.save_dir.to_string_lossy().to_string(),
            base_url: format!("http://{}:{}", rt.bind_addr, rt.port),
            requires_token: !receiver_token().is_empty(),
        })
    };

    let http_share = {
        let g = SHARE.lock().await;
        g.as_ref().map(|rt| HttpShareStatus {
            running: true,
            bind_addr: rt.bind_addr.clone(),
            port: rt.port,
            root_dir: rt.root_dir.to_string_lossy().to_string(),
            base_url: format!("http://{}:{}", rt.bind_addr, rt.port),
            read_only: SHARE_READ_ONLY.load(std::sync::atomic::Ordering::Relaxed),
            requires_token: !SHARE_TOKEN.lock().is_empty(),
        })
    };

    let discovery_running = DISCOVERY.lock().await.is_some();

    Ok(FileTransferStatus {
        receiver,
        http_share,
        discovery_running,
    })
}

#[derive(Serialize, Deserialize)]
struct DiscoveryAnnounce {
    peer_id: String,
    name: String,
    port: u16,
    ts: i64,
    #[serde(default)]
    requires_token: bool,
}

fn machine_name_guess() -> String {
    let n = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Guaxi".to_string());
    format!("{}-{}", n, &PEER_ID[..8.min(PEER_ID.len())])
}

#[tauri::command]
pub async fn filetransfer_start_lan_discovery(
    app: AppHandle,
    discovery_port: Option<u16>,
) -> Result<(), String> {
    let mut g = DISCOVERY.lock().await;
    if g.is_some() {
        return Ok(());
    }

    let port = discovery_port.unwrap_or(DEFAULT_DISCOVERY_PORT);
    info!("[FileTransfer] start LAN discovery port={}", port);

    let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();
    *g = Some(DiscoveryRuntime { stop: tx });

    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let sock = match tokio::net::UdpSocket::bind(("0.0.0.0", port)).await {
            Ok(s) => s,
            Err(e) => {
                error!("[FileTransfer] discovery bind failed: {}", e);
                return;
            }
        };
        if let Err(e) = sock.set_broadcast(true) {
            warn!("[FileTransfer] set_broadcast failed: {}", e);
        }

        let name = machine_name_guess();
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(1200));
        let mut buf = vec![0u8; 64 * 1024];

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let port2 = RECEIVER.lock().await.as_ref().map(|x| x.port).unwrap_or(DEFAULT_RECEIVER_PORT);
                    let msg = DiscoveryAnnounce {
                        peer_id: PEER_ID.clone(),
                        name: name.clone(),
                        port: port2,
                        ts: now_ms(),
                        requires_token: !receiver_token().is_empty(),
                    };
                    if let Ok(bytes) = serde_json::to_vec(&msg) {
                        let _ = sock.send_to(&bytes, ("255.255.255.255", port)).await;
                    }
                }
                r = sock.recv_from(&mut buf) => {
                    if let Ok((n, addr)) = r {
                        if n == 0 { continue; }
                        if let Ok(msg) = serde_json::from_slice::<DiscoveryAnnounce>(&buf[..n]) {
                            if msg.peer_id == *PEER_ID { continue; }
                            let peer = PeerInfo {
                                peer_id: msg.peer_id.clone(),
                                name: msg.name.clone(),
                                addr: addr.ip().to_string(),
                                port: msg.port,
                                last_seen_ms: now_ms(),
                                requires_token: msg.requires_token,
                            };
                            {
                                let mut m = PEERS.lock().await;
                                m.insert(peer.peer_id.clone(), peer);
                                let list: Vec<PeerInfo> = m.values().cloned().collect();
                                let _ = app2.emit(EVENT_PEERS_UPDATE, list);
                            }
                        }
                    }
                }
                _ = &mut rx => {
                    info!("[FileTransfer] stop LAN discovery");
                    break;
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn filetransfer_stop_lan_discovery() -> Result<(), String> {
    let mut g = DISCOVERY.lock().await;
    if let Some(rt) = g.take() {
        let _ = rt.stop.send(());
    }
    Ok(())
}

#[tauri::command]
pub async fn filetransfer_get_peers() -> Result<Vec<PeerInfo>, String> {
    let m = PEERS.lock().await;
    Ok(m.values().cloned().collect())
}

#[tauri::command]
pub async fn filetransfer_list_tasks() -> Result<Vec<TransferTask>, String> {
    ensure_tasks_loaded().await;
    let m = TASKS.lock().await;
    let mut v: Vec<TransferTask> = m.values().cloned().collect();
    v.sort_by_key(|x| -(x.updated_at_ms));
    Ok(v)
}

#[tauri::command]
pub async fn filetransfer_clear_finished() -> Result<(), String> {
    ensure_tasks_loaded().await;
    {
        let mut map = TASKS.lock().await;
        map.retain(|_, task| task.status == "running" || task.status == "paused");
    }
    persist_tasks(true).await;
    Ok(())
}

#[tauri::command]
pub async fn filetransfer_pause_task(app: AppHandle, task_id: String) -> Result<(), String> {
    let id = task_id.trim().to_string();
    {
        let ctrls = TASK_CONTROLS.lock().await;
        if let Some(c) = ctrls.get(&id) {
            let mut g = c.lock().await;
            g.paused = true;
        }
    }
    let current = TASKS.lock().await.get(&id).cloned();
    if let Some(mut task) = current {
        task.status = "paused".to_string();
        task.updated_at_ms = now_ms();
        update_task(&app, task).await;
        info!("[FileTransfer] pause task {}", id);
    }
    Ok(())
}

#[tauri::command]
pub async fn filetransfer_resume_task(app: AppHandle, task_id: String) -> Result<(), String> {
    let id = task_id.trim().to_string();
    {
        let ctrls = TASK_CONTROLS.lock().await;
        if let Some(c) = ctrls.get(&id) {
            let mut g = c.lock().await;
            g.paused = false;
            g.notify.notify_waiters();
        }
    }
    let current = TASKS.lock().await.get(&id).cloned();
    if let Some(mut task) = current {
        task.status = "running".to_string();
        task.updated_at_ms = now_ms();
        update_task(&app, task).await;
        info!("[FileTransfer] resume task {}", id);
    }
    Ok(())
}

#[tauri::command]
pub async fn filetransfer_retry_task(
    app: AppHandle,
    task_id: String,
    access_token: Option<String>,
) -> Result<(), String> {
    let id = task_id.trim().to_string();
    ensure_tasks_loaded().await;
    let mut task = TASKS
        .lock()
        .await
        .get(&id)
        .cloned()
        .ok_or_else(|| "任务不存在".to_string())?;
    if task.direction != "send" {
        return Err("只有发送任务可以重试".to_string());
    }
    if !matches!(task.status.as_str(), "error" | "interrupted" | "canceled") {
        return Err("当前任务不需要重试".to_string());
    }
    let file_path = task.file_path.clone();
    let meta = tokio::fs::metadata(&file_path)
        .await
        .map_err(|error| format!("源文件不可用：{}", error))?;
    if !meta.is_file() {
        return Err("源文件不可用".to_string());
    }
    let total = meta.len();
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|value| value.as_millis() as i64)
        .unwrap_or(0);
    let relative_path = task.file_name.clone();
    let file_name = Path::new(&file_path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("file")
        .to_string();
    let upload_id = sha256_short(&format!(
        "{}:{}:{}:{}",
        file_path, total, mtime_ms, relative_path
    ));
    let peer = task.peer.clone();
    let token = access_token
        .unwrap_or_default()
        .trim()
        .to_string();
    let token = if token.is_empty() {
        task.access_token.clone().unwrap_or_default()
    } else {
        token
    };
    if !token.is_empty() {
        task.access_token = Some(token.clone());
    }

    let ctrl = Arc::new(Mutex::new(TaskControl::new()));
    TASK_CONTROLS.lock().await.insert(id.clone(), ctrl.clone());

    task.status = "running".to_string();
    task.transferred_bytes = 0;
    task.total_bytes = total;
    task.error = None;
    task.updated_at_ms = now_ms();
    update_task(&app, task).await;

    let app2 = app.clone();
    let task_id = id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = do_send_file(
            &app2,
            &peer,
            &file_path,
            &file_name,
            &relative_path,
            total,
            mtime_ms,
            &upload_id,
            &task_id,
            &token,
            ctrl,
        )
        .await
        {
            error!("[FileTransfer] retry failed: {}", error);
            let current = TASKS.lock().await.get(&task_id).cloned();
        if let Some(mut task) = current {
                task.status = "error".to_string();
                task.error = Some(error);
                task.updated_at_ms = now_ms();
                update_task(&app2, task).await;
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn filetransfer_cancel_task(app: AppHandle, task_id: String) -> Result<(), String> {
    let id = task_id.trim().to_string();
    {
        let ctrls = TASK_CONTROLS.lock().await;
        if let Some(c) = ctrls.get(&id) {
            let mut g = c.lock().await;
            g.canceled = true;
            g.paused = false;
            g.notify.notify_waiters();
        }
    }
    let current = TASKS.lock().await.get(&id).cloned();
    if let Some(mut task) = current {
        task.status = "canceled".to_string();
        task.updated_at_ms = now_ms();
        update_task(&app, task).await;
        info!("[FileTransfer] cancel task {}", id);
    }
    Ok(())
}

fn tasks_state_path() -> PathBuf {
    get_config_path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("filetransfer")
        .join("tasks.json")
}

/// 首次访问任务列表时从磁盘恢复；上次没传完的任务标记为中断，保留历史。
async fn ensure_tasks_loaded() {
    use std::sync::atomic::Ordering;
    if TASKS_LOADED.load(Ordering::Acquire) {
        return;
    }
    let _guard = TASKS_LOAD_LOCK.lock().await;
    if TASKS_LOADED.load(Ordering::Acquire) {
        return;
    }
    if let Ok(raw) = tokio::fs::read_to_string(tasks_state_path()).await {
        if let Ok(list) = serde_json::from_str::<Vec<TransferTask>>(&raw) {
            let mut map = TASKS.lock().await;
            for mut task in list {
                if task.status == "running" || task.status == "paused" {
                    task.status = "interrupted".to_string();
                    task.error = Some("应用重启，传输已中断".to_string());
                    task.updated_at_ms = now_ms();
                }
                map.insert(task.id.clone(), task);
            }
        }
    }
    TASKS_LOADED.store(true, Ordering::Release);
}

async fn persist_tasks(force: bool) {
    use std::sync::atomic::Ordering;
    if !force && now_ms() - LAST_PERSIST_MS.load(Ordering::Relaxed) < 1200 {
        return;
    }
    LAST_PERSIST_MS.store(now_ms(), Ordering::Relaxed);
    let mut list: Vec<TransferTask> = TASKS.lock().await.values().cloned().collect();
    list.sort_by_key(|task| std::cmp::Reverse(task.created_at_ms));
    list.truncate(300);
    let path = tasks_state_path();
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    if let Ok(raw) = serde_json::to_vec_pretty(&list) {
        if let Err(error) = tokio::fs::write(&path, raw).await {
            warn!("[FileTransfer] persist tasks failed: {}", error);
        }
    }
}

async fn update_task(app: &AppHandle, t: TransferTask) {
    ensure_tasks_loaded().await;
    let force = t.status != "running";
    {
        let mut m = TASKS.lock().await;
        m.insert(t.id.clone(), t.clone());
    }
    persist_tasks(force).await;
    let _ = app.emit(EVENT_TASK_UPDATE, t);
}

/// 把用户选中的文件/文件夹展开成待发送条目；文件夹保留顶层目录名。
#[tauri::command]
pub async fn filetransfer_collect_entries(paths: Vec<String>) -> Result<Vec<SendPathEntry>, String> {
    let mut out = Vec::new();
    for raw in paths {
        let root = PathBuf::from(raw.trim());
        if root.is_file() {
            let name = root
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("file")
                .to_string();
            out.push(SendPathEntry {
                path: root.to_string_lossy().to_string(),
                relative_path: name,
            });
            continue;
        }
        if !root.is_dir() {
            continue;
        }
        let root_name = root
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("folder")
            .to_string();
        for entry in walkdir::WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_map(|item| item.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = match entry.path().strip_prefix(&root) {
                Ok(value) => value.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };
            out.push(SendPathEntry {
                path: entry.path().to_string_lossy().to_string(),
                relative_path: format!("{}/{}", root_name, rel),
            });
            if out.len() > 50_000 {
                return Err("一次最多发送 50000 个文件".to_string());
            }
        }
    }
    out.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(out)
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SendPathEntry {
    pub path: String,
    /// 文件夹内文件的相对路径（含文件名）；单文件留空。
    #[serde(default)]
    pub relative_path: String,
}

#[tauri::command]
pub async fn filetransfer_send_files(
    app: AppHandle,
    peer_url: String,
    file_paths: Vec<String>,
) -> Result<(), String> {
    let entries = file_paths
        .into_iter()
        .map(|path| SendPathEntry {
            path,
            relative_path: String::new(),
        })
        .collect();
    filetransfer_send_entries(app, peer_url, entries, None).await
}

fn apply_token(request: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
    if token.is_empty() {
        request
    } else {
        request.header("x-guaxi-token", token)
    }
}

#[tauri::command]
pub async fn filetransfer_send_entries(
    app: AppHandle,
    peer_url: String,
    entries: Vec<SendPathEntry>,
    access_token: Option<String>,
) -> Result<(), String> {
    let peer = normalize_base_url(&peer_url);
    if peer.is_empty() {
        return Err("peer_url 不能为空".to_string());
    }
    let token = access_token.unwrap_or_default().trim().to_string();

    let list: Vec<SendPathEntry> = entries
        .into_iter()
        .filter(|entry| !entry.path.trim().is_empty())
        .collect();

    if list.is_empty() {
        return Err("没有要发送的文件".to_string());
    }

    for entry in list {
        let app2 = app.clone();
        let peer2 = peer.clone();

        let task_id = Uuid::new_v4().to_string();
        let ctrl = Arc::new(Mutex::new(TaskControl::new()));
        TASK_CONTROLS
            .lock()
            .await
            .insert(task_id.clone(), ctrl.clone());

        let file_path = entry.path.trim().to_string();
        let file_name = Path::new(&file_path)
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or("file")
            .to_string();
        let relative_path = if entry.relative_path.trim().is_empty() {
            file_name.clone()
        } else {
            entry.relative_path.trim().replace('\\', "/")
        };

        let meta = tokio::fs::metadata(&file_path)
            .await
            .map_err(|e| e.to_string())?;
        if !meta.is_file() {
            continue;
        }
        let total = meta.len();
        let mtime_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let upload_id =
            sha256_short(&format!("{}:{}:{}:{}", file_path, total, mtime_ms, relative_path));

        let t0 = TransferTask {
            id: task_id.clone(),
            direction: "send".to_string(),
            status: "running".to_string(),
            file_name: relative_path.clone(),
            file_path: file_path.clone(),
            peer: peer2.clone(),
            total_bytes: total,
            transferred_bytes: 0,
            created_at_ms: now_ms(),
            updated_at_ms: now_ms(),
            error: None,
            access_token: if token.is_empty() { None } else { Some(token.clone()) },
        };
        update_task(&app2, t0).await;

        let task_token = token.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = do_send_file(
                &app2,
                &peer2,
                &file_path,
                &file_name,
                &relative_path,
                total,
                mtime_ms,
                &upload_id,
                &task_id,
                &task_token,
                ctrl,
            )
            .await
            {
                error!("[FileTransfer] send failed: {}", e);
                let mut t = TASKS
                    .lock()
                    .await
                    .get(&task_id)
                    .cloned()
                    .unwrap_or_else(|| TransferTask {
                        id: task_id.clone(),
                        direction: "send".to_string(),
                        status: "error".to_string(),
                        file_name: relative_path.clone(),
                        file_path: file_path.clone(),
                        peer: peer2.clone(),
                        total_bytes: total,
                        transferred_bytes: 0,
                        created_at_ms: now_ms(),
                        updated_at_ms: now_ms(),
                        error: Some(e.clone()),
                        access_token: if task_token.is_empty() { None } else { Some(task_token.clone()) },
                    });
                t.status = "error".to_string();
                t.updated_at_ms = now_ms();
                t.error = Some(e);
                update_task(&app2, t).await;
            }
        });
    }

    Ok(())
}

async fn do_send_file(
    app: &AppHandle,
    peer: &str,
    file_path: &str,
    file_name: &str,
    relative_path: &str,
    total: u64,
    mtime_ms: i64,
    upload_id: &str,
    task_id: &str,
    token: &str,
    ctrl: Arc<Mutex<TaskControl>>,
) -> Result<(), String> {
    info!(
        "[FileTransfer] send start task={} peer={} file={} bytes={}",
        task_id, peer, file_path, total
    );

    let client = reqwest::Client::new();

    let init_url = format!("{}/ft/upload/init", peer);
    let init_req = UploadInitReq {
        upload_id: upload_id.to_string(),
        file_name: file_name.to_string(),
        file_size: total,
        mtime_ms,
        relative_path: relative_path.to_string(),
    };

    let init_resp = apply_token(client.post(init_url).json(&init_req), token)
        .send()
        .await
        .map_err(|e| format!("init 请求失败: {}", e))?;

    if !init_resp.status().is_success() {
        let text = init_resp.text().await.unwrap_or_default();
        return Err(format!("init 失败: {}", text));
    }

    let init_data: UploadInitResp = init_resp.json().await.map_err(|e| e.to_string())?;
    let mut offset = init_data.received_bytes.min(total);

    let mut f = tokio::fs::File::open(file_path)
        .await
        .map_err(|e| e.to_string())?;
    f.seek(std::io::SeekFrom::Start(offset))
        .await
        .map_err(|e| e.to_string())?;

    let mut buf = vec![0u8; CHUNK_SIZE];

    loop {
        {
            let mut c = ctrl.lock().await;
            if c.canceled {
                let mut t = TASKS.lock().await.get(task_id).cloned().unwrap();
                t.status = "canceled".to_string();
                t.updated_at_ms = now_ms();
                update_task(app, t).await;
                return Ok(());
            }
            while c.paused {
                let notify = c.notify.clone();
                drop(c);
                notify.notified().await;
                c = ctrl.lock().await;
                if c.canceled {
                    let mut t = TASKS.lock().await.get(task_id).cloned().unwrap();
                    t.status = "canceled".to_string();
                    t.updated_at_ms = now_ms();
                    update_task(app, t).await;
                    return Ok(());
                }
            }
        }

        let n = f.read(&mut buf).await.map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }

        let chunk = &buf[..n];
        let chunk_url = format!(
            "{}/ft/upload/chunk?upload_id={}&offset={}",
            peer, upload_id, offset
        );

        let resp = apply_token(client.post(chunk_url).body(chunk.to_vec()), token)
            .send()
            .await
            .map_err(|e| format!("chunk 请求失败: {}", e))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("chunk 失败: {}", text));
        }

        offset += n as u64;

        let mut t = TASKS.lock().await.get(task_id).cloned().unwrap();
        t.transferred_bytes = offset;
        t.updated_at_ms = now_ms();
        t.status = "running".to_string();
        update_task(app, t).await;
    }

    let done_url = format!("{}/ft/upload/complete", peer);
    let resp = apply_token(
        client.post(done_url).json(&UploadCompleteReq {
            upload_id: upload_id.to_string(),
        }),
        token,
    )
    .send()
    .await
    .map_err(|e| format!("complete 请求失败: {}", e))?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("complete 失败: {}", text));
    }

    let mut t = TASKS.lock().await.get(task_id).cloned().unwrap();
    t.transferred_bytes = total;
    t.status = "done".to_string();
    t.updated_at_ms = now_ms();
    update_task(app, t).await;

    info!(
        "[FileTransfer] send done task={} file={}",
        task_id, file_path
    );
    Ok(())
}

async fn webrtc_get_lock(upload_id: &str) -> Arc<Mutex<()>> {
    let mut locks = WEBRTC_LOCKS.lock().await;
    if let Some(l) = locks.get(upload_id) {
        return l.clone();
    }
    let l = Arc::new(Mutex::new(()));
    locks.insert(upload_id.to_string(), l.clone());
    l
}

fn effective_incoming_dir(save_dir: Option<String>) -> PathBuf {
    if let Some(s) = save_dir {
        let p = PathBuf::from(s.trim());
        if !p.as_os_str().is_empty() {
            return p;
        }
    }

    if let Ok(g) = RECEIVER.try_lock() {
        if let Some(rt) = g.as_ref() {
            return rt.save_dir.clone();
        }
    }

    default_incoming_dir()
}

#[tauri::command]
pub async fn filetransfer_stat_file(file_path: String) -> Result<FileStatResp, String> {
    let path = PathBuf::from(file_path.trim());
    if path.as_os_str().is_empty() {
        return Err("file_path 不能为空".to_string());
    }

    let file_name = path
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("file")
        .to_string();

    let meta = tokio::fs::metadata(&path).await.map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("file_path 不是文件".to_string());
    }

    let total_bytes = meta.len();
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    Ok(FileStatResp {
        file_name,
        total_bytes,
        mtime_ms,
    })
}

#[tauri::command]
pub async fn filetransfer_compute_upload_id(file_path: String) -> Result<String, String> {
    let st = filetransfer_stat_file(file_path.clone()).await?;
    Ok(sha256_short(&format!(
        "{}:{}:{}",
        file_path.trim(),
        st.total_bytes,
        st.mtime_ms
    )))
}

#[tauri::command]
pub async fn filetransfer_read_file_chunk_base64(
    file_path: String,
    offset: u64,
    length: u32,
) -> Result<String, String> {
    let path = PathBuf::from(file_path.trim());
    if path.as_os_str().is_empty() {
        return Err("file_path 不能为空".to_string());
    }

    let meta = tokio::fs::metadata(&path).await.map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("file_path 不是文件".to_string());
    }

    if offset >= meta.len() {
        return Ok(String::new());
    }

    let take = std::cmp::min(length as u64, meta.len().saturating_sub(offset)) as usize;
    if take == 0 {
        return Ok(String::new());
    }

    let mut f = tokio::fs::File::open(&path).await.map_err(|e| e.to_string())?;
    f.seek(std::io::SeekFrom::Start(offset))
        .await
        .map_err(|e| e.to_string())?;

    let mut buf = vec![0u8; take];
    let n = f.read(&mut buf).await.map_err(|e| e.to_string())?;
    buf.truncate(n);

    if n == 0 {
        return Ok(String::new());
    }

    Ok(general_purpose::STANDARD.encode(&buf))
}

/// WebRTC 接收端：初始化一次“落盘上传会话”。
///
/// 协议说明（供前端 DataChannel 使用）：
/// - init: { upload_id, file_name, file_size, mtime_ms, peer_label? }
/// - init_ack: { upload_id, received_bytes }
/// - chunk: { upload_id, offset, data_base64 }
/// - complete: { upload_id }
#[tauri::command]
pub async fn filetransfer_webrtc_recv_init(
    app: AppHandle,
    save_dir: Option<String>,
    upload_id: String,
    file_name: String,
    file_size: u64,
    mtime_ms: i64,
    peer_label: Option<String>,
) -> Result<WebRtcRecvInitResp, String> {
    let upload_id = upload_id.trim().to_string();
    if upload_id.is_empty() {
        return Err("upload_id 不能为空".to_string());
    }

    let save_dir_path = effective_incoming_dir(save_dir);

    tokio::fs::create_dir_all(save_dir_path.join(".guaxi_transfer_tmp"))
        .await
        .map_err(|e| {
            error!("[FileTransfer][WebRTC] create tmp dir failed: {}", e);
            "创建临时目录失败".to_string()
        })?;
    tokio::fs::create_dir_all(&save_dir_path).await.map_err(|e| {
        error!("[FileTransfer][WebRTC] create save dir failed: {}", e);
        "创建保存目录失败".to_string()
    })?;

    let meta_path = upload_meta_path(&save_dir_path, &upload_id);
    let partial_path = upload_partial_path(&save_dir_path, &upload_id);

    let mut final_name = unique_relative_path(&save_dir_path, &file_name)?;
    let mut received_bytes = 0u64;

    if meta_path.exists() {
        if let Ok(raw) = tokio::fs::read_to_string(&meta_path).await {
            if let Ok(meta) = serde_json::from_str::<UploadMeta>(&raw) {
                if meta.file_size == file_size && meta.mtime_ms == mtime_ms {
                    final_name = meta.final_name;
                    if let Ok(m) = tokio::fs::metadata(&partial_path).await {
                        received_bytes = m.len();
                    }
                } else {
                    warn!(
                        "[FileTransfer][WebRTC] meta mismatch, reset upload_id={} old_size={} new_size={} old_mtime={} new_mtime={}",
                        upload_id, meta.file_size, file_size, meta.mtime_ms, mtime_ms
                    );
                }
            }
        }
    }

    if !meta_path.exists() {
        let meta = UploadMeta {
            upload_id: upload_id.clone(),
            file_name: file_name.clone(),
            file_size,
            mtime_ms,
            final_name: final_name.clone(),
            task_id: String::new(),
        };
        if let Ok(s) = serde_json::to_string(&meta) {
            if let Err(e) = tokio::fs::write(&meta_path, s).await {
                error!("[FileTransfer][WebRTC] write meta failed: {}", e);
            }
        }
    }

    let task_id = {
        let mut uploads = WEBRTC_UPLOADS.lock().await;
        if let Some(ctx) = uploads.get(&upload_id) {
            ctx.task_id.clone()
        } else {
            let task_id = Uuid::new_v4().to_string();
            uploads.insert(
                upload_id.clone(),
                WebRtcUploadCtx {
                    save_dir: save_dir_path.clone(),
                    file_name: file_name.clone(),
                    file_size,
                    final_name: final_name.clone(),
                    task_id: task_id.clone(),
                },
            );
            task_id
        }
    };

    let peer = peer_label
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| format!("webrtc:{}", s))
        .unwrap_or_else(|| "webrtc".to_string());

    let t0 = TransferTask {
        id: task_id.clone(),
        direction: "recv".to_string(),
        status: "running".to_string(),
        file_name: file_name.clone(),
        file_path: save_dir_path.to_string_lossy().to_string(),
        peer,
        total_bytes: file_size,
        transferred_bytes: received_bytes.min(file_size),
        created_at_ms: now_ms(),
        updated_at_ms: now_ms(),
        error: None,
        access_token: None,
    };
    update_task(&app, t0).await;

    info!(
        "[FileTransfer][WebRTC] recv init upload_id={} file={} size={} received={} save_dir={}",
        upload_id,
        file_name,
        file_size,
        received_bytes,
        save_dir_path.to_string_lossy()
    );

    Ok(WebRtcRecvInitResp {
        upload_id,
        received_bytes,
        save_dir: save_dir_path.to_string_lossy().to_string(),
        final_name,
        task_id,
    })
}

#[tauri::command]
pub async fn filetransfer_webrtc_recv_chunk_base64(
    app: AppHandle,
    upload_id: String,
    offset: u64,
    data_base64: String,
) -> Result<u64, String> {
    let upload_id = upload_id.trim().to_string();
    if upload_id.is_empty() {
        return Err("upload_id 不能为空".to_string());
    }

    let ctx = {
        let uploads = WEBRTC_UPLOADS.lock().await;
        uploads
            .get(&upload_id)
            .cloned()
            .ok_or_else(|| "unknown upload_id（请先 init）".to_string())?
    };

    let bytes = general_purpose::STANDARD
        .decode(data_base64.as_bytes())
        .map_err(|e| format!("base64 解码失败: {}", e))?;
    if bytes.is_empty() {
        return Ok(offset);
    }

    let lock = webrtc_get_lock(&upload_id).await;
    let _g = lock.lock().await;

    let partial_path = upload_partial_path(&ctx.save_dir, &upload_id);

    let mut f = tokio::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&partial_path)
        .await
        .map_err(|e| e.to_string())?;

    f.seek(std::io::SeekFrom::Start(offset))
        .await
        .map_err(|e| e.to_string())?;
    f.write_all(&bytes).await.map_err(|e| e.to_string())?;
    f.flush().await.ok();

    let end = offset.saturating_add(bytes.len() as u64);

    let mut t = TASKS
        .lock()
        .await
        .get(&ctx.task_id)
        .cloned()
        .unwrap_or_else(|| TransferTask {
            id: ctx.task_id.clone(),
            direction: "recv".to_string(),
            status: "running".to_string(),
            file_name: ctx.file_name.clone(),
            file_path: ctx.save_dir.to_string_lossy().to_string(),
            peer: "webrtc".to_string(),
            total_bytes: ctx.file_size,
            transferred_bytes: 0,
            created_at_ms: now_ms(),
            updated_at_ms: now_ms(),
            error: None,
            access_token: None,
        });

    t.transferred_bytes = std::cmp::min(ctx.file_size, end);
    t.updated_at_ms = now_ms();
    t.status = "running".to_string();
    update_task(&app, t).await;

    Ok(end)
}

#[tauri::command]
pub async fn filetransfer_webrtc_recv_complete(
    app: AppHandle,
    upload_id: String,
) -> Result<String, String> {
    let upload_id = upload_id.trim().to_string();
    if upload_id.is_empty() {
        return Err("upload_id 不能为空".to_string());
    }

    let ctx = {
        let uploads = WEBRTC_UPLOADS.lock().await;
        uploads
            .get(&upload_id)
            .cloned()
            .ok_or_else(|| "unknown upload_id（请先 init）".to_string())?
    };

    let lock = webrtc_get_lock(&upload_id).await;
    let _g = lock.lock().await;

    let meta_path = upload_meta_path(&ctx.save_dir, &upload_id);
    let partial_path = upload_partial_path(&ctx.save_dir, &upload_id);

    let meta: Option<UploadMeta> = match tokio::fs::read_to_string(&meta_path).await {
        Ok(raw) => serde_json::from_str(&raw).ok(),
        Err(_) => None,
    };

    let final_name = meta
        .as_ref()
        .map(|m| m.final_name.clone())
        .unwrap_or_else(|| ctx.final_name.clone());

    let final_path = safe_join(&ctx.save_dir, &final_name)?;
    if let Some(parent) = final_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("创建目标目录失败: {}", e))?;
    }

    tokio::fs::rename(&partial_path, &final_path)
        .await
        .map_err(|e| format!("保存文件失败: {}", e))?;

    let _ = tokio::fs::remove_file(&meta_path).await;

    let mut t = TASKS
        .lock()
        .await
        .get(&ctx.task_id)
        .cloned()
        .unwrap_or_else(|| TransferTask {
            id: ctx.task_id.clone(),
            direction: "recv".to_string(),
            status: "done".to_string(),
            file_name: ctx.file_name.clone(),
            file_path: final_path.to_string_lossy().to_string(),
            peer: "webrtc".to_string(),
            total_bytes: ctx.file_size,
            transferred_bytes: ctx.file_size,
            created_at_ms: now_ms(),
            updated_at_ms: now_ms(),
            error: None,
            access_token: None,
        });

    t.transferred_bytes = ctx.file_size;
    t.status = "done".to_string();
    t.updated_at_ms = now_ms();
    t.file_path = final_path.to_string_lossy().to_string();
    update_task(&app, t).await;

    info!(
        "[FileTransfer][WebRTC] recv done upload_id={} saved={}",
        upload_id,
        final_path.to_string_lossy()
    );

    Ok(final_path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_relative_path_rejects_escape_and_normalizes() {
        assert_eq!(
            sanitize_relative_path("docs/2026/report.pdf").expect("normal"),
            "docs/2026/report.pdf"
        );
        assert_eq!(
            sanitize_relative_path("./docs//report.pdf").expect("normalize"),
            "docs/report.pdf"
        );
        assert!(sanitize_relative_path("../escape.pdf").is_err());
        assert!(sanitize_relative_path("/abs/escape.pdf").is_err());
        assert!(sanitize_relative_path("C:/escape.pdf").is_err());
        assert!(sanitize_relative_path("   ").is_err());
    }

    #[test]
    fn unique_relative_path_preserves_folder_and_avoids_overwrite() {
        let root = std::env::temp_dir().join(format!("guaxi-ft-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("docs")).expect("create docs");
        std::fs::write(root.join("docs/report.pdf"), b"old").expect("write old");

        assert_eq!(
            unique_relative_path(&root, "docs/new.pdf").expect("new"),
            "docs/new.pdf"
        );
        assert_eq!(
            unique_relative_path(&root, "docs/report.pdf").expect("unique"),
            "docs/report (1).pdf"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn task_roundtrip_keeps_access_token_for_retry() {
        let task = TransferTask {
            id: "t1".to_string(),
            direction: "send".to_string(),
            status: "error".to_string(),
            file_name: "docs/a.pdf".to_string(),
            file_path: "D:/docs/a.pdf".to_string(),
            peer: "http://127.0.0.1:39190".to_string(),
            total_bytes: 10,
            transferred_bytes: 5,
            created_at_ms: 1,
            updated_at_ms: 2,
            error: Some("断开".to_string()),
            access_token: Some("pair-code".to_string()),
        };
        let raw = serde_json::to_string(&task).expect("serialize");
        let decoded: TransferTask = serde_json::from_str(&raw).expect("deserialize");
        assert_eq!(decoded.access_token.as_deref(), Some("pair-code"));
        assert_eq!(decoded.file_name, "docs/a.pdf");
    }

}