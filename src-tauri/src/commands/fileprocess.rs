use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tracing::{info, warn};

use base64::engine::general_purpose;
use base64::Engine as _;

use crate::commands::kgraph;

#[derive(Debug, Serialize, Deserialize)]
pub struct FileNode {
    id: String,
    name: String,
    is_directory: bool,
    children: Option<Vec<FileNode>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileMeta {
    path: String,
    mtime_ms: Option<u64>,
    file_size: u64,
    is_directory: bool,
}

// 辅助函数：规范化路径分隔符
fn normalize_path(path: &str) -> PathBuf {
    use std::path::Component;

    let mut s = path.trim().replace('\\', "/");

    if s.starts_with('/') && s.len() >= 4 {
        let bytes = s.as_bytes();
        if bytes[2] == b':' && bytes[3] == b'/' {
            s = s[1..].to_string();
        }
    }

    let mut out = PathBuf::new();
    let mut anchored = false;
    let mut depth = 0usize;

    for comp in Path::new(&s).components() {
        match comp {
            Component::Prefix(p) => {
                out.push(p.as_os_str());
                anchored = true;
                depth = 0;
            }
            Component::RootDir => {
                out.push(Path::new("/"));
                anchored = true;
                depth = 0;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if depth > 0 {
                    out.pop();
                    depth -= 1;
                } else if !anchored {
                    out.push("..");
                }
            }
            Component::Normal(part) => {
                out.push(part);
                depth += 1;
            }
        }
    }

    out
}

#[tauri::command]
pub fn get_file_tree(path: String) -> Result<Vec<FileNode>, String> {
    let raw_path = path.clone();
    let path = normalize_path(&path);
    let dir = Path::new(&path);

    info!("get_file_tree input: {}", raw_path);
    info!("get_file_tree normalized: {}", dir.display());
    info!("get_file_tree normalized(debug): {:?}", dir);

    if !dir.is_dir() {
        warn!("get_file_tree: not a directory: {}", dir.display());
        return Err("路径不是目录".to_string());
    }

    let mut nodes = Vec::new();
    for entry in fs::read_dir(dir)
        .map_err(|e| format!("read_dir failed: {} ({})", e, dir.display()))?
    {
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
pub fn get_file_meta(path: String) -> Result<FileMeta, String> {
    let normalized = normalize_path(&path);
    let metadata = fs::metadata(&normalized).map_err(|e| e.to_string())?;
    let mtime_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64);

    let meta = FileMeta {
        path: normalized.to_string_lossy().to_string(),
        mtime_ms,
        file_size: metadata.len(),
        is_directory: metadata.is_dir(),
    };

    info!(
        path = %meta.path,
        mtime_ms = ?meta.mtime_ms,
        file_size = meta.file_size,
        is_directory = meta.is_directory,
        "[fileprocess] get_file_meta"
    );

    Ok(meta)
}

#[tauri::command]
pub fn read_file_bytes_base64(path: String) -> Result<String, String> {
    let path = normalize_path(&path);
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    Ok(general_purpose::STANDARD.encode(bytes))
}

#[tauri::command]
pub fn write_file_bytes_base64(path: String, base64: String) -> Result<(), String> {
    let path = normalize_path(&path);
    let bytes = general_purpose::STANDARD
        .decode(base64.as_bytes())
        .map_err(|e| e.to_string())?;
    // 设计说明：
    // 1. 前端会把二进制内容直接写到资源库中的分层日期目录，这些父目录不一定已经存在。
    // 2. 这里统一在后端兜底创建父目录，避免不同前端调用方各自重复处理并遗漏。
    // 3. 同时补充日志，便于排查资源写回、导出文件等链路中的路径问题。
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            info!(
                parent = %parent.display(),
                "[fileprocess] ensured parent directory for binary write"
            );
        }
    }
    fs::write(&path, bytes).map_err(|e| e.to_string())?;
    info!(
        path = %path.display(),
        "[fileprocess] wrote binary file from base64 payload"
    );
    Ok(())
}

#[tauri::command]
pub fn write_file_content(path: String, content: String) -> Result<(), String> {
    let path = normalize_path(&path);
    fs::write(&path, &content).map_err(|e| e.to_string())?;

    if let Err(e) = kgraph::after_write_file(&path, &content) {
        warn!(error = %e, path = %path.display(), "kbmeta after_write_file failed");
    }

    Ok(())
}

#[tauri::command]
pub fn get_executable_dir() -> Result<String, String> {
    let exe_path = env::current_exe().map_err(|e| format!("获取可执行文件路径失败: {}", e))?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "无法获取可执行文件所在目录".to_string())?
        .to_path_buf();

    info!(
        exe_path = %exe_path.display(),
        exe_dir = %exe_dir.display(),
        "get_executable_dir"
    );

    Ok(exe_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub fn create_directory(path: String) -> Result<(), String> {
    let path = normalize_path(&path);
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;

    if let Err(e) = kgraph::after_create_path(&path, true, None) {
        warn!(error = %e, path = %path.display(), "kbmeta after_create_path failed");
    }

    Ok(())
}

#[tauri::command]
pub fn create_file(path: String) -> Result<(), String> {
    let path = normalize_path(&path);

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }

    fs::File::create(&path)
        .map_err(|e| e.to_string())
        .map(|_| ())?;

    if let Err(e) = kgraph::after_create_path(&path, false, None) {
        warn!(error = %e, path = %path.display(), "kbmeta after_create_path failed");
    }

    Ok(())
}

#[tauri::command]
pub fn delete_path(path: String) -> Result<(), String> {
    let path = normalize_path(&path);
    let is_dir = fs::metadata(&path).map_err(|e| e.to_string())?.is_dir();

    if is_dir {
        fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
    } else {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }

    if let Err(e) = kgraph::after_delete_path(&path, is_dir) {
        warn!(error = %e, path = %path.display(), "kbmeta after_delete_path failed");
    }

    Ok(())
}

#[tauri::command]
pub fn move_path(source_path: String, destination_path: String) -> Result<(), String> {
    let source_path = normalize_path(&source_path);
    let destination_path = normalize_path(&destination_path);

    info!("move_path: {} -> {}", source_path.display(), destination_path.display());
    std::fs::rename(&source_path, &destination_path).map_err(|e| e.to_string())?;

    if let Err(e) = kgraph::after_move_path(&source_path, &destination_path) {
        warn!(error = %e, src = %source_path.display(), dst = %destination_path.display(), "kbmeta after_move_path failed");
    }

    Ok(())
}

#[tauri::command]
pub async fn create_file_with_content(
    path: String,
    content: Option<String>, // 改为接受可选参数
) -> Result<(), String> {
    let content = content.unwrap_or_default(); // 处理null情况

    let path = normalize_path(&path.to_string());

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
        }
    }

    fs::write(&path, &content).map_err(|e| format!("写入文件失败: {}", e))?;

    if let Err(e) = kgraph::after_create_path(&path, false, Some(&content)) {
        warn!(error = %e, path = %path.display(), "kbmeta after_create_path failed");
    }

    Ok(())
}

// 辅助函数：检查是否是有效目录
fn is_valid_directory(path: &Path) -> bool {
    path.exists() && path.is_dir()
}

#[tauri::command]
pub fn copy_path(source_path: String, destination_path: String) -> Result<(), String> {
    let source_path = normalize_path(&source_path);
    let destination_path = normalize_path(&destination_path);
    if let Some(parent) = destination_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
        }
    }
    let meta = fs::metadata(&source_path).map_err(|e| e.to_string())?;
    if meta.is_dir() {
        copy_dir_recursive(&source_path, &destination_path)?;
    } else {
        fs::copy(&source_path, &destination_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
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

    if let Err(e) = kgraph::after_move_path(&old_path, &new_path) {
        warn!(error = %e, src = %old_path.display(), dst = %new_path.display(), "kbmeta after_move_path failed");
    }

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

    if let Err(e) = kgraph::after_move_path(&old_path, &new_path) {
        warn!(error = %e, src = %old_path.display(), dst = %new_path.display(), "kbmeta after_move_path failed");
    }

    info!(
        "Path renamed successfully: {:?} -> {:?}",
        old_path, new_path
    );
    Ok(())
}
