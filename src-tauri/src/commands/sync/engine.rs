use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tracing::{info, warn};
use walkdir::WalkDir;

use super::{
    RemoteEntry, SyncBackend, SyncConnection, SyncRunStatus, SyncTarget, build_backend,
    emit_progress, is_temp_name, parent_key, resolve_local_path, state_file,
};

#[derive(Default, Debug)]
pub struct SyncReport {
    pub message: String,
    pub current: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct StateEntry {
    #[serde(default)]
    local_size: u64,
    #[serde(default)]
    local_mtime_ms: i64,
    #[serde(default)]
    remote_size: u64,
    #[serde(default)]
    remote_mtime_ms: i64,
}

#[derive(Default, Serialize, Deserialize)]
struct SyncState {
    #[serde(default)]
    entries: HashMap<String, StateEntry>,
}

#[derive(Debug, Clone)]
struct LocalEntry {
    size: u64,
    mtime_ms: i64,
}

#[derive(Debug, Clone)]
enum SyncOp {
    Upload { key: String },
    Download { key: String },
    Conflict { key: String },
    DeleteRemote { key: String },
    DeleteLocal { key: String },
}

impl SyncOp {
    fn key(&self) -> &str {
        match self {
            SyncOp::Upload { key }
            | SyncOp::Download { key }
            | SyncOp::Conflict { key }
            | SyncOp::DeleteRemote { key }
            | SyncOp::DeleteLocal { key } => key,
        }
    }

    fn transfers(&self) -> bool {
        matches!(
            self,
            SyncOp::Upload { .. } | SyncOp::Download { .. } | SyncOp::Conflict { .. }
        )
    }
}

struct Progress {
    app: AppHandle,
    target_id: String,
    run_id: String,
    total: AtomicU64,
    current: AtomicU64,
    bytes_total: AtomicU64,
    bytes_done: AtomicU64,
    started: std::time::Instant,
}

impl Progress {
    fn new(app: AppHandle, target_id: String, run_id: String) -> Self {
        Self {
            app,
            target_id,
            run_id,
            total: AtomicU64::new(0),
            current: AtomicU64::new(0),
            bytes_total: AtomicU64::new(0),
            bytes_done: AtomicU64::new(0),
            started: std::time::Instant::now(),
        }
    }

    fn emit(&self, phase: &str, status: &str, message: String, file: String) {
        let elapsed = self.started.elapsed().as_secs_f64().max(0.001);
        let bytes = self.bytes_done.load(Ordering::Relaxed);
        emit_progress(
            &self.app,
            SyncRunStatus {
                target_id: self.target_id.clone(),
                run_id: self.run_id.clone(),
                phase: phase.to_string(),
                status: status.to_string(),
                message,
                current: self.current.load(Ordering::Relaxed),
                total: self.total.load(Ordering::Relaxed),
                file,
                bytes_current: bytes,
                bytes_total: self.bytes_total.load(Ordering::Relaxed),
                speed_bps: (bytes as f64 / elapsed) as u64,
                updated_at_ms: chrono::Utc::now().timestamp_millis(),
            },
        );
    }

    fn advance(&self, file: String, bytes: u64) {
        self.current.fetch_add(1, Ordering::Relaxed);
        self.bytes_done.fetch_add(bytes, Ordering::Relaxed);
        self.emit("transferring", "running", String::new(), file);
    }
}

pub async fn run_target(
    app: &AppHandle,
    target: &SyncTarget,
    connection: &SyncConnection,
    run_id: &str,
) -> Result<SyncReport, String> {
    let local_root = resolve_local_path(target)?;
    if !local_root.is_dir() {
        return Err(format!("本地目录不存在：{}", local_root.to_string_lossy()));
    }
    let local_root = local_root
        .canonicalize()
        .unwrap_or_else(|_| local_root.clone());

    let progress = Arc::new(Progress::new(
        app.clone(),
        target.id.clone(),
        run_id.to_string(),
    ));
    progress.emit("scanning", "running", "正在扫描本地文件".to_string(), String::new());

    let local = scan_local(&local_root)?;
    info!("[Sync] local scan target={} files={}", target.id, local.len());

    let backend = build_backend(app, connection, &target.remote_path).await?;
    let _ = backend.mkdir("").await;
    progress.emit("listing", "running", "正在读取远端清单".to_string(), String::new());
    let remote = backend
        .list()
        .await?
        .into_iter()
        .map(|entry| (entry.key.clone(), entry))
        .collect::<HashMap<String, RemoteEntry>>();

    let state = load_state(&state_file(&target.id));
    let direction = target.direction.as_str();
    let (ops, mut keep_state) = plan(&local, &remote, &state, target);

    let bytes_total = ops
        .iter()
        .filter(|op| op.transfers())
        .map(|op| local.get(op.key()).map(|v| v.size).unwrap_or(0))
        .sum();
    progress.total.store(ops.len() as u64, Ordering::Relaxed);
    progress.bytes_total.store(bytes_total, Ordering::Relaxed);
    progress.emit(
        "transferring",
        "running",
        format!("需要传输 {} 项", ops.len()),
        String::new(),
    );

    // 远端目录先一次性建好，再并发传文件；SFTP/WebDAV 不会再为每个文件建目录。
    let mut dirs = HashSet::new();
    for op in &ops {
        if matches!(op, SyncOp::Upload { .. } | SyncOp::Conflict { .. }) {
            let mut parent = parent_key(op.key());
            while !parent.is_empty() {
                dirs.insert(parent.clone());
                parent = parent_key(&parent);
            }
        }
    }
    let mut dirs: Vec<String> = dirs.into_iter().collect();
    dirs.sort_by_key(|d| d.matches('/').count());
    for dir in dirs {
        if let Err(error) = backend.mkdir(&dir).await {
            warn!("[Sync] mkdir {} failed: {}", dir, error);
        }
    }

    let errors = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let succeeded = Arc::new(std::sync::Mutex::new(HashSet::<String>::new()));
    let failed_keys = Arc::new(std::sync::Mutex::new(HashSet::<String>::new()));
    let local_root = Arc::new(local_root);
    let local = Arc::new(local);
    let concurrency = backend.concurrency().max(1);

    let mut results = futures_util::stream::iter(ops.into_iter().map(|op| {
        let backend = Arc::clone(&backend);
        let progress = Arc::clone(&progress);
        let errors = Arc::clone(&errors);
        let succeeded = Arc::clone(&succeeded);
        let failed_keys = Arc::clone(&failed_keys);
        let local_root = Arc::clone(&local_root);
        let local = Arc::clone(&local);
        async move {
            let key = op.key().to_string();
            let size = if op.transfers() {
                local.get(&key).map(|v| v.size).unwrap_or(0)
            } else {
                0
            };
            match execute_op(&backend, &local_root, op).await {
                Ok(()) => {
                    if let Ok(mut set) = succeeded.lock() {
                        set.insert(key.clone());
                    }
                    progress.advance(key, size);
                }
                Err(error) => {
                    warn!("[Sync] op failed: {}", error);
                    if let Ok(mut set) = failed_keys.lock() {
                        set.insert(key.clone());
                    }
                    if let Ok(mut list) = errors.lock() {
                        if list.len() < 20 {
                            list.push(format!("{}: {}", key, error));
                        }
                    }
                    progress.advance(key, 0);
                }
            }
        }
    }))
    .buffer_unordered(concurrency);
    while results.next().await.is_some() {}

    let failed = errors.lock().map(|v| v.clone()).unwrap_or_default();
    let succeeded = succeeded.lock().map(|v| v.clone()).unwrap_or_default();
    let failed_keys = failed_keys.lock().map(|v| v.clone()).unwrap_or_default();

    // 失败项保留旧快照，下次仍然会被判成“有变化”；成功项用实际远端/本地属性回填。
    for key in &failed_keys {
        if let Some(old) = state.entries.get(key) {
            keep_state.entries.insert(key.clone(), old.clone());
        }
    }
    for key in succeeded {
        if let Some(entry) = verify_entry(&backend, &local_root, &key).await {
            keep_state.entries.insert(key, entry);
        }
    }
    save_state(&state_file(&target.id), &keep_state)?;

    let message = if failed.is_empty() {
        format!(
            "同步完成：{} 项（{}）",
            progress.current.load(Ordering::Relaxed),
            direction_label(direction)
        )
    } else {
        format!(
            "完成 {} 项，失败 {} 项：{}",
            progress.current.load(Ordering::Relaxed),
            failed.len(),
            failed.first().cloned().unwrap_or_default()
        )
    };
    info!("[Sync] target={} {}", target.id, message);
    progress.emit(
        "done",
        if failed.is_empty() { "success" } else { "error" },
        message.clone(),
        String::new(),
    );
    Ok(SyncReport {
        message,
        current: progress.current.load(Ordering::Relaxed),
        total: progress.total.load(Ordering::Relaxed),
    })
}

/// 传输完成后按实际两端属性回填快照，避免“刚传完下次又被判成变了”。
async fn verify_entry(
    backend: &Arc<dyn SyncBackend>,
    local_root: &Path,
    key: &str,
) -> Option<StateEntry> {
    let local_meta = tokio::fs::metadata(local_root.join(key)).await.ok();
    let remote = backend.stat(key).await.ok().flatten();
    match (local_meta, remote) {
        (Some(meta), Some(remote)) => Some(StateEntry {
            local_size: meta.len(),
            local_mtime_ms: super::mtime_ms(&meta),
            remote_size: remote.size,
            remote_mtime_ms: remote.mtime_ms,
        }),
        (Some(meta), None) => Some(StateEntry {
            local_size: meta.len(),
            local_mtime_ms: super::mtime_ms(&meta),
            ..Default::default()
        }),
        (None, Some(remote)) => Some(StateEntry {
            remote_size: remote.size,
            remote_mtime_ms: remote.mtime_ms,
            ..Default::default()
        }),
        (None, None) => None,
    }
}

async fn execute_op(
    backend: &Arc<dyn SyncBackend>,
    local_root: &Path,
    op: SyncOp,
) -> Result<(), String> {
    match op {
        SyncOp::Upload { key } => backend.put_file(&key, &local_root.join(&key)).await,
        SyncOp::Download { key } => {
            let path = local_root.join(&key);
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            let temp = temp_path(&path);
            backend.get_file(&key, &temp).await?;
            tokio::fs::rename(&temp, &path)
                .await
                .map_err(|e| format!("替换本地文件失败：{}", e))?;
            Ok(())
        }
        SyncOp::Conflict { key } => {
            // 两边都改过：远端版本落到冲突副本，本地版本继续作为主文件并覆盖远端。
            let source = local_root.join(&key);
            let conflict = conflict_path(&source);
            backend.get_file(&key, &conflict).await?;
            backend.put_file(&key, &source).await
        }
        SyncOp::DeleteRemote { key } => backend.delete(&key).await,
        SyncOp::DeleteLocal { key } => {
            let path = local_root.join(&key);
            match tokio::fs::remove_file(&path).await {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(format!("删除本地文件失败：{}", error)),
            }
        }
    }
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().and_then(|v| v.to_str()).unwrap_or("file").to_string();
    name.push_str(".guaxi-part");
    path.with_file_name(name)
}

fn conflict_path(source: &Path) -> PathBuf {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let stem = source
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("file");
    let ext = source.extension().and_then(|v| v.to_str()).unwrap_or("");
    let name = if ext.is_empty() {
        format!("{}.conflict-{}", stem, stamp)
    } else {
        format!("{}.conflict-{}.{}", stem, stamp, ext)
    };
    source.with_file_name(name)
}

fn plan(
    local: &HashMap<String, LocalEntry>,
    remote: &HashMap<String, RemoteEntry>,
    state: &SyncState,
    target: &SyncTarget,
) -> (Vec<SyncOp>, SyncState) {
    let mut ops = Vec::new();
    let mut keep = SyncState::default();
    let direction = target.direction.as_str();
    let mut keys: HashSet<String> = local.keys().cloned().collect();
    keys.extend(remote.keys().cloned());

    for key in keys {
        let l = local.get(&key);
        let r = remote.get(&key);
        let s = state.entries.get(&key);
        match (l, r) {
            (Some(l), Some(r)) => {
                if l.size == r.size && close_mtime(l.mtime_ms, r.mtime_ms) {
                    keep.entries.insert(key, entries_of(l, r));
                    continue;
                }
                let local_changed = s
                    .map(|s| s.local_size != l.size || !close_mtime(s.local_mtime_ms, l.mtime_ms))
                    .unwrap_or(true);
                let remote_changed = s
                    .map(|s| s.remote_size != r.size || !close_mtime(s.remote_mtime_ms, r.mtime_ms))
                    .unwrap_or(true);

                let mut pushed = false;
                match direction {
                    "upload" => {
                        if local_changed || s.is_none() {
                            ops.push(SyncOp::Upload { key: key.clone() });
                            pushed = true;
                        }
                    }
                    "download" => {
                        if remote_changed || s.is_none() {
                            ops.push(SyncOp::Download { key: key.clone() });
                            pushed = true;
                        }
                    }
                    _ => {
                        if s.is_none() || (local_changed && remote_changed) {
                            match target.conflict_policy.as_str() {
                                "local" => ops.push(SyncOp::Upload { key: key.clone() }),
                                "remote" => ops.push(SyncOp::Download { key: key.clone() }),
                                _ => ops.push(SyncOp::Conflict { key: key.clone() }),
                            }
                            pushed = true;
                        } else if local_changed {
                            ops.push(SyncOp::Upload { key: key.clone() });
                            pushed = true;
                        } else if remote_changed {
                            ops.push(SyncOp::Download { key: key.clone() });
                            pushed = true;
                        }
                    }
                }
                // 有操作的项由传输完成后的实际属性回填；这里只保留真正无需动作的项。
                if !pushed {
                    keep.entries.insert(key, entries_of(l, r));
                }
            }
            (Some(_l), None) => {
                if target.delete_missing && direction != "upload" {
                    ops.push(SyncOp::DeleteLocal { key: key.clone() });
                } else if direction != "download" {
                    ops.push(SyncOp::Upload { key: key.clone() });
                }
            }
            (None, Some(_r)) => {
                if target.delete_missing && direction != "download" {
                    ops.push(SyncOp::DeleteRemote { key: key.clone() });
                } else if direction != "upload" {
                    ops.push(SyncOp::Download { key: key.clone() });
                }
            }
            (None, None) => {}
        }
    }
    (ops, keep)
}

fn entries_of(local: &LocalEntry, remote: &RemoteEntry) -> StateEntry {
    StateEntry {
        local_size: local.size,
        local_mtime_ms: local.mtime_ms,
        remote_size: remote.size,
        remote_mtime_ms: remote.mtime_ms,
    }
}

fn close_mtime(a: i64, b: i64) -> bool {
    // 部分 WebDAV/S3 实现不返回精确 mtime，0 视为“未知”，交给大小判断。
    a == 0 || b == 0 || (a - b).abs() <= 2000
}

fn direction_label(direction: &str) -> &'static str {
    match direction {
        "download" => "拉取",
        "two_way" => "双向",
        _ => "推送",
    }
}

fn scan_local(root: &Path) -> Result<HashMap<String, LocalEntry>, String> {
    let mut out = HashMap::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if is_temp_name(&name) {
            continue;
        }
        let rel = match entry.path().strip_prefix(root) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let key = rel
            .to_string_lossy()
            .replace('\\', "/")
            .trim_matches('/')
            .to_string();
        if key.is_empty() {
            continue;
        }
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        out.insert(
            key,
            LocalEntry {
                size: meta.len(),
                mtime_ms: super::mtime_ms(&meta),
            },
        );
    }
    Ok(out)
}

fn load_state(path: &Path) -> SyncState {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_state(path: &Path, state: &SyncState) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temp = path.with_extension("json.tmp");
    let raw = serde_json::to_vec_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(&temp, raw).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(direction: &str, delete_missing: bool) -> SyncTarget {
        SyncTarget {
            id: "t".to_string(),
            direction: direction.to_string(),
            delete_missing,
            conflict_policy: "newer".to_string(),
            ..Default::default()
        }
    }

    fn local(size: u64, mtime: i64) -> HashMap<String, LocalEntry> {
        HashMap::from([("a.txt".to_string(), LocalEntry { size, mtime_ms: mtime })])
    }

    fn remote(size: u64, mtime: i64) -> HashMap<String, RemoteEntry> {
        HashMap::from([(
            "a.txt".to_string(),
            RemoteEntry { key: "a.txt".to_string(), size, mtime_ms: mtime },
        )])
    }

    #[test]
    fn upload_mode_pushes_new_local_file() {
        let (ops, _) = plan(
            &local(10, 1000),
            &HashMap::new(),
            &SyncState::default(),
            &target("upload", false),
        );
        assert!(matches!(ops.as_slice(), [SyncOp::Upload { .. }]));
    }

    #[test]
    fn download_mode_pulls_new_remote_file() {
        let (ops, _) = plan(
            &HashMap::new(),
            &remote(10, 1000),
            &SyncState::default(),
            &target("download", false),
        );
        assert!(matches!(ops.as_slice(), [SyncOp::Download { .. }]));
    }

    #[test]
    fn two_way_conflicts_when_both_sides_changed() {
        let mut state = SyncState::default();
        state.entries.insert(
            "a.txt".to_string(),
            StateEntry {
                local_size: 5,
                local_mtime_ms: 500,
                remote_size: 5,
                remote_mtime_ms: 500,
            },
        );
        let (ops, _) = plan(
            &local(10, 1000),
            &remote(12, 2000),
            &state,
            &target("two_way", false),
        );
        assert!(matches!(ops.as_slice(), [SyncOp::Conflict { .. }]));
    }

    #[test]
    fn two_way_skips_unchanged_file_and_keeps_state() {
        let mut state = SyncState::default();
        state.entries.insert(
            "a.txt".to_string(),
            StateEntry {
                local_size: 10,
                local_mtime_ms: 1000,
                remote_size: 10,
                remote_mtime_ms: 1000,
            },
        );
        let (ops, keep) = plan(
            &local(10, 1000),
            &remote(10, 1000),
            &state,
            &target("two_way", false),
        );
        assert!(ops.is_empty());
        assert!(keep.entries.contains_key("a.txt"));
    }

    #[test]
    fn delete_missing_removes_remote_when_local_was_deleted() {
        let mut state = SyncState::default();
        state.entries.insert(
            "a.txt".to_string(),
            StateEntry {
                local_size: 10,
                local_mtime_ms: 1000,
                remote_size: 10,
                remote_mtime_ms: 1000,
            },
        );
        let (ops, _) = plan(
            &HashMap::new(),
            &remote(10, 1000),
            &state,
            &target("upload", true),
        );
        assert!(matches!(ops.as_slice(), [SyncOp::DeleteRemote { .. }]));
    }

    #[test]
    fn unknown_remote_mtime_does_not_force_conflict() {
        assert!(close_mtime(0, 1234));
        assert!(close_mtime(1000, 1500));
        assert!(!close_mtime(1000, 5000));
    }
}
