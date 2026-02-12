use tauri::Manager;
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tracing::info;
use urlencoding::encode;

#[tauri::command]
pub async fn create_resource_repo_manager_window(
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
        WebviewUrl::App(format!("/resource-repo?theme={}", encode(&theme)).into())
    } else {
        WebviewUrl::App("/resource-repo".into())
    };

    info!(
        "[ResourceRepoWindow] create window label={} title={} url={:?}",
        label, title, url
    );

    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(1080.0, 760.0)
        .min_inner_size(820.0, 560.0)
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
    window.set_focus().ok();

    Ok(())
}

#[tauri::command]
pub async fn create_slidev_manager_window(
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
        WebviewUrl::App(format!("/slidev?theme={}", encode(&theme)).into())
    } else {
        WebviewUrl::App("/slidev".into())
    };

    info!(
        "[SlidevWindow] create window label={} title={} url={:?}",
        label, title, url
    );

    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(1400.0, 860.0)
        .min_inner_size(1100.0, 680.0)
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
    window.set_focus().ok();

    Ok(())
}
