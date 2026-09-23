//! 资源嗅探下载引擎：全网搜索 → 嗅探文件 → 多线程/续传/yt-dlp 下载 → 资源库管理。
//! 命令出口在本文件，注册进 main.rs 的 invoke_handler。
pub mod downloader;
pub mod model;
pub mod proxy;
pub mod search;
pub mod store;
pub mod ytdlp_fetch;

use model::{DownloadStatus, DownloadTask, ProxyConfig, ResourceItem, ResourceKind, SearchQuery};
use std::sync::OnceLock;
use tauri::{command, AppHandle};

static INITED: OnceLock<()> = OnceLock::new();

async fn ensure_init(app: &AppHandle) -> Result<(), String> {
    if INITED.get().is_some() {
        return Ok(());
    }
    store::init(app)?;
    downloader::restore_from_store(app).await;
    let _ = INITED.set(());
    Ok(())
}

fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if "\\/:*?\"<>|\n\r\t".contains(c) { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').to_string();
    let short: String = trimmed.chars().take(80).collect();
    if short.is_empty() {
        "resource".into()
    } else {
        short
    }
}

fn ext_for(item: &ResourceItem) -> String {
    if let Some(f) = &item.format {
        if !f.is_empty() {
            return f.clone();
        }
    }
    if let Some(u) = &item.download_url {
        if let Some(seg) = u.split('?').next().and_then(|p| p.rsplit('.').next()) {
            if seg.len() <= 5 && !seg.is_empty() {
                return seg.to_ascii_lowercase();
            }
        }
    }
    "bin".into()
}

fn build_task(app: &AppHandle, item: &ResourceItem) -> Result<DownloadTask, String> {
    let dir = store::downloads_dir(app)?;
    let stem = format!("{}-{}", sanitize(&item.title), &item.id[..item.id.len().min(6)]);
    let dest = if item.needs_ytdlp {
        dir.join(&stem)
    } else {
        dir.join(format!("{stem}.{}", ext_for(item)))
    };
    let urls = item
        .download_url
        .clone()
        .map(|u| vec![u])
        .unwrap_or_default();
    if urls.is_empty() && item.magnet.is_none() && !item.needs_ytdlp {
        return Err("该资源没有可下载链接".into());
    }
    Ok(DownloadTask {
        id: item.id.clone(),
        title: item.title.clone(),
        kind: item.kind,
        urls: if item.needs_ytdlp {
            vec![item.source_url.clone()]
        } else {
            urls
        },
        magnet: item.magnet.clone(),
        via_ytdlp: item.needs_ytdlp,
        dest: dest.to_string_lossy().to_string(),
        total_size: 0,
        downloaded: 0,
        chunks: vec![],
        status: DownloadStatus::Queued,
        speed: 0,
        eta_secs: None,
        priority: 0,
        retries: 0,
        error: None,
        source: item.source.clone(),
        source_url: Some(item.source_url.clone()),
        created_at: chrono::Utc::now().timestamp(),
    })
}

// -------- 命令出口 --------

#[command]
pub async fn rh_search(app: AppHandle, query: SearchQuery) -> Result<Vec<ResourceItem>, String> {
    ensure_init(&app).await?;
    Ok(search::run(query).await)
}

#[command]
pub async fn rh_enqueue(app: AppHandle, item: ResourceItem) -> Result<String, String> {
    ensure_init(&app).await?;
    if item.magnet.is_some() && item.download_url.is_none() && !item.needs_ytdlp {
        return Err("磁力/BT 下载将在书籍阶段接入（librqbit），当前版本暂不支持".into());
    }
    let task = build_task(&app, &item)?;
    downloader::enqueue(app, task).await
}

#[command]
pub async fn rh_enqueue_batch(app: AppHandle, items: Vec<ResourceItem>) -> Result<Vec<String>, String> {
    ensure_init(&app).await?;
    let mut ids = Vec::new();
    for item in items {
        if item.magnet.is_some() && item.download_url.is_none() && !item.needs_ytdlp {
            continue;
        }
        if let Ok(task) = build_task(&app, &item) {
            if let Ok(id) = downloader::enqueue(app.clone(), task).await {
                ids.push(id);
            }
        }
    }
    Ok(ids)
}

#[command]
pub async fn rh_list_tasks(app: AppHandle) -> Result<Vec<DownloadTask>, String> {
    ensure_init(&app).await?;
    Ok(downloader::list_tasks().await)
}

#[command]
pub async fn rh_pause(app: AppHandle, id: String) -> Result<(), String> {
    ensure_init(&app).await?;
    downloader::pause(&id).await
}

#[command]
pub async fn rh_resume(app: AppHandle, id: String) -> Result<(), String> {
    ensure_init(&app).await?;
    downloader::resume(app, id).await
}

#[command]
pub async fn rh_cancel(app: AppHandle, id: String) -> Result<(), String> {
    ensure_init(&app).await?;
    downloader::cancel(app, id).await
}

#[command]
pub async fn rh_list_resources(
    app: AppHandle,
    kind: Option<ResourceKind>,
) -> Result<Vec<serde_json::Value>, String> {
    ensure_init(&app).await?;
    store::list_resources(kind)
}

#[command]
pub async fn rh_get_proxy(app: AppHandle) -> Result<ProxyConfig, String> {
    ensure_init(&app).await?;
    Ok(store::get_setting("proxy")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default())
}

#[command]
pub async fn rh_set_proxy(app: AppHandle, config: ProxyConfig) -> Result<(), String> {
    ensure_init(&app).await?;
    let json = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    store::set_setting("proxy", &json)
}

#[command]
pub async fn rh_open_path(_app: AppHandle, path: String) -> Result<(), String> {
    open_in_os(&path)
}

fn open_in_os(path: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", path])
            .spawn()
            .map_err(|e| format!("打开失败: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开失败: {e}"))?;
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开失败: {e}"))?;
    }
    Ok(())
}
