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
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;
use tokio::time::{sleep, Duration};

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

#[allow(dead_code)]
fn pick_free_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener
        .local_addr()
        .map_err(|e| e.to_string())?
        .port();
    Ok(port)
}

fn resolve_marker_source_dir(exe_dir: &Path) -> Result<PathBuf, String> {
    let p = exe_dir
        .join("utils")
        .join("MarkdownConvertor")
        .join("marker-1.10.1");
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

    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| "Failed to get parent directory of exe".to_string())?;

    let marker_src_dir = resolve_marker_source_dir(exe_dir)?;
    let python_path = resolve_marker_python_path(&marker_src_dir)?
        .to_string_lossy()
        .to_string();

    info!(
        exe_dir = exe_dir.to_string_lossy().as_ref(),
        marker_src_dir = marker_src_dir.to_string_lossy().as_ref(),
        python_path = python_path.as_str(),
        "marker server resolved paths"
    );

    let model_dir = exe_dir
        .join("utils")
        .join("Models")
        .join("surya_models");
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


const PDFIUM_LIB_PATH: &str = if cfg!(debug_assertions) {
    "../../libs"
} else {
    "./libs"
};

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

    // 初始化PDFium
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(
            PDFIUM_LIB_PATH,
        ))
        .map_err(|e| format!("PDFium初始化失败: {}", e))?,
    );

    // 加载文档
    let document = pdfium.load_pdf_from_file(&req.filepath, None)
        .map_err(|e| format!("PDF加载失败: {}", e))?;

    let total_pages = document.pages().len() as usize;
    if req.curpage < 1 || req.curpage > total_pages {
        return Err(format!("页码 {} 超出范围(总页数: {})", req.curpage, total_pages));
    }

    // 获取目标页
    let page = document.pages().get((req.curpage - 1) as u16)
        .map_err(|_| format!("获取页面 {} 失败", req.curpage))?;

    // 根据模式处理
    let response = match req.mode.as_str() {
        "vector" => render_vector(&page, req.curpage, total_pages, req.scale),
        "hybrid" => render_hybrid(&page, req.curpage, total_pages, req.scale),
        _ => render_image(&page, req.curpage, total_pages, req.scale),
    };

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

    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| "Failed to get parent directory of exe".to_string())?;

    let marker_src_dir = resolve_marker_source_dir(exe_dir)?;
    let python_path = resolve_marker_python_path(&marker_src_dir)?
        .to_string_lossy()
        .to_string();

    let model_dir = exe_dir
        .join("utils")
        .join("Models")
        .join("surya_models");
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
    export_markdown_pandoc(input_path, output_path, "pdf".to_string()).await
}

fn find_tectonic_exe_for_export() -> Result<(PathBuf, bool), String> {
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to get current exe path: {e}"))?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| "Failed to get parent directory of exe".to_string())?;

    #[cfg(windows)]
    {
        let bundled = exe_dir.join("utils").join("tectonic.exe");
        if bundled.is_file() {
            return Ok((bundled, true));
        }
        return Ok((PathBuf::from("tectonic"), false));
    }

    #[cfg(not(windows))]
    {
        let bundled = exe_dir.join("utils").join("tectonic");
        if bundled.is_file() {
            return Ok((bundled, true));
        }
        return Ok((PathBuf::from("tectonic"), false));
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

#[tauri::command]
pub async fn export_markdown_pandoc(
    input_path: String,
    output_path: String,
    format: String,
) -> Result<(), String> {
    info!(
        "Exporting markdown via pandoc: input={}, output={}, format={}",
        input_path, output_path, format
    );

    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to get current exe path: {}", e))?;

    let exe_dir = current_exe
        .parent()
        .ok_or("Failed to get parent directory of exe")?;

    let pandoc_path = exe_dir.join("utils").join("pandoc.exe");
    let pandoc_path_str = pandoc_path
        .to_str()
        .ok_or("Failed to convert pandoc path to string")?;

    if !pandoc_path.exists() {
        return Err(format!(
            "pandoc executable not found at: {}",
            pandoc_path_str
        ));
    }

    let (processed_md_path, tmp_dir) = preprocess_markdown(&input_path).await?;

    let input_dir = Path::new(&input_path)
        .parent()
        .unwrap_or(exe_dir);
    let input_dir_str = input_dir
        .to_str()
        .ok_or("Failed to convert input dir to string")?;

    let lua_filter_path = exe_dir
        .join("utils")
        .join("horizontalrule.lua");
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
        .arg("markdown+hard_line_breaks+raw_html+implicit_figures")
        .arg("--resource-path")
        .arg(input_dir_str)
        .arg("--highlight-style=tango");

    if format == "pdf" {
        cmd.arg("--pdf-engine=xelatex")
            .arg("-V")
            .arg("mainfont=Microsoft YaHei")
            .arg("-V")
            .arg("CJKmainfont=Microsoft YaHei")
            .arg("-V")
            .arg("monofont=Consolas");
    } else if format == "tex" {
        cmd.arg("-t").arg("latex");
    } else if format == "txt" {
        cmd.arg("-t").arg("plain");
    }

    if let Some(lua) = lua_filter_str {
        cmd.arg("--lua-filter").arg(lua);
    }

    let status = cmd
        .status()
        .map_err(|e| format!("Failed to execute pandoc: {}", e))?;

    if let Err(e) = std::fs::remove_file(&processed_md_path) {
        error!("Failed to remove temp markdown file: {:?}", e);
    }
    if let Err(e) = std::fs::remove_dir_all(&tmp_dir) {
        error!("Failed to remove temp directory: {:?}", e);
    }

    if status.success() {
        info!("Pandoc export succeeded: {}", output_path);
        Ok(())
    } else {
        Err(format!("Pandoc export failed with status: {:?}", status.code()))
    }
}

async fn preprocess_markdown(
    input_path: &str,
) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    let content = std::fs::read_to_string(input_path)
        .map_err(|e| format!("读取 Markdown 失败: {}", e))?;

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

    let mut result = String::with_capacity(content.len());
    let mut last_end = 0;
    let mut idx = 0;

    for caps in re.captures_iter(&content) {
        if let Some(m) = caps.get(0) {
            result.push_str(&content[last_end..m.start()]);
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

    result.push_str(&content[last_end..]);

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
