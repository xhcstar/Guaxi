//! 行迹的个人偏好：家庭地址、车辆、默认出行/预算/人数、兴趣与学习计数。
//!
//! 只保存在本机 `repos/travel/preferences.json`，不参与分享、长图、PDF 或任何网络请求。
//! 偏好里可能包含家庭地址，因此这里不写日志，也不和计划文件混在一起。

use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;

use super::store;

fn preferences_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(store::root(app)?.join("preferences.json"))
}

pub fn load(app: &AppHandle) -> Result<Value, String> {
    let path = preferences_path(app)?;
    if !path.exists() {
        return Ok(json!({}));
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取偏好失败: {e}"))?;
    if raw.trim().is_empty() {
        return Ok(json!({}));
    }
    match serde_json::from_str::<Value>(&raw) {
        Ok(value) if value.is_object() => Ok(value),
        _ => {
            // 损坏的偏好不影响规划；留一份备份方便排查。
            let backup = path.with_file_name(format!(
                "preferences.corrupt-{}.json",
                chrono::Local::now().format("%Y%m%d-%H%M%S")
            ));
            let _ = fs::copy(&path, &backup);
            Ok(json!({}))
        }
    }
}

pub fn save(app: &AppHandle, preferences: Value) -> Result<Value, String> {
    if !preferences.is_object() {
        return Err("偏好必须是 JSON 对象".to_string());
    }
    let path = preferences_path(app)?;
    let text = serde_json::to_string_pretty(&preferences)
        .map_err(|e| format!("序列化偏好失败: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text.as_bytes()).map_err(|e| format!("写入偏好失败: {e}"))?;
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    fs::rename(&tmp, &path).map_err(|e| format!("保存偏好失败: {e}"))?;
    Ok(preferences)
}
