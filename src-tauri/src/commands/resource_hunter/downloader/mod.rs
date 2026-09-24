//! 下载引擎：全局任务调度器 + 控制信号。
//! - HTTP 多线程分块续传在 http.rs；yt-dlp 进程下载在 ytdlp.rs。
//! - 任务在内存中持有 Control（暂停/取消/进度），周期性落盘到 store。
use super::model::{DownloadStatus, DownloadTask, ResourceKind};
use super::store;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, Semaphore};

pub mod http;
pub mod torrent;
pub mod ytdlp;

pub const EVENT_TASK_UPDATE: &str = "resource_hunter://download_update";

/// 全局最大并发下载数。
const MAX_CONCURRENT: usize = 3;

/// 运行时控制信号：0=运行 1=暂停 2=取消。
pub struct Control {
    pub state: AtomicU8,
    pub downloaded: AtomicU64,
    pub total: AtomicU64,
    pub speed: AtomicU64,
    /// 下载起始时刻(ms since epoch)，0 表示尚未开始；限速节流用。
    pub started_ms: AtomicU64,
    /// 每分块已下字节，支持多连接续传。
    pub chunk_done: Vec<AtomicU64>,
}

impl Control {
    pub fn new(total: u64, chunks: usize) -> Arc<Self> {
        Arc::new(Control {
            state: AtomicU8::new(0),
            downloaded: AtomicU64::new(0),
            total: AtomicU64::new(total),
            speed: AtomicU64::new(0),
            started_ms: AtomicU64::new(0),
            chunk_done: (0..chunks.max(1)).map(|_| AtomicU64::new(0)).collect(),
        })
    }
    pub fn is_paused(&self) -> bool {
        self.state.load(Ordering::Relaxed) == 1
    }
    pub fn is_cancelled(&self) -> bool {
        self.state.load(Ordering::Relaxed) == 2
    }
}

struct Entry {
    task: DownloadTask,
    control: Arc<Control>,
}

type Registry = Mutex<HashMap<String, Entry>>;

fn registry() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

fn sem() -> &'static Semaphore {
    static S: OnceLock<Semaphore> = OnceLock::new();
    S.get_or_init(|| Semaphore::new(MAX_CONCURRENT))
}

/// 快照当前所有任务（供前端列表）。
pub async fn list_tasks() -> Vec<DownloadTask> {
    let reg = registry().lock().await;
    let mut v: Vec<DownloadTask> = reg.values().map(|e| e.task.clone()).collect();
    v.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    v
}

/// 入队并启动一个下载任务。
pub async fn enqueue(app: AppHandle, mut task: DownloadTask) -> Result<String, String> {
    task.status = DownloadStatus::Queued;
    let control = Control::new(task.total_size, task.chunks.len().max(1));
    // 恢复已下进度到控制器。
    for (i, c) in task.chunks.iter().enumerate() {
        if let Some(a) = control.chunk_done.get(i) {
            a.store(c.downloaded, Ordering::Relaxed);
        }
    }
    control
        .downloaded
        .store(task.downloaded, Ordering::Relaxed);
    let id = task.id.clone();
    let _ = store::save_task(&task);
    {
        let mut reg = registry().lock().await;
        reg.insert(
            id.clone(),
            Entry {
                task: task.clone(),
                control: control.clone(),
            },
        );
    }
    spawn_worker(app, id.clone(), control);
    Ok(id)
}

fn spawn_worker(app: AppHandle, id: String, control: Arc<Control>) {
    tokio::spawn(async move {
        let permit = sem().acquire().await;
        if permit.is_err() || control.is_cancelled() {
            return;
        }
        set_status(&app, &id, DownloadStatus::Active, None).await;

        let task = {
            let reg = registry().lock().await;
            match reg.get(&id) {
                Some(e) => e.task.clone(),
                None => return,
            }
        };

        let result = if task.via_ytdlp {
            ytdlp::download(&app, &task, &control).await
        } else if task.magnet.is_some() {
            torrent::download(&app, &task, &control).await
        } else {
            http::download(&app, &task, &control).await
        };

        match result {
            Ok(local_path) => {
                finish_ok(&app, &id, local_path).await;
            }
            Err(e) => {
                if control.is_cancelled() {
                    remove(&app, &id).await;
                } else if control.is_paused() {
                    set_status(&app, &id, DownloadStatus::Paused, None).await;
                } else {
                    set_status(&app, &id, DownloadStatus::Error, Some(e)).await;
                }
            }
        }
    });
}

async fn finish_ok(app: &AppHandle, id: &str, local_path: String) {
    let mut reg = registry().lock().await;
    if let Some(e) = reg.get_mut(id) {
        e.task.status = DownloadStatus::Completed;
        e.task.dest = local_path.clone();
        e.task.downloaded = e.task.total_size;
        let _ = store::save_task(&e.task);
        // 完成即入库，便于资源库管理与用现成 reader 打开。
        let item = super::model::ResourceItem {
            id: e.task.id.clone(),
            title: e.task.title.clone(),
            kind: e.task.kind,
            source: e.task.source.clone(),
            source_url: e.task.source_url.clone().unwrap_or_default(),
            download_url: e.task.urls.first().cloned(),
            magnet: None,
            needs_ytdlp: e.task.via_ytdlp,
            format: None,
            thumbnail: None,
            author: None,
            published_at: None,
            summary: None,
            size_hint: Some(e.task.total_size),
            relevance: 1.0,
            tags: vec![],
        };
        let _ = store::upsert_resource(&item, Some(&local_path));
        let _ = app.emit(EVENT_TASK_UPDATE, e.task.clone());
    }
}

async fn set_status(app: &AppHandle, id: &str, status: DownloadStatus, err: Option<String>) {
    let mut reg = registry().lock().await;
    if let Some(e) = reg.get_mut(id) {
        e.task.status = status;
        e.task.error = err;
        let _ = store::save_task(&e.task);
        let _ = app.emit(EVENT_TASK_UPDATE, e.task.clone());
    }
}

async fn remove(app: &AppHandle, id: &str) {
    registry().lock().await.remove(id);
    let _ = store::delete_task(id);
    let _ = app.emit(EVENT_TASK_UPDATE, serde_json::json!({ "id": id, "removed": true }));
}

/// 进度回写：由传输层周期调用，更新任务并节流上报前端。
pub async fn report_progress(app: &AppHandle, id: &str, downloaded: u64, speed: u64, eta: Option<u64>) {
    let mut reg = registry().lock().await;
    if let Some(e) = reg.get_mut(id) {
        e.task.downloaded = downloaded;
        e.task.speed = speed;
        e.task.eta_secs = eta;
        e.task.status = DownloadStatus::Active;
        let _ = app.emit(EVENT_TASK_UPDATE, e.task.clone());
    }
}

/// 回写分块进度（暂停/周期落盘），供续传恢复。
pub async fn persist_chunks(id: &str, chunks: Vec<super::model::ChunkState>, downloaded: u64) {
    let mut reg = registry().lock().await;
    if let Some(e) = reg.get_mut(id) {
        e.task.chunks = chunks;
        e.task.downloaded = downloaded;
        let _ = store::save_task(&e.task);
    }
}

/// 记录探测到的总大小（首次拿到 Content-Length 时）。
pub async fn set_total(id: &str, total: u64) {
    let mut reg = registry().lock().await;
    if let Some(e) = reg.get_mut(id) {
        e.task.total_size = total;
    }
}

pub async fn pause(id: &str) -> Result<(), String> {
    let reg = registry().lock().await;
    if let Some(e) = reg.get(id) {
        e.control.state.store(1, Ordering::Relaxed);
    }
    Ok(())
}

pub async fn resume(app: AppHandle, id: String) -> Result<(), String> {
    let control = {
        let mut reg = registry().lock().await;
        match reg.get_mut(&id) {
            Some(e) => {
                if e.task.status != DownloadStatus::Paused && e.task.status != DownloadStatus::Error
                {
                    return Ok(());
                }
                e.control.state.store(0, Ordering::Relaxed);
                e.control.clone()
            }
            None => return Err("任务不存在".into()),
        }
    };
    spawn_worker(app, id, control);
    Ok(())
}

pub async fn cancel(app: AppHandle, id: String) -> Result<(), String> {
    {
        let reg = registry().lock().await;
        if let Some(e) = reg.get(&id) {
            e.control.state.store(2, Ordering::Relaxed);
        }
    }
    remove(&app, &id).await;
    Ok(())
}

/// 启动时从库中恢复未完成任务（置为暂停，等待用户继续）。
pub async fn restore_from_store(app: &AppHandle) {
    if let Ok(tasks) = store::load_tasks() {
        let mut reg = registry().lock().await;
        for mut t in tasks {
            if t.status == DownloadStatus::Active || t.status == DownloadStatus::Queued {
                t.status = DownloadStatus::Paused;
            }
            let control = Control::new(t.total_size, t.chunks.len().max(1));
            control.downloaded.store(t.downloaded, Ordering::Relaxed);
            reg.insert(t.id.clone(), Entry { task: t, control });
        }
    }
    let _ = app;
}

#[allow(dead_code)]
fn _kind_touch(_: ResourceKind) {}
