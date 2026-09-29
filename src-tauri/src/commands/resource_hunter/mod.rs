//! 资源嗅探下载引擎：全网搜索 → 嗅探文件 → 多线程/续传/yt-dlp 下载 → 资源库管理。
//! 命令出口在本文件，注册进 main.rs 的 invoke_handler。
pub mod downloader;
pub mod library_search;
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
    Ok(search::run(&app, query).await)
}

#[command]
pub async fn rh_enqueue(app: AppHandle, item: ResourceItem) -> Result<String, String> {
    ensure_init(&app).await?;
    let task = build_task(&app, &item)?;
    downloader::enqueue(app, task).await
}

#[command]
pub async fn rh_enqueue_batch(app: AppHandle, items: Vec<ResourceItem>) -> Result<Vec<String>, String> {
    ensure_init(&app).await?;
    let mut ids = Vec::new();
    for item in items {
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
pub async fn rh_library_search(
    app: AppHandle,
    query: String,
    kind: Option<ResourceKind>,
) -> Result<Vec<serde_json::Value>, String> {
    ensure_init(&app).await?;
    let mut rows = library_search::search(&query, 200)?;
    if let Some(k) = kind {
        let ks = match k {
            ResourceKind::Book => "book",
            ResourceKind::Paper => "paper",
            ResourceKind::Video => "video",
            ResourceKind::Audio => "audio",
            ResourceKind::Image => "image",
            ResourceKind::Code => "code",
            ResourceKind::Other => "other",
        };
        rows.retain(|r| r.get("kind").and_then(|v| v.as_str()) == Some(ks));
    }
    Ok(rows)
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

#[command]
pub async fn rh_list_engines() -> Result<Vec<search::engines::EngineMeta>, String> {
    Ok(search::engines::engine_metas())
}

#[command]
pub async fn rh_delete_resource(app: AppHandle, id: String, remove_file: bool) -> Result<(), String> {
    ensure_init(&app).await?;
    store::delete_resource(&id, remove_file)
}

#[command]
pub async fn rh_update_tags(app: AppHandle, id: String, tags: Vec<String>) -> Result<(), String> {
    ensure_init(&app).await?;
    store::update_resource_tags(&id, &tags)
}

#[command]
pub async fn rh_all_tags(app: AppHandle) -> Result<Vec<String>, String> {
    ensure_init(&app).await?;
    store::all_tags()
}

#[command]
pub async fn rh_reveal(_app: AppHandle, path: String) -> Result<(), String> {
    reveal_in_os(&path)
}

#[command]
pub async fn rh_get_download_dir(app: AppHandle) -> Result<String, String> {
    ensure_init(&app).await?;
    Ok(store::downloads_dir(&app)?.to_string_lossy().to_string())
}

#[command]
pub async fn rh_set_download_dir(app: AppHandle, path: String) -> Result<(), String> {
    ensure_init(&app).await?;
    let p = path.trim();
    if p.is_empty() {
        store::set_setting("download_dir", "")?;
        return Ok(());
    }
    std::fs::create_dir_all(p).map_err(|e| format!("目录不可用: {e}"))?;
    store::set_setting("download_dir", p)
}

#[command]
pub async fn rh_get_speed_limit(app: AppHandle) -> Result<u64, String> {
    ensure_init(&app).await?;
    Ok(store::get_setting("speed_limit_kbps")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0))
}

#[command]
pub async fn rh_set_speed_limit(app: AppHandle, kbps: u64) -> Result<(), String> {
    ensure_init(&app).await?;
    store::set_setting("speed_limit_kbps", &kbps.to_string())
}

/// 读取第三方 API Key 配置（JSON 对象；未设置返回空对象）。
#[command]
pub async fn rh_get_api_keys(app: AppHandle) -> Result<serde_json::Value, String> {
    ensure_init(&app).await?;
    let v = store::get_setting("api_keys")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    Ok(v)
}

/// 保存第三方 API Key 配置（core/pexels/jamendo/unpaywall_email 等键）。
#[command]
pub async fn rh_set_api_keys(app: AppHandle, keys: serde_json::Value) -> Result<(), String> {
    ensure_init(&app).await?;
    let json = serde_json::to_string(&keys).map_err(|e| e.to_string())?;
    store::set_setting("api_keys", &json)
}

/// 读取 SearXNG 实例地址（用于元搜索引擎，空表示未配置）。
#[command]
pub async fn rh_get_searxng(app: AppHandle) -> Result<String, String> {
    ensure_init(&app).await?;
    Ok(store::get_setting("searxng_url").ok().flatten().unwrap_or_default())
}

/// 保存 SearXNG 实例地址。
#[command]
pub async fn rh_set_searxng(app: AppHandle, url: String) -> Result<(), String> {
    ensure_init(&app).await?;
    store::set_setting("searxng_url", url.trim())
}

/// 经代理拉取(境外)缩略图，返回 data URI，规避 webview 直连境外图床失败。
#[command]
pub async fn rh_fetch_thumb(app: AppHandle, url: String) -> Result<String, String> {
    ensure_init(&app).await?;
    let cfg = store::get_setting("proxy")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let client = proxy::build_client_for(&cfg, &url)?;
    let resp = client.get(&url).send().await.map_err(|e| format!("拉取失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > 3 * 1024 * 1024 {
        return Err("缩略图过大".into());
    }
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{ct};base64,{b64}"))
}

/// 导出资源库为 JSON 清单（备份/同步）。返回导出文件路径。
#[command]
pub async fn rh_export_library(app: AppHandle) -> Result<String, String> {
    ensure_init(&app).await?;
    let rows = store::export_all()?;
    let json = serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())?;
    let path = store::downloads_dir(&app)?.join("library_export.json");
    std::fs::write(&path, json).map_err(|e| format!("写入失败: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

/// 从 JSON 清单导入资源库，返回导入条数。
#[command]
pub async fn rh_import_library(app: AppHandle, path: String) -> Result<usize, String> {
    ensure_init(&app).await?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("读取失败: {e}"))?;
    let rows: Vec<serde_json::Value> = serde_json::from_str(&text).map_err(|e| format!("解析失败: {e}"))?;
    store::import_rows(&rows)
}

fn reveal_in_os(path: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // explorer 对含空格的路径挑剔：/select, 必须在引号外、路径整体加引号，
        // 且用反斜杠。用 raw_arg 精确控制命令行，避免 std 的自动引号把 "/select," 一起括进去。
        let norm = path.replace('/', "\\");
        let p = std::path::Path::new(&norm);
        if p.exists() {
            std::process::Command::new("explorer")
                .raw_arg(format!("/select,\"{norm}\""))
                .spawn()
                .map_err(|e| format!("定位失败: {e}"))?;
        } else if let Some(dir) = p.parent().filter(|d| d.exists()) {
            // 文件已不在原处（被移动/清理）→ 退而打开其所在目录。
            std::process::Command::new("explorer")
                .raw_arg(format!("\"{}\"", dir.to_string_lossy().replace('/', "\\")))
                .spawn()
                .map_err(|e| format!("定位失败: {e}"))?;
        } else {
            return Err("文件与所在目录都不存在，可能已被删除".into());
        }
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", path])
            .spawn()
            .map_err(|e| format!("定位失败: {e}"))?;
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let parent = std::path::Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        std::process::Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| format!("定位失败: {e}"))?;
    }
    Ok(())
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
