//! 磁力 / BT 下载：进程内 librqbit（rustls+ring 后端）。所有磁力任务共用一个 Session，
//! 落地到统一下载目录，进度并入下载队列。
use super::super::store;
use super::Control;
use crate::commands::resource_hunter::model::DownloadTask;
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, Session};
use once_cell::sync::OnceCell;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tokio::sync::Mutex as AsyncMutex;

static SESSION: OnceCell<AsyncMutex<Option<Arc<Session>>>> = OnceCell::new();

async fn session(app: &AppHandle) -> Result<Arc<Session>, String> {
    let cell = SESSION.get_or_init(|| AsyncMutex::new(None));
    let mut guard = cell.lock().await;
    if let Some(s) = guard.as_ref() {
        return Ok(s.clone());
    }
    let dir = store::downloads_dir(app)?;
    let s = Session::new(dir)
        .await
        .map_err(|e| format!("启动 BT 会话失败: {e}"))?;
    *guard = Some(s.clone());
    Ok(s)
}

pub async fn download(
    app: &AppHandle,
    task: &DownloadTask,
    control: &Arc<Control>,
) -> Result<String, String> {
    let magnet = task
        .magnet
        .clone()
        .or_else(|| task.urls.first().cloned())
        .ok_or("缺少磁力/种子链接")?;
    let session = session(app).await?;

    let resp = session
        .add_torrent(
            AddTorrent::from_url(&magnet),
            Some(AddTorrentOptions {
                overwrite: true,
                ..Default::default()
            }),
        )
        .await
        .map_err(|e| format!("添加磁力任务失败: {e}"))?;

    let handle = match resp {
        AddTorrentResponse::Added(_, h) => h,
        AddTorrentResponse::AlreadyManaged(_, h) => {
            // 续传：之前暂停过，重新开跑。
            session.unpause(&h).await.ok();
            h
        }
        AddTorrentResponse::ListOnly(_) => return Err("该链接只返回了文件列表，无法下载".into()),
    };

    let out_dir = store::downloads_dir(app)?.to_string_lossy().to_string();

    loop {
        tokio::time::sleep(Duration::from_millis(700)).await;
        if control.is_cancelled() {
            session.pause(&handle).await.ok();
            return Err("已取消".into());
        }
        if control.is_paused() {
            session.pause(&handle).await.ok();
            return Err("已暂停".into());
        }
        let stats = handle.stats();
        if stats.total_bytes > 0 {
            control.total.store(stats.total_bytes, Ordering::Relaxed);
        }
        control.downloaded.store(stats.progress_bytes, Ordering::Relaxed);
        let speed = stats
            .live
            .as_ref()
            .map(|l| (l.download_speed.mbps * 1_048_576.0) as u64)
            .unwrap_or(0);
        super::report_progress(app, &task.id, stats.progress_bytes, speed, None).await;
        if stats.finished {
            break;
        }
        if let Some(err) = stats.error {
            return Err(format!("BT 下载出错: {err}"));
        }
    }
    Ok(out_dir)
}
