use pyo3::prelude::*;
use pyo3::types::PyList; // 导入 PyList 类型
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use std::path::Path;

#[derive(Deserialize, Debug)]
pub struct OpenFileRequest {
    filetype: String, // 文件类型：pdf, md, docx, txt
    filepath: String,
    curpage: u32,
    lastpage: u32,
}

#[derive(Serialize, Debug)]
pub struct OpenFileResponse {
    filetype: String, // 文件类型：pdf, md, docx, txt
    content: String,
    curpage: u32,
    lastpage: u32,
    totalpages: u32,
}

// pub struct FileProcessorRespond {
//     content: String,
//     totalpages: u32,
// }

/// 初始化 Python 解释器
fn initialize_python() -> Result<(), String> {
    pyo3::prepare_freethreaded_python();
    Ok(())
}

/// 处理 PDF 文件
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
        let scripts_dir_str = scripts_dir.to_str().ok_or("Failed to convert path to string")?;

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
        let scripts_dir_str = scripts_dir.to_str().ok_or("Failed to convert path to string")?;

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
        let scripts_dir_str = scripts_dir.to_str().ok_or("Failed to convert path to string")?;

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
        let scripts_dir_str = scripts_dir.to_str().ok_or("Failed to convert path to string")?;

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
pub async fn openfile(request: OpenFileRequest) -> Result<OpenFileResponse, String> {
    info!("Open file request: {:?}", request);

    // 根据文件类型调用对应的处理函数
    let file_processor_respond = match request.filetype.as_str() {
        "pdf" => handle_pdf(request.filepath, request.curpage)?,
        "md" => handle_md(request.filepath, request.curpage)?,
        "txt" => handle_txt(request.filepath, request.curpage)?,
        "docx" => handle_docx(request.filepath, request.curpage)?,
        _ => return Err(format!("Unsupported file type: {}", request.filetype)),
    };

    // 返回响应
    Ok(OpenFileResponse {
        filetype: request.filetype,
        content: file_processor_respond.0,
        curpage: request.curpage,
        lastpage: request.lastpage,
        totalpages: file_processor_respond.1,
    })
}