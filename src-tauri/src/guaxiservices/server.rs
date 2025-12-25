use crate::guaxiservices::chrome_extension_service;
use crate::guaxiservices::lsp;
use crate::guaxiservices::markdown_browser::browser_preview_config;
use crate::guaxiservices::markdown_image_process;
use crate::state::SharedState;
use actix_cors::Cors;
use actix_web::web::{Data, JsonConfig, PayloadConfig};
use actix_web::{web, App, HttpResponse, HttpServer};
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

pub async fn start_backend_services(state: SharedState) -> std::io::Result<()> {
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
            .route("/api/web-model-chat", web::post().to(web_model_chat_push))
            .route("/lsp/{language}", web::get().to(lsp::lsp_ws_route))
            .app_data(JsonConfig::default().limit(10 * 1024 * 1024))
            .app_data(PayloadConfig::default().limit(10 * 1024 * 1024))
    })
    .bind("127.0.0.1:8888")?;

    server.run().await
}
