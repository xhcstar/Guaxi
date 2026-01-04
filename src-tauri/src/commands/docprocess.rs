use pdfium_render::prelude::*;
use pyo3::prelude::*;
use pyo3::types::PyList; // 导入 PyList 类型
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Cursor;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use tauri::{Emitter, Window};
use tracing::{error, info};
use image::ImageFormat; // 确保已导入 ImageFormat
use base64::Engine as _; // 使用新的 base64 编码方式
use base64::engine::general_purpose;
use svg::node::element::{Rectangle, Text as SvgText};
use svg::Document;
use std::path::PathBuf;
use regex::Regex;
use reqwest::Client;

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
            // 使用更精确的分页符匹配
            let pages: Vec<&str> = content.split("\n{").collect();
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
    info!("Converting file to markdown: {:?}", request);

    // 发送转换开始事件
    window
        .emit(
            "conversion_progress",
            ConversionProgress {
                current: 0,
                total: 100,
                message: "开始转换...".to_string(),
            },
        )
        .map_err(|e| format!("Failed to emit start progress: {}", e))?;

    // 获取当前可执行文件路径
    let current_exe =
        std::env::current_exe().map_err(|e| format!("Failed to get current exe path: {}", e))?;

    // 获取可执行文件所在目录
    let exe_dir = current_exe
        .parent()
        .ok_or("Failed to get parent directory of exe")?;

    // 构建转换器路径
    let converter_path = exe_dir.join("utils").join("Guaxi_doc_convertor.exe");
    let converter_path_str = converter_path
        .to_str()
        .ok_or("Failed to convert converter path to string")?;

    // 创建子进程并捕获输出
    let mut child = Command::new(converter_path_str)
        .arg(&request.input_path)
        .arg("--output_format")
        .arg("markdown")
        .arg("--disable_multiprocessing")
        .arg("--paginate_output")
        .arg("--output_dir")
        .arg(&request.output_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to execute converter: {}", e))?;

    // 先获取标准输出和标准错误的句柄
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Create async tasks for output streams
    let window_clone_stdout = window.clone();
    let stdout_task = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            let mut reader = BufReader::new(stdout);
            let mut buffer = Vec::new();

            loop {
                buffer.clear();
                match reader.read_until(b'\n', &mut buffer) {
                    Ok(0) => break,
                    Ok(_) => {
                        let filtered: Vec<u8> = buffer
                            .iter()
                            .filter(|&&b| b.is_ascii() && !b.is_ascii_control())
                            .cloned()
                            .collect();

                        let line = String::from_utf8_lossy(&filtered).trim().to_string();

                        if !line.is_empty() {
                            info!("Converter output: {}", line);

                            if let Some(percent) = parse_progress(&line) {
                                window_clone_stdout
                                    .emit(
                                        "conversion_progress",
                                        ConversionProgress {
                                            current: percent,
                                            total: 100,
                                            message: format!("转换中... {}%", percent),
                                        },
                                    )
                                    .map_err(|e| error!("Failed to emit progress: {}", e))
                                    .ok();
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to read output: {}", e);
                        break;
                    }
                }
            }
        }
    });

    // Handle stderr output
    let window_clone_stderr = window.clone();
    let stderr_task = tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let mut reader = BufReader::new(stderr);
            let mut buffer = Vec::new();

            loop {
                buffer.clear();
                match reader.read_until(b'\n', &mut buffer) {
                    Ok(0) => break,
                    Ok(_) => {
                        let filtered: Vec<u8> = buffer
                            .iter()
                            .filter(|&&b| b.is_ascii() && !b.is_ascii_control())
                            .cloned()
                            .collect();

                        let line = String::from_utf8_lossy(&filtered).trim().to_string();

                        if !line.is_empty() {
                            error!("Converter stderr: {}", line);

                            if let Some(percent) = parse_progress(&line) {
                                window_clone_stderr
                                    .emit(
                                        "conversion_progress",
                                        ConversionProgress {
                                            current: percent,
                                            total: 100,
                                            message: format!("转换中... {}%", percent),
                                        },
                                    )
                                    .map_err(|e| error!("Failed to emit progress: {}", e))
                                    .ok();
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to read stderr: {}", e);
                        break;
                    }
                }
            }
        }
    });

    // 等待子进程结束
    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for converter: {}", e))?;

    // 等待输出处理任务完成
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    if status.success() {
        window
            .emit(
                "conversion_progress",
                ConversionProgress {
                    current: 100,
                    total: 100,
                    message: "转换完成".to_string(),
                },
            )
            .map_err(|e| format!("Failed to emit completion: {}", e))?;
        Ok(())
    } else {
        Err("Conversion failed".to_string())
    }
}

fn parse_progress(line: &str) -> Option<u32> {
    info!("Parsing progress: {}", line);

    // 使用更精确的字符串匹配
    if line.contains("Loaded ") {
        return Some(10);
    } else if line.contains("Recognizing layout") {
        return Some(25);
    } else if line.contains("Running OCR Error Detection") {
        return Some(35);
    } else if line.contains("Texify inference started") {
        return Some(50);
    } else if line.contains("Recognizing Text") {
        return Some(70);
    } else if line.contains("Recognizing tables") {
        return Some(85);
    } else if line.contains("Saving markdown to") {
        return Some(95);
    }

    None
}

#[tauri::command]
pub async fn export_markdown_pdf_pandoc(
    input_path: String,
    output_path: String,
) -> Result<(), String> {
    export_markdown_pandoc(input_path, output_path, "pdf".to_string()).await
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
