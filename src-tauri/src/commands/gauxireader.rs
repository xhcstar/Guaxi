use std::sync::Mutex;
use tauri::Manager;
use tauri::{WebviewWindowBuilder, WebviewUrl};
use tracing::info;
use urlencoding::encode;
use serde::{Deserialize, Serialize};


// #[tauri::command]
// pub async fn create_reader_window(
//     app: tauri::AppHandle,
//     label: String,
//     title: String,
//     file_data: serde_json::Value,
// ) -> Result<(), String> {
//     // 检查窗口是否已存在
//     if app.get_webview_window(&label).is_some() {
//         return Err(format!("窗口 {} 已存在", label));
//     }

//     let url = WebviewUrl::App(format!(
//         "/reader?fileType={}&fileName={}",
//         encode(&file_data["type"].as_str().unwrap_or("unknown")),
//         encode(&file_data["name"].as_str().unwrap_or("unknown"))
//     ).into());

//     info!("打开文件 {}" , file_data["name"].as_str().unwrap_or("unknown"));

//     let window = WebviewWindowBuilder::new(&app, &label, url)
//         .title(title)
//         .inner_size(1200.0, 800.0)
//         .min_inner_size(800.0, 600.0)
//         .visible(false)
//         .build()
//         .map_err(|e| format!("创建窗口失败: {}", e))?;

//     // 短暂延迟确保窗口初始化完成
//     tokio::time::sleep(std::time::Duration::from_millis(100)).await;
//     window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
    
//     Ok(())
// }

// // 数据存储结构（需提前注册）
// struct WindowDataStorage {
//     data: Mutex<std::collections::HashMap<String, serde_json::Value>>,
// }


// 定义文件数据结构
#[derive(Debug, Serialize, Deserialize)]
pub struct FileData {
    file_type: String,
    file_name: String,
    file_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
}

// 窗口数据存储
pub struct WindowDataStorage {
    data: Mutex<std::collections::HashMap<String, FileData>>,
}

#[tauri::command]
pub async fn create_reader_window(
    app: tauri::AppHandle,
    label: String,
    title: String,
    file_data: FileData,  // 使用强类型结构体替代 serde_json::Value
) -> Result<(), String> {
    // 检查窗口是否已存在
    if app.get_webview_window(&label).is_some() {
        return Err(format!("窗口 {} 已存在", label));
    }

    // 构建URL查询参数
    let url = WebviewUrl::App(format!(
        "/reader?fileType={}&filePath={}&fileName={}",
        encode(&file_data.file_type),
        encode(&file_data.file_path),
        encode(&file_data.file_name)
    ).into());

    info!("正在打开文件: {} (路径: {})", file_data.file_name, file_data.file_path);

    // 创建窗口
    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(1200.0, 800.0)
        .min_inner_size(800.0, 600.0)
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    // 存储文件数据
    if let Some(storage) = app.try_state::<WindowDataStorage>() {
        storage.data.lock().unwrap().insert(label.clone(), file_data);
    }

    // 短暂延迟后显示窗口
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
    
    Ok(())
}