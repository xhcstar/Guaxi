//! 独立的写信窗口。
//!
//! 为什么要把写信搬进独立窗口：
//! - 写信是一个持续几分钟、可能粘很多图的过程，而应用页的编辑器是模态的——
//!   一旦打开，用户在应用里做不了别的事（想回去看一眼原文都不行）；
//! - 回复 / 转发时用户常需要在正文里对照原邮件，独立窗口可以和阅读区并排。
//!
//! 与 `diff_compare_window` 同一套交接方式：
//! - 草稿（含 HTML 正文、内联图片路径、引用头）在 Rust 侧过一手，不走 URL——
//!   一封带引用的回复正文动辄几十 KB，塞进 query string 会被长度与转义拖死；
//! - 窗口冷启动时前端还没挂上监听，事件必丢，所以请求先进待取槽位，页面挂载后 take 一次。
//!
//! 窗口按草稿 id 命名：同一份草稿只会有一扇窗口，重复打开只是把它拉到前台。

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tracing::info;
use urlencoding::encode;

/// 通知已经打开的写信窗口来取新草稿（只有写信窗口在听）。
const COMPOSE_REQUEST_EVENT: &str = "mail-compose:request";
/// 窗口关掉后告诉主窗口「这封草稿已经存好，可以刷新草稿箱」。
pub const COMPOSE_CLOSED_EVENT: &str = "mail-compose:closed";

/// 一次写信请求：字段和前端 `mailStore.composing` 一一对应。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ComposeRequest {
    /// 稳定 id：同一份草稿重复打开会复用同一扇窗口。
    pub id: String,
    /// `new` | `reply` | `replyAll` | `forward` | `draft`
    pub mode: String,
    pub account_id: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub text: String,
    pub html: String,
    pub html_mode: bool,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
    /// 续写的源草稿（发送 / 保存后要删掉它）。
    pub draft_uid: Option<u32>,
    pub draft_folder: Option<String>,
    /// 内联图片（`contentId` ↔ 本地路径）。
    pub inline_images: Vec<ComposeInlineImage>,
    /// 附件本地路径。
    pub attachments: Vec<String>,
    /// 打开时是否直接进「源码」模式。
    pub source_mode: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ComposeInlineImage {
    pub content_id: String,
    pub path: String,
    pub name: String,
}

static PENDING: Lazy<Mutex<Vec<(String, ComposeRequest)>>> = Lazy::new(|| Mutex::new(Vec::new()));

/// 窗口 label：`mail-compose-<短 id>`。label 只允许字母数字与 `-`。
fn window_label(draft_id: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in draft_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("mail-compose-{hash:016x}")
}

/// 打开（或唤醒）一扇写信窗口。
///
/// 传进来的完整草稿会落进待取槽位：新窗口挂载后主动取一次；窗口已经开着时
/// 再补一个事件把它叫醒来取。
#[tauri::command]
pub async fn mail_open_compose_window(
    app: AppHandle,
    request: ComposeRequest,
    theme: Option<String>,
) -> Result<String, String> {
    let label = window_label(&request.id);
    let id = request.id.clone();
    PENDING.lock().push((id.clone(), request));

    if let Some(win) = app.get_webview_window(&label) {
        // 已经开着：让它来取新草稿，并把窗口拉到前台。
        let _ = app.emit(COMPOSE_REQUEST_EVENT, label.clone());
        let _ = win.unminimize();
        win.show().map_err(|e| format!("显示写信窗口失败: {e}"))?;
        let _ = win.set_focus();
        info!("[MailCompose] focus existing window label={label}");
        return Ok(label);
    }

    let theme = theme.unwrap_or_default().trim().to_lowercase();
    let url = if theme == "dark" || theme == "light" {
        WebviewUrl::App(format!("/mail-compose.html?theme={}", encode(&theme)).into())
    } else {
        WebviewUrl::App("/mail-compose.html".into())
    };

    info!("[MailCompose] create window label={label}");

    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title("写邮件")
        // 写信要放得下正文 + 引用；最小尺寸保证工具栏不折成三行。
        .inner_size(980.0, 760.0)
        .min_inner_size(720.0, 520.0)
        .resizable(true)
        // 先隐藏，等前端首帧就绪再显示，避免「先白一下」。
        .visible(false)
        .build()
        .map_err(|e| format!("创建写信窗口失败: {e}"))?;

    // 兜底：前端脚本出错时 show() 永远不会被调用，窗口会无声地一直隐藏。
    let fallback = window.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(5_000)).await;
        if fallback.is_visible().unwrap_or(false) {
            return;
        }
        info!("[MailCompose] no first-paint signal, showing window as fallback");
        fallback.show().ok();
        fallback.set_focus().ok();
    });

    Ok(label)
}

/// 写信窗口挂载后 / 收到事件后取一次；取走即清空自己那条。
#[tauri::command]
pub fn mail_take_compose_request(label: String) -> Option<ComposeRequest> {
    let mut pending = PENDING.lock();
    let index = pending.iter().position(|(_, req)| window_label(&req.id) == label)?;
    let (_, req) = pending.remove(index);
    Some(req)
}

/// 写信窗口确认草稿已经落库（或发送成功），从待取槽位里删掉，避免内存里长期留着正文。
#[tauri::command]
pub fn mail_drop_compose_request(label: String) {
    PENDING.lock().retain(|(_, req)| window_label(&req.id) != label);
}

/// 前端首帧就绪：把窗口显示出来（主窗口也可以调用它来聚焦）。
#[tauri::command]
pub fn mail_compose_window_ready(app: AppHandle, label: String) {
    if let Some(win) = app.get_webview_window(&label) {
        win.show().ok();
        win.set_focus().ok();
    }
}

/// 窗口关闭前由前端调用：把「关闭原因」广播给主窗口，让它刷新草稿箱。
#[tauri::command]
pub fn mail_compose_closed(app: AppHandle, label: String, saved: bool) {
    let _ = app.emit(COMPOSE_CLOSED_EVENT, serde_json::json!({ "label": label, "saved": saved }));
}

/// 关掉某扇写信窗口（发送成功后由前端调用）。
#[tauri::command]
pub fn mail_close_compose_window(app: AppHandle, label: String) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(&label) {
        win.close().map_err(|e| format!("关闭写信窗口失败: {e}"))?;
    }
    Ok(())
}

/// 列出当前开着的写信窗口（主窗口用来判断「是否还有未保存的写信窗口」）。
#[tauri::command]
pub fn mail_compose_windows(app: AppHandle) -> Vec<String> {
    app.webview_windows()
        .keys()
        .filter(|label| label.starts_with("mail-compose-"))
        .cloned()
        .collect()
}
