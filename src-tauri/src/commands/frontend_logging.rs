use serde::Deserialize;
use serde_json::Value;
use tracing::{debug, error, info, warn};

#[derive(Debug, Deserialize, Clone)]
pub struct FrontendLogEntry {
    pub level: String,
    pub scope: Option<String>,
    pub message: String,
    pub fields: Option<Value>,
    pub ts: Option<String>,
}

fn norm_level(level: &str) -> &str {
    match level.to_ascii_lowercase().as_str() {
        "debug" | "trace" => "debug",
        "info" | "log" => "info",
        "warn" | "warning" => "warn",
        "error" | "fatal" => "error",
        _ => "info",
    }
}

fn write_entry(e: &FrontendLogEntry) {
    let scope = e.scope.as_deref().unwrap_or("Frontend");
    let ts = e.ts.as_deref().unwrap_or("");
    let fields = e
        .fields
        .as_ref()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "".to_string());

    match norm_level(&e.level) {
        "debug" => {
            debug!(target: "frontend", scope = scope, ts = ts, fields = %fields, "{}", e.message)
        }
        "info" => {
            info!(target: "frontend", scope = scope, ts = ts, fields = %fields, "{}", e.message)
        }
        "warn" => {
            warn!(target: "frontend", scope = scope, ts = ts, fields = %fields, "{}", e.message)
        }
        "error" => {
            error!(target: "frontend", scope = scope, ts = ts, fields = %fields, "{}", e.message)
        }
        _ => info!(target: "frontend", scope = scope, ts = ts, fields = %fields, "{}", e.message),
    }
}

#[tauri::command]
pub fn frontend_log(
    level: String,
    scope: Option<String>,
    message: String,
    fields: Option<Value>,
) -> Result<(), String> {
    let entry = FrontendLogEntry {
        level,
        scope,
        message,
        fields,
        ts: Some(chrono::Local::now().to_rfc3339()),
    };
    write_entry(&entry);
    Ok(())
}

#[tauri::command]
pub fn frontend_log_batch(entries: Vec<FrontendLogEntry>) -> Result<(), String> {
    for e in entries {
        write_entry(&e);
    }
    Ok(())
}
