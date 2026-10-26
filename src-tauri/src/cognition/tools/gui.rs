//! 操控本机：截屏、窗口、鼠标键盘、剪贴板。
//!
//! 默认关闭。坐标一律是逻辑像素，与截图坐标同一坐标系。

use super::*;
use serde_json::{json, Value};

pub(crate) fn handles(name: &str) -> bool {
    matches!(
        name,
        "guaxi_screen_capture"
        | "guaxi_screen_ocr"
        | "guaxi_window_list"
        | "guaxi_window_focus"
        | "guaxi_mouse"
        | "guaxi_keyboard"
        | "guaxi_clipboard"
        | "read"
        | "write"
    )
}

pub(crate) async fn run(name: &str, a: &Value) -> Result<Value, String> {
    match name {
        "guaxi_screen_capture" => {
            let dir = capture_dir()?;
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S-%3f");
            let raw = dir.join(format!("capture-{stamp}.png"));
            let region = parse_region(opt_str(a, "region"))?;
            let shot = crate::commands::computer::capture_screen(region, &raw)?;
            // 默认叠网格：模型用「第几格」表达位置比裸坐标稳，审批卡里也好解释。
            if opt_bool(a, "grid", true) {
                let gridded = dir.join(format!("capture-{stamp}-grid.png"));
                let cols = opt_usize(a, "cols", 12).clamp(1, 40) as u32;
                let rows = opt_usize(a, "rows", 8).clamp(1, 40) as u32;
                match crate::commands::computer::draw_grid(&raw, &gridded, cols, rows) {
                    Ok(()) => {
                        return Ok(json!({
                            "path": gridded.to_string_lossy(),
                            "rawPath": shot.path,
                            "width": shot.width, "height": shot.height,
                            "scale": shot.scale,
                            "grid": { "cols": cols, "rows": rows,
                                      "cellWidth": shot.width / cols, "cellHeight": shot.height / rows },
                            "hint": "坐标从 (0,0) 起算，网格编号按行优先（左上角是 1 号）"
                        }));
                    }
                    Err(e) => warn!("[cognition] 网格叠加失败，返回原图: {e}"),
                }
            }
            Ok(json!({
                "path": shot.path, "width": shot.width, "height": shot.height,
                "scale": shot.scale,
                "origin": { "x": shot.origin_x, "y": shot.origin_y }
            }))
        }
        "guaxi_screen_ocr" => {
            let app = crate::cognition::runtime::app_handle()
                .ok_or_else(|| "AppHandle 未就绪，暂时无法 OCR".to_string())?;
            let dir = capture_dir()?;
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S-%3f");
            let raw = dir.join(format!("ocr-{stamp}.png"));
            crate::commands::computer::capture_screen(None, &raw)?;
            let v = crate::commands::ocr::ocr_recognize(
                app,
                Some(raw.to_string_lossy().to_string()),
                None,
            )
            .await?;
            Ok(json!({ "path": raw.to_string_lossy(), "ocr": v }))
        }
        "guaxi_window_list" => {
            let list = crate::commands::computer::list_windows()?;
            Ok(json!({ "windows": list, "count": list.len() }))
        }
        "guaxi_window_focus" => {
            let w = crate::commands::computer::find_window(&req_str(a, "title")?)?;
            crate::commands::computer::focus_window(w.handle)?;
            Ok(json!({ "ok": true, "window": w }))
        }
        "guaxi_mouse" => {
            let action = crate::commands::computer::MouseAction {
                action: req_str(a, "action")?,
                x: a.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                y: a.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                to_x: a.get("toX").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                to_y: a.get("toY").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                amount: a.get("amount").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            };
            crate::commands::computer::mouse(&action)
        }
        "guaxi_keyboard" => {
            let keys = a
                .get("keys")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let action = crate::commands::computer::KeyboardAction {
                action: req_str(a, "action")?,
                text: opt_str(a, "text").unwrap_or_default(),
                keys,
            };
            crate::commands::computer::keyboard(&action)
        }
        "guaxi_clipboard" => match req_str(a, "action")?.as_str() {
            "read" => {
                let t = crate::commands::computer::read_clipboard_text()?;
                Ok(json!({ "text": t, "chars": t.chars().count() }))
            }
            "write" => {
                crate::commands::computer::write_clipboard_text(&opt_str(a, "text").unwrap_or_default())?;
                Ok(json!({ "ok": true }))
            }
            other => Err(format!("未知剪贴板动作: {other}")),
        },
        other => Err(format!("工具 {other} 尚未实现分发")),
    }
}
