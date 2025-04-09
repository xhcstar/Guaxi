use xcap::Monitor;
use base64::{engine::general_purpose, Engine as _};
use std::fs;

#[tauri::command]
pub fn capture_screen() -> Result<String, String> {
    let monitors = Monitor::all().map_err(|e| e.to_string())?;
    let monitor = monitors.first().ok_or("未找到显示器")?;
    
    let image = monitor.capture_image().map_err(|e| e.to_string())?;
    let temp_path = std::env::temp_dir().join("guaxi_screenshot.png");
    image.save(&temp_path).map_err(|e| e.to_string())?;
    
    let image_data = fs::read(temp_path).map_err(|e| e.to_string())?;
    Ok(general_purpose::STANDARD.encode(&image_data))
}