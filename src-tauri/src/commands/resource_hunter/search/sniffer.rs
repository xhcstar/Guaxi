//! 链接嗅探器：访问候选页面，扒出可下载文件直链 / 磁力 / 需 yt-dlp 解析的视频页。
use super::super::model::{ResourceItem, ResourceKind};
use super::super::proxy;
use super::make_id;
use futures_util::future::join_all;
use scraper::{Html, Selector};
use url::Url;

const VIDEO_HOSTS: [&str; 10] = [
    "youtube.com", "youtu.be", "bilibili.com", "vimeo.com", "twitter.com", "x.com", "tiktok.com",
    "douyin.com", "dailymotion.com", "ixigua.com",
];

fn client_for(url: &str) -> Option<reqwest::Client> {
    let cfg = super::load_proxy();
    proxy::build_client_for(&cfg, url).ok()
}

/// 从 URL 路径末段取扩展名（去掉 query/fragment）。
fn ext_of(u: &str) -> Option<String> {
    let path = Url::parse(u).ok()?.path().to_string();
    let seg = path.rsplit('/').next()?;
    let dot = seg.rfind('.')?;
    let ext = seg[dot + 1..].to_ascii_lowercase();
    if ext.is_empty() || ext.len() > 5 {
        None
    } else {
        Some(ext)
    }
}

fn is_video_host(u: &str) -> bool {
    Url::parse(u)
        .ok()
        .and_then(|p| p.host_str().map(|h| h.to_string()))
        .map(|h| VIDEO_HOSTS.iter().any(|v| h.contains(v)))
        .unwrap_or(false)
}

fn wants(kinds: &[ResourceKind], k: ResourceKind) -> bool {
    kinds.is_empty() || kinds.contains(&k)
}

/// 嗅探单个候选。返回该页面命中的资源条目（可能多条）。
pub async fn sniff(hit_url: &str, hit_title: &str, kinds: &[ResourceKind]) -> Vec<ResourceItem> {
    // 1) 链接本身就是文件。
    if let Some(ext) = ext_of(hit_url) {
        let kind = ResourceKind::from_ext(&ext);
        if kind != ResourceKind::Other && wants(kinds, kind) {
            return vec![file_item(hit_url, hit_title, hit_url, &ext, kind)];
        }
    }
    // 2) 视频/音频站点页面 → 交 yt-dlp 解析。
    if is_video_host(hit_url) && (wants(kinds, ResourceKind::Video) || wants(kinds, ResourceKind::Audio)) {
        let kind = if kinds == [ResourceKind::Audio] {
            ResourceKind::Audio
        } else {
            ResourceKind::Video
        };
        let mut item = file_item(hit_url, hit_title, hit_url, "", kind);
        item.needs_ytdlp = true;
        item.download_url = None;
        item.format = None;
        return vec![item];
    }
    // 3) 抓页面，扒文件链接与磁力。
    let client = match client_for(hit_url) {
        Some(c) => c,
        None => return vec![],
    };
    let body = match client.get(hit_url).send().await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(_) => return vec![],
    };
    extract_links(hit_url, &body, kinds)
}

fn extract_links(base: &str, html: &str, kinds: &[ResourceKind]) -> Vec<ResourceItem> {
    let doc = Html::parse_document(html);
    let a_sel = Selector::parse("a[href]").unwrap();
    let img_sel = Selector::parse("img[src]").unwrap();
    let base_url = Url::parse(base).ok();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();

    let push_url = |raw: &str, out: &mut Vec<ResourceItem>, seen: &mut std::collections::HashSet<String>| {
        if raw.starts_with("magnet:") {
            if wants(kinds, ResourceKind::Book) && seen.insert(raw.to_string()) {
                let mut item = file_item(raw, "磁力资源", raw, "", ResourceKind::Book);
                item.download_url = None;
                item.magnet = Some(raw.to_string());
                out.push(item);
            }
            return;
        }
        let abs = match &base_url {
            Some(b) => b.join(raw).map(|u| u.to_string()).unwrap_or_default(),
            None => raw.to_string(),
        };
        if abs.is_empty() || !seen.insert(abs.clone()) {
            return;
        }
        if let Some(ext) = ext_of(&abs) {
            let kind = ResourceKind::from_ext(&ext);
            if kind != ResourceKind::Other && wants(kinds, kind) {
                out.push(file_item(&abs, &abs, &abs, &ext, kind));
            }
        }
    };

    for a in doc.select(&a_sel).take(400) {
        if let Some(href) = a.value().attr("href") {
            push_url(href, &mut out, &mut seen);
        }
        if out.len() >= 60 {
            break;
        }
    }
    if wants(kinds, ResourceKind::Image) {
        for img in doc.select(&img_sel).take(200) {
            if let Some(src) = img.value().attr("src") {
                push_url(src, &mut out, &mut seen);
            }
            if out.len() >= 120 {
                break;
            }
        }
    }
    out
}

fn file_item(id_src: &str, title: &str, url: &str, ext: &str, kind: ResourceKind) -> ResourceItem {
    let t = title.trim();
    let title = if t.is_empty() {
        url.rsplit('/').next().unwrap_or("资源").to_string()
    } else {
        t.chars().take(160).collect()
    };
    ResourceItem {
        id: make_id(id_src),
        title,
        kind,
        source: "web".into(),
        source_url: url.to_string(),
        download_url: Some(url.to_string()),
        magnet: None,
        needs_ytdlp: false,
        format: if ext.is_empty() { None } else { Some(ext.to_string()) },
        thumbnail: None,
        author: None,
        published_at: None,
        summary: None,
        size_hint: None,
        relevance: 0.5,
        tags: vec![],
    }
}

/// HEAD 校验：对抓取到的直链做并发探测，剔除明确失效(404/403/410)的，
/// 并补全 size_hint 与 format。仅校验 web 抓取的直链，API 源与磁力/yt-dlp 跳过。
pub async fn verify(items: Vec<ResourceItem>) -> Vec<ResourceItem> {
    let cfg = super::load_proxy();
    let mut checks = Vec::new();
    let mut passthrough = Vec::new();
    for it in items {
        let is_web_link = it.source == "web" && it.download_url.is_some() && !it.needs_ytdlp && it.magnet.is_none();
        // 图片直链常有防盗链(HEAD 返回 403)，一律不校验，避免误杀。
        if is_web_link && it.kind != ResourceKind::Image {
            checks.push(it);
        } else {
            passthrough.push(it);
        }
    }
    checks.truncate(48);
    let futs = checks.into_iter().map(|it| {
        let cfg = cfg.clone();
        async move {
            let url = it.download_url.clone().unwrap();
            let client = match proxy::build_client_for(&cfg, &url) {
                Ok(c) => c,
                Err(_) => return Some(it),
            };
            match client.head(&url).send().await {
                Ok(resp) => {
                    let code = resp.status().as_u16();
                    // 只丢明确不存在的(404/410)；403 多为反爬/防盗链，保留。
                    if code == 404 || code == 410 {
                        return None;
                    }
                    let mut it = it;
                    if let Some(len) = resp
                        .headers()
                        .get(reqwest::header::CONTENT_LENGTH)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                    {
                        it.size_hint = Some(len);
                    }
                    Some(it)
                }
                // 网络错误/服务器不支持 HEAD：保留（不误杀）。
                Err(_) => Some(it),
            }
        }
    });
    let mut out = passthrough;
    for r in join_all(futs).await {
        if let Some(it) = r {
            out.push(it);
        }
    }
    out
}
