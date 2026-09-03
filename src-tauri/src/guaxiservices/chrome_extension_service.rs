use crate::commands::repoview::repofileproc::add_web_clipper_import_file;
use crate::commands::repoview::{add_imported_file, ImportedFileInput};
use crate::commands::settingview::get_libraries_root;
use crate::commands::convert::task_store::{self, NewConversionTask};
use crate::state::SharedState;
use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use base64::{engine::general_purpose, Engine as _};
use chrono::Local;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, CACHE_CONTROL, PRAGMA, UPGRADE_INSECURE_REQUESTS};
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
use uuid::Uuid;


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
pub struct CaptureImportRequest {
    #[serde(default)]
    pub title: String,
    #[serde(default, alias = "sourceUrl")]
    pub source_url: String,
    #[serde(default, alias = "sourceApp")]
    pub source_app: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, alias = "knowledgeBase")]
    pub knowledge_repo: String,
    #[serde(default, alias = "fileRepo")]
    pub file_repo: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, alias = "createdAt", alias = "timestamp")]
    pub created_at: String,
    #[serde(default)]
    pub markdown: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub assets: Vec<CaptureAsset>,
}

#[derive(Debug, Deserialize)]
pub struct CaptureAsset {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mime: String,
    #[serde(default, alias = "dataBase64")]
    pub data_base64: String,
}

#[derive(Serialize)]
pub struct CaptureImportResponse {
    success: bool,
    message: String,
    file_id: Option<i32>,
    source_path: Option<String>,
    library: Option<String>,
    knowledge_repo: Option<String>,
    asset_count: usize,
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
    #[allow(dead_code)]
    #[serde(default)]
    pub knowledge_base: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub tags: Vec<String>,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub timestamp: String,
    pub mode: String,
}

// 分块上传元数据（使用rename兼容两种命名格式）
#[derive(Debug, Deserialize)]
struct ChunkedMeta {
    pub url: String,
    pub title: String,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
                .service(handle_webclipper_complete)
                .service(handle_capture_import),
        );
}

// 设计说明：
// 手机端 Guaxi Capture 只需要写入 Capture Import API，不应该为了移动端同步而把
// PC 端完整的 8888 后端服务暴露到局域网。这里提供一个窄作用域配置，仅注册
// `/extension/capture/import`，供独立的局域网监听端口使用。
pub fn capture_import_config(cfg: &mut web::ServiceConfig, state: web::Data<SharedState>) {
    cfg.app_data(state)
        .service(web::scope("/extension").service(handle_capture_import));
}

#[post("/capture/import")]
async fn handle_capture_import(
    req: web::Json<CaptureImportRequest>,
    state: web::Data<SharedState>,
) -> HttpResponse {
    let req = req.into_inner();
    let title = normalize_capture_title(&req);
    let knowledge_repo = default_if_empty(&req.knowledge_repo, "default");
    let file_repo = default_if_empty(&req.file_repo, "default");
    let created_at = default_if_empty(
        &req.created_at,
        &Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    );
    let tags = normalize_capture_tags(&req.tags, &req.source_app);

    info!(
        "[capture_import] received title='{}' source_app='{}' knowledge_repo='{}' file_repo='{}' asset_count={}",
        title,
        req.source_app,
        knowledge_repo,
        file_repo,
        req.assets.len()
    );

    let prepared_assets = match prepare_capture_assets(&req.assets) {
        Ok(assets) => assets,
        Err(e) => {
            warn!("[capture_import] asset decode failed: {}", e);
            return HttpResponse::BadRequest().json(capture_error_response(&e));
        }
    };

    let capture_dir = PathBuf::from(get_libraries_root()).join("collect_capture");
    if let Err(e) = fs::create_dir_all(&capture_dir) {
        error!("[capture_import] failed to create collect_capture dir: {}", e);
        return HttpResponse::InternalServerError().json(capture_error_response(&format!(
            "Failed to create capture directory: {}",
            e
        )));
    }

    let file_stem = build_capture_file_stem(&title);
    let filename = format!(
        "{}_{}.md",
        file_stem,
        Local::now().format("%Y%m%d_%H%M%S")
    );
    let source_path = capture_dir.join(&filename);
    let source_path_string = source_path.to_string_lossy().to_string();
    let capture_output_dir = capture_dir.to_string_lossy().to_string();

    let app_handle = match state.get_app_handle() {
        Some(handle) => handle,
        None => {
            error!("[capture_import] AppHandle is not initialized");
            return HttpResponse::InternalServerError().json(capture_error_response(
                "AppHandle is not initialized",
            ));
        }
    };

    let capture_task_id = start_capture_import_task(
        &app_handle,
        &title,
        &req,
        &source_path_string,
        &capture_output_dir,
    );

    let body = match capture_body(&req) {
        Some(body) => {
            update_capture_import_task(
                &app_handle,
                capture_task_id,
                35,
                "已收到正文，准备写入文件库",
            );
            body
        }
        None if !req.source_url.trim().is_empty() => {
            update_capture_import_task(
                &app_handle,
                capture_task_id,
                15,
                "正在抓取分享 URL 正文",
            );
            match build_url_capture_body(&req, &title).await {
                Ok(body) => {
                    update_capture_import_task(
                        &app_handle,
                        capture_task_id,
                        65,
                        "URL 正文已转换为 Markdown",
                    );
                    body
                }
                Err(e) => {
                    warn!("[capture_import] url extraction failed: {}", e);
                    update_capture_import_task(
                        &app_handle,
                        capture_task_id,
                        45,
                        "URL 抽取失败，保留来源链接等待后续处理",
                    );
                    build_pc_side_processing_body(&req, &prepared_assets, Some(&e))
                }
            }
        }
        None if should_accept_pc_side_processing(&req, &prepared_assets) => {
            update_capture_import_task(
                &app_handle,
                capture_task_id,
                35,
                "已进入 PC 端附件处理队列",
            );
            build_pc_side_processing_body(&req, &prepared_assets, None)
        }
        None => {
            warn!("[capture_import] rejected empty capture for title='{}'", title);
            fail_capture_import_task(
                &app_handle,
                capture_task_id,
                "markdown/content/text, sourceUrl, or assets cannot all be empty",
            );
            return HttpResponse::BadRequest().json(capture_error_response(
                "markdown/content/text, sourceUrl, or assets cannot all be empty",
            ));
        }
    };

    let markdown = build_capture_markdown(&req, &title, &created_at, &tags, &body, &prepared_assets);

    if let Err(e) = write_capture_markdown_atomically(&source_path, &markdown) {
        error!("[capture_import] failed to write markdown: {}", e);
        fail_capture_import_task(&app_handle, capture_task_id, &format!("Failed to write markdown: {}", e));
        return HttpResponse::InternalServerError().json(capture_error_response(&format!(
            "Failed to write markdown: {}",
            e
        )));
    }

    let file_size = fs::metadata(&source_path).map(|m| m.len()).unwrap_or(0);
    update_capture_import_task(
        &app_handle,
        capture_task_id,
        78,
        "采集 Markdown 已写入暂存区",
    );
    // 认知层：采集内容同时进感知收件箱，从此参与召回、矛盾检测与简报，
    // 而不是只静静躺在 Inbox 里等人来整理。
    crate::cognition::hook_percept(
        "capture",
        &title,
        &body,
        req.source_url.trim(),
        req.source_app.trim(),
        req.knowledge_repo.trim(),
    );
    info!(
        "[capture_import] saved staging markdown path='{}' size={}",
        source_path_string, file_size
    );

    let imported_stem = Path::new(&filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&file_stem)
        .to_string();

    if let Err(e) = add_web_clipper_import_file(file_repo.clone(), source_path_string.clone()) {
        error!("[capture_import] file library import failed: {}", e);
        fail_capture_import_task(&app_handle, capture_task_id, &format!("Failed to import file into library: {}", e));
        return HttpResponse::InternalServerError().json(capture_error_response(&format!(
            "Failed to import file into library: {}",
            e
        )));
    }

    if let Err(e) = write_capture_assets_to_library(&file_repo, &imported_stem, &prepared_assets) {
        error!("[capture_import] asset library import failed: {}", e);
        fail_capture_import_task(&app_handle, capture_task_id, &format!("Failed to import capture assets: {}", e));
        return HttpResponse::InternalServerError().json(capture_error_response(&format!(
            "Failed to import capture assets: {}",
            e
        )));
    }

    let file_id = match add_imported_file(
        app_handle.clone(),
        ImportedFileInput::new(
            filename.clone(),
            default_if_empty(&req.description, &title),
            file_repo.clone(),
            tags,
            "md".to_string(),
            created_at,
            file_size,
            knowledge_repo.clone(),
        ),
    ) {
        Ok(id) => id,
        Err(e) => {
            error!("[capture_import] imported_files insert failed: {}", e);
            fail_capture_import_task(&app_handle, capture_task_id, &format!("Failed to add imported file record: {}", e));
            return HttpResponse::InternalServerError().json(capture_error_response(&format!(
                "Failed to add imported file record: {}",
                e
            )));
        }
    };

    let imported_path = PathBuf::from(get_libraries_root())
        .join("Libraries")
        .join(&file_repo)
        .join(&imported_stem)
        .join("md")
        .join(&filename);

    info!(
        "[capture_import] completed file_id={} imported_path='{}'",
        file_id,
        imported_path.display()
    );
    finish_capture_import_task(&app_handle, capture_task_id, &imported_path.to_string_lossy());

    HttpResponse::Ok().json(CaptureImportResponse {
        success: true,
        message: "Capture imported successfully".to_string(),
        file_id: Some(file_id),
        source_path: Some(imported_path.to_string_lossy().to_string()),
        library: Some(file_repo),
        knowledge_repo: Some(knowledge_repo),
        asset_count: prepared_assets.len(),
    })
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
struct PreparedCaptureAsset {
    file_name: String,
    mime: String,
    bytes: Vec<u8>,
}

fn capture_error_response(message: &str) -> CaptureImportResponse {
    CaptureImportResponse {
        success: false,
        message: message.to_string(),
        file_id: None,
        source_path: None,
        library: None,
        knowledge_repo: None,
        asset_count: 0,
    }
}

fn start_capture_import_task(
    app_handle: &AppHandle,
    title: &str,
    req: &CaptureImportRequest,
    input_path: &str,
    output_dir: &str,
) -> Option<i64> {
    let display_name = if title.trim().is_empty() {
        "Guaxi Capture Import".to_string()
    } else {
        format!("Guaxi Capture - {}", title.trim())
    };
    let task = NewConversionTask {
        uuid: Uuid::new_v4().to_string(),
        imported_file_id: None,
        file_name: display_name,
        input_path: if req.source_url.trim().is_empty() {
            input_path.to_string()
        } else {
            req.source_url.trim().to_string()
        },
        output_dir: output_dir.to_string(),
        expected_md_path: Some(input_path.to_string()),
        requested_engine: "capture".to_string(),
        max_attempts: 0,
        priority: 5,
    };

    match task_store::insert_task(&task) {
        Ok(id) => {
            let _ = task_store::set_resolved_engine(id, "capture", None);
            let _ = task_store::update_progress(id, 5, 100, Some("capture-import"), Some("收到移动端采集任务"));
            emit_capture_import_task(app_handle, id);
            info!(
                "[capture_import] task created task_id={} title='{}' source_url='{}'",
                id,
                title,
                req.source_url
            );
            Some(id)
        }
        Err(e) => {
            warn!("[capture_import] create task failed: {}", e);
            None
        }
    }
}

fn update_capture_import_task(
    app_handle: &AppHandle,
    task_id: Option<i64>,
    current: i64,
    message: &str,
) {
    let Some(id) = task_id else {
        return;
    };
    if let Err(e) = task_store::update_progress(id, current, 100, Some("capture-import"), Some(message)) {
        warn!("[capture_import] update task progress failed task_id={} error={}", id, e);
        return;
    }
    emit_capture_import_task(app_handle, id);
}

fn fail_capture_import_task(app_handle: &AppHandle, task_id: Option<i64>, error: &str) {
    let Some(id) = task_id else {
        return;
    };
    let _ = task_store::mark_failed(id, error);
    emit_capture_import_task(app_handle, id);
}

fn finish_capture_import_task(app_handle: &AppHandle, task_id: Option<i64>, md_path: &str) {
    let Some(id) = task_id else {
        return;
    };
    let _ = task_store::mark_succeeded(id, Some(md_path));
    emit_capture_import_task(app_handle, id);
}

fn emit_capture_import_task(app_handle: &AppHandle, task_id: i64) {
    if let Ok(Some(task)) = task_store::get_task(task_id) {
        let _ = app_handle.emit("conversion_task_update", &task);
    }
}

async fn build_url_capture_body(
    req: &CaptureImportRequest,
    fallback_title: &str,
) -> Result<String, String> {
    let source_url = req.source_url.trim();
    if source_url.is_empty() {
        return Err("sourceUrl is empty".to_string());
    }

    info!("[capture_import] extracting url body source_url='{}'", source_url);
    let client = build_capture_http_client()?;
    // 用站点 host 作为渲染 profile，复用 Flipboard 侧同站的持久登录会话（如用户曾登录
    // 知乎 / 公众号等），让“分享的公开内容”尽量在无需再次登录的情况下抓到正文。
    let profile = crate::commands::flipboard::render_profile_for_url(source_url);
    let rendered = crate::commands::flipboard_render::fetch_page(
        &client,
        source_url,
        false,
        &profile,
    )
    .await?;
    // 命中登录墙 / 验证码 / 风控限流页：不要把风控话术或错误 JSON 当作正文落库，
    // 直接判为抽取失败，由上层保留原始链接并提示（登录后）稍后重试。
    if rendered.blocked {
        warn!(
            "[capture_import] rendered page looks blocked source_url='{}' mode={}",
            source_url,
            rendered.mode
        );
        return Err("目标页面命中登录墙/风控（可登录后在 PC 端重试），已保留原始链接".to_string());
    }

    let final_url = rendered.final_url.clone();
    let html = rendered.html;
    let base_url = final_url.clone();
    let extracted = tokio::task::spawn_blocking(move || {
        crate::commands::flipboard_extract::extract(&html, &base_url)
    })
    .await
    .map_err(|e| format!("extract task join failed: {e}"))?
    .map_err(|e| format!("extract article failed: {e}"))?;

    // 反爬兜底：即便未被上面判为 blocked，抽取到的“正文”仍可能是风控/错误响应
    // （如知乎限流返回的 {"error":{...}} JSON）。命中则判为失败，保留原始链接。
    if capture_body_looks_blocked(&extracted.markdown)
        || capture_body_looks_blocked(&extracted.plain_text)
    {
        warn!(
            "[capture_import] extracted body looks like anti-bot/error page source_url='{}'",
            source_url
        );
        return Err("抓取到风控/错误响应而非正文，已保留原始链接待登录后重试".to_string());
    }

    let title = extracted
        .title
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(fallback_title)
        .trim();
    let mut body = String::new();
    body.push_str("# ");
    body.push_str(title);
    body.push_str("\n\n");
    body.push_str("> 原文：");
    body.push_str(source_url);
    body.push_str("\n\n");
    if final_url.trim() != source_url {
        body.push_str("> 最终地址：");
        body.push_str(final_url.trim());
        body.push_str("\n\n");
    }
    body.push_str("> 抽取引擎：");
    body.push_str(extracted.engine);
    body.push_str("\n\n");

    if let Some(lead) = extracted.lead_image.as_deref().filter(|s| !s.trim().is_empty()) {
        body.push_str("![](");
        body.push_str(lead);
        body.push_str(")\n\n");
    }

    let markdown = extracted.markdown.trim();
    if !markdown.is_empty() {
        body.push_str(markdown);
        body.push('\n');
    } else if !extracted.plain_text.trim().is_empty() {
        body.push_str(extracted.plain_text.trim());
        body.push('\n');
    } else {
        return Err("article extraction produced empty markdown".to_string());
    }

    info!(
        "[capture_import] url body extracted source_url='{}' markdown_len={} image_count={}",
        source_url,
        body.len(),
        extracted.images.len()
    );
    Ok(body)
}

fn build_capture_http_client() -> Result<reqwest::Client, String> {
    let mut default_headers = HeaderMap::new();
    default_headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"));
    default_headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"));
    default_headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    default_headers.insert(PRAGMA, HeaderValue::from_static("no-cache"));
    default_headers.insert(UPGRADE_INSECURE_REQUESTS, HeaderValue::from_static("1"));

    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0 Safari/537.36 GuaxiCapture/1.0")
        .default_headers(default_headers)
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| format!("build capture http client failed: {e}"))
}

fn default_if_empty(value: &str, default_value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        default_value.to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_capture_title(req: &CaptureImportRequest) -> String {
    let title = req.title.trim();
    if !title.is_empty() {
        return title.to_string();
    }

    let source_url = req.source_url.trim();
    if !source_url.is_empty() {
        return source_url.to_string();
    }

    "未命名采集".to_string()
}

fn capture_body(req: &CaptureImportRequest) -> Option<String> {
    [&req.markdown, &req.content, &req.text]
        .iter()
        .filter_map(|value| value.as_ref())
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn should_accept_pc_side_processing(
    req: &CaptureImportRequest,
    assets: &[PreparedCaptureAsset],
) -> bool {
    !req.source_url.trim().is_empty() || !assets.is_empty()
}

fn build_pc_side_processing_body(
    req: &CaptureImportRequest,
    assets: &[PreparedCaptureAsset],
    extraction_error: Option<&str>,
) -> String {
    // 设计说明：
    // 移动端只负责“采集事实”：URL、标题、平台、图片或文件。网页正文抽取、图片 OCR、
    // 附件落库这些重活交给 PC 端处理。这里先生成一个稳定的待处理 Markdown，
    // 后续可在同一导入链路里接入网页正文转换和 OCR 队列，而不要求手机端手动填正文。
    let mut body = String::new();
    body.push_str("> Capture received from mobile. PC-side conversion is pending.\n\n");

    if !req.source_url.trim().is_empty() {
        body.push_str("## Source\n\n");
        body.push_str(&format!("- URL: {}\n", req.source_url.trim()));
        body.push_str(&format!(
            "- App: {}\n\n",
            default_if_empty(&req.source_app, "mobile")
        ));
    }

    if let Some(error) = extraction_error.filter(|s| !s.trim().is_empty()) {
        body.push_str("## URL 抽取状态\n\n");
        body.push_str("PC 端尝试抓取正文失败，已保留原始 URL，可稍后在 PC 端重试。\n\n");
        body.push_str(&format!("- 错误：{}\n\n", error.trim()));
    }

    if !assets.is_empty() {
        body.push_str("## PC OCR Queue\n\n");
        body.push_str("The attached images should be recognized by the PC OCR pipeline.\n\n");
        for asset in assets {
            body.push_str(&format!("- `{}` ({})\n", asset.file_name, asset.mime));
        }
        body.push('\n');
    }

    if body.trim().is_empty() {
        "Capture received. PC-side conversion is pending.".to_string()
    } else {
        body
    }
}

fn normalize_capture_tags(tags: &[String], source_app: &str) -> Vec<String> {
    let mut normalized = Vec::new();
    normalized.push("capture".to_string());

    let app = source_app.trim();
    if !app.is_empty() {
        normalized.push(app.to_string());
    }

    for tag in tags {
        let tag = tag.trim();
        if !tag.is_empty() && !normalized.iter().any(|item| item == tag) {
            normalized.push(tag.to_string());
        }
    }

    normalized
}

fn build_capture_file_stem(title: &str) -> String {
    // 关键：采集标题多为中文（知乎/公众号等），必须用保留 Unicode 的清洗，否则
    // sanitize_filename 会把中文整段吃成下划线并被 trim 成空串，最终落成无意义的
    // capture_<uuid> 文件名（用户反馈的“无意义字符”正是这种情况）。
    let sanitized = sanitize_capture_filename(title);
    if sanitized.is_empty() {
        format!("capture_{}", Uuid::new_v4().simple())
    } else {
        sanitized
    }
}

/// 采集文件名清洗：保留中英文/数字等可见字符，仅把文件系统非法字符与空白折叠为下划线。
/// 与只保留 ASCII 字母数字的 `sanitize_filename` 不同——那个会把中文标题吃空。
fn sanitize_capture_filename(title: &str) -> String {
    let mut out = String::new();
    let mut last_underscore = false;
    for c in title.trim().chars() {
        let illegal = matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            || c.is_control()
            || c.is_whitespace();
        if illegal {
            if !out.is_empty() && !last_underscore {
                out.push('_');
                last_underscore = true;
            }
        } else {
            out.push(c);
            last_underscore = false;
        }
    }
    let capped: String = out.trim_matches('_').chars().take(80).collect();
    capped.trim_matches('_').to_string()
}

/// 反爬兜底判定：抽取到的“正文”是否其实是风控/错误响应（知乎限流 JSON、登录墙话术等）。
fn capture_body_looks_blocked(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    // 极短且形如 {"error":{...}} 的 JSON 错误体（典型：知乎限流 code 40362）。
    let lower = trimmed.to_lowercase();
    if trimmed.starts_with('{') && lower.contains("\"error\"") && trimmed.len() < 800 {
        return true;
    }

    const SIGNS: &[&str] = &[
        "您当前请求存在异常",
        "暂时限制本次访问",
        "知乎小管家",
        "系统检测到您的网络",
        "请登录后查看",
        "登录后查看",
        "访问过于频繁",
        "操作过于频繁",
        "请求过于频繁",
    ];
    // 风控话术通常出现在很短的错误页里；正文很长时即便偶然包含也不误伤。
    if trimmed.chars().filter(|c| !c.is_whitespace()).count() > 600 {
        return false;
    }
    SIGNS.iter().any(|s| trimmed.contains(s))
}

fn prepare_capture_assets(assets: &[CaptureAsset]) -> Result<Vec<PreparedCaptureAsset>, String> {
    let mut prepared = Vec::new();

    for (index, asset) in assets.iter().enumerate() {
        if asset.data_base64.trim().is_empty() {
            warn!(
                "[capture_import] skip empty asset index={} name='{}'",
                index, asset.name
            );
            continue;
        }

        let payload = strip_data_url_prefix(&asset.data_base64);
        let bytes = general_purpose::STANDARD
            .decode(payload.as_bytes())
            .map_err(|e| format!("asset {} base64 decode failed: {}", index, e))?;

        let file_name = build_capture_asset_file_name(asset, index);
        prepared.push(PreparedCaptureAsset {
            file_name,
            mime: asset.mime.trim().to_string(),
            bytes,
        });
    }

    Ok(prepared)
}

fn strip_data_url_prefix(value: &str) -> &str {
    let trimmed = value.trim();
    match trimmed.find(',') {
        Some(index) if trimmed[..index].contains(";base64") => &trimmed[index + 1..],
        _ => trimmed,
    }
}

fn build_capture_asset_file_name(asset: &CaptureAsset, index: usize) -> String {
    let raw_name = asset.name.trim();
    let extension = Path::new(raw_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_string())
        .or_else(|| extension_from_mime(&asset.mime))
        .unwrap_or_else(|| "bin".to_string());

    let stem = Path::new(raw_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(sanitize_filename)
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| format!("asset_{:03}", index + 1));

    format!("{}.{}", stem, extension.trim_start_matches('.'))
}

fn extension_from_mime(mime: &str) -> Option<String> {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => Some("jpg".to_string()),
        "image/png" => Some("png".to_string()),
        "image/webp" => Some("webp".to_string()),
        "image/gif" => Some("gif".to_string()),
        "application/pdf" => Some("pdf".to_string()),
        _ => None,
    }
}

fn build_capture_markdown(
    req: &CaptureImportRequest,
    title: &str,
    created_at: &str,
    tags: &[String],
    body: &str,
    assets: &[PreparedCaptureAsset],
) -> String {
    // 设计思路：所有采集端先统一输出 Markdown，服务端只追加稳定元信息和附件引用。
    // 这样知乎、公众号、小红书、B 站、手机 OCR 都能走同一条文件库导入链路。
    let mut markdown = String::new();
    markdown.push_str("---\n");
    markdown.push_str(&format!("title: {}\n", yaml_quote(title)));
    markdown.push_str(&format!(
        "source_url: {}\n",
        yaml_quote(req.source_url.trim())
    ));
    markdown.push_str(&format!(
        "source_app: {}\n",
        yaml_quote(req.source_app.trim())
    ));
    markdown.push_str(&format!("captured_at: {}\n", yaml_quote(created_at)));
    markdown.push_str("tags:\n");
    for tag in tags {
        markdown.push_str(&format!("  - {}\n", yaml_quote(tag)));
    }
    markdown.push_str("---\n\n");
    markdown.push_str(body.trim());
    markdown.push('\n');

    if !assets.is_empty() {
        markdown.push_str("\n## 附件\n\n");
        for asset in assets {
            markdown.push_str(&format!(
                "![{}](../images/{})\n\n",
                asset.file_name, asset.file_name
            ));
        }
    }

    markdown
}

fn yaml_quote(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

fn write_capture_markdown_atomically(path: &Path, markdown: &str) -> std::io::Result<()> {
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, markdown.as_bytes())?;
    fs::rename(&temp_path, path)?;
    Ok(())
}

fn write_capture_assets_to_library(
    file_repo: &str,
    imported_stem: &str,
    assets: &[PreparedCaptureAsset],
) -> Result<(), String> {
    if assets.is_empty() {
        return Ok(());
    }

    let images_dir = PathBuf::from(get_libraries_root())
        .join("Libraries")
        .join(file_repo)
        .join(imported_stem)
        .join("images");

    fs::create_dir_all(&images_dir).map_err(|e| e.to_string())?;

    for asset in assets {
        let target_path = images_dir.join(&asset.file_name);
        fs::write(&target_path, &asset.bytes).map_err(|e| e.to_string())?;
        info!(
            "[capture_import] wrote asset name='{}' mime='{}' bytes={} path='{}'",
            asset.file_name,
            asset.mime,
            asset.bytes.len(),
            target_path.display()
        );
    }

    Ok(())
}

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
    _file_content: &[u8],
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
    _file_content: &[u8],
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
            "add_web_clipper_file_event",
            serde_json::to_value(&file_info)
                .map_err(|e| format!("Failed to serialize file info: {}", e))?,
        )
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    info!("Emitted add_web_clipper_file_event with file info: {:?}", file_info);

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
            "add_web_clipper_file_event",
            serde_json::to_value(&file_info)
                .map_err(|e| format!("Failed to serialize file info: {}", e))?,
        )
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    info!("Emitted add_web_clipper_file_event with file info: {:?}", file_info);

    Ok(())
}
