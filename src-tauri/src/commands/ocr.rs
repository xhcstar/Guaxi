//! 离线 OCR —— 转发到 agent_server 的 /ocr 端点（rapidocr-onnxruntime）。
//!
//! 复用现有 agent_server 进程管理：首次调用会自动把 rapidocr 依赖装进托管
//! .venv 并拉起 FastAPI 服务（127.0.0.1:9876），无需额外运行时。

use base64::Engine;
use serde_json::json;
use std::process::Command;
use std::time::Duration;
use tauri::AppHandle;

const AGENT_SERVER_URL: &str = "http://127.0.0.1:9876";

/// 调 agent_server /ocr 端点识别指定路径的图片，返回 `{lines, text}`。
async fn call_ocr_endpoint(app: &AppHandle, image_path: &str) -> Result<serde_json::Value, String> {
    // 复用现有启动逻辑：首次调用会自动安装 rapidocr 依赖并拉起服务。
    crate::commands::agent_server::ensure_agent_server_started(app).await?;

    let resp = reqwest::Client::new()
        .post(format!("{}/ocr", AGENT_SERVER_URL))
        .json(&json!({ "image_path": image_path }))
        // 首次请求含模型加载，给足超时。
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| format!("OCR 请求失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("OCR 服务返回错误: {}", resp.status()));
    }

    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| format!("解析 OCR 结果失败: {}", e))
}

/// 识别图片文字。
///
/// `image_path` 与 `image_base64` 二选一；base64 兼容 `data:image/png;base64,` 前缀。
/// 返回 `{ lines: [{ text, confidence, box }], text }`。
#[tauri::command]
pub async fn ocr_recognize(
    app: AppHandle,
    image_path: Option<String>,
    image_base64: Option<String>,
) -> Result<serde_json::Value, String> {
    // 复用现有启动逻辑：首次调用会自动安装 rapidocr 依赖并拉起服务。
    crate::commands::agent_server::ensure_agent_server_started(&app).await?;

    let resp = reqwest::Client::new()
        .post(format!("{}/ocr", AGENT_SERVER_URL))
        .json(&json!({
            "image_path": image_path.unwrap_or_default(),
            "image_base64": image_base64.unwrap_or_default(),
        }))
        // 首次请求含模型加载，给足超时。
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| format!("OCR 请求失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("OCR 服务返回错误: {}", resp.status()));
    }

    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| format!("解析 OCR 结果失败: {}", e))
}

/// 截图识别：拉起系统截图工具让用户框选，将截图存到临时文件后直接 OCR。
///
/// 返回 `{ cancelled, lines, text, image_base64 }`：
/// - 用户取消（未产生截图）时返回 `cancelled: true`，其余为空；
/// - `image_base64` 为 data URL，供前端预览（临时文件不在 assetProtocol scope 内，
///   故以 base64 回传而非路径）。识别完成后临时文件会被删除。
#[tauri::command]
pub async fn screenshot_ocr(app: AppHandle) -> Result<serde_json::Value, String> {
    // 1. 解析截图工具路径（与 guaxiscreencap 保持一致，位于 exe 同级 utils/）。
    let current_exe = std::env::current_exe().map_err(|e| format!("获取程序路径失败: {}", e))?;
    let tool = current_exe
        .parent()
        .ok_or("获取程序目录失败")?
        .join("utils")
        .join("ScreenShotTool.exe");
    if !tool.exists() {
        return Err(format!("未找到截图工具: {}", tool.display()));
    }

    // 2. 生成唯一临时文件路径。
    let temp_path = std::env::temp_dir().join(format!("guaxi_ocr_{}.png", uuid::Uuid::new_v4()));
    let temp_str = temp_path.to_string_lossy().to_string();

    // 3. 运行截图工具，阻塞直到用户完成框选或取消（放到阻塞线程池，避免占用异步执行器线程）。
    let output = {
        let tool = tool.clone();
        let temp_str = temp_str.clone();
        tokio::task::spawn_blocking(move || Command::new(&tool).arg(&temp_str).output())
            .await
            .map_err(|e| format!("截图任务失败: {}", e))?
            .map_err(|e| format!("启动截图工具失败: {}", e))?
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("截图工具执行失败: {}", stderr));
    }

    // 4. 用户可能取消（未产生文件）→ 返回 cancelled，不当作错误。
    let produced = std::fs::metadata(&temp_path)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false);
    if !produced {
        return Ok(json!({ "cancelled": true, "lines": [], "text": "", "image_base64": "" }));
    }

    // 5. 读取截图字节用于预览（data URL）。读取失败不阻断识别。
    let image_base64 = match std::fs::read(&temp_path) {
        Ok(bytes) => {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            format!("data:image/png;base64,{}", b64)
        }
        Err(_) => String::new(),
    };

    // 6. OCR 识别（走文件路径，Python 进程直接读盘，无 asset scope 限制）。
    let ocr_result = call_ocr_endpoint(&app, &temp_str).await;

    // 7. 清理临时文件（无论识别成败）。
    let _ = std::fs::remove_file(&temp_path);

    let mut result = ocr_result?;
    if let Some(obj) = result.as_object_mut() {
        obj.insert("cancelled".to_string(), json!(false));
        obj.insert("image_base64".to_string(), json!(image_base64));
    }
    Ok(result)
}
