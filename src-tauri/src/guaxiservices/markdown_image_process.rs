use crate::commands::settingview::{get_image_bed_root, get_knowledge_bases_root, get_libraries_root};
use crate::commands::repoview::{ResourceInput, add_resource};
use actix_web::{get, post, web, HttpResponse};
use mime_guess;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use urlencoding::{decode, encode};

fn normalize_list_display_math(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut in_fence = false;
    let mut fence_marker: Option<char> = None;
    let mut i = 0usize;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

        let fence = if trimmed.starts_with("```") {
            Some('`')
        } else if trimmed.starts_with("~~~") {
            Some('~')
        } else {
            None
        };

        if let Some(marker) = fence {
            if !in_fence {
                in_fence = true;
                fence_marker = Some(marker);
            } else if fence_marker == Some(marker) {
                in_fence = false;
                fence_marker = None;
            }
            out.push(line.to_string());
            i += 1;
            continue;
        }

        if in_fence {
            out.push(line.to_string());
            i += 1;
            continue;
        }

        let open_is_math = line.trim() == "$$";
        if !open_is_math {
            out.push(line.to_string());
            i += 1;
            continue;
        }

        let mut close_at: Option<usize> = None;
        let mut j = i + 1;
        while j < lines.len() {
            let inner = lines[j];
            let inner_trim = inner.trim_start();
            if inner.trim() == "$$" {
                close_at = Some(j);
                break;
            }
            if inner_trim.starts_with("```") || inner_trim.starts_with("~~~") {
                break;
            }
            j += 1;
        }

        let Some(close_idx) = close_at else {
            out.push(line.to_string());
            i += 1;
            continue;
        };

        let prev_non_empty = (0..i)
            .rev()
            .find_map(|k| {
                let s = lines[k];
                if s.trim().is_empty() {
                    None
                } else {
                    Some(s)
                }
            })
            .unwrap_or("");

        let indent_len = line.len() - line.trim_start().len();
        let list_context = {
            let prev_trim = prev_non_empty.trim_start();
            let is_list_marker = prev_trim.starts_with("- ")
                || prev_trim.starts_with("* ")
                || prev_trim.starts_with("+ ")
                || prev_trim
                    .split_once('.')
                    .map(|(left, right)| {
                        !left.is_empty()
                            && left.chars().all(|c| c.is_ascii_digit())
                            && right.starts_with(' ')
                    })
                    .unwrap_or(false);
            is_list_marker || prev_non_empty.starts_with(' ') || indent_len >= 2
        };

        if !list_context {
            out.push(line.to_string());
            i += 1;
            continue;
        }

        let expr = lines[(i + 1)..close_idx]
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        if expr.is_empty() {
            out.push(line.to_string());
            out.push(lines[close_idx].to_string());
            i = close_idx + 1;
            continue;
        }

        let indent = " ".repeat(indent_len);
        out.push(format!("{indent}$$ {expr} $$"));
        i = close_idx + 1;
    }

    out.join("\n")
}

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

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceRegisterRequest {
    resource_type: Option<String>,
    file_name: Option<String>,
    relative_path: Option<String>,
    absolute_path: Option<String>,
    owner_file_path: String,
    created_at: Option<String>,
    extra: Option<serde_json::Value>,
}

fn process_image_paths(content: &str, base_path: &Path) -> (String, Vec<ImageMapping>) {
    let mut mappings = Vec::new();
    let mut processed = content.to_string();
    let re = Regex::new(r#"!\[([^\]]*)\]\(([^)\s]+)(?:\s+"([^"]+)")?\)"#).unwrap();

    for cap in re.captures_iter(content) {
        let img_path = cap.get(2).unwrap().as_str();
        info!("Found image path: {}", img_path);

        if img_path.starts_with("http") {
            continue;
        }

        let normalized_path = img_path.replace('\\', "/");
        let joined_path = base_path.join(&normalized_path);
        info!("Joined path: {:?}", joined_path);

        let full_path = joined_path.canonicalize().ok();
        info!("Normalized path: {}", normalized_path);
        info!("Full path: {:?}", full_path);

        if let Some(full_path) = full_path {
            let abs = full_path
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .trim_start_matches('/')
                .replace('\\', "/")
                .to_string();
            let served_path = format!("http://localhost:8888/markdown/api/images/{}", encode(&abs));
            info!("Served path: {}", served_path);

            mappings.push(ImageMapping {
                original_path: img_path.to_string(),
                served_path: served_path.clone(),
            });
            info!("Mapping: {:?}", mappings.last());

            processed = processed.replace(&format!("]({})", img_path), &format!("]({})", served_path));
        } else {
            info!("Failed to resolve path: {}", img_path);
        }
    }

    (processed, mappings)
}

fn resolve_base_path(req: &MarkdownRequest) -> PathBuf {
    req.base_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_default()
}

#[post("/api/markdown/image_process")]
async fn process_markdown(req: web::Json<MarkdownRequest>) -> HttpResponse {
    let base_path = resolve_base_path(&req);
    let (processed, mappings) = process_image_paths(&req.content, &base_path);

    HttpResponse::Ok().json(serde_json::json!({
        "processed_content": processed,
        "mappings": mappings
    }))
}

#[post("/api/markdown/chat_process")]
async fn process_chat_markdown(req: web::Json<MarkdownRequest>) -> HttpResponse {
    let base_path = resolve_base_path(&req);
    let (processed, mappings) = process_image_paths(&req.content, &base_path);
    let processed = normalize_list_display_math(&processed);

    HttpResponse::Ok().json(serde_json::json!({
        "processed_content": processed,
        "mappings": mappings
    }))
}

#[get("/markdown/api/images/{path:.*}")]
async fn serve_image(path: web::Path<String>) -> HttpResponse {
    let decoded = match decode(&path) {
        Ok(v) => v.into_owned(),
        Err(_) => return HttpResponse::BadRequest().body("Invalid path encoding"),
    };

    let decoded = decoded.trim_start_matches(r"\\?\").to_string();
    let candidate = PathBuf::from(decoded);

    let file_canon = match candidate.canonicalize() {
        Ok(p) => p,
        Err(_) => return HttpResponse::NotFound().finish(),
    };

    if !file_canon.is_file() {
        return HttpResponse::NotFound().finish();
    }

    let mut allowed_roots: Vec<PathBuf> = Vec::new();
    for raw in [get_image_bed_root(), get_libraries_root(), get_knowledge_bases_root()] {
        let p = PathBuf::from(raw);
        if let Ok(canon) = p.canonicalize() {
            allowed_roots.push(canon);
        }
    }

    if allowed_roots.is_empty() {
        warn!(
            file = file_canon.to_string_lossy().as_ref(),
            "serve_image forbidden: no allowed roots"
        );
        return HttpResponse::Forbidden().finish();
    }

    let allowed = allowed_roots.iter().any(|r| file_canon.starts_with(r));
    if !allowed {
        warn!(
            file = file_canon.to_string_lossy().as_ref(),
            "serve_image forbidden: file is outside allowed roots"
        );
        return HttpResponse::Forbidden().finish();
    }

    info!(
        file = file_canon.to_string_lossy().as_ref(),
        "serve_image allowed"
    );

    match fs::read(&file_canon) {
        Ok(content) => HttpResponse::Ok()
            .content_type(mime_guess::from_path(&file_canon).first_or_octet_stream())
            .body(content),
        Err(_) => HttpResponse::NotFound().finish(),
    }
}

#[get("/markdown/api/bed/{rel:.*}")]
async fn serve_bed_image(rel: web::Path<String>) -> HttpResponse {
    let root = PathBuf::from(get_image_bed_root());
    let rel_raw = rel.into_inner();

    let decoded_rel = match decode(&rel_raw) {
        Ok(v) => v.into_owned(),
        Err(_) => return HttpResponse::BadRequest().body("Invalid path encoding"),
    };

    let safe_rel = decoded_rel.replace('\\', "/");
    let candidate = root.join(&safe_rel);

    let root_canon = match root.canonicalize() {
        Ok(p) => p,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    let file_canon = match candidate.canonicalize() {
        Ok(p) => p,
        Err(_) => return HttpResponse::NotFound().finish(),
    };

    if !file_canon.starts_with(&root_canon) {
        return HttpResponse::Forbidden().finish();
    }

    match fs::read(&file_canon) {
        Ok(content) => HttpResponse::Ok()
            .content_type(mime_guess::from_path(&file_canon).first_or_octet_stream())
            .body(content),
        Err(_) => HttpResponse::NotFound().finish(),
    }
}

#[post("/markdown/api/resources")]
async fn register_resource(req: web::Json<ResourceRegisterRequest>) -> HttpResponse {
    let root = PathBuf::from(get_image_bed_root());

    let root_canon = match root.canonicalize() {
        Ok(p) => p,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let (relative_path, file_canon) = if let Some(rel) = req.relative_path.as_ref() {
        let rel = rel.replace('\\', "/");
        let candidate = root.join(&rel);
        let file_canon = match candidate.canonicalize() {
            Ok(p) => p,
            Err(_) => return HttpResponse::BadRequest().body("Invalid resource path"),
        };
        (rel, file_canon)
    } else if let Some(abs) = req.absolute_path.as_ref() {
        let candidate = PathBuf::from(abs);
        let file_canon = match candidate.canonicalize() {
            Ok(p) => p,
            Err(_) => return HttpResponse::BadRequest().body("Invalid resource path"),
        };

        let rel = match file_canon.strip_prefix(&root_canon) {
            Ok(p) => p.to_string_lossy().replace('\\', "/"),
            Err(_) => return HttpResponse::Forbidden().finish(),
        };

        (rel, file_canon)
    } else {
        return HttpResponse::BadRequest().body("Missing relative_path or absolute_path");
    };

    if !file_canon.starts_with(&root_canon) {
        return HttpResponse::Forbidden().finish();
    }

    let absolute_path = file_canon.to_string_lossy().to_string();

    let file_name = if let Some(v) = req.file_name.as_ref() {
        v.clone()
    } else {
        match file_canon.file_name().and_then(|n| n.to_str()) {
            Some(v) => v.to_string(),
            None => return HttpResponse::BadRequest().body("Invalid file name"),
        }
    };

    let resource_type = if let Some(v) = req.resource_type.as_ref() {
        v.clone()
    } else {
        match Path::new(&file_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .as_deref()
        {
            Some("png") | Some("jpg") | Some("jpeg") | Some("gif") | Some("webp") | Some("bmp") | Some("svg") => {
                "image".to_string()
            }
            Some("mp4") | Some("mov") | Some("avi") | Some("mkv") | Some("webm") => "video".to_string(),
            Some("pdf")
            | Some("doc")
            | Some("docx")
            | Some("xls")
            | Some("xlsx")
            | Some("ppt")
            | Some("pptx")
            | Some("txt")
            | Some("md")
            | Some("log") => "document".to_string(),
            Some("exe") | Some("dll") | Some("bin") | Some("so") | Some("dylib") => "binary".to_string(),
            _ => "other".to_string(),
        }
    };

    let created_at = req.created_at.clone().unwrap_or_default();
    let extra_str = req.extra.as_ref().map(|v| v.to_string());

    let input = ResourceInput::new(
        resource_type,
        file_name,
        relative_path,
        absolute_path,
        req.owner_file_path.clone(),
        created_at,
        extra_str,
    );

    match add_resource(input) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({ "success": true })),
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({ "success": false, "error": e })),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(process_markdown)
        .service(process_chat_markdown)
        .service(serve_image)
        .service(serve_bed_image)
        .service(register_resource);
}
