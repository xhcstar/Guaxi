//! yt-dlp / ffmpeg 按需联网下载与缓存。首次用到视频/音频解析时拉取到
//! <app_data>/resource_hunter/tools，缓存复用；离线则相关功能优雅降级。
use super::{proxy, store};
use std::path::PathBuf;
use tauri::AppHandle;
use tokio::io::AsyncWriteExt;

#[cfg(windows)]
const YTDLP_BIN: &str = "yt-dlp.exe";
#[cfg(not(windows))]
const YTDLP_BIN: &str = "yt-dlp";

#[cfg(windows)]
const YTDLP_URL: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
#[cfg(target_os = "macos")]
const YTDLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos";
#[cfg(all(not(windows), not(target_os = "macos")))]
const YTDLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp";

fn load_proxy() -> super::model::ProxyConfig {
    store::get_setting("proxy")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

async fn fetch_to(url: &str, dest: &PathBuf) -> Result<(), String> {
    let cfg = load_proxy();
    let client = proxy::build_client_for(&cfg, url)?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("下载失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("下载失败，HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("读取失败: {e}"))?;
    let mut f = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("写文件失败: {e}"))?;
    f.write_all(&bytes).await.map_err(|e| format!("写文件失败: {e}"))?;
    f.flush().await.ok();
    Ok(())
}

/// 确保 yt-dlp 可用，返回其路径；不存在则联网下载。
pub async fn ensure_ytdlp(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = store::tools_dir(app)?;
    let bin = dir.join(YTDLP_BIN);
    if bin.exists() {
        return Ok(bin);
    }
    fetch_to(YTDLP_URL, &bin).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&bin) {
            let mut p = meta.permissions();
            p.set_mode(0o755);
            let _ = std::fs::set_permissions(&bin, p);
        }
    }
    Ok(bin)
}

/// 确保 ffmpeg 可用（视频+音频轨合并需要）。best-effort：失败返回 None，
/// yt-dlp 仍可下载无需合并的单流格式。
#[cfg(windows)]
pub async fn ensure_ffmpeg(app: &AppHandle) -> Option<PathBuf> {
    let dir = store::tools_dir(app).ok()?;
    let bin = dir.join("ffmpeg.exe");
    if bin.exists() {
        return Some(bin);
    }
    let url = "https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-win64-gpl.zip";
    let zip_path = dir.join("ffmpeg.zip");
    if fetch_to(url, &zip_path).await.is_err() {
        return None;
    }
    let extracted = tokio::task::spawn_blocking(move || extract_ffmpeg(&zip_path, &bin).map(|_| bin))
        .await
        .ok()?;
    extracted.ok()
}

#[cfg(windows)]
fn extract_ffmpeg(zip_path: &PathBuf, out_bin: &PathBuf) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if name.ends_with("bin/ffmpeg.exe") {
            let mut out = std::fs::File::create(out_bin).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(zip_path);
            return Ok(());
        }
    }
    Err("压缩包内未找到 ffmpeg".into())
}

#[cfg(not(windows))]
pub async fn ensure_ffmpeg(_app: &AppHandle) -> Option<PathBuf> {
    // 非 Windows 优先用系统 ffmpeg（PATH 中）。
    which_system("ffmpeg")
}

#[cfg(not(windows))]
fn which_system(bin: &str) -> Option<PathBuf> {
    let out = std::process::Command::new("which").arg(bin).output().ok()?;
    if out.status.success() {
        let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    None
}
