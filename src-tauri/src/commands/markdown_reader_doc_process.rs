use serde::Serialize;
use tracing::info;

use crate::commands::notebook::strip_math_cell_blocks;
use crate::commands::notebook::strip_python_cell_blocks;

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
            // 设计说明：
            // 1. Reader 通过客户端 renderMarkdown() 渲染 Markdown，不会经过 preprocess_markdown_for_static_render。
            // 2. 必须在返回原始内容前剥离 <!--py--> 和 <!--math--> cell 源码块，
            //    否则 HTML 注释标记内部的纯文本会直接暴露在预览中。
            // 3. 注意：不剥离 output blocks（<!-- guaxi:pyout:... -->），
            //    因为预览模式下 Python/Math Cell 的执行输出（表格、图表等）应当可见。
            let without_python = strip_python_cell_blocks(&content);
            let cleaned = strip_math_cell_blocks(&without_python);
            MarkdownResponse {
                success: true,
                content: cleaned,
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