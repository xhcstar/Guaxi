use std::sync::Mutex;
use tauri::Manager;
use tauri::{WebviewWindowBuilder, WebviewUrl};
use urlencoding::encode;


// #[tauri::command]
// pub async fn create_reader_window(
//     app: tauri::AppHandle,
//     label: String,
//     title: String,
//     file_data: serde_json::Value // 接收前端传递的数据
// ) -> Result<(), String> {
//     use tauri::{WebviewWindowBuilder, WebviewUrl};
    
//     let window = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App("/reader".into()))
//         .title(title)
//         .inner_size(1200.0, 800.0)
//         .min_inner_size(800.0, 600.0)
//         .visible(false)
//         .build()
//         .map_err(|e| format!("创建窗口失败: {}", e))?;

//     // 存储窗口数据（供前端读取）
//     // app.state::<WindowDataStorage>().data.lock().unwrap().insert(label, file_data);
    
//     window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
//     Ok(())
// }

#[tauri::command]
pub async fn create_reader_window(
    app: tauri::AppHandle,
    label: String,
    title: String,
    file_data: serde_json::Value,
) -> Result<(), String> {
    // 检查窗口是否已存在
    if app.get_webview_window(&label).is_some() {
        return Err(format!("窗口 {} 已存在", label));
    }

    let url = WebviewUrl::App(format!(
        "/reader?fileType={}&fileName={}",
        encode(&file_data["type"].as_str().unwrap_or("unknown")),
        encode(&file_data["name"].as_str().unwrap_or("unknown"))
    ).into());

    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(title)
        .inner_size(1200.0, 800.0)
        .min_inner_size(800.0, 600.0)
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    // 短暂延迟确保窗口初始化完成
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
    
    Ok(())
}

// 数据存储结构（需提前注册）
struct WindowDataStorage {
    data: Mutex<std::collections::HashMap<String, serde_json::Value>>,
}