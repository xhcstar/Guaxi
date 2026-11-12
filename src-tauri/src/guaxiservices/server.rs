use crate::guaxiservices::chrome_extension_service;
use crate::guaxiservices::lsp;
use crate::guaxiservices::markdown_browser::browser_preview_config;
use crate::guaxiservices::markdown_image_process;
use crate::commands::notebook_lab::{
    dispatch_notebook_data_lab_rpc, notebook_data_lab_health_response,
    NotebookDataLabRpcRequest,
};
use crate::commands::plugin_llm::{complete as plugin_llm_run, PluginLlmRequest, PluginLlmResponse};
use crate::state::SharedState;
use actix_cors::Cors;
use actix_web::web::{Data, JsonConfig, PayloadConfig};
use actix_web::dev::ServerHandle;
use actix_web::{dev::Server, web, App, HttpResponse, HttpServer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{Emitter, Manager};
use tracing::{info, warn};

#[derive(Debug, Deserialize, Serialize)]
struct WebModelChatMessage {
    role: String,
    content: String,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    message_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WebModelChatPushRequest {
    provider: String,
    #[serde(default)]
    model_id: Option<i32>,
    #[serde(default)]
    topic_id: Option<i32>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    messages: Vec<WebModelChatMessage>,
}

fn provider_to_model_id(provider: &str) -> Option<i32> {
    crate::commands::chatview::web_provider_model_id(provider)
}

async fn web_model_chat_push(
    state: Data<SharedState>,
    req: web::Json<WebModelChatPushRequest>,
) -> HttpResponse {
    let provider = req.provider.trim().to_lowercase();
    let status = req.status.as_deref().map(|s| s.trim().to_lowercase());

    let model_id = req
        .model_id
        .or_else(|| provider_to_model_id(&provider));

    let Some(model_id) = model_id else {
        return HttpResponse::BadRequest().json(json!({
            "success": false,
            "message": "Unsupported provider"
        }));
    };

    let topic_id = req.topic_id.unwrap_or(0);

    let payload = json!({
        "provider": provider,
        "model_id": model_id,
        "topic_id": topic_id,
        "session_id": req.session_id,
        "title": req.title,
        "status": status,
        "messages": req.messages,
    });

    if let Some(app_handle) = state.get_app_handle() {
        if let Some(status) = payload.get("status").and_then(|v| v.as_str()) {
            let label = format!("web-model-{}", payload.get("provider").and_then(|v| v.as_str()).unwrap_or(""));
            if let Some(w) = app_handle.get_webview_window(&label) {
                if status == "need_login" {
                    let _ = w.show();
                    w.set_focus().ok();
                } else if status == "ready" {
                    let _ = w.hide();
                }
            }
        }

        let pushed_session = payload
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let pushed_status = payload
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let pushed_messages = payload
            .get("messages")
            .and_then(|v| v.as_array())
            .map(|v| v.len())
            .unwrap_or(0);

        if let Err(e) = app_handle.emit("web-model-chat-push", payload) {
            warn!("emit web-model-chat-push failed: {}", e);
            return HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "emit failed"
            }));
        }
        info!(
            model_id = model_id,
            topic_id = topic_id,
            session_id = %pushed_session,
            status = %pushed_status,
            messages = pushed_messages,
            "received web-model chat push"
        );
    } else {
        warn!("AppHandle not initialized; skipping web-model-chat-push emit");
    }

    HttpResponse::Ok().json(json!({ "success": true }))
}

/// 插件专用补全：密钥留在 Rust 侧，Python 插件只发 prompt。
/// 失败也返回 200 + success=false，避免插件侧 urllib 直接抛 HTTPError 丢消息。
async fn plugin_llm_complete(req: web::Json<PluginLlmRequest>) -> HttpResponse {
    match plugin_llm_run(req.into_inner()).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(message) => HttpResponse::Ok().json(PluginLlmResponse {
            success: false,
            text: String::new(),
            message: Some(message),
            provider: None,
        }),
    }
}

/// 插件宿主上下文：让插件拿到稳定路径与能力声明，不用自己猜目录。
async fn plugin_host_context() -> HttpResponse {
    let plugins = crate::commands::editor_plugin::get_editor_plugins_dir().unwrap_or_default();
    let knowledge_bases_root = crate::commands::settingview::get_knowledge_bases_root();
    let libraries_root = crate::commands::settingview::get_libraries_root();
    let task_root = crate::commands::taskmanager::task_get_config()
        .map(|config| config.root)
        .unwrap_or_default();

    HttpResponse::Ok().json(json!({
        "success": true,
        "app": { "name": "Guaxi", "version": env!("CARGO_PKG_VERSION") },
        "paths": {
            "plugins": plugins,
            "knowledgeBasesRoot": knowledge_bases_root,
            "librariesRoot": libraries_root,
            "taskRoot": task_root
        },
        "capabilities": {
            "llm": true,
            "mcp": true,
            "pluginUi": "declarative",
            "filePicker": true
        }
    }))
}

async fn notebook_data_lab_health() -> HttpResponse {
    HttpResponse::Ok().json(notebook_data_lab_health_response())
}

async fn notebook_data_lab_rpc_http(
    state: Data<SharedState>,
    req: web::Json<NotebookDataLabRpcRequest>,
) -> HttpResponse {
    match dispatch_notebook_data_lab_rpc(state.get_ref(), &req.0, "http-api") {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(error) => {
            warn!(error = error.as_str(), "[NotebookDataLabRpc] http rpc failed");
            HttpResponse::BadRequest().json(json!({
                "success": false,
                "message": error,
            }))
        }
    }
}

/// 本地后端（8888）与局域网导入接收（14219）的停止句柄。
///
/// actix 的 `ServerHandle` 可以在服务运行期间要求它退出。存下来之后「设置 → 服务器」才能
/// 停掉/重启这两个服务 —— 在此之前它们只是 fire-and-forget 的 spawn，句柄没被任何人持有，
/// 除了杀进程没有任何办法停。
static LOCAL_BACKEND_HANDLE: OnceLock<Mutex<Option<RunningActix>>> = OnceLock::new();
static CAPTURE_IMPORT_HANDLE: OnceLock<Mutex<Option<RunningActix>>> = OnceLock::new();

/// 一个正在跑的 actix 服务：停止句柄 + 「它还活着吗」的标记。
///
/// 光有句柄不够：服务自己崩了（worker panic、内部错误退出）时没人会去清句柄，
/// 于是状态页会一直报「运行中」。`alive` 由跑服务的那个后台任务在退出时置 false。
/// 它是**每个实例自己的一份**（`Arc`），所以旧实例退出时不会把新实例的标记带下来。
struct RunningActix {
    handle: ServerHandle,
    alive: Arc<AtomicBool>,
}

impl RunningActix {
    fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }
}

fn local_backend_handle_slot() -> &'static Mutex<Option<RunningActix>> {
    LOCAL_BACKEND_HANDLE.get_or_init(|| Mutex::new(None))
}

fn capture_import_handle_slot() -> &'static Mutex<Option<RunningActix>> {
    CAPTURE_IMPORT_HANDLE.get_or_init(|| Mutex::new(None))
}

/// 本进程是否正在跑 8888 后端（服务已退出则视为没在跑）。
///
/// 为什么需要这个：端口探测只能看出「有人在监听」，看不出「监听的是不是我这个进程」。
/// 用调试版和安装版同时开、或上一个实例没退干净时，8888 会被**别人**占着，端口探测照样
/// 报「运行中」，而请求其实被另一个实例（另一份配置、另一套图床根目录）接走了 ——
/// 表现就是「服务显示运行中，但图床接口 404」。拿句柄判断才能区分这两种情况。
pub fn local_backend_owned() -> bool {
    local_backend_handle_slot()
        .lock()
        .map(|mut guard| match guard.as_ref() {
            Some(running) if running.is_alive() => true,
            Some(_) => {
                guard.take();
                false
            }
            None => false,
        })
        .unwrap_or(false)
}

/// 本进程是否正在跑 14219 局域网导入接收。
pub fn capture_import_owned() -> bool {
    capture_import_handle_slot()
        .lock()
        .map(|mut guard| match guard.as_ref() {
            Some(running) if running.is_alive() => true,
            Some(_) => {
                guard.take();
                false
            }
            None => false,
        })
        .unwrap_or(false)
}

/// 优雅停机的等待上限。
///
/// 8888 上挂着 LSP websocket、浏览器预览这类**长连接**，而 actix 的优雅停机要等所有连接
/// 自行结束，默认上限 30 秒 —— 设置页点「重启」就会转圈半分钟。3 秒足够让在途的取图请求
/// 收尾；超过就强制断开，由前端各自的重连逻辑接管。
const SHUTDOWN_TIMEOUT_SECS: u64 = 3;

/// 等 `stop(true)` 返回的兜底上限：比 actix 自己的 shutdown_timeout 宽一点，
/// 只在 actix 内部状态异常时才会触发。
const STOP_WAIT: Duration = Duration::from_secs(8);

/// 停掉一个 actix 服务。
///
/// actix 的优雅停机要等所有连接自行结束，默认上限 30 秒；8888 上挂着 LSP websocket、
/// 浏览器预览这类长连接，所以点「重启」会转圈半分钟。现在两个服务在建的时候就设了
/// `shutdown_timeout(SHUTDOWN_TIMEOUT_SECS)`，actix 到点会强制丢弃残余连接；这里的
/// 超时只是最后一道兜底（正常情况下永远不该触发）。
async fn stop_actix(handle: ServerHandle, label: &str) {
    if tokio::time::timeout(STOP_WAIT, handle.stop(true))
        .await
        .is_err()
    {
        warn!(
            service = label,
            wait_secs = STOP_WAIT.as_secs(),
            "[backend] stop did not complete in time; leaving it to actix force shutdown"
        );
    }
}

/// 停止 8888 本地后端。未在运行时空操作。
pub async fn stop_local_backend() {
    let running = local_backend_handle_slot()
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());
    if let Some(running) = running {
        stop_actix(running.handle, "local-backend").await;
        info!("[backend] local service stopped");
    }
}

/// 停止 14219 局域网导入接收。未在运行时空操作。
pub async fn stop_capture_import() {
    let running = capture_import_handle_slot()
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());
    if let Some(running) = running {
        stop_actix(running.handle, "capture-import").await;
        info!("[capture_import] LAN import service stopped");
    }
}

/// 启动 8888 本地后端。**bind 失败立刻返回 Err**（上层据此退避重试）；
/// 绑定成功后立刻返回 Ok，服务本体在自己的后台任务里一直跑到被 `stop_local_backend` 停掉。
///
/// 为什么不在调用方里 `await` 服务本体：`Server` 是一个「服务退出才完成」的 future。
/// 之前直接 `await` 它，会让 `service_start` / `service_restart` 这个 Tauri 命令一直挂到
/// 服务被停为止 —— 前端表现为「重启」无限转圈，且该命令永远拿不到结果再回读状态。
/// 现在把「等结果」的责任交给后台任务，命令只负责「确认已经绑上端口」。
///
/// 这里刻意拆掉了原来的「一次把两个服务都起来并 try_join」的写法：8888 与 14219
/// 现在是两个可各自启停的服务（14219 绑在 0.0.0.0，严格网络管控下用户可能想单独关掉它）。
pub async fn run_local_backend(state: SharedState) -> Result<(), String> {
    // 已经由本进程在跑就不要再建一个：否则第二个实例绑不上 8888，会把「本来就好着」当成失败。
    if local_backend_owned() {
        info!("[backend] local service already owned by this process; skip");
        return Ok(());
    }

    let server = HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "POST"])
            .allow_any_header()
            .max_age(3600);

        App::new()
            .app_data(Data::new(state.clone()))
            .wrap(cors)
            .configure(markdown_image_process::config)
            .configure(browser_preview_config)
            .configure(|cfg| chrome_extension_service::config(cfg, Data::new(state.clone())))
            // MCP 服务端：挂在既有 loopback 端口上，不新开端口、不扩大暴露面。
            // 只会说 stdio 的客户端（如内置 codex）由 guaxi_mcp 桥接进程转发到这里。
            .configure(crate::cognition::mcp::config)
            // 库网站:挂到 loopback,供 Guaxi 内嵌 webview 访问(无 token)。
            // 对外 LAN 访问由带 token 的独立端口提供(见 library_site::config_lan)。
            .configure(crate::commands::library_site::config_local)
            // 音乐播放器音频流代理：给带防盗链的直链补 Referer/UA，透传 Range 供 <audio> 播放与拖动。
            .configure(crate::commands::music::stream::config)
            .route("/api/web-model-chat", web::post().to(web_model_chat_push))
            .route("/api/plugin/llm", web::post().to(plugin_llm_complete))
            .route("/api/plugin/host/context", web::get().to(plugin_host_context))
            .route("/api/notebook/data-lab/health", web::get().to(notebook_data_lab_health))
            .route("/api/notebook/data-lab/action", web::post().to(notebook_data_lab_rpc_http))
            .route("/lsp/{language}", web::get().to(lsp::lsp_ws_route))
            .app_data(JsonConfig::default().limit(10 * 1024 * 1024))
            .app_data(PayloadConfig::default().limit(10 * 1024 * 1024))
    })
    .shutdown_timeout(SHUTDOWN_TIMEOUT_SECS)
    .bind("127.0.0.1:8888")
    .map_err(|error| {
        // 10048（地址已被占用）几乎总是「另一个 Guaxi 实例还开着」：两个实例抢同一个
        // loopback 端口，后起的那个绑不上。把这句话直接写进错误里，用户才知道该关谁。
        format!(
            "绑定 127.0.0.1:8888 失败: {error}。该端口可能被另一个 Guaxi 实例或别的程序占用"
        )
    })?;

    info!("[backend] local service listening on http://127.0.0.1:8888");

    let running = server.run();
    let server_handle = running.handle();
    let alive = spawn_until_stopped(running, "local-backend");
    // 存下停止句柄：「设置 → 服务器」据此停掉/重启本地后端。
    *local_backend_handle_slot().lock().unwrap() = Some(RunningActix {
        handle: server_handle,
        alive,
    });
    Ok(())
}

/// 启动 14219 局域网导入接收。bind 失败立刻返回 Err；绑定成功后立刻返回 Ok，
/// 服务本体在后台任务里运行（理由同 `run_local_backend`）。
pub async fn run_capture_import(state: SharedState) -> Result<(), String> {
    if capture_import_owned() {
        info!("[capture_import] LAN import service already owned by this process; skip");
        return Ok(());
    }
    let running = start_capture_import_lan_service(state)?;
    let server_handle = running.handle();
    let alive = spawn_until_stopped(running, "capture-import");
    *capture_import_handle_slot().lock().unwrap() = Some(RunningActix {
        handle: server_handle,
        alive,
    });
    Ok(())
}

/// 把 actix 服务丢进后台任务，直到它自己退出（被 stop 或出错）。
/// 返回的标记会在任务结束时置 false，状态页据此识别「服务其实已经不在了」。
fn spawn_until_stopped(running: Server, label: &'static str) -> Arc<AtomicBool> {
    let alive = Arc::new(AtomicBool::new(true));
    let flag = alive.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = running.await {
            warn!(service = label, %error, "[backend] service exited with error");
        } else {
            info!(service = label, "[backend] service exited");
        }
        // 服务不再运行时清掉标记，避免后续 stop 对着一个已死的 handle 白等超时。
        flag.store(false, Ordering::SeqCst);
    });
    alive
}

fn capture_import_bind_addr() -> String {
    std::env::var("GUAXI_CAPTURE_BIND")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "0.0.0.0:14219".to_string())
}

/// 局域网导入接收实际监听的端口（可能被 `GUAXI_CAPTURE_BIND` 改写）。
pub fn capture_import_port() -> Option<u16> {
    capture_import_bind_addr()
        .rsplit(':')
        .next()
        .and_then(|port| port.parse().ok())
}

fn start_capture_import_lan_service(state: SharedState) -> Result<Server, String> {
    let bind_addr = capture_import_bind_addr();

    // 设计说明：
    // 主后端仍只监听 127.0.0.1:8888，保证编辑器、Markdown 渲染、WebModel 等内部 API
    // 不被局域网访问。移动端只通过这个窄端口访问 Capture Import，便于手机采集内容
    // 写入文件库，也降低暴露面。端口被占用时返回 Err，由调用方决定重试还是放弃。
    let server = HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["POST", "OPTIONS"])
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .configure(|cfg| {
                chrome_extension_service::capture_import_config(cfg, Data::new(state.clone()))
            })
            .app_data(JsonConfig::default().limit(20 * 1024 * 1024))
            .app_data(PayloadConfig::default().limit(20 * 1024 * 1024))
    })
    .shutdown_timeout(SHUTDOWN_TIMEOUT_SECS)
    .bind(&bind_addr)
    .map_err(|error| {
        warn!(
            "[capture_import] LAN import service bind failed addr={} error={}",
            bind_addr, error
        );
        format!("绑定 {bind_addr} 失败: {error}")
    })?;

    info!(
        "[capture_import] LAN import service listening on http://{}",
        bind_addr
    );
    Ok(server.run())
}
