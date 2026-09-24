//! HTTP 多线程分块下载 + 断点续传。
//! - 支持 Range 且文件够大 → 切分多连接并发；否则单流。
//! - .part 临时文件，完成后 rename；分块进度回写 store 供续传。
use super::super::model::{ChunkState, DownloadTask, ProxyConfig};
use super::super::{proxy, store};
use super::{persist_chunks, report_progress, set_total, Control};
use futures_util::StreamExt;
use std::io::SeekFrom;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

const MIN_SPLIT: u64 = 2 * 1024 * 1024;
const MAX_CHUNKS: u64 = 8;

fn load_proxy() -> ProxyConfig {
    store::get_setting("proxy")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// 全局限速(B/s)，0 表示不限。
fn speed_limit_bps() -> u64 {
    store::get_setting("speed_limit_kbps")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<u64>().ok())
        .map(|k| k.saturating_mul(1024))
        .unwrap_or(0)
}

/// 按聚合速率节流：让 downloaded/limit 的应有耗时不小于实际耗时。
async fn throttle(control: &Arc<Control>, limit_bps: u64) {
    if limit_bps == 0 {
        return;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let start = control.started_ms.load(Ordering::Relaxed);
    if start == 0 {
        control.started_ms.store(now, Ordering::Relaxed);
        return;
    }
    let total = control.downloaded.load(Ordering::Relaxed);
    let expected = total.saturating_mul(1000) / limit_bps.max(1);
    let elapsed = now.saturating_sub(start);
    if elapsed < expected {
        tokio::time::sleep(Duration::from_millis(expected - elapsed)).await;
    }
}

async fn probe(client: &reqwest::Client, url: &str) -> (Option<u64>, bool) {
    if let Ok(resp) = client.head(url).send().await {
        let len = resp
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        let ranges = resp
            .headers()
            .get(reqwest::header::ACCEPT_RANGES)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.contains("bytes"))
            .unwrap_or(false);
        if len.is_some() {
            return (len, ranges);
        }
    }
    (None, false)
}

// PLACEHOLDER_APPEND

pub async fn download(
    app: &AppHandle,
    task: &DownloadTask,
    control: &Arc<Control>,
) -> Result<String, String> {
    let cfg = load_proxy();
    let url = task.urls.first().ok_or("无下载地址")?.clone();
    let client = proxy::build_client_for(&cfg, &url)?;

    let (len, ranges) = probe(&client, &url).await;
    let total = len.unwrap_or(0);
    if total > 0 {
        set_total(&task.id, total).await;
        control.total.store(total, Ordering::Relaxed);
    }

    if let Some(parent) = std::path::Path::new(&task.dest).parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    let part = format!("{}.part", task.dest);
    let reporter = spawn_reporter(app.clone(), task.id.clone(), control.clone());
    let limit = speed_limit_bps();

    let result = if ranges && total >= MIN_SPLIT {
        multi(&client, &url, &part, total, task, control, limit).await
    } else {
        single(&client, &url, &part, ranges, task, control, limit).await
    };
    reporter.abort();
    result?;

    tokio::fs::rename(&part, &task.dest)
        .await
        .map_err(|e| format!("重命名失败: {e}"))?;
    let final_dl = control.downloaded.load(Ordering::Relaxed);
    report_progress(app, &task.id, final_dl, 0, Some(0)).await;
    Ok(task.dest.clone())
}

fn spawn_reporter(
    app: AppHandle,
    id: String,
    control: Arc<Control>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut last = control.downloaded.load(Ordering::Relaxed);
        let mut last_t = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(700)).await;
            let dl = control.downloaded.load(Ordering::Relaxed);
            let now = Instant::now();
            let dt = now.duration_since(last_t).as_secs_f64().max(0.001);
            let speed = ((dl.saturating_sub(last)) as f64 / dt) as u64;
            let total = control.total.load(Ordering::Relaxed);
            let eta = if speed > 0 && total > dl {
                Some((total - dl) / speed)
            } else {
                None
            };
            control.speed.store(speed, Ordering::Relaxed);
            report_progress(&app, &id, dl, speed, eta).await;
            last = dl;
            last_t = now;
        }
    })
}
// PLACEHOLDER_APPEND2

async fn multi(
    client: &reqwest::Client,
    url: &str,
    part: &str,
    total: u64,
    task: &DownloadTask,
    control: &Arc<Control>,
    limit_bps: u64,
) -> Result<(), String> {
    {
        let f = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(part)
            .await
            .map_err(|e| format!("创建文件失败: {e}"))?;
        f.set_len(total).await.ok();
    }
    let n = ((total + MIN_SPLIT - 1) / MIN_SPLIT).min(MAX_CHUNKS).max(1);
    let base = total / n;
    let mut chunks: Vec<ChunkState> = Vec::new();
    for i in 0..n {
        let start = i * base;
        let end = if i == n - 1 { total - 1 } else { (i + 1) * base - 1 };
        let done = task
            .chunks
            .get(i as usize)
            .map(|c| c.downloaded)
            .unwrap_or(0)
            .min(end - start + 1);
        chunks.push(ChunkState { start, end, downloaded: done });
    }
    let mut seed = 0u64;
    for (i, c) in chunks.iter().enumerate() {
        if let Some(a) = control.chunk_done.get(i) {
            a.store(c.downloaded, Ordering::Relaxed);
        }
        seed += c.downloaded;
    }
    control.downloaded.store(seed, Ordering::Relaxed);

    let mut handles = Vec::new();
    for (i, c) in chunks.iter().cloned().enumerate() {
        if c.is_done() {
            continue;
        }
        let client = client.clone();
        let url = url.to_string();
        let part = part.to_string();
        let control = control.clone();
        handles.push(tokio::spawn(async move {
            download_chunk(&client, &url, &part, i, c, &control, limit_bps).await
        }));
    }
    let mut err = None;
    for h in handles {
        match h.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => err = Some(e),
            Err(e) => err = Some(format!("分块任务异常: {e}")),
        }
    }
    let snap: Vec<ChunkState> = chunks
        .iter()
        .enumerate()
        .map(|(i, c)| ChunkState {
            start: c.start,
            end: c.end,
            downloaded: control
                .chunk_done
                .get(i)
                .map(|a| a.load(Ordering::Relaxed))
                .unwrap_or(c.downloaded),
        })
        .collect();
    let total_dl = snap.iter().map(|c| c.downloaded).sum();
    persist_chunks(&task.id, snap, total_dl).await;
    if let Some(e) = err {
        return Err(e);
    }
    Ok(())
}
// PLACEHOLDER_APPEND3

async fn download_chunk(
    client: &reqwest::Client,
    url: &str,
    part: &str,
    idx: usize,
    chunk: ChunkState,
    control: &Arc<Control>,
    limit_bps: u64,
) -> Result<(), String> {
    let offset = chunk.start + chunk.downloaded;
    if offset > chunk.end {
        return Ok(());
    }
    let mut f = tokio::fs::OpenOptions::new()
        .write(true)
        .open(part)
        .await
        .map_err(|e| format!("打开分块失败: {e}"))?;
    f.seek(SeekFrom::Start(offset))
        .await
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(url)
        .header(reqwest::header::RANGE, format!("bytes={}-{}", offset, chunk.end))
        .send()
        .await
        .map_err(|e| format!("分块请求失败: {e}"))?;
    let mut stream = resp.bytes_stream();
    while let Some(item) = stream.next().await {
        if control.is_cancelled() {
            return Err("已取消".into());
        }
        if control.is_paused() {
            return Err("已暂停".into());
        }
        let bytes = item.map_err(|e| format!("读取失败: {e}"))?;
        f.write_all(&bytes).await.map_err(|e| format!("写入失败: {e}"))?;
        let len = bytes.len() as u64;
        if let Some(a) = control.chunk_done.get(idx) {
            a.fetch_add(len, Ordering::Relaxed);
        }
        control.downloaded.fetch_add(len, Ordering::Relaxed);
        throttle(control, limit_bps).await;
    }
    Ok(())
}
// PLACEHOLDER_APPEND4

async fn single(
    client: &reqwest::Client,
    url: &str,
    part: &str,
    ranges: bool,
    task: &DownloadTask,
    control: &Arc<Control>,
    limit_bps: u64,
) -> Result<(), String> {
    let existing = tokio::fs::metadata(part).await.map(|m| m.len()).unwrap_or(0);
    let want_resume = ranges && existing > 0;
    let mut req = client.get(url);
    if want_resume {
        req = req.header(reqwest::header::RANGE, format!("bytes={}-", existing));
    }
    let resp = req.send().await.map_err(|e| format!("请求失败: {e}"))?;
    let is_partial = resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    let resume = want_resume && is_partial;

    let mut f = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(part)
        .await
        .map_err(|e| format!("创建文件失败: {e}"))?;
    if resume {
        f.seek(SeekFrom::Start(existing)).await.ok();
        control.downloaded.store(existing, Ordering::Relaxed);
    } else {
        f.set_len(0).await.ok();
        control.downloaded.store(0, Ordering::Relaxed);
    }
    let mut stream = resp.bytes_stream();
    while let Some(item) = stream.next().await {
        if control.is_cancelled() {
            return Err("已取消".into());
        }
        if control.is_paused() {
            persist_chunks(&task.id, vec![], control.downloaded.load(Ordering::Relaxed)).await;
            return Err("已暂停".into());
        }
        let bytes = item.map_err(|e| format!("读取失败: {e}"))?;
        f.write_all(&bytes).await.map_err(|e| format!("写入失败: {e}"))?;
        control
            .downloaded
            .fetch_add(bytes.len() as u64, Ordering::Relaxed);
        throttle(control, limit_bps).await;
    }
    Ok(())
}
