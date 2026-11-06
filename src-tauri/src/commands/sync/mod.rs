//! 统一同步内核：连接（怎么传）与目标（传什么）分离。
//!
//! 资源库只是其中一个目标；WebDAV / S3 / SFTP 共用同一套清单与增量逻辑。
//! P2P 与 Syncthing 仍走旧路径，后续再适配到同样的 trait。

mod cloud;
mod engine;
mod local;
mod s3;
mod sftp;
mod webdav;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::commands::settingview;

pub use engine::run_target;

// ───────────────────────── 配置模型 ─────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SyncEngineConfig {
    #[serde(default)]
    pub connections: Vec<SyncConnection>,
    #[serde(default)]
    pub targets: Vec<SyncTarget>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SyncConnection {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// webdav | s3 | sftp
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub webdav: WebDavConnection,
    #[serde(default)]
    pub s3: S3Connection,
    #[serde(default)]
    pub sftp: SftpConnection,
    #[serde(default)]
    pub local: LocalConnection,
    #[serde(default)]
    pub cloud: CloudConnection,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct WebDavConnection {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct S3Connection {
    /// 例如 https://<account>.r2.cloudflarestorage.com 或 http://127.0.0.1:9000
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub bucket: String,
    #[serde(default)]
    pub access_key: String,
    #[serde(default)]
    pub secret_key: String,
    /// MinIO/R2 多数用路径风格；AWS 默认虚拟主机风格。
    #[serde(default = "default_true")]
    pub path_style: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SftpConnection {
    /// 复用「网络工具箱 → SSH 档案」，密码/密钥不重复保存。
    #[serde(default)]
    pub profile_id: String,
    /// 远端起始目录，留空使用登录后的家目录。
    #[serde(default)]
    pub root: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct LocalConnection {
    /// 本地目录、网络映射盘或网盘同步文件夹。
    #[serde(default)]
    pub root: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CloudConnection {
    /// baidu | aliyun | quark
    #[serde(default)]
    pub provider: String,
    /// 已授权的 access token；留空时用 refresh_token 自动换取。
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    /// 百度 AppKey / 阿里云 Client ID。
    #[serde(default)]
    pub client_id: String,
    /// 百度 SecretKey / 阿里云 Client Secret。
    #[serde(default)]
    pub client_secret: String,
    /// 阿里云盘 drive_id；留空自动取默认盘。
    #[serde(default)]
    pub drive_id: String,
    /// 百度应用目录名（/apps/<app_name>）。
    #[serde(default)]
    pub app_name: String,
    /// 夸克 Cookie（网页登录后复制）；官方开放 API 未公开，先走 Cookie 模式。
    #[serde(default)]
    pub cookie: String,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncTarget {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// resource_repo | knowledge_base | libraries | template | slidev | custom
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub local_path: String,
    #[serde(default)]
    pub connection_id: String,
    #[serde(default)]
    pub remote_path: String,
    /// upload | download | two_way
    #[serde(default = "default_direction")]
    pub direction: String,
    /// newer | local | remote
    #[serde(default = "default_conflict")]
    pub conflict_policy: String,
    #[serde(default)]
    pub delete_missing: bool,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_interval")]
    pub interval_minutes: u64,
    #[serde(default)]
    pub last_run_ms: i64,
    #[serde(default)]
    pub last_status: String,
    #[serde(default)]
    pub last_message: String,
}

impl Default for SyncTarget {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            scope: "custom".to_string(),
            local_path: String::new(),
            connection_id: String::new(),
            remote_path: String::new(),
            direction: default_direction(),
            conflict_policy: default_conflict(),
            delete_missing: false,
            enabled: true,
            interval_minutes: default_interval(),
            last_run_ms: 0,
            last_status: String::new(),
            last_message: String::new(),
        }
    }
}

fn default_direction() -> String {
    "upload".to_string()
}

fn default_conflict() -> String {
    "newer".to_string()
}

fn default_interval() -> u64 {
    30
}

// ───────────────────────── 远端抽象 ─────────────────────────

#[derive(Debug, Clone)]
pub struct RemoteEntry {
    pub key: String,
    pub size: u64,
    pub mtime_ms: i64,
}

#[async_trait]
pub trait SyncBackend: Send + Sync {
    /// 人类可读的连接结果，例如“WebDAV 连接正常”。
    async fn test(&self) -> Result<String, String>;
    /// 递归列出目标根下的所有文件（key 为相对路径，统一用 /）。
    async fn list(&self) -> Result<Vec<RemoteEntry>, String>;
    async fn stat(&self, key: &str) -> Result<Option<RemoteEntry>, String>;
    async fn get_file(&self, key: &str, local: &Path) -> Result<(), String>;
    async fn put_file(&self, key: &str, local: &Path) -> Result<(), String>;
    async fn delete(&self, key: &str) -> Result<(), String>;
    async fn mkdir(&self, key: &str) -> Result<(), String>;
    /// 同一后端可并行传输的请求数。
    fn concurrency(&self) -> usize {
        4
    }
}

pub(crate) fn normalize_key(raw: &str) -> String {
    raw.replace('\\', "/")
        .trim_matches('/')
        .split('/')
        .filter(|seg| !seg.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) fn encode_key(key: &str) -> String {
    key.split('/')
        .map(|seg| urlencoding::encode(seg).into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) fn parent_key(key: &str) -> String {
    match key.rfind('/') {
        Some(idx) => key[..idx].to_string(),
        None => String::new(),
    }
}

pub(crate) fn mtime_ms(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub(crate) fn is_temp_name(name: &str) -> bool {
    name.ends_with(".guaxi-part") || name == ".guaxi-sync.json" || name.starts_with(".~")
}

// ───────────────────────── 运行状态 ─────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SyncRunStatus {
    pub target_id: String,
    pub run_id: String,
    pub phase: String,
    pub status: String,
    pub message: String,
    pub current: u64,
    pub total: u64,
    pub file: String,
    pub bytes_current: u64,
    pub bytes_total: u64,
    pub speed_bps: u64,
    pub updated_at_ms: i64,
}

static STATUSES: Lazy<StdMutex<HashMap<String, SyncRunStatus>>> =
    Lazy::new(|| StdMutex::new(HashMap::new()));
static RUNNING: Lazy<StdMutex<HashSet<String>>> = Lazy::new(|| StdMutex::new(HashSet::new()));

pub(crate) fn set_status(app: &AppHandle, status: SyncRunStatus) {
    let payload = status.clone();
    if let Ok(mut map) = STATUSES.lock() {
        map.insert(status.target_id.clone(), status);
    }
    let _ = app.emit("sync:progress", payload);
}

pub(crate) fn finish_status(app: &AppHandle, status: SyncRunStatus) {
    let payload = status.clone();
    if let Ok(mut map) = STATUSES.lock() {
        map.insert(status.target_id.clone(), status.clone());
    }
    if let Ok(mut running) = RUNNING.lock() {
        running.remove(&status.target_id);
    }
    let _ = app.emit("sync:progress", payload.clone());
    let _ = app.emit("sync:done", payload);
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn mark_running(target_id: &str) -> Result<(), String> {
    let mut running = RUNNING.lock().map_err(|_| "同步状态锁失败")?;
    if !running.insert(target_id.to_string()) {
        return Err("该同步目标正在运行".to_string());
    }
    Ok(())
}

pub(crate) fn clear_running(target_id: &str) {
    if let Ok(mut running) = RUNNING.lock() {
        running.remove(target_id);
    }
}

// ───────────────────────── 路径与后端构建 ─────────────────────────

pub(crate) fn resolve_local_path(target: &SyncTarget) -> Result<PathBuf, String> {
    let explicit = target.local_path.trim();
    if !explicit.is_empty() {
        return Ok(PathBuf::from(explicit));
    }
    let path = match target.scope.as_str() {
        "resource_repo" => settingview::get_image_bed_root(),
        "knowledge_base" => settingview::get_knowledge_bases_root(),
        "libraries" => {
            let root = settingview::get_libraries_root();
            if root.trim().is_empty() {
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                exe.parent()
                    .map(|p| p.join("Libraries").to_string_lossy().to_string())
                    .unwrap_or_default()
            } else {
                root
            }
        }
        "template" => settingview::get_template_repo_path(),
        "slidev" => settingview::get_slidev_repo_root(),
        _ => String::new(),
    };
    if path.trim().is_empty() {
        return Err("同步目标还没有本地目录".to_string());
    }
    Ok(PathBuf::from(path))
}

pub(crate) fn state_file(target_id: &str) -> PathBuf {
    settingview::get_config_path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("sync_state")
        .join(format!("{}.json", target_id))
}

pub(crate) async fn build_backend(
    app: &AppHandle,
    connection: &SyncConnection,
    remote_root: &str,
) -> Result<Arc<dyn SyncBackend>, String> {
    match connection.kind.trim().to_lowercase().as_str() {
        "webdav" => Ok(Arc::new(webdav::WebDavBackend::new(
            connection,
            remote_root,
        )?)),
        "s3" => Ok(Arc::new(s3::S3Backend::new(connection, remote_root)?)),
        "sftp" => Ok(Arc::new(
            sftp::SftpBackend::connect(app, connection, remote_root).await?,
        )),
        "local" => Ok(Arc::new(local::LocalBackend::new(connection, remote_root)?)),
        "cloud" => Ok(Arc::new(cloud::CloudBackend::new(connection, remote_root)?)),
        other => Err(format!("暂不支持的同步后端：{}", other)),
    }
}

fn find_target(config: &SyncEngineConfig, target_id: &str) -> Result<SyncTarget, String> {
    config
        .targets
        .iter()
        .find(|t| t.id == target_id)
        .cloned()
        .ok_or_else(|| "同步目标不存在".to_string())
}

fn find_connection<'a>(
    config: &'a SyncEngineConfig,
    connection_id: &str,
) -> Result<&'a SyncConnection, String> {
    config
        .connections
        .iter()
        .find(|c| c.id == connection_id)
        .ok_or_else(|| "同步连接不存在".to_string())
}

pub(crate) fn persist_target_result(target_id: &str, status: &str, message: &str) {
    let mut config = settingview::load_config();
    if let Some(target) = config.sync_engine.targets.iter_mut().find(|t| t.id == target_id) {
        target.last_run_ms = now_ms();
        target.last_status = status.to_string();
        target.last_message = message.to_string();
        settingview::save_config(&config);
    }
}

// ───────────────────────── 命令 ─────────────────────────

/// 资源库是否已经绑定到新同步内核。返回 (目标, 连接)。
pub(crate) fn resource_repo_binding() -> Option<(SyncTarget, SyncConnection)> {
    let mut config = settingview::load_config();
    maybe_migrate_legacy(&mut config);
    let target = config
        .sync_engine
        .targets
        .iter()
        .find(|t| t.scope == "resource_repo" && t.enabled && !t.connection_id.is_empty())?;
    let connection = config
        .sync_engine
        .connections
        .iter()
        .find(|c| c.id == target.connection_id)?;
    if !matches!(
        connection.kind.trim().to_lowercase().as_str(),
        "webdav" | "s3" | "sftp" | "local" | "cloud"
    ) {
        return None;
    }
    Some((target.clone(), connection.clone()))
}

#[tauri::command]
pub fn sync_engine_get_config() -> SyncEngineConfig {
    let mut config = settingview::load_config();
    maybe_migrate_legacy(&mut config);
    config.sync_engine
}

/// 旧 WebDAV 配置自动变成“一条连接 + 一条资源库目标”，迁移后旧字段仍保留但不再参与同步。
fn maybe_migrate_legacy(config: &mut settingview::AppConfig) {
    if !config.sync_engine.connections.is_empty() || !config.sync_engine.targets.is_empty() {
        return;
    }
    let (legacy, resource_root) = settingview::get_legacy_sync_snapshot();
    if legacy.provider.trim().eq_ignore_ascii_case("webdav")
        && !legacy.webdav.url.trim().is_empty()
    {
        let connection_id = Uuid::new_v4().to_string();
        config.sync_engine.connections.push(SyncConnection {
            id: connection_id.clone(),
            name: "WebDAV".to_string(),
            kind: "webdav".to_string(),
            webdav: WebDavConnection {
                url: legacy.webdav.url.clone(),
                username: legacy.webdav.username.clone(),
                password: legacy.webdav.password.clone(),
            },
            ..Default::default()
        });
        config.sync_engine.targets.push(SyncTarget {
            id: Uuid::new_v4().to_string(),
            name: "资源库".to_string(),
            scope: "resource_repo".to_string(),
            local_path: resource_root,
            connection_id,
            remote_path: legacy.webdav.remote_base_path.clone(),
            direction: if legacy.prefer_pull {
                "download".to_string()
            } else {
                "upload".to_string()
            },
            enabled: true,
            ..Default::default()
        });
        settingview::save_config(config);
    }
}

#[tauri::command]
pub fn sync_engine_set_config(config_input: SyncEngineConfig) -> Result<(), String> {
    for connection in &config_input.connections {
        if connection.id.trim().is_empty() {
            return Err("同步连接缺少 id".to_string());
        }
    }
    for target in &config_input.targets {
        if target.id.trim().is_empty() {
            return Err("同步目标缺少 id".to_string());
        }
        if !config_input
            .connections
            .iter()
            .any(|c| c.id == target.connection_id)
        {
            return Err(format!("目标「{}」没有有效的连接", target.name));
        }
    }
    let mut config = settingview::load_config();
    config.sync_engine = config_input;
    settingview::save_config(&config);
    Ok(())
}

#[tauri::command]
pub fn sync_engine_statuses() -> Vec<SyncRunStatus> {
    STATUSES
        .lock()
        .map(|map| map.values().cloned().collect())
        .unwrap_or_default()
}

#[tauri::command]
pub async fn sync_engine_test_connection(
    app: AppHandle,
    connection_id: Option<String>,
    connection: Option<SyncConnection>,
) -> Result<String, String> {
    let config = settingview::load_config();
    let conn = match connection {
        Some(value) => value,
        None => {
            let id = connection_id.ok_or_else(|| "缺少连接".to_string())?;
            find_connection(&config.sync_engine, &id)?.clone()
        }
    };
    let backend = build_backend(&app, &conn, "").await?;
    backend.test().await
}

#[tauri::command]
pub fn sync_engine_run_target(app: AppHandle, target_id: String) -> Result<String, String> {
    let config = settingview::load_config();
    let target = find_target(&config.sync_engine, &target_id)?;
    let connection = find_connection(&config.sync_engine, &target.connection_id)?.clone();
    mark_running(&target_id)?;
    let run_id = Uuid::new_v4().to_string();
    let run_id_out = run_id.clone();
    let app_for_task = app.clone();
    tauri::async_runtime::spawn(async move {
        match run_target(&app_for_task, &target, &connection, &run_id).await {
            Ok(report) => {
                let status = SyncRunStatus {
                    target_id: target.id.clone(),
                    run_id: run_id.clone(),
                    phase: "done".to_string(),
                    status: "success".to_string(),
                    message: report.message.clone(),
                    current: report.current,
                    total: report.total,
                    ..Default::default()
                };
                persist_target_result(&target.id, "success", &report.message);
                finish_status(&app_for_task, status);
            }
            Err(error) => {
                persist_target_result(&target.id, "error", &error);
                finish_status(
                    &app_for_task,
                    SyncRunStatus {
                        target_id: target.id.clone(),
                        run_id: run_id.clone(),
                        phase: "done".to_string(),
                        status: "error".to_string(),
                        message: error,
                        ..Default::default()
                    },
                );
            }
        }
    });
    Ok(run_id_out)
}

/// 后台调度：每分钟检查一次，按每个目标自己的间隔触发。
/// 不在这里做全局开关——目标上的 enabled 就是开关。
pub async fn spawn_scheduler(app: AppHandle) {
    // 启动后先等一会儿，避免和窗口初始化、首屏扫描抢资源。
    tokio::time::sleep(std::time::Duration::from_secs(45)).await;
    loop {
        let config = settingview::load_config();
        let now = now_ms();
        for target in config.sync_engine.targets.iter().filter(|t| t.enabled) {
            let interval = (target.interval_minutes.max(5) as i64) * 60_000;
            if target.last_run_ms > 0 && now - target.last_run_ms < interval {
                continue;
            }
            if mark_running(&target.id).is_err() {
                continue;
            }
            let connection = match find_connection(&config.sync_engine, &target.connection_id) {
                Ok(value) => value.clone(),
                Err(_) => {
                    clear_running(&target.id);
                    continue;
                }
            };
            let target = target.clone();
            let app = app.clone();
            let run_id = Uuid::new_v4().to_string();
            tauri::async_runtime::spawn(async move {
                match run_target(&app, &target, &connection, &run_id).await {
                    Ok(report) => {
                        persist_target_result(&target.id, "success", &report.message);
                        finish_status(
                            &app,
                            SyncRunStatus {
                                target_id: target.id.clone(),
                                run_id,
                                phase: "done".to_string(),
                                status: "success".to_string(),
                                message: report.message,
                                current: report.current,
                                total: report.total,
                                ..Default::default()
                            },
                        );
                    }
                    Err(error) => {
                        persist_target_result(&target.id, "error", &error);
                        finish_status(
                            &app,
                            SyncRunStatus {
                                target_id: target.id.clone(),
                                run_id,
                                phase: "done".to_string(),
                                status: "error".to_string(),
                                message: error,
                                ..Default::default()
                            },
                        );
                    }
                }
            });
        }
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}

#[tauri::command]
pub fn sync_engine_run_all(app: AppHandle) -> Result<Vec<String>, String> {
    let config = settingview::load_config();
    let mut started = Vec::new();
    for target in config.sync_engine.targets.iter().filter(|t| t.enabled) {
        let app = app.clone();
        if mark_running(&target.id).is_err() {
            continue;
        }
        let target = target.clone();
        let connection = match find_connection(&config.sync_engine, &target.connection_id) {
            Ok(value) => value.clone(),
            Err(_) => {
                clear_running(&target.id);
                continue;
            }
        };
        let run_id = Uuid::new_v4().to_string();
        started.push(run_id.clone());
        tauri::async_runtime::spawn(async move {
            match run_target(&app, &target, &connection, &run_id).await {
                Ok(report) => {
                    persist_target_result(&target.id, "success", &report.message);
                    finish_status(
                        &app,
                        SyncRunStatus {
                            target_id: target.id.clone(),
                            run_id: run_id.clone(),
                            phase: "done".to_string(),
                            status: "success".to_string(),
                            message: report.message,
                            current: report.current,
                            total: report.total,
                            ..Default::default()
                        },
                    );
                }
                Err(error) => {
                    persist_target_result(&target.id, "error", &error);
                    finish_status(
                        &app,
                        SyncRunStatus {
                            target_id: target.id.clone(),
                            run_id: run_id.clone(),
                            phase: "done".to_string(),
                            status: "error".to_string(),
                            message: error,
                            ..Default::default()
                        },
                    );
                }
            }
        });
    }
    Ok(started)
}

// 仅供引擎内部使用。
pub(crate) fn emit_progress(app: &AppHandle, status: SyncRunStatus) {
    set_status(app, status);
}
