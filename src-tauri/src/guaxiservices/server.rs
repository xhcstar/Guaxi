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
use std::sync::{Mutex, OnceLock};
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
    match provider {
        "yuanbao" => Some(7),
        "chatgpt" => Some(8),
        _ => None,
    }
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
static LOCAL_BACKEND_HANDLE: OnceLock<Mutex<Option<ServerHandle>>> = OnceLock::new();
static CAPTURE_IMPORT_HANDLE: OnceLock<Mutex<Option<ServerHandle>>> = OnceLock::new();

fn local_backend_handle_slot() -> &'static Mutex<Option<ServerHandle>> {
    LOCAL_BACKEND_HANDLE.get_or_init(|| Mutex::new(None))
}

fn capture_import_handle_slot() -> &'static Mutex<Option<ServerHandle>> {
    CAPTURE_IMPORT_HANDLE.get_or_init(|| Mutex::new(None))
}

/// 停止 8888 本地后端。未在运行时空操作。
pub async fn stop_local_backend() {
    let handle = local_backend_handle_slot()
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());
    if let Some(handle) = handle {
        // graceful=true：等在途请求收尾，别打断正在进行的取图请求。
        handle.stop(true).await;
        info!("[backend] local service stopped");
    }
}

/// 停止 14219 局域网导入接收。未在运行时空操作。
pub async fn stop_capture_import() {
    let handle = capture_import_handle_slot()
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());
    if let Some(handle) = handle {
        handle.stop(true).await;
        info!("[capture_import] LAN import service stopped");
    }
}

/// 启动 8888 本地后端。**bind 失败立刻返回 Err**（上层据此退避重试）；
/// 成功后一直运行到被 `stop_local_backend` 停掉。
///
/// 这里刻意拆掉了原来的「一次把两个服务都起来并 try_join」的写法：8888 与 14219
/// 现在是两个可各自启停的服务（14219 绑在 0.0.0.0，严格网络管控下用户可能想单独关掉它）。
pub async fn run_local_backend(state: SharedState) -> Result<(), String> {
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
    .bind("127.0.0.1:8888")
    .map_err(|error| format!("绑定 127.0.0.1:8888 失败: {error}"))?;

    info!("[backend] local service listening on http://127.0.0.1:8888");

    let running = server.run();
    // 存下停止句柄：「设置 → 服务器」据此停掉/重启本地后端。
    *local_backend_handle_slot().lock().unwrap() = Some(running.handle());
    running.await.map_err(|error| error.to_string())
}

/// 启动 14219 局域网导入接收。bind 失败立刻返回 Err。
pub async fn run_capture_import(state: SharedState) -> Result<(), String> {
    let running = start_capture_import_lan_service(state)?;
    running.await.map_err(|error| error.to_string())
}

fn capture_import_bind_addr() -> String {
    std::env::var("GUAXI_CAPTURE_BIND")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "0.0.0.0:14219".to_string())
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
    let running = server.run();
    *capture_import_handle_slot().lock().unwrap() = Some(running.handle());
    Ok(running)
}
