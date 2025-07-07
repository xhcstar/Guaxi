use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct ExcerptNote {
    pub file_path: String, // 完整文件路径
    pub file_name: String, // 文件名（不含路径）
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExcerptItem {
    id: String,
    r#type: String,
    content: String,
    timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExcerptCard {
    id: String,
    r#type: String,
    title: String,
    items: Vec<ExcerptItem>,
    timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExcerptFile {
    cards: Vec<ExcerptCard>,
}

#[tauri::command]
pub async fn search_excerpt_notes(pattern: &str, path: &str) -> Result<Vec<ExcerptNote>, String> {
    let libraries_path = Path::new(path).join("Libraries");
    info!(
        "Searching for excerpt notes in: {}",
        libraries_path.display()
    );

    let notes_dirs = match find_notes_dirs(&libraries_path) {
        Ok(dirs) => dirs,
        Err(e) => return Err(format!("Failed to find notes directories: {}", e)),
    };

    info!("Found {} notes directories to search", notes_dirs.len());

    let results: Vec<ExcerptNote> = notes_dirs
        .par_iter()
        .filter_map(|notes_dir| match search_notes_in_dir(notes_dir, pattern) {
            Ok(notes) => Some(notes),
            Err(e) => {
                warn!("Error searching in {}: {}", notes_dir.display(), e);
                None
            }
        })
        .flatten()
        .collect();

    info!(
        "Found {} excerpt notes matching '{}'",
        results.len(),
        pattern
    );
    Ok(results)
}

fn find_notes_dirs(libraries_path: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut notes_dirs = Vec::new();

    for entry in std::fs::read_dir(libraries_path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            for sub_entry in std::fs::read_dir(entry.path())? {
                let sub_entry = sub_entry?;
                if sub_entry.file_type()?.is_dir() {
                    let notes_path = sub_entry.path().join("notes");
                    if notes_path.exists() {
                        notes_dirs.push(notes_path);
                    }
                }
            }
        }
    }

    Ok(notes_dirs)
}

fn search_notes_in_dir(notes_dir: &Path, pattern: &str) -> std::io::Result<Vec<ExcerptNote>> {
    let mut results = Vec::new();

    for entry in std::fs::read_dir(notes_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map_or(false, |ext| ext == "json") {
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // 只匹配文件名（不再检查文件内容）
            if pattern.is_empty() || file_name.contains(pattern) {
                results.push(ExcerptNote {
                    file_path: path.to_string_lossy().into_owned(),
                    file_name,
                });
            }
        }
    }

    Ok(results)
}

#[tauri::command]
pub async fn read_excerpt_file(path: String) -> Result<ExcerptFile, String> {
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse JSON: {}", e))
}