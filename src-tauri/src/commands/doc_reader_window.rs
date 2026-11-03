//! 独立文档阅读器窗口。
//!
//! 设计约束（与主编辑器零耦合）：
//!
//! 阅读器原本作为覆盖层挂在编辑器里，与 Monaco 共用同一个组件树。二进制文档的阅读、
//! 搜索、批注、编辑能力要持续扩张，挤在编辑器里每加一个功能都要先确认「不会碰到编辑器」。
//! 这里把它搬进独立窗口：主编辑器只负责「发现是二进制文档 → 建一个窗口」，此后所有
//! 阅读器逻辑都在自己的进程内运行，编辑器的 DOM、model、保存链路完全不受影响。
//!
//! 窗口参数通过 URL query 传递（与 markdown-manual / syntax-manual 同一套做法），
//! 因为 Windows 路径里的空格、反斜杠与中文必须显式编码，否则会破坏路由解析。

use serde::Serialize;
use tauri::Manager;
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tracing::info;
use urlencoding::encode;

/// 一个已登记的阅读器窗口（用于「同一文件不重复开窗」）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenReaderWindow {
    pub label: String,
    pub existed: bool,
}

/// 等前端「首帧就绪」信号的最长时间。
///
/// 正常情况下前端渲染完会立刻 show()，远早于这个期限。它的作用是兜底：前端脚本
/// 崩了、或某个资源缺失导致渲染永远不完成时，至少在这个时间后把窗口显示出来，
/// 让用户看到内容（或错误提示），而不是对着一个永远不出现的窗口干等。
const FIRST_PAINT_FALLBACK_MS: u64 = 5_000;

/// 窗口标签：以文件路径的稳定哈希命名，保证同一文件只有一扇窗口。
///
/// 不能用纯路径（含 `/`、`\`、`:` 等非法字符），也不能用时间戳（那样每次点都开新窗）。
fn reader_window_label(path: &str) -> String {
    // FNV-1a 64：稳定、无依赖，碰撞概率对「同时打开的窗口数」这个量级完全够用。
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in path.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("doc-reader-{hash:016x}")
}

/// 打开（或聚焦）一个文档阅读器窗口。
///
/// `kind` 是后端 `sniff_file_kind` 判定的规范化类型（epub / mobi / pdf / docx / xlsx …），
/// 阅读器窗口据此决定用哪个子组件。
#[tauri::command]
pub async fn create_doc_reader_window(
    app: tauri::AppHandle,
    path: String,
    name: String,
    kind: String,
    reader: String,
    theme: Option<String>,
) -> Result<OpenReaderWindow, String> {
    let path = path.trim().to_string();
    if path.is_empty() {
        return Err("文件路径为空".to_string());
    }
    let label = reader_window_label(&path);

    // 同一文件再次打开：把已有窗口提到前面，而不是再开一个。
    if let Some(existing) = app.get_webview_window(&label) {
        existing.show().ok();
        existing.set_focus().ok();
        info!("[DocReader] focus existing window label={}", label);
        return Ok(OpenReaderWindow {
            label,
            existed: true,
        });
    }

    let theme = theme.unwrap_or_default().trim().to_lowercase();
    let mut query = format!(
        "path={}&name={}&kind={}&reader={}",
        encode(&path),
        encode(name.trim()),
        encode(kind.trim()),
        encode(reader.trim())
    );
    if theme == "dark" || theme == "light" {
        query.push_str(&format!("&theme={}", encode(&theme)));
    }
    // 专用入口 doc-reader.html：只挂载阅读器子树。走主入口（/doc-reader 路由）会连带启动
    // Monaco/LSP 并加载全部路由页面，每个新窗口都要多花数秒。
    //
    // 文件名不能叫 reader.html：知识库的旧阅读窗口走主应用路由 `/reader`，而 Tauri 的
    // 资源解析（以及开发期 Vite 的 html fallback）会先把 `/reader` 当作 `reader.html`，
    // 两个入口撞名后旧窗口会被这个入口抢走，最后停在「缺少文件路径」。
    let url = WebviewUrl::App(format!("/doc-reader.html?{}", query).into());

    info!(
        "[DocReader] create window label={} kind={} reader={} path={}",
        label,
        kind,
        reader,
        path
    );

    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(if name.trim().is_empty() {
            "文档阅读器"
        } else {
            name.trim()
        })
        // 阅读器要能铺开看版式；最小尺寸保证目录栏与正文都放得下。
        .inner_size(1280.0, 860.0)
        .min_inner_size(900.0, 600.0)
        .resizable(true)
        .visible(false)
        .build()
        .map_err(|e| format!("创建阅读器窗口失败: {}", e))?;

    // 窗口以隐藏状态创建，真正显示的时刻交给前端：DocReaderWindow 等到阅读器渲染出
    // 首帧后才调用 show()，用户看到的第一眼就是内容，而不是一块白底。
    spawn_first_paint_fallback(window);

    Ok(OpenReaderWindow {
        label,
        existed: false,
    })
}

/// 等前端「首帧就绪」信号的兜底。
///
/// 这里不再用固定 sleep 猜时间 —— 慢机器上 100ms 远远不够（那正是「窗口先白一下」
/// 的来源），快机器上又白等。但这个兜底是必需的：前端脚本报错、资源缺失时
/// show() 永远不会被调用，窗口就会无声地一直隐藏，用户只会以为程序没反应。
fn spawn_first_paint_fallback(window: tauri::WebviewWindow) {
    let fallback = window.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(FIRST_PAINT_FALLBACK_MS)).await;
        if fallback.is_visible().unwrap_or(false) {
            return;
        }
        info!("[DocReader] no first-paint signal from frontend, showing window as fallback");
        fallback.show().ok();
        fallback.set_focus().ok();
    });
}

/// 打开（或聚焦）一篇外部资料的「全文阅读器」窗口。
///
/// 这根命令服务的不是磁盘上的文件，而是认知层收件箱里的一条感知（正文在 sqlite 里）。
/// 所以窗口里不放文件路径，只放感知 id —— 正文可能有几十 KB，塞进 query string
/// 会被长度与转义拖死（与 mail-compose 同一个理由）；窗口自己在挂载后取一次正文。
///
/// 窗口 label 沿用 `doc-reader-` 前缀：capabilities 里 `doc-reader-*` 已经放行，
/// 不为一扇新窗口再放宽一份权限范围。
#[tauri::command]
pub async fn create_article_reader_window(
    app: tauri::AppHandle,
    id: String,
    title: String,
    theme: Option<String>,
) -> Result<OpenReaderWindow, String> {
    let id = id.trim().to_string();
    if id.is_empty() {
        return Err("资料 id 为空".to_string());
    }
    let title = title.trim().to_string();
    let label = reader_window_label(&format!("article:{id}"));

    if let Some(existing) = app.get_webview_window(&label) {
        existing.show().ok();
        existing.set_focus().ok();
        info!("[ArticleReader] focus existing window label={}", label);
        return Ok(OpenReaderWindow {
            label,
            existed: true,
        });
    }

    let theme = theme.unwrap_or_default().trim().to_lowercase();
    let mut query = format!("id={}&title={}", encode(&id), encode(&title));
    if theme == "dark" || theme == "light" {
        query.push_str(&format!("&theme={}", encode(&theme)));
    }

    let url = WebviewUrl::App(format!("/article-reader.html?{}", query).into());

    info!(
        "[ArticleReader] create window label={} title={} id={}",
        label, title, id
    );

    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(if title.is_empty() {
            "全文阅读"
        } else {
            title.as_str()
        })
        .inner_size(1040.0, 860.0)
        .min_inner_size(560.0, 480.0)
        .resizable(true)
        .visible(false)
        .build()
        .map_err(|e| format!("创建阅读器窗口失败: {}", e))?;

    spawn_first_paint_fallback(window);

    Ok(OpenReaderWindow {
        label,
        existed: false,
    })
}

/// 关闭某个文件的阅读器窗口（主编辑器关闭标签时调用）。
#[tauri::command]
pub async fn close_doc_reader_window(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let label = reader_window_label(path.trim());
    if let Some(window) = app.get_webview_window(&label) {
        info!("[DocReader] close window label={}", label);
        window.close().map_err(|e| format!("关闭阅读器窗口失败: {}", e))?;
    }
    Ok(())
}
