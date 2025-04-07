use actix_web::{get, post, web, HttpResponse};
use regex::Regex;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use std::fs;
use urlencoding::decode;
use mime_guess;

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

#[post("/api/markdown/process")]
async fn process_markdown(req: web::Json<MarkdownRequest>) -> HttpResponse {
    let base_path = req.base_path.as_ref().map(PathBuf::from).unwrap_or_default();
    
    let mut mappings = Vec::new();
    let mut processed = req.content.clone();
    
    let re = Regex::new(r#"!\[([^\]]*)\]\(([^)\s]+)(?:\s+"([^"]+)")?\)"#).unwrap();
    
    for cap in re.captures_iter(&req.content) {
        let img_path = cap.get(2).unwrap().as_str();
        
        if !img_path.starts_with("http") && !img_path.starts_with('/') {
            let normalized_path = img_path.replace('\\', "/");
            let full_path = base_path.join(&normalized_path).canonicalize().ok();
            
            if let Some(full_path) = full_path {
                let served_path = format!("/images/{}", full_path.display());
                
                mappings.push(ImageMapping {
                    original_path: img_path.to_string(),
                    served_path: served_path.clone(),
                });
                
                processed = processed.replace(
                    &format!("]({})", img_path),
                    &format!("]({})", served_path)
                );
            }
        }
    }
    
    HttpResponse::Ok().json(serde_json::json!({
        "processed_content": processed,
        "mappings": mappings
    }))
}

#[get("/images/{path:.*}")]
async fn serve_image(path: web::Path<String>) -> HttpResponse {
    match decode(&path) {
        Ok(decoded_path) => {
            let file_path = Path::new(&*decoded_path);
            
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
    cfg.service(process_markdown)
       .service(serve_image);
}