use xcap::Monitor;
use image::{DynamicImage, RgbaImage, ImageBuffer, imageops::crop};
use std::path::PathBuf;
use std::io::Cursor;
use tauri::{AppHandle, Emitter};
use tauri_plugin_dialog::DialogExt;
use enigo::{Enigo, Mouse, Settings, InputError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;


#[derive(serde::Serialize)]
pub struct CaptureResult {
    width: u32,
    height: u32,
    temp_path: String,
}

struct CaptureState {
    is_running: bool,
}

#[tauri::command]
pub async fn start_global_capture(app: AppHandle) -> Result<(), String> {
    let state = Arc::new(Mutex::new(CaptureState { is_running: true }));
    
    // 使用默认设置初始化 Enigo
    let settings = Settings::default();
    let mut enigo = Enigo::new(&settings).map_err(|e| e.to_string())?;
    
    // 启动鼠标位置监听线程
    let app_clone = app.clone();
    let state_clone = state.clone();
    thread::spawn(move || {
        while state_clone.lock().unwrap().is_running {
            match enigo.location() {
                Ok((x, y)) => {
                    let _ = app_clone.emit("global-mouse-move", (x, y));
                }
                Err(e) => eprintln!("Failed to get mouse location: {}", e),
            }
            thread::sleep(Duration::from_millis(16));
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_global_capture(app: AppHandle) -> Result<(), String> {
    let _ = app.emit("stop-global-capture", ());
    Ok(())
}

#[tauri::command]
pub async fn get_mouse_position() -> Result<(i32, i32), String> {
    let settings = Settings::default();
    let mut enigo = Enigo::new(&settings).map_err(|e| e.to_string())?;
    enigo.location().map_err(|e| e.to_string())
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
    // 获取主显示器
    let primary = Monitor::all()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .ok_or("Primary monitor not found")?;

    // 捕获整个屏幕
    let image = primary.capture_image().map_err(|e| e.to_string())?;
    
    // 裁剪选定区域
    let cropped = image::imageops::crop_imm(
        &image,
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