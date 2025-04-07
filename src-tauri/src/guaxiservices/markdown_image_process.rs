use actix_web::{get, post, web, HttpResponse};
use mime_guess;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use urlencoding::decode;

#[derive(Debug, Serialize, Deserialize)]
pub struct MarkdownRequest {
    content: String,
    base_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageMapping {
    original_path: String,
    served_path: String,
}

#[post("/api/markdown/image_process")]
async fn process_markdown(req: web::Json<MarkdownRequest>) -> HttpResponse {
    let base_path = req
        .base_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_default();
    // info!("Received Markdown content: {}", req.content);
    info!("Base path: {:?}", base_path);
    let mut mappings = Vec::new();
    let mut processed = req.content.clone();

    let re = Regex::new(r#"!\[([^\]]*)\]\(([^)\s]+)(?:\s+"([^"]+)")?\)"#).unwrap();

    for cap in re.captures_iter(&req.content) {
        let img_path = cap.get(2).unwrap().as_str();
        info!("Found image path: {}", img_path);

        if !img_path.starts_with("http") {
            let normalized_path = img_path.replace('\\', "/");
            let joined_path = base_path.join(&normalized_path);
            info!("Joined path: {:?}", joined_path);

            // 尝试规范化路径
            let full_path = joined_path.canonicalize().ok();

            info!("Normalized path: {}", normalized_path);
            info!("Full path: {:?}", full_path);

            if let Some(full_path) = full_path {
                // 转换为适合 URL 的路径格式
                let served_path = format!(
                    "http://localhost:8888/markdown/api/images/{}",
                    full_path
                        .to_string_lossy()
                        .trim_start_matches(r"\\?\")  // 先移除UNC前缀
                        .trim_start_matches('/')     // 再移除残留斜杠
                        .replace('\\', "/")
                );
                info!("Served path: {}", served_path);

                mappings.push(ImageMapping {
                    original_path: img_path.to_string(),
                    served_path: served_path.clone(),
                });
                info!("Mapping: {:?}", mappings.last());

                processed =
                    processed.replace(&format!("]({})", img_path), &format!("]({})", served_path));
                // info!("Processed content: {}", processed);
            } else {
                info!("Failed to resolve path: {}", img_path);
            }
        }
    }

    HttpResponse::Ok().json(serde_json::json!({
        "processed_content": processed,
        "mappings": mappings
    }))
}

#[get("/markdown/api/images/{path:.*}")]
async fn serve_image(path: web::Path<String>) -> HttpResponse {
    match decode(&path) {
        Ok(decoded_path) => {
            let file_path = Path::new(&*decoded_path);
            info!("Decoded path: {:?}", file_path);

            match fs::read(file_path) {
                Ok(content) => HttpResponse::Ok()
                    .content_type(mime_guess::from_path(file_path).first_or_octet_stream())
                    .body(content),
                Err(_) => HttpResponse::NotFound().finish(),
            }
        }
        Err(_) => HttpResponse::BadRequest().body("Invalid path encoding"),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(process_markdown).service(serve_image);
}
