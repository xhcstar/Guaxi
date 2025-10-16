use tracing::{info, debug, error};
use std::path::Path;
use std::fs;

#[tauri::command]
pub async fn read_text_file(path: String) -> Result<String, String> {
    // 记录调试信息
    info!("[RUST] 正在读取文本文件: {}", path);
    
    // 安全检查
    let path_obj = Path::new(&path);
    if !path_obj.exists() {
        return Err(format!("文件不存在: {}", path));
    }
    
    if !path_obj.is_file() {
        return Err(format!("路径不是文件: {}", path));
    }
    
    // 检查文件扩展名（可选，根据需求保留）
    if let Some(ext) = path_obj.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        if !["txt", "md", "log", "csv", "json", "xml", "html", "htm"].contains(&ext_str.as_str()) {
            info!("[RUST] 警告: 文件扩展名不是常见的文本文件类型: {}", ext_str);
        }
    }
    
    // 读取文件内容为字符串
    match fs::read_to_string(path_obj) {
        Ok(content) => {
            info!("[RUST] 成功读取文本文件，字符数: {}", content.len());
            Ok(content)
        },
        Err(e) => {
            error!("[RUST] 读取文本文件错误: {}", e);
            
            // 尝试用不同编码读取（如果需要处理非UTF-8文件）
            if e.kind() == std::io::ErrorKind::InvalidData {
                info!("[RUST] 尝试使用GBK编码读取文件");
                match read_with_encoding(path_obj, "gbk") {
                    Ok(content) => Ok(content),
                    Err(e) => Err(format!("读取文件失败(尝试GBK编码后): {}", e))
                }
            } else {
                Err(format!("读取文件失败: {}", e))
            }
        }
    }
}

// 辅助函数：使用指定编码读取文件
fn read_with_encoding(path: &Path, encoding: &str) -> Result<String, std::io::Error> {
    use encoding_rs::*;
    
    // 先读取为字节
    let bytes = fs::read(path)?;
    
    // 根据编码解码
    let (cow, _, had_errors) = match encoding.to_lowercase().as_str() {
        "gbk" => GBK.decode(&bytes),
        "gb18030" => GB18030.decode(&bytes),
        "utf8" | "utf-8" => UTF_8.decode(&bytes),
        _ => UTF_8.decode(&bytes) // 默认使用UTF-8
    };
    
    if had_errors {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("解码{}编码时出现错误", encoding)
        ))
    } else {
        Ok(cow.into_owned())
    }
}