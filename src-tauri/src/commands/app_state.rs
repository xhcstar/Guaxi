//! 前端状态快照（localStorage 的落盘镜像）。
//!
//! 正式版的 localStorage 存在 WebView2 的数据目录里，卸载时勾一下"删除应用数据"就没了。
//! 前端把有长期价值的 key 汇总成一份 JSON 交给这里，落在用户数据根下，下次启动再补回去。
//! 只做整份覆盖写，不做增量：快照本身很小，简单比聪明可靠。

use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn snapshot_path() -> PathBuf {
    crate::paths::sub_dir("state").join("local_storage.json")
}

#[tauri::command]
pub fn storage_snapshot_read() -> Result<Option<String>, String> {
    let path = snapshot_path();
    if !path.is_file() {
        return Ok(None);
    }
    fs::read_to_string(&path)
        .map(Some)
        .map_err(|e| format!("读取状态快照失败: {e}"))
}

#[tauri::command]
pub fn storage_snapshot_write(json: String) -> Result<(), String> {
    // 限制一下体积：正常的键值快照只有几十 KB，真涨到这个量级说明前端把缓存也塞进来了。
    if json.len() > 8 * 1024 * 1024 {
        return Err("状态快照过大，已跳过本次保存".to_string());
    }
    let path = snapshot_path();
    // 先写临时文件再 rename：避免进程被杀时留下半截 JSON。
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&tmp).map_err(|e| format!("写入状态快照失败: {e}"))?;
        file.write_all(json.as_bytes())
            .map_err(|e| format!("写入状态快照失败: {e}"))?;
        let _ = file.sync_all();
    }
    fs::rename(&tmp, &path).map_err(|e| format!("保存状态快照失败: {e}"))?;
    Ok(())
}
