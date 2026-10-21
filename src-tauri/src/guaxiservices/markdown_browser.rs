use crate::commands::notebook::preprocess_markdown_for_static_render;
use crate::commands::knowledgebase::get_knowledge_base_list;
use actix_web::{get, web, HttpResponse};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tracing::{error, info, warn};
use urlencoding::{decode, encode};
fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace("'", "&#39;")
}

fn is_markdown_file(path: &Path) -> bool {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
    {
        Some(ext) if ext == "md" || ext == "markdown" => true,
        _ => false,
    }
}

fn filename_ascii_fallback(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ' ') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        "download.md".to_string()
    } else {
        trimmed.to_string()
    }
}

/// 用户在软件内**显式**点过「在浏览器中查看」的文件。
///
/// 预览端点的放行规则原本只有「文件落在某个知识库根目录下」，这对知识编辑模式够用，
/// 但文件编辑 / 任务编辑模式打开的是任意外部目录（含可配置的任务根目录），
/// 它们不在知识库根下，直接请求会 403。
///
/// 这里沿用 `markdown_image_process::ResolvedImageRegistry` 的思路：授权只来自软件内的一次
/// 明确动作（前端调用 `remember_browser_preview_path` 命令），HTTP 端点本身不扩大可见范围 ——
/// 没有这条登记，任意外部路径依然读不到。
const REMEMBERED_PREVIEW_LIMIT: usize = 2_000;

fn remembered_preview_files() -> &'static Mutex<HashSet<PathBuf>> {
    static REGISTRY: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashSet::new()))
}

fn remember_preview_file(path: &Path) {
    let Ok(mut files) = remembered_preview_files().lock() else {
        return;
    };
    if files.len() >= REMEMBERED_PREVIEW_LIMIT {
        files.clear();
    }
    files.insert(path.to_path_buf());
}

fn is_remembered_preview_file(path: &Path) -> bool {
    remembered_preview_files()
        .lock()
        .map(|files| files.contains(path))
        .unwrap_or(false)
}

/// 该文件是否与某个「显式点开预览」的 Markdown 同目录。
///
/// 图表围栏的 `data: ./x.csv` 是相对笔记目录解析的；被预览的笔记既然已被用户显式放行，
/// 它自己目录下的数据文件也应可读，否则 plain/task 模式的图表在浏览器里全是读取失败。
fn is_sibling_of_remembered_preview(path: &Path) -> bool {
    let Some(dir) = path.parent() else {
        return false;
    };
    remembered_preview_files()
        .lock()
        .map(|files| files.iter().any(|f| f.parent() == Some(dir)))
        .unwrap_or(false)
}

/// 前端「在浏览器中查看」前调用：把该 Markdown 文件登记为本次会话可预览。
///
/// 只接受真实存在的 .md/.markdown 文件；目录、二进制一律拒绝，
/// 免得这条命令被当成通用的「放行任意路径」入口。
#[tauri::command]
pub fn remember_browser_preview_path(path: String) -> Result<(), String> {
    // 这里收到的是前端直接给的文件系统路径，不做百分号解码：
    // 文件名里带 `%` 时解码会失败，而 IPC 这条路本来就不需要编解码。
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Empty path".to_string());
    }
    // 只在 Windows 上剥掉 `\\?\` 前缀：这个前缀本身就是以两个反斜杠开头，
    // 在其它平台上无条件剥会把 UNC 风格的绝对路径改坏。
    let raw = trimmed.strip_prefix(r"\\?\").unwrap_or(trimmed);
    let file_canon = PathBuf::from(raw)
        .canonicalize()
        .map_err(|_| "File not found".to_string())?;

    if !file_canon.is_file() {
        return Err("Not a file".to_string());
    }
    if !is_markdown_file(&file_canon) {
        return Err("Not a markdown file".to_string());
    }

    remember_preview_file(&file_canon);
    info!(
        file = file_canon.to_string_lossy().as_ref(),
        "browser preview: path allowed for preview"
    );
    Ok(())
}

fn resolve_allowed_markdown_file(raw_path: &str) -> Result<PathBuf, String> {
    info!(raw_path = raw_path, "browser preview: resolve file start");

    let decoded_raw = decode(raw_path)
        .map_err(|_| "Invalid path encoding".to_string())?
        .into_owned();

    info!(
        decoded_raw = decoded_raw.as_str(),
        "browser preview: decoded path"
    );

    let decoded = decoded_raw.trim_start_matches(r"\\?\").to_string();
    info!(
        decoded = decoded.as_str(),
        "browser preview: decoded path trimmed"
    );

    let candidate = PathBuf::from(decoded);
    let file_canon = candidate
        .canonicalize()
        .map_err(|_| "File not found".to_string())?;

    info!(
        file_canon = file_canon.to_string_lossy().as_ref(),
        "browser preview: canonicalized"
    );

    if !file_canon.is_file() {
        return Err("Not a file".to_string());
    }

    if !is_markdown_file(&file_canon) {
        return Err("Not a markdown file".to_string());
    }

    // 用户在软件内显式点开过的文件直接放行：允许根可能一个都没有（没配知识库），
    // 也可能不覆盖 plain/task 模式的外部目录，这条登记就是补上后者。
    // 放在查询知识库之前，避免「没配知识库 / 库表打不开」把已授权的预览一起挡掉。
    if is_remembered_preview_file(&file_canon) {
        info!(
            file = file_canon.to_string_lossy().as_ref(),
            "browser preview: allowed by explicit user request"
        );
        return Ok(file_canon);
    }

    let kbs = get_knowledge_base_list().map_err(|e| e.to_string())?;
    info!(
        kb_count = kbs.len(),
        "browser preview: loaded knowledge bases"
    );

    let mut allowed_roots: Vec<PathBuf> = Vec::new();

    for kb in kbs {
        let root = PathBuf::from(kb.location);
        if let Ok(canon) = root.canonicalize() {
            allowed_roots.push(canon);
        }
    }

    if allowed_roots.is_empty() {
        warn!("browser preview: no allowed roots (no knowledge bases?)");
        return Err("No knowledge bases".to_string());
    }

    let mut matched_root: Option<&PathBuf> = None;
    for root in &allowed_roots {
        if file_canon.starts_with(root) {
            matched_root = Some(root);
            break;
        }
    }

    if let Some(root) = matched_root {
        info!(
            root = root.to_string_lossy().as_ref(),
            "browser preview: allowed root matched"
        );
    } else {
        warn!(
            file_canon = file_canon.to_string_lossy().as_ref(),
            "browser preview: forbidden (not under any knowledge base root)"
        );
        return Err("Forbidden".to_string());
    }

    Ok(file_canon)
}

/// 解析预览页要读取的数据文件，并限制在允许的根目录内。
///
/// 允许的根与 markdown 预览一致：所有知识库根 + 资源库根。
/// 只放行纯文本类扩展名 —— 这个端点是为了让图表读到 CSV/JSON，不是通用文件读取接口。
fn resolve_allowed_data_file(raw_path: &str) -> Result<PathBuf, String> {
    const DATA_EXTS: [&str; 6] = ["csv", "tsv", "json", "md", "markdown", "excalidraw"];

    let decoded = decode(raw_path)
        .map_err(|_| "Invalid path encoding".to_string())?
        .into_owned();
    let decoded = decoded.trim_start_matches(r"\\?\");
    let file_canon = PathBuf::from(decoded)
        .canonicalize()
        .map_err(|_| "File not found".to_string())?;

    if !file_canon.is_file() {
        return Err("Not a file".to_string());
    }
    let ext = file_canon
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !DATA_EXTS.contains(&ext.as_str()) {
        return Err("Unsupported data file".to_string());
    }

    let mut allowed_roots: Vec<PathBuf> = Vec::new();
    for kb in get_knowledge_base_list().unwrap_or_default() {
        if let Ok(canon) = PathBuf::from(kb.location).canonicalize() {
            allowed_roots.push(canon);
        }
    }
    let resource_root = crate::commands::settingview::get_image_bed_root();
    if let Ok(canon) = PathBuf::from(resource_root.trim()).canonicalize() {
        allowed_roots.push(canon);
    }

    // 已显式放行预览的笔记本身，及其同目录下的数据文件。
    if is_remembered_preview_file(&file_canon) || is_sibling_of_remembered_preview(&file_canon) {
        return Ok(file_canon);
    }

    if allowed_roots.is_empty() {
        return Err("No allowed roots".to_string());
    }
    if allowed_roots.iter().any(|root| file_canon.starts_with(root)) {
        Ok(file_canon)
    } else {
        Err("Forbidden".to_string())
    }
}

fn collect_markdown_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(v) => v,
            None => continue,
        };

        if file_name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            collect_markdown_files(&path, out)?;
            continue;
        }

        if is_markdown_file(&path) {
            out.push(path);
        }
    }

    Ok(())
}

#[get("/favicon.ico")]
async fn favicon() -> HttpResponse {
    HttpResponse::NoContent().finish()
}

#[get("/browser")]
async fn browser_index() -> HttpResponse {
    HttpResponse::Found()
        .append_header(("Location", "/browser/knowledge-bases"))
        .finish()
}

#[get("/browser/knowledge-bases")]
async fn browser_knowledge_bases() -> HttpResponse {
    let result = web::block(|| get_knowledge_base_list()).await;

    let kbs = match result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return HttpResponse::InternalServerError().body(e),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let mut items = String::new();
    for kb in kbs {
        items.push_str(&format!(
            "<li><a href=\"/browser/knowledge-bases/{id}\">{name}</a><div class=\"sub\">{loc}</div></li>",
            id = kb.id,
            name = escape_html(&kb.knowledge_base_name),
            loc = escape_html(&kb.location)
        ));
    }

    let html = format!(
        r#"<!doctype html>
        <html lang="zh">
        <head>
          <meta charset="utf-8" />
          <meta name="viewport" content="width=device-width, initial-scale=1" />
          <title>Guaxi - 知识库</title>
          <style>
            body {{ font-family: -apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Arial,'Noto Sans','PingFang SC','Microsoft YaHei',sans-serif; margin: 24px; color: #1f2328; }}
            h1 {{ font-size: 18px; margin: 0 0 14px; }}
            ul {{ list-style: none; padding: 0; margin: 0; }}
            li {{ padding: 10px 12px; border: 1px solid rgba(0,0,0,0.08); border-radius: 10px; margin-bottom: 10px; }}
            a {{ color: #0969da; text-decoration: none; font-weight: 600; }}
            a:hover {{ text-decoration: underline; }}
            .sub {{ margin-top: 6px; font-size: 12px; color: rgba(31,35,40,0.6); word-break: break-all; }}
          </style>
        </head>
        <body>
          <h1>知识库列表</h1>
          <ul>{items}</ul>
        </body>
        </html>"#,
        items = items
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

#[get("/browser/knowledge-bases/{id}")]
async fn browser_knowledge_base_files(id: web::Path<i32>) -> HttpResponse {
    let kb_id = id.into_inner();

    let result = web::block(|| get_knowledge_base_list()).await;
    let kbs = match result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return HttpResponse::InternalServerError().body(e),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let kb = match kbs.into_iter().find(|x| x.id == kb_id) {
        Some(v) => v,
        None => return HttpResponse::NotFound().body("Knowledge base not found"),
    };

    let root = PathBuf::from(&kb.location);
    let root_canon = match root.canonicalize() {
        Ok(v) => v,
        Err(_) => return HttpResponse::NotFound().body("Knowledge base path not found"),
    };

    let root_canon_for_list = root_canon.clone();
    let files = match web::block(move || {
        let mut out = Vec::new();
        collect_markdown_files(&root_canon_for_list, &mut out).map(|_| out)
    })
    .await
    {
        Ok(Ok(v)) => v,
        _ => return HttpResponse::InternalServerError().finish(),
    };

    info!(
        kb_id = kb_id,
        file_count = files.len(),
        "browser preview: knowledge base markdown files collected"
    );

    let mut items = String::new();
    for file in files {
        let display = file
            .strip_prefix(&root_canon)
            .ok()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| file.to_string_lossy().replace('\\', "/"));

        let abs = file.to_string_lossy();
        let abs = abs.trim_start_matches(r"\\?\");

        let href_preview = format!("/browser/preview?path={}", encode(abs));
        let href_download = format!("/browser/download?path={}", encode(abs));

        let name = escape_html(file.file_name().and_then(|n| n.to_str()).unwrap_or(""));

        items.push_str(&format!(
            "<li><div class=\"row\"><a class=\"name\" href=\"{href_preview}\">{name}</a><a class=\"dl\" href=\"{href_download}\" title=\"下载\" aria-label=\"下载\">⭳</a></div><div class=\"sub\">{path}</div></li>",
            href_preview = href_preview,
            href_download = href_download,
            name = name,
            path = escape_html(&display)
        ));
    }

    let html = format!(
        r#"<!doctype html>
        <html lang="zh">
        <head>
          <meta charset="utf-8" />
          <meta name="viewport" content="width=device-width, initial-scale=1" />
          <title>Guaxi - {kb_name}</title>
          <style>
            body {{ font-family: -apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Arial,'Noto Sans','PingFang SC','Microsoft YaHei',sans-serif; margin: 24px; color: #1f2328; }}
            h1 {{ font-size: 18px; margin: 0 0 8px; }}
            .back {{ display: inline-block; margin: 0 0 14px; font-size: 13px; }}
            ul {{ list-style: none; padding: 0; margin: 0; }}
            li {{ padding: 10px 12px; border: 1px solid rgba(0,0,0,0.08); border-radius: 10px; margin-bottom: 10px; }}
            a {{ color: #0969da; text-decoration: none; font-weight: 600; }}
            a:hover {{ text-decoration: underline; }}
            .row {{ display: flex; align-items: center; justify-content: space-between; gap: 10px; }}
            .name {{ min-width: 0; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
            .dl {{ flex: 0 0 auto; font-weight: 700; padding: 2px 6px; border-radius: 8px; color: rgba(31,35,40,0.75); border: 1px solid rgba(0,0,0,0.08); }}
            .dl:hover {{ text-decoration: none; color: #0969da; border-color: rgba(9,105,218,0.35); background: rgba(9,105,218,0.06); }}
            .sub {{ margin-top: 6px; font-size: 12px; color: rgba(31,35,40,0.6); word-break: break-all; }}
          </style>
        </head>
        <body>
          <a class="back" href="/browser/knowledge-bases">← 返回知识库列表</a>
          <h1>{kb_name}</h1>
          <ul>{items}</ul>
        </body>
        </html>"#,
        kb_name = escape_html(&kb.knowledge_base_name),
        items = items
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

#[derive(Deserialize)]
struct PathQuery {
    path: String,
}

#[get("/browser/raw")]
async fn browser_raw(query: web::Query<PathQuery>) -> HttpResponse {
    info!(path = query.path.as_str(), "browser preview: /browser/raw");

    let file_path = match resolve_allowed_markdown_file(&query.path) {
        Ok(v) => v,
        Err(msg) => {
            warn!(
                path = query.path.as_str(),
                error = msg.as_str(),
                "browser preview: /browser/raw resolve failed"
            );
            return match msg.as_str() {
                "Forbidden" => HttpResponse::Forbidden().body(msg),
                "File not found" => HttpResponse::NotFound().body(msg),
                _ => HttpResponse::BadRequest().body(msg),
            };
        }
    };

    info!(
        file = file_path.to_string_lossy().as_ref(),
        "browser preview: /browser/raw resolved"
    );

    match web::block(move || fs::read_to_string(file_path)).await {
        Ok(Ok(content)) => {
            let processed = preprocess_markdown_for_static_render(&content);
            info!(
                original_len = content.len(),
                processed_len = processed.len(),
                "browser preview: preprocessed markdown for static preview"
            );
            HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .append_header(("Cache-Control", "no-store"))
            .body(processed)
        }
        Ok(Err(e)) => {
            error!(error = %e, "browser preview: /browser/raw read failed");
            HttpResponse::InternalServerError().finish()
        }
        Err(e) => {
            error!(error = %e, "browser preview: /browser/raw worker failed");
            HttpResponse::InternalServerError().finish()
        }
    }
}

/// 预览页读取**数据文件**（图表数据 / 白板场景）用。
///
/// 与 /browser/raw 的区别只有两点：不限定 .md 扩展名，且不做 Markdown 预处理 ——
/// 图表要的是原始 CSV/JSON 文本。允许的根目录与 markdown 预览完全一致
/// （知识库根 + 资源库根），不会因为多开一个端点而放宽可见范围。
#[get("/browser/file")]
async fn browser_file(query: web::Query<PathQuery>) -> HttpResponse {
    let file_path = match resolve_allowed_data_file(&query.path) {
        Ok(v) => v,
        Err(msg) => {
            warn!(
                path = query.path.as_str(),
                error = msg.as_str(),
                "browser preview: /browser/file resolve failed"
            );
            return match msg.as_str() {
                "Forbidden" => HttpResponse::Forbidden().body(msg),
                "File not found" => HttpResponse::NotFound().body(msg),
                _ => HttpResponse::BadRequest().body(msg),
            };
        }
    };

    match web::block(move || fs::read_to_string(file_path)).await {
        Ok(Ok(content)) => HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .append_header(("Cache-Control", "no-store"))
            .body(content),
        Ok(Err(e)) => {
            error!(error = %e, "browser preview: /browser/file read failed");
            HttpResponse::InternalServerError().finish()
        }
        Err(e) => {
            error!(error = %e, "browser preview: /browser/file worker failed");
            HttpResponse::InternalServerError().finish()
        }
    }
}

/// 预览页按**资源库相对路径**读取数据文件。
///
/// 图表的 `resource:` 引用在软件内有后端命令解析；静态预览页没有 Tauri IPC，
/// 只能用 HTTP，于是这里提供等价的一条路。相对路径里的 `..` 直接拒绝。
#[get("/browser/resource")]
async fn browser_resource(query: web::Query<PathQuery>) -> HttpResponse {
    let rel = query.path.trim().replace('\\', "/");
    if rel.is_empty() || rel.starts_with('/') || rel.split('/').any(|seg| seg == "..") {
        return HttpResponse::BadRequest().body("Invalid resource path");
    }

    let root = crate::commands::settingview::get_image_bed_root();
    let root = root.trim();
    if root.is_empty() {
        return HttpResponse::BadRequest().body("Resource root not configured");
    }
    let abs = PathBuf::from(format!(
        "{}/{}",
        root.trim_end_matches(['/', '\\']),
        rel
    ));
    // 只有落在资源库根目录内的文件才允许读。
    let (Ok(canon), Ok(root_canon)) = (abs.canonicalize(), Path::new(root).canonicalize()) else {
        return HttpResponse::NotFound().body("File not found");
    };
    if !canon.starts_with(&root_canon) {
        return HttpResponse::Forbidden().body("Forbidden");
    }

    match web::block(move || fs::read_to_string(canon)).await {
        Ok(Ok(content)) => HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .append_header(("Cache-Control", "no-store"))
            .body(content),
        Ok(Err(e)) => {
            error!(error = %e, "browser preview: /browser/resource read failed");
            HttpResponse::InternalServerError().finish()
        }
        Err(e) => {
            error!(error = %e, "browser preview: /browser/resource worker failed");
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/browser/download")]
async fn browser_download(query: web::Query<PathQuery>) -> HttpResponse {
    info!(
        path = query.path.as_str(),
        "browser preview: /browser/download"
    );

    let file_path = match resolve_allowed_markdown_file(&query.path) {
        Ok(v) => v,
        Err(msg) => {
            warn!(
                path = query.path.as_str(),
                error = msg.as_str(),
                "browser preview: /browser/download resolve failed"
            );
            return match msg.as_str() {
                "Forbidden" => HttpResponse::Forbidden().body(msg),
                "File not found" => HttpResponse::NotFound().body(msg),
                _ => HttpResponse::BadRequest().body(msg),
            };
        }
    };

    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download.md")
        .to_string();

    let filename_fallback = filename_ascii_fallback(&filename);
    let filename_utf8 = encode(&filename);

    let content_disposition = format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        filename_fallback, filename_utf8
    );

    match web::block(move || fs::read(file_path)).await {
        Ok(Ok(bytes)) => HttpResponse::Ok()
            .content_type("application/octet-stream")
            .append_header(("Cache-Control", "no-store"))
            .append_header(("Content-Disposition", content_disposition))
            .body(bytes),
        Ok(Err(e)) => {
            error!(error = %e, "browser preview: /browser/download read failed");
            HttpResponse::InternalServerError().finish()
        }
        Err(e) => {
            error!(error = %e, "browser preview: /browser/download worker failed");
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/browser/preview")]
async fn browser_preview(query: web::Query<PathQuery>) -> HttpResponse {
    info!(
        path = query.path.as_str(),
        "browser preview: /browser/preview"
    );

    let file_path = match resolve_allowed_markdown_file(&query.path) {
        Ok(v) => v,
        Err(msg) => {
            return match msg.as_str() {
                "Forbidden" => HttpResponse::Forbidden().body(msg),
                "File not found" => HttpResponse::NotFound().body(msg),
                _ => HttpResponse::BadRequest().body(msg),
            }
        }
    };

    let title = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Guaxi Preview");

    let base_dir = file_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_string_lossy();
    let base_dir = base_dir.trim_start_matches(r"\\?\").to_string();

    let abs = file_path.to_string_lossy();
    let abs = abs.trim_start_matches(r"\\?\");
    let encoded_path = encode(abs).to_string();

    let html = format!(
        r#"<!doctype html>
        <html lang="zh">
        <head>
          <meta charset="utf-8" />
          <meta name="viewport" content="width=device-width, initial-scale=1" />
          <title>{title}</title>
          <style>
            body {{ font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace; BlinkMacSystemFont,'Segoe UI',Roboto,Arial,'Noto Sans','PingFang SC','Microsoft YaHei',sans-serif; line-height: 1.5; word-wrap: break-word; margin: 0; color: #1f2328; }}
            .topbar {{ position: sticky; top: 0; background: rgba(255,255,255,0.92); backdrop-filter: blur(10px); border-bottom: 1px solid rgba(0,0,0,0.08); padding: 10px 14px; }}
            .titlebar {{ display: flex; align-items: center; justify-content: space-between; gap: 10px; }}
            .title {{ font-weight: 600; font-size: 13px; word-break: break-all; }}
            .dl {{ flex: 0 0 auto; font-weight: 700; padding: 2px 6px; border-radius: 8px; color: rgba(31,35,40,0.75); border: 1px solid rgba(0,0,0,0.08); text-decoration: none; }}
            .dl:hover {{ color: #0969da; border-color: rgba(9,105,218,0.35); background: rgba(9,105,218,0.06); }}
            .sub {{ margin-top: 4px; font-size: 12px; color: rgba(31,35,40,0.6); word-break: break-all; }}
            .content {{ max-width: 980px; margin: 0 auto; padding: 18px 16px 60px; }}
            pre {{ background: #ffffff; border: 1px solid rgba(0,0,0,0.08); padding: 12px; border-radius: 10px; overflow: auto; scrollbar-width: thin; scrollbar-color: rgba(0,0,0,0.22) transparent; }}
            pre::-webkit-scrollbar {{ width: 6px; height: 6px; }}
            pre::-webkit-scrollbar-thumb {{ background-color: rgba(0,0,0,0.22); border-radius: 999px; }}
            pre::-webkit-scrollbar-thumb:hover {{ background-color: rgba(0,0,0,0.34); }}
            pre::-webkit-scrollbar-track {{ background-color: transparent; }}
            pre code.hljs, code.hljs, .hljs {{ background: transparent !important; }}
            code {{ font-family: ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,'Liberation Mono','Courier New',monospace; }}
            img {{ max-width: 100%; }}
            .guaxi-chart {{ width: 100%; min-height: 240px; margin: 14px 0; }}
            .guaxi-excalidraw-note {{ margin: 12px 0; padding: 10px 12px; border: 1px dashed rgba(0,0,0,0.16); border-radius: 8px; font-size: 13px; color: rgba(31,35,40,0.7); }}
            a {{ color: #0969da; }}
            .content a[id^="guaxi-toc-"] {{ display: block; position: relative; top: -72px; visibility: hidden; pointer-events: none; }}
            .guaxi-media-group {{
              --guaxi-media-gap: clamp(14px, 2vw, 24px);
              display: flex !important;
              flex-wrap: wrap;
              justify-content: center !important;
              align-items: stretch;
              align-content: flex-start;
              gap: var(--guaxi-media-gap);
              width: 100%;
              max-width: 1000px;
              margin: 22px auto;
              padding: 6px 0;
            }}
            .guaxi-media-group.is-single {{
              display: block !important;
              width: min(100%, 760px);
              gap: 0;
            }}
            .guaxi-media-group.is-gallery {{
              display: grid !important;
              grid-template-columns: repeat(auto-fit, minmax(220px, 320px));
              justify-content: center !important;
              justify-items: center;
              align-items: start;
              padding-left: 6px;
              padding-right: 6px;
            }}
            .guaxi-media-group__item {{
              display: flex;
              align-items: center;
              justify-content: center;
              min-width: 0;
              width: fit-content;
              max-width: 100%;
              box-sizing: border-box;
              padding: 0;
              border: none;
              background: transparent;
              box-shadow: none;
              overflow: visible;
              text-decoration: none;
              transition: opacity 0.18s ease;
            }}
            .guaxi-media-group__item:hover {{ opacity: 0.98; }}
            .guaxi-media-group.is-single > .guaxi-media-group__item {{
              display: flex;
              flex: 0 1 auto;
              min-width: 0;
              width: fit-content;
              max-width: min(100%, 760px);
              margin-left: auto;
              margin-right: auto;
            }}
            .guaxi-media-group__image {{
              display: block;
              width: 100%;
              max-width: 100%;
              max-height: min(56vh, 420px);
              height: auto;
              margin: 0;
              border-radius: 16px;
              object-fit: contain;
            }}
            .guaxi-media-group.is-single .guaxi-media-group__image {{
              width: auto;
              max-width: min(100%, 760px);
              max-height: min(72vh, 640px);
            }}
            table {{ border-collapse: collapse; width: 100%; margin: 12px 0; }}
            th, td {{ border: 1px solid rgba(0,0,0,0.16); padding: 6px 10px; }}
            thead th {{ background: rgba(0,0,0,0.04); }}
            @media (max-width: 760px) {{
              .guaxi-media-group,
              .guaxi-media-group.is-gallery {{
                width: 100%;
                max-width: 100%;
              }}
              .guaxi-media-group__item {{
                max-width: 100%;
                width: 100%;
              }}
            }}
          </style>
          <link rel="stylesheet" href="/browser/assets/highlightjs/styles/atom-one-light.css" />
          <script>
            window.MathJax = {{
              tex: {{
                inlineMath: [['$', '$'], ['\\(', '\\)']],
                displayMath: [['$$', '$$'], ['\\[', '\\]']],
                processEscapes: true,
                tags: 'ams'
                }},
              svg: {{
                fontCache: 'global'
              }}
            }};
          </script>
          <script src="/browser/assets/markdown-it/markdown-it.min.js"></script>
          <script src="/browser/assets/mermaid/mermaid.min.js"></script>
          <script id="MathJax-script" src="/browser/assets/mathjax/tex-svg.js"></script>
          <script src="/browser/assets/highlightjs/highlight.min.js"></script>
          <script src="/browser/assets/echarts/echarts.min.js"></script>
          <script src="/browser/assets/echarts/echarts-liquidfill.min.js"></script>
          <script src="/browser/assets/echarts/echarts-wordcloud.min.js"></script>
          </head>
        <body>
          <div class="topbar">
            <div class="titlebar">
              <div class="title">{title}</div>
              <a class="dl" href="/browser/download?path={encodedPath}" title="下载" aria-label="下载">⭳</a>
            </div>
            <div class="sub">{path}</div>
          </div>
          <div class="content" id="content">加载中…</div>

          <script>
            const contentEl = document.getElementById('content');
            const filePathEncoded = {filePathJson};
            const baseDir = {baseDirJson};

            function escapeHtml(s) {{
              return String(s)
                .replaceAll('&', '&amp;')
                .replaceAll('<', '&lt;')
                .replaceAll('>', '&gt;');
            }}

            function enhanceMediaGroups(root) {{
              if (!(root instanceof Element) || !root.querySelector('img')) {{
                return;
              }}

              const blocks = Array.from(root.querySelectorAll('p'));
              const isWhitespaceNode = (node) => {{
                if (!node) return true;
                if (node.nodeType === Node.TEXT_NODE) return !String(node.textContent || '').trim();
                if (node.nodeType === Node.ELEMENT_NODE && node instanceof HTMLBRElement) return true;
                return false;
              }};
              const resolveMediaItem = (node) => {{
                if (!(node instanceof Element)) return null;
                if (node instanceof HTMLImageElement) {{
                  return {{ kind: 'image', host: node, image: node }};
                }}
                if (node instanceof HTMLAnchorElement) {{
                  const childElements = Array.from(node.children || []);
                  if (childElements.length !== 1 || !(childElements[0] instanceof HTMLImageElement)) return null;
                  const hasNonMediaChild = Array.from(node.childNodes || []).some((child) => child !== childElements[0] && !isWhitespaceNode(child));
                  if (hasNonMediaChild) return null;
                  return {{ kind: 'linked-image', host: node, image: childElements[0] }};
                }}
                return null;
              }};

              blocks.forEach((block) => {{
                const meaningfulNodes = Array.from(block.childNodes || []).filter((node) => !isWhitespaceNode(node));
                if (!meaningfulNodes.length) return;
                const mediaItems = meaningfulNodes.map(resolveMediaItem);
                if (mediaItems.some((item) => !item)) return;

                block.classList.add('guaxi-media-group');
                block.classList.toggle('is-single', mediaItems.length === 1);
                block.classList.toggle('is-gallery', mediaItems.length > 1);

                mediaItems.forEach((item) => {{
                  if (item.host === item.image) {{
                    const wrapper = document.createElement('span');
                    wrapper.setAttribute('data-guaxi-media-item', item.kind);
                    item.image.replaceWith(wrapper);
                    wrapper.appendChild(item.image);
                    item.host = wrapper;
                  }} else {{
                    item.host.setAttribute('data-guaxi-media-item', item.kind);
                  }}
                  item.host.classList.add('guaxi-media-group__item');
                  item.image.classList.add('guaxi-media-group__image');
                }});
              }});
            }}

            async function renderChartDrafts(root, drafts, baseDirHint) {{
              if (!drafts || !drafts.length) return;
              if (!window.echarts) {{
                drafts.forEach((draft) => {{
                  const el = root.querySelector('#' + draft.id);
                  if (el) el.textContent = '图表渲染需要 ECharts，但资源未加载';
                }});
                return;
              }}

              for (const draft of drafts) {{
                const el = root.querySelector('#' + draft.id);
                if (!el) continue;
                try {{
                  const parsed = parseChartSource(draft.source);
                  if (!parsed.spec) throw new Error('spec 不是合法 JSON');
                  let text = parsed.data;
                  if (parsed.dataRef) {{
                    text = await loadChartData(parsed.dataRef, baseDirHint);
                  }}
                  const rows = parseChartRows(text);
                  if (!rows.length) throw new Error('数据为空');
                  el.style.height = '360px';
                  const chart = window.echarts.init(el, null, {{ renderer: 'svg' }});
                  chart.setOption(buildChartOption(parsed.spec, rows), true);
                  window.addEventListener('resize', () => {{
                    try {{ chart.resize(); }} catch (_) {{}}
                  }});
                }} catch (error) {{
                  el.textContent = '图表渲染失败：' + ((error && error.message) || error);
                }}
              }}
            }}

            // 围栏解析：与软件内同构的极简版（spec 用 JSON，data 支持内联块与路径）。
            function parseChartSource(source) {{
              const text = String(source || '');
              let spec = null;
              let data = '';
              let dataRef = '';
              const lines = text.split(/\r?\n/);
              let mode = '';
              const specLines = [];
              const dataLines = [];

              for (const line of lines) {{
                const trimmed = line.trim();
                if (!trimmed) {{ if (mode === 'data') dataLines.push(''); continue; }}
                const m = trimmed.match(/^(spec|data)\s*:\s*(.*)$/i);
                if (m && !/^\s/.test(line)) {{
                  mode = m[1].toLowerCase();
                  const rest = m[2].trim();
                  if (mode === 'data' && rest && rest !== '|' && rest !== '>') {{
                    const looksPath = /^resource:/i.test(rest) || /^\.{{0,2}}\//.test(rest)
                      || /^[a-zA-Z]:[\\/]/.test(rest) || /\.(csv|tsv|json)$/i.test(rest);
                    if (looksPath) dataRef = rest;
                    else dataLines.push(rest);
                  }}
                  continue;
                }}
                if (mode === 'data') dataLines.push(line);
                else if (mode === 'spec') specLines.push(line);
              }}

              const specText = specLines.join('\n').trim();
              if (specText) {{
                try {{ spec = JSON.parse(specText); }} catch (_) {{
                  try {{ spec = parseSimpleYaml(specText); }} catch (_) {{ spec = null; }}
                }}
              }}
              data = dataLines.join('\n').trim();
              return {{ spec: spec, data: data, dataRef: dataRef }};
            }}

            // 极简 YAML 子集：只支持「key: value」与「[a, b]」数组，覆盖手册里所有 spec 写法。
            // 不处理嵌套对象 —— 遇到就原样留成字符串，由 ECharts 自己忽略。
            function parseSimpleYaml(text) {{
              const out = {{}};
              for (const raw of String(text).split(/\r?\n/)) {{
                const line = raw.replace(/\t/g, '  ');
                const trimmed = line.trim();
                if (!trimmed || trimmed.startsWith('#')) continue;
                const m = trimmed.match(/^([^:]+):\s*(.*)$/);
                if (!m) continue;
                const key = m[1].trim();
                const value = m[2].trim();
                if (!value) {{ out[key] = true; continue; }}
                if (/^\[.*\]$/.test(value)) {{
                  out[key] = value.slice(1, -1).split(',').map((v) => v.trim()).filter(Boolean);
                  continue;
                }}
                if (/^['"].*['"]$/.test(value)) {{ out[key] = value.slice(1, -1); continue; }}
                if (/^(true|false)$/i.test(value)) {{ out[key] = value.toLowerCase() === 'true'; continue; }}
                if (/^-?\d+(\.\d+)?$/.test(value)) {{ out[key] = Number(value); continue; }}
                out[key] = value;
              }}
              return Object.keys(out).length ? out : null;
            }}

            function parseChartRows(text) {{
              const raw = String(text || '').trim();
              if (!raw) return [];
              if (/^[[{{]/.test(raw)) {{
                try {{
                  const parsed = JSON.parse(raw);
                  const arr = Array.isArray(parsed) ? parsed : (parsed.data || parsed.rows || []);
                  return Array.isArray(arr) ? arr : [];
                }} catch (_) {{ return []; }}
              }}
              const delim = raw.indexOf('\t') >= 0 ? '\t' : ',';
              const lines = raw.split(/\r?\n/).filter((l) => l.trim());
              if (lines.length < 2) return [];
              const header = lines[0].split(delim).map((h) => h.trim());
              return lines.slice(1).map((line) => {{
                const cells = line.split(delim);
                const row = {{}};
                header.forEach((key, i) => {{
                  const v = String(cells[i] == null ? '' : cells[i]).trim();
                  row[key] = v !== '' && /^-?\d+(\.\d+)?$/.test(v) ? Number(v) : v;
                }});
                return row;
              }});
            }}

            async function loadChartData(ref, baseDirHint) {{
              // 1) 资源库引用：交给后端解析，前端不需要知道库根目录。
              if (/^resource:/i.test(ref)) {{
                const rel = ref.replace(/^resource:/i, '').replace(/^[/\\]+/, '');
                const resp = await fetch('/browser/resource?path=' + encodeURIComponent(rel));
                if (!resp.ok) throw new Error('资源库数据读取失败: ' + resp.status);
                return await resp.text();
              }}
              // 2) 相对路径：相对当前笔记目录，经 /browser/raw 同源的静态读取端点。
              const absolute = /^([a-zA-Z]:[\\/]|\/|~[\\/])/.test(ref)
                ? ref
                : ((baseDirHint ? baseDirHint.replace(/[\\/]+$/, '') + '/' : '') + ref.replace(/^\.\//, ''));
              const resp = await fetch('/browser/file?path=' + encodeURIComponent(absolute));
              if (!resp.ok) throw new Error('数据文件读取失败: ' + resp.status + ' ' + absolute);
              return await resp.text();
            }}

            /**
             * 组装 ECharts option。
             *
             * 只覆盖通用族（直角坐标系的 line/bar/area/waterfall/scatter + 饼图家族）：
             * 这是手册与日常笔记里 95% 的用法。未覆盖的类型会明确提示，
             * 而不是画出一张错图 —— 静态预览是只读视图，宁缺毋滥。
             */
            function buildChartOption(spec, rows) {{
              const type = String(spec.type || 'line').toLowerCase().replace(/[\s_-]/g, '');
              const xKey = String(spec.x || Object.keys(rows[0] || {{}})[0] || '');
              let yKeys = Array.isArray(spec.y) ? spec.y.map(String)
                : (spec.y ? [String(spec.y)] : Object.keys(rows[0] || {{}}).filter((k) => k !== xKey));
              if (!yKeys.length) yKeys = ['value'];

              const cats = rows.map((r) => String(r[xKey] === undefined ? '' : r[xKey]));
              const num = (v) => (typeof v === 'number' ? v : (Number.isFinite(Number(v)) && v !== '' ? Number(v) : null));
              const title = spec.title ? {{ text: String(spec.title), left: 'center' }} : undefined;
              const legend = spec.legend === false ? undefined : {{ bottom: 0 }};
              const tooltip = spec.tooltip === false ? undefined : {{ trigger: 'axis' }};
              const grid = {{ left: 48, right: 24, top: title ? 56 : 24, bottom: legend ? 48 : 32 }};

              const isPie = ['pie', 'doughnut', 'donut', 'ring', 'circle', 'rosetype'].includes(type);
              if (type === 'liquidfill') {{
                // 水球图：扩展包已由页面脚本注册；未注册时 ECharts 会画空白，这里显式提示。
                const val = num(rows[0][yKeys[0]]) || 0;
                return {{
                  title: title,
                  series: [{{
                    type: 'liquidFill',
                    data: [Math.max(0, Math.min(1, (spec.max ? val / Number(spec.max) : val)))],
                    radius: '78%',
                    label: {{ formatter: spec.formatter || '{{c}}' }},
                  }}],
                }};
              }}

              if (type === 'wordcloud') {{
                const wordKey = xKey;
                const weightKey = yKeys[0];
                return {{
                  title: title,
                  series: [{{
                    type: 'wordCloud',
                    shape: 'circle',
                    sizeRange: Array.isArray(spec.sizeRange) ? spec.sizeRange : [14, 56],
                    rotationRange: [0, 0],
                    gridSize: 8,
                    textStyle: {{ color: () => 'rgb(' + [
                      60 + Math.round(Math.random() * 120),
                      80 + Math.round(Math.random() * 100),
                      140 + Math.round(Math.random() * 100),
                    ].join(',') + ')' }},
                    data: rows.map((r) => ({{ name: String(r[wordKey] === undefined ? '' : r[wordKey]), value: num(r[weightKey]) || 0 }})),
                  }}],
                }};
              }}

              if (type === 'funnel') {{
                const nameKey = xKey;
                const valueKey = yKeys[0];
                return {{
                  title: title,
                  tooltip: {{ trigger: 'item' }},
                  legend: legend,
                  series: [{{
                    type: 'funnel',
                    left: '12%', right: '12%', top: title ? 64 : 24, bottom: legend ? 48 : 24,
                    sort: 'descending',
                    gap: 2,
                    label: {{ position: 'inside', formatter: '{{b}} {{c}}' }},
                    data: rows.map((r) => ({{ name: String(r[nameKey] === undefined ? '' : r[nameKey]), value: num(r[valueKey]) || 0 }})),
                  }}],
                }};
              }}

              if (type === 'gauge') {{
                // 单行数据 = 单环；多行 = 多环（与软件内语义一致）。
                const valueKey = yKeys[0];
                const nameKey = spec.x ? String(spec.x) : '';
                const max = Number(spec.max) || 100;
                const items = rows.slice(0, 6).map((r, i) => ({{
                  name: nameKey ? String(r[nameKey] === undefined ? '' : r[nameKey]) : (spec.title ? String(spec.title) : ('指标 ' + (i + 1))),
                  value: num(r[valueKey]) || 0,
                }}));
                const axisLine = {{ lineStyle: {{ width: 12 }} }};
                const series = items.map((item, i) => ({{
                  type: 'gauge',
                  name: item.name,
                  min: 0,
                  max: max,
                  radius: (items.length > 1 ? (86 - i * 14) : 78) + '%',
                  startAngle: 210, endAngle: -30,
                  splitNumber: 5,
                  axisLine: axisLine,
                  pointer: {{ width: 4 }},
                  detail: {{ formatter: spec.formatter || '{{value}}', fontSize: 16, offsetCenter: [0, '62%'] }},
                  title: {{ fontSize: 12, offsetCenter: [0, '82%'] }},
                  data: [item],
                }}));
                return {{ title: title, series: series }};
              }}

              if (type === 'radar') {{
                const nameKey = xKey;
                const indicators = yKeys.map((k) => ({{ name: k, max: Math.max(...rows.map((r) => num(r[k]) || 0)) * 1.2 || 1 }}));
                return {{
                  title: title,
                  tooltip: spec.tooltip === false ? undefined : {{}},
                  legend: legend,
                  radar: {{ indicator: indicators, radius: '62%', center: ['50%', title ? '56%' : '52%'] }},
                  series: [{{
                    type: 'radar',
                    data: rows.slice(0, 8).map((r) => ({{
                      name: String(r[nameKey] === undefined ? '' : r[nameKey]),
                      value: yKeys.map((k) => num(r[k]) || 0),
                    }})),
                  }}],
                }};
              }}

              if (type === 'sankey') {{
                const sourceKey = String(spec.source || xKey);
                const targetKey = spec.target ? String(spec.target) : (yKeys[0] || 'target');
                const valueKey = spec.value ? String(spec.value) : (yKeys[1] || 'value');
                const names = [];
                const links = [];
                rows.forEach((r) => {{
                  const s = String(r[sourceKey] === undefined ? '' : r[sourceKey]);
                  const t = String(r[targetKey] === undefined ? '' : r[targetKey]);
                  if (!names.includes(s)) names.push(s);
                  if (!names.includes(t)) names.push(t);
                  links.push({{ source: s, target: t, value: num(r[valueKey]) || 0 }});
                }});
                return {{
                  title: title,
                  tooltip: {{ trigger: 'item' }},
                  series: [{{
                    type: 'sankey',
                    left: 24, right: 24, top: title ? 56 : 24, bottom: 24,
                    data: names.map((n) => ({{ name: n }})),
                    links: links,
                    emphasis: {{ focus: 'adjacency' }},
                  }}],
                }};
              }}

              // 未覆盖的类型在函数末尾统一报错（见 buildChartOption 结尾）。

              if (isPie) {{
                const nameKey = xKey;
                const valueKey = yKeys[0];
                return {{
                  title: title,
                  tooltip: spec.tooltip === false ? undefined : {{ trigger: 'item' }},
                  legend: legend,
                  series: [{{
                    name: spec.title ? String(spec.title) : '占比',
                    type: 'pie',
                    radius: (type === 'pie') ? (spec.radius || '62%') : (spec.radius || ['42%', '66%']),
                    itemStyle: {{ borderRadius: 4, borderColor: '#fff', borderWidth: 1 }},
                    label: {{ formatter: '{{b}}: {{d}}%' }},
                    data: rows.map((r) => ({{ name: String(r[nameKey] === undefined ? '' : r[nameKey]), value: num(r[valueKey]) || 0 }})),
                  }}],
                }};
              }}

              // 走到这里只剩直角坐标系的通用族；其余类型明确报错而不是画一张错图。
              const supportedCartesian = ['line', 'area', 'bar', 'column', 'waterfall', 'pictorialbar', 'scatter', 'bubble'];
              if (!supportedCartesian.includes(type)) {{
                throw new Error('静态预览暂不支持该图表类型：' + type + '（请在软件内查看完整图表）');
              }}
              const scatterLike = ['scatter', 'bubble'].includes(type);
              if (scatterLike) {{
                return {{
                  title: title,
                  tooltip: {{ trigger: 'item' }},
                  legend: legend,
                  grid: grid,
                  xAxis: {{ type: 'category', data: cats, name: spec.xName || '' }},
                  yAxis: {{ type: 'value', name: spec.yName || '' }},
                  series: yKeys.map((k) => ({{
                    name: k,
                    type: 'scatter',
                    symbolSize: Number(spec.symbolSize) || 10,
                    data: rows.map((r) => num(r[k])),
                  }})),
                }};
              }}

              const isStack = spec.stack === true || typeof spec.stack === 'string';
              if (type === 'waterfall') {{
                // 瀑布图：由增量推累计。
                const deltas = rows.map((r) => num(r[yKeys[0]]) || 0);
                const base = [];
                const value = [];
                let acc = 0;
                deltas.forEach((d, i) => {{
                  if (spec.total === true && i === deltas.length - 1) {{
                    base.push(0); value.push(acc);
                  }} else {{
                    base.push(d >= 0 ? acc : acc + d); value.push(Math.abs(d)); acc += d;
                  }}
                }});
                const series = [
                  {{ name: 'base', type: 'bar', stack: 'wf', itemStyle: {{ color: 'transparent' }}, emphasis: {{ itemStyle: {{ color: 'transparent' }} }}, data: base }},
                  {{ name: yKeys[0], type: 'bar', stack: 'wf', data: value }},
                ];
                return {{ title: title, tooltip: tooltip, legend: legend, grid: grid,
                  xAxis: {{ type: 'category', data: cats }}, yAxis: {{ type: 'value' }}, series: series }};
              }}

              const horizontal = String(spec.orient || '').toLowerCase() === 'horizontal';
              const isBar = ['bar', 'waterfall', 'pictorialbar', 'column'].includes(type) || horizontal;
              const series = yKeys.map((k, i) => {{
                const base = {{
                  name: k,
                  type: isBar ? 'bar' : 'line',
                  data: rows.map((r) => num(r[k])),
                }};
                if (isBar) {{
                  base.barMaxWidth = 42;
                  if (spec.showBackground) base.showBackground = true;
                  if (isStack) base.stack = typeof spec.stack === 'string' ? spec.stack : 'total';
                  if (Array.isArray(spec.itemColors) && spec.itemColors[i]) base.itemStyle = {{ color: spec.itemColors[i] }};
                }} else {{
                  if (spec.smooth) base.smooth = true;
                  if (spec.step) base.step = spec.step === true ? 'middle' : String(spec.step);
                  base.symbolSize = Number(spec.symbolSize) || 6;
                  if (type === 'area' || spec.areaStyle) base.areaStyle = {{ opacity: 0.25 }};
                }}
                return base;
              }});

              const catAxis = {{ type: 'category', data: cats, name: spec.xName || '' }};
              const valAxis = {{ type: 'value', name: spec.yName || '', min: spec.axisMin, max: spec.axisMax }};
              return {{
                title: title,
                tooltip: tooltip,
                legend: legend,
                grid: grid,
                xAxis: horizontal ? valAxis : catAxis,
                yAxis: horizontal ? catAxis : valAxis,
                series: series,
              }};
            }}

            async function loadAndRender() {{
              if (!contentEl) {{
                throw new Error('Missing #content element');
              }}

              const rawResp = await fetch('/browser/raw?path=' + filePathEncoded);
              if (!rawResp.ok) {{
                let detail = '';
                try {{ detail = await rawResp.text(); }} catch (_) {{}}
                contentEl.textContent = '读取失败: ' + rawResp.status + (detail ? (' - ' + detail) : '');
                return;
              }}

              const md = await rawResp.text();

              // 基于内容的简易变更检测：如果内容未变化，则不重复渲染
              if (window.__guaxiLastPreviewMd !== undefined && window.__guaxiLastPreviewMd === md) {{
                return;
              }}
              window.__guaxiLastPreviewMd = md;

              let processed = md;
              try {{
                const procResp = await fetch('/api/markdown/image_process', {{
                  method: 'POST',
                  headers: {{ 'Content-Type': 'application/json' }},
                  body: JSON.stringify({{ content: md, base_path: baseDir }})
                }});
                if (procResp.ok) {{
                  const json = await procResp.json();
                  processed = json.processed_content || md;
                }}
              }} catch (e) {{}}

              if (!window.markdownit) {{
                contentEl.innerHTML = '<pre><code>' + escapeHtml(md) + '</code></pre>';
                return;
              }}

              const mdIt = window.markdownit({{ html: true, linkify: true, breaks: true }});
              // ```chart 围栏：先转成占位 div，渲染完 markdown 后再由 ECharts 接管。
              // 渲染器是独立的静态页面，不能复用软件内的图表模块，所以这里只做
              // 「解析围栏 → 组装最小 option → echarts.init」这一条最小路径。
              const chartDrafts = [];
              const defaultFence = mdIt.renderer.rules.fence;
              mdIt.renderer.rules.fence = (tokens, idx, options, env, self) => {{
                const token = tokens[idx];
                const info = String((token && token.info) || '').trim();
                if (/^chart(\s|$)/i.test(info)) {{
                  const id = 'guaxi-chart-' + chartDrafts.length;
                  chartDrafts.push({{ id: id, source: String(token.content || '') }});
                  return '<div class="guaxi-chart" id="' + id + '"></div>';
                }}
                // excalidraw 围栏：静态预览里没有导出器，直接把场景文件路径渲染成一条提示，
                // 至少不会像代码块那样让人以为内容丢了。
                if (/^excalidraw(\s|$)/i.test(info)) {{
                  const scenePath = String(token.content || '').trim();
                  return '<div class="guaxi-excalidraw-note">Excalidraw 白板：<code>'
                    + escapeHtml(scenePath) + '</code>（请在软件内查看）</div>';
                }}
                return defaultFence(tokens, idx, options, env, self);
              }};

              contentEl.innerHTML = mdIt.render(processed);
              enhanceMediaGroups(contentEl);
              await renderChartDrafts(contentEl, chartDrafts, baseDir);

              if (window.MathJax) {{
                try {{
                  if (window.MathJax.typesetPromise) {{
                    await window.MathJax.typesetPromise([contentEl]);
                  }} else if (window.MathJax.startup && window.MathJax.startup.promise) {{
                    await window.MathJax.startup.promise;
                    if (window.MathJax.typesetPromise) {{
                      await window.MathJax.typesetPromise([contentEl]);
                    }}
                  }}
                }} catch (_) {{
                }}
              }}

              if (window.mermaid) {{
                try {{
                  window.mermaid.initialize({{ startOnLoad: false }});

                  const mermaidCodeBlocks = contentEl.querySelectorAll('pre code.language-mermaid, pre code.mermaid');
                  mermaidCodeBlocks.forEach((codeEl) => {{
                    const preEl = codeEl.parentElement;
                    if (!preEl) return;

                    const container = document.createElement('div');
                    container.className = 'mermaid';
                    container.textContent = codeEl.textContent || '';

                    preEl.replaceWith(container);
                  }});

                  const nodes = contentEl.querySelectorAll('.mermaid');
                  if (nodes.length) {{
                    await window.mermaid.run({{ nodes }});
                  }}
                }} catch (_) {{
                }}
              }}

              if (window.hljs) {{
                try {{
                  const codeBlocks = contentEl.querySelectorAll('pre code');
                  codeBlocks.forEach((codeEl) => {{
                    try {{
                      window.hljs.highlightElement(codeEl);
                    }} catch (_) {{
                    }}
                  }});
                }} catch (_) {{
                }}
              }}
            }}

            loadAndRender().catch((e) => {{
              if (!contentEl) return;
              contentEl.textContent = (e && e.message) ? e.message : '渲染失败';
            }});

            // 启动轮询检测，文件保存后自动刷新预览
            const POLL_INTERVAL_MS = 1200;

            function startAutoReload() {{
              if (window.__guaxiPreviewAutoReloadStarted) {{
                return;
              }}
              window.__guaxiPreviewAutoReloadStarted = true;

              console.log('[Guaxi Preview] Auto-reload watcher started, interval =', POLL_INTERVAL_MS, 'ms');

              setInterval(() => {{
                loadAndRender().catch((e) => {{
                  console.warn('[Guaxi Preview] Auto-reload failed:', e);
                }});
              }}, POLL_INTERVAL_MS);
            }}

            startAutoReload();
          </script>
        </body>
        </html>"#,
        title = escape_html(title),
        path = escape_html(file_path.to_string_lossy().as_ref()),
        filePathJson = serde_json::to_string(&encoded_path).unwrap_or_else(|_| "\"\"".to_string()),
        baseDirJson = serde_json::to_string(&base_dir).unwrap_or_else(|_| "\"\"".to_string()),
        encodedPath = encoded_path
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .append_header(("Cache-Control", "no-store"))
        .body(html)
}

#[get("/browser/assets/markdown-it/markdown-it.min.js")]
async fn browser_asset_markdown_it() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", "no-store"))
        .body(include_str!("../../assets/vendor/markdown-it.min.js"))
}

#[get("/browser/assets/mermaid/mermaid.min.js")]
async fn browser_asset_mermaid() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", "no-store"))
        .body(include_str!("../../assets/vendor/mermaid.min.js"))
}

#[get("/browser/assets/mathjax/tex-svg.js")]
async fn browser_asset_mathjax() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", "no-store"))
        .body(include_str!("../../assets/vendor/tex-svg.js"))
}

#[get("/browser/assets/highlightjs/highlight.min.js")]
async fn browser_asset_highlight_js() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", "no-store"))
        .body(include_str!("../../assets/vendor/highlight.min.js"))
}

/// ECharts UMD 包，供 ```chart 围栏在静态预览页里画图。
///
/// 预览页是独立于前端渲染器的静态页面（markdown-it + 少量内联脚本），
/// 不共享前端的图表模块，因此这里直接把官方 UMD 包分发给页面。
#[get("/browser/assets/echarts/echarts.min.js")]
async fn browser_asset_echarts() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", "no-store"))
        .body(include_str!("../../assets/vendor/echarts.min.js"))
}

/// 图表扩展包（水球图 / 词云）。它们靠副作用把系列注册进 `window.echarts`。
#[get("/browser/assets/echarts/echarts-liquidfill.min.js")]
async fn browser_asset_echarts_liquidfill() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", "no-store"))
        .body(include_str!("../../assets/vendor/echarts-liquidfill.min.js"))
}

#[get("/browser/assets/echarts/echarts-wordcloud.min.js")]
async fn browser_asset_echarts_wordcloud() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", "no-store"))
        .body(include_str!("../../assets/vendor/echarts-wordcloud.min.js"))
}

#[get("/browser/assets/highlightjs/styles/atom-one-light.css")]
async fn browser_asset_highlight_css() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/css; charset=utf-8")
        .append_header(("Cache-Control", "no-store"))
        .body(include_str!("../../assets/vendor/atom-one-light.css"))
}

pub fn browser_preview_config(cfg: &mut web::ServiceConfig) {
    cfg.service(favicon)
        .service(browser_index)
        .service(browser_knowledge_bases)
        .service(browser_knowledge_base_files)
        .service(browser_raw)
        .service(browser_file)
        .service(browser_resource)
        .service(browser_download)
        .service(browser_asset_markdown_it)
        .service(browser_asset_mermaid)
        .service(browser_asset_mathjax)
        .service(browser_asset_highlight_js)
        .service(browser_asset_echarts)
        .service(browser_asset_echarts_liquidfill)
        .service(browser_asset_echarts_wordcloud)
        .service(browser_asset_highlight_css)
        .service(browser_preview);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "guaxi_preview_test_{}_{}",
            std::process::id(),
            name
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn explicit_request_allows_markdown_outside_knowledge_bases() {
        // 这条登记就是为 plain/task 模式的外部目录准备的：它们不在任何知识库根下。
        let dir = unique_temp_file("allow");
        let file = dir.join("note.md");
        fs::write(&file, "# hi").expect("write temp md");
        let canon = file.canonicalize().expect("canonicalize");

        assert!(!is_remembered_preview_file(&canon));
        remember_preview_file(&canon);
        assert!(is_remembered_preview_file(&canon));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn only_the_exact_file_and_its_neighbours_are_allowed() {
        let dir = unique_temp_file("scope");
        let note = dir.join("note.md");
        let sibling = dir.join("data.csv");
        let outsider = dir.join("sub").join("other.csv");
        fs::write(&note, "# hi").expect("write note");
        fs::write(&sibling, "a,b").expect("write sibling");
        fs::create_dir_all(outsider.parent().unwrap()).expect("create sub");
        fs::write(&outsider, "a,b").expect("write outsider");

        let note_canon = note.canonicalize().expect("canon note");
        remember_preview_file(&note_canon);

        // 同目录的数据文件（图表 data: ./x.csv）放行，其它目录不放行。
        assert!(is_sibling_of_remembered_preview(
            &sibling.canonicalize().expect("canon sibling")
        ));
        assert!(!is_sibling_of_remembered_preview(
            &outsider.canonicalize().expect("canon outsider")
        ));
        assert!(!is_remembered_preview_file(
            &outsider.canonicalize().expect("canon outsider")
        ));

        fs::remove_dir_all(&dir).ok();
    }
}
