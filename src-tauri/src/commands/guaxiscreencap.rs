use tauri::Window;
use crate::screenshot::{capture_full_screen, CaptureConfig};

#[derive(serde::Serialize)]
pub struct CaptureResult {
    image_data: Vec<u8>,
    text: Option<String>,
}

#[tauri::command]
pub async fn take_screenshot(window: Window) -> Result<CaptureResult, String> {
    let config = CaptureConfig::default();
    let image_data = capture_full_screen(&config)
        .await
        .map_err(|e| e.to_string())?;

    // 克隆一份数据用于UI线程
    let ui_image_data = image_data.clone();
    
    window.run_on_main_thread(move || {
        if let Err(e) = crate::screenshot::ui::ScreenshotApp::show_with_image(&ui_image_data) {
            eprintln!("Failed to open screenshot UI: {}", e);
        }
    })
    .map_err(|e| e.to_string())?; // 将tauri::Error转换为String

    Ok(CaptureResult { image_data, text: None })
}

#[tauri::command]
pub fn open_screenshot_ui(window: Window) -> Result<(), String> {
    window.run_on_main_thread(|| {
        if let Err(e) = crate::screenshot::ui::ScreenshotApp::show_standalone() {
            eprintln!("Failed to open screenshot UI: {}", e);
        }
    })
    .map_err(|e| e.to_string()) // 将tauri::Error转换为String
}