use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::Manager;
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tracing::info;
use urlencoding::encode;
// use base64::{Engine as _, engine::general_purpose};

use std::fs::{self, OpenOptions};
use std::io::Write;
use chrono::Local;

#[derive(Serialize)]
pub struct NoteResponse {
    file_path: String,
    content: String,
}

// 定义文件数据结构
#[derive(Debug, Serialize, Deserialize)]
pub struct FileData {
    file_type: String,
    file_name: String,
    file_path: String,
    library: String,
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
    file_data: FileData, // 使用强类型结构体替代 serde_json::Value
) -> Result<(), String> {
    // 检查窗口是否已存在
    if app.get_webview_window(&label).is_some() {
        return Err(format!("窗口 {} 已存在", label));
    }

    // 使用 Base64 编码文件路径
    // let encoded_path = general_purpose::URL_SAFE.encode(&file_data.file_path);

    // 构建URL查询参数
    let url = WebviewUrl::App(
        format!(
            "/reader?filePath={}&fileType={}&fileName={}&library={}",
            encode(&file_data.file_path),
            encode(&file_data.file_type),
            encode(&file_data.file_name),
            encode(&file_data.library),
        )
        .into(),
    );

    info!("url: {}", url);
    info!(
        "正在打开文件: {} (路径: {})",
        file_data.file_name, file_data.file_path
    );

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
        storage
            .data
            .lock()
            .unwrap()
            .insert(label.clone(), file_data);
    }

    // 短暂延迟后显示窗口
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn save_notes_to_file(content: String, notes_dir: String, file_name: String) -> Result<String, String> {
    // 确保笔记目录存在
    fs::create_dir_all(&notes_dir).map_err(|e| e.to_string())?;
    
    // 检查目录中是否已有笔记文件
    let entries = fs::read_dir(&notes_dir).map_err(|e| e.to_string())?;
    let mut note_files: Vec<String> = Vec::new();
    
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "md") {
            note_files.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    
    // 确定要写入的文件路径
    let file_path = if note_files.len() == 1 {
        // 如果只有一个笔记文件，使用它
        format!("{}/{}", notes_dir, note_files[0])
    } else {
        // 否则创建新的笔记文件
        let now = Local::now().format("%Y_%m_%d_%H_%M_%S").to_string();
        format!("{}/{}-note-{}.md", notes_dir, file_name, now)
    };
    
    // 写入文件（追加模式）
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)
        .map_err(|e| e.to_string())?;
        
    writeln!(&mut file, "{}", content).map_err(|e| e.to_string())?;
    
    Ok(format!("笔记已保存到: {}", file_path))
}

#[tauri::command]
pub fn load_existing_notes(notes_dir: String, file_name: String) -> Result<NoteResponse, String> {
    use std::fs;
    
    // 确保笔记目录存在
    fs::create_dir_all(&notes_dir).map_err(|e| e.to_string())?;
    
    // 检查目录中的笔记文件
    let entries = fs::read_dir(&notes_dir).map_err(|e| e.to_string())?;
    let mut note_files: Vec<String> = Vec::new();
    
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "md") {
            note_files.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    
    // 如果有且只有一个笔记文件，加载它
    if note_files.len() == 1 {
        let file_path = format!("{}/{}", notes_dir, note_files[0]);
        let content = fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
        return Ok(NoteResponse {
            file_path,
            content,
        });
    }
    
    Ok(NoteResponse {
        file_path: String::new(),
        content: String::new(),
    })
}