use crate::commands::settingview::{get_libraries_root};
use actix_web::{post, web, HttpResponse, Responder};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

// 扩展请求数据结构
#[derive(Debug, Deserialize)]
pub struct ExtensionRequest {
    pub url: String,
    pub title: String,
    pub html: String, // 改为 html 字段
    pub timestamp: String,
}

// 响应数据结构
#[derive(Serialize)]
pub struct ExtensionResponse {
    success: bool,
    message: String,
    saved_path: Option<String>,
}

// 配置服务路由
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/extension").service(handle_extension_data));
}

// 处理扩展数据的主端点
#[post("/save")]
async fn handle_extension_data(req: web::Json<ExtensionRequest>) -> impl Responder {
    if req.html.is_empty() {
        return HttpResponse::BadRequest().json(ExtensionResponse {
            success: false,
            message: "Empty html".to_string(),
            saved_path: None,
        });
    }

    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let sanitized_title = sanitize_filename(&req.title);
    let filename = format!("{}_{}.html", sanitized_title, timestamp);

    let save_dir = PathBuf::from(get_libraries_root())
        .join("collect_html");

    if !save_dir.exists() {
        if let Err(e) = fs::create_dir_all(&save_dir) {
            return HttpResponse::InternalServerError().json(ExtensionResponse {
                success: false,
                message: format!("Failed to create directory: {}", e),
                saved_path: None,
            });
        }
    }

    let raw_path = save_dir.join(filename);
    let write_result = File::create(&raw_path).and_then(|mut f| f.write_all(req.html.as_bytes()));

    if let Err(e) = write_result {
        return HttpResponse::InternalServerError().json(ExtensionResponse {
            success: false,
            message: format!("Failed to save html: {}", e),
            saved_path: None,
        });
    }

    HttpResponse::Ok().json(ExtensionResponse {
        success: true,
        message: "HTML saved successfully".to_string(),
        saved_path: Some(raw_path.to_string_lossy().into_owned()),
    })
}

// 辅助函数：清理文件名
fn sanitize_filename(filename: &str) -> String {
    let mut sanitized = String::new();
    let mut last_was_underscore = false;
    for c in filename.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            sanitized.push(c);
            last_was_underscore = false;
        } else if !last_was_underscore {
            sanitized.push('_');
            last_was_underscore = true;
        }
    }
    sanitized.trim_matches('_').to_string()
}
