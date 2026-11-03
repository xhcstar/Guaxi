use crate::commands::settingview::{get_image_bed_root, get_knowledge_bases_root, get_libraries_root};
use crate::commands::repoview::{ResourceInput, add_resource};
use actix_web::{get, post, web, HttpResponse};
use mime_guess;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tracing::{info, warn};
use urlencoding::{decode, encode};

/// 预处理阶段已经解析成功的图片文件，以及这些图片所在的目录。
///
/// 背景：`serve_image` 原先只放行图床 / 库 / 知识库这三个根目录下的文件，于是从这三个目录
/// 之外打开的笔记（例如 `E:\Work\training\note.md` 里的 `![](./images/a.png)`）虽然能正确
/// 解析出磁盘路径、也能拼出 served_path，但浏览器真去取图时会被 403 挡掉 —— 表现就是
/// "路径明明存在，图片却渲染不出来"。
///
/// 这里不放开整个磁盘，只登记"Markdown 预处理确实解析出来、并且已经写进返回内容里的"图片：
/// - `files`：精确到单个文件，覆盖指向任意位置的绝对路径图片；
/// - `dirs`：这些图片所在的目录。同一篇笔记的插图基本都堆在同一个 images 目录里，登记目录后
///   即使某张图是后来加的、或本轮预处理漏了（前端分片渲染 + 渲染缓存命中时并不会重新请求
///   预处理接口），也不会再出现"同目录下有的图能显示、有的 403"这种随机现象。
struct ResolvedImageRegistry {
    files: HashSet<PathBuf>,
    dirs: HashSet<PathBuf>,
}

fn resolved_image_registry() -> &'static Mutex<ResolvedImageRegistry> {
    static REGISTRY: OnceLock<Mutex<ResolvedImageRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        Mutex::new(ResolvedImageRegistry {
            files: HashSet::new(),
            dirs: HashSet::new(),
        })
    })
}

/// 单次会话里登记的条目上限。超过就整体清空重来：已渲染页面最多触发一次重新预处理，
/// 不会让集合无上限地涨下去。
const RESOLVED_IMAGE_REGISTRY_LIMIT: usize = 20_000;

fn remember_resolved_image(path: &Path) {
    let Ok(mut registry) = resolved_image_registry().lock() else {
        return;
    };
    if registry.files.len() >= RESOLVED_IMAGE_REGISTRY_LIMIT {
        warn!(
            limit = RESOLVED_IMAGE_REGISTRY_LIMIT,
            "resolved image registry is full, clearing"
        );
        registry.files.clear();
        registry.dirs.clear();
    }
    if let Some(dir) = path.parent() {
        registry.dirs.insert(dir.to_path_buf());
    }
    registry.files.insert(path.to_path_buf());
}

/// 返回 (是否命中, 已登记文件数, 已登记目录数)，后两个只用于排查日志。
fn lookup_resolved_image(path: &Path) -> (bool, usize, usize) {
    let Ok(registry) = resolved_image_registry().lock() else {
        return (false, 0, 0);
    };
    let hit = registry.files.contains(path)
        || path
            .parent()
            .map(|dir| registry.dirs.contains(dir))
            .unwrap_or(false);
    (hit, registry.files.len(), registry.dirs.len())
}

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

fn remote_proxy_url(url: &str) -> String {
    format!(
        "http://localhost:8888/markdown/api/remote_image?url={}",
        encode(url)
    )
}

/// Markdown 里的原始 HTML 图片（作者头像、封面在转换产物里常是 `<img>`）。
/// 只改写 http(s) 的 src，其余保持原样；返回改写后的内容与映射。
fn proxy_html_images(content: &str, mappings: &mut Vec<ImageMapping>) -> String {
    let tag_re = Regex::new(r"(?is)<img\b[^>]*>").unwrap();
    let src_re = Regex::new(r#"(?i)\bsrc\s*=\s*(["'])(https?://[^"']+)["']"#).unwrap();
    tag_re
        .replace_all(content, |caps: &regex::Captures| {
            let tag = caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();
            src_re
                .replace_all(&tag, |src_caps: &regex::Captures| {
                    let quote = src_caps.get(1).map(|m| m.as_str()).unwrap_or("\"");
                    let url = src_caps.get(2).map(|m| m.as_str()).unwrap_or("");
                    if url.is_empty() || url.starts_with("http://localhost:8888/") {
                        return src_caps
                            .get(0)
                            .map(|m| m.as_str().to_string())
                            .unwrap_or_default();
                    }
                    let served = remote_proxy_url(url);
                    mappings.push(ImageMapping {
                        original_path: url.to_string(),
                        served_path: served.clone(),
                    });
                    format!("src={quote}{served}{quote}")
                })
                .to_string()
        })
        .to_string()
}

fn process_image_paths(content: &str, base_path: &Path) -> (String, Vec<ImageMapping>) {
    let mut mappings = Vec::new();
    let mut processed = content.to_string();
    let re = Regex::new(r#"!\[([^\]]*)\]\(([^)\s]+)(?:\s+"([^"]+)")?\)"#).unwrap();

    for cap in re.captures_iter(content) {
        let img_path = cap.get(2).unwrap().as_str();
        info!("Found image path: {}", img_path);

        if img_path.starts_with("http://") || img_path.starts_with("https://") {
            // 外链图片交给本地后端代理，不再让 WebView 直连第三方：
            // 1) 避免 WebView2 跟踪防护对第三方图片存储访问刷 Tracking Prevention 警告；
            // 2) 绕开部分站点的防盗链（后端不像浏览器那样带来源页）；
            // 3) 代理侧落盘缓存，同一张图只取一次。
            if img_path.starts_with("http://localhost:8888/") {
                continue;
            }
            let served_path = remote_proxy_url(img_path);
            mappings.push(ImageMapping {
                original_path: img_path.to_string(),
                served_path: served_path.clone(),
            });
            processed = processed.replace(&format!("]({})", img_path), &format!("]({})", served_path));
            continue;
        }

        let normalized_path = img_path.replace('\\', "/");
        let joined_path = base_path.join(&normalized_path);
        info!("Joined path: {:?}", joined_path);

        let full_path = joined_path.canonicalize().ok();
        info!("Normalized path: {}", normalized_path);
        info!("Full path: {:?}", full_path);

        if let Some(full_path) = full_path {
            // 登记这个文件及其所在目录，让 serve_image 放行（详见 ResolvedImageRegistry 的说明）。
            remember_resolved_image(&full_path);
            info!("Registered image for serving: {:?}", full_path);

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

/// 先处理 Markdown 语法里的图片，再处理原始 HTML 的 `<img>`。
/// 两步都只改写外链；本地路径仍走 serve_image 的授权登记。
fn process_all_images(content: &str, base_path: &Path) -> (String, Vec<ImageMapping>) {
    let (markdown_processed, mut mappings) = process_image_paths(content, base_path);
    let processed = proxy_html_images(&markdown_processed, &mut mappings);
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
    let (processed, mappings) = process_all_images(&req.content, &base_path);

    HttpResponse::Ok().json(serde_json::json!({
        "processed_content": processed,
        "mappings": mappings
    }))
}

#[post("/api/markdown/chat_process")]
async fn process_chat_markdown(req: web::Json<MarkdownRequest>) -> HttpResponse {
    let base_path = resolve_base_path(&req);
    let (processed, mappings) = process_all_images(&req.content, &base_path);
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

    // 放行条件二选一：
    // 1. 文件在配置的三个根目录（图床 / 库 / 知识库）之下；
    // 2. 文件是 Markdown 预处理阶段解析出来的图片，或位于这类图片所在的目录（笔记可以放在任意目录）。
    let (allowed_by_registry, known_files, known_dirs) = lookup_resolved_image(&file_canon);

    let mut allowed_roots: Vec<PathBuf> = Vec::new();
    for raw in [get_image_bed_root(), get_libraries_root(), get_knowledge_bases_root()] {
        let p = PathBuf::from(raw);
        if let Ok(canon) = p.canonicalize() {
            allowed_roots.push(canon);
        }
    }

    let allowed_by_root = allowed_roots.iter().any(|r| file_canon.starts_with(r));

    if !allowed_by_registry && !allowed_by_root {
        warn!(
            file = file_canon.to_string_lossy().as_ref(),
            roots = allowed_roots.len(),
            known_files,
            known_dirs,
            "serve_image forbidden: file is neither under an allowed root nor a resolved markdown image"
        );
        return HttpResponse::Forbidden().finish();
    }

    info!(
        file = file_canon.to_string_lossy().as_ref(),
        by_registry = allowed_by_registry,
        by_root = allowed_by_root,
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

// ───────────────────────── 远程图片代理 ─────────────────────────
//
// 只代理图片，且目标不能是回环 / 内网地址：Markdown 来自不可信内容，
// `![](http://127.0.0.1:8080/...)` 不能借代理变成一次 SSRF。
// 缓存落在 exe 同级 cache/remote-images/，按 URL 哈希命名。

const REMOTE_IMAGE_MAX_BYTES: usize = 20 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct RemoteImageQuery {
    url: String,
}

fn remote_image_cache_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.join("cache").join("remote-images");
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn remote_image_ext(content_type: &str) -> Option<&'static str> {
    match content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        "image/avif" => Some("avif"),
        "image/bmp" => Some("bmp"),
        _ => None,
    }
}

fn remote_image_content_type(ext: &str) -> &'static str {
    match ext {
        "jpg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
}

fn ip_is_forbidden(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_multicast()
                || v4.is_broadcast()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
        }
    }
}

fn url_host_is_forbidden(url: &reqwest::Url) -> bool {
    let Some(host) = url.host_str() else {
        return true;
    };
    let host = host.trim_matches(['[', ']']);
    if let Ok(ip) = host.parse::<IpAddr>() {
        return ip_is_forbidden(ip);
    }
    let lower = host.to_ascii_lowercase();
    lower == "localhost" || lower.ends_with(".localhost") || lower.ends_with(".local")
}

async fn ensure_public_host(url: &reqwest::Url) -> Result<(), String> {
    let host = url.host_str().ok_or_else(|| "缺少主机名".to_string())?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "缺少端口".to_string())?;
    let addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| format!("解析域名失败: {e}"))?;
    let mut any = false;
    for addr in addrs {
        any = true;
        if ip_is_forbidden(addr.ip()) {
            return Err("拒绝代理内网或本机地址".to_string());
        }
    }
    if !any {
        return Err("域名没有解析到地址".to_string());
    }
    Ok(())
}

fn remote_image_cache_key(url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn read_cached_remote(url: &str) -> Option<(String, Vec<u8>)> {
    let dir = remote_image_cache_dir()?;
    let key = remote_image_cache_key(url);
    for ext in ["jpg", "png", "gif", "webp", "svg", "avif", "bmp"] {
        let path = dir.join(format!("{key}.{ext}"));
        if let Ok(bytes) = fs::read(&path) {
            return Some((remote_image_content_type(ext).to_string(), bytes));
        }
    }
    None
}

async fn fetch_remote_image(url_str: &str) -> Result<(String, Vec<u8>), String> {
    if let Some(cached) = read_cached_remote(url_str) {
        return Ok(cached);
    }
    let url = reqwest::Url::parse(url_str).map_err(|e| format!("无效的图片地址: {e}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("只代理 http/https 图片".to_string());
    }
    if url_host_is_forbidden(&url) {
        return Err("拒绝代理内网或本机地址".to_string());
    }
    ensure_public_host(&url).await?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 3 || url_host_is_forbidden(attempt.url()) {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "Guaxi/1.0 image proxy")
        .send()
        .await
        .map_err(|e| format!("下载失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("上游返回 {}", resp.status()));
    }
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let ext = remote_image_ext(&content_type)
        .ok_or_else(|| format!("不是图片类型: {content_type}"))?;
    if let Some(len) = resp.content_length() {
        if len as usize > REMOTE_IMAGE_MAX_BYTES {
            return Err("图片过大".to_string());
        }
    }
    let bytes = resp.bytes().await.map_err(|e| format!("读取图片失败: {e}"))?;
    if bytes.len() > REMOTE_IMAGE_MAX_BYTES {
        return Err("图片过大".to_string());
    }

    if let Some(dir) = remote_image_cache_dir() {
        let key = remote_image_cache_key(url_str);
        let path = dir.join(format!("{key}.{ext}"));
        let tmp = dir.join(format!("{key}.{ext}.tmp"));
        if fs::write(&tmp, &bytes).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
    Ok((remote_image_content_type(ext).to_string(), bytes.to_vec()))
}

#[get("/api/markdown/remote_image")]
async fn remote_image(query: web::Query<RemoteImageQuery>) -> HttpResponse {
    match fetch_remote_image(query.url.trim()).await {
        Ok((content_type, bytes)) => HttpResponse::Ok()
            .insert_header(("Content-Type", content_type))
            .insert_header(("Cache-Control", "public, max-age=604800"))
            .body(bytes),
        Err(error) => {
            warn!(url = %query.url, error = %error, "remote image proxy failed");
            HttpResponse::BadGateway()
                .content_type("text/plain; charset=utf-8")
                .body(format!("远程图片代理失败: {error}"))
        }
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(process_markdown)
        .service(process_chat_markdown)
        .service(serve_image)
        .service(serve_bed_image)
        .service(register_resource)
        .service(remote_image);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxies_markdown_remote_images() {
        let (out, mappings) =
            process_image_paths("![](https://example.com/a.png)", Path::new(""));
        assert!(out.contains("/markdown/api/remote_image?url="), "out={out}");
        assert!(out.contains("https%3A%2F%2Fexample.com%2Fa.png"), "out={out}");
        assert_eq!(mappings.len(), 1);
    }

    #[test]
    fn proxies_html_img_src() {
        let mut mappings = Vec::new();
        let out = proxy_html_images(
            r#"<p><img src="https://example.com/a.png" alt="x"></p>"#,
            &mut mappings,
        );
        assert!(out.contains("/markdown/api/remote_image?url="), "out={out}");
        assert!(!out.contains(r#"src="https://example.com"#), "out={out}");
        assert_eq!(mappings.len(), 1);
    }

    #[test]
    fn forbids_private_hosts() {
        for entry in [
            "http://127.0.0.1/x",
            "http://localhost/x",
            "http://192.168.1.2/x",
            "http://10.0.0.1/x",
            "http://[::1]/x",
        ] {
            let url = reqwest::Url::parse(entry).unwrap();
            assert!(url_host_is_forbidden(&url), "{entry} 应被拒绝");
        }
        let public = reqwest::Url::parse("https://example.com/x").unwrap();
        assert!(!url_host_is_forbidden(&public));
    }
}
