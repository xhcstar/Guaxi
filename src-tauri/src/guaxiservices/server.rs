use crate::guaxiservices::chrome_extension_service;
use crate::guaxiservices::lsp;
use crate::guaxiservices::markdown_browser::browser_preview_config;
use crate::guaxiservices::markdown_image_process;
use crate::commands::notebook_lab::{
    dispatch_notebook_data_lab_rpc, notebook_data_lab_health_response,
    NotebookDataLabRpcRequest,
};
use crate::state::SharedState;
use actix_cors::Cors;
use actix_web::web::{Data, JsonConfig, PayloadConfig};
use actix_web::{dev::Server, web, App, HttpResponse, HttpServer};
use futures_util::future;
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

pub async fn start_backend_services(state: SharedState) -> std::io::Result<()> {
    let capture_import_server = start_capture_import_lan_service(state.clone());

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
            .route("/api/notebook/data-lab/health", web::get().to(notebook_data_lab_health))
            .route("/api/notebook/data-lab/action", web::post().to(notebook_data_lab_rpc_http))
            .route("/lsp/{language}", web::get().to(lsp::lsp_ws_route))
            .app_data(JsonConfig::default().limit(10 * 1024 * 1024))
            .app_data(PayloadConfig::default().limit(10 * 1024 * 1024))
    })
    .bind("127.0.0.1:8888")?;

    info!("[backend] local service listening on http://127.0.0.1:8888");

    let local_server = server.run();
    if let Some(capture_import_server) = capture_import_server {
        future::try_join(local_server, capture_import_server)
            .await
            .map(|_| ())
    } else {
        local_server.await
    }
}

fn capture_import_bind_addr() -> String {
    std::env::var("GUAXI_CAPTURE_BIND")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "0.0.0.0:14219".to_string())
}

fn start_capture_import_lan_service(state: SharedState) -> Option<Server> {
    let bind_addr = capture_import_bind_addr();

    // 设计说明：
    // 主后端仍只监听 127.0.0.1:8888，保证编辑器、Markdown 渲染、WebModel 等内部 API
    // 不被局域网访问。移动端只通过这个窄端口访问 Capture Import，便于手机采集内容
    // 写入文件库，也降低暴露面。端口被占用时只记录日志，不阻断 PC 主程序启动。
    match HttpServer::new(move || {
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
    {
        Ok(server) => {
            info!(
                "[capture_import] LAN import service listening on http://{}",
                bind_addr
            );
            Some(server.run())
        }
        Err(error) => {
            warn!(
                "[capture_import] LAN import service bind failed addr={} error={}",
                bind_addr, error
            );
            None
        }
    }
}
