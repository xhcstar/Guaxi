use crate::commands::settingview::get_libraries_root;
use crate::state::SharedState;
use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{hash_map::DefaultHasher, HashMap};
use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tracing::{error, info, warn};


#[derive(Debug, Deserialize)]
pub struct GuaxiWebClipperRequest {
    pub url: String,
    pub title: String,
    pub description: String,
    pub content: String,
    pub timestamp: String,
    pub mode: String,
    #[serde(rename = "knowledgeBase")]
    pub knowledge_base: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(rename = "fileRepo")]
    pub file_repo: String,
}

#[derive(Debug, Deserialize)]
struct GuaxiWebClipperChunkedMeta {
    pub url: String,
    pub title: String,
    pub timestamp: String,
    pub mode: String,
    #[serde(rename = "fileId")]
    pub file_id: String,
    #[serde(rename = "totalChunks")]
    pub total_chunks: usize,
    #[serde(rename = "originalLength")]
    pub original_length: usize,
    #[serde(default)]
    pub knowledge_base: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub file_repo: String,
}

// 文件信息结构
#[derive(Serialize, Debug)]
struct BrowserSavedHtmlInfo {
    file_name: String,
    file_path: String,
    create_time: u64,
    modify_time: u64,
    file_type: String,
    knowledge_repo: String,
    file_repo: String,
    tags: Vec<String>,
    description: String,
    file_size: u64,
}

#[derive(Serialize, Debug)]
struct GuaxiWebClipperInfo {
    file_name: String,
    file_path: String,
    create_time: String,
    modify_time: String,
    file_type: String,
    knowledge_repo: String,
    file_repo: String,
    tags: Vec<String>,
    description: String,
    file_size: u64,
}

// 扩展请求数据结构
#[derive(Debug, Deserialize)]
pub struct ExtensionRequest {
    pub url: String,
    pub title: String,
    pub html: String,
    pub timestamp: String,
    pub mode: String,
}

// 分块上传元数据（使用rename兼容两种命名格式）
#[derive(Debug, Deserialize)]
struct ChunkedMeta {
    pub url: String,
    pub title: String,
    pub timestamp: String,
    pub mode: String,
    #[serde(rename = "fileId", alias = "file_id")]
    pub file_id: String,
    #[serde(rename = "totalChunks", alias = "total_chunks")]
    pub total_chunks: usize,
    #[serde(rename = "originalLength", alias = "original_length")]
    pub original_length: usize,
}

// 响应数据结构
#[derive(Serialize)]
pub struct ExtensionResponse {
    success: bool,
    message: String,
    saved_path: Option<String>,
}

// 分块上传状态（增强状态跟踪）
struct UploadState {
    meta: ChunkedMeta,
    received_chunks: usize,
    temp_file: Option<File>,
    received_bytes: usize, // 跟踪已接收字节数
}

type UploadStore = Arc<Mutex<HashMap<String, UploadState>>>;

// 配置服务路由
pub fn config(cfg: &mut web::ServiceConfig, state: web::Data<SharedState>) {
    let store = Arc::new(Mutex::new(HashMap::<String, UploadState>::new()));

    cfg.app_data(web::Data::new(store))
        .app_data(state) // 注册 SharedState
        .service(
            web::scope("/extension")
                .service(handle_extension_data)
                .service(handle_chunked_meta)
                .service(handle_chunk_upload)
                .service(handle_chunked_complete)
                .service(handle_webclipper_save)
                .service(handle_webclipper_meta)
                .service(handle_webclipper_chunk)
                .service(handle_webclipper_complete),
        );
}

#[post("/save")]
async fn handle_extension_data(req: web::Json<ExtensionRequest>) -> impl Responder {
    info!("Received standard extension data for URL: {}", req.url);

    if req.html.is_empty() {
        return HttpResponse::BadRequest().json(error_response("Empty html"));
    }

    let save_dir = match prepare_save_dir() {
        Ok(dir) => dir,
        Err(e) => return e,
    };

    let (filename, result) = match save_html_content(&req, &save_dir) {
        Ok(res) => res,
        Err(e) => return e,
    };

    match result {
        Ok(_) => HttpResponse::Ok().json(success_response(
            "HTML saved successfully",
            Some(save_dir.join(filename)),
        )),
        Err(e) => HttpResponse::InternalServerError()
            .json(error_response(&format!("Failed to save html: {}", e))),
    }
}

#[post("/save_chunk")]
async fn handle_chunk_upload(
    req: HttpRequest,
    chunk: web::Bytes,
    store: web::Data<UploadStore>,
) -> HttpResponse {
    // 验证必须的头信息
    let file_id = match req.headers().get("X-File-ID") {
        Some(header) => match header.to_str() {
            Ok(id) => id,
            Err(_) => {
                return HttpResponse::BadRequest().json(error_response("Invalid X-File-ID header"))
            }
        },
        None => return HttpResponse::BadRequest().json(error_response("Missing X-File-ID header")),
    };

    // 确保在函数返回前清理临时文件
    let cleanup = || cleanup_temp_file(file_id);

    // 验证其他头信息
    let chunk_index = match req.headers().get("X-Chunk-Index") {
        Some(header) => match header.to_str() {
            Ok(idx) => match idx.parse::<usize>() {
                Ok(n) => n,
                Err(_) => {
                    cleanup();
                    return HttpResponse::BadRequest()
                        .json(error_response("Invalid X-Chunk-Index"));
                }
            },
            Err(_) => {
                cleanup();
                return HttpResponse::BadRequest()
                    .json(error_response("Invalid X-Chunk-Index header"));
            }
        },
        None => {
            cleanup();
            return HttpResponse::BadRequest().json(error_response("Missing X-Chunk-Index header"));
        }
    };

    let chunk_start = match req.headers().get("X-Chunk-Start") {
        Some(header) => match header.to_str() {
            Ok(start) => match start.parse::<usize>() {
                Ok(n) => n,
                Err(_) => {
                    cleanup();
                    return HttpResponse::BadRequest()
                        .json(error_response("Invalid X-Chunk-Start"));
                }
            },
            Err(_) => {
                cleanup();
                return HttpResponse::BadRequest()
                    .json(error_response("Invalid X-Chunk-Start header"));
            }
        },
        None => {
            cleanup();
            return HttpResponse::BadRequest().json(error_response("Missing X-Chunk-Start header"));
        }
    };

    let chunk_end = match req.headers().get("X-Chunk-End") {
        Some(header) => match header.to_str() {
            Ok(end) => match end.parse::<usize>() {
                Ok(n) => n,
                Err(_) => {
                    cleanup();
                    return HttpResponse::BadRequest().json(error_response("Invalid X-Chunk-End"));
                }
            },
            Err(_) => {
                cleanup();
                return HttpResponse::BadRequest()
                    .json(error_response("Invalid X-Chunk-End header"));
            }
        },
        None => {
            cleanup();
            return HttpResponse::BadRequest().json(error_response("Missing X-Chunk-End header"));
        }
    };

    // 验证分块大小
    if chunk.len() != chunk_end - chunk_start {
        cleanup();
        return HttpResponse::BadRequest().json(error_response(&format!(
            "Chunk size mismatch: expected {}, got {}",
            chunk_end - chunk_start,
            chunk.len()
        )));
    }

    // 存储分块
    let mut store = match store.lock() {
        Ok(store) => store,
        Err(_) => {
            cleanup();
            return HttpResponse::InternalServerError()
                .json(error_response("Failed to lock store"));
        }
    };

    let state = match store.get_mut(file_id) {
        Some(state) => state,
        None => {
            cleanup();
            return HttpResponse::NotFound().json(error_response("Upload session not found"));
        }
    };

    // 检查分块顺序
    if chunk_index != state.received_chunks {
        cleanup();
        return HttpResponse::BadRequest().json(error_response(&format!(
            "Expected chunk {}, got {}",
            state.received_chunks, chunk_index
        )));
    }

    // 写入文件（追加模式）
    let temp_path = get_temp_path(file_id);
    let mut file = match OpenOptions::new()
        .create(true)
        .append(true)
        .open(&temp_path)
    {
        Ok(file) => file,
        Err(e) => {
            cleanup();
            return HttpResponse::InternalServerError()
                .json(error_response(&format!("Failed to open temp file: {}", e)));
        }
    };

    if let Err(e) = file.write_all(&chunk) {
        cleanup();
        return HttpResponse::InternalServerError()
            .json(error_response(&format!("Failed to write chunk: {}", e)));
    }

    state.received_chunks += 1;
    state.received_bytes += chunk.len();

    HttpResponse::Ok().json(json!({
        "success": true,
        "received_bytes": state.received_bytes
    }))
}

#[post("/complete")]
async fn handle_chunked_complete(
    req: HttpRequest,
    store: web::Data<UploadStore>,
    state: web::Data<SharedState>,
) -> HttpResponse {
    // 验证头信息
    let file_id = match req.headers().get("X-File-ID") {
        Some(header) => match header.to_str() {
            Ok(id) => id,
            Err(_) => {
                return HttpResponse::BadRequest().json(error_response("Invalid X-File-ID header"))
            }
        },
        None => return HttpResponse::BadRequest().json(error_response("Missing X-File-ID header")),
    };

    // 确保最终清理临时文件
    let cleanup = || cleanup_temp_file(file_id);

    let expected_length = match req.headers().get("X-Expected-Length") {
        Some(header) => match header.to_str() {
            Ok(len) => match len.parse::<usize>() {
                Ok(n) => n,
                Err(_) => {
                    cleanup();
                    return HttpResponse::BadRequest()
                        .json(error_response("Invalid X-Expected-Length"));
                }
            },
            Err(_) => {
                cleanup();
                return HttpResponse::BadRequest()
                    .json(error_response("Invalid X-Expected-Length header"));
            }
        },
        None => {
            cleanup();
            return HttpResponse::BadRequest()
                .json(error_response("Missing X-Expected-Length header"));
        }
    };

    let expected_checksum = match req.headers().get("X-Checksum") {
        Some(header) => match header.to_str() {
            Ok(checksum) => checksum,
            Err(_) => {
                cleanup();
                return HttpResponse::BadRequest()
                    .json(error_response("Invalid X-Checksum header"));
            }
        },
        None => {
            cleanup();
            return HttpResponse::BadRequest().json(error_response("Missing X-Checksum header"));
        }
    };

    // 获取上传状态
    let (meta, temp_path) = {
        let mut store = match store.lock() {
            Ok(store) => store,
            Err(_) => {
                cleanup();
                return HttpResponse::InternalServerError()
                    .json(error_response("Failed to lock store"));
            }
        };

        let state = match store.remove(file_id) {
            Some(state) => state,
            None => {
                cleanup();
                return HttpResponse::NotFound().json(error_response("Upload session not found"));
            }
        };
        (state.meta, get_temp_path(file_id))
    };

    // 验证文件大小
    let actual_length = match fs::metadata(&temp_path) {
        Ok(meta) => meta.len() as usize,
        Err(e) => {
            cleanup();
            return HttpResponse::InternalServerError().json(error_response(&format!(
                "Failed to get file metadata: {}",
                e
            )));
        }
    };

    if actual_length != expected_length {
        cleanup();
        return HttpResponse::BadRequest().json(error_response(&format!(
            "Size mismatch (expected: {}, actual: {})",
            expected_length, actual_length
        )));
    }

    // 验证校验和
    let file_content = match fs::read(&temp_path) {
        Ok(content) => content,
        Err(e) => {
            cleanup();
            return HttpResponse::InternalServerError()
                .json(error_response(&format!("Failed to read file: {}", e)));
        }
    };

    let actual_checksum = calculate_sha256(&file_content);
    if actual_checksum != expected_checksum {
        cleanup();
        return HttpResponse::BadRequest().json(error_response(&format!(
            "Checksum mismatch (expected: {}, actual: {})",
            expected_checksum, actual_checksum
        )));
    }

    // 保存最终文件
    let save_path = match save_final_file(&meta, &file_content) {
        Ok(path) => path,
        Err(e) => {
            cleanup();
            return e;
        }
    };

    info!("Saved file: {}", save_path.to_string_lossy());
    // 处理文件信息并触发事件
    if let Some(app_handle) = state.get_app_handle() {
        if let Err(e) =
            process_and_emit_browser_saved_html_info(&meta, &file_content, &save_path, &app_handle)
        {
            error!("Failed to emit event: {}", e);
        }
    } else {
        warn!("AppHandle not initialized, skipping event emission");
    }

    // 清理临时文件
    cleanup();

    HttpResponse::Ok().json(json!({
        "success": true,
        "saved_path": save_path.to_string_lossy().to_string()
    }))
}

#[post("/save_meta")]
async fn handle_chunked_meta(
    req: web::Json<ChunkedMeta>,
    store: web::Data<UploadStore>,
) -> impl Responder {
    // 验证必填字段
    if req.url.is_empty() {
        info!("Rejected meta: Empty URL");
        return HttpResponse::BadRequest().json(error_response("URL cannot be empty"));
    }

    if req.title.is_empty() {
        info!("Rejected meta: Empty title");
        return HttpResponse::BadRequest().json(error_response("Title cannot be empty"));
    }

    if req.file_id.is_empty() {
        info!("Rejected meta: Empty file_id");
        return HttpResponse::BadRequest().json(error_response("File ID cannot be empty"));
    }

    if req.total_chunks == 0 {
        info!("Rejected meta: Invalid total_chunks");
        return HttpResponse::BadRequest()
            .json(error_response("Total chunks must be greater than 0"));
    }

    if req.original_length == 0 {
        info!("Rejected meta: Invalid original_length");
        return HttpResponse::BadRequest()
            .json(error_response("Original length must be greater than 0"));
    }

    if req.mode != "fullPage" && req.mode != "partial" {
        info!("Rejected meta: Invalid mode {}", req.mode);
        return HttpResponse::BadRequest().json(error_response(
            "Invalid mode, must be 'fullPage' or 'partial'",
        ));
    }

    info!("Received valid chunked meta for file: {}", req.file_id);

    // 创建临时目录
    let temp_dir = PathBuf::from(get_libraries_root()).join("temp_uploads");
    if let Err(e) = fs::create_dir_all(&temp_dir) {
        error!("Failed to create temp dir: {}", e);
        return HttpResponse::InternalServerError()
            .json(error_response(&format!("Failed to create temp dir: {}", e)));
    }

    // 清理可能存在的旧临时文件
    let temp_path = temp_dir.join(&req.file_id);
    if temp_path.exists() {
        if let Err(e) = fs::remove_file(&temp_path) {
            error!(
                "Failed to cleanup existing temp file {}: {}",
                temp_path.display(),
                e
            );
            return HttpResponse::InternalServerError().json(error_response(&format!(
                "Failed to cleanup existing temp file: {}",
                e
            )));
        }
    }

    // 创建新临时文件
    let temp_file = match OpenOptions::new().create(true).write(true).open(&temp_path) {
        Ok(file) => file,
        Err(e) => {
            error!("Failed to create temp file {}: {}", temp_path.display(), e);
            return HttpResponse::InternalServerError().json(error_response(&format!(
                "Failed to create temp file: {}",
                e
            )));
        }
    };

    // 存储上传状态
    let mut store = store.lock().unwrap();
    store.insert(
        req.file_id.clone(),
        UploadState {
            meta: req.into_inner(),
            received_chunks: 0,
            temp_file: Some(temp_file),
            received_bytes: 0,
        },
    );

    HttpResponse::Ok().json(success_response("Meta received", None))
}

// 辅助函数保持不变
fn prepare_save_dir() -> Result<PathBuf, HttpResponse> {
    let save_dir = PathBuf::from(get_libraries_root()).join("collect_html");
    fs::create_dir_all(&save_dir).map_err(|e| {
        error!("Failed to create save dir: {}", e);
        HttpResponse::InternalServerError().json(error_response(&format!(
            "Failed to create directory: {}",
            e
        )))
    })?;
    Ok(save_dir)
}

fn save_html_content(
    req: &ExtensionRequest,
    save_dir: &Path,
) -> Result<(String, std::io::Result<()>), HttpResponse> {
    let sanitized_title = sanitize_filename(&req.title);

    if req.mode == "fullPage" {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let filename = format!("{}_{}.html", sanitized_title, timestamp);
        let raw_path = save_dir.join(&filename);
        let result = File::create(&raw_path).and_then(|mut f| f.write_all(req.html.as_bytes()));
        Ok((filename, result))
    } else {
        let mut hasher = DefaultHasher::new();
        req.url.hash(&mut hasher);
        let url_hash = hasher.finish();
        let filename = format!("{}_{}.html", sanitized_title, url_hash);
        let raw_path = save_dir.join(&filename);
        let html_fragment = format!(
            "<div data-guaxi-timestamp=\"{}\">\n{}\n</div>\n",
            req.timestamp, req.html
        );
        let result = if raw_path.exists() {
            OpenOptions::new()
                .append(true)
                .open(&raw_path)
                .and_then(|mut f| f.write_all(html_fragment.as_bytes()))
        } else {
            File::create(&raw_path).and_then(|mut f| f.write_all(html_fragment.as_bytes()))
        };
        Ok((filename, result))
    }
}

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

fn success_response(message: &str, path: Option<PathBuf>) -> ExtensionResponse {
    ExtensionResponse {
        success: true,
        message: message.to_string(),
        saved_path: path.map(|p| p.to_string_lossy().into_owned()),
    }
}

fn error_response(message: &str) -> ExtensionResponse {
    ExtensionResponse {
        success: false,
        message: message.to_string(),
        saved_path: None,
    }
}

// 计算SHA-256校验和
fn calculate_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

// 获取临时文件路径
fn get_temp_path(file_id: &str) -> PathBuf {
    PathBuf::from(get_libraries_root())
        .join("temp_uploads")
        .join(file_id)
}

// 保存最终文件
fn save_final_file(meta: &ChunkedMeta, content: &[u8]) -> Result<PathBuf, HttpResponse> {
    let save_dir = PathBuf::from(get_libraries_root()).join("collect_html");
    if let Err(e) = fs::create_dir_all(&save_dir) {
        return Err(
            HttpResponse::InternalServerError().json(error_response(&format!(
                "Failed to create save directory: {}",
                e
            ))),
        );
    }

    let filename = format!(
        "{}_{}.html",
        sanitize_filename(&meta.title),
        Local::now().format("%Y%m%d_%H%M%S")
    );

    let path = save_dir.join(filename);

    // 使用原子写入方式
    let temp_path = path.with_extension("tmp");

    // 1. 先写入临时文件
    if let Err(e) = fs::write(&temp_path, content) {
        let _ = fs::remove_file(&temp_path); // 尝试清理
        return Err(HttpResponse::InternalServerError()
            .json(error_response(&format!("Failed to save file: {}", e))));
    }

    // 2. 重命名为最终文件
    if let Err(e) = fs::rename(&temp_path, &path) {
        let _ = fs::remove_file(&temp_path); // 尝试清理
        return Err(HttpResponse::InternalServerError()
            .json(error_response(&format!("Failed to rename file: {}", e))));
    }

    Ok(path)
}

// 清理临时文件的函数
fn cleanup_temp_file(file_id: &str) {
    let temp_path = get_temp_path(file_id);
    if temp_path.exists() {
        if let Err(e) = fs::remove_file(&temp_path) {
            error!(
                "Failed to clean up temp file {}: {}",
                temp_path.display(),
                e
            );
        }
    }
}

// 处理html文件信息并触发事件
fn process_and_emit_browser_saved_html_info(
    meta: &ChunkedMeta,
    file_content: &[u8],
    save_path: &Path,
    app_handle: &AppHandle,
) -> Result<(), String> {
    info!(
        "Processing and emitting browser saved HTML info for file: {}",
        save_path.display()
    );

    // 获取文件元数据
    let file_metadata = match fs::metadata(save_path) {
        Ok(meta) => meta,
        Err(e) => {
            error!("Failed to get file metadata: {}", e);
            return Err(format!("Failed to get file metadata: {}", e));
        }
    };

    // 构建文件信息结构
    let file_info = BrowserSavedHtmlInfo {
        file_name: save_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        file_path: save_path.to_string_lossy().into_owned(),
        create_time: file_metadata
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0),
        modify_time: file_metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0),
        file_type: "html".to_string(),
        knowledge_repo: "default".to_string(),
        tags: vec!["webpage".to_string()],
        description: meta.title.clone(),
        file_size: file_metadata.len(),
        file_repo: "default".to_string(),
    };

    info!("File info: {:?}", file_info);
    // 触发 Tauri 事件 - 使用 AppHandle 的 emit_all 方法
    app_handle
        .emit(
            "add_browser_saved_html_file_event",
            serde_json::to_value(&file_info)
                .map_err(|e| format!("Failed to serialize file info: {}", e))?,
        )
        .map_err(|e| format!("Failed to emit event: {}", e))?;
    info!("Emitted add_browser_saved_html_file_event with file info: {:?}", file_info);
    Ok(())
}


// GuaxiWebClipper普通保存接口
#[post("/webclipper/save")]
async fn handle_webclipper_save(req: web::Json<GuaxiWebClipperRequest>, state: web::Data<SharedState>) -> impl Responder {
    info!("Received GuaxiWebClipper data for URL: {}", req.url);
    info!("Title: {}, Description: {}, Mode: {}, knowledgeBase: {}, fileRepo: {}, tags: {:?}, timestamp: {}", 
        req.title, req.description, req.mode, req.knowledge_base, req.file_repo, req.tags, req.timestamp);

    if req.content.is_empty() {
        return HttpResponse::BadRequest().json(error_response("Empty content"));
    }

    let save_dir = match prepare_webclipper_save_dir() {
        Ok(dir) => dir,
        Err(e) => return e,
    };

    let (filename, result) = match save_webclipper_content(&req, &save_dir) {
        Ok(res) => res,
        Err(e) => return e,
    };

    let save_path = save_dir.join(&filename);
    let save_path_ref = save_path.as_path();
    // 处理文件信息并触发事件
    if let Some(app_handle) = state.get_app_handle() {
        if let Err(e) = process_and_emit_webclipper_info_for_md(req, &save_path_ref, &app_handle) {
            error!("Failed to emit event: {}", e);
        }
    }

    match result {
        Ok(_) => HttpResponse::Ok().json(success_response(
            "Content saved successfully",
            Some(save_dir.join(filename)),
        )),
        Err(e) => HttpResponse::InternalServerError()
            .json(error_response(&format!("Failed to save content: {}", e))),
    }
}

// GuaxiWebClipper元数据接口
#[post("/webclipper/save_meta")]
async fn handle_webclipper_meta(
    req: web::Json<GuaxiWebClipperChunkedMeta>,
    store: web::Data<UploadStore>,
) -> impl Responder {
    // 验证必填字段
    if req.url.is_empty() {
        return HttpResponse::BadRequest().json(error_response("URL cannot be empty"));
    }
    if req.title.is_empty() {
        return HttpResponse::BadRequest().json(error_response("Title cannot be empty"));
    }
    if req.file_id.is_empty() {
        return HttpResponse::BadRequest().json(error_response("File ID cannot be empty"));
    }
    if req.total_chunks == 0 || req.original_length == 0 {
        return HttpResponse::BadRequest().json(error_response("Invalid chunk parameters"));
    }

    // 创建临时目录
    let temp_dir = PathBuf::from(get_libraries_root()).join("temp_webclipper");
    if let Err(e) = fs::create_dir_all(&temp_dir) {
        return HttpResponse::InternalServerError()
            .json(error_response(&format!("Failed to create temp dir: {}", e)));
    }

    // 创建临时文件
    let temp_path = temp_dir.join(&req.file_id);
    let temp_file = match OpenOptions::new().create(true).write(true).open(&temp_path) {
        Ok(file) => file,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(error_response(&format!("Failed to create temp file: {}", e)));
        }
    };

    // 存储上传状态
    let mut store = store.lock().unwrap();
    store.insert(
        req.file_id.clone(),
        UploadState {
            meta: ChunkedMeta {
                url: req.url.clone(),
                title: req.title.clone(),
                timestamp: req.timestamp.clone(),
                mode: req.mode.clone(),
                file_id: req.file_id.clone(),
                total_chunks: req.total_chunks,
                original_length: req.original_length,
            },
            received_chunks: 0,
            temp_file: Some(temp_file),
            received_bytes: 0,
        },
    );

    HttpResponse::Ok().json(success_response("Meta received", None))
}

// GuaxiWebClipper分块上传接口
#[post("/webclipper/save_chunk")]
async fn handle_webclipper_chunk(
    req: HttpRequest,
    chunk: web::Bytes,
    store: web::Data<UploadStore>,
) -> HttpResponse {
    // 验证头信息
    let file_id = match req.headers().get("X-File-ID") {
        Some(header) => header.to_str().unwrap_or(""),
        None => return HttpResponse::BadRequest().json(error_response("Missing X-File-ID header")),
    };

    let chunk_index: usize = match req.headers().get("X-Chunk-Index").and_then(|h| h.to_str().ok()) {
        Some(idx) => idx.parse().unwrap_or(0),
        None => return HttpResponse::BadRequest().json(error_response("Missing X-Chunk-Index header")),
    };

    // 获取上传状态
    let mut store = match store.lock() {
        Ok(store) => store,
        Err(_) => return HttpResponse::InternalServerError().json(error_response("Failed to lock store")),
    };

    let state = match store.get_mut(file_id) {
        Some(state) => state,
        None => return HttpResponse::NotFound().json(error_response("Upload session not found")),
    };

    // 检查分块顺序
    if chunk_index != state.received_chunks {
        return HttpResponse::BadRequest().json(error_response(&format!(
            "Expected chunk {}, got {}",
            state.received_chunks, chunk_index
        )));
    }

    // 写入文件
    let temp_path = get_webclipper_temp_path(file_id);
    if let Err(e) = fs::write(&temp_path, &chunk) {
        return HttpResponse::InternalServerError()
            .json(error_response(&format!("Failed to write chunk: {}", e)));
    }

    state.received_chunks += 1;
    state.received_bytes += chunk.len();

    HttpResponse::Ok().json(json!({
        "success": true,
        "received_bytes": state.received_bytes
    }))
}

// GuaxiWebClipper完成上传接口
#[post("/webclipper/complete")]
async fn handle_webclipper_complete(
    req: HttpRequest,
    store: web::Data<UploadStore>,
    state: web::Data<SharedState>,
) -> HttpResponse {
    // 验证头信息
    let file_id = match req.headers().get("X-File-ID") {
        Some(header) => header.to_str().unwrap_or(""),
        None => return HttpResponse::BadRequest().json(error_response("Missing X-File-ID header")),
    };

    let expected_length: usize = match req.headers().get("X-Expected-Length").and_then(|h| h.to_str().ok()) {
        Some(len) => len.parse().unwrap_or(0),
        None => return HttpResponse::BadRequest().json(error_response("Missing X-Expected-Length header")),
    };

    let expected_checksum = match req.headers().get("X-Checksum") {
        Some(header) => header.to_str().unwrap_or(""),
        None => return HttpResponse::BadRequest().json(error_response("Missing X-Checksum header")),
    };

    // 获取上传状态
    let (meta, temp_path) = {
        let mut store = match store.lock() {
            Ok(store) => store,
            Err(_) => return HttpResponse::InternalServerError().json(error_response("Failed to lock store")),
        };

        let state = match store.remove(file_id) {
            Some(state) => state,
            None => return HttpResponse::NotFound().json(error_response("Upload session not found")),
        };
        (state.meta, get_webclipper_temp_path(file_id))
    };

    // 验证文件
    let file_content = match validate_webclipper_file(&temp_path, expected_length, expected_checksum) {
        Ok(content) => content,
        Err(res) => return res,
    };

    // 保存最终文件
    let save_path = match save_webclipper_final_file(&meta, &file_content) {
        Ok(path) => path,
        Err(e) => return e,
    };

    // 清理临时文件
    let _ = fs::remove_file(&temp_path);

    // 处理文件信息并触发事件
    if let Some(app_handle) = state.get_app_handle() {
        if let Err(e) = process_and_emit_webclipper_info(&meta, &file_content, &save_path, &app_handle) {
            error!("Failed to emit event: {}", e);
        }
    }

    HttpResponse::Ok().json(ExtensionResponse {
        success: true,
        message: "File saved successfully".to_string(),
        saved_path: Some(save_path.to_string_lossy().into_owned()),
    })
}

// GuaxiWebClipper专用辅助函数
fn prepare_webclipper_save_dir() -> Result<PathBuf, HttpResponse> {
    let save_dir = PathBuf::from(get_libraries_root()).join("collect_webclipper");
    fs::create_dir_all(&save_dir).map_err(|e| {
        HttpResponse::InternalServerError().json(error_response(&format!(
            "Failed to create directory: {}", e
        )))
    })?;
    Ok(save_dir)
}

fn save_webclipper_content(
    req: &GuaxiWebClipperRequest,
    save_dir: &Path,
) -> Result<(String, std::io::Result<()>), HttpResponse> {
    let sanitized_title = sanitize_filename(&req.title);
    let filename = format!("{}_{}.md", sanitized_title, Local::now().format("%Y%m%d_%H%M%S"));
    let path = save_dir.join(&filename);
    
    let result = File::create(&path).and_then(|mut f| f.write_all(req.content.as_bytes()));
    Ok((filename, result))
}

fn get_webclipper_temp_path(file_id: &str) -> PathBuf {
    PathBuf::from(get_libraries_root())
        .join("temp_webclipper")
        .join(file_id)
}

fn validate_webclipper_file(
    path: &Path,
    expected_length: usize,
    expected_checksum: &str,
) -> Result<Vec<u8>, HttpResponse> {
    let metadata = fs::metadata(path).map_err(|e| {
        HttpResponse::InternalServerError().json(error_response(&format!(
            "Failed to get file metadata: {}", e
        )))
    })?;

    if metadata.len() as usize != expected_length {
        return Err(HttpResponse::BadRequest().json(error_response(&format!(
            "Size mismatch: expected {}, got {}", expected_length, metadata.len()
        ))));
    }

    let content = fs::read(path).map_err(|e| {
        HttpResponse::InternalServerError().json(error_response(&format!(
            "Failed to read file: {}", e
        )))
    })?;

    let actual_checksum = calculate_sha256(&content);
    if actual_checksum != expected_checksum {
        return Err(HttpResponse::BadRequest().json(error_response(&format!(
            "Checksum mismatch: expected {}, got {}", expected_checksum, actual_checksum
        ))));
    }

    Ok(content)
}

fn save_webclipper_final_file(meta: &ChunkedMeta, content: &[u8]) -> Result<PathBuf, HttpResponse> {
    let save_dir = PathBuf::from(get_libraries_root()).join("collect_webclipper");
    fs::create_dir_all(&save_dir).map_err(|e| {
        HttpResponse::InternalServerError().json(error_response(&format!(
            "Failed to create save dir: {}", e
        )))
    })?;

    let filename = format!(
        "{}_{}.html",
        sanitize_filename(&meta.title),
        Local::now().format("%Y%m%d_%H%M%S")
    );
    let path = save_dir.join(filename);

    // 原子写入
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, content).map_err(|e| {
        let _ = fs::remove_file(&temp_path);
        HttpResponse::InternalServerError().json(error_response(&format!(
            "Failed to write file: {}", e
        )))
    })?;

    fs::rename(&temp_path, &path).map_err(|e| {
        let _ = fs::remove_file(&temp_path);
        HttpResponse::InternalServerError().json(error_response(&format!(
            "Failed to rename file: {}", e
        )))
    })?;

    Ok(path)
}

fn process_and_emit_webclipper_info(
    meta: &ChunkedMeta,
    file_content: &[u8],
    save_path: &Path,
    app_handle: &AppHandle,
) -> Result<(), String> {
    let file_metadata = fs::metadata(save_path).map_err(|e| format!("Failed to get file metadata: {}", e))?;

    let file_info = BrowserSavedHtmlInfo {
        file_name: save_path.file_name().unwrap().to_string_lossy().into_owned(),
        file_path: save_path.to_string_lossy().into_owned(),
        create_time: file_metadata.created().ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0),
        modify_time: file_metadata.modified().ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0),
        file_type: "md".to_string(),
        knowledge_repo: "default".to_string(),
        tags: vec!["webpage".to_string()],
        description: meta.title.clone(),
        file_size: file_metadata.len(),
        file_repo: "default".to_string(),
    };

    app_handle
        .emit(
            "add_browser_saved_html_file_event",
            serde_json::to_value(&file_info)
                .map_err(|e| format!("Failed to serialize file info: {}", e))?,
        )
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

fn process_and_emit_webclipper_info_for_md(
    req: web::Json<GuaxiWebClipperRequest>,
    save_path: &Path,
    app_handle: &AppHandle,
) -> Result<(), String> {
    let file_metadata = fs::metadata(save_path).map_err(|e| format!("Failed to get file metadata: {}", e))?;

    let file_info = GuaxiWebClipperInfo {
        file_name: save_path.file_name().unwrap().to_string_lossy().into_owned(),
        file_path: save_path.to_string_lossy().into_owned(),
        create_time: req.timestamp.clone(),
        modify_time: req.timestamp.clone(),
        file_type: "md".to_string(),
        knowledge_repo: req.knowledge_base.clone(),
        tags: req.tags.clone(),
        description: req.description.clone(),
        file_size: file_metadata.len(),
        file_repo: req.file_repo.clone(),
    };

    app_handle
        .emit(
            "add_browser_saved_html_file_event",
            serde_json::to_value(&file_info)
                .map_err(|e| format!("Failed to serialize file info: {}", e))?,
        )
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    info!("Emitted add_browser_saved_html_file_event with file info: {:?}", file_info);

    Ok(())
}