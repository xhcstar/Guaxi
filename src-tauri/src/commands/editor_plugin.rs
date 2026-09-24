use crate::guaxiservices::bundled_resources;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use tauri::command;

/// 用户可编写的编辑器插件(MVP:transform 类)。
/// 插件目录:utils/guaxi_editor_plugins/<id>/,内含 manifest.json + 入口脚本(默认 main.py)。
/// 契约:入口脚本定义 `def run(ctx): return {"text": <替换文本>}`,ctx 含 selection/document/args。
/// surfaces 字段现在只识别 "editor";"agent" 先解析后忽略,为将来 agent 工具接线预留。
#[derive(Serialize, Debug, Clone)]
pub struct EditorPluginInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub capabilities: Vec<String>,
    pub surfaces: Vec<String>,
    pub scope: String,
    pub dir: String,
    pub entry: String,
    pub entry_path: String,
}

fn editor_plugins_dir() -> Result<PathBuf, String> {
    let dir = bundled_resources::ensure_relative_dir("utils/guaxi_editor_plugins")?;
    fs::create_dir_all(&dir).map_err(|e| format!("create editor plugins dir failed: {e}"))?;
    Ok(dir)
}

const EXAMPLE_PLUGIN_MANIFEST: &str = r#"{
  "id": "pangu-spacing",
  "name": "中英文加空格",
  "description": "在中文与英文/数字之间自动插入空格,让排版更整齐(示例插件,可复制修改)",
  "entry": "main.py",
  "capabilities": ["transform"],
  "surfaces": ["editor"],
  "scope": "selection"
}
"#;

const EXAMPLE_PLUGIN_MAIN: &str = r#"# Guaxi 示例插件:中英文之间自动加空格(pangu 风格)。
# 契约:实现 run(ctx) 并返回 {"text": 新文本}。ctx 提供:
#   ctx["selection"] 当前选中文本 / ctx["document"] 全文 / ctx["args"] 预留参数
import re

_CJK = "一-鿿぀-ヿ㐀-䶿"


def _pangu(text):
    text = re.sub("([" + _CJK + "])([A-Za-z0-9])", r"\1 \2", text)
    text = re.sub("([A-Za-z0-9])([" + _CJK + "])", r"\1 \2", text)
    return text


def run(ctx):
    selection = ctx.get("selection", "") if isinstance(ctx, dict) else ""
    return {"text": _pangu(selection)}
"#;

// PLACEHOLDER_EDITOR_PLUGIN_REST

fn str_array(v: &serde_json::Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn seed_example_plugin_if_empty(dir: &PathBuf) {
    // 插件目录为空时写入一个可运行示例,给用户可参照、可复制的起点。
    let has_any = fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).any(|e| e.path().is_dir()))
        .unwrap_or(false);
    if has_any {
        return;
    }
    let example_dir = dir.join("pangu-spacing");
    if fs::create_dir_all(&example_dir).is_err() {
        return;
    }
    let _ = fs::write(example_dir.join("manifest.json"), EXAMPLE_PLUGIN_MANIFEST);
    let _ = fs::write(example_dir.join("main.py"), EXAMPLE_PLUGIN_MAIN);
}

#[command]
pub fn get_editor_plugins_dir() -> Result<String, String> {
    Ok(editor_plugins_dir()?.to_string_lossy().to_string())
}

#[command]
pub fn list_editor_plugins() -> Result<Vec<EditorPluginInfo>, String> {
    let dir = editor_plugins_dir()?;
    seed_example_plugin_if_empty(&dir);
    let mut plugins = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("read plugins dir failed: {e}"))?;
    for entry in entries.filter_map(|e| e.ok()) {
        let plugin_dir = entry.path();
        if !plugin_dir.is_dir() {
            continue;
        }
        let raw = match fs::read_to_string(plugin_dir.join("manifest.json")) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let manifest: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let dir_name = plugin_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let id = manifest
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or(&dir_name)
            .to_string();
        if id.is_empty() {
            continue;
        }
        let name = manifest
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&id)
            .to_string();
        let description = manifest
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let capabilities = str_array(&manifest, "capabilities");
        let surfaces = {
            let s = str_array(&manifest, "surfaces");
            if s.is_empty() {
                vec!["editor".to_string()]
            } else {
                s
            }
        };
        // 现在只暴露带 editor 触发面的插件;仅声明 agent 的先跳过(预留不实装)。
        if !surfaces.iter().any(|s| s == "editor") {
            continue;
        }
        let scope = manifest
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("selection")
            .to_string();
        let entry = manifest
            .get("entry")
            .and_then(|v| v.as_str())
            .unwrap_or("main.py")
            .to_string();
        let entry_path = plugin_dir.join(&entry);
        if !entry_path.is_file() {
            continue;
        }
        plugins.push(EditorPluginInfo {
            id,
            name,
            description,
            capabilities,
            surfaces,
            scope,
            dir: plugin_dir.to_string_lossy().to_string(),
            entry,
            entry_path: entry_path.to_string_lossy().to_string(),
        });
    }
    plugins.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(plugins)
}

