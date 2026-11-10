use pdfium_render::prelude::*;
use pyo3::prelude::*;
use pyo3::types::PyList; // 导入 PyList 类型
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::Cursor;
use std::net::TcpListener;
use std::path::Path;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{Emitter, Window};
use tracing::{error, info, warn};
use image::ImageFormat; // 确保已导入 ImageFormat
use base64::Engine as _; // 使用新的 base64 编码方式
use base64::engine::general_purpose;
use svg::node::element::{Rectangle, Text as SvgText};
use svg::Document;
use regex::Regex;
use reqwest::Client;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;
use tokio::time::{sleep, Duration};
use crate::commands::notebook::preprocess_markdown_for_static_render;
use crate::guaxiservices::bundled_resources;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Deserialize, Debug)]
pub struct OpenFileRequest {
    filetype: String,
    filepath: String,
    curpage: u32,
    lastpage: u32,
}

#[derive(Serialize, Debug)]
pub struct OpenFileResponse {
    success: bool, // 明确添加success字段
    filetype: String,
    content: String,
    curpage: u32,
    lastpage: u32,
    totalpages: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>, // 可选错误信息
}

#[derive(Deserialize, Debug)]
pub struct ConvertFileRequest {
    input_path: String,
    output_dir: String,
}

#[derive(serde::Serialize, Clone)]
struct ConversionProgress {
    current: u32,
    total: u32,
    message: String,
    #[serde(rename = "fileName", skip_serializing_if = "Option::is_none")]
    file_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Default)]
struct MarkerServerState {
    child: Option<Child>,
    port: Option<u16>,
    python_path: String,
    model_dir: Option<String>,
}

#[allow(dead_code)]
static MARKER_SERVER: OnceLock<Mutex<MarkerServerState>> = OnceLock::new();

#[allow(dead_code)]
fn marker_server_state() -> &'static Mutex<MarkerServerState> {
    MARKER_SERVER.get_or_init(|| Mutex::new(MarkerServerState::default()))
}

/// 关闭常驻的 Marker 文档转换服务（python）。
///
/// 设计说明：Marker 服务启动一次后会一直挂着复用模型，主程序退出时必须收掉，
/// 否则会残留一个占用大量内存/显存的 python 进程。
pub fn shutdown_marker_server() {
    match marker_server_state().lock() {
        Ok(mut state) => {
            if let Some(mut child) = state.child.take() {
                crate::guaxiservices::process_shutdown::terminate_child(
                    "marker-server(python)",
                    &mut child,
                );
            }
            state.port = None;
        }
        Err(_) => {
            warn!("[docprocess] Marker 服务状态锁损坏，退出时无法回收");
        }
    }
}

#[allow(dead_code)]
fn pick_free_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener
        .local_addr()
        .map_err(|e| e.to_string())?
        .port();
    Ok(port)
}

fn resolve_marker_source_dir() -> Result<PathBuf, String> {
    let p = bundled_resources::ensure_relative_dir("utils/MarkdownConvertor/marker-1.10.1")?;
    if p.exists() {
        info!(
            marker_source_dir = p.to_string_lossy().as_ref(),
            "marker source dir resolved from app install dir"
        );
        return Ok(p);
    }

    if cfg!(debug_assertions) {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("marker-1.10.1");
        if p.exists() {
            info!(
                marker_source_dir = p.to_string_lossy().as_ref(),
                "marker source dir resolved from workspace"
            );
            return Ok(p);
        }
    }

    Err("找不到 marker-1.10.1 源码目录，请检查是否存在：<软件目录>\\utils\\MarkdownConvertor\\marker-1.10.1".to_string())
}

fn resolve_marker_python_path(marker_src_dir: &Path) -> Result<PathBuf, String> {
    let venv_dir = marker_src_dir.join(".venv");

    if cfg!(windows) {
        let candidates = [
            venv_dir.join("Scripts").join("python.exe"),
            venv_dir.join("python.exe"),
        ];

        for p in candidates {
            if p.exists() {
                return Ok(p);
            }
        }

        return Err(format!(
            "找不到 Marker 虚拟环境中的 python，请先构建运行环境：{}",
            venv_dir.to_string_lossy()
        ));
    }

    let p = venv_dir.join("bin").join("python");
    if p.exists() {
        return Ok(p);
    }

    Err(format!(
        "找不到 Marker 虚拟环境中的 python，请先构建运行环境：{}",
        venv_dir.to_string_lossy()
    ))
}

#[allow(dead_code)]
async fn ensure_marker_server(window: &Window) -> Result<u16, String> {
    {
        let mut st = marker_server_state()
            .lock()
            .map_err(|_| "MarkerServerState 锁失败".to_string())?;
        if let Some(child) = st.child.as_mut() {
            match child.try_wait() {
                Ok(None) => {
                    if let Some(port) = st.port {
                        info!(port, "marker server already running, reuse");
                        return Ok(port);
                    }
                }
                Ok(Some(status)) => {
                    warn!(?status, "marker server exited, will restart");
                    st.child = None;
                    st.port = None;
                }
                Err(e) => {
                    warn!(err = e.to_string(), "marker server state check failed, will restart");
                    st.child = None;
                    st.port = None;
                }
            }
        }
    }

    let marker_src_dir = resolve_marker_source_dir()?;
    let python_path = resolve_marker_python_path(&marker_src_dir)?
        .to_string_lossy()
        .to_string();

    info!(
        marker_src_dir = marker_src_dir.to_string_lossy().as_ref(),
        python_path = python_path.as_str(),
        "marker server resolved paths"
    );

    let model_dir = bundled_resources::ensure_relative_dir("utils/Models/surya_models")?;
    let _ = fs::create_dir_all(&model_dir);

    info!(
        python = python_path.as_str(),
        models_dir = model_dir.to_string_lossy().as_ref(),
        "marker server config"
    );

    let port = pick_free_port()?;

    window
        .emit(
            "conversion_progress",
            ConversionProgress {
                current: 1,
                total: 100,
                message: "正在启动 Marker 服务...".to_string(),
                file_name: None,
            },
        )
        .ok();

    let mut cmd = Command::new(&python_path);
    cmd.arg("-m")
        .arg("marker.scripts.server")
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .current_dir(&marker_src_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let combined_pythonpath = marker_src_dir.to_string_lossy().to_string();

    cmd.env("PYTHONPATH", combined_pythonpath);

    let model_dir = model_dir.to_string_lossy().to_string();
    cmd.env("HF_HOME", &model_dir);
    cmd.env("HUGGINGFACE_HUB_CACHE", &model_dir);
    cmd.env("TRANSFORMERS_CACHE", &model_dir);
    cmd.env("TORCH_HOME", &model_dir);
    cmd.env("XDG_CACHE_HOME", &model_dir);

    info!(
        python = python_path.as_str(),
        port,
        cwd = marker_src_dir.to_string_lossy().as_ref(),
        "spawning marker server"
    );
    let child = cmd.spawn().map_err(|e| format!("启动 Marker 服务失败: {}", e))?;

    {
        let mut st = marker_server_state().lock().map_err(|_| "MarkerServerState 锁失败".to_string())?;
        st.child = Some(child);
        st.port = Some(port);
        st.python_path = python_path;
        st.model_dir = Some(model_dir);
    }

    let client = Client::new();
    let health_url = format!("http://127.0.0.1:{}/", port);

    for i in 0..600 {
        if let Ok(resp) = client.get(&health_url).send().await {
            if resp.status().is_success() {
                info!(port, tries = i + 1, "marker server is ready");
                return Ok(port);
            }
        }
        if i == 0 || (i + 1) % 40 == 0 {
            info!(port, tries = i + 1, "waiting for marker server...");
        }
        sleep(Duration::from_millis(500)).await;
    }

    warn!(port, "marker server startup timed out");

    {
        let mut st = marker_server_state().lock().map_err(|_| "MarkerServerState 锁失败".to_string())?;
        if let Some(mut child) = st.child.take() {
            let _ = child.kill();
        }
        st.port = None;
    }

    Err("Marker 服务启动超时（可能在下载/加载模型），请稍后重试".to_string())
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct MarkerConvertResponse {
    success: bool,
    output: Option<String>,
    images: Option<HashMap<String, String>>,
    error: Option<String>,
}

// 请求数据结构
#[derive(Debug, Deserialize)]
pub struct PdfRenderRequest {
    filepath: String,
    curpage: usize,
    #[allow(dead_code)]
    lastpage: usize,
    #[serde(default)]
    mode: String, // "vector"|"hybrid"|"image"(默认)
    #[serde(default)]
    scale: Option<f32>,
}

// 响应数据结构
#[derive(Debug, Serialize)]
pub struct PdfPageResponse {
    page_number: usize,
    total_pages: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    svg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_data: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    text_blocks: Vec<TextBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct TextBlock {
    text: String,
    bounds: Bounds,
}

#[derive(Debug, Serialize)]
struct Bounds {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[derive(Debug, Serialize)]
pub struct MarkdownResponse {
    success: bool,
    content: String,
    error: Option<String>,
}



#[tauri::command]
pub async fn copy_image_file(
    src_path: String,
    dest_dir: String,
    file_name: String,
) -> Result<PathBuf, String> {
    let src = PathBuf::from(&src_path);
    let dest = PathBuf::from(&dest_dir).join(&file_name);

    // 验证源文件
    if !src.exists() {
        return Err(format!("源文件不存在: {}", src.display()));
    }

    // 创建目标目录（递归）
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建目录失败: {}", e))?;
    }

    // 复制文件（支持大文件）
    let mut options = fs_extra::file::CopyOptions::new();
    options.overwrite = true;
    fs_extra::file::copy(&src, &dest, &options)
        .map_err(|e| format!("复制文件失败: {}", e))?;

    info!("文件复制成功: {} -> {}", src.display(), dest.display());

    // 处理相对路径的显示
    let relative_path = dest.strip_prefix(&dest_dir)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "无法计算相对路径".to_string());
    info!("相对路径 {}", relative_path);

    // 返回相对路径
    dest.strip_prefix(&dest_dir)
        .map(|p| p.to_owned())
        .map_err(|_| "无法计算相对路径".to_string())
}

#[tauri::command]
pub async fn load_markdown_content(filepath: String, page: usize) -> MarkdownResponse {
    info!("load_markdown_content: {}, page: {}", filepath, page);
    match std::fs::read_to_string(&filepath) {
        Ok(content) => {
            let pages: Vec<&str> = content
                .split("\n{")
                .filter(|p| !p.trim().is_empty())
                .collect();
            let page_content = if page < pages.len() {
                pages[page]
                    .trim_start_matches(|c: char| c.is_numeric())
                    .trim_start_matches('}')
                    .trim_start_matches('-')
                    .trim()
            } else {
                ""
            };

            MarkdownResponse {
                success: true,
                content: page_content.to_string(),
                error: None,
            }
        }
        Err(e) => MarkdownResponse {
            success: false,
            content: String::new(),
            error: Some(format!("Failed to read markdown file: {}", e)),
        },
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedSearchMatch {
    page: usize,
    count: usize,
    snippet: String,
}

#[derive(Debug, Serialize)]
pub struct PaginatedSearchResponse {
    success: bool,
    matches: Vec<PaginatedSearchMatch>,
    error: Option<String>,
}

#[tauri::command]
pub async fn search_paginated_markdown(filepath: String, query: String) -> PaginatedSearchResponse {
    let q = query.trim();
    if q.is_empty() {
        return PaginatedSearchResponse {
            success: true,
            matches: vec![],
            error: None,
        };
    }

    info!("search_paginated_markdown: {}", filepath);

    fn lower_ascii(b: u8) -> u8 {
        if (b'A'..=b'Z').contains(&b) {
            b + 32
        } else {
            b
        }
    }

    fn count_and_first_non_overlapping(hay: &str, needle: &str) -> (usize, Option<usize>) {
        if needle.is_empty() {
            return (0, None);
        }

        let mut count = 0usize;
        let mut first: Option<usize> = None;
        let mut cursor = 0usize;

        while cursor <= hay.len() {
            let Some(pos) = hay[cursor..].find(needle) else {
                break;
            };
            let abs = cursor + pos;
            if first.is_none() {
                first = Some(abs);
            }
            count += 1;
            cursor = abs + needle.len();
            if cursor > hay.len() {
                break;
            }
        }

        (count, first)
    }

    fn count_and_first_ascii_ci_non_overlapping(hay: &str, needle_lower: &[u8]) -> (usize, Option<usize>) {
        if needle_lower.is_empty() {
            return (0, None);
        }

        let h = hay.as_bytes();
        let n = needle_lower;
        if n.len() > h.len() {
            return (0, None);
        }

        let mut count = 0usize;
        let mut first: Option<usize> = None;
        let mut i = 0usize;

        while i + n.len() <= h.len() {
            let mut ok = true;
            for j in 0..n.len() {
                if lower_ascii(h[i + j]) != n[j] {
                    ok = false;
                    break;
                }
            }

            if ok {
                if first.is_none() {
                    first = Some(i);
                }
                count += 1;
                i += n.len();
            } else {
                i += 1;
            }
        }

        (count, first)
    }

    match std::fs::read_to_string(&filepath) {
        Ok(content) => {
            let pages: Vec<&str> = content
                .split("\n{")
                .filter(|p| !p.trim().is_empty())
                .collect();

            let need_ascii_ci = q.bytes().any(|b| (b'A'..=b'Z').contains(&b) || (b'a'..=b'z').contains(&b));
            let needle_lower: Vec<u8> = q.as_bytes().iter().map(|b| lower_ascii(*b)).collect();

            let mut matches = Vec::new();

            for (i, raw_page) in pages.iter().enumerate() {
                let page_content = raw_page
                    .trim_start_matches(|c: char| c.is_numeric())
                    .trim_start_matches('}')
                    .trim_start_matches('-')
                    .trim();

                if page_content.is_empty() {
                    continue;
                }

                let (count, first_pos) = if need_ascii_ci {
                    count_and_first_ascii_ci_non_overlapping(page_content, &needle_lower)
                } else {
                    count_and_first_non_overlapping(page_content, q)
                };

                if count == 0 {
                    continue;
                }

                let mut snippet = String::new();
                if let Some(p) = first_pos {
                    let mut start = p.saturating_sub(30);
                    while start > 0 && !page_content.is_char_boundary(start) {
                        start -= 1;
                    }

                    let mut end = std::cmp::min(page_content.len(), p + q.len() + 30);
                    while end < page_content.len() && !page_content.is_char_boundary(end) {
                        end += 1;
                    }

                    snippet = page_content[start..end].replace(['\n', '\r', '\t'], " ");
                    snippet = snippet.split_whitespace().collect::<Vec<_>>().join(" ");
                }

                matches.push(PaginatedSearchMatch {
                    page: i + 1,
                    count,
                    snippet,
                });
            }

            PaginatedSearchResponse {
                success: true,
                matches,
                error: None,
            }
        }
        Err(e) => PaginatedSearchResponse {
            success: false,
            matches: vec![],
            error: Some(format!("Failed to read markdown file: {}", e)),
        },
    }
}

#[tauri::command]
pub async fn get_converted_pdf_page(request: Value) -> Result<String, String> {
    info!("get_converted_pdf_page: {:?}", request);

    // 解析请求参数
    let req: PdfRenderRequest = serde_json::from_value(request)
        .map_err(|e| format!("请求解析失败: {}", e))?;

    // pdfium 全进程唯一、非线程安全：不再自行 bind，统一投递到 pdf_reader 的工作线程执行，
    // 复用同一份实例与文档缓存，避免与阅读器/TTS 并发踩踏导致的"获取页面 N 失败"。
    let response = crate::commands::pdf_reader::run_pdf(move |state| {
        state.with_doc(&req.filepath, |document| {
            let total_pages = document.pages().len() as usize;
            if req.curpage < 1 || req.curpage > total_pages {
                return Err(format!("页码 {} 超出范围(总页数: {})", req.curpage, total_pages));
            }

            // 获取目标页
            let page = document
                .pages()
                .get((req.curpage - 1) as u16)
                .map_err(|_| format!("获取页面 {} 失败", req.curpage))?;

            // 根据模式处理
            let resp = match req.mode.as_str() {
                "vector" => render_vector(&page, req.curpage, total_pages, req.scale),
                "hybrid" => render_hybrid(&page, req.curpage, total_pages, req.scale),
                _ => render_image(&page, req.curpage, total_pages, req.scale),
            };
            Ok(resp)
        })
    })
    .await?;

    serde_json::to_string(&response).map_err(|e| e.to_string())
}

// 矢量模式 (SVG+文本)
fn render_vector(page: &PdfPage, curpage: usize, total_pages: usize, scale: Option<f32>) -> PdfPageResponse {
    let scale_factor = scale.unwrap_or(1.5);
    let page_width = page.width().value;
    let page_height = page.height().value;
    
    // 创建SVG文档
    let mut doc = Document::new()
        .set("width", format!("{}pt", page_width * scale_factor))
        .set("height", format!("{}pt", page_height * scale_factor))
        .set("viewBox", format!("0 0 {} {}", page_width, page_height))
        .set("xmlns", "http://www.w3.org/2000/svg")
        .set("xmlns:xlink", "http://www.w3.org/1999/xlink");

    // 1. 渲染页面背景
    doc = doc.add(
        Rectangle::new()
            .set("x", 0)
            .set("y", 0)
            .set("width", page_width)
            .set("height", page_height)
            .set("fill", "white")
    );

    // 2. 提取并渲染文本
    let mut text_blocks = Vec::new();
    if let Ok(text) = page.text() {
        // 使用 text.all() 获取所有文本
        let all_text = text.all();
        text_blocks.push(TextBlock {
            text: all_text.clone(),
            bounds: Bounds {
                x: 0.0,
                y: 0.0,
                w: page_width as f32,
                h: page_height as f32,
            },
        });

        // 正确创建SVG文本元素 - 提供初始内容参数
        let text_element = SvgText::new(all_text)  // 这里提供初始文本内容
            .set("x", 10)
            .set("y", 20)
            .set("font-family", "Arial")
            .set("font-size", 12)
            .set("fill", "black");
        
        doc = doc.add(text_element);
    }

    // 3. 生成SVG字符串
    let svg_string = doc.to_string();

    PdfPageResponse {
        page_number: curpage,
        total_pages,
        svg: Some(svg_string),
        image_data: None,
        text_blocks,
        error: None,
    }
}

// 混合模式 (图片+文本)
fn render_hybrid(page: &PdfPage, curpage: usize, total_pages: usize, scale: Option<f32>) -> PdfPageResponse {
    let mut config = PdfRenderConfig::new()
        .set_target_width(1200)
        .use_print_quality(true)
        .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);
    
    if let Some(s) = scale {
        config = config.scale(s, s).expect("Failed to set scale");
    }

    let image = match page.render_with_config(&config) {
        Ok(img) => img,
        Err(e) => return PdfPageResponse {
            page_number: curpage,
            total_pages,
            svg: None,
            image_data: None,
            text_blocks: vec![],
            error: Some(format!("页面渲染失败: {}", e)),
        },
    };

    let image_data = match encode_image(&image) {
        Ok(data) => Some(data),
        Err(e) => return PdfPageResponse {
            page_number: curpage,
            total_pages,
            svg: None,
            image_data: None,
            text_blocks: vec![],
            error: Some(format!("图片编码失败: {}", e)),
        },
    };

    let text_blocks = extract_text_blocks(page);

    PdfPageResponse {
        page_number: curpage,
        total_pages,
        svg: None,
        image_data,
        text_blocks,
        error: None,
    }
}

// 图片模式 (兼容旧版)
fn render_image(page: &PdfPage, curpage: usize, total_pages: usize, scale: Option<f32>) -> PdfPageResponse {
    let mut config = PdfRenderConfig::new()
        .set_target_width(1200)
        .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);
    
    if let Some(s) = scale {
        config = config.scale(s, s).expect("Failed to set scale");
    }

    let image = match page.render_with_config(&config) {
        Ok(img) => img,
        Err(e) => return PdfPageResponse {
            page_number: curpage,
            total_pages,
            svg: None,
            image_data: None,
            text_blocks: vec![],
            error: Some(format!("页面渲染失败: {}", e)),
        },
    };

    let image_data = match encode_image(&image) {
        Ok(data) => Some(data),
        Err(e) => return PdfPageResponse {
            page_number: curpage,
            total_pages,
            svg: None,
            image_data: None,
            text_blocks: vec![],
            error: Some(format!("图片编码失败: {}", e)),
        },
    };

    PdfPageResponse {
        page_number: curpage,
        total_pages,
        svg: None,
        image_data,
        text_blocks: vec![],
        error: None,
    }
}

// 图片编码通用方法
fn encode_image(image: &PdfBitmap) -> Result<String, String> {
    let mut bytes = Vec::new();
    image
        .as_image()
        .into_rgb8()
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|e| format!("图片编码失败: {}", e))?;
    Ok(general_purpose::STANDARD.encode(&bytes))
}

// 提取文本块
fn extract_text_blocks(page: &PdfPage) -> Vec<TextBlock> {
    let mut blocks = Vec::new();
    
    if let Ok(text) = page.text() {
        // 使用 text.all() 获取所有文本
        let all_text = text.all();
        blocks.push(TextBlock {
            text: all_text,
            bounds: Bounds {
                x: 0.0,
                y: 0.0,
                w: page.width().value as f32,
                h: page.height().value as f32,
            },
        });
    }
    
    blocks
}

#[tauri::command]
pub async fn check_file_converted(
    output_dir: String, // 只需要传入output_dir参数
) -> Result<bool, String> {
    // 检查输出文件是否存在
    info!("Checking if file is converted: {}", output_dir);
    Ok(Path::new(&output_dir).exists())
}

#[tauri::command]
pub async fn convert_to_markdown(
    request: ConvertFileRequest,
    window: Window,
) -> Result<(), String> {
    let file_name = Path::new(&request.input_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("未知文件")
        .to_string();

    info!(
        file_name = file_name.as_str(),
        input_path = request.input_path.as_str(),
        output_dir = request.output_dir.as_str(),
        "convert_to_markdown start"
    );

    window
        .emit(
            "conversion_progress",
            ConversionProgress {
                current: 0,
                total: 100,
                message: "开始转换...".to_string(),
                file_name: Some(file_name.clone()),
            },
        )
        .map_err(|e| format!("Failed to emit start progress: {}", e))?;

    let marker_src_dir = resolve_marker_source_dir()?;
    let python_path = resolve_marker_python_path(&marker_src_dir)?
        .to_string_lossy()
        .to_string();

    let model_dir = bundled_resources::ensure_relative_dir("utils/Models/surya_models")?;
    let _ = fs::create_dir_all(&model_dir);

    let existing_pythonpath = std::env::var("PYTHONPATH").unwrap_or_default();
    let marker_pythonpath = marker_src_dir.to_string_lossy().to_string();
    let path_list_sep = if cfg!(windows) { ";" } else { ":" };
    let combined_pythonpath = if existing_pythonpath.trim().is_empty() {
        marker_pythonpath
    } else {
        format!("{}{}{}", marker_pythonpath, path_list_sep, existing_pythonpath)
    };

    let script_path = marker_src_dir.join("convert_single.py");
    if !script_path.exists() {
        return Err(format!(
            "找不到 convert_single.py: {}",
            script_path.to_string_lossy()
        ));
    }

    let input_stem = Path::new(&request.input_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .to_string();
    let expected_output_dir = PathBuf::from(&request.output_dir).join(&input_stem);
    let expected_md_path = expected_output_dir.join(format!("{}.md", input_stem));

    info!(
        file_name = file_name.as_str(),
        script_path = script_path.to_string_lossy().as_ref(),
        output_dir = request.output_dir.as_str(),
        expected_output_dir = expected_output_dir.to_string_lossy().as_ref(),
        expected_md_path = expected_md_path.to_string_lossy().as_ref(),
        "convert_to_markdown output targets"
    );

    window
        .emit(
            "conversion_progress",
            ConversionProgress {
                current: 5,
                total: 100,
                message: "正在启动转换...".to_string(),
                file_name: Some(file_name.clone()),
            },
        )
        .ok();

    let model_dir_str = model_dir.to_string_lossy().to_string();

    let _ = fs::create_dir_all(&expected_output_dir);

    let args: Vec<String> = vec![
        "-u".to_string(),
        script_path.to_string_lossy().to_string(),
        request.input_path.clone(),
        "--output_format".to_string(),
        "markdown".to_string(),
        "--paginate_output".to_string(),
        "--output_dir".to_string(),
        expected_output_dir.to_string_lossy().to_string(),
        "--progress_json".to_string(),
    ];

    info!(
        file_name = file_name.as_str(),
        cwd = marker_src_dir.to_string_lossy().as_ref(),
        python = python_path.as_str(),
        args = ?args,
        model_dir = model_dir_str.as_str(),
        "spawning marker convert_single"
    );

    let mut cmd = TokioCommand::new(&python_path);
    cmd.args(&args)
        .current_dir(&marker_src_dir)
        .env("PYTHONPATH", combined_pythonpath)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .env("HF_HOME", model_dir_str.as_str())
        .env("HUGGINGFACE_HUB_CACHE", model_dir_str.as_str())
        .env("TRANSFORMERS_CACHE", model_dir_str.as_str())
        .env("TORCH_HOME", model_dir_str.as_str())
        .env("XDG_CACHE_HOME", model_dir_str.as_str())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("启动 Marker 转换失败: {}", e))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture converter stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture converter stderr".to_string())?;

    let output_file_holder: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let stderr_tail: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));

    let window_stdout = window.clone();
    let file_name_stdout = file_name.clone();
    let output_file_holder_stdout = output_file_holder.clone();
    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut buf: Vec<u8> = Vec::new();

        loop {
            buf.clear();
            let n = match reader.read_until(b'\n', &mut buf).await {
                Ok(n) => n,
                Err(e) => {
                    warn!(file_name = file_name_stdout.as_str(), err = %e, "failed reading converter stdout");
                    break;
                }
            };
            if n == 0 {
                break;
            }

            let line = String::from_utf8_lossy(&buf);
            let trimmed = line.trim_matches(&['\r', '\n'][..]).trim();
            if trimmed.is_empty() {
                continue;
            }

            let v: serde_json::Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(_) => {
                    info!(file_name = file_name_stdout.as_str(), line = trimmed, "converter stdout");
                    continue;
                }
            };

            match v.get("type").and_then(|t| t.as_str()) {
                Some("conversion_progress") => {
                    let current = v.get("current").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                    let total = v.get("total").and_then(|x| x.as_u64()).unwrap_or(100) as u32;
                    let message = v
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("转换中...")
                        .to_string();

                    let _ = window_stdout.emit(
                        "conversion_progress",
                        ConversionProgress {
                            current,
                            total,
                            message,
                            file_name: Some(file_name_stdout.clone()),
                        },
                    );
                }
                Some("conversion_result") => {
                    let output_exists = v
                        .get("output_exists")
                        .and_then(|x| x.as_bool())
                        .unwrap_or(false);

                    if let Some(p) = v.get("output_file").and_then(|p| p.as_str()) {
                        info!(
                            file_name = file_name_stdout.as_str(),
                            output_file = p,
                            output_exists,
                            "converter reported conversion_result"
                        );

                        if let Ok(mut g) = output_file_holder_stdout.lock() {
                            *g = Some(p.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    });

    let file_name_stderr = file_name.clone();
    let stderr_tail_task = stderr_tail.clone();
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buf: Vec<u8> = Vec::new();

        loop {
            buf.clear();
            let n = match reader.read_until(b'\n', &mut buf).await {
                Ok(n) => n,
                Err(e) => {
                    warn!(file_name = file_name_stderr.as_str(), err = %e, "failed reading converter stderr");
                    break;
                }
            };
            if n == 0 {
                break;
            }

            let line = String::from_utf8_lossy(&buf);
            let trimmed = line.trim_matches(&['\r', '\n'][..]).trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Ok(mut g) = stderr_tail_task.lock() {
                if g.len() >= 200 {
                    g.pop_front();
                }
                g.push_back(trimmed.to_string());
            }

            warn!(file_name = file_name_stderr.as_str(), line = trimmed, "converter stderr");
        }
    });

    let status = child
        .wait()
        .await
        .map_err(|e| format!("等待转换进程失败: {}", e))?;

    info!(
        file_name = file_name.as_str(),
        success = status.success(),
        status = %status,
        "marker convert_single exited"
    );

    let _ = stdout_task.await;
    let _ = stderr_task.await;

    if !status.success() {
        let tail = stderr_tail
            .lock()
            .ok()
            .map(|g| {
                let keep = 80usize;
                let mut v: Vec<String> = g.iter().rev().take(keep).cloned().collect();
                v.reverse();
                v.join("\n")
            })
            .unwrap_or_default();

        if tail.trim().is_empty() {
            return Err(format!("Marker 转换失败: {}", status));
        }

        return Err(format!("Marker 转换失败: {}\n{}", status, tail));
    }

    let reported_output_file = output_file_holder
        .lock()
        .ok()
        .and_then(|g| g.clone());

    if let Some(p) = reported_output_file.as_deref() {
        info!(file_name = file_name.as_str(), output_file = p, "converter reported output file");
    }

    let actual_md_path = reported_output_file
        .map(PathBuf::from)
        .unwrap_or_else(|| expected_md_path.clone());

    let md_exists = actual_md_path.exists();
    info!(
        file_name = file_name.as_str(),
        expected_md_path = expected_md_path.to_string_lossy().as_ref(),
        actual_md_path = actual_md_path.to_string_lossy().as_ref(),
        md_exists,
        "conversion output check"
    );

    if !md_exists {
        let expected_dir_exists = expected_output_dir.exists();
        warn!(
            file_name = file_name.as_str(),
            expected_output_dir = expected_output_dir.to_string_lossy().as_ref(),
            expected_dir_exists,
            "conversion finished but markdown not found"
        );

        let _ = window.emit(
            "conversion_progress",
            ConversionProgress {
                current: 100,
                total: 100,
                message: "转换结束，但未找到输出 Markdown 文件".to_string(),
                file_name: Some(file_name.clone()),
            },
        );

        return Err(format!(
            "转换结束但未找到输出文件，期望路径: {}",
            expected_md_path.to_string_lossy()
        ));
    }

    window
        .emit(
            "conversion_progress",
            ConversionProgress {
                current: 100,
                total: 100,
                message: "转换完成".to_string(),
                file_name: Some(file_name.clone()),
            },
        )
        .map_err(|e| format!("Failed to emit completion: {}", e))?;

    Ok(())
}


#[tauri::command]
pub async fn export_markdown_pdf_pandoc(
    input_path: String,
    output_path: String,
) -> Result<(), String> {
    export_markdown_pandoc(input_path, output_path, "pdf".to_string(), None).await
}

fn escape_html_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn percent_encode_file_url_path(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for b in value.as_bytes() {
        match *b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b'/'
            | b':' => encoded.push(*b as char),
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}

fn path_to_file_url(path: &Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    let normalized = if cfg!(windows) && raw.as_bytes().get(1) == Some(&b':') {
        format!("/{raw}")
    } else {
        raw
    };
    format!("file://{}", percent_encode_file_url_path(&normalized))
}

fn inject_export_base_href(html: &str, input_path: &Path) -> String {
    if html.to_ascii_lowercase().contains("<base ") {
        return html.to_string();
    }

    let base_dir = input_path.parent().unwrap_or_else(|| Path::new("."));
    let mut base_href = path_to_file_url(base_dir);
    if !base_href.ends_with('/') {
        base_href.push('/');
    }
    let base_tag = format!(r#"<base href="{}" />"#, escape_html_attr(&base_href));

    // 设计说明：
    // 1. 前端生成的是“预览同源 HTML”，里面的图片可能仍然是相对路径。
    // 2. 后端会把 HTML 写入临时目录再交给 Chromium 打印，若不补 base href，
    //    相对图片会错误地指向临时目录，导致 PDF 缺图。
    // 3. 即使前端已经传了 base，这里也做一次兜底，保证后端命令可以独立运行。
    if let Some(index) = html.to_ascii_lowercase().find("<head>") {
        let insert_at = index + "<head>".len();
        let mut next = String::with_capacity(html.len() + base_tag.len() + 1);
        next.push_str(&html[..insert_at]);
        next.push('\n');
        next.push_str(&base_tag);
        next.push_str(&html[insert_at..]);
        next
    } else {
        html.to_string()
    }
}

fn chromium_pdf_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    #[cfg(windows)]
    {
        if let Ok(program_files) = std::env::var("PROGRAMFILES") {
            candidates.push(PathBuf::from(&program_files).join("Microsoft/Edge/Application/msedge.exe"));
            candidates.push(PathBuf::from(&program_files).join("Google/Chrome/Application/chrome.exe"));
        }
        if let Ok(program_files_x86) = std::env::var("PROGRAMFILES(X86)") {
            candidates.push(PathBuf::from(&program_files_x86).join("Microsoft/Edge/Application/msedge.exe"));
            candidates.push(PathBuf::from(&program_files_x86).join("Google/Chrome/Application/chrome.exe"));
        }
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            candidates.push(PathBuf::from(&local_app_data).join("Microsoft/Edge/Application/msedge.exe"));
            candidates.push(PathBuf::from(&local_app_data).join("Google/Chrome/Application/chrome.exe"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        candidates.push(PathBuf::from("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"));
        candidates.push(PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"));
        candidates.push(PathBuf::from("microsoft-edge"));
        candidates.push(PathBuf::from("google-chrome"));
        candidates.push(PathBuf::from("chromium"));
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        candidates.push(PathBuf::from("microsoft-edge"));
        candidates.push(PathBuf::from("google-chrome"));
        candidates.push(PathBuf::from("chromium"));
        candidates.push(PathBuf::from("chromium-browser"));
    }

    candidates
}

fn select_chromium_for_pdf_export() -> Result<PathBuf, String> {
    for candidate in chromium_pdf_candidates() {
        if candidate.is_file() {
            info!(
                browser = %candidate.display(),
                "[ExportPdf] selected chromium browser by existing executable"
            );
            return Ok(candidate);
        }

        #[cfg(not(windows))]
        {
            if candidate.components().count() == 1 && can_execute_program(&candidate) {
                info!(
                    browser = %candidate.display(),
                    "[ExportPdf] selected chromium browser from PATH"
                );
                return Ok(candidate);
            }
        }
    }

    Err("未找到可用的 Edge/Chrome/Chromium，无法使用浏览器高保真导出 PDF".to_string())
}

fn run_chromium_print_to_pdf(
    browser: &Path,
    html_path: &Path,
    output_path: &Path,
    profile_dir: &Path,
    base_dir: &Path,
    headless_arg: &str,
) -> Result<(), String> {
    let html_url = path_to_file_url(html_path);
    let output_arg = format!("--print-to-pdf={}", output_path.to_string_lossy());
    let profile_arg = format!("--user-data-dir={}", profile_dir.to_string_lossy());

    let mut cmd = Command::new(browser);

    #[cfg(windows)]
    // 设计说明：release 模式下调用 Edge/Chrome 这类外部程序时，必须禁用控制台窗口；
    // 否则 Windows 会短暂弹出黑色命令行窗口，导出 PDF 时体验非常割裂。
    cmd.creation_flags(CREATE_NO_WINDOW);

    info!(
        browser = %browser.display(),
        html = %html_path.display(),
        output = %output_path.display(),
        headless = headless_arg,
        "[ExportPdf] running chromium print-to-pdf"
    );

    let output = cmd
        .current_dir(base_dir)
        .arg(headless_arg)
        .arg("--disable-gpu")
        // 设计说明：让 Chromium 依据页面 H1~H6 标题结构生成 PDF 书签大纲（document outline），
        // 这样导出的 PDF 在阅读器“书签 / 大纲”面板里可以直接按标题层级导航。
        // 该开关只在现代 headless（--headless=new）下生效，旧版会忽略未知开关，因此始终携带是安全的。
        .arg("--generate-pdf-document-outline")
        // 注意：不要加 `--no-startup-window`。该参数会让 Edge/Chrome 在后台启动且不创建窗口、
        // 不加载页面，配合 headless `--print-to-pdf` 时浏览器会以退出码 0 “成功”结束，但根本
        // 不会渲染 HTML、也不会写出 PDF，导致随后读取输出文件报 os error 2（找不到文件）。
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-extensions")
        .arg("--disable-background-mode")
        .arg("--disable-component-update")
        .arg("--disable-sync")
        .arg("--metrics-recording-only")
        .arg("--mute-audio")
        .arg("--allow-file-access-from-files")
        .arg(profile_arg)
        .arg(output_arg)
        .arg(html_url)
        .output()
        .map_err(|e| format!("启动浏览器 PDF 导出失败: {e}"))?;

    let stdout_text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr_text = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if output.status.success() {
        if !stdout_text.is_empty() {
            info!(stdout = stdout_text.as_str(), "[ExportPdf] chromium stdout");
        }
        if !stderr_text.is_empty() {
            warn!(stderr = stderr_text.as_str(), "[ExportPdf] chromium stderr");
        }

        // 设计说明：
        // 1. 某些 Edge/Chrome 版本会以 0 退出码结束，但没有真正写出 --print-to-pdf 指定的文件。
        // 2. 把产物校验放到单次浏览器调用内部，才能让 --headless=new 失败时继续重试传统 --headless。
        // 3. 日志中保留 stdout/stderr，方便区分“浏览器成功退出但无产物”和“命令执行失败”。
        match fs::metadata(output_path) {
            Ok(meta) if meta.len() > 0 => {
                info!(
                    output = %output_path.display(),
                    bytes = meta.len(),
                    headless = headless_arg,
                    "[ExportPdf] chromium produced pdf"
                );
                return Ok(());
            }
            Ok(_) => {
                warn!(
                    output = %output_path.display(),
                    headless = headless_arg,
                    "[ExportPdf] chromium exited successfully but output pdf is empty"
                );
                return Err(format!("浏览器 PDF 导出未生成有效文件：{} 字节", 0));
            }
            Err(err) => {
                warn!(
                    output = %output_path.display(),
                    headless = headless_arg,
                    error = %err,
                    "[ExportPdf] chromium exited successfully but output pdf is missing"
                );
                return Err(format!("浏览器 PDF 导出未生成输出文件: {err}"));
            }
        }
    }

    error!(
        status = ?output.status.code(),
        stdout = stdout_text.as_str(),
        stderr = stderr_text.as_str(),
        headless = headless_arg,
        "[ExportPdf] chromium print-to-pdf failed"
    );
    Err(format!(
        "浏览器 PDF 导出失败 (code={:?}): {}",
        output.status.code(),
        if stderr_text.is_empty() { stdout_text } else { stderr_text }
    ))
}

#[tauri::command]
pub async fn export_rendered_html_pdf(
    html: String,
    input_path: String,
    output_path: String,
) -> Result<(), String> {
    let input = PathBuf::from(input_path.trim());
    let output = PathBuf::from(output_path.trim());
    let base_dir = input.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();

    if html.trim().is_empty() {
        return Err("导出的 HTML 内容为空".to_string());
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("创建 PDF 输出目录失败: {e}"))?;
    }

    let browser = select_chromium_for_pdf_export()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let tmp_dir = std::env::temp_dir()
        .join("guaxi_rendered_html_pdf")
        .join(format!("{}", now.as_millis()));
    let profile_dir = tmp_dir.join("profile");
    fs::create_dir_all(&profile_dir)
        .map_err(|e| format!("创建浏览器临时目录失败: {e}"))?;

    let html_path = tmp_dir.join("export.html");
    let html_with_base = inject_export_base_href(&html, &input);
    fs::write(&html_path, html_with_base.as_bytes())
        .map_err(|e| format!("写入临时 HTML 失败: {e}"))?;

    info!(
        input = %input.display(),
        output = %output.display(),
        html = %html_path.display(),
        html_len = html.len(),
        "[ExportPdf] rendered html pdf export start"
    );

    // 设计说明：
    // 1. 优先使用新版 Chromium 的 --headless=new，能更接近现代浏览器渲染路径。
    // 2. 部分旧版 Chrome/Edge 不认识该参数，或者 0 退出但不写 PDF；单次调用会校验产物。
    // 3. 若第一种 headless 没有有效产物，继续回退到传统 --headless。
    let _ = fs::remove_file(&output);
    let first = run_chromium_print_to_pdf(
        &browser,
        &html_path,
        &output,
        &profile_dir,
        &base_dir,
        "--headless=new",
    );
    if first.is_err() {
        warn!(
            error = first.as_ref().err().map(|e| e.as_str()).unwrap_or("unknown"),
            "[ExportPdf] retry chromium print-to-pdf with legacy headless"
        );
        run_chromium_print_to_pdf(
            &browser,
            &html_path,
            &output,
            &profile_dir,
            &base_dir,
            "--headless",
        )?;
    }

    let size = fs::metadata(&output)
        .map_err(|e| format!("PDF 导出后未找到输出文件: {e}"))?
        .len();
    if size == 0 {
        return Err("PDF 导出完成但文件大小为 0".to_string());
    }

    info!(
        output = %output.display(),
        bytes = size,
        "[ExportPdf] rendered html pdf export complete"
    );

    if let Err(err) = fs::remove_dir_all(&tmp_dir) {
        warn!(
            tmp_dir = %tmp_dir.display(),
            error = %err,
            "[ExportPdf] cleanup temp dir failed"
        );
    }

    Ok(())
}

fn find_tectonic_exe_for_export() -> Result<(PathBuf, bool), String> {
    #[cfg(windows)]
    {
        let bundled = bundled_resources::ensure_relative_dir("utils/tectonic.exe")?;
        if bundled.is_file() {
            return Ok((bundled, true));
        }
        return Ok((PathBuf::from("tectonic"), false));
    }

    #[cfg(not(windows))]
    {
        let bundled = bundled_resources::ensure_relative_dir("utils/tectonic")?;
        if bundled.is_file() {
            return Ok((bundled, true));
        }
        return Ok((PathBuf::from("tectonic"), false));
    }
}

/// 随包分发的 TeX 资源种子缓存版本。重新生成 `utils/tectonic_cache` 时要递增。
const TECTONIC_SEED_VERSION: &str = "1";

static TECTONIC_SEED_LOCK: Mutex<()> = Mutex::new(());

fn copy_dir_contents(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_contents(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// TeX 资源缓存目录。tectonic 默认按需从 relay 逐文件下载宏包与字体，
/// 冷文档要几十秒；这里把随包分发的种子缓存复制到 config 目录，之后编译全部命中本地。
/// 没有种子缓存时返回 None，保持 tectonic 默认行为（不覆盖用户已有的预热缓存）。
fn tectonic_cache_dir_blocking() -> Option<PathBuf> {
    let config_path = crate::commands::settingview::get_config_path();
    let dir = config_path.parent()?.join("tectonic_cache");
    let marker = dir.join(".guaxi_seed_version");
    let seeded = || {
        fs::read_to_string(&marker)
            .map(|value| value.trim() == TECTONIC_SEED_VERSION)
            .unwrap_or(false)
    };
    if seeded() {
        return Some(dir);
    }

    let _guard = TECTONIC_SEED_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if seeded() {
        return Some(dir);
    }

    let seed = bundled_resources::ensure_relative_dir("utils/tectonic_cache").ok()?;
    if !seed.is_dir() {
        return None;
    }
    if let Err(error) = copy_dir_contents(&seed, &dir) {
        warn!(
            seed = %seed.display(),
            error = %error,
            "[LatexPreview] 复制 TeX 种子缓存失败，首次构建可能仍需联网下载"
        );
        return Some(dir);
    }
    if let Err(error) = fs::write(&marker, TECTONIC_SEED_VERSION) {
        warn!(error = %error, "[LatexPreview] 写入 TeX 种子缓存标记失败");
    }
    Some(dir)
}

async fn tectonic_cache_dir() -> Option<PathBuf> {
    tokio::task::spawn_blocking(tectonic_cache_dir_blocking)
        .await
        .ok()
        .flatten()
}

/// tectonic 只按 PATH 查找 biber 这类外部工具，和它放在同一目录并不会被搜索到，
/// 所以把 tectonic 所在目录以及解析到的 biber 目录都前置进子进程 PATH。
fn tectonic_tool_path(tectonic_path: &Path) -> Option<std::ffi::OsString> {
    fn push_dir(dirs: &mut Vec<PathBuf>, dir: &Path) {
        if !dirs.iter().any(|existing| existing == dir) {
            dirs.push(dir.to_path_buf());
        }
    }

    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(parent) = tectonic_path.parent() {
        push_dir(&mut dirs, parent);
    }
    let biber_rel = if cfg!(windows) {
        "utils/biber.exe"
    } else {
        "utils/biber"
    };
    if let Ok(biber) = bundled_resources::ensure_relative_dir(biber_rel) {
        if biber.is_file() {
            if let Some(parent) = biber.parent() {
                push_dir(&mut dirs, parent);
            }
        }
    }
    dirs.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    std::env::join_paths(dirs).ok()
}

fn apply_tectonic_child_env_std(cmd: &mut Command, tectonic_path: &Path) {
    if let Some(path) = tectonic_tool_path(tectonic_path) {
        cmd.env("PATH", path);
    }
    if let Some(cache_dir) = tectonic_cache_dir_blocking() {
        cmd.env("TECTONIC_CACHE_DIR", cache_dir);
    }
}

fn apply_tectonic_child_env_tokio(
    cmd: &mut TokioCommand,
    tectonic_path: &Path,
    cache_dir: Option<&Path>,
) {
    if let Some(path) = tectonic_tool_path(tectonic_path) {
        cmd.env("PATH", path);
    }
    if let Some(cache_dir) = cache_dir {
        cmd.env("TECTONIC_CACHE_DIR", cache_dir);
    }
}

#[tauri::command]
pub async fn export_tex_pdf_tectonic(input_path: String, output_path: String) -> Result<(), String> {
    // 说明：用于“当前文件是 .tex，导出为 PDF”的场景；保留相对路径依赖（图片/\input）时，必须在 tex 文件所在目录运行。
    info!(
        input_path = input_path.as_str(),
        output_path = output_path.as_str(),
        "Exporting tex via tectonic"
    );

    let input = PathBuf::from(input_path.trim());
    if !input.is_file() {
        return Err(format!("tex file not found: {}", input.to_string_lossy()));
    }

    let input_dir = input
        .parent()
        .ok_or_else(|| "Failed to get tex parent dir".to_string())?
        .to_path_buf();

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Failed to get tex file stem".to_string())?
        .to_string();

    let (tectonic_path, is_bundled) = find_tectonic_exe_for_export()?;

    // 临时输出目录：tectonic 的输出先落地，再 copy 到用户选择的 output_path。
    let tmp_base = std::env::temp_dir().join("guaxi_tectonic_export");
    let _ = fs::create_dir_all(&tmp_base);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let outdir = tmp_base.join(format!("{}", now.as_millis()));
    let _ = fs::remove_dir_all(&outdir);
    fs::create_dir_all(&outdir).map_err(|e| format!("create outdir failed: {e}"))?;

    info!(
        tectonic = %tectonic_path.display(),
        bundled = is_bundled,
        input_dir = %input_dir.display(),
        tmp_outdir = %outdir.display(),
        "[ExportTex] running tectonic"
    );

    let mut cmd = Command::new(&tectonic_path);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    // 导出同样要能找到 biber，并复用随包分发的资源缓存。
    apply_tectonic_child_env_std(&mut cmd, &tectonic_path);

    cmd.current_dir(&input_dir)
        .arg("-X")
        .arg("compile")
        .arg("--outdir")
        .arg(&outdir)
        .arg("--untrusted")
        .arg(&input);

    let out = cmd
        .output()
        .map_err(|e| format!("spawn tectonic failed: {e} (path={})", tectonic_path.display()))?;

    if !out.status.success() {
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();

        error!(
            code = out.status.code(),
            stdout_len = stdout.len(),
            stderr_len = stderr.len(),
            "[ExportTex] tectonic failed"
        );

        let mut hint = String::new();
        if !is_bundled {
            hint.push_str("\n\nHint: 未找到内置 tectonic，可将对应平台的二进制放到 src-tauri/utils 下，或确保系统 PATH 中存在 tectonic。\n");
        }

        return Err(format!("tectonic export failed (code={:?})\n{}\n{}{}", out.status.code(), stdout, stderr, hint));
    }

    let pdf_path = outdir.join(format!("{stem}.pdf"));
    if !pdf_path.is_file() {
        return Err(format!(
            "tectonic finished but PDF not found: {}",
            pdf_path.to_string_lossy()
        ));
    }

    let output = PathBuf::from(output_path.trim());
    if let Some(parent) = output.parent() {
        let _ = fs::create_dir_all(parent);
    }

    fs::copy(&pdf_path, &output)
        .map_err(|e| format!("copy pdf failed: {e} (from={}, to={})", pdf_path.to_string_lossy(), output.to_string_lossy()))?;

    info!(
        from = %pdf_path.display(),
        to = %output.display(),
        "[ExportTex] pdf exported"
    );

    let _ = fs::remove_dir_all(&outdir);

    Ok(())
}

/// 预览构建目录（稳定）：按 `<引擎>/<源文件路径哈希>` 分目录，使同一份文档每次重建
/// 都落到**同一个输出路径**。
///
/// 这是前端增量渲染的前提：`GuaxiPdfViewer` 只在 PDF 路径不变时才走 `incrementalReconcile`
/// （逐页指纹 diff、保留未变页的位图与滚动位置）；路径一变就整篇冷重载、重新栅格化所有页。
/// 放在 Guaxi 配置目录下（与 codex-home / cognition 同级），不用系统临时目录——那是随时会被清的。
/// 预览构建目录的**路径**（不创建目录）。
/// 清理时必须用这个版本：`preview_build_dir` 会 create_dir_all，拿它去清理等于又把目录建回来。
pub(crate) fn preview_build_dir_path(engine: &str, source_key: &str) -> Result<PathBuf, String> {
    let config_path = crate::commands::settingview::get_config_path();
    let config_dir = config_path
        .parent()
        .ok_or_else(|| "无法获取 Guaxi 配置目录".to_string())?;
    let mut hasher = Sha256::new();
    hasher.update(source_key.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    Ok(config_dir
        .join("preview_build")
        .join(engine)
        .join(&digest[..16]))
}

/// 取（必要时创建）预览构建目录。见 `preview_build_dir_path` 的稳定性说明。
pub(crate) fn preview_build_dir(engine: &str, source_key: &str) -> Result<PathBuf, String> {
    let dir = preview_build_dir_path(engine, source_key)?;
    fs::create_dir_all(&dir).map_err(|e| format!("创建预览构建目录失败: {e}"))?;
    Ok(dir)
}

/// 预览构建目录的键（源文件路径的规范形式）。
/// 两个 build_*_pdf_preview 与 clear_preview_build 必须共用这一套，否则清理算不出同一个哈希、
/// 删不掉自己建的目录。
pub(crate) fn preview_source_key(source_path: &str) -> String {
    PathBuf::from(source_path.trim()).to_string_lossy().to_string()
}

/// 正在构建的文档锁：同一份文档的预览构建必须**串行**。
///
/// 为什么需要：Typst 的预览是**进程内**编译，没有子进程可以像 tectonic 那样"新构建先杀掉上一轮"
/// （见 LATEX_PREVIEW_PIDS），于是两次构建可以真正并发跑。而它们写的是同一个临时文件名，
/// 先完成的那个 `rename` 把临时文件移走之后，后一个 `rename` 就找不到源文件，
/// 直接抛 "提交预览 PDF 失败: 系统找不到指定的文件。 (os error 2)" —— 这就是那条偶现报错的来源。
/// 串行还顺带保证"后发起的构建最后落盘"，不会让旧内容盖掉新内容。
static PREVIEW_BUILD_LOCKS: OnceLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    OnceLock::new();

/// 取（必要时创建）某文档的构建锁。键与 `preview_source_key` 一致。
pub(crate) fn preview_build_lock(key: &str) -> Arc<tokio::sync::Mutex<()>> {
    let registry = PREVIEW_BUILD_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = registry.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .entry(key.to_string())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

/// 构建期间唯一的临时文件后缀：即使上面的串行锁因故没生效，也不会两个构建踩同一个文件。
pub(crate) fn preview_staging_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    format!("{nanos:x}{:x}", SEQ.fetch_add(1, Ordering::Relaxed))
}

/// 正在构建的预览进程：key = 源文件路径，value = PID。
///
/// 前端只能丢弃过期构建的**结果**（latexBuildSeq），杀不掉已经在跑的 tectonic；
/// 所以这里按文档记 PID，新一轮构建开始前先杀掉上一轮，避免过期构建堆积抢占 CPU。
static LATEX_PREVIEW_PIDS: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();

fn latex_preview_pids() -> &'static Mutex<HashMap<String, u32>> {
    LATEX_PREVIEW_PIDS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 杀掉同一文档上一轮还没结束的预览构建（若有）。
fn kill_previous_latex_preview(key: &str) {
    let pid = latex_preview_pids()
        .lock()
        .ok()
        .and_then(|mut guard| guard.remove(key));
    if let Some(pid) = pid {
        info!(pid, key, "[LatexPreview] 终止上一轮过期构建");
        crate::guaxiservices::process_shutdown::terminate_process_tree(pid, "latex-preview");
    }
}

#[derive(Clone, Serialize)]
struct LatexPreviewProgress {
    source_path: String,
    file: String,
    count: u32,
}

/// 逐行读子进程输出：把 tectonic 的 `note: downloading ...` 转成前端进度事件，
/// 同时把完整输出留给失败时的错误信息。
async fn collect_latex_pipe<R>(
    pipe: Option<R>,
    progress: Option<(tauri::AppHandle, String)>,
) -> String
where
    R: tokio::io::AsyncRead + Unpin,
{
    let Some(pipe) = pipe else {
        return String::new();
    };
    let mut lines = BufReader::new(pipe).lines();
    let mut text = String::new();
    let mut count = 0u32;
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some((app, source_path)) = progress.as_ref() {
            if let Some(file) = line.trim().strip_prefix("note: downloading ") {
                count += 1;
                let _ = app.emit(
                    "latex-preview-progress",
                    LatexPreviewProgress {
                        source_path: source_path.clone(),
                        file: file.trim().to_string(),
                        count,
                    },
                );
            }
        }
        text.push_str(&line);
        text.push('\n');
    }
    text
}

/// 为 LaTeX 实时预览构建 PDF：
/// - `source_path`：当前 .tex 文件路径（用于定位其所在目录，保证 \input/图片等相对依赖可解析）。
/// - `content`：编辑器当前内容（未必已落盘）；写入源目录下的临时 .tex 再编译，从而支持"实时预览"。
/// 返回生成的 PDF 临时文件路径，前端用 read_file_bytes_base64 读取后用 pdf.js 渲染。
///
/// 输出路径**稳定**（见 `latex_preview_build_dir`）并保留中间文件：既让前端能增量渲染，
/// 也让多趟（目录/交叉引用）收敛更快。`.synctex.gz` 与 PDF 同目录，`synctex_path_for` 仍能派生。
#[tauri::command]
pub async fn build_tex_pdf_preview(
    app: tauri::AppHandle,
    source_path: String,
    content: String,
) -> Result<String, String> {
    let source = PathBuf::from(source_path.trim());
    let input_dir = source
        .parent()
        .map(|p| p.to_path_buf())
        .filter(|p| p.is_dir())
        .ok_or_else(|| "无法定位 .tex 文件所在目录".to_string())?;

    // 临时源文件名：放在源目录里（让相对路径依赖可用），用固定前缀避免污染。
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("preview")
        .to_string();
    let tmp_stem = format!(".guaxi_preview_{stem}");
    let tmp_tex = input_dir.join(format!("{tmp_stem}.tex"));

    fs::write(&tmp_tex, content.as_bytes())
        .map_err(|e| format!("写入预览临时文件失败: {e}"))?;

    let (tectonic_path, is_bundled) = find_tectonic_exe_for_export()?;

    let key = preview_source_key(&source_path);
    let outdir = preview_build_dir("latex", &key)?;

    // 同文档的上一轮构建先杀掉：过期构建既浪费 CPU，又可能在同一个稳定输出目录里
    // 用旧内容覆盖新结果（两次写入的先后顺序不确定）。
    kill_previous_latex_preview(&key);

    // 用 tokio 的进程 API 而非 std：std 的 `output()` 会把当前 tokio 工作线程阻塞整个
    // 编译期间（数秒~数十秒），期间其它 IPC 命令全部排队，整个界面跟着发卡。
    let mut cmd = TokioCommand::new(&tectonic_path);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    // 种子缓存让常见宏包/字体直接命中本地；biber 需要靠 PATH 才能被 tectonic 找到。
    let cache_dir = tectonic_cache_dir().await;
    apply_tectonic_child_env_tokio(&mut cmd, &tectonic_path, cache_dir.as_deref());

    cmd.current_dir(&input_dir)
        .arg("-X")
        .arg("compile")
        .arg("--outdir")
        .arg(&outdir)
        .arg("--synctex") // 生成 <stem>.synctex.gz，供内嵌阅读器做双向定位
        .arg("--keep-intermediates") // 保留 .aux/.toc：多趟交叉引用收敛更快
        .arg("--untrusted")
        .arg("-p") // 输出 chatter：下载宏包/字体时前端能显示进度
        .arg(&tmp_tex)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            let _ = fs::remove_file(&tmp_tex);
            return Err(format!(
                "启动 tectonic 失败: {e} (path={})",
                tectonic_path.display()
            ));
        }
    };
    let pid = child.id();
    if let Some(pid) = pid {
        if let Ok(mut guard) = latex_preview_pids().lock() {
            guard.insert(key.clone(), pid);
        }
    }

    let stdout_task = tokio::spawn(collect_latex_pipe(child.stdout.take(), None));
    let stderr_task = tokio::spawn(collect_latex_pipe(
        child.stderr.take(),
        Some((app.clone(), source_path.clone())),
    ));
    let status = child.wait().await;
    let stdout = stdout_task.await.unwrap_or_default();
    let stderr = stderr_task.await.unwrap_or_default();

    // 无论成功失败都清理：登记表 + 源目录里的临时 .tex。
    // 注意只在登记的还是自己这个 PID 时才删，否则会把下一轮（更新）的登记抹掉，
    // 让那个构建再也无法被后续构建取消。
    if let Some(pid) = pid {
        if let Ok(mut guard) = latex_preview_pids().lock() {
            if guard.get(&key).copied() == Some(pid) {
                guard.remove(&key);
            }
        }
    }
    let _ = fs::remove_file(&tmp_tex);

    let status = status.map_err(|e| format!("等待 tectonic 结束失败: {e}"))?;

    if !status.success() {
        let mut hint = String::new();
        if !is_bundled {
            hint.push_str("\n\nHint: 未找到内置 tectonic，可将对应平台二进制放到 src-tauri/utils 下，或确保系统 PATH 中存在 tectonic。");
        }
        return Err(format!(
            "tectonic 构建失败 (code={:?})\n{}\n{}{}",
            status.code(),
            stdout,
            stderr,
            hint
        ));
    }

    let pdf_path = outdir.join(format!("{tmp_stem}.pdf"));
    if !pdf_path.is_file() {
        return Err(format!(
            "tectonic 构建完成但未找到 PDF: {}",
            pdf_path.to_string_lossy()
        ));
    }

    // 稳定路径是**覆写**同一个文件，而 pdfium 的文档缓存只按 mtime 判断新鲜度：
    // 同一时间刻度内重建会命中旧缓存、把上一次构建的字节继续吐出来，前端就会认为"没变化"
    // 而跳过重渲。这里显式失效，与 pdf_save / pdf_add_highlight 的做法一致。
    let pdf_path_str = pdf_path.to_string_lossy().to_string();
    if let Err(e) = crate::commands::pdf_reader::invalidate_pdf(&pdf_path_str).await {
        warn!(err = %e, "[LatexPreview] 失效 PDF 缓存失败，阅读器可能仍显示上一次的渲染结果");
    }

    Ok(pdf_path_str)
}

/// 关闭文档时清掉该文件的预览构建产物，避免 `config/preview_build` 无限膨胀。
///
/// 覆盖三类残留：
/// 1. `config/preview_build/{latex,typst}/<hash>/` —— PDF 及 `.aux/.toc/.synctex` 等中间文件；
/// 2. 该 PDF 的页图缓存（直接复用 `pdf_session_close`，它同时让 pdfium 丢掉这份文档）；
/// 3. **源目录**里的 `.guaxi_preview_<stem>.tex` —— 构建用的临时源文件，构建被中途杀掉时会留下。
#[tauri::command]
pub async fn clear_preview_build(source_path: String) -> Result<(), String> {
    let raw = source_path.trim().to_string();
    if raw.is_empty() {
        return Ok(());
    }
    let source = PathBuf::from(&raw);
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    // 1. 先停掉可能还在往这些目录里写的构建，否则删完又被写回来。
    kill_previous_latex_preview(&preview_source_key(&raw));
    crate::commands::typst_compile::forget_typst_jump_cache(&raw);

    // 2. 删构建目录。键同时尝试"规范化"与"原始"两种写法：typst 最早用原始串做键，
    //    两种都试一次，免得历史目录成了谁也扫不到的残留。
    let mut keys = vec![preview_source_key(&raw)];
    if raw != keys[0] {
        keys.push(raw.clone());
    }
    for engine in ["latex", "typst"] {
        for key in &keys {
            let Ok(dir) = preview_build_dir_path(engine, key) else {
                continue;
            };
            if !dir.is_dir() {
                continue;
            }
            // 有构建正在进行时不能删：它正往这个目录里写临时文件，删掉会让它的 rename
            // 找不到源文件而报"提交预览 PDF 失败 (os error 2)"。跳过即可 —— 下次打开该文档
            // 会重建目录，关闭标签也还有后续机会清理。
            let build_lock = preview_build_lock(key);
            let Ok(_build_guard) = build_lock.try_lock() else {
                info!(engine, key, "[PreviewCleanup] 该文档正在构建，跳过本次清理");
                continue;
            };
            if !stem.is_empty() {
                // 顺带释放页图缓存并让 pdfium 丢掉这份 PDF（阅读器可能还握着上一次的字节）。
                let pdf = dir.join(format!(".guaxi_preview_{stem}.pdf"));
                let _ = crate::commands::pdf_reader::pdf_session_close(
                    pdf.to_string_lossy().to_string(),
                )
                .await;
            }
            match fs::remove_dir_all(&dir) {
                Ok(()) => info!(dir = %dir.display(), "[PreviewCleanup] 已删除预览构建目录"),
                Err(e) => warn!(dir = %dir.display(), err = %e, "[PreviewCleanup] 删除预览构建目录失败"),
            }
        }
    }

    // 3. 源目录里的临时源文件（正常收尾时 build_tex_pdf_preview 自己会删，被中断时靠这里兜底）。
    if !stem.is_empty() {
        if let Some(parent) = source.parent() {
            let _ = fs::remove_file(parent.join(format!(".guaxi_preview_{stem}.tex")));
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn export_markdown_pandoc(
    input_path: String,
    output_path: String,
    format: String,
    source_markdown: Option<String>,
) -> Result<(), String> {
    info!(
        "Exporting markdown via pandoc: input={}, output={}, format={}, source_override={}",
        input_path,
        output_path,
        format,
        source_markdown.is_some()
    );

    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to get current exe path: {}", e))?;

    let exe_dir = current_exe
        .parent()
        .ok_or("Failed to get parent directory of exe")?;

    let pandoc_path = bundled_resources::ensure_relative_dir("utils/pandoc.exe")?;
    let pandoc_path_str = pandoc_path
        .to_str()
        .ok_or("Failed to convert pandoc path to string")?;

    if !pandoc_path.exists() {
        return Err(format!(
            "pandoc executable not found at: {}",
            pandoc_path_str
        ));
    }

    let (processed_md_path, tmp_dir) =
        preprocess_markdown(&input_path, source_markdown.as_deref()).await?;
    info!(
        processed_md_path = processed_md_path.to_string_lossy().as_ref(),
        tmp_dir = tmp_dir.to_string_lossy().as_ref(),
        "[DocExport] prepared processed markdown for pandoc"
    );

    let input_dir = Path::new(&input_path)
        .parent()
        .unwrap_or(exe_dir);
    let input_dir_str = input_dir
        .to_str()
        .ok_or("Failed to convert input dir to string")?;

    let lua_filter_path = bundled_resources::ensure_relative_dir("utils/horizontalrule.lua")?;
    let use_lua_filter = lua_filter_path.exists();
    let lua_filter_str = if use_lua_filter {
        Some(
            lua_filter_path
                .to_str()
                .ok_or("Failed to convert lua filter path to string")?
                .to_string(),
        )
    } else {
        None
    };

    let mut cmd = Command::new(pandoc_path_str);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd.current_dir(input_dir)
        .arg(&processed_md_path)
        .arg("-o")
        .arg(&output_path)
        .arg("--from")
        // 设计说明：
        // 1. 导出链路需要显式保留 fenced code blocks，否则 Mermaid 代码块里以 `---` 开头的配置
        //    会被 Pandoc 误判为 YAML metadata，最终触发 “did not find expected <document start>”。
        // 2. 这里补上常用 Markdown 扩展，保证导出解析结果更接近软件内预览。
        // 3. tex_math_dollars 只认 $...$ / $$...$$；很多文档里的行间公式写成 \[...\] / \(...\)，
        //    必须额外开 tex_math_single_backslash，否则这些公式会被原样当作源码输出（EPUB 尤其明显）。
        .arg("markdown+hard_line_breaks+raw_html+implicit_figures+fenced_code_blocks+backtick_code_blocks+pipe_tables+task_lists+footnotes+tex_math_dollars+tex_math_single_backslash")
        .arg("--resource-path")
        .arg(input_dir_str)
        // 设计说明：
        // 1. Pandoc 新参数名是 `--syntax-highlighting`，但取值仍需使用它支持的内置风格。
        // 2. `highlight` 不是合法值，这里改回稳定可用的 `default`，避免导出直接失败。
        .arg("--syntax-highlighting=default");

    if format == "pdf" {
        let pdf_engine = select_pandoc_pdf_engine()?;
        let pdf_engine_arg = format!("--pdf-engine={}", pdf_engine.program.to_string_lossy());
        info!(
            engine = pdf_engine.display_name.as_str(),
            program = %pdf_engine.program.display(),
            bundled = pdf_engine.is_bundled,
            output = output_path.as_str(),
            "[DocExport] configuring pandoc pdf engine"
        );

        cmd.arg(pdf_engine_arg);

        if pdf_engine.supports_font_config {
            // 设计说明：
            // 1. 中文 PDF 导出对字体很敏感，不显式指定字体时，常见问题是中文乱码或方块字。
            // 2. 这里只在支持字体变量的引擎上注入字体设置，避免把不兼容的变量传给
            //    `pdflatex` 之类的引擎，反而引入新的导出失败。
            cmd.arg("-V")
                .arg("mainfont=Microsoft YaHei")
                .arg("-V")
                .arg("CJKmainfont=Microsoft YaHei")
                .arg("-V")
                .arg("monofont=Consolas");
        } else {
            warn!(
                engine = pdf_engine.display_name.as_str(),
                "[DocExport] current pdf engine does not support explicit CJK font variables; export result may depend on local latex defaults"
            );
        }
    } else if format == "tex" {
        cmd.arg("-t").arg("latex");
    } else if format == "txt" {
        cmd.arg("-t").arg("plain");
    }

    // 设计说明：
    // 1. Word(docx)/ODT/EPUB 这类“可导航文档”需要一份按标题层级生成的目录，
    //    导出后才能在 Word 导航窗格、电子书阅读器目录里直接跳转。
    // 2. pandoc 会把 Markdown 的 #/##/### 映射成对应的标题样式，再据此生成 TOC；
    //    `--toc-depth=3` 控制目录最深收录到三级标题，避免目录过长。
    // 3. 纯文本(txt)没有目录概念，PDF 走浏览器高保真链路（由 Chromium 生成书签大纲），
    //    都不在这里处理。
    if matches!(format.as_str(), "docx" | "odt" | "epub") {
        cmd.arg("--toc").arg("--toc-depth=3");
    }

    // 设计说明：
    // 1. EPUB 阅读器默认 CSS 下 <pre> 不折行，超出页宽的代码会被直接截断（既不折行也无横向滚动）。
    // 2. 这里写一份只作用于 EPUB 的样式，让代码块软换行 + 长 token 强制断行，保证完整可读。
    // 3. CSS 写进本次导出的临时目录（导出完成后随 tmp_dir 一起清理）。
    if format == "epub" {
        // 设计说明（数学公式）：
        // 块级公式（$$\begin{aligned}...$$ 等）改由前端用 MathJax 预渲染成 PNG 图片再交给 pandoc，
        // 因为很多 EPUB 阅读器并不渲染 MathML，复杂环境会退化成 LaTeX 源码。行内简单公式
        // 仍走 pandoc 默认（在常见阅读器里能正常显示），故这里不再传 --mathml（它会把行内公式
        // 也变成阅读器不一定支持的 MathML）。

        let epub_css_path = tmp_dir.join("guaxi-epub.css");
        let epub_css = "pre, pre code, code { white-space: pre-wrap; word-wrap: break-word; overflow-wrap: break-word; word-break: break-word; } pre { padding: 0.6em 0.8em; } img { max-width: 100%; height: auto; }";
        match std::fs::write(&epub_css_path, epub_css) {
            Ok(_) => {
                cmd.arg("--css").arg(&epub_css_path);
            }
            Err(error) => {
                warn!(
                    error = %error,
                    "[DocExport] failed to write epub wrap stylesheet; code blocks may overflow"
                );
            }
        }
    }

    if let Some(lua) = lua_filter_str {
        cmd.arg("--lua-filter").arg(lua);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute pandoc: {}", e))?;

    let stdout_text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr_text = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if output.status.success() {
        if !stdout_text.is_empty() {
            info!(stdout = stdout_text.as_str(), "[DocExport] pandoc stdout");
        }
        if !stderr_text.is_empty() {
            warn!(stderr = stderr_text.as_str(), "[DocExport] pandoc stderr");
        }
        if let Err(e) = std::fs::remove_file(&processed_md_path) {
            error!("Failed to remove temp markdown file: {:?}", e);
        }
        if let Err(e) = std::fs::remove_dir_all(&tmp_dir) {
            error!("Failed to remove temp directory: {:?}", e);
        }
        info!("Pandoc export succeeded: {}", output_path);
        Ok(())
    } else {
        error!(
            status = ?output.status.code(),
            stdout = stdout_text.as_str(),
            stderr = stderr_text.as_str(),
            processed_md_path = processed_md_path.to_string_lossy().as_ref(),
            tmp_dir = tmp_dir.to_string_lossy().as_ref(),
            "[DocExport] pandoc export failed"
        );
        Err(format!(
            "Pandoc export failed with status: {:?}. processed_md_path={}. stderr={}",
            output.status.code(),
            processed_md_path.to_string_lossy(),
            if stderr_text.is_empty() { stdout_text } else { stderr_text }
        ))
    }
}

/// 导入非 Markdown 文件时，用 pandoc 将其转换为 Markdown 后再入库。
/// 与导出链路共用同一份内置 pandoc.exe；输入格式由 pandoc 依据扩展名自动推断，
/// 输出统一为通用 Markdown。文档内嵌图片通过 `--extract-media` 抽取到 Markdown
/// 同级的 `<stem>_assets` 目录，并在正文里以相对路径引用，避免图片丢失。
#[tauri::command]
pub async fn convert_file_to_markdown_pandoc(
    input_path: String,
    output_path: String,
) -> Result<(), String> {
    info!(
        "Importing file via pandoc: input={}, output={}",
        input_path, output_path
    );

    let pandoc_path = bundled_resources::ensure_relative_dir("utils/pandoc.exe")?;
    let pandoc_path_str = pandoc_path
        .to_str()
        .ok_or("Failed to convert pandoc path to string")?;

    if !pandoc_path.exists() {
        return Err(format!(
            "pandoc executable not found at: {}",
            pandoc_path_str
        ));
    }

    if !Path::new(&input_path).exists() {
        return Err(format!("input file not found: {}", input_path));
    }

    let output_path_buf = PathBuf::from(&output_path);
    let output_dir = output_path_buf
        .parent()
        .ok_or("Failed to resolve output directory")?
        .to_path_buf();

    let stem = output_path_buf
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imported")
        .to_string();

    // 抽取内嵌媒体到 Markdown 同级的独立目录（相对路径），避免多次导入互相覆盖，
    // 也让图片能随 Markdown 一起被知识库/Git 管理。current_dir 设为输出目录，
    // pandoc 写入的媒体链接才会是可随文件迁移的相对路径。
    let media_rel = format!("{}_assets", stem);

    let mut cmd = Command::new(pandoc_path_str);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd.current_dir(&output_dir)
        .arg(&input_path)
        .arg("-o")
        .arg(&output_path)
        .arg("--to")
        // 输出通用 Markdown 并保留常用扩展（表格/任务列表/公式），
        // --wrap=none 避免 pandoc 硬折行破坏中文段落与后续编辑。
        .arg("markdown+pipe_tables+task_lists+tex_math_dollars")
        .arg("--wrap=none")
        .arg("--extract-media")
        .arg(&media_rel);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute pandoc: {}", e))?;

    let stdout_text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr_text = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if output.status.success() {
        if !stderr_text.is_empty() {
            warn!(stderr = stderr_text.as_str(), "[DocImport] pandoc stderr");
        }
        info!("Pandoc import succeeded: {}", output_path);
        Ok(())
    } else {
        error!(
            status = ?output.status.code(),
            stdout = stdout_text.as_str(),
            stderr = stderr_text.as_str(),
            "[DocImport] pandoc import failed"
        );
        Err(format!(
            "Pandoc import failed with status: {:?}. stderr={}",
            output.status.code(),
            if stderr_text.is_empty() { stdout_text } else { stderr_text }
        ))
    }
}

#[derive(Debug, Clone)]
struct PandocPdfEngine {
    program: PathBuf,
    display_name: String,
    is_bundled: bool,
    supports_font_config: bool,
}

fn can_execute_program(program: &Path) -> bool {
    let mut cmd = Command::new(program);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) => {
            let ok = status.success();
            info!(
                program = %program.display(),
                status = ?status.code(),
                ok,
                "[DocExport] probe external program"
            );
            ok
        }
        Err(err) => {
            warn!(
                program = %program.display(),
                error = %err,
                "[DocExport] external program probe failed"
            );
            false
        }
    }
}

fn select_pandoc_pdf_engine() -> Result<PandocPdfEngine, String> {
    // 设计说明：
    // 1. 过去固定写死 `xelatex`，只要用户机器上没有安装它，Pandoc 就会直接失败。
    // 2. 这里改为按顺序探测可用引擎，优先使用项目更容易控制的 `tectonic`，
    //    其次回退到常见的 `xelatex` / `lualatex` / `pdflatex`。
    // 3. 每次探测和最终选择都会打日志，后续如果某些机器仍然导出失败，可以直接从日志看出
    //    选中了哪个引擎、是否使用了内置二进制，以及失败前最后一步停在哪。
    let (tectonic_path, is_bundled_tectonic) = find_tectonic_exe_for_export()?;
    let candidates = vec![
        PandocPdfEngine {
            program: tectonic_path,
            display_name: "tectonic".to_string(),
            is_bundled: is_bundled_tectonic,
            supports_font_config: true,
        },
        PandocPdfEngine {
            program: PathBuf::from("xelatex"),
            display_name: "xelatex".to_string(),
            is_bundled: false,
            supports_font_config: true,
        },
        PandocPdfEngine {
            program: PathBuf::from("lualatex"),
            display_name: "lualatex".to_string(),
            is_bundled: false,
            supports_font_config: true,
        },
        PandocPdfEngine {
            program: PathBuf::from("pdflatex"),
            display_name: "pdflatex".to_string(),
            is_bundled: false,
            supports_font_config: false,
        },
    ];

    for candidate in candidates {
        if can_execute_program(&candidate.program) {
            info!(
                engine = candidate.display_name.as_str(),
                program = %candidate.program.display(),
                bundled = candidate.is_bundled,
                supports_font_config = candidate.supports_font_config,
                "[DocExport] selected pandoc pdf engine"
            );
            return Ok(candidate);
        }
    }

    Err("未找到可用的 PDF 引擎。请安装 xelatex/lualatex/pdflatex，或将 tectonic 放到 src-tauri/utils 后重试。".to_string())
}

fn clamp_static_toc_for_pdf(markdown: &str, max_indent_levels: usize) -> (String, bool) {
    // 设计说明：
    // 1. `preprocess_markdown_for_static_render()` 会把 `[toc]` 展开成按标题层级缩进的 Markdown 列表。
    // 2. HTML 预览可以接受很深的目录层级，但 Pandoc 导出到 LaTeX 时，过深的 `itemize`
    //    会触发 `Too deeply nested`，尤其是文档包含 5~6 级标题时。
    // 3. 这里仅在 PDF 导出链路中把 TOC 的缩进层级钳制到安全范围，避免影响编辑器里的原始预览体验。
    let toc_heading_regex = Regex::new(r"^\s{0,3}##\s+(目录|TOC)\s*$")
        .expect("valid toc heading regex");
    let toc_item_regex = Regex::new(r"^(?P<indent>\s*)-\s+\[[^\]]+\]\(#guaxi-toc-[^)]+\)\s*$")
        .expect("valid toc item regex");

    let mut lines: Vec<String> = markdown.split('\n').map(|line| line.to_string()).collect();
    let mut changed = false;
    let mut inside_toc = false;
    let mut toc_item_count = 0usize;
    let mut deepest_original_level = 0usize;

    for line in &mut lines {
        if !inside_toc {
            if toc_heading_regex.is_match(line.trim_end()) {
                inside_toc = true;
                toc_item_count = 0;
                deepest_original_level = 0;
            }
            continue;
        }

        if line.trim().is_empty() && toc_item_count == 0 {
            continue;
        }

        if let Some(captures) = toc_item_regex.captures(line) {
            let indent = captures
                .name("indent")
                .map(|m| m.as_str())
                .unwrap_or_default();
            let original_level = indent.len() / 2;
            let clamped_level = original_level.min(max_indent_levels);
            deepest_original_level = deepest_original_level.max(original_level);
            toc_item_count += 1;

            if clamped_level != original_level {
                let trimmed = line.trim_start();
                *line = format!("{}{}", "  ".repeat(clamped_level), trimmed);
                changed = true;
            }
            continue;
        }

        if line.trim().is_empty() {
            inside_toc = false;
            if toc_item_count > 0 {
                info!(
                    toc_item_count,
                    deepest_original_level,
                    max_indent_levels,
                    changed,
                    "[DocExport] finished clamping static toc for pdf"
                );
            }
            continue;
        }

        inside_toc = false;
        if toc_item_count > 0 {
            info!(
                toc_item_count,
                deepest_original_level,
                max_indent_levels,
                changed,
                "[DocExport] finished clamping static toc for pdf"
            );
        }
    }

    (lines.join("\n"), changed)
}

async fn preprocess_markdown(
    input_path: &str,
    source_override: Option<&str>,
) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    // 设计说明：
    // - 常规导出从磁盘读取 Markdown 原文。
    // - 当前端已经把 ```mermaid``` 代码块预渲染成图片并替换成图片引用时（pandoc 自身无法渲染
    //   mermaid），会通过 source_override 传入“处理过的 Markdown”，这里直接采用它，
    //   但 input_path 仍指向原文件，用于解析相对图片资源路径（--resource-path）。
    let content = match source_override {
        Some(text) => text.to_string(),
        None => std::fs::read_to_string(input_path)
            .map_err(|e| format!("读取 Markdown 失败: {}", e))?,
    };
    let static_content = preprocess_markdown_for_static_render(&content);
    let (static_content, toc_clamped) = clamp_static_toc_for_pdf(&static_content, 3);
    info!(
        input_path = input_path,
        original_len = content.len(),
        processed_len = static_content.len(),
        toc_clamped,
        "[DocExport] preprocessed markdown for static export"
    );

    let tmp_base = std::env::temp_dir().join("guaxi_pandoc");
    std::fs::create_dir_all(&tmp_base)
        .map_err(|e| format!("创建临时目录失败: {}", e))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let tmp_dir = tmp_base.join(format!("{}", now.as_millis()));
    std::fs::create_dir_all(&tmp_dir)
        .map_err(|e| format!("创建临时目录失败: {}", e))?;

    let re = Regex::new(r#"!\[([^\]]*)\]\((https?://[^)\s]+)(?:\s+\"[^\"]*\")?\)"#)
        .map_err(|e| e.to_string())?;
    let client = Client::new();

    let mut result = String::with_capacity(static_content.len());
    let mut last_end = 0;
    let mut idx = 0;

    for caps in re.captures_iter(&static_content) {
        if let Some(m) = caps.get(0) {
            result.push_str(&static_content[last_end..m.start()]);
            let alt = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let url = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            idx += 1;
            let ext = url
                .split('?')
                .next()
                .and_then(|u| u.rsplit('.').next())
                .filter(|e| !e.is_empty() && e.len() <= 5)
                .unwrap_or("png");
            let filename = format!("remote_img_{}.{}", idx, ext);
            let local_path = tmp_dir.join(&filename);

            match client.get(url).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        match resp.bytes().await {
                            Ok(bytes) => {
                                if let Err(e) = std::fs::write(&local_path, bytes) {
                                    error!("保存远程图片失败: {:?}", e);
                                }
                            }
                            Err(e) => {
                                error!("读取远程图片数据失败: {:?}", e);
                            }
                        }
                    } else {
                        error!("下载远程图片失败: {:?}", resp.status());
                    }
                }
                Err(e) => {
                    error!("请求远程图片失败: {:?}", e);
                }
            }

            let path_str = local_path.to_string_lossy().replace('\\', "/");
            result.push_str(&format!("![{}]({})", alt, path_str));
            last_end = m.end();
        }
    }

    result.push_str(&static_content[last_end..]);

    let processed_md_path = tmp_dir.join("processed.md");
    std::fs::write(&processed_md_path, &result)
        .map_err(|e| format!("写入临时 Markdown 失败: {}", e))?;

    Ok((processed_md_path, tmp_dir))
}

fn initialize_python() -> Result<(), String> {
    pyo3::prepare_freethreaded_python();
    Ok(())
}

fn handle_pdf(filepath: String, curpage: u32) -> Result<(String, u32), String> {
    // 初始化 Python 解释器
    initialize_python()?;

    // 在 Python GIL 上下文中执行
    Python::with_gil(|py| {
        // 获取当前工作目录
        let current_dir = std::env::current_dir().map_err(|e| {
            error!("Failed to get current directory: {:?}", e);
            format!("Failed to get current directory: {:?}", e)
        })?;

        // 构建脚本目录的绝对路径
        let scripts_dir = current_dir.join("scripts");
        let scripts_dir_str = scripts_dir
            .to_str()
            .ok_or("Failed to convert path to string")?;

        // 导入 sys 模块
        let sys = PyModule::import(py, "sys").map_err(|e| {
            error!("Failed to import sys module: {:?}", e);
            format!("Failed to import sys module: {:?}", e)
        })?;

        // 获取 sys.path
        let sys_path = sys.getattr("path").map_err(|e| {
            error!("Failed to get sys.path: {:?}", e);
            format!("Failed to get sys.path: {:?}", e)
        })?;

        // 将 sys.path 转换为 Bound<PyList>
        let sys_path: &Bound<'_, PyList> = sys_path.downcast().map_err(|e| {
            error!("Failed to downcast sys.path to PyList: {:?}", e);
            format!("Failed to downcast sys.path to PyList: {:?}", e)
        })?;

        // 打印当前的 sys.path
        info!("Current sys.path: {:?}", sys_path);

        // 将脚本目录添加到 sys.path
        sys_path.append(scripts_dir_str).map_err(|e| {
            error!("Failed to add path to sys.path: {:?}", e);
            format!("Failed to add path to sys.path: {:?}", e)
        })?;

        // 打印更新后的 sys.path
        info!("Updated sys.path: {:?}", sys_path);

        // 导入 pdf_extractor 模块
        let pdf_extractor = PyModule::import(py, "pdf_extractor").map_err(|e| {
            error!("Failed to import pdf_extractor module: {:?}", e);
            format!("Failed to import pdf_extractor module: {:?}", e)
        })?;

        // 调用 extract_pdf_page 函数
        let result: (String, u32) = pdf_extractor
            .getattr("extract_pdf_page")
            .map_err(|e| {
                error!("Failed to get extract_pdf_page function: {:?}", e);
                format!("Failed to get extract_pdf_page function: {:?}", e)
            })?
            .call1((filepath, curpage as i32))
            .map_err(|e| {
                error!("Failed to call extract_pdf_page function: {:?}", e);
                format!("Failed to call extract_pdf_page function: {:?}", e)
            })?
            .extract()
            .map_err(|e| {
                error!("Failed to extract content and totalpage: {:?}", e);
                format!("Failed to extract content and totalpage: {:?}", e)
            })?;

        info!("Extracted content: {}", result.0);
        Ok(result)
    })
}

/// 处理 Markdown 文件
#[allow(dead_code)]
fn handle_md(filepath: String, curpage: u32) -> Result<(String, u32), String> {
    // TODO: 实现 Markdown 文件处理逻辑
    // 初始化 Python 解释器
    initialize_python()?;

    // 在 Python GIL 上下文中执行
    Python::with_gil(|py| {
        // 获取当前工作目录
        let current_dir = std::env::current_dir().map_err(|e| {
            error!("Failed to get current directory: {:?}", e);
            format!("Failed to get current directory: {:?}", e)
        })?;

        // 构建脚本目录的绝对路径
        let scripts_dir = current_dir.join("scripts");
        let scripts_dir_str = scripts_dir
            .to_str()
            .ok_or("Failed to convert path to string")?;

        // 导入 sys 模块
        let sys = PyModule::import(py, "sys").map_err(|e| {
            error!("Failed to import sys module: {:?}", e);
            format!("Failed to import sys module: {:?}", e)
        })?;

        // 获取 sys.path
        let sys_path = sys.getattr("path").map_err(|e| {
            error!("Failed to get sys.path: {:?}", e);
            format!("Failed to get sys.path: {:?}", e)
        })?;

        // 将 sys.path 转换为 Bound<PyList>
        let sys_path: &Bound<'_, PyList> = sys_path.downcast().map_err(|e| {
            error!("Failed to downcast sys.path to PyList: {:?}", e);
            format!("Failed to downcast sys.path to PyList: {:?}", e)
        })?;

        // 打印当前的 sys.path
        // info!("Current sys.path: {:?}", sys_path);

        // 将脚本目录添加到 sys.path
        sys_path.append(scripts_dir_str).map_err(|e| {
            error!("Failed to add path to sys.path: {:?}", e);
            format!("Failed to add path to sys.path: {:?}", e)
        })?;

        // 打印更新后的 sys.path
        info!("Updated sys.path: {:?}", sys_path);

        // 导入 pdf_extractor 模块
        let pdf_extractor = PyModule::import(py, "pdf_extractor").map_err(|e| {
            error!("Failed to import pdf_extractor module: {:?}", e);
            format!("Failed to import pdf_extractor module: {:?}", e)
        })?;

        // 调用 extract_pdf_page 函数
        let result: (String, u32) = pdf_extractor
            .getattr("extract_pdf_page")
            .map_err(|e| {
                error!("Failed to get extract_pdf_page function: {:?}", e);
                format!("Failed to get extract_pdf_page function: {:?}", e)
            })?
            .call1((filepath, curpage as i32))
            .map_err(|e| {
                error!("Failed to call extract_pdf_page function: {:?}", e);
                format!("Failed to call extract_pdf_page function: {:?}", e)
            })?
            .extract()
            .map_err(|e| {
                error!("Failed to extract content and totalpage: {:?}", e);
                format!("Failed to extract content and totalpage: {:?}", e)
            })?;

        Ok(result)
    })
}

/// 处理文本文件
#[allow(dead_code)]
fn handle_txt(filepath: String, curpage: u32) -> Result<(String, u32), String> {
    // TODO: 实现文本文件处理逻辑
    // 初始化 Python 解释器
    initialize_python()?;

    // 在 Python GIL 上下文中执行
    Python::with_gil(|py| {
        // 获取当前工作目录
        let current_dir = std::env::current_dir().map_err(|e| {
            error!("Failed to get current directory: {:?}", e);
            format!("Failed to get current directory: {:?}", e)
        })?;

        // 构建脚本目录的绝对路径
        let scripts_dir = current_dir.join("scripts");
        let scripts_dir_str = scripts_dir
            .to_str()
            .ok_or("Failed to convert path to string")?;

        // 导入 sys 模块
        let sys = PyModule::import(py, "sys").map_err(|e| {
            error!("Failed to import sys module: {:?}", e);
            format!("Failed to import sys module: {:?}", e)
        })?;

        // 获取 sys.path
        let sys_path = sys.getattr("path").map_err(|e| {
            error!("Failed to get sys.path: {:?}", e);
            format!("Failed to get sys.path: {:?}", e)
        })?;

        // 将 sys.path 转换为 Bound<PyList>
        let sys_path: &Bound<'_, PyList> = sys_path.downcast().map_err(|e| {
            error!("Failed to downcast sys.path to PyList: {:?}", e);
            format!("Failed to downcast sys.path to PyList: {:?}", e)
        })?;

        // 打印当前的 sys.path
        // info!("Current sys.path: {:?}", sys_path);

        // 将脚本目录添加到 sys.path
        sys_path.append(scripts_dir_str).map_err(|e| {
            error!("Failed to add path to sys.path: {:?}", e);
            format!("Failed to add path to sys.path: {:?}", e)
        })?;

        // 打印更新后的 sys.path
        // info!("Updated sys.path: {:?}", sys_path);

        // 导入 pdf_extractor 模块
        let pdf_extractor = PyModule::import(py, "pdf_extractor").map_err(|e| {
            error!("Failed to import pdf_extractor module: {:?}", e);
            format!("Failed to import pdf_extractor module: {:?}", e)
        })?;

        // 调用 extract_pdf_page 函数
        let result: (String, u32) = pdf_extractor
            .getattr("extract_pdf_page")
            .map_err(|e| {
                error!("Failed to get extract_pdf_page function: {:?}", e);
                format!("Failed to get extract_pdf_page function: {:?}", e)
            })?
            .call1((filepath, curpage as i32))
            .map_err(|e| {
                error!("Failed to call extract_pdf_page function: {:?}", e);
                format!("Failed to call extract_pdf_page function: {:?}", e)
            })?
            .extract()
            .map_err(|e| {
                error!("Failed to extract content and totalpage: {:?}", e);
                format!("Failed to extract content and totalpage: {:?}", e)
            })?;

        Ok(result)
    })
}

/// 处理 Word 文件
#[allow(dead_code)]
fn handle_docx(filepath: String, curpage: u32) -> Result<(String, u32), String> {
    // TODO: 实现 Word 文件处理逻辑
    // 初始化 Python 解释器
    initialize_python()?;

    // 在 Python GIL 上下文中执行
    Python::with_gil(|py| {
        // 获取当前工作目录
        let current_dir = std::env::current_dir().map_err(|e| {
            error!("Failed to get current directory: {:?}", e);
            format!("Failed to get current directory: {:?}", e)
        })?;

        // 构建脚本目录的绝对路径
        let scripts_dir = current_dir.join("scripts");
        let scripts_dir_str = scripts_dir
            .to_str()
            .ok_or("Failed to convert path to string")?;

        // 导入 sys 模块
        let sys = PyModule::import(py, "sys").map_err(|e| {
            error!("Failed to import sys module: {:?}", e);
            format!("Failed to import sys module: {:?}", e)
        })?;

        // 获取 sys.path
        let sys_path = sys.getattr("path").map_err(|e| {
            error!("Failed to get sys.path: {:?}", e);
            format!("Failed to get sys.path: {:?}", e)
        })?;

        // 将 sys.path 转换为 Bound<PyList>
        let sys_path: &Bound<'_, PyList> = sys_path.downcast().map_err(|e| {
            error!("Failed to downcast sys.path to PyList: {:?}", e);
            format!("Failed to downcast sys.path to PyList: {:?}", e)
        })?;

        // 打印当前的 sys.path
        // info!("Current sys.path: {:?}", sys_path);

        // 将脚本目录添加到 sys.path
        sys_path.append(scripts_dir_str).map_err(|e| {
            error!("Failed to add path to sys.path: {:?}", e);
            format!("Failed to add path to sys.path: {:?}", e)
        })?;

        // 打印更新后的 sys.path
        // info!("Updated sys.path: {:?}", sys_path);

        // 导入 pdf_extractor 模块
        let pdf_extractor = PyModule::import(py, "pdf_extractor").map_err(|e| {
            error!("Failed to import pdf_extractor module: {:?}", e);
            format!("Failed to import pdf_extractor module: {:?}", e)
        })?;

        // 调用 extract_pdf_page 函数
        let result: (String, u32) = pdf_extractor
            .getattr("extract_pdf_page")
            .map_err(|e| {
                error!("Failed to get extract_pdf_page function: {:?}", e);
                format!("Failed to get extract_pdf_page function: {:?}", e)
            })?
            .call1((filepath, curpage as i32))
            .map_err(|e| {
                error!("Failed to call extract_pdf_page function: {:?}", e);
                format!("Failed to call extract_pdf_page function: {:?}", e)
            })?
            .extract()
            .map_err(|e| {
                error!("Failed to extract content and totalpage: {:?}", e);
                format!("Failed to extract content and totalpage: {:?}", e)
            })?;

        Ok(result)
    })
}

#[tauri::command]
pub async fn reader_open_file(request: OpenFileRequest) -> Result<OpenFileResponse, String> {
    info!("Open file request: {:?}", request);

    let (content, totalpages) = match request.filetype.as_str() {
        "pdf" => handle_pdf(request.filepath, request.curpage)?,
        _ => return Err(format!("Unsupported file type: {}", request.filetype)),
    };

    // 检查Python返回的内容是否包含错误
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
        if parsed.get("error").is_some() {
            return Ok(OpenFileResponse {
                success: false,
                filetype: request.filetype,
                content,
                curpage: request.curpage,
                lastpage: request.lastpage,
                totalpages,
                error: parsed["error"].as_str().map(|s| s.to_string()),
            });
        }
    }

    Ok(OpenFileResponse {
        success: true,
        filetype: request.filetype,
        content,
        curpage: request.curpage,
        lastpage: request.lastpage,
        totalpages,
        error: None,
    })
}
