//! yt-dlp 进程下载：解析视频/音频站点页面并落地，进度并入统一队列。
//! 首次调用会触发 yt-dlp/ffmpeg 的按需下载（ytdlp_fetch）。
use super::super::model::{DownloadTask, ResourceKind};
use super::super::{proxy, store, ytdlp_fetch};
use super::Control;
use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

fn load_proxy() -> super::super::model::ProxyConfig {
    store::get_setting("proxy")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn parse_percent(line: &str) -> Option<f64> {
    let idx = line.find('%')?;
    let start = line[..idx]
        .rfind(|c: char| c == ' ' || c == ']')
        .map(|p| p + 1)
        .unwrap_or(0);
    line[start..idx].trim().parse::<f64>().ok()
}

#[cfg(windows)]
fn no_window(cmd: &mut Command) {
    cmd.creation_flags(0x0800_0000);
}
#[cfg(not(windows))]
fn no_window(_cmd: &mut Command) {}

pub async fn download(
    app: &AppHandle,
    task: &DownloadTask,
    control: &Arc<Control>,
) -> Result<String, String> {
    let ytdlp = ytdlp_fetch::ensure_ytdlp(app).await?;
    let ffmpeg = ytdlp_fetch::ensure_ffmpeg(app).await;
    let url = task.urls.first().ok_or("无解析地址")?.clone();

    let dest = std::path::Path::new(&task.dest);
    let parent = dest.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    tokio::fs::create_dir_all(&parent).await.ok();
    let stem = dest
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| task.id.clone());
    let outtmpl = parent.join(format!("{stem}.%(ext)s"));

    let mut cmd = Command::new(&ytdlp);
    cmd.arg(&url)
        .arg("-o")
        .arg(&outtmpl)
        .arg("--newline")
        .arg("--no-playlist")
        .arg("--no-part");
    if let Some(px) = proxy::proxy_arg(&load_proxy()) {
        cmd.arg("--proxy").arg(px);
    }
    if let Some(ff) = &ffmpeg {
        cmd.arg("--ffmpeg-location").arg(ff);
    }
    if task.kind == ResourceKind::Audio {
        cmd.arg("-x").arg("--audio-format").arg("mp3");
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());
    no_window(&mut cmd);

    control.total.store(10000, Ordering::Relaxed);
    let mut child = cmd.spawn().map_err(|e| format!("启动 yt-dlp 失败: {e}"))?;
    let stdout = child.stdout.take().ok_or("无法读取 yt-dlp 输出")?;
    let mut lines = BufReader::new(stdout).lines();

    loop {
        tokio::select! {
            line = lines.next_line() => {
                match line {
                    Ok(Some(l)) => {
                        if let Some(pct) = parse_percent(&l) {
                            let dl = (pct * 100.0) as u64;
                            control.downloaded.store(dl, Ordering::Relaxed);
                            super::report_progress(app, &task.id, dl, 0, None).await;
                        }
                    }
                    _ => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(500)) => {
                if control.is_cancelled() {
                    let _ = child.kill().await;
                    return Err("已取消".into());
                }
                if control.is_paused() {
                    let _ = child.kill().await;
                    return Err("已暂停".into());
                }
            }
        }
    }

    let status = child.wait().await.map_err(|e| format!("等待 yt-dlp 失败: {e}"))?;
    if !status.success() {
        return Err("yt-dlp 解析/下载失败".into());
    }
    find_output(&parent, &stem)
        .await
        .ok_or_else(|| "未找到 yt-dlp 输出文件".to_string())
}

async fn find_output(dir: &std::path::Path, stem: &str) -> Option<String> {
    let mut rd = tokio::fs::read_dir(dir).await.ok()?;
    while let Ok(Some(entry)) = rd.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(stem) && !name.ends_with(".part") {
            return Some(entry.path().to_string_lossy().to_string());
        }
    }
    None
}
