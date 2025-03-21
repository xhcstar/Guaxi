use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use std::path::Path;
use tracing::info;

#[derive(serde::Serialize)]
pub struct FileInfo {
    file_name: String,
    file_type: String,
    create_time: String,
    file_size: String,
}

#[tauri::command]
pub fn get_file_info(file_path: String) -> Result<FileInfo, String> {
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err("File does not exist".to_string());
    }

    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("").to_string();
    let file_type = path.extension().and_then(|ext| ext.to_str()).unwrap_or("").to_string();
    let create_time = metadata.created().map_err(|e| e.to_string())?.duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_secs();
    let file_size = metadata.len();

    Ok(FileInfo {
        file_name,
        file_type,
        create_time: format!("{}", create_time),
        file_size: format!("{} KB", file_size / 1024),
    })
}

#[tauri::command]
pub fn add_repo_import_file(library: String, file_path: String) -> Result<String, String> {
    println!("add_repo_import_file: library: {} file_path: {}", library, file_path);
    // 获取当前程序所在路径
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    println!("Current directory: {}", current_dir.display());

    // 构建 Libraries 目录路径
    let libraries_dir = current_dir.join("Libraries");
    if !libraries_dir.exists() {
        fs::create_dir(&libraries_dir).map_err(|e| e.to_string())?;
    }
    println!("Libraries directory: {}", libraries_dir.display());

    // 构建以所属库名字命名的子目录路径
    let library_dir = libraries_dir.join(&library);
    if !library_dir.exists() {
        fs::create_dir(&library_dir).map_err(|e| e.to_string())?;
        println!("create_dir Library directory: {}", library_dir.display());
    }

    // 获取文件名
    let file_name = Path::new(&file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Failed to get file name")?;
    println!("file_name: {}", file_name);

    // 构建目标文件路径
    let target_path = library_dir.join(file_name);
    println!("target_path: {}", target_path.display());

    // 拷贝文件
    fs::copy(&file_path, &target_path).map_err(|e| e.to_string())?;
    println!("Copy file from {} to {}", file_path, target_path.display());

    // 返回目标文件路径
    Ok(target_path.to_string_lossy().to_string())
}
