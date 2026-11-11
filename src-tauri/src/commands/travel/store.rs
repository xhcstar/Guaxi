//! 旅行计划的本地存储。
//!
//! 计划是文档型数据（每天的结构都在变），用 JSON 文件比 SQLite 更合适：可读、可手动备份、
//! 不需要迁移。目录放在 exe 同级的 `repos/travel`，和 TODO 一样可随绿色版整体拷走。

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

pub fn sanitize_id(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("计划 id 不能为空".to_string());
    }
    if trimmed.len() > 80 {
        return Err("计划 id 过长".to_string());
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err("计划 id 只能包含字母、数字、下划线和短横线".to_string());
    }
    Ok(trimmed.to_string())
}

/// 旅行数据的根目录。计划、偏好、地图缓存都放在这里，随绿色版整体迁移。
pub fn travel_root() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("无法获取程序路径: {e}"))?;
    let base = exe
        .parent()
        .ok_or_else(|| "无法获取程序目录".to_string())?
        .join("repos")
        .join("travel");
    fs::create_dir_all(&base).map_err(|e| format!("创建旅行数据目录失败: {e}"))?;
    for sub in ["plans", "share"] {
        fs::create_dir_all(base.join(sub)).map_err(|e| format!("创建旅行子目录失败: {e}"))?;
    }
    Ok(base)
}

pub fn root(_app: &AppHandle) -> Result<PathBuf, String> {
    travel_root()
}

fn plans_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(root(app)?.join("plans"))
}

fn plan_path(app: &AppHandle, plan_id: &str) -> Result<PathBuf, String> {
    let id = sanitize_id(plan_id)?;
    Ok(plans_dir(app)?.join(format!("{id}.json")))
}

pub fn save_plan(app: &AppHandle, mut plan: Value) -> Result<Value, String> {
    let id = plan
        .get("id")
        .and_then(|item| item.as_str())
        .ok_or_else(|| "计划缺少 id".to_string())?;
    let id = sanitize_id(id)?;
    if !plan.is_object() {
        return Err("计划数据必须是对象".to_string());
    }
    let now = chrono::Local::now().to_rfc3339();
    if plan
        .get("createdAt")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        plan["createdAt"] = Value::String(now.clone());
    }
    plan["updatedAt"] = Value::String(now);
    plan["id"] = Value::String(id.clone());
    plan["schemaVersion"] = json!(1);

    let path = plan_path(app, &id)?;
    let text = serde_json::to_string_pretty(&plan)
        .map_err(|e| format!("序列化计划失败: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text.as_bytes()).map_err(|e| format!("写入计划失败: {e}"))?;
    if path.exists() {
        let backup = path.with_extension("json.bak");
        let _ = fs::copy(&path, &backup);
        let _ = fs::remove_file(&path);
    }
    fs::rename(&tmp, &path).map_err(|e| format!("保存计划失败: {e}"))?;
    Ok(plan)
}

fn summarize(plan: &Value) -> Value {
    let text = |pointer: &str| {
        plan.pointer(pointer)
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .to_string()
    };
    let number = |pointer: &str| {
        plan.pointer(pointer)
            .and_then(|item| item.as_f64())
            .unwrap_or(0.0)
    };
    let bool_value = |pointer: &str| {
        plan.pointer(pointer)
            .and_then(|item| item.as_bool())
            .unwrap_or(false)
    };
    json!({
        "id": text("/id"),
        "kind": text("/kind"),
        "title": text("/title"),
        "subtitle": text("/subtitle"),
        "startDate": text("/startDate"),
        "endDate": text("/endDate"),
        "createdAt": text("/createdAt"),
        "updatedAt": text("/updatedAt"),
        "originName": text("/origin/name"),
        "destinationName": text("/destination/name"),
        "destinationCity": text("/destination/city"),
        "seriesId": text("/seriesId"),
        "weekendStart": text("/weekendStart"),
        "adoptedAt": text("/adoptedAt"),
        "snapshotOf": text("/snapshotOf"),
        "originalKind": text("/originalKind"),
        "locked": bool_value("/locked"),
        "source": text("/source"),
        "budgetTotal": number("/budget/total"),
        "people": number("/travelers/adults") + number("/travelers/children") + number("/travelers/elders"),
        "dayCount": plan.get("days").and_then(|item| item.as_array()).map(|items| items.len()).unwrap_or(0),
    })
}

pub fn list_plans(app: &AppHandle) -> Result<Vec<Value>, String> {
    let dir = plans_dir(app)?;
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(Vec::new()),
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|item| item.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(plan) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        if plan.get("id").and_then(|item| item.as_str()).is_some() {
            out.push(summarize(&plan));
        }
    }
    out.sort_by(|a, b| {
        let a = a
            .get("updatedAt")
            .and_then(|item| item.as_str())
            .unwrap_or("");
        let b = b
            .get("updatedAt")
            .and_then(|item| item.as_str())
            .unwrap_or("");
        b.cmp(a)
    });
    Ok(out)
}

pub fn get_plan(app: &AppHandle, plan_id: &str) -> Result<Value, String> {
    let path = plan_path(app, plan_id)?;
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取计划失败: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析计划失败: {e}"))
}

pub fn delete_plan(app: &AppHandle, plan_id: &str) -> Result<(), String> {
    let path = plan_path(app, plan_id)?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("删除计划失败: {e}"))?;
    }
    let backup = path.with_extension("json.bak");
    if backup.exists() {
        let _ = fs::remove_file(&backup);
    }
    Ok(())
}

#[allow(dead_code)]
pub fn plan_exists(app: &AppHandle, plan_id: &str) -> bool {
    plan_path(app, plan_id)
        .map(|path| Path::new(&path).exists())
        .unwrap_or(false)
}
