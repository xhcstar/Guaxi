use serde::Serialize;
use tracing::info;

#[derive(Debug, Serialize)]
pub struct MarkdownResponse {
    success: bool,
    content: String,
    error: Option<String>,
}

#[tauri::command]
pub async fn markdown_reader_load_markdown_file(filepath: String) -> MarkdownResponse {
    info!("load_markdown_content: {}", filepath);
    match std::fs::read_to_string(&filepath) {
        Ok(content) => {
            // 使用更精确的分页符匹配
            MarkdownResponse {
                success: true,
                content: content,
                error: None,
            }
        }
        Err(e) => MarkdownResponse {
            success: false,
            content: String::new(),
            error: Some(format!("Failed to read markdown file: {}", e)),
        },
    }
}