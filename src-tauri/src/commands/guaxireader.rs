use chrono::Local;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::Mutex;
use tauri::Manager;
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tracing::info;
use urlencoding::encode;
use rusqlite::{params, OptionalExtension};
use crate::commands::repoview::{init_db, add_note, update_note, NoteInput};

#[derive(Debug, Serialize, Deserialize)]
pub struct SvgSaveResult {
    pub success: bool,
    pub relative_path: String,
    pub content_hash: String,
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    import_file_id: Option<u64>,
}

// 窗口数据存储
pub struct WindowDataStorage {
    data: Mutex<std::collections::HashMap<String, FileData>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NoteItem {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: String, // "text", "image", "code", "math", "diagram"
    pub content: String,
    pub original_markdown: String,
    pub timestamp: String, // ISO 8601格式字符串
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NoteCard {
    pub id: String,
    #[serde(rename = "type")]
    pub card_type: String, // "single" or "composite"
    pub title: String,
    pub items: Vec<NoteItem>,
    pub timestamp: String, // ISO 8601格式字符串
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotesData {
    pub cards: Vec<NoteCard>,
}

#[tauri::command]
pub async fn create_reader_window(
    app: tauri::AppHandle,
    label: String,
    title: String,
    file_data: FileData, // 使用强类型结构体替代 serde_json::Value
    theme: Option<String>,
) -> Result<(), String> {
    // 检查窗口是否已存在
    if app.get_webview_window(&label).is_some() {
        return Err(format!("窗口 {} 已存在", label));
    }

    let mut url_qs = format!(
        "filePath={}&fileType={}&fileName={}&library={}&importFileId={}",
        encode(&file_data.file_path),
        encode(&file_data.file_type),
        encode(&file_data.file_name),
        encode(&file_data.library),
        encode(&file_data.import_file_id.unwrap_or(0).to_string()),
    );

    let theme = theme.unwrap_or_default().trim().to_lowercase();
    if theme == "dark" || theme == "light" {
        url_qs.push_str("&theme=");
        url_qs.push_str(encode(&theme).as_ref());
    }

    let url = WebviewUrl::App(format!("/reader?{}", url_qs).into());

    info!("file_data: {:?}", file_data);
    info!("url: {}", url);
    info!(
        "正在打开文件: {} (路径: {})",
        file_data.file_name, file_data.file_path
    );

    // 创建窗口
    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(1200.0, 800.0)
        .min_inner_size(1100.0, 650.0)
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
pub fn save_notes_to_file(
    content: String,
    notes_dir: String,
    file_name: String,
) -> Result<String, String> {
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
pub fn save_notes_to_file_no_append(
    content: String,
    notes_dir: String,
    file_name: String,
) -> Result<String, String> {
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
        .open(&file_path)
        .map_err(|e| e.to_string())?;

    writeln!(&mut file, "{}", content).map_err(|e| e.to_string())?;

    Ok(format!("笔记已保存到: {}", file_path))
}

#[tauri::command]
pub fn load_existing_notes(notes_dir: String, _file_name: String) -> Result<NoteResponse, String> {
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
        return Ok(NoteResponse { file_path, content });
    }

    Ok(NoteResponse {
        file_path: String::new(),
        content: String::new(),
    })
}

#[tauri::command]
pub async fn save_svg_file(
    content: String,
    file_name: String,
    image_dir: String,
) -> Result<SvgSaveResult, String> {
    // 创建目标目录
    let mut images_path =
        std::env::current_dir().map_err(|e| format!("获取当前目录失败: {}", e))?;
    images_path.push(image_dir);

    std::fs::create_dir_all(&images_path).map_err(|e| format!("创建目录失败: {}", e))?;

    // 计算SHA256哈希（即使前端用了简单哈希，后端仍保持强哈希）
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let content_hash = format!("{:x}", hasher.finalize());

    // 构建完整路径
    let mut file_path = images_path.clone();
    file_path.push(&file_name);
    info!("文件路径: {}", file_path.display());

    // 保存文件（如果不存在）
    if !file_path.exists() {
        std::fs::write(&file_path, content).map_err(|e| format!("写入文件失败: {}", e))?;
    }

    info!("保存成功，文件路径: {} {}", file_name, content_hash);

    Ok(SvgSaveResult {
        success: true,
        relative_path: format!("images/{}", file_name), // 注意路径格式
        content_hash,
    })
}

#[tauri::command]
pub async fn load_notes_file(file_path: String) -> Result<NotesData, String> {
    // 如果文件不存在，返回空数据结构
    if !std::path::Path::new(&file_path).exists() {
        return Ok(NotesData { cards: Vec::new() });
    }

    // 读取文件内容
    let content = tokio::fs::read_to_string(&file_path)
        .await
        .map_err(|e| e.to_string())?;

    // 解析JSON数据
    let notes: NotesData =
        serde_json::from_str(&content).map_err(|e| format!("解析笔记文件失败: {}", e))?;

    Ok(notes)
}

#[tauri::command]
pub async fn update_notes_file(
    action: String,
    data: serde_json::Value,
    file_path: String,
    import_file_id: i32,
    import_file_name: String,
) -> Result<(), String> {
    // 读取现有数据
    let mut notes = load_notes_file(file_path.clone())
        .await
        .unwrap_or_else(|_| NotesData { cards: Vec::new() });

    // 解析传入的数据
    let card: NoteCard =
        serde_json::from_value(data).map_err(|e| format!("解析笔记数据失败: {}", e))?;

    // 处理操作
    match action.as_str() {
        "add" => {
            notes.cards.push(card);
        }
        "update" => {
            if let Some(pos) = notes.cards.iter().position(|c| c.id == card.id) {
                notes.cards[pos] = card;
            } else {
                return Err("未找到要更新的笔记".into());
            }
        }
        "delete" => {
            notes.cards.retain(|c| c.id != card.id);
        }
        _ => return Err("无效的操作类型".into()),
    }

    // 异步写入文件
    let json = serde_json::to_string_pretty(&notes).map_err(|e| e.to_string())?;
    tokio::fs::write(&file_path, json).await.map_err(|e| e.to_string())?;

    let conn = init_db().map_err(|e| e.to_string())?;
    let imported_id_val: i32 = if import_file_id != 0 {
        import_file_id
    } else {
        conn
            .query_row(
                "SELECT id FROM imported_files WHERE original_filename = ?",
                params![import_file_name],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .unwrap_or(0)
    };

    let existing_note_id: Option<i32> = if imported_id_val != 0 {
        conn
            .query_row("SELECT id FROM notes WHERE imported_file_id = ?", params![imported_id_val], |row| row.get(0))
            .optional()
            .map_err(|e| e.to_string())?
    } else {
        conn
            .query_row(
                "SELECT id FROM notes WHERE from_imported_file = ?",
                params![import_file_name],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
    };

    if let Some(note_id) = existing_note_id {
        let (note_file_create_time, note_file_update_time) = {
            let md = fs::metadata(&file_path).map_err(|e| e.to_string())?;
            let created = md.created().ok();
            let modified = md.modified().ok();
            let cstr = created
                .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
            let ustr = modified
                .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| cstr.clone());
            (cstr, ustr)
        };

        let ni = NoteInput::new(
            format!("{}的摘录笔记", import_file_name),
            file_path.clone(),
            Vec::new(),
            "json".to_string(),
            import_file_name.clone(),
            "".to_string(),
            note_file_create_time,
            note_file_update_time,
            "".to_string(),
            imported_id_val,
        );
        update_note(note_id, ni)?;
    } else {
        let (note_file_create_time, note_file_update_time) = {
            let md = fs::metadata(&file_path).map_err(|e| e.to_string())?;
            let created = md.created().ok();
            let modified = md.modified().ok();
            let cstr = created
                .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
            let ustr = modified
                .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| cstr.clone());
            (cstr, ustr)
        };

        let ni = NoteInput::new(
            format!("{}的摘录笔记", import_file_name),
            file_path.clone(),
            Vec::new(),
            "json".to_string(),
            import_file_name.clone(),
            "".to_string(),
            note_file_create_time,
            note_file_update_time,
            "".to_string(),
            imported_id_val,
        );
        let _ = add_note(ni)?;
    }

    Ok(())
}
