//! 独立差异对比窗口：窗口生命周期 + 主窗口到对比窗口的请求交接。
//!
//! 请求为什么要在 Rust 侧过一手，而不是全塞进 URL 或直接发事件：
//! 1. "未保存改动"的对比要带上整份缓冲区文本，几 MB 的内容放进 query string
//!    会被 URL 长度和百分号转义拖死；
//! 2. 窗口冷启动时前端还没挂上监听，这时候发事件必丢。所以请求先落到一个待取槽位，
//!    页面挂载后主动 take 一次；窗口已经开着的话再补一个事件把它叫醒来取。
//!
//! 对比窗口只开一个：新请求替换窗口内容并把窗口拉到前台，语义和编辑器里复用 diff 标签一致。

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tracing::info;
use urlencoding::encode;

pub const DIFF_WINDOW_LABEL: &str = "diff-compare";
const DIFF_WINDOW_TITLE: &str = "差异对比";
/// 通知已经打开的对比窗口来取新请求（只有对比窗口在听）。
const DIFF_REQUEST_EVENT: &str = "diff-compare:request";

/// 一次对比请求。字段和前端 diffCompareStore 的 state 一一对应。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DiffWindowRequest {
    /// "file" | "dir" | "text" | "buffer"
    pub mode: String,
    pub left_path: String,
    pub right_path: String,
    pub left_label: String,
    pub right_label: String,
    /// text / buffer 模式才有值，可能很大，只在交接时传一次。
    pub left_text: Option<String>,
    pub right_text: Option<String>,
    /// 预填型请求（Header 那个"带上次路径开窗"的入口）落地后不该自己跑起来：
    /// 目录扫描很贵，路径是替用户填的，跑不跑得他自己点。缺省和 true 都算跑。
    pub auto_run: Option<bool>,
}

static PENDING: Lazy<Mutex<Option<DiffWindowRequest>>> = Lazy::new(|| Mutex::new(None));

/// 打开（或唤醒）独立差异对比窗口。
///
/// - `request` 为空表示只把窗口叫出来，两侧留给用户自己挑；
/// - `only_if_new` 为真时，请求只作为新窗口的初始值：窗口已经开着就单纯拉到前台，
///   不覆盖用户正在看的对比结果（工具栏那种"顺手带上当前文件"的入口需要这个语义）。
#[tauri::command]
pub async fn open_diff_compare_window(
    app: AppHandle,
    request: Option<DiffWindowRequest>,
    theme: Option<String>,
    only_if_new: Option<bool>,
) -> Result<(), String> {
    let only_if_new = only_if_new.unwrap_or(false);

    if let Some(win) = app.get_webview_window(DIFF_WINDOW_LABEL) {
        // 已经开着：请求落槽后让它自己来取（only_if_new 时不打扰，纯粹前台化）。
        // 这里用全局 emit 而不是 emit_to：前端 listen() 注册的是 Any 目标，
        // emit_to(label) 的过滤只认带 label 的监听者，配不上就等于没发出去。
        if let Some(req) = request {
            if !only_if_new {
                info!(
                    "[DiffCompareWindow] queue request mode={} left={} right={}",
                    req.mode, req.left_path, req.right_path
                );
                *PENDING.lock() = Some(req);
                let _ = app.emit(DIFF_REQUEST_EVENT, ());
            }
        }
        let _ = win.unminimize();
        win.show().map_err(|e| format!("显示窗口失败: {}", e))?;
        let _ = win.set_focus();
        return Ok(());
    }

    if let Some(req) = request {
        info!(
            "[DiffCompareWindow] queue request mode={} left={} right={}",
            req.mode, req.left_path, req.right_path
        );
        *PENDING.lock() = Some(req);
    }

    let theme = theme.unwrap_or_default().trim().to_lowercase();
    let url = if theme == "dark" || theme == "light" {
        WebviewUrl::App(format!("/diff-compare?theme={}", encode(&theme)).into())
    } else {
        WebviewUrl::App("/diff-compare".into())
    };

    info!("[DiffCompareWindow] create window url={:?}", url);

    let window = WebviewWindowBuilder::new(&app, DIFF_WINDOW_LABEL, url)
        .title(DIFF_WINDOW_TITLE)
        .inner_size(1360.0, 860.0)
        .min_inner_size(880.0, 520.0)
        .resizable(true)
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
    window.set_focus().ok();

    Ok(())
}

/// 对比窗口挂载完、或收到 `diff-compare:request` 之后来取一次；取走即清空。
#[tauri::command]
pub fn diff_compare_take_request() -> Option<DiffWindowRequest> {
    PENDING.lock().take()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffPathKind {
    /// 原样回传，前端按输入字符串对号入座。
    pub path: String,
    /// "file" | "dir" | "missing"
    pub kind: &'static str,
}

/// 拖进窗口或手填的路径要先分清是文件还是目录，才知道该走哪种对比。
/// 跟随符号链接：指向目录的链接按目录算，和对比引擎的扫描行为一致。
#[tauri::command]
pub fn diff_probe_paths(paths: Vec<String>) -> Vec<DiffPathKind> {
    paths
        .into_iter()
        .map(|p| {
            let kind = match std::fs::metadata(Path::new(&p)) {
                Ok(md) if md.is_dir() => "dir",
                Ok(_) => "file",
                Err(_) => "missing",
            };
            DiffPathKind { path: p, kind }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_tells_dir_from_file_and_missing() {
        let dir = std::env::temp_dir().join("guaxi-diff-probe-test");
        let file = dir.join("a.txt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&file, b"x").unwrap();

        let missing = dir.join("nope-nope").to_string_lossy().to_string();
        let got = diff_probe_paths(vec![
            dir.to_string_lossy().to_string(),
            file.to_string_lossy().to_string(),
            missing,
        ]);

        assert_eq!(got.len(), 3);
        assert_eq!(got[0].kind, "dir");
        assert_eq!(got[1].kind, "file");
        assert_eq!(got[2].kind, "missing");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pending_request_is_taken_once() {
        *PENDING.lock() = Some(DiffWindowRequest {
            mode: "file".into(),
            left_path: "L".into(),
            right_path: "R".into(),
            ..Default::default()
        });
        let first = diff_compare_take_request();
        assert!(first.is_some());
        assert_eq!(first.unwrap().left_path, "L");
        assert!(diff_compare_take_request().is_none());
    }
}
