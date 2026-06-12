use tauri::Manager;
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tracing::info;
use urlencoding::encode;

/// 打开独立的任务管理面板窗口（加载 SPA 路由 /task-manager）。
#[tauri::command]
pub async fn create_task_manager_window(
    app: tauri::AppHandle,
    label: String,
    title: String,
    theme: Option<String>,
) -> Result<(), String> {
    if app.get_webview_window(&label).is_some() {
        return Err(format!("窗口 {} 已存在", label));
    }

    let theme = theme.unwrap_or_default().trim().to_lowercase();
    let url = if theme == "dark" || theme == "light" {
        WebviewUrl::App(format!("/task-manager?theme={}", encode(&theme)).into())
    } else {
        WebviewUrl::App("/task-manager".into())
    };

    info!(
        "[TaskManagerWindow] create window label={} title={} url={:?}",
        label, title, url
    );

    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(1280.0, 820.0)
        .min_inner_size(960.0, 640.0)
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
    window.set_focus().ok();

    Ok(())
}
