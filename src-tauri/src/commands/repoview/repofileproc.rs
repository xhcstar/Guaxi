use super::super::settingview::get_libraries_root;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use crate::commands::repoview::delete_imported_file;

#[derive(serde::Serialize)]
pub struct FileInfo {
    file_name: String,
    file_type: String,
    create_time: String,
    file_size: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AddResultInfo {
    not_converted: Option<bool>,
}

#[tauri::command]
pub fn get_file_info(file_path: String) -> Result<FileInfo, String> {
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err("File does not exist".to_string());
    }

    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string();
    let file_type = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_string();

    // 使用 chrono 获取当前系统时间并格式化
    let create_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let file_size = metadata.len();

    Ok(FileInfo {
        file_name,
        file_type,
        create_time,
        file_size,
    })
}

#[tauri::command]
pub fn add_repo_import_file(library: String, file_path: String) -> Result<AddResultInfo, String> {
    info!(
        "add_repo_import_file: library: {} file_path: {}",
        library, file_path
    );
    // 获取配置的文件库路径
    let current_dir = get_libraries_root();
    info!("current_dir: {}", current_dir);

    // 构建 Libraries 目录路径
    let libraries_dir = PathBuf::from(current_dir).join("Libraries");
    if !libraries_dir.exists() {
        fs::create_dir(&libraries_dir).map_err(|e| e.to_string())?;
    }
    info!("Libraries directory: {}", libraries_dir.display());

    // 构建以所属库名字命名的子目录路径
    let library_dir = libraries_dir.join(&library);
    if !library_dir.exists() {
        fs::create_dir(&library_dir).map_err(|e| e.to_string())?;
        info!("create_dir Library directory: {}", library_dir.display());
    }

    info!("library_dir: {}", library_dir.display());
    // 获取文件名
    let file_name = Path::new(&file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Failed to get file name")?;
    info!("file_name: {}", file_name);

    // 去掉文件后缀，获取文件名称
    let file_stem = Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or("Failed to get file stem")?;
    info!("file_stem: {}", file_stem);

    // 获取文件后缀名(不包含“.”)
    let file_extension = Path::new(file_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| match ext {
            "doc" | "docx" => "word",
            ext => ext,
        })
        .unwrap_or(""); // 如果文件没有后缀名，则默认为空字符串
    info!("file_extension: {}", file_extension);

    match file_extension {
        "txt" | "pdf" | "doc" | "docx" | "epub" | "html" | "png" | "jpg" | "jpeg" | "pptx"
        | "ppt" => {}
        _ => {
            return Err("UnsupportedFileType".to_string());
        }
    }

    // 构建以文件名称命名的子目录路径
    let file_dir = library_dir.join(file_stem);
    if !file_dir.exists() {
        fs::create_dir(&file_dir).map_err(|e| e.to_string())?;
        info!("create_dir file directory: {}", file_dir.display());
    }

    // 在 file_dir 目录下根据文件后缀名创建文件夹
    let extension_dir = if !file_extension.is_empty() {
        let dir = file_dir.join(file_extension);
        if !dir.exists() {
            fs::create_dir(&dir).map_err(|e| e.to_string())?;
            info!("create_dir extension directory: {}", dir.display());
        }
        dir
    } else {
        file_dir.clone()
    };

    // 在 file_dir 目录下创建 txt, pdf, word, epub, html, images 文件夹
    let folders = ["txt", "pdf", "word", "epub", "html", "images"];
    for folder in folders.iter() {
        let folder_path = file_dir.join(folder);
        if !folder_path.exists() {
            fs::create_dir(&folder_path).map_err(|e| e.to_string())?;
            info!("create_dir {} directory: {}", folder, folder_path.display());
        }
    }

    if file_extension == "txt" {
        let text_convert_dir = file_dir.join("markdown");
        if !text_convert_dir.exists() {
            fs::create_dir(&text_convert_dir).map_err(|e| e.to_string())?;
            info!("create_dir directory: {}", text_convert_dir.display());
        }

        let text_convert_file_path = text_convert_dir.join(file_stem).with_extension("md");
        info!(
            ">>>>>>>> text_convert_file_path: {} , file_path: {} ",
            text_convert_file_path.display(),
            &file_path
        );
        fs::copy(&file_path, &text_convert_file_path).map_err(|e| e.to_string())?;
        info!(
            "Copy file from {} to {}",
            file_path,
            text_convert_file_path.display()
        );
    }

    // 在文件名称目录下创建 notes 目录
    let notes_dir = file_dir.join("notes");
    if !notes_dir.exists() {
        fs::create_dir(&notes_dir).map_err(|e| e.to_string())?;
        info!("create_dir notes directory: {}", notes_dir.display());
    }

    // 构建目标文件路径（将文件拷贝到以文件后缀名命名的目录中）
    let target_path = extension_dir.join(file_name);
    info!("target_path: {}", target_path.display());

    // 拷贝文件
    fs::copy(&file_path, &target_path).map_err(|e| e.to_string())?;
    info!("Copy file from {} to {}", file_path, target_path.display());

    // 获取当前时间并格式化为 YYYY-MM-DD HH:MM:SS
    let create_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    info!("create_time: {}", create_time);

    // 首次创建时，update_time 与 create_time 相同
    let update_time = create_time.clone();
    info!("update_time: {}", update_time);

    // 返回创建的 note 文件信息
    let import_note_info = AddResultInfo {
        not_converted: if file_extension == "txt" {
            Some(true)
        } else {
            Some(false)
        },
    };

    info!("import_note_info: {:?}", import_note_info);

    Ok(import_note_info)
}

#[tauri::command]
pub fn add_web_clipper_import_file(
    library: String,
    file_path: String,
) -> Result<AddResultInfo, String> {
    info!(
        "add_repo_import_file: library: {} file_path: {}",
        library, file_path
    );
    // 获取配置的文件库路径
    let current_dir = get_libraries_root();
    info!("current_dir: {}", current_dir);

    // 构建 Libraries 目录路径
    let libraries_dir = PathBuf::from(current_dir).join("Libraries");
    if !libraries_dir.exists() {
        fs::create_dir(&libraries_dir).map_err(|e| e.to_string())?;
    }
    info!("Libraries directory: {}", libraries_dir.display());

    // 构建以所属库名字命名的子目录路径
    let library_dir = libraries_dir.join(&library);
    if !library_dir.exists() {
        fs::create_dir(&library_dir).map_err(|e| e.to_string())?;
        info!("create_dir Library directory: {}", library_dir.display());
    }

    info!("library_dir: {}", library_dir.display());
    // 获取文件名
    let file_name = Path::new(&file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Failed to get file name")?;
    info!("file_name: {}", file_name);

    // 去掉文件后缀，获取文件名称
    let file_stem = Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or("Failed to get file stem")?;
    info!("file_stem: {}", file_stem);

    // 获取文件后缀名（不包含“.”）
    let file_extension = Path::new(file_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or(""); // 如果文件没有后缀名，则默认为空字符串
    info!("file_extension: {}", file_extension);

    // 构建以文件名称命名的子目录路径
    let file_dir = library_dir.join(file_stem);
    if !file_dir.exists() {
        fs::create_dir(&file_dir).map_err(|e| e.to_string())?;
        info!("create_dir file directory: {}", file_dir.display());
    }

    // 在 file_dir 目录下根据文件后缀名创建文件夹
    let extension_dir = if !file_extension.is_empty() {
        let dir = file_dir.join(file_extension);
        if !dir.exists() {
            fs::create_dir(&dir).map_err(|e| e.to_string())?;
            info!("create_dir extension directory: {}", dir.display());
        }
        dir
    } else {
        file_dir.clone()
    };

    // 在 file_dir 目录下创建 txt, pdf, word, epub, html, images 文件夹
    let folders = ["txt", "pdf", "word", "epub", "html", "images"];
    for folder in folders.iter() {
        let folder_path = file_dir.join(folder);
        if !folder_path.exists() {
            fs::create_dir(&folder_path).map_err(|e| e.to_string())?;
            info!("create_dir {} directory: {}", folder, folder_path.display());
        }
    }

    // 在文件名称目录下创建 notes 目录
    let notes_dir = file_dir.join("notes");
    if !notes_dir.exists() {
        fs::create_dir(&notes_dir).map_err(|e| e.to_string())?;
        info!("create_dir notes directory: {}", notes_dir.display());
    }

    // 构建目标文件路径（将文件拷贝到以文件后缀名命名的目录中）
    let target_path = extension_dir.join(file_name);
    info!("target_path: {}", target_path.display());

    // 拷贝文件
    fs::copy(&file_path, &target_path).map_err(|e| e.to_string())?;
    info!("Copy file from {} to {}", file_path, target_path.display());

    // 获取当前时间并格式化为 YYYY-MM-DD HH:MM:SS
    let create_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    info!("create_time: {}", create_time);

    // 首次创建时，update_time 与 create_time 相同
    let update_time = create_time.clone();
    info!("update_time: {}", update_time);

    fs::remove_file(&file_path).map_err(|e| e.to_string())?;

    // 返回创建的 note 文件信息
    let import_note_info = AddResultInfo {
        not_converted: if file_extension == "txt" {
            Some(true)
        } else {
            Some(false)
        },
    };

    Ok(import_note_info)
}

#[tauri::command]
pub async fn open_in_file_explorer(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path.replace("/", "\\")) // 确保Windows使用反斜杠
            .spawn()
            .map_err(|e| format!("Failed to open explorer: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open Finder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        // 尝试使用xdg-open（大多数Linux桌面环境支持）
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open file manager: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub fn delete_imported_all_file(id: i32, path: String) -> Result<(), String> {
    // 首先删除数据库记录
    delete_imported_file(id)?;

    // 然后删除文件系统中的目录及其所有内容
    let dir_path = Path::new(&path);
    if !dir_path.exists() {
        return Err(format!("Directory does not exist: {}", path));
    }

    // 递归删除目录及其所有内容
    fs::remove_dir_all(dir_path)
        .map_err(|e| format!("Failed to delete directory {}: {}", path, e))?;

    info!("Successfully deleted directory and all contents: {}", path);
    Ok(())
}