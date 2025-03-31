use pyo3::prelude::*;
use pyo3::types::PyList; // 导入 PyList 类型
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use tauri::{Emitter, Window};
use tracing::{error, info};
use pdfium_render::prelude::*;

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

#[derive(Serialize, Deserialize)]
struct PdfElement {
    #[serde(rename = "type")]
    element_type: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
struct PdfPageResponse {
    page_number: usize,
    total_pages: usize,
    elements: Vec<PdfElement>,
    error: Option<String>,
}

#[tauri::command]
pub async fn get_converted_pdf_page(
    request: serde_json::Value
) -> Result<String, String> {
    use std::io::Cursor;

    let filepath = request["filepath"].as_str().ok_or("Missing filepath")?;
    let curpage = request["curpage"].as_u64().ok_or("Invalid page number")? as usize;

    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
            .map_err(|e| e.to_string())?
    );

    let document = pdfium.load_pdf_from_file(filepath, None)
        .map_err(|e| e.to_string())?;

    let total_pages = document.pages().len();
    let page = document.pages().get((curpage - 1).try_into().map_err(|_| "Page number conversion failed")?)
        .map_err(|_| format!("Page {} out of bounds", curpage))?;

    let mut elements = Vec::new();

    // 提取文本元素（使用all()方法获取完整文本）
    match page.text() {
        Ok(text) => {
            let full_text = text.all(); // 获取页面全部文本
            elements.push(PdfElement {
                element_type: "text".to_string(),
                content: full_text,
            });
        }
        Err(e) => return Err(e.to_string()),
    }

    // 提取图像元素
    let render_config = PdfRenderConfig::new()
        .set_target_width(2000)
        .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

    if let Ok(image) = page.render_with_config(&render_config) {
        let mut bytes = Vec::new();
        let mut cursor = Cursor::new(&mut bytes);
        if image.as_image().into_rgb8()
            .write_to(&mut cursor, image::ImageFormat::Png)
            .is_ok() 
        {
            elements.push(PdfElement {
                element_type: "image".to_string(),
                content: base64::encode(&bytes),
            });
        }
    }

    // 构建响应
    let response = PdfPageResponse {
        page_number: curpage,
        total_pages: total_pages.into(), // 处理u16到usize的转换
        elements,
        error: None,
    };

    serde_json::to_string(&response).map_err(|e| e.to_string())
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

// "Recognizing layout" -> 20%
// "Running OCR Error Detection: " -> 30%
// "Detecting bboxes: " -> 40%
// "Texify inference:" -> 50%
// "Detecting bboxes: " -> 70%
// "Recognizing tables:" -> 80%
// "Saved markdown to " -> 90%

fn parse_progress(line: &str) -> Option<u32> {
    info!("Parsing progress: {}", line);

    // 使用更精确的字符串匹配
    if line.contains("Loaded ") {
        return Some(10);
    } else if line.contains("Recognizing layout...") {
        return Some(25);
    } else if line.contains("Running OCR Error Detection") {
        return Some(35);
    } else if line.contains("First pass: Detecting bboxes") {
        return Some(45);
    } else if line.contains("Texify inference started") {
        return Some(55);
    } else if line.contains("Second pass: Detecting bboxes") {
        return Some(65);
    } else if line.contains("Recognizing tables") {
        return Some(75);
    } else if line.contains("Saving markdown to") {
        return Some(85);
    }

    None
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