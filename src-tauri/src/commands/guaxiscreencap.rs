use xcap::Monitor;
use image::{DynamicImage, ImageBuffer, imageops::crop};
use std::path::PathBuf;
use std::io::Cursor;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;


#[derive(serde::Serialize)]
pub struct CaptureResult {
    width: u32,
    height: u32,
    temp_path: String,
}

#[tauri::command]
pub async fn prepare_capture() -> Result<CaptureResult, String> {
    let monitors = Monitor::all().map_err(|e| e.to_string())?;
    let primary = monitors.iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .ok_or("Primary monitor not found")?;

    let temp_path = PathBuf::from(std::env::temp_dir())
        .join("screenshot_temp.png")
        .to_string_lossy()
        .into_owned();

    Ok(CaptureResult {
        width: primary.width().unwrap_or(0),
        height: primary.height().unwrap_or(0),
        temp_path,
    })
}

#[tauri::command]
pub async fn capture_region(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    let monitors = Monitor::all().map_err(|e| e.to_string())?;
    let target = monitors.iter().find(|m| {
        let mx = m.x().unwrap_or(0);
        let my = m.y().unwrap_or(0);
        let mw = m.width().unwrap_or(0);
        let mh = m.height().unwrap_or(0);
        
        mx <= x as i32 && 
        my <= y as i32 &&
        mx + mw as i32 >= (x + width) as i32 &&
        my + mh as i32 >= (y + height) as i32
    }).ok_or("Selected region out of bounds")?;

    let image = target.capture_image().map_err(|e| e.to_string())?;
    let dynamic_img = ImageBuffer::from_raw(
        image.width(),
        image.height(),
        image.to_vec(),
    ).ok_or("Failed to create image buffer")?;

    let mut img_copy = dynamic_img.clone();
    let cropped = crop(
        &mut img_copy,
        x,
        y,
        width,
        height
    ).to_image();

    let mut buf = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(cropped)
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    
    Ok(buf.into_inner())
}


#[tauri::command]
pub async fn show_save_dialog(
    app: AppHandle,
    default_path: String,
    filters: Vec<(String, Vec<String>)>,
) -> Result<Option<String>, String> {
    // 构建对话框
    let dialog = app.dialog()
        .file()
        .set_file_name(&default_path);

    // 添加过滤器（需要转换类型）
    let dialog = if !filters.is_empty() {
        // 将Vec<String>转换为Vec<&str>
        let extensions: Vec<&str> = filters[0].1.iter().map(|s| s.as_str()).collect();
        dialog.add_filter(&filters[0].0, &extensions)
    } else {
        dialog
    };

    // 同步方式保存文件
    let file_path = dialog.blocking_save_file();

    // 转换路径格式并返回（使用to_string()替代to_string_lossy()）
    Ok(file_path.map(|p| p.to_string()))
}