use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use tauri::command;
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
pub struct FileNode {
    id: String,
    name: String,
    is_directory: bool,
    children: Option<Vec<FileNode>>,
}

#[command]
pub fn get_file_tree(path: String) -> Result<Vec<FileNode>, String> {
    let dir = Path::new(&path);
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
            children: if is_directory {
                Some(Vec::new())
            } else {
                None
            },
        };

        nodes.push(node);
        info!("{:?}", nodes);
    }

    Ok(nodes)
}

#[command]
pub fn read_file_content(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[command]
pub fn write_file_content(path: String, content: String) -> Result<(), String> {
    fs::write(&path, content).map_err(|e| e.to_string())
}
