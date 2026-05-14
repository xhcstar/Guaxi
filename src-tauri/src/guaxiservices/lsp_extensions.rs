use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use fs_extra::dir::{copy as copy_dir, CopyOptions};
use once_cell::sync::Lazy;
use parking_lot::Mutex as ParkingMutex;
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;
use tracing::{info, warn};
use uuid::Uuid;
use walkdir::WalkDir;
use zip::ZipArchive;

use crate::guaxiservices::{bundled_resources, lsp};

#[derive(Debug, Clone, Deserialize)]
struct CatalogRoot {
    version: u32,
    extensions: Vec<CatalogExtension>,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogExtension {
    id: String,
    display_name: String,
    description: Option<String>,
    open_vsx: Option<String>,
    vscode_marketplace: Option<String>,
    languages: Vec<String>,
    compatibility_level: String,
    recommended: bool,
    lsp: CatalogLspRuntime,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogLspRuntime {
    server_id: String,
    runtime: String,
    bin_name: String,
    args: Vec<String>,
    entry_paths: Vec<String>,
    resolve_order: Vec<String>,
    fallback_server: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct InstalledExtensionMeta {
    source: String,
    installed_at: String,
    download_url: Option<String>,
    open_vsx: Option<String>,
    vscode_marketplace: Option<String>,
    original_file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LspExtensionRuntimeStatus {
    pub ready: bool,
    pub runtime: String,
    pub program_path: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LspExtensionState {
    pub id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub source: String,
    pub version: String,
    pub languages: Vec<String>,
    pub installed: bool,
    pub installed_path: Option<String>,
    pub compatibility_level: String,
    pub recommended: bool,
    pub open_vsx: Option<String>,
    pub vscode_marketplace: Option<String>,
    pub manifest_name: Option<String>,
    pub manifest_publisher: Option<String>,
    pub manifest_browser: Option<String>,
    pub manifest_main: Option<String>,
    pub activation_events: Vec<String>,
    pub runtime_status: LspExtensionRuntimeStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenVsxSearchResult {
    pub id: String,
    pub namespace: String,
    pub name: String,
    pub version: String,
    pub display_name: String,
    pub description: Option<String>,
    pub download_url: Option<String>,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VscodeMarketplaceSearchResult {
    pub id: String,
    pub publisher: String,
    pub name: String,
    pub version: String,
    pub display_name: String,
    pub description: Option<String>,
    pub download_url: Option<String>,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LspExtensionTaskSnapshot {
    pub task_id: String,
    pub operation: String,
    pub extension_id: String,
    pub status: String,
    pub message: String,
    pub error: Option<String>,
    pub state: Option<LspExtensionState>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedLspExtensionCommand {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) envs: Vec<(String, String)>,
    pub(crate) cwd: Option<String>,
    pub(crate) extension_id: String,
    pub(crate) extension_display_name: String,
    pub(crate) server_id: String,
}

#[derive(Debug, Clone)]
struct InstalledExtensionRecord {
    id: String,
    version: String,
    installed_at: String,
    display_name: String,
    description: Option<String>,
    source: String,
    install_dir: PathBuf,
    open_vsx: Option<String>,
    vscode_marketplace: Option<String>,
    manifest_name: Option<String>,
    manifest_publisher: Option<String>,
    manifest_browser: Option<String>,
    manifest_main: Option<String>,
    activation_events: Vec<String>,
    languages: Vec<String>,
}

static LSP_EXTENSION_CATALOG: Lazy<CatalogRoot> = Lazy::new(|| {
    serde_json::from_str(include_str!("../../../shared/lsp_extensions.json"))
        .expect("invalid shared LSP extension catalog")
});
static LSP_EXTENSION_TASKS: Lazy<ParkingMutex<HashMap<String, LspExtensionTaskSnapshot>>> =
    Lazy::new(|| ParkingMutex::new(HashMap::new()));
static LSP_SHARED_NODE_RUNTIME_INSTALL_LOCK: Lazy<AsyncMutex<()>> =
    Lazy::new(|| AsyncMutex::new(()));
const LSP_CATALOG_RUNTIME_CONFIGURE_MAX_ATTEMPTS: u32 = 4;

fn normalize_string(raw: &str) -> String {
    raw.trim().to_string()
}

fn normalize_language(raw: &str) -> String {
    raw.trim().to_lowercase()
}

fn normalize_extension_id(raw: &str) -> String {
    raw.trim().to_lowercase()
}

fn exe_dir() -> Result<PathBuf, String> {
    let current_exe =
        std::env::current_exe().map_err(|e| format!("failed to resolve current exe: {e}"))?;
    let dir = current_exe
        .parent()
        .ok_or_else(|| "failed to resolve current exe directory".to_string())?
        .to_path_buf();
    Ok(dir)
}

fn extensions_root_dir() -> Result<PathBuf, String> {
    let dir = exe_dir()?.join("extensions");
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create extensions root: {e}"))?;
    Ok(dir)
}

fn extension_meta_path(dir: &Path) -> PathBuf {
    dir.join(".guaxi-extension.json")
}

fn temp_root_dir() -> Result<PathBuf, String> {
    let dir = exe_dir()?.join("tmp").join("lsp_extensions");
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create extension temp root: {e}"))?;
    Ok(dir)
}

fn lsp_runtime_root_dir() -> Result<PathBuf, String> {
    let dir = exe_dir()?.join("LspRuntimes");
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create LSP runtime root: {e}"))?;
    Ok(dir)
}

fn shared_node_runtime_dir() -> Result<PathBuf, String> {
    let dir = lsp_runtime_root_dir()?.join("node");
    fs::create_dir_all(&dir)
        .map_err(|e| format!("failed to create shared Node LSP runtime root: {e}"))?;
    Ok(dir)
}

fn proxy_env_is_set(name: &str) -> bool {
    std::env::var(name)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn log_lsp_network_proxy_context(operation: &str) {
    info!(
        operation,
        http_proxy = proxy_env_is_set("HTTP_PROXY") || proxy_env_is_set("http_proxy"),
        https_proxy = proxy_env_is_set("HTTPS_PROXY") || proxy_env_is_set("https_proxy"),
        all_proxy = proxy_env_is_set("ALL_PROXY") || proxy_env_is_set("all_proxy"),
        no_proxy = proxy_env_is_set("NO_PROXY") || proxy_env_is_set("no_proxy"),
        "building LSP extension HTTP client with system/env proxy support"
    );
}

fn lsp_extension_http_client(operation: &str) -> Result<Client, String> {
    log_lsp_network_proxy_context(operation);

    // 设计思路：
    // 1. LSP 扩展下载面向 Open VSX、VSCode Marketplace、GitHub 等外网站点，不能禁用 reqwest 默认代理能力。
    // 2. reqwest 默认会读取系统/环境代理；这里集中构造 Client，避免后续调用点误用 no_proxy。
    // 3. 下载慢时更需要稳定超时而不是无限挂起，所以连接和整体请求分别设置上限，并用日志记录代理环境是否存在。
    ClientBuilder::new()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| format!("failed to build LSP extension HTTP client: {e}"))
}

fn slidev_env_root() -> Result<PathBuf, String> {
    bundled_resources::ensure_relative_dir("utils/SlidevEnv")
}

fn slidev_node_path() -> Option<PathBuf> {
    let root = slidev_env_root().ok()?;
    #[cfg(target_os = "windows")]
    let candidate = root.join("node.exe");
    #[cfg(not(target_os = "windows"))]
    let candidate = root.join("node");
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

fn slidev_bin_dir() -> Option<PathBuf> {
    let root = slidev_env_root().ok()?;
    let dir = root.join("node_modules").join(".bin");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

fn sanitize_dir_fragment(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
}

fn install_dir_name(id: &str, version: &str) -> String {
    format!(
        "{}-{}",
        sanitize_dir_fragment(&normalize_extension_id(id)),
        sanitize_dir_fragment(version)
    )
}

fn unique_install_dir(id: &str, version: &str) -> Result<PathBuf, String> {
    let root = extensions_root_dir()?;
    let base_name = install_dir_name(id, version);
    let base = root.join(&base_name);
    if !base.exists() {
        return Ok(base);
    }

    Ok(root.join(format!(
        "{}-{}",
        base_name,
        sanitize_dir_fragment(&Uuid::new_v4().to_string())
    )))
}

fn parse_extension_identifier(id: &str) -> Result<(String, String), String> {
    let normalized = normalize_extension_id(id);
    let (namespace, name) = normalized
        .split_once('.')
        .ok_or_else(|| format!("invalid extension identifier: {id}"))?;
    if namespace.is_empty() || name.is_empty() {
        return Err(format!("invalid extension identifier: {id}"));
    }
    Ok((namespace.to_string(), name.to_string()))
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn current_open_vsx_target_platform() -> Option<&'static str> {
    Some("win32-x64")
}

#[cfg(all(target_os = "windows", target_arch = "aarch64"))]
fn current_open_vsx_target_platform() -> Option<&'static str> {
    Some("win32-arm64")
}

#[cfg(all(target_os = "windows", target_arch = "x86"))]
fn current_open_vsx_target_platform() -> Option<&'static str> {
    Some("win32-ia32")
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn current_open_vsx_target_platform() -> Option<&'static str> {
    Some("linux-x64")
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn current_open_vsx_target_platform() -> Option<&'static str> {
    Some("linux-arm64")
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn current_open_vsx_target_platform() -> Option<&'static str> {
    Some("darwin-x64")
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn current_open_vsx_target_platform() -> Option<&'static str> {
    Some("darwin-arm64")
}

#[cfg(not(any(
    all(
        target_os = "windows",
        any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "x86")
    ),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
)))]
fn current_open_vsx_target_platform() -> Option<&'static str> {
    None
}

fn is_open_vsx_target_platform_segment(value: &str) -> bool {
    matches!(
        value,
        "win32-x64"
            | "win32-arm64"
            | "win32-ia32"
            | "linux-x64"
            | "linux-arm64"
            | "linux-armhf"
            | "darwin-x64"
            | "darwin-arm64"
            | "alpine-x64"
            | "alpine-arm64"
    )
}

fn rewrite_open_vsx_download_url_for_current_platform(url: &str) -> String {
    let Some(target_platform) = current_open_vsx_target_platform() else {
        return normalize_string(url);
    };
    let mut parts = normalize_string(url)
        .split('/')
        .map(|item| item.to_string())
        .collect::<Vec<_>>();

    let Some(platform_index) = parts
        .iter()
        .position(|item| is_open_vsx_target_platform_segment(item))
    else {
        return parts.join("/");
    };
    let source_platform = parts[platform_index].clone();
    if source_platform == target_platform {
        return parts.join("/");
    }

    parts[platform_index] = target_platform.to_string();
    if let Some(file_name) = parts.last_mut() {
        *file_name = file_name.replace(
            &format!("@{source_platform}.vsix"),
            &format!("@{target_platform}.vsix"),
        );
    }
    let rewritten = parts.join("/");
    warn!(
        source_platform = source_platform.as_str(),
        target_platform,
        original_url = url,
        rewritten_url = rewritten.as_str(),
        "rewrote Open VSX platform-specific download URL for current runtime platform"
    );
    rewritten
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn is_lsp_extension_task_running(status: &str) -> bool {
    matches!(status, "queued" | "running")
}

fn update_lsp_extension_task(
    task_id: &str,
    status: &str,
    message: &str,
    error: Option<String>,
    state: Option<LspExtensionState>,
) {
    let mut tasks = LSP_EXTENSION_TASKS.lock();
    if let Some(task) = tasks.get_mut(task_id) {
        task.status = status.to_string();
        task.message = message.to_string();
        task.error = error;
        task.state = state;
        task.updated_at = now_rfc3339();
        info!(
            task_id = task.task_id.as_str(),
            operation = task.operation.as_str(),
            extension_id = task.extension_id.as_str(),
            status = task.status.as_str(),
            message = task.message.as_str(),
            "updated LSP extension background task"
        );
    }
}

fn node_command_program() -> String {
    slidev_node_path()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| "node".to_string())
}

fn insert_lsp_extension_task(
    operation: &str,
    extension_id: &str,
    message: &str,
) -> Result<LspExtensionTaskSnapshot, String> {
    let normalized_extension_id = normalize_extension_id(extension_id);
    if normalized_extension_id.is_empty() {
        return Err("extension_id cannot be empty".to_string());
    }

    let mut tasks = LSP_EXTENSION_TASKS.lock();
    if let Some(running) = tasks.values().find(|task| {
        normalize_extension_id(&task.extension_id) == normalized_extension_id
            && is_lsp_extension_task_running(&task.status)
    }) {
        warn!(
            operation = operation,
            extension_id = normalized_extension_id.as_str(),
            running_task_id = running.task_id.as_str(),
            running_operation = running.operation.as_str(),
            "LSP extension mutation task is already running"
        );
        return Err(format!(
            "LSP extension task already running for {}: {}",
            normalized_extension_id, running.operation
        ));
    }

    let now = now_rfc3339();
    let snapshot = LspExtensionTaskSnapshot {
        task_id: Uuid::new_v4().to_string(),
        operation: operation.to_string(),
        extension_id: normalized_extension_id,
        status: "queued".to_string(),
        message: message.to_string(),
        error: None,
        state: None,
        created_at: now.clone(),
        updated_at: now,
    };
    tasks.insert(snapshot.task_id.clone(), snapshot.clone());
    info!(
        task_id = snapshot.task_id.as_str(),
        operation = snapshot.operation.as_str(),
        extension_id = snapshot.extension_id.as_str(),
        "created LSP extension background task"
    );
    Ok(snapshot)
}

fn spawn_lsp_extension_task<F>(
    snapshot: LspExtensionTaskSnapshot,
    future: F,
) -> LspExtensionTaskSnapshot
where
    F: std::future::Future<Output = Result<LspExtensionState, String>> + Send + 'static,
{
    let task_id = snapshot.task_id.clone();
    tokio::spawn(async move {
        update_lsp_extension_task(&task_id, "running", "task is running", None, None);
        match future.await {
            Ok(state) => {
                update_lsp_extension_task(&task_id, "success", "task completed", None, Some(state));
            }
            Err(error) => {
                warn!(
                    task_id = task_id.as_str(),
                    error = error.as_str(),
                    "LSP extension background task failed"
                );
                update_lsp_extension_task(&task_id, "error", "task failed", Some(error), None);
            }
        }
    });
    snapshot
}

fn spawn_lsp_extension_void_task<F>(
    snapshot: LspExtensionTaskSnapshot,
    future: F,
) -> LspExtensionTaskSnapshot
where
    F: std::future::Future<Output = Result<(), String>> + Send + 'static,
{
    let task_id = snapshot.task_id.clone();
    tokio::spawn(async move {
        update_lsp_extension_task(&task_id, "running", "task is running", None, None);
        match future.await {
            Ok(()) => {
                update_lsp_extension_task(&task_id, "success", "task completed", None, None);
            }
            Err(error) => {
                warn!(
                    task_id = task_id.as_str(),
                    error = error.as_str(),
                    "LSP extension background task failed"
                );
                update_lsp_extension_task(&task_id, "error", "task failed", Some(error), None);
            }
        }
    });
    snapshot
}

pub fn get_lsp_extension_task(task_id: &str) -> Option<LspExtensionTaskSnapshot> {
    LSP_EXTENSION_TASKS.lock().get(task_id).cloned()
}

pub fn list_lsp_extension_tasks() -> Vec<LspExtensionTaskSnapshot> {
    let mut tasks = LSP_EXTENSION_TASKS
        .lock()
        .values()
        .cloned()
        .collect::<Vec<_>>();
    tasks.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    tasks.truncate(80);
    tasks
}

fn is_downloadable_fallback(server_id: &str) -> bool {
    matches!(
        server_id,
        "clangd"
            | "rust-analyzer"
            | "lua-language-server"
            | "texlab"
            | "marksman"
            | "mermaid-lsp"
            | "ruff"
            | "biome"
    )
}

fn catalog_by_id() -> HashMap<String, CatalogExtension> {
    LSP_EXTENSION_CATALOG
        .extensions
        .iter()
        .map(|item| (normalize_extension_id(&item.id), item.clone()))
        .collect()
}

fn read_extension_meta(dir: &Path) -> InstalledExtensionMeta {
    let meta_path = extension_meta_path(dir);
    if let Ok(content) = fs::read_to_string(&meta_path) {
        if let Ok(meta) = serde_json::from_str::<InstalledExtensionMeta>(&content) {
            return meta;
        }
    }

    InstalledExtensionMeta {
        source: "unknown".to_string(),
        installed_at: now_rfc3339(),
        download_url: None,
        open_vsx: None,
        vscode_marketplace: None,
        original_file_name: None,
    }
}

fn write_extension_meta(dir: &Path, meta: &InstalledExtensionMeta) -> Result<(), String> {
    let path = extension_meta_path(dir);
    let content = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("failed to serialize extension metadata: {e}"))?;
    fs::write(&path, content).map_err(|e| format!("failed to write extension metadata: {e}"))
}

fn parse_manifest_from_path(path: &Path) -> Option<Value> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str::<Value>(&content).ok()
}

fn is_extension_manifest(manifest: &Value) -> bool {
    manifest.get("name").and_then(Value::as_str).is_some()
        && manifest.get("publisher").and_then(Value::as_str).is_some()
}

fn find_manifest_path(root: &Path) -> Option<PathBuf> {
    for entry in WalkDir::new(root)
        .min_depth(1)
        .max_depth(4)
        .into_iter()
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name() != "package.json" {
            continue;
        }
        let path = entry.path().to_path_buf();
        let Some(manifest) = parse_manifest_from_path(&path) else {
            continue;
        };
        if is_extension_manifest(&manifest) {
            return Some(path);
        }
    }
    None
}

fn manifest_root_from_dir(root: &Path) -> Option<(PathBuf, Value)> {
    let manifest_path = find_manifest_path(root)?;
    let manifest = parse_manifest_from_path(&manifest_path)?;
    let manifest_root = manifest_path.parent()?.to_path_buf();
    Some((manifest_root, manifest))
}

fn read_string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(normalize_string)
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn extract_manifest_languages(manifest: &Value) -> Vec<String> {
    let mut result = vec![];
    if let Some(items) = manifest
        .get("contributes")
        .and_then(|item| item.get("languages"))
        .and_then(Value::as_array)
    {
        for item in items {
            if let Some(id) = item.get("id").and_then(Value::as_str) {
                let normalized = normalize_language(id);
                if !normalized.is_empty() {
                    result.push(normalized);
                }
            }
        }
    }
    result.sort();
    result.dedup();
    result
}

fn record_from_dir(dir: &Path) -> Option<InstalledExtensionRecord> {
    let meta = read_extension_meta(dir);
    let (manifest_root, manifest) = manifest_root_from_dir(dir)?;

    let publisher = normalize_string(manifest.get("publisher")?.as_str()?);
    let name = normalize_string(manifest.get("name")?.as_str()?);
    if publisher.is_empty() || name.is_empty() {
        return None;
    }

    let id = normalize_extension_id(&format!("{publisher}.{name}"));
    let version = normalize_string(
        manifest
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("0.0.0"),
    );
    let display_name = normalize_string(
        manifest
            .get("displayName")
            .and_then(Value::as_str)
            .unwrap_or(&name),
    );

    let languages = {
        let manifest_languages = extract_manifest_languages(&manifest);
        if !manifest_languages.is_empty() {
            manifest_languages
        } else {
            catalog_by_id()
                .get(&id)
                .map(|entry| {
                    entry
                        .languages
                        .iter()
                        .map(|lang| normalize_language(lang))
                        .collect()
                })
                .unwrap_or_default()
        }
    };

    Some(InstalledExtensionRecord {
        id,
        version,
        installed_at: meta.installed_at.clone(),
        display_name,
        description: manifest
            .get("description")
            .and_then(Value::as_str)
            .map(normalize_string),
        source: meta.source,
        install_dir: manifest_root,
        open_vsx: meta.open_vsx,
        vscode_marketplace: meta.vscode_marketplace,
        manifest_name: Some(name),
        manifest_publisher: Some(publisher),
        manifest_browser: manifest
            .get("browser")
            .and_then(Value::as_str)
            .map(normalize_string),
        manifest_main: manifest
            .get("main")
            .and_then(Value::as_str)
            .map(normalize_string),
        activation_events: read_string_array(manifest.get("activationEvents")),
        languages,
    })
}

fn load_installed_extensions() -> Result<Vec<InstalledExtensionRecord>, String> {
    let root = extensions_root_dir()?;
    let mut result = vec![];
    for entry in fs::read_dir(&root).map_err(|e| format!("failed to read extensions root: {e}"))? {
        let entry = entry.map_err(|e| format!("failed to read extension entry: {e}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(record) = record_from_dir(&path) {
            result.push(record);
        }
    }

    // 设计思路：
    // 1. Windows 上 LSP 子进程可能仍把旧扩展目录作为 cwd，导致删除旧版本返回 os error 32。
    // 2. 安装新版本时我们会使用唯一目录名，因此这里按 extension id 只保留 installed_at 最新的一条，
    //    避免一个被锁住的旧目录继续参与 runtime 解析或重复出现在设置页。
    result.sort_by(|a, b| {
        a.id.cmp(&b.id)
            .then(b.installed_at.cmp(&a.installed_at))
            .then(a.display_name.cmp(&b.display_name))
    });
    result.dedup_by(|a, b| normalize_extension_id(&a.id) == normalize_extension_id(&b.id));
    result.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(result)
}

fn extract_zip_to_dir(zip_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("failed to open zip archive: {e}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("failed to parse zip archive: {e}"))?;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|e| format!("failed to read zip entry: {e}"))?;

        let Some(out_path) = file.enclosed_name().map(|path| dest_dir.join(path)) else {
            continue;
        };

        if file.is_dir() {
            fs::create_dir_all(&out_path)
                .map_err(|e| format!("failed to create extracted directory: {e}"))?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create extracted parent directory: {e}"))?;
        }

        let mut out_file = fs::File::create(&out_path)
            .map_err(|e| format!("failed to create extracted file: {e}"))?;
        io::copy(&mut file, &mut out_file)
            .map_err(|e| format!("failed to write extracted file: {e}"))?;
    }

    Ok(())
}

fn try_remove_extension_dir(target: &Path, extension_id: &str, reason: &str) -> bool {
    for attempt in 1..=3 {
        match fs::remove_dir_all(target) {
            Ok(_) => {
                info!(
                    extension_id = extension_id,
                    path = target.to_string_lossy().as_ref(),
                    attempt = attempt,
                    reason = reason,
                    "removed LSP extension directory"
                );
                return true;
            }
            Err(error) => {
                warn!(
                    extension_id = extension_id,
                    path = target.to_string_lossy().as_ref(),
                    attempt = attempt,
                    reason = reason,
                    error = ?error,
                    "failed to remove LSP extension directory, will retry or defer cleanup"
                );
                std::thread::sleep(Duration::from_millis(120 * attempt));
            }
        }
    }

    if let Some(parent) = target.parent() {
        let pending_name = format!(
            ".delete-pending-{}-{}",
            sanitize_dir_fragment(&normalize_extension_id(extension_id)),
            sanitize_dir_fragment(&Uuid::new_v4().to_string())
        );
        let pending_path = parent.join(pending_name);
        match fs::rename(target, &pending_path) {
            Ok(_) => {
                warn!(
                    extension_id = extension_id,
                    from = target.to_string_lossy().as_ref(),
                    to = pending_path.to_string_lossy().as_ref(),
                    reason = reason,
                    "renamed locked LSP extension directory for deferred cleanup"
                );
                let _ = fs::remove_dir_all(&pending_path);
                return true;
            }
            Err(error) => {
                warn!(
                    extension_id = extension_id,
                    path = target.to_string_lossy().as_ref(),
                    pending_path = pending_path.to_string_lossy().as_ref(),
                    reason = reason,
                    error = ?error,
                    "locked LSP extension directory could not be renamed; installation will continue with a unique target directory"
                );
            }
        }
    }

    false
}

fn remove_existing_extension_versions(extension_id: &str) -> Result<(), String> {
    let root = extensions_root_dir()?;
    let installed = load_installed_extensions()?;
    for item in installed {
        if normalize_extension_id(&item.id) != normalize_extension_id(extension_id) {
            continue;
        }
        let target = item.install_dir;
        if target.exists() {
            info!(
                extension_id = extension_id,
                path = target.to_string_lossy().as_ref(),
                "removing previous installed extension version"
            );
            let _ = try_remove_extension_dir(&target, extension_id, "replace-installed-version");
        }
    }

    for entry in fs::read_dir(&root).map_err(|e| format!("failed to read extensions root: {e}"))? {
        let entry = entry.map_err(|e| format!("failed to read extension directory entry: {e}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|item| item.to_str()) {
            if name.starts_with(&format!("{}-", sanitize_dir_fragment(extension_id))) {
                let _ = try_remove_extension_dir(&path, extension_id, "cleanup-stale-version");
            }
        }
    }

    Ok(())
}

fn copy_dir_contents(src_dir: &Path, dest_dir: &Path) -> Result<(), String> {
    if dest_dir.exists() {
        fs::remove_dir_all(dest_dir)
            .map_err(|e| format!("failed to reset destination directory: {e}"))?;
    }
    fs::create_dir_all(dest_dir)
        .map_err(|e| format!("failed to create destination directory: {e}"))?;

    let mut options = CopyOptions::new();
    options.copy_inside = true;
    options.overwrite = true;
    options.content_only = true;

    copy_dir(src_dir, dest_dir, &options)
        .map_err(|e| format!("failed to copy extension directory: {e}"))?;
    Ok(())
}

fn build_path_env_with_roots(roots: &[PathBuf]) -> Vec<(String, String)> {
    let mut all_paths = vec![];
    for root in roots {
        all_paths.push(root.clone());
    }
    if let Some(old) = std::env::var_os("PATH") {
        all_paths.extend(std::env::split_paths(&old));
    }

    let joined = std::env::join_paths(all_paths)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    vec![("PATH".to_string(), joined)]
}

fn resolve_extension_bin(dir: &Path, bin_name: &str) -> Option<PathBuf> {
    if bin_name.trim().is_empty() {
        return None;
    }

    let bin_dir = dir.join("node_modules").join(".bin");
    #[cfg(target_os = "windows")]
    {
        let cmd = bin_dir.join(format!("{bin_name}.cmd"));
        if cmd.is_file() {
            return Some(cmd);
        }
        let exe = bin_dir.join(format!("{bin_name}.exe"));
        if exe.is_file() {
            return Some(exe);
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let direct = bin_dir.join(bin_name);
        if direct.is_file() {
            return Some(direct);
        }
    }

    None
}

fn node_package_candidates_for_bin(bin_name: &str) -> Vec<String> {
    let normalized = bin_name.trim();
    let mut candidates = node_package_name_for_bin(normalized)
        .map(|item| vec![item])
        .unwrap_or_default();
    if normalized == "pyright-langserver"
        && !candidates.iter().any(|item| item == "basedpyright")
    {
        // 设计说明：打包环境里可能预置 basedpyright，它兼容提供 pyright-langserver bin。
        // 把它作为只针对 Pyright 的候选，不影响其他 LSP 包名的精确解析。
        candidates.push("basedpyright".to_string());
    }
    if !normalized.is_empty() && !candidates.iter().any(|item| item == normalized) {
        candidates.push(normalized.to_string());
    }
    candidates
}

fn node_package_name_for_bin(bin_name: &str) -> Option<String> {
    let normalized = bin_name.trim();

    // 设计思路：
    // 1. npm 包名和 language server 可执行名并不总是相同，例如 pyright-langserver 来自 pyright。
    // 2. 但 typescript-language-server、bash-language-server 的包名本身就等于 bin 名，不能再把“相等”当成无效映射过滤掉。
    // 3. 这里保留显式白名单，避免未知 bin 名被盲目 npm install；缺映射时仍然返回清晰错误，方便继续补 catalog。
    match normalized {
        "pyright-langserver" => Some("pyright".to_string()),
        "bash-language-server" => Some("bash-language-server".to_string()),
        "typescript-language-server" => Some("typescript-language-server".to_string()),
        "vscode-html-language-server" => Some("vscode-langservers-extracted".to_string()),
        "vscode-css-language-server" => Some("vscode-langservers-extracted".to_string()),
        "vscode-json-language-server" => Some("vscode-langservers-extracted".to_string()),
        _ => None,
    }
}

fn node_package_install_specs_for_bin(bin_name: &str) -> Vec<String> {
    match bin_name.trim() {
        "typescript-language-server" => vec![
            "typescript-language-server@latest".to_string(),
            "typescript@latest".to_string(),
        ],
        "vscode-html-language-server"
        | "vscode-css-language-server"
        | "vscode-json-language-server" => {
            vec!["vscode-langservers-extracted@latest".to_string()]
        }
        _ => node_package_name_for_bin(bin_name)
            .map(|name| vec![format!("{name}@latest")])
            .unwrap_or_default(),
    }
}

#[cfg(target_os = "windows")]
fn npm_command_name() -> &'static str {
    "npm.cmd"
}

#[cfg(not(target_os = "windows"))]
fn npm_command_name() -> &'static str {
    "npm"
}

fn apply_proxy_env_pair_to_child(cmd: &mut Command, upper: &str, lower: &str) -> bool {
    let upper_value = std::env::var(upper)
        .ok()
        .filter(|value| !value.trim().is_empty());
    let lower_value = std::env::var(lower)
        .ok()
        .filter(|value| !value.trim().is_empty());
    match (upper_value, lower_value) {
        (Some(value), None) => {
            cmd.env(lower, &value);
            true
        }
        (None, Some(value)) => {
            cmd.env(upper, &value);
            true
        }
        (Some(_), Some(_)) => true,
        (None, None) => false,
    }
}

fn apply_proxy_env_to_npm(cmd: &mut Command) {
    let http_proxy = apply_proxy_env_pair_to_child(cmd, "HTTP_PROXY", "http_proxy");
    let https_proxy = apply_proxy_env_pair_to_child(cmd, "HTTPS_PROXY", "https_proxy");
    let all_proxy = apply_proxy_env_pair_to_child(cmd, "ALL_PROXY", "all_proxy");
    let no_proxy = apply_proxy_env_pair_to_child(cmd, "NO_PROXY", "no_proxy");

    // 设计思路：
    // 1. npm 会继承父进程环境变量，但不同代理工具可能只写入大写或小写变量。
    // 2. 这里不记录代理 URL，避免日志泄露账号密码，只记录是否存在代理配置。
    // 3. 非中国大陆网络或用户开启系统代理时，只要代理工具同步了环境变量，runtime 安装就能沿用这条通道。
    info!(
        http_proxy,
        https_proxy, all_proxy, no_proxy, "prepared proxy environment for npm LSP runtime install"
    );
}

fn resolve_node_module_bin_entry(dir: &Path, bin_name: &str) -> Option<PathBuf> {
    if bin_name.trim().is_empty() {
        return None;
    }

    for package_name in node_package_candidates_for_bin(bin_name) {
        let package_dir = dir.join("node_modules").join(&package_name);
        let manifest_path = package_dir.join("package.json");
        let Some(manifest) = parse_manifest_from_path(&manifest_path) else {
            continue;
        };
        let Some(bin) = manifest.get("bin") else {
            continue;
        };
        let Some(relative) = (match bin {
            Value::String(value) => Some(value.as_str()),
            Value::Object(map) => map
                .get(bin_name)
                .or_else(|| map.values().next())
                .and_then(Value::as_str),
            _ => None,
        }) else {
            continue;
        };
        let entry = package_dir.join(relative);
        if entry.is_file() {
            return Some(entry);
        }
    }

    None
}

fn resolve_node_binary_entry(dir: &Path, bin_name: &str) -> Option<PathBuf> {
    resolve_node_module_bin_entry(dir, bin_name)
}

fn resolve_node_binary_command_from_entry(
    entry_path: &Path,
    args: &[String],
    cwd: &Path,
    env_roots: &[PathBuf],
    extension_id: &str,
    extension_display_name: &str,
    server_id: &str,
) -> ResolvedLspExtensionCommand {
    let mut command_args = vec![entry_path.to_string_lossy().to_string()];
    command_args.extend(args.iter().cloned());
    ResolvedLspExtensionCommand {
        program: node_command_program(),
        args: command_args,
        envs: build_path_env_with_roots(env_roots),
        cwd: Some(cwd.to_string_lossy().to_string()),
        extension_id: extension_id.to_string(),
        extension_display_name: extension_display_name.to_string(),
        server_id: server_id.to_string(),
    }
}

fn extension_bin_candidate_paths(dir: &Path, bin_name: &str) -> Vec<String> {
    if bin_name.trim().is_empty() {
        return vec![];
    }

    let bin_dir = dir.join("node_modules").join(".bin");
    let mut candidates = vec![];
    #[cfg(target_os = "windows")]
    {
        candidates.push(bin_dir.join(format!("{bin_name}.cmd")));
        candidates.push(bin_dir.join(format!("{bin_name}.exe")));
    }
    candidates.push(bin_dir.join(bin_name));
    for package_name in node_package_candidates_for_bin(bin_name) {
        candidates.push(
            dir.join("node_modules")
                .join(package_name)
                .join("package.json"),
        );
    }
    candidates
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect()
}

fn shared_node_runtime_candidate_paths(bin_name: &str) -> Vec<String> {
    match shared_node_runtime_dir() {
        Ok(dir) => extension_bin_candidate_paths(&dir, bin_name),
        Err(error) => vec![format!("shared node runtime unavailable: {error}")],
    }
}

fn resolve_relative_entry(dir: &Path, entry_paths: &[String]) -> Option<PathBuf> {
    entry_paths
        .iter()
        .map(|item| dir.join(item))
        .find(|path| is_supported_runtime_entry_path(path))
}

fn relative_entry_candidate_paths(dir: &Path, entry_paths: &[String]) -> Vec<String> {
    entry_paths
        .iter()
        .map(|item| dir.join(item).to_string_lossy().to_string())
        .collect()
}

fn is_node_runtime_entry_path(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|item| item.to_str())
        .map(|item| item.to_ascii_lowercase())
        .unwrap_or_default();
    matches!(extension.as_str(), "js" | "cjs" | "mjs")
}

fn is_supported_runtime_entry_path(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    if is_node_runtime_entry_path(path) {
        return true;
    }

    #[cfg(target_os = "windows")]
    {
        let extension = path
            .extension()
            .and_then(|item| item.to_str())
            .map(|item| item.to_ascii_lowercase())
            .unwrap_or_default();
        matches!(extension.as_str(), "exe" | "cmd" | "bat")
    }

    #[cfg(not(target_os = "windows"))]
    {
        true
    }
}

fn shared_node_runtime_marker_path(runtime_dir: &Path, bin_name: &str) -> PathBuf {
    runtime_dir.join(format!(
        ".guaxi-node-runtime-ready-{}.json",
        sanitize_dir_fragment(bin_name)
    ))
}

fn shared_node_runtime_marker_matches(
    runtime_dir: &Path,
    bin_name: &str,
    package_specs: &[String],
) -> bool {
    let marker_path = shared_node_runtime_marker_path(runtime_dir, bin_name);
    let Ok(content) = fs::read_to_string(&marker_path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(&content) else {
        return false;
    };
    let marker_bin = value
        .get("bin_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let marker_packages = value
        .get("packages")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|item| item.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    marker_bin == bin_name && marker_packages == package_specs
}

fn write_shared_node_runtime_marker(
    runtime_dir: &Path,
    catalog: &CatalogExtension,
    source_label: &str,
    package_specs: &[String],
) -> Result<(), String> {
    let marker = json!({
        "extension_id": catalog.id.as_str(),
        "source": source_label,
        "bin_name": catalog.lsp.bin_name.as_str(),
        "packages": package_specs,
        "prepared_at": now_rfc3339(),
    });
    fs::write(
        shared_node_runtime_marker_path(runtime_dir, &catalog.lsp.bin_name),
        serde_json::to_string_pretty(&marker)
            .map_err(|e| format!("failed to serialize shared Node runtime marker: {e}"))?,
    )
    .map_err(|e| format!("failed to write shared Node runtime marker: {e}"))
}

fn npm_output_indicates_runtime_tree_corruption(output: &str) -> bool {
    let normalized = output.to_ascii_uppercase();
    normalized.contains("TAR_ENTRY_ERROR")
        || normalized.contains("ENOTEMPTY")
        || normalized.contains("ENOENT")
        || normalized.contains("MODULE_NOT_FOUND")
}

fn reset_shared_node_runtime_tree(runtime_dir: &Path, reason: &str) -> Result<(), String> {
    info!(
        runtime_dir = runtime_dir.to_string_lossy().as_ref(),
        reason = reason,
        "resetting shared Node LSP runtime cache before npm repair"
    );

    // 设计说明：
    // 1. 共享 Node runtime 是自动生成缓存，不是用户源码；并发 npm install 失败后会留下半安装 node_modules。
    // 2. 遇到 ENOENT/ENOTEMPTY/TAR_ENTRY_ERROR 这类目录树损坏信号时，继续增量安装通常只会反复失败。
    // 3. 这里只清理 runtime_dir 下的 npm 产物和 Guaxi 标记文件，并保留目录本身，降低误删范围。
    for relative in [
        "node_modules",
        "package-lock.json",
        "package.json",
        ".guaxi-node-runtime-ready.json",
    ] {
        let target = runtime_dir.join(relative);
        if target.is_dir() {
            fs::remove_dir_all(&target).map_err(|e| {
                format!(
                    "failed to remove shared Node runtime dir {}: {e}",
                    target.to_string_lossy()
                )
            })?;
        } else if target.is_file() {
            fs::remove_file(&target).map_err(|e| {
                format!(
                    "failed to remove shared Node runtime file {}: {e}",
                    target.to_string_lossy()
                )
            })?;
        }
    }
    for entry in fs::read_dir(runtime_dir)
        .map_err(|e| {
            format!(
                "failed to list shared Node runtime dir {}: {e}",
                runtime_dir.to_string_lossy()
            )
        })?
        .flatten()
    {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|item| item.to_str()) else {
            continue;
        };
        if path.is_file()
            && name.starts_with(".guaxi-node-runtime-ready-")
            && name.ends_with(".json")
        {
            fs::remove_file(&path).map_err(|e| {
                format!(
                    "failed to remove shared Node runtime marker {}: {e}",
                    path.to_string_lossy()
                )
            })?;
        }
    }
    Ok(())
}

async fn run_shared_node_runtime_npm_install(
    catalog: &CatalogExtension,
    source_label: &str,
    runtime_dir: &Path,
    package_specs: &[String],
) -> Result<(), String> {
    let npm = npm_command_name();
    let mut npm_install = Command::new(npm);
    apply_proxy_env_to_npm(&mut npm_install);
    let output = npm_install
        .arg("install")
        .arg("--prefix")
        .arg(runtime_dir)
        .arg("--no-save")
        .arg("--no-audit")
        .arg("--no-fund")
        .arg("--fetch-retries")
        .arg("5")
        .arg("--fetch-retry-mintimeout")
        .arg("10000")
        .arg("--fetch-retry-maxtimeout")
        .arg("120000")
        .args(package_specs)
        .output()
        .await
        .map_err(|e| {
            format!(
                "failed to start npm runtime install for {}: {}",
                catalog.id, e
            )
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    warn!(
        extension_id = catalog.id.as_str(),
        source = source_label,
        status = ?output.status,
        stdout = stdout.as_str(),
        stderr = stderr.as_str(),
        "failed to prepare shared LSP node runtime"
    );
    Err(format!(
        "failed to install shared LSP runtime packages {}: {}{}{}",
        package_specs.join(", "),
        stderr,
        if stderr.is_empty() || stdout.is_empty() {
            ""
        } else {
            " | "
        },
        stdout
    ))
}

async fn prepare_shared_node_runtime(
    catalog: &CatalogExtension,
    source_label: &str,
) -> Result<(), String> {
    if normalize_string(&catalog.lsp.runtime) != "node_binary" {
        return Err(format!(
            "{} runtime for {} is not a node_binary runtime and cannot be prepared automatically",
            source_label, catalog.id
        ));
    }
    let runtime_dir = shared_node_runtime_dir()?;
    let package_specs = node_package_install_specs_for_bin(&catalog.lsp.bin_name);
    if package_specs.is_empty() {
        return Err(format!(
            "no npm package mapping for LSP bin {}",
            catalog.lsp.bin_name
        ));
    }
    let runtime_entry_exists = || {
        resolve_extension_bin(&runtime_dir, &catalog.lsp.bin_name).is_some()
            || resolve_node_module_bin_entry(&runtime_dir, &catalog.lsp.bin_name).is_some()
    };
    if runtime_entry_exists()
        && shared_node_runtime_marker_matches(&runtime_dir, &catalog.lsp.bin_name, &package_specs)
    {
        info!(
            extension_id = catalog.id.as_str(),
            source = source_label,
            runtime_dir = runtime_dir.to_string_lossy().as_ref(),
            bin_name = catalog.lsp.bin_name.as_str(),
            "shared LSP node runtime already prepared"
        );
        return Ok(());
    }
    info!(
        extension_id = catalog.id.as_str(),
        source = source_label,
        runtime_dir = runtime_dir.to_string_lossy().as_ref(),
        packages = ?package_specs,
        bin_name = catalog.lsp.bin_name.as_str(),
        "preparing missing shared LSP node runtime"
    );

    // 设计说明：
    // 1. 插件安装完成后必须立刻补齐 runtime，不能把问题留给运行时再暴露。
    // 2. Node LSP 包统一安装到 <软件目录>/LspRuntimes/node，多个语言插件共用一份 node_modules，避免每个扩展目录重复占用磁盘。
    // 3. 这里只写共享 runtime 目录，不修改 SlidevEnv；安装后会再次解析并执行 initialize 探测。
    // 设计说明：
    // 1. 多个 LSP 扩展共用同一个 LspRuntimes/node/node_modules，npm install 必须串行执行。
    // 2. 进入锁后再次检查 marker，避免排队期间前一个任务已经完成安装，后一个任务重复写目录。
    // 3. 老版本并发安装可能留下半安装依赖；遇到 npm 文件树错误时清理缓存并重试一次。
    info!(
        extension_id = catalog.id.as_str(),
        source = source_label,
        "waiting for shared LSP node runtime install lock"
    );
    let _install_guard = LSP_SHARED_NODE_RUNTIME_INSTALL_LOCK.lock().await;
    if runtime_entry_exists()
        && shared_node_runtime_marker_matches(&runtime_dir, &catalog.lsp.bin_name, &package_specs)
    {
        info!(
            extension_id = catalog.id.as_str(),
            source = source_label,
            "shared LSP node runtime became ready while waiting for install lock"
        );
        return Ok(());
    }

    let first_install =
        run_shared_node_runtime_npm_install(catalog, source_label, &runtime_dir, &package_specs)
            .await;
    if let Err(error) = first_install {
        if npm_output_indicates_runtime_tree_corruption(&error) {
            warn!(
                extension_id = catalog.id.as_str(),
                source = source_label,
                error = error.as_str(),
                "shared LSP node runtime install looks corrupted; resetting cache and retrying once"
            );
            reset_shared_node_runtime_tree(&runtime_dir, "npm-tree-corruption")?;
            run_shared_node_runtime_npm_install(
                catalog,
                source_label,
                &runtime_dir,
                &package_specs,
            )
            .await?;
        } else {
            return Err(error);
        }
    }

    if resolve_extension_bin(&runtime_dir, &catalog.lsp.bin_name).is_none()
        && resolve_node_module_bin_entry(&runtime_dir, &catalog.lsp.bin_name).is_none()
    {
        return Err(format!(
            "runtime packages {} installed but bin {} was not found in {}",
            package_specs.join(", "),
            catalog.lsp.bin_name,
            runtime_dir.to_string_lossy()
        ));
    }
    write_shared_node_runtime_marker(&runtime_dir, catalog, source_label, &package_specs)?;

    info!(
        extension_id = catalog.id.as_str(),
        source = source_label,
        runtime_dir = runtime_dir.to_string_lossy().as_ref(),
        packages = ?package_specs,
        "prepared shared LSP node runtime"
    );
    Ok(())
}

fn resolve_runtime_status(
    record: Option<&InstalledExtensionRecord>,
    catalog: Option<&CatalogExtension>,
) -> LspExtensionRuntimeStatus {
    let Some(catalog) = catalog else {
        return LspExtensionRuntimeStatus {
            ready: false,
            runtime: "unknown".to_string(),
            program_path: None,
            message: Some(
                "extension is installed but not covered by the Guaxi LSP compatibility catalog"
                    .to_string(),
            ),
        };
    };

    let runtime = normalize_string(&catalog.lsp.runtime);

    let Some(record) = record else {
        return LspExtensionRuntimeStatus {
            ready: false,
            runtime,
            program_path: None,
            message: Some("extension not installed yet".to_string()),
        };
    };

    let install_dir = &record.install_dir;
    for item in &catalog.lsp.resolve_order {
        match item.as_str() {
            "extension_bin" => {
                if runtime == "node_binary" {
                    // 设计说明：状态页和真实启动路径必须一致；Node LSP 优先展示 JS bin 入口，
                    // 避免用户看到 .cmd ready，但实际探针改走 node entry 时难以对应日志。
                    if let Some(entry_path) =
                        resolve_node_binary_entry(install_dir, &catalog.lsp.bin_name)
                    {
                        return LspExtensionRuntimeStatus {
                            ready: true,
                            runtime: runtime.clone(),
                            program_path: Some(node_command_program()),
                            message: Some(format!(
                                "resolved from extension node package entry: {}",
                                entry_path.to_string_lossy()
                            )),
                        };
                    }
                }
                if let Some(path) = resolve_extension_bin(install_dir, &catalog.lsp.bin_name) {
                    return LspExtensionRuntimeStatus {
                        ready: true,
                        runtime: runtime.clone(),
                        program_path: Some(path.to_string_lossy().to_string()),
                        message: Some("resolved from extension packaged node binary".to_string()),
                    };
                }
            }
            "extension_entry" => {
                if let Some(path) = resolve_relative_entry(install_dir, &catalog.lsp.entry_paths) {
                    if is_node_runtime_entry_path(&path) {
                        return LspExtensionRuntimeStatus {
                            ready: true,
                            runtime: runtime.clone(),
                            program_path: Some(node_command_program()),
                            message: Some(format!(
                                "resolved from extension bundled Node entry: {}",
                                path.to_string_lossy()
                            )),
                        };
                    }
                    return LspExtensionRuntimeStatus {
                        ready: true,
                        runtime: runtime.clone(),
                        program_path: Some(path.to_string_lossy().to_string()),
                        message: Some("resolved from extension bundled entry".to_string()),
                    };
                }
            }
            "shared_node_runtime" => {
                if runtime == "node_binary" {
                    if let Ok(root) = shared_node_runtime_dir() {
                        // 设计说明：共享 runtime 也优先报告 package.json bin 入口，和安装验证/编辑器启动保持同源。
                        if let Some(entry_path) =
                            resolve_node_binary_entry(&root, &catalog.lsp.bin_name)
                        {
                            return LspExtensionRuntimeStatus {
                                ready: true,
                                runtime: runtime.clone(),
                                program_path: Some(node_command_program()),
                                message: Some(format!(
                                    "resolved from shared Node LSP runtime package entry: {}",
                                    entry_path.to_string_lossy()
                                )),
                            };
                        }
                        if let Some(path) = resolve_extension_bin(&root, &catalog.lsp.bin_name) {
                            return LspExtensionRuntimeStatus {
                                ready: true,
                                runtime: runtime.clone(),
                                program_path: Some(path.to_string_lossy().to_string()),
                                message: Some("resolved from shared Node LSP runtime".to_string()),
                            };
                        }
                    }
                }
            }
            "slidev_env_bin" => {
                if let Some(root) = slidev_env_root().ok() {
                    if runtime == "node_binary" {
                        // 设计说明：内置 SlidevEnv 中的 Node 包同样绕过 .cmd shim，减少 Windows stdio 卡死点。
                        if let (Some(node_path), Some(entry_path)) = (
                            slidev_node_path(),
                            resolve_node_binary_entry(&root, &catalog.lsp.bin_name),
                        ) {
                            return LspExtensionRuntimeStatus {
                                ready: true,
                                runtime: runtime.clone(),
                                program_path: Some(node_path.to_string_lossy().to_string()),
                                message: Some(format!(
                                    "resolved from bundled SlidevEnv node package entry: {}",
                                    entry_path.to_string_lossy()
                                )),
                            };
                        }
                    }
                    if let Some(path) = resolve_extension_bin(&root, &catalog.lsp.bin_name) {
                        return LspExtensionRuntimeStatus {
                            ready: true,
                            runtime: runtime.clone(),
                            program_path: Some(path.to_string_lossy().to_string()),
                            message: Some(
                                "resolved from bundled SlidevEnv node runtime".to_string(),
                            ),
                        };
                    }
                }
            }
            "downloadable_server" => {
                if let Some(server_id) = catalog.lsp.fallback_server.as_deref() {
                    if is_downloadable_fallback(server_id) {
                        return LspExtensionRuntimeStatus {
                            ready: true,
                            runtime: runtime.clone(),
                            program_path: None,
                            message: Some(
                                "server runtime can be auto-downloaded on first use".to_string(),
                            ),
                        };
                    }
                }
            }
            _ => {}
        }
    }

    LspExtensionRuntimeStatus {
        ready: false,
        runtime,
        program_path: None,
        message: Some("runtime entry could not be resolved from extension, shared runtime, SlidevEnv, or fallback server".to_string()),
    }
}

fn build_extension_state(
    record: Option<InstalledExtensionRecord>,
    catalog: Option<CatalogExtension>,
) -> LspExtensionState {
    let runtime_status = resolve_runtime_status(record.as_ref(), catalog.as_ref());
    let id = record
        .as_ref()
        .map(|item| item.id.clone())
        .or_else(|| {
            catalog
                .as_ref()
                .map(|item| normalize_extension_id(&item.id))
        })
        .unwrap_or_default();
    let display_name = record
        .as_ref()
        .map(|item| item.display_name.clone())
        .or_else(|| catalog.as_ref().map(|item| item.display_name.clone()))
        .unwrap_or_else(|| id.clone());
    let description = record
        .as_ref()
        .and_then(|item| item.description.clone())
        .or_else(|| catalog.as_ref().and_then(|item| item.description.clone()));
    let languages = if let Some(record) = &record {
        record.languages.clone()
    } else {
        catalog
            .as_ref()
            .map(|item| {
                item.languages
                    .iter()
                    .map(|lang| normalize_language(lang))
                    .collect()
            })
            .unwrap_or_default()
    };
    let compatibility_level = catalog
        .as_ref()
        .map(|item| item.compatibility_level.clone())
        .unwrap_or_else(|| "C".to_string());
    let recommended = catalog
        .as_ref()
        .map(|item| item.recommended)
        .unwrap_or(false);
    let open_vsx = record
        .as_ref()
        .and_then(|item| item.open_vsx.clone())
        .or_else(|| catalog.as_ref().and_then(|item| item.open_vsx.clone()));
    let vscode_marketplace = record
        .as_ref()
        .and_then(|item| item.vscode_marketplace.clone())
        .or_else(|| {
            catalog
                .as_ref()
                .and_then(|item| item.vscode_marketplace.clone())
        });

    LspExtensionState {
        id,
        display_name,
        description,
        source: record
            .as_ref()
            .map(|item| item.source.clone())
            .unwrap_or_else(|| "catalog".to_string()),
        version: record
            .as_ref()
            .map(|item| item.version.clone())
            .unwrap_or_else(|| "not-installed".to_string()),
        languages,
        installed: record.is_some(),
        installed_path: record
            .as_ref()
            .map(|item| item.install_dir.to_string_lossy().to_string()),
        compatibility_level,
        recommended,
        open_vsx,
        vscode_marketplace,
        manifest_name: record.as_ref().and_then(|item| item.manifest_name.clone()),
        manifest_publisher: record
            .as_ref()
            .and_then(|item| item.manifest_publisher.clone()),
        manifest_browser: record
            .as_ref()
            .and_then(|item| item.manifest_browser.clone()),
        manifest_main: record.as_ref().and_then(|item| item.manifest_main.clone()),
        activation_events: record
            .as_ref()
            .map(|item| item.activation_events.clone())
            .unwrap_or_default(),
        runtime_status,
    }
}

pub fn list_lsp_extensions() -> Result<Vec<LspExtensionState>, String> {
    info!(
        catalog_version = LSP_EXTENSION_CATALOG.version,
        "listing LSP extension registry state"
    );

    let installed = load_installed_extensions()?;
    let installed_by_id = installed
        .into_iter()
        .map(|item| (normalize_extension_id(&item.id), item))
        .collect::<HashMap<_, _>>();
    let catalog_by_id = catalog_by_id();

    let mut ids = HashSet::new();
    ids.extend(catalog_by_id.keys().cloned());
    ids.extend(installed_by_id.keys().cloned());

    let mut result = ids
        .into_iter()
        .map(|id| {
            build_extension_state(
                installed_by_id.get(&id).cloned(),
                catalog_by_id.get(&id).cloned(),
            )
        })
        .collect::<Vec<_>>();

    result.sort_by(|a, b| {
        a.installed
            .cmp(&b.installed)
            .reverse()
            .then(a.display_name.cmp(&b.display_name))
    });
    Ok(result)
}

async fn download_to_file(url: &str, destination: &Path) -> Result<(), String> {
    let client = lsp_extension_http_client("download-extension-archive")?;
    let response = client
        .get(url)
        .header("User-Agent", "guaxi-lsp-extension")
        .send()
        .await
        .map_err(|e| format!("failed to download extension archive: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "failed to download extension archive: HTTP {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("failed to read extension archive response: {e}"))?;

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create download parent directory: {e}"))?;
    }

    fs::write(destination, &bytes).map_err(|e| format!("failed to save extension archive: {e}"))
}

async fn finalize_install_from_unpacked_root(
    unpacked_root: &Path,
    meta: InstalledExtensionMeta,
    expected_extension_id: Option<&str>,
) -> Result<LspExtensionState, String> {
    let (manifest_root, manifest) = manifest_root_from_dir(unpacked_root).ok_or_else(|| {
        "failed to locate extension package.json in unpacked extension".to_string()
    })?;

    let publisher = normalize_string(
        manifest
            .get("publisher")
            .and_then(Value::as_str)
            .ok_or_else(|| "extension manifest is missing publisher".to_string())?,
    );
    let name = normalize_string(
        manifest
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| "extension manifest is missing name".to_string())?,
    );
    let version = normalize_string(
        manifest
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("0.0.0"),
    );
    let id = normalize_extension_id(&format!("{publisher}.{name}"));
    let expected_id = expected_extension_id
        .map(normalize_extension_id)
        .filter(|item| !item.is_empty());
    if let Some(expected_id) = expected_id.as_deref() {
        if id != expected_id {
            warn!(
                expected_extension_id = expected_id,
                manifest_extension_id = id.as_str(),
                "extension archive manifest id mismatch"
            );
            return Err(format!(
                "扩展包内容不匹配：请求安装 {}，但 VSIX manifest 是 {}。已阻止安装错误扩展。",
                expected_id, id
            ));
        }
    }

    // 设计说明：
    // 1. Windows 会锁住正在运行的 language server 所在目录，直接覆盖或删除扩展容易遇到 os error 32。
    // 2. 安装新版本前先按扩展 id 停掉活跃 LSP 子进程，再做目录替换，避免下载任务和旧进程互相打断。
    // 3. 这里写入日志记录被停止的进程数量，后续排查“为什么卸载/覆盖慢”时能定位到 LSP runtime 占用。
    let stopped = lsp::stop_lsp_processes_for_extension(&id).await;
    info!(
        extension_id = id.as_str(),
        stopped_process_count = stopped,
        "stopped active LSP processes before finalizing extension install"
    );
    remove_existing_extension_versions(&id)?;

    let install_dir = unique_install_dir(&id, &version)?;
    copy_dir_contents(&manifest_root, &install_dir)?;
    write_extension_meta(&install_dir, &meta)?;

    info!(
        extension_id = id.as_str(),
        version = version.as_str(),
        install_dir = install_dir.to_string_lossy().as_ref(),
        source = meta.source.as_str(),
        "extension installation finalized"
    );

    let record = record_from_dir(&install_dir)
        .ok_or_else(|| "failed to rebuild installed extension record after install".to_string())?;
    Ok(build_extension_state(
        Some(record),
        catalog_by_id().get(&id).cloned(),
    ))
}

fn write_catalog_extension_package(
    dest_dir: &Path,
    catalog: &CatalogExtension,
) -> Result<(), String> {
    let (publisher, name) = parse_extension_identifier(&catalog.id)?;
    fs::create_dir_all(dest_dir)
        .map_err(|e| format!("failed to create catalog extension dir: {e}"))?;

    let manifest = json!({
        "publisher": publisher,
        "name": name,
        "displayName": catalog.display_name,
        "description": catalog.description,
        "version": format!("catalog.{}", LSP_EXTENSION_CATALOG.version),
        "activationEvents": catalog.languages.iter().map(|lang| format!("onLanguage:{lang}")).collect::<Vec<_>>(),
        "contributes": {
            "languages": catalog.languages.iter().map(|lang| json!({ "id": lang })).collect::<Vec<_>>()
        },
        "guaxi": {
            "catalogExtension": true,
            "serverId": catalog.lsp.server_id,
            "runtime": catalog.lsp.runtime,
            "fallbackServer": catalog.lsp.fallback_server,
        }
    });
    let manifest_content = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("failed to serialize catalog extension manifest: {e}"))?;
    fs::write(dest_dir.join("package.json"), manifest_content)
        .map_err(|e| format!("failed to write catalog extension manifest: {e}"))?;

    let mut readme = fs::File::create(dest_dir.join("README.md"))
        .map_err(|e| format!("failed to create catalog extension readme: {e}"))?;
    writeln!(
        readme,
        "# {}\n\nThis is a Guaxi managed LSP catalog extension. It provides a stable extension registry entry and lets Guaxi resolve the configured language server runtime.",
        catalog.display_name
    )
    .map_err(|e| format!("failed to write catalog extension readme: {e}"))?;
    Ok(())
}

async fn verify_catalog_extension_runtime(
    state: &LspExtensionState,
    source_label: &str,
) -> Result<Vec<lsp::LspRuntimeProbeResult>, String> {
    if !state.installed {
        return Err(format!(
            "{} did not install extension {}",
            source_label, state.id
        ));
    }

    let languages = state
        .languages
        .iter()
        .map(|item| normalize_language(item))
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    if languages.is_empty() {
        return Err(format!(
            "{} installed extension {} but no languages were declared",
            source_label, state.id
        ));
    }

    let mut probes = vec![];
    let mut errors = vec![];
    for language in languages {
        match lsp::probe_lsp_runtime_for_language(&language, None).await {
            Ok(probe) => {
                if !probe.completion_provider {
                    errors.push(format!(
                        "{}: initialize succeeded but completionProvider is missing",
                        language
                    ));
                    continue;
                }
                if probe.completion_item_count == 0 {
                    // 设计说明：
                    // 1. LSP 允许 completion 返回 null 或空列表，这只表示当前探针文本没有候选，不代表 server 不可用。
                    // 2. rust-analyzer、Marksman、TexLab、bash-language-server 在最小临时文档里经常返回空结果。
                    // 3. 安装验证只要求 initialize 成功且声明 completionProvider；候选数量保留在日志中用于后续调试。
                    info!(
                        extension_id = state.id.as_str(),
                        source = source_label,
                        language = language.as_str(),
                        "catalog LSP extension completion probe returned zero items; accepting initialized runtime"
                    );
                }
                info!(
                    extension_id = state.id.as_str(),
                    source = source_label,
                    language = language.as_str(),
                    completion_provider = probe.completion_provider,
                    completion_item_count = probe.completion_item_count,
                    completion_first_label = probe.completion_first_label.as_deref().unwrap_or(""),
                    hover_provider = probe.hover_provider,
                    definition_provider = probe.definition_provider,
                    semantic_tokens_provider = probe.semantic_tokens_provider,
                    semantic_tokens_request_ok = probe.semantic_tokens_request_ok,
                    semantic_tokens_token_count = probe.semantic_tokens_token_count,
                    server_name = probe.server_name.as_deref().unwrap_or(""),
                    server_version = probe.server_version.as_deref().unwrap_or(""),
                    "catalog LSP extension runtime capability probe completed"
                );
                probes.push(probe);
            }
            Err(error) => {
                errors.push(format!("{}: {}", language, error));
            }
        }
    }

    if !errors.is_empty() {
        return Err(format!(
            "{} runtime probe failed for {}: {}",
            source_label,
            state.id,
            errors.join(" | ")
        ));
    }

    Ok(probes)
}

async fn prepare_catalog_extension_runtime_if_missing(
    state: LspExtensionState,
    source_label: &str,
) -> Result<LspExtensionState, String> {
    let catalog = catalog_by_id()
        .get(&normalize_extension_id(&state.id))
        .cloned()
        .ok_or_else(|| {
            format!(
                "{} installed extension {} but it is not in the LSP catalog",
                source_label, state.id
            )
        })?;
    let install_dir = state
        .installed_path
        .as_deref()
        .map(PathBuf::from)
        .ok_or_else(|| {
            format!(
                "{} installed extension {} but no install path was recorded",
                source_label, state.id
            )
        })?;

    if state.runtime_status.ready && normalize_string(&catalog.lsp.runtime) != "node_binary" {
        return Ok(state);
    }

    warn!(
        extension_id = state.id.as_str(),
        source = source_label,
        runtime = state.runtime_status.runtime.as_str(),
        message = state.runtime_status.message.as_deref().unwrap_or(""),
        install_dir = install_dir.to_string_lossy().as_ref(),
        "catalog LSP extension runtime is missing; preparing runtime before verification"
    );

    match normalize_string(&catalog.lsp.runtime).as_str() {
        "node_binary" => {
            // 设计说明：
            // 1. 共享 Node runtime 只看 bin 文件是否存在不足够，旧版本并发 npm install 会留下可解析但不可运行的半安装依赖。
            // 2. 因此 node_binary catalog 每次安装验证前都会经过 prepare_shared_node_runtime，由该函数用 marker 决定是否跳过。
            // 3. 这样可以自动修复日志里的 core-js MODULE_NOT_FOUND、TAR_ENTRY_ERROR、ENOTEMPTY 等缓存损坏。
            prepare_shared_node_runtime(&catalog, source_label).await?;
        }
        runtime => {
            return Err(format!(
                "{} installed extension {} but runtime is not ready and automatic preparation is unsupported for runtime {}: {}",
                source_label,
                state.id,
                runtime,
                state.runtime_status.message.unwrap_or_else(|| "missing runtime entry".to_string())
            ));
        }
    }

    let record = record_from_dir(&install_dir).ok_or_else(|| {
        format!(
            "failed to rebuild extension record after preparing runtime for {}",
            state.id
        )
    })?;
    let refreshed = build_extension_state(Some(record), Some(catalog));
    if !refreshed.runtime_status.ready {
        return Err(format!(
            "runtime preparation completed for {} but runtime is still not ready: {}",
            refreshed.id,
            refreshed
                .runtime_status
                .message
                .unwrap_or_else(|| "missing runtime entry".to_string())
        ));
    }
    Ok(refreshed)
}

async fn verify_if_catalog_extension_runtime(
    state: LspExtensionState,
    source_label: &str,
) -> Result<LspExtensionState, String> {
    let catalog = catalog_by_id();
    let id = normalize_extension_id(&state.id);
    if !catalog.contains_key(&id) {
        return Ok(state);
    }

    let prepared = prepare_catalog_extension_runtime_if_missing(state, source_label).await;
    let state = match prepared {
        Ok(state) => state,
        Err(error) => {
            rollback_failed_catalog_install(&id, &error);
            return Err(error);
        }
    };

    match verify_catalog_extension_runtime(&state, source_label).await {
        Ok(probes) => {
            info!(
                extension_id = id.as_str(),
                source = source_label,
                probe_count = probes.len(),
                probes = ?probes,
                "verified installed catalog LSP extension runtime"
            );
            Ok(state)
        }
        Err(error) => {
            rollback_failed_catalog_install(&id, &error);
            Err(error)
        }
    }
}

fn rollback_failed_catalog_install(extension_id: &str, reason: &str) {
    warn!(
        extension_id = extension_id,
        reason = reason,
        "rolling back LSP extension installation because runtime verification failed"
    );
    if let Err(error) = remove_existing_extension_versions(extension_id) {
        warn!(
            extension_id = extension_id,
            error = error.as_str(),
            "failed to rollback LSP extension after runtime verification failure"
        );
    }
}

async fn install_catalog_lsp_extension_record(
    catalog: &CatalogExtension,
) -> Result<LspExtensionState, String> {
    let id = normalize_extension_id(&catalog.id);
    let version = format!("catalog.{}", LSP_EXTENSION_CATALOG.version);
    // 设计说明：
    // 1. catalog 扩展也可能已经拉起了托管 LSP server，卸载旧 catalog 目录前必须先释放进程。
    // 2. 这里和真实 VSIX 安装使用同一套停止逻辑，保证“下载/覆盖”和“卸载”对文件锁的处理一致。
    let stopped = lsp::stop_lsp_processes_for_extension(&id).await;
    info!(
        extension_id = id.as_str(),
        stopped_process_count = stopped,
        "stopped active LSP processes before installing catalog extension"
    );
    remove_existing_extension_versions(&id)?;
    let install_dir = unique_install_dir(&id, &version)?;
    write_catalog_extension_package(&install_dir, catalog)?;
    write_extension_meta(
        &install_dir,
        &InstalledExtensionMeta {
            source: "guaxi-catalog".to_string(),
            installed_at: now_rfc3339(),
            download_url: None,
            open_vsx: catalog.open_vsx.clone(),
            vscode_marketplace: catalog.vscode_marketplace.clone(),
            original_file_name: None,
        },
    )?;

    info!(
        extension_id = id.as_str(),
        install_dir = install_dir.to_string_lossy().as_ref(),
        "catalog LSP extension installed"
    );
    let record = record_from_dir(&install_dir)
        .ok_or_else(|| "failed to rebuild catalog extension record after install".to_string())?;
    Ok(build_extension_state(Some(record), Some(catalog.clone())))
}

pub async fn install_catalog_lsp_extension(
    extension_id: &str,
) -> Result<LspExtensionState, String> {
    let id = normalize_extension_id(extension_id);
    if id.is_empty() {
        return Err("extension_id cannot be empty".to_string());
    }
    let catalog = catalog_by_id()
        .get(&id)
        .cloned()
        .ok_or_else(|| format!("unknown LSP catalog extension: {}", extension_id))?;

    // 设计说明：
    // 1. “安装成功”必须代表后端可以真实启动 language server，而不只是写入一个扩展登记项。
    // 2. 因此按真实 VSIX -> Open VSX -> Guaxi catalog fallback 的顺序尝试，每次安装后都发送 initialize 探测。
    // 3. 探测要求 completionProvider 存在；语法高亮由前端 TextMate grammar 保底，semanticTokens 会在日志中记录。
    let mut attempts = vec![];

    if let Some(identifier) = catalog.vscode_marketplace.as_deref() {
        match install_vscode_marketplace_extension(identifier, None, None).await {
            Ok(state) => {
                match prepare_catalog_extension_runtime_if_missing(state, "VSCode Marketplace")
                    .await
                {
                    Ok(state) => {
                        match verify_catalog_extension_runtime(&state, "VSCode Marketplace").await {
                            Ok(probes) => {
                                info!(
                                    extension_id = id.as_str(),
                                    probe_count = probes.len(),
                                    probes = ?probes,
                                    "catalog LSP extension verified from VSCode Marketplace"
                                );
                                return Ok(state);
                            }
                            Err(error) => {
                                rollback_failed_catalog_install(&id, &error);
                                attempts.push(error);
                            }
                        }
                    }
                    Err(error) => {
                        rollback_failed_catalog_install(&id, &error);
                        attempts.push(error);
                    }
                }
            }
            Err(error) => {
                warn!(
                    extension_id = id.as_str(),
                    error = error.as_str(),
                    "VSCode Marketplace catalog install attempt failed"
                );
                attempts.push(format!("VSCode Marketplace install failed: {}", error));
            }
        }
    }

    if let Some(identifier) = catalog.open_vsx.as_deref() {
        match install_open_vsx_extension(identifier, None, None).await {
            Ok(state) => {
                match prepare_catalog_extension_runtime_if_missing(state, "Open VSX").await {
                    Ok(state) => match verify_catalog_extension_runtime(&state, "Open VSX").await {
                        Ok(probes) => {
                            info!(
                                extension_id = id.as_str(),
                                probe_count = probes.len(),
                                probes = ?probes,
                                "catalog LSP extension verified from Open VSX"
                            );
                            return Ok(state);
                        }
                        Err(error) => {
                            rollback_failed_catalog_install(&id, &error);
                            attempts.push(error);
                        }
                    },
                    Err(error) => {
                        rollback_failed_catalog_install(&id, &error);
                        attempts.push(error);
                    }
                }
            }
            Err(error) => {
                warn!(
                    extension_id = id.as_str(),
                    error = error.as_str(),
                    "Open VSX catalog install attempt failed"
                );
                attempts.push(format!("Open VSX install failed: {}", error));
            }
        }
    }

    let state = install_catalog_lsp_extension_record(&catalog).await?;
    let state =
        match prepare_catalog_extension_runtime_if_missing(state, "Guaxi catalog fallback").await {
            Ok(state) => state,
            Err(error) => {
                rollback_failed_catalog_install(&id, &error);
                attempts.push(error);
                return Err(format!(
                    "LSP 扩展 {} 安装后未能完成运行时准备，已回滚。尝试结果: {}",
                    id,
                    attempts.join(" || ")
                ));
            }
        };
    match verify_catalog_extension_runtime(&state, "Guaxi catalog fallback").await {
        Ok(probes) => {
            info!(
                extension_id = id.as_str(),
                probe_count = probes.len(),
                probes = ?probes,
                "catalog LSP extension verified from Guaxi fallback runtime"
            );
            Ok(state)
        }
        Err(error) => {
            rollback_failed_catalog_install(&id, &error);
            attempts.push(error);
            Err(format!(
                "LSP 扩展 {} 安装后未能通过真实启动检查，已回滚。尝试结果: {}",
                id,
                attempts.join(" || ")
            ))
        }
    }
}

pub async fn import_vsix_extension(vsix_path: &str) -> Result<LspExtensionState, String> {
    let source_path = PathBuf::from(vsix_path.trim());
    if !source_path.is_file() {
        return Err(format!(
            "vsix file not found: {}",
            source_path.to_string_lossy()
        ));
    }

    info!(
        path = source_path.to_string_lossy().as_ref(),
        "importing VSIX extension"
    );

    let unpack_root = temp_root_dir()?.join(format!("vsix_{}", Uuid::new_v4()));
    fs::create_dir_all(&unpack_root)
        .map_err(|e| format!("failed to create VSIX temp directory: {e}"))?;
    extract_zip_to_dir(&source_path, &unpack_root)?;

    let result = finalize_install_from_unpacked_root(
        &unpack_root,
        InstalledExtensionMeta {
            source: "local-vsix".to_string(),
            installed_at: now_rfc3339(),
            download_url: None,
            open_vsx: None,
            vscode_marketplace: None,
            original_file_name: source_path
                .file_name()
                .and_then(|item| item.to_str())
                .map(normalize_string),
        },
        None,
    )
    .await;

    let _ = fs::remove_dir_all(&unpack_root);
    let state = result?;
    verify_if_catalog_extension_runtime(state, "local VSIX").await
}

pub async fn import_extension_directory(dir_path: &str) -> Result<LspExtensionState, String> {
    let source_dir = PathBuf::from(dir_path.trim());
    if !source_dir.is_dir() {
        return Err(format!(
            "extension directory not found: {}",
            source_dir.to_string_lossy()
        ));
    }

    info!(
        path = source_dir.to_string_lossy().as_ref(),
        "importing unpacked extension directory"
    );

    let state = finalize_install_from_unpacked_root(
        &source_dir,
        InstalledExtensionMeta {
            source: "local-directory".to_string(),
            installed_at: now_rfc3339(),
            download_url: None,
            open_vsx: None,
            vscode_marketplace: None,
            original_file_name: source_dir
                .file_name()
                .and_then(|item| item.to_str())
                .map(normalize_string),
        },
        None,
    )
    .await?;
    verify_if_catalog_extension_runtime(state, "local directory").await
}

fn parse_open_vsx_result(item: &Value) -> Option<OpenVsxSearchResult> {
    let namespace = normalize_string(item.get("namespace")?.as_str()?);
    let name = normalize_string(item.get("name")?.as_str()?);
    let id = normalize_extension_id(&format!("{namespace}.{name}"));
    let version = normalize_string(
        item.get("version")
            .and_then(Value::as_str)
            .unwrap_or("latest"),
    );
    let display_name = normalize_string(
        item.get("displayName")
            .and_then(Value::as_str)
            .or_else(|| item.get("display_name").and_then(Value::as_str))
            .unwrap_or(&name),
    );

    let download_url = item
        .get("files")
        .and_then(|files| files.get("download"))
        .and_then(Value::as_str)
        .map(normalize_string)
        .or_else(|| {
            item.get("downloadUrl")
                .and_then(Value::as_str)
                .map(normalize_string)
        });

    let icon_url = item
        .get("files")
        .and_then(|files| files.get("icon"))
        .and_then(Value::as_str)
        .map(normalize_string)
        .or_else(|| {
            item.get("iconUrl")
                .and_then(Value::as_str)
                .map(normalize_string)
        });

    Some(OpenVsxSearchResult {
        id,
        namespace,
        name,
        version,
        display_name,
        description: item
            .get("description")
            .and_then(Value::as_str)
            .map(normalize_string),
        download_url,
        icon_url,
    })
}

pub async fn search_open_vsx_extensions(query: &str) -> Result<Vec<OpenVsxSearchResult>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(vec![]);
    }

    let url = format!(
        "https://open-vsx.org/api/-/search?size=20&query={}",
        urlencoding::encode(trimmed)
    );
    info!(
        query = trimmed,
        url = url.as_str(),
        "searching Open VSX extensions"
    );

    let client = lsp_extension_http_client("search-open-vsx")?;
    let response = client
        .get(&url)
        .header("User-Agent", "guaxi-lsp-extension")
        .send()
        .await
        .map_err(|e| format!("failed to query Open VSX search API: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Open VSX search failed: HTTP {}",
            response.status()
        ));
    }

    let payload = response
        .json::<Value>()
        .await
        .map_err(|e| format!("failed to parse Open VSX search response: {e}"))?;

    let items = payload
        .get("extensions")
        .and_then(Value::as_array)
        .or_else(|| payload.get("results").and_then(Value::as_array))
        .cloned()
        .unwrap_or_default();

    let mut result = items
        .iter()
        .filter_map(parse_open_vsx_result)
        .collect::<Vec<_>>();

    result.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(result)
}

fn parse_marketplace_extension(item: &Value) -> Option<VscodeMarketplaceSearchResult> {
    let name = normalize_string(item.get("extensionName")?.as_str()?);
    let publisher = normalize_string(
        item.get("publisher")
            .and_then(|publisher| publisher.get("publisherName"))
            .and_then(Value::as_str)?,
    );
    let id = normalize_extension_id(&format!("{publisher}.{name}"));
    let display_name = normalize_string(
        item.get("displayName")
            .and_then(Value::as_str)
            .unwrap_or(&name),
    );
    let version_item = item
        .get("versions")
        .and_then(Value::as_array)
        .and_then(|items| items.first())?;
    let version = normalize_string(
        version_item
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("latest"),
    );
    let files = version_item
        .get("files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let asset_url = |asset_type: &str| {
        files
            .iter()
            .find(|file| file.get("assetType").and_then(Value::as_str) == Some(asset_type))
            .and_then(|file| file.get("source").and_then(Value::as_str))
            .map(normalize_string)
    };

    let download_url = asset_url("Microsoft.VisualStudio.Services.VSIXPackage")
        .or_else(|| {
            Some(format!(
                "https://marketplace.visualstudio.com/_apis/public/gallery/publishers/{}/vsextensions/{}/{}/vspackage",
                publisher, name, version
            ))
        });
    let icon_url = asset_url("Microsoft.VisualStudio.Services.Icons.Default");

    Some(VscodeMarketplaceSearchResult {
        id,
        publisher,
        name,
        version,
        display_name,
        description: item
            .get("shortDescription")
            .or_else(|| item.get("description"))
            .and_then(Value::as_str)
            .map(normalize_string),
        download_url,
        icon_url,
    })
}

pub async fn search_vscode_marketplace_extensions(
    query: &str,
) -> Result<Vec<VscodeMarketplaceSearchResult>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(vec![]);
    }

    // 设计说明：
    // 1. VSCode Marketplace 没有像 Open VSX 那样简单的公开搜索 GET 接口，VSCode 客户端使用 gallery
    //    extensionquery API 查询，这里按 VS Code Target + 搜索文本组合过滤。
    // 2. flags 取回版本、asset files、manifest 等下载所需信息；真正安装仍统一走 VSIX 解包与 manifest 解析。
    // 3. 这样配置页可以同时支持 Marketplace 与 Open VSX，但运行时 registry 仍只有一份。
    let body = json!({
        "filters": [{
            "criteria": [
                { "filterType": 8, "value": "Microsoft.VisualStudio.Code" },
                { "filterType": 10, "value": trimmed }
            ],
            "pageNumber": 1,
            "pageSize": 20,
            "sortBy": 0,
            "sortOrder": 0
        }],
        "flags": 914
    });

    info!(query = trimmed, "searching VSCode Marketplace extensions");
    let client = lsp_extension_http_client("search-vscode-marketplace")?;
    let response = client
        .post("https://marketplace.visualstudio.com/_apis/public/gallery/extensionquery")
        .header("Accept", "application/json;api-version=3.0-preview.1")
        .header("Content-Type", "application/json")
        .header("User-Agent", "guaxi-lsp-extension")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("failed to query VSCode Marketplace: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "VSCode Marketplace search failed: HTTP {}",
            response.status()
        ));
    }

    let payload = response
        .json::<Value>()
        .await
        .map_err(|e| format!("failed to parse VSCode Marketplace response: {e}"))?;
    let items = payload
        .get("results")
        .and_then(Value::as_array)
        .and_then(|results| results.first())
        .and_then(|result| result.get("extensions"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut result = items
        .iter()
        .filter_map(parse_marketplace_extension)
        .collect::<Vec<_>>();
    result.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(result)
}

async fn resolve_open_vsx_download_from_search(
    identifier: &str,
    requested_version: Option<&str>,
) -> Result<Option<(String, String, String)>, String> {
    let normalized_identifier = normalize_extension_id(identifier);
    if normalized_identifier.is_empty() {
        return Ok(None);
    }

    // 设计说明：
    // 1. Open VSX 的 detail API 对个别扩展会返回 404，但 search API 仍能给出可下载的 files.download。
    // 2. 这里做一次“精确 ID 优先”的兜底搜索，避免目录安装按钮因为单一 detail 接口失败而完全不可用。
    let search_result = search_open_vsx_extensions(&normalized_identifier).await?;
    let matched = search_result
        .into_iter()
        .find(|item| normalize_extension_id(&item.id) == normalized_identifier);

    let Some(item) = matched else {
        warn!(
            extension_id = normalized_identifier.as_str(),
            "Open VSX search fallback did not find an exact extension match"
        );
        return Ok(None);
    };

    let resolved_version = requested_version
        .map(normalize_string)
        .filter(|item| !item.is_empty())
        .unwrap_or_else(|| normalize_string(&item.version));
    let resolved_version = if resolved_version.is_empty() {
        "latest".to_string()
    } else {
        resolved_version
    };

    let resolved_url = item.download_url.clone().unwrap_or_else(|| {
        format!(
            "https://open-vsx.org/api/{}/{}/{}/file/{}-{}.vsix",
            item.namespace, item.name, resolved_version, item.id, resolved_version
        )
    });
    let resolved_url = rewrite_open_vsx_download_url_for_current_platform(&resolved_url);

    info!(
        extension_id = normalized_identifier.as_str(),
        version = resolved_version.as_str(),
        download_url = resolved_url.as_str(),
        "resolved Open VSX extension download URL from search fallback"
    );

    Ok(Some((resolved_url, resolved_version, item.id)))
}

async fn resolve_open_vsx_download(
    identifier: &str,
    version: Option<&str>,
    download_url: Option<&str>,
) -> Result<(String, String, String), String> {
    let (namespace, name) = parse_extension_identifier(identifier)?;
    let requested_version = version.map(normalize_string);

    if let Some(url) = download_url {
        let effective_version = requested_version.unwrap_or_else(|| "latest".to_string());
        return Ok((
            normalize_string(url),
            effective_version,
            format!("{namespace}.{name}"),
        ));
    }

    let detail_url = if let Some(version) = requested_version.as_deref() {
        format!("https://open-vsx.org/api/{namespace}/{name}/{version}")
    } else {
        format!("https://open-vsx.org/api/{namespace}/{name}")
    };

    info!(
        extension_id = identifier,
        detail_url = detail_url.as_str(),
        "resolving Open VSX extension download URL"
    );

    let client = lsp_extension_http_client("resolve-open-vsx-detail")?;
    let response = client
        .get(&detail_url)
        .header("User-Agent", "guaxi-lsp-extension")
        .send()
        .await
        .map_err(|e| format!("failed to query Open VSX extension detail: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        warn!(
            extension_id = identifier,
            detail_url = detail_url.as_str(),
            status = status.as_u16(),
            "Open VSX detail lookup failed, attempting search fallback"
        );

        if let Some(fallback) =
            resolve_open_vsx_download_from_search(identifier, requested_version.as_deref()).await?
        {
            return Ok(fallback);
        }

        return Err(format!(
            "Open VSX detail lookup failed for {}: HTTP {}",
            identifier, status
        ));
    }

    let payload = response
        .json::<Value>()
        .await
        .map_err(|e| format!("failed to parse Open VSX extension detail: {e}"))?;

    let resolved_version = payload
        .get("version")
        .and_then(Value::as_str)
        .map(normalize_string)
        .or(requested_version)
        .unwrap_or_else(|| "latest".to_string());

    let resolved_url = payload
        .get("files")
        .and_then(|files| files.get("download"))
        .and_then(Value::as_str)
        .map(normalize_string)
        .or_else(|| payload.get("download").and_then(Value::as_str).map(normalize_string))
        .unwrap_or_else(|| {
            format!(
                "https://open-vsx.org/api/{namespace}/{name}/{resolved_version}/file/{namespace}.{name}-{resolved_version}.vsix"
            )
        });
    let resolved_url = rewrite_open_vsx_download_url_for_current_platform(&resolved_url);

    Ok((
        resolved_url,
        resolved_version,
        format!("{namespace}.{name}"),
    ))
}

pub async fn install_open_vsx_extension(
    identifier: &str,
    version: Option<&str>,
    download_url: Option<&str>,
) -> Result<LspExtensionState, String> {
    let (resolved_url, resolved_version, resolved_id) =
        resolve_open_vsx_download(identifier, version, download_url).await?;
    let download_file = temp_root_dir()?.join(format!(
        "{}-{}.vsix",
        sanitize_dir_fragment(&resolved_id),
        sanitize_dir_fragment(&resolved_version)
    ));

    info!(
        extension_id = resolved_id.as_str(),
        version = resolved_version.as_str(),
        download_url = resolved_url.as_str(),
        destination = download_file.to_string_lossy().as_ref(),
        "downloading Open VSX extension archive"
    );

    download_to_file(&resolved_url, &download_file).await?;

    let unpack_root = temp_root_dir()?.join(format!("openvsx_{}", Uuid::new_v4()));
    fs::create_dir_all(&unpack_root)
        .map_err(|e| format!("failed to create Open VSX temp directory: {e}"))?;
    extract_zip_to_dir(&download_file, &unpack_root)?;

    let result = finalize_install_from_unpacked_root(
        &unpack_root,
        InstalledExtensionMeta {
            source: "openvsx".to_string(),
            installed_at: now_rfc3339(),
            download_url: Some(resolved_url),
            open_vsx: Some(resolved_id.clone()),
            vscode_marketplace: None,
            original_file_name: download_file
                .file_name()
                .and_then(|item| item.to_str())
                .map(normalize_string),
        },
        Some(&resolved_id),
    )
    .await;

    let _ = fs::remove_file(&download_file);
    let _ = fs::remove_dir_all(&unpack_root);
    let state = result?;
    verify_if_catalog_extension_runtime(state, "Open VSX").await
}

pub async fn install_vscode_marketplace_extension(
    identifier: &str,
    version: Option<&str>,
    download_url: Option<&str>,
) -> Result<LspExtensionState, String> {
    let (publisher, name) = parse_extension_identifier(identifier)?;
    let requested_version = version
        .map(normalize_string)
        .filter(|item| !item.is_empty());

    let resolved = if let Some(url) = download_url {
        (
            normalize_string(url),
            requested_version.unwrap_or_else(|| "latest".to_string()),
            format!("{publisher}.{name}"),
        )
    } else {
        let search_result =
            search_vscode_marketplace_extensions(&format!("{publisher}.{name}")).await?;
        let matched = search_result
            .into_iter()
            .find(|item| normalize_extension_id(&item.id) == normalize_extension_id(identifier))
            .ok_or_else(|| {
                format!(
                    "VSCode Marketplace did not return extension: {}",
                    identifier
                )
            })?;
        let resolved_version = requested_version.unwrap_or_else(|| matched.version.clone());
        let resolved_url = matched.download_url.unwrap_or_else(|| {
            format!(
                "https://marketplace.visualstudio.com/_apis/public/gallery/publishers/{}/vsextensions/{}/{}/vspackage",
                publisher, name, resolved_version
            )
        });
        (resolved_url, resolved_version, matched.id)
    };

    let (resolved_url, resolved_version, resolved_id) = resolved;
    let download_file = temp_root_dir()?.join(format!(
        "{}-{}.vsix",
        sanitize_dir_fragment(&resolved_id),
        sanitize_dir_fragment(&resolved_version)
    ));

    info!(
        extension_id = resolved_id.as_str(),
        version = resolved_version.as_str(),
        download_url = resolved_url.as_str(),
        destination = download_file.to_string_lossy().as_ref(),
        "downloading VSCode Marketplace extension archive"
    );

    download_to_file(&resolved_url, &download_file).await?;

    let unpack_root = temp_root_dir()?.join(format!("marketplace_{}", Uuid::new_v4()));
    fs::create_dir_all(&unpack_root)
        .map_err(|e| format!("failed to create Marketplace temp directory: {e}"))?;
    extract_zip_to_dir(&download_file, &unpack_root)?;

    let expected_id = resolved_id.clone();
    let result = finalize_install_from_unpacked_root(
        &unpack_root,
        InstalledExtensionMeta {
            source: "vscode-marketplace".to_string(),
            installed_at: now_rfc3339(),
            download_url: Some(resolved_url),
            open_vsx: None,
            vscode_marketplace: Some(resolved_id),
            original_file_name: download_file
                .file_name()
                .and_then(|item| item.to_str())
                .map(normalize_string),
        },
        Some(&expected_id),
    )
    .await;

    let _ = fs::remove_file(&download_file);
    let _ = fs::remove_dir_all(&unpack_root);
    let state = result?;
    verify_if_catalog_extension_runtime(state, "VSCode Marketplace").await
}

pub async fn remove_lsp_extension(extension_id: &str) -> Result<(), String> {
    let normalized = normalize_extension_id(extension_id);
    if normalized.is_empty() {
        return Err("extension_id cannot be empty".to_string());
    }

    info!(
        extension_id = normalized.as_str(),
        "removing installed LSP extension"
    );
    // 设计说明：
    // 1. 卸载失败最常见的原因是 language server 进程仍以扩展目录为 cwd 或正在读取扩展内入口文件。
    // 2. 删除目录前统一通知 LSP actor 关闭，并短暂等待子进程退出，让 Windows 文件锁先释放。
    // 3. 如果仍有外部进程占用，后续 try_remove_extension_dir 会继续做重试和延迟删除兜底。
    let stopped = lsp::stop_lsp_processes_for_extension(&normalized).await;
    info!(
        extension_id = normalized.as_str(),
        stopped_process_count = stopped,
        "stopped active LSP processes before removing extension"
    );
    remove_existing_extension_versions(&normalized)
}

pub fn start_install_open_vsx_extension_task(
    identifier: String,
    version: Option<String>,
    download_url: Option<String>,
) -> Result<LspExtensionTaskSnapshot, String> {
    let extension_id = normalize_extension_id(&identifier);
    let snapshot =
        insert_lsp_extension_task("install-openvsx", &extension_id, "queued Open VSX install")?;
    Ok(spawn_lsp_extension_task(snapshot, async move {
        install_open_vsx_extension(&identifier, version.as_deref(), download_url.as_deref()).await
    }))
}

pub fn start_install_vscode_marketplace_extension_task(
    identifier: String,
    version: Option<String>,
    download_url: Option<String>,
) -> Result<LspExtensionTaskSnapshot, String> {
    let extension_id = normalize_extension_id(&identifier);
    let snapshot = insert_lsp_extension_task(
        "install-marketplace",
        &extension_id,
        "queued VSCode Marketplace install",
    )?;
    Ok(spawn_lsp_extension_task(snapshot, async move {
        install_vscode_marketplace_extension(
            &identifier,
            version.as_deref(),
            download_url.as_deref(),
        )
        .await
    }))
}

pub fn start_install_catalog_lsp_extension_task(
    identifier: String,
) -> Result<LspExtensionTaskSnapshot, String> {
    let extension_id = normalize_extension_id(&identifier);
    let snapshot =
        insert_lsp_extension_task("install-catalog", &extension_id, "queued catalog install")?;
    let task_id = snapshot.task_id.clone();
    Ok({
        tokio::spawn(async move {
            let mut attempt: u32 = 1;
            loop {
                update_lsp_extension_task(
                    &task_id,
                    "running",
                    &format!("configuring LSP extension runtime, attempt {}", attempt),
                    None,
                    None,
                );
                info!(
                    task_id = task_id.as_str(),
                    extension_id = extension_id.as_str(),
                    attempt,
                    "starting catalog LSP extension install/configuration attempt"
                );

                match install_catalog_lsp_extension(&identifier).await {
                    Ok(state) => {
                        update_lsp_extension_task(
                            &task_id,
                            "success",
                            "task completed after runtime probe returned completion items",
                            None,
                            Some(state),
                        );
                        break;
                    }
                    Err(error) => {
                        if attempt >= LSP_CATALOG_RUNTIME_CONFIGURE_MAX_ATTEMPTS {
                            warn!(
                                task_id = task_id.as_str(),
                                extension_id = extension_id.as_str(),
                                attempt,
                                error = error.as_str(),
                                "catalog LSP extension runtime configuration reached max attempts"
                            );
                            update_lsp_extension_task(
                                &task_id,
                                "error",
                                &format!(
                                    "runtime configuration failed after {} attempts",
                                    LSP_CATALOG_RUNTIME_CONFIGURE_MAX_ATTEMPTS
                                ),
                                Some(error),
                                None,
                            );
                            break;
                        }
                        let delay_secs = u64::from((attempt.min(10)) * 6);
                        warn!(
                            task_id = task_id.as_str(),
                            extension_id = extension_id.as_str(),
                            attempt,
                            delay_secs,
                            error = error.as_str(),
                            "catalog LSP extension runtime is not ready; retrying configuration"
                        );
                        update_lsp_extension_task(
                            &task_id,
                            "running",
                            &format!(
                                "runtime probe failed on attempt {}; retrying in {}s: {}",
                                attempt, delay_secs, error
                            ),
                            Some(error),
                            None,
                        );

                        // Design note:
                        // 1. Catalog install is marked successful only after a real LSP completion probe passes.
                        // 2. Network, npm registry, GitHub Release, or runtime startup failures can be transient, so retry with backoff.
                        // 3. The retry count is capped to avoid a bad source consuming background resources forever.
                        tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                        attempt = attempt.saturating_add(1);
                    }
                }
            }
        });
        snapshot
    })
}

pub fn start_import_vsix_extension_task(path: String) -> Result<LspExtensionTaskSnapshot, String> {
    let task_key = format!("local-vsix:{}", path.trim().to_lowercase());
    let snapshot = insert_lsp_extension_task("import-vsix", &task_key, "queued VSIX import")?;
    Ok(spawn_lsp_extension_task(snapshot, async move {
        import_vsix_extension(&path).await
    }))
}

pub fn start_import_extension_directory_task(
    path: String,
) -> Result<LspExtensionTaskSnapshot, String> {
    let task_key = format!("local-directory:{}", path.trim().to_lowercase());
    let snapshot = insert_lsp_extension_task(
        "import-directory",
        &task_key,
        "queued extension directory import",
    )?;
    Ok(spawn_lsp_extension_task(snapshot, async move {
        import_extension_directory(&path).await
    }))
}

pub fn start_remove_lsp_extension_task(
    identifier: String,
) -> Result<LspExtensionTaskSnapshot, String> {
    let extension_id = normalize_extension_id(&identifier);
    let snapshot = insert_lsp_extension_task("remove", &extension_id, "queued extension removal")?;
    Ok(spawn_lsp_extension_void_task(snapshot, async move {
        remove_lsp_extension(&identifier).await
    }))
}

pub(crate) fn has_installed_lsp_extension_for_language(language: &str) -> Result<bool, String> {
    let normalized_language = normalize_language(language);
    if normalized_language.is_empty() {
        return Ok(false);
    }

    let catalog = catalog_by_id();
    let installed = load_installed_extensions()?;
    let matched = installed.into_iter().any(|item| {
        if !item
            .languages
            .iter()
            .any(|lang| normalize_language(lang) == normalized_language)
        {
            return false;
        }
        catalog.contains_key(&normalize_extension_id(&item.id))
    });
    Ok(matched)
}

pub(crate) fn installed_lsp_extension_ids_for_language(
    language: &str,
) -> Result<Vec<String>, String> {
    let normalized_language = normalize_language(language);
    if normalized_language.is_empty() {
        return Ok(vec![]);
    }

    let catalog = catalog_by_id();
    let mut matched = load_installed_extensions()?
        .into_iter()
        .filter(|item| {
            item.languages
                .iter()
                .any(|lang| normalize_language(lang) == normalized_language)
        })
        .filter(|item| catalog.contains_key(&normalize_extension_id(&item.id)))
        .map(|item| normalize_extension_id(&item.id))
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    matched.sort();
    matched.dedup();
    Ok(matched)
}

pub(crate) async fn resolve_lsp_command_for_language(
    language: &str,
) -> Result<Option<ResolvedLspExtensionCommand>, String> {
    let normalized_language = normalize_language(language);
    if normalized_language.is_empty() {
        return Ok(None);
    }

    let installed = load_installed_extensions()?;
    let catalog = catalog_by_id();

    let mut candidates = installed
        .into_iter()
        .filter(|item| {
            item.languages
                .iter()
                .any(|lang| normalize_language(lang) == normalized_language)
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|a, b| a.display_name.cmp(&b.display_name));

    for candidate in candidates {
        let Some(catalog_entry) = catalog.get(&normalize_extension_id(&candidate.id)).cloned()
        else {
            continue;
        };

        info!(
            language = normalized_language.as_str(),
            extension_id = candidate.id.as_str(),
            runtime = catalog_entry.lsp.runtime.as_str(),
            "attempting to resolve LSP command from installed extension"
        );

        for strategy in &catalog_entry.lsp.resolve_order {
            match strategy.as_str() {
                "extension_bin" => {
                    if normalize_string(&catalog_entry.lsp.runtime) == "node_binary" {
                        if let Some(entry_path) = resolve_node_binary_entry(
                            &candidate.install_dir,
                            &catalog_entry.lsp.bin_name,
                        ) {
                            // 设计说明：
                            // 1. Windows 下 npm 生成的 .cmd shim 在后台管道场景里偶发无法正确保持 stdio，
                            //    日志表现为 initialize/completion 探针一直超时但没有 stderr。
                            // 2. 对 Node 类 LSP 优先使用 package.json 的 bin 入口，并由内置 node.exe 直接执行，
                            //    可以绕过 cmd.exe/bat 包装层，让安装探针和编辑器补全走同一条稳定 stdio 通道。
                            // 3. PATH 仍包含扩展目录和 .bin，保留 server 运行时按需查找相邻依赖的能力。
                            let env_roots = vec![
                                candidate.install_dir.clone(),
                                candidate.install_dir.join("node_modules").join(".bin"),
                            ];
                            let node_program = node_command_program();
                            info!(
                                language = normalized_language.as_str(),
                                extension_id = candidate.id.as_str(),
                                strategy = "extension_bin_node_entry",
                                node = node_program.as_str(),
                                entry = entry_path.to_string_lossy().as_ref(),
                                cwd = candidate.install_dir.to_string_lossy().as_ref(),
                                "resolved LSP server from extension node package bin entry"
                            );
                            return Ok(Some(resolve_node_binary_command_from_entry(
                                &entry_path,
                                &catalog_entry.lsp.args,
                                &candidate.install_dir,
                                &env_roots,
                                &candidate.id,
                                &candidate.display_name,
                                &catalog_entry.lsp.server_id,
                            )));
                        }
                    }
                    if let Some(path) =
                        resolve_extension_bin(&candidate.install_dir, &catalog_entry.lsp.bin_name)
                    {
                        let env_roots = vec![
                            candidate.install_dir.clone(),
                            candidate.install_dir.join("node_modules").join(".bin"),
                        ];
                        return Ok(Some(ResolvedLspExtensionCommand {
                            program: path.to_string_lossy().to_string(),
                            args: catalog_entry.lsp.args.clone(),
                            envs: build_path_env_with_roots(&env_roots),
                            cwd: Some(candidate.install_dir.to_string_lossy().to_string()),
                            extension_id: candidate.id.clone(),
                            extension_display_name: candidate.display_name.clone(),
                            server_id: catalog_entry.lsp.server_id.clone(),
                        }));
                    }
                    info!(
                        language = normalized_language.as_str(),
                        extension_id = candidate.id.as_str(),
                        strategy = "extension_bin",
                        bin_name = catalog_entry.lsp.bin_name.as_str(),
                        candidates = ?extension_bin_candidate_paths(&candidate.install_dir, &catalog_entry.lsp.bin_name),
                        shared_candidates = ?shared_node_runtime_candidate_paths(&catalog_entry.lsp.bin_name),
                        "LSP extension runtime strategy did not resolve"
                    );
                }
                "extension_entry" => {
                    if let Some(path) = resolve_relative_entry(
                        &candidate.install_dir,
                        &catalog_entry.lsp.entry_paths,
                    ) {
                        if normalize_string(&catalog_entry.lsp.runtime) == "node_entry"
                            || is_node_runtime_entry_path(&path)
                        {
                            // Design note: VSCode extensions can ship the language server as a
                            // bundled JS entry under the downloaded extension directory. We run
                            // that JS entry with the available Node command while keeping package
                            // lookup scoped to the extension and shared runtime directories.
                            let mut args = vec![path.to_string_lossy().to_string()];
                            args.extend(catalog_entry.lsp.args.clone());
                            let env_roots = vec![
                                candidate.install_dir.clone(),
                                candidate.install_dir.join("node_modules").join(".bin"),
                            ];
                            let node_program = node_command_program();
                            info!(
                                language = normalized_language.as_str(),
                                extension_id = candidate.id.as_str(),
                                strategy = "extension_entry",
                                runtime = catalog_entry.lsp.runtime.as_str(),
                                node = node_program.as_str(),
                                entry = path.to_string_lossy().as_ref(),
                                cwd = candidate.install_dir.to_string_lossy().as_ref(),
                                "resolved LSP server from downloaded extension entry"
                            );
                            return Ok(Some(ResolvedLspExtensionCommand {
                                program: node_program,
                                args,
                                envs: build_path_env_with_roots(&env_roots),
                                cwd: Some(candidate.install_dir.to_string_lossy().to_string()),
                                extension_id: candidate.id.clone(),
                                extension_display_name: candidate.display_name.clone(),
                                server_id: catalog_entry.lsp.server_id.clone(),
                            }));
                        } else {
                            return Ok(Some(ResolvedLspExtensionCommand {
                                program: path.to_string_lossy().to_string(),
                                args: catalog_entry.lsp.args.clone(),
                                envs: vec![],
                                cwd: Some(candidate.install_dir.to_string_lossy().to_string()),
                                extension_id: candidate.id.clone(),
                                extension_display_name: candidate.display_name.clone(),
                                server_id: catalog_entry.lsp.server_id.clone(),
                            }));
                        }
                    }
                    info!(
                        language = normalized_language.as_str(),
                        extension_id = candidate.id.as_str(),
                        strategy = "extension_entry",
                        candidates = ?relative_entry_candidate_paths(&candidate.install_dir, &catalog_entry.lsp.entry_paths),
                        "LSP extension runtime strategy did not resolve"
                    );
                }
                "shared_node_runtime" => {
                    if normalize_string(&catalog_entry.lsp.runtime) == "node_binary" {
                        let Ok(root) = shared_node_runtime_dir() else {
                            info!(
                                language = normalized_language.as_str(),
                                extension_id = candidate.id.as_str(),
                                strategy = "shared_node_runtime",
                                "shared Node LSP runtime root is unavailable while resolving extension runtime"
                            );
                            continue;
                        };
                        if let Some(entry_path) =
                            resolve_node_binary_entry(&root, &catalog_entry.lsp.bin_name)
                        {
                            // 设计说明：
                            // 1. 共享 Node runtime 是 HTML/CSS/JSON/TS/Python 等插件的共同兜底，稳定性比单个插件更关键。
                            // 2. 这里同样优先执行 npm package 的真实 JS 入口，避免 .cmd shim 在 Rust 子进程管道中造成无响应。
                            // 3. 日志记录 node、entry、cwd，后续排查能直接复现启动命令。
                            let env_roots =
                                vec![root.clone(), root.join("node_modules").join(".bin")];
                            let node_program = node_command_program();
                            info!(
                                language = normalized_language.as_str(),
                                extension_id = candidate.id.as_str(),
                                strategy = "shared_node_runtime_entry",
                                node = node_program.as_str(),
                                entry = entry_path.to_string_lossy().as_ref(),
                                cwd = root.to_string_lossy().as_ref(),
                                "resolved LSP server from shared Node runtime package entry"
                            );
                            return Ok(Some(resolve_node_binary_command_from_entry(
                                &entry_path,
                                &catalog_entry.lsp.args,
                                &root,
                                &env_roots,
                                &candidate.id,
                                &candidate.display_name,
                                &catalog_entry.lsp.server_id,
                            )));
                        }
                        if let Some(path) =
                            resolve_extension_bin(&root, &catalog_entry.lsp.bin_name)
                        {
                            let env_roots =
                                vec![root.clone(), root.join("node_modules").join(".bin")];
                            info!(
                                language = normalized_language.as_str(),
                                extension_id = candidate.id.as_str(),
                                strategy = "shared_node_runtime_bin",
                                bin = path.to_string_lossy().as_ref(),
                                cwd = root.to_string_lossy().as_ref(),
                                "resolved LSP server from shared Node runtime bin"
                            );
                            return Ok(Some(ResolvedLspExtensionCommand {
                                program: path.to_string_lossy().to_string(),
                                args: catalog_entry.lsp.args.clone(),
                                envs: build_path_env_with_roots(&env_roots),
                                cwd: Some(root.to_string_lossy().to_string()),
                                extension_id: candidate.id.clone(),
                                extension_display_name: candidate.display_name.clone(),
                                server_id: catalog_entry.lsp.server_id.clone(),
                            }));
                        }
                        info!(
                            language = normalized_language.as_str(),
                            extension_id = candidate.id.as_str(),
                            strategy = "shared_node_runtime",
                            candidates = ?shared_node_runtime_candidate_paths(&catalog_entry.lsp.bin_name),
                            "LSP extension shared Node runtime strategy did not resolve"
                        );
                    }
                }
                "slidev_env_bin" => {
                    if let Some(root) = slidev_env_root().ok() {
                        if normalize_string(&catalog_entry.lsp.runtime) == "node_binary" {
                            if let (Some(node_path), Some(entry_path)) = (
                                slidev_node_path(),
                                resolve_node_binary_entry(&root, &catalog_entry.lsp.bin_name),
                            ) {
                                let env_roots = {
                                    let mut roots = vec![root.clone()];
                                    if let Some(bin_dir) = slidev_bin_dir() {
                                        roots.push(bin_dir);
                                    }
                                    roots
                                };
                                info!(
                                    language = normalized_language.as_str(),
                                    extension_id = candidate.id.as_str(),
                                    strategy = "slidev_env_node_entry",
                                    node = node_path.to_string_lossy().as_ref(),
                                    entry = entry_path.to_string_lossy().as_ref(),
                                    cwd = root.to_string_lossy().as_ref(),
                                    "resolved LSP server from bundled SlidevEnv node package bin entry"
                                );
                                return Ok(Some(resolve_node_binary_command_from_entry(
                                    &entry_path,
                                    &catalog_entry.lsp.args,
                                    &root,
                                    &env_roots,
                                    &candidate.id,
                                    &candidate.display_name,
                                    &catalog_entry.lsp.server_id,
                                )));
                            }
                        }
                        if let Some(path) =
                            resolve_extension_bin(&root, &catalog_entry.lsp.bin_name)
                        {
                            let mut env_roots = vec![root.clone()];
                            if let Some(bin_dir) = slidev_bin_dir() {
                                env_roots.push(bin_dir);
                            }
                            return Ok(Some(ResolvedLspExtensionCommand {
                                program: path.to_string_lossy().to_string(),
                                args: catalog_entry.lsp.args.clone(),
                                envs: build_path_env_with_roots(&env_roots),
                                cwd: Some(root.to_string_lossy().to_string()),
                                extension_id: candidate.id.clone(),
                                extension_display_name: candidate.display_name.clone(),
                                server_id: catalog_entry.lsp.server_id.clone(),
                            }));
                        }
                        info!(
                            language = normalized_language.as_str(),
                            extension_id = candidate.id.as_str(),
                            strategy = "slidev_env_bin",
                            bin_name = catalog_entry.lsp.bin_name.as_str(),
                            candidates = ?extension_bin_candidate_paths(&root, &catalog_entry.lsp.bin_name),
                            "LSP extension runtime strategy did not resolve"
                        );
                    } else {
                        info!(
                            language = normalized_language.as_str(),
                            extension_id = candidate.id.as_str(),
                            strategy = "slidev_env_bin",
                            "SlidevEnv root is unavailable while resolving LSP extension runtime"
                        );
                    }
                }
                "downloadable_server" => {
                    if let Some(server_id) = catalog_entry.lsp.fallback_server.as_deref() {
                        info!(
                            language = normalized_language.as_str(),
                            extension_id = candidate.id.as_str(),
                            managed_server = server_id,
                            "extension runtime not directly bundled, requesting managed LSP server runtime"
                        );
                        return Ok(None);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(None)
}
