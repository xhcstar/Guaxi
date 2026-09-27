//! 旅行志照片查看器独立窗口:窗口生命周期 + 主窗口到查看器窗口的照片交接。
//!
//! 为什么走 Rust 过一手而不是塞进 URL 或直接发事件(与 diff_compare_window 同理):
//! 1. 整册可能有数千张照片,连同路径/坐标/城市塞进 query string 会被 URL 长度和
//!    百分号转义拖死;
//! 2. 窗口冷启动时前端还没挂上监听,这时候发事件必丢。所以请求先落到一个待取槽位,
//!    页面挂载后主动 take 一次;窗口已经开着的话再补一个事件把它叫醒来取。
//!
//! 查看器窗口只开一个:再点一张照片就替换窗口内容并把窗口拉到前台。

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tracing::info;
use urlencoding::encode;

pub const VIEWER_WINDOW_LABEL: &str = "life-album-viewer";
const VIEWER_WINDOW_TITLE: &str = "旅行志 · 影像";
/// 通知已经打开的查看器窗口来取新请求(只有查看器窗口在听)。
const VIEWER_REQUEST_EVENT: &str = "life-album-viewer:request";

/// 单张照片的展示所需字段。字段名保持 snake_case,与前端现有照片对象一一对应。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewerPhoto {
    pub path: String,
    pub thumb: Option<String>,
    pub taken_at: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub file_name: String,
    pub city: String,
    /// "image" | "video"。老数据缺省为空串,前端按扩展名兜底判定。
    pub media_type: String,
    /// 视频时长(秒)。图片为 None。
    pub duration: Option<f64>,
}

/// 一次查看请求:整册(或当前序列)照片 + 起始下标。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LifeAlbumViewerRequest {
    pub photos: Vec<ViewerPhoto>,
    pub index: usize,
}

static PENDING: Lazy<Mutex<Option<LifeAlbumViewerRequest>>> = Lazy::new(|| Mutex::new(None));

/// 把某个文件路径的父目录加入 asset 协议作用域(去重)。独立窗口是另一个 webview,
/// 但 asset 作用域是 app 全局的,给原图/缩略图父目录放行即可让新窗口加载它们。
fn allow_parent(app: &AppHandle, seen: &mut HashSet<String>, file_path: &str) {
    let raw = file_path.trim();
    if raw.is_empty() {
        return;
    }
    if let Some(parent) = Path::new(raw).parent() {
        let key = parent.to_string_lossy().to_string();
        if key.is_empty() || !seen.insert(key) {
            return;
        }
        let _ = app.asset_protocol_scope().allow_directory(parent, false);
    }
}

/// 打开(或唤醒)独立照片查看器窗口。
#[tauri::command]
pub async fn open_life_album_viewer_window(
    app: AppHandle,
    request: LifeAlbumViewerRequest,
    theme: Option<String>,
) -> Result<(), String> {
    // 给原图与缩略图所在目录放行(去重),保证新窗口能通过 asset:// 加载。
    {
        let mut seen: HashSet<String> = HashSet::new();
        for p in &request.photos {
            allow_parent(&app, &mut seen, &p.path);
            if let Some(t) = p.thumb.as_deref() {
                allow_parent(&app, &mut seen, t);
            }
        }
    }

    info!(
        "[LifeAlbumViewerWindow] request photos={} index={}",
        request.photos.len(),
        request.index
    );

    if let Some(win) = app.get_webview_window(VIEWER_WINDOW_LABEL) {
        // 已经开着:请求落槽后让它自己来取(全局 emit —— 前端 listen 注册的是 Any 目标)。
        *PENDING.lock() = Some(request);
        let _ = app.emit(VIEWER_REQUEST_EVENT, ());
        let _ = win.unminimize();
        win.show().map_err(|e| format!("显示窗口失败: {}", e))?;
        let _ = win.set_focus();
        return Ok(());
    }

    *PENDING.lock() = Some(request);

    let theme = theme.unwrap_or_default().trim().to_lowercase();
    let url = if theme == "dark" || theme == "light" {
        WebviewUrl::App(format!("/life-album-viewer?theme={}", encode(&theme)).into())
    } else {
        WebviewUrl::App("/life-album-viewer".into())
    };

    info!("[LifeAlbumViewerWindow] create window url={:?}", url);

    let window = WebviewWindowBuilder::new(&app, VIEWER_WINDOW_LABEL, url)
        .title(VIEWER_WINDOW_TITLE)
        .inner_size(1360.0, 880.0)
        .min_inner_size(900.0, 600.0)
        .resizable(true)
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
    window.set_focus().ok();

    Ok(())
}

/// 查看器窗口挂载完、或收到 `life-album-viewer:request` 之后来取一次;取走即清空。
#[tauri::command]
pub fn life_album_viewer_take_request() -> Option<LifeAlbumViewerRequest> {
    PENDING.lock().take()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_request_is_taken_once() {
        *PENDING.lock() = Some(LifeAlbumViewerRequest {
            photos: vec![ViewerPhoto {
                path: "P".into(),
                ..Default::default()
            }],
            index: 3,
        });
        let first = life_album_viewer_take_request();
        assert!(first.is_some());
        let first = first.unwrap();
        assert_eq!(first.index, 3);
        assert_eq!(first.photos.len(), 1);
        assert_eq!(first.photos[0].path, "P");
        assert!(life_album_viewer_take_request().is_none());
    }
}
