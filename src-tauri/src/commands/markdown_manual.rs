use tauri::Manager;
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tracing::info;
use urlencoding::encode;

#[tauri::command]
pub async fn create_markdown_manual_window(
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
        WebviewUrl::App(format!("/markdown-manual?theme={}", encode(&theme)).into())
    } else {
        WebviewUrl::App("/markdown-manual".into())
    };

    info!(
        "[MarkdownManual] create window label={} title={} url={:?}",
        label, title, url
    );

    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(1200.0, 800.0)
        .min_inner_size(900.0, 600.0)
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
    window.set_focus().ok();

    Ok(())
}

#[tauri::command]
pub async fn create_syntax_manual_window(
    app: tauri::AppHandle,
    label: String,
    title: String,
    language: String,
    source_path: String,
    theme: Option<String>,
) -> Result<(), String> {
    if app.get_webview_window(&label).is_some() {
        return Err(format!("窗口 {} 已存在", label));
    }

    let normalized_language = match language.trim().to_lowercase().as_str() {
        "latex" | "tex" => "latex",
        _ => "typst",
    };
    let theme = theme.unwrap_or_default().trim().to_lowercase();

    // 设计说明：
    // 1. Typst/LaTeX 手册与 Markdown 手册一样使用独立 WebviewWindow，允许用户缩放、最大化。
    // 2. sourcePath 只用于预览构建时确定当前文档所在目录，使相对路径和现有引擎逻辑保持一致。
    // 3. 所有参数显式编码进 App URL，避免 Windows 路径中的空格、反斜杠和中文破坏路由解析。
    let mut query = format!(
        "language={}&sourcePath={}",
        encode(normalized_language),
        encode(source_path.trim())
    );
    if theme == "dark" || theme == "light" {
        query.push_str(&format!("&theme={}", encode(&theme)));
    }
    let url = WebviewUrl::App(format!("/syntax-manual?{}", query).into());

    info!(
        "[SyntaxManual] create window label={} title={} language={} sourcePath={} url={:?}",
        label,
        title,
        normalized_language,
        source_path,
        url
    );

    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(1360.0, 860.0)
        .min_inner_size(980.0, 640.0)
        .resizable(true)
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
    window.set_focus().ok();

    Ok(())
}
