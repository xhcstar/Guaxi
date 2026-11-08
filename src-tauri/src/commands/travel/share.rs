//! 局域网分享：把行程写成一个自包含的移动端 HTML，起一个只读 HTTP 服务。
//!
//! 不复用文件传输助手的分享服务：那个服务是单例，用户可能正拿它分享别的目录。
//! 这里用独立端口 + 随机 token + 只读，2 小时后自动停止，避免忘记关闭。

use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use tauri::AppHandle;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::store;

struct Runtime {
    handle: actix_web::dev::ServerHandle,
    token: String,
    port: u16,
    plan_id: String,
    urls: Vec<String>,
    started_at: String,
}

struct Context {
    root: PathBuf,
    token: String,
}

fn state() -> &'static Mutex<Option<Runtime>> {
    static STATE: OnceLock<Mutex<Option<Runtime>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareStatus {
    pub running: bool,
    pub url: String,
    pub urls: Vec<String>,
    pub port: u16,
    pub plan_id: String,
    pub started_at: String,
}

fn empty_status() -> ShareStatus {
    ShareStatus {
        running: false,
        url: String::new(),
        urls: Vec::new(),
        port: 0,
        plan_id: String::new(),
        started_at: String::new(),
    }
}

fn lan_urls(port: u16, token: &str) -> Vec<String> {
    let mut urls = Vec::new();
    if let Ok(items) = crate::commands::filetransfer::filetransfer_list_local_ips() {
        for item in items {
            if item.is_loopback || item.family != "ipv4" {
                continue;
            }
            urls.push(format!("http://{}:{}/{}/", item.address, port, token));
        }
    }
    if urls.is_empty() {
        urls.push(format!("http://127.0.0.1:{}/{}/", port, token));
    }
    urls
}

async fn stop_locked(runtime: Option<Runtime>) -> Result<(), String> {
    if let Some(runtime) = runtime {
        runtime.handle.stop(true).await;
    }
    Ok(())
}

pub async fn stop() -> Result<(), String> {
    let runtime = {
        let mut guard = state().lock().await;
        guard.take()
    };
    stop_locked(runtime).await
}

pub async fn status() -> ShareStatus {
    let guard = state().lock().await;
    match guard.as_ref() {
        Some(runtime) => ShareStatus {
            running: true,
            url: runtime.urls.first().cloned().unwrap_or_default(),
            urls: runtime.urls.clone(),
            port: runtime.port,
            plan_id: runtime.plan_id.clone(),
            started_at: runtime.started_at.clone(),
        },
        None => empty_status(),
    }
}

async fn serve(req: HttpRequest, context: web::Data<Context>) -> HttpResponse {
    let path = req.path().trim_matches('/');
    let mut parts = path.splitn(2, '/');
    if parts.next().unwrap_or("") != context.token {
        return HttpResponse::NotFound().finish();
    }
    let relative = parts.next().unwrap_or("").trim_start_matches('/');
    let relative = if relative.is_empty() {
        "index.html"
    } else {
        relative
    };
    if relative.contains("..") || relative.contains('/') || relative.contains('\\') {
        return HttpResponse::BadRequest().finish();
    }
    match fs::read(context.root.join(relative)) {
        Ok(bytes) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .insert_header(("Cache-Control", "no-store"))
            .insert_header(("X-Content-Type-Options", "nosniff"))
            .body(bytes),
        Err(_) => HttpResponse::NotFound().body("行程不存在，或分享已经结束。"),
    }
}

pub async fn start(app: &AppHandle, plan_id: &str, html: &str) -> Result<ShareStatus, String> {
    let plan_id = store::sanitize_id(plan_id)?;
    if html.trim().is_empty() {
        return Err("分享页面内容为空".to_string());
    }
    // 同一时间只保留一个旅行分享，避免端口和二维码指向旧计划。
    stop().await?;

    let root = store::root(app)?.join("share").join(&plan_id);
    fs::create_dir_all(&root).map_err(|e| format!("创建分享目录失败: {e}"))?;
    fs::write(root.join("index.html"), html.as_bytes())
        .map_err(|e| format!("写入分享页面失败: {e}"))?;

    let token = Uuid::new_v4().simple().to_string();
    let mut selected = None;
    let mut last_error = String::new();
    for port in 39210u16..39225u16 {
        let context = web::Data::new(Context {
            root: root.clone(),
            token: token.clone(),
        });
        match HttpServer::new(move || {
            App::new()
                .app_data(context.clone())
                .route("/{tail:.*}", web::get().to(serve))
        })
        .bind(("0.0.0.0", port))
        {
            Ok(server) => {
                let server = server.run();
                let handle = server.handle();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = server.await {
                        tracing::warn!("[travel-share] server exited: {error}");
                    }
                });
                selected = Some((port, handle));
                break;
            }
            Err(error) => last_error = error.to_string(),
        }
    }

    let Some((port, handle)) = selected else {
        return Err(format!("局域网分享启动失败，端口被占用：{last_error}"));
    };
    let urls = lan_urls(port, &token);
    let started_at = chrono::Local::now().to_rfc3339();
    let runtime = Runtime {
        handle,
        token: token.clone(),
        port,
        plan_id: plan_id.clone(),
        urls: urls.clone(),
        started_at: started_at.clone(),
    };
    {
        let mut guard = state().lock().await;
        *guard = Some(runtime);
    }

    // 两小时后自动停止；用户也可以随时手动停止。
    let token_for_timer = token.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2 * 3600)).await;
        let runtime = {
            let mut guard = state().lock().await;
            let should_stop = guard
                .as_ref()
                .map(|item| item.token == token_for_timer)
                .unwrap_or(false);
            if should_stop {
                guard.take()
            } else {
                None
            }
        };
        if let Some(runtime) = runtime {
            runtime.handle.stop(true).await;
        }
    });

    Ok(ShareStatus {
        running: true,
        url: urls.first().cloned().unwrap_or_default(),
        urls,
        port,
        plan_id,
        started_at,
    })
}
