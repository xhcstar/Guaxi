use tracing::{info, debug, error};

#[tauri::command]
pub async fn read_epub_file(path: String) -> Result<Vec<u8>, String> {
    use std::path::Path;
    use std::fs;
    
    // 记录调试信息
    info!("[RUST] 正在读取epub文件: {}", path);
    
    // 安全检查
    let path_obj = Path::new(&path);
    if !path_obj.exists() {
        return Err(format!("文件不存在: {}", path));
    }
    
    if !path_obj.is_file() {
        return Err(format!("路径不是文件: {}", path));
    }
    
    // 检查文件扩展名
    if let Some(ext) = path_obj.extension() {
        if ext != "epub" {
            info!("[RUST] 警告: 文件扩展名不是.epub");
        }
    }
    
    // 读取文件内容
    match fs::read(path_obj) {
        Ok(content) => {
            info!("[RUST] 成功读取文件，大小: {} 字节", content.len());
            Ok(content)
        },
        Err(e) => {
            info!("[RUST] 读取文件错误: {}", e);
            Err(format!("读取文件失败: {}", e))
        }
    }
}