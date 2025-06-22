use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct FileNode {
    id: String,
    name: String,
    is_directory: bool,
    children: Option<Vec<FileNode>>,
}

// 辅助函数：规范化路径分隔符
fn normalize_path(path: &str) -> PathBuf {
    PathBuf::from(path.replace('\\', "/"))
}

#[tauri::command]
pub fn get_file_tree(path: String) -> Result<Vec<FileNode>, String> {
    let path = normalize_path(&path);
    let dir = Path::new(&path);

    // 使用 .display() 方法格式化路径
    info!("get_file_tree: {}", dir.display());
    info!("get_file_tree: {:?}", dir);

    if !dir.is_dir() {
        return Err("路径不是目录".to_string());
    }

    let mut nodes = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let is_directory = path.is_dir();

        let node = FileNode {
            id: path.to_string_lossy().to_string(),
            name: path.file_name().unwrap().to_string_lossy().to_string(),
            is_directory,
            children: if is_directory { Some(Vec::new()) } else { None },
        };

        nodes.push(node);
    }

    Ok(nodes)
}

#[tauri::command]
pub fn read_file_content(path: String) -> Result<String, String> {
    let path = normalize_path(&path);
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_file_content(path: String, content: String) -> Result<(), String> {
    fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_directory(path: String) -> Result<(), String> {
    fs::create_dir_all(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_file(path: String) -> Result<(), String> {
    fs::File::create(&path)
        .map_err(|e| e.to_string())
        .map(|_| ())
}

#[tauri::command]
pub fn delete_path(path: String) -> Result<(), String> {
    if fs::metadata(&path).map_err(|e| e.to_string())?.is_dir() {
        fs::remove_dir_all(&path).map_err(|e| e.to_string())
    } else {
        fs::remove_file(&path).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn move_path(source_path: String, destination_path: String) -> Result<(), String> {
    info!("move_path: {} -> {}", source_path, destination_path);
    std::fs::rename(source_path, destination_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_file_with_content(
    path: String,
    content: Option<String>, // 改为接受可选参数
) -> Result<(), String> {
    let content = content.unwrap_or_default(); // 处理null情况

    let path = PathBuf::from(path.replace('\\', "/")); // 统一路径格式

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
        }
    }

    fs::write(&path, content).map_err(|e| format!("写入文件失败: {}", e))?;

    Ok(())
}

// 辅助函数：检查是否是有效目录
fn is_valid_directory(path: &Path) -> bool {
    path.exists() && path.is_dir()
}

#[tauri::command]
pub fn rename_directory(old_path: String, new_path: String) -> Result<(), String> {
    let old_path = normalize_path(&old_path);
    let new_path = normalize_path(&new_path);

    info!(
        "Attempting to rename directory: {:?} -> {:?}",
        old_path, new_path
    );

    // 1. 验证源目录
    if !is_valid_directory(&old_path) {
        warn!(
            "Source directory does not exist or is not a directory: {:?}",
            old_path
        );
        return Err("源目录不存在或不是目录".to_string());
    }

    // 2. 检查目标路径是否已存在
    if new_path.exists() {
        warn!("Target path already exists: {:?}", new_path);
        return Err("目标路径已存在".to_string());
    }

    // 3. 确保父目录存在
    if let Some(parent) = new_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("创建父目录失败: {}", e))?;
        }
    }

    // 4. 执行重命名
    fs::rename(&old_path, &new_path).map_err(|e| format!("重命名目录失败: {}", e))?;

    info!(
        "Directory renamed successfully: {:?} -> {:?}",
        old_path, new_path
    );
    Ok(())
}

#[tauri::command]
pub fn rename_path(old_path: String, new_path: String) -> Result<(), String> {
    let old_path = normalize_path(&old_path);
    let new_path = normalize_path(&new_path);

    info!(
        "Attempting to rename path: {:?} -> {:?}",
        old_path, new_path
    );

    // 1. 验证源路径是否存在
    if !old_path.exists() {
        warn!("Source path does not exist: {:?}", old_path);
        return Err("源路径不存在".to_string());
    }

    // 2. 检查目标路径是否已存在
    if new_path.exists() {
        warn!("Target path already exists: {:?}", new_path);
        return Err("目标路径已存在".to_string());
    }

    // 3. 确保父目录存在
    if let Some(parent) = new_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("创建父目录失败: {}", e))?;
        }
    }

    // 4. 执行重命名
    fs::rename(&old_path, &new_path).map_err(|e| format!("重命名失败: {}", e))?;

    info!(
        "Path renamed successfully: {:?} -> {:?}",
        old_path, new_path
    );
    Ok(())
}
