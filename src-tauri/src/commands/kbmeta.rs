//! 知识库管理器窗口入口。
//!
//! 旧的 `.meta` 侧车知识管理(kbmeta_* 命令、文件钩子)已废弃 —— 现统一由 kgraph
//! 子系统(kg_index.db + .manual.kgraph.json 可移植快照)承担知识管理与多机同步。
//! 本模块仅保留打开“知识库管理器”窗口的命令。

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use urlencoding::encode;

#[tauri::command]
pub async fn create_knowledge_base_manager_window(
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
        WebviewUrl::App(format!("/kb-manager?theme={}", encode(&theme)).into())
    } else {
        WebviewUrl::App("/kb-manager".into())
    };

    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(1000.0, 720.0)
        .min_inner_size(820.0, 560.0)
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
    Ok(())
}
