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

/// 由 Content-Type 判定资源类型（比扩展名可靠）。返回 None 表示不是可下载文件（页面/文本）。
fn kind_from_ct(ct: &str) -> Option<ResourceKind> {
    let main = ct.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    match main.as_str() {
        "application/pdf" | "application/epub+zip" | "application/x-mobipocket-ebook"
        | "application/vnd.amazon.ebook" | "image/vnd.djvu" => Some(ResourceKind::Book),
        _ if main.starts_with("image/") && main != "image/svg+xml" => Some(ResourceKind::Image),
        _ if main.starts_with("video/") => Some(ResourceKind::Video),
        _ if main.starts_with("audio/") => Some(ResourceKind::Audio),
        _ => None,
    }
}

/// Content-Type → 文件扩展名（用于命名/展示）。
fn ext_from_ct(ct: &str) -> Option<&'static str> {
    let main = ct.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    Some(match main.as_str() {
        "application/pdf" => "pdf",
        "application/epub+zip" => "epub",
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "audio/mpeg" => "mp3",
        "audio/flac" => "flac",
        "audio/mp4" => "m4a",
        _ => return None,
    })
}

fn is_html_ct(ct: &str) -> bool {
    let ct = ct.to_ascii_lowercase();
    ct.is_empty() || ct.contains("text/html") || ct.contains("application/xhtml")
}

/// 嗅探单个候选。跟随重定向拿到真实 URL（解掉百度/搜狗跳转链），再按 Content-Type 分流。
pub async fn sniff(hit_url: &str, hit_title: &str, kinds: &[ResourceKind]) -> Vec<ResourceItem> {
    // 1) 视频/音频站点页面 → 交 yt-dlp 解析。
    if is_video_host(hit_url) && (wants(kinds, ResourceKind::Video) || wants(kinds, ResourceKind::Audio)) {
        return vec![ytdlp_item(hit_url, hit_title, kinds)];
    }
    // 2) URL 路径直接带媒体扩展名 → 无需抓取即可判定为文件。
    if let Some(ext) = ext_of(hit_url) {
        let kind = ResourceKind::from_ext(&ext);
        if kind != ResourceKind::Other && wants(kinds, kind) {
            return vec![file_item(hit_url, hit_title, hit_url, &ext, kind)];
        }
    }
    // 3) 抓取候选页（跟随重定向）：先看 Content-Type，再决定是直链文件还是需扒链接的页面。
    let client = match client_for(hit_url) {
        Some(c) => c,
        None => return vec![],
    };
    let resp = match client.get(hit_url).send().await {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let final_url = resp.url().to_string();
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    // 3a) 响应本身就是文件（Content-Type 命中媒体类型）。
    if let Some(kind) = kind_from_ct(&ct) {
        if wants(kinds, kind) {
            let ext = ext_from_ct(&ct)
                .map(|s| s.to_string())
                .or_else(|| ext_of(&final_url))
                .unwrap_or_default();
            return vec![file_item(&final_url, hit_title, &final_url, &ext, kind)];
        }
        return vec![];
    }
    // 3b) 重定向后落到视频站（如百度跳转到 YouTube/B 站）→ yt-dlp。
    if is_video_host(&final_url) && (wants(kinds, ResourceKind::Video) || wants(kinds, ResourceKind::Audio)) {
        return vec![ytdlp_item(&final_url, hit_title, kinds)];
    }
    // 3c) HTML 页面 → 扒文件直链与磁力。非 HTML 非媒体（zip/json 等）跳过。
    if !is_html_ct(&ct) {
        return vec![];
    }
    let body = resp.text().await.unwrap_or_default();
    // 百度/搜狗等 JS/meta 跳转页：从页面提取真实目标再判一次视频站。
    if let Some(real) = meta_redirect(&body) {
        if is_video_host(&real) && (wants(kinds, ResourceKind::Video) || wants(kinds, ResourceKind::Audio)) {
            return vec![ytdlp_item(&real, hit_title, kinds)];
        }
        if let Some(ext) = ext_of(&real) {
            let kind = ResourceKind::from_ext(&ext);
            if kind != ResourceKind::Other && wants(kinds, kind) {
                return vec![file_item(&real, hit_title, &real, &ext, kind)];
            }
        }
    }
    extract_links(&final_url, &body, kinds)
}

/// 构造一个需 yt-dlp 解析的视频/音频条目。
fn ytdlp_item(url: &str, title: &str, kinds: &[ResourceKind]) -> ResourceItem {
    let kind = if kinds == [ResourceKind::Audio] {
        ResourceKind::Audio
    } else {
        ResourceKind::Video
    };
    let mut item = file_item(url, title, url, "", kind);
    item.needs_ytdlp = true;
    item.download_url = None;
    item.format = None;
    item
}

/// 从 HTML 里抽出 JS/meta 跳转目标（百度/搜狗中转页常见）。
fn meta_redirect(html: &str) -> Option<String> {
    // window.location.replace("...") / window.location.href="..."
    for pat in ["location.replace(\"", "location.href=\"", "location.href = \"", "URL=", "url="] {
        if let Some(idx) = html.find(pat) {
            let tail = &html[idx + pat.len()..];
            let end = tail.find(['"', '\'', '&', ' ', '>']).unwrap_or(tail.len().min(600));
            let cand = tail[..end].trim();
            if cand.starts_with("http") && cand.len() > 12 {
                return Some(cand.to_string());
            }
        }
    }
    None
}

/// 明显不是资源的链接/图片，过滤掉降噪。
fn is_junk(url: &str) -> bool {
    let lc = url.to_ascii_lowercase();
    if lc.starts_with("javascript:") || lc.starts_with("mailto:") || lc.starts_with("tel:") || lc.starts_with("data:") {
        return true;
    }
    const JUNK: [&str; 12] = [
        "icon", "logo", "sprite", "avatar", "emoji", "blank", "pixel", "spacer",
        "placeholder", "loading", "1x1", "thumb_default",
    ];
    JUNK.iter().any(|j| lc.contains(j))
}

fn extract_links(base: &str, html: &str, kinds: &[ResourceKind]) -> Vec<ResourceItem> {
    let doc = Html::parse_document(html);
    let a_sel = Selector::parse("a[href]").unwrap();
    let img_sel = Selector::parse("img[src]").unwrap();
    let base_url = Url::parse(base).ok();
    // 页面标题：作为链接自身无有意义文本时的兜底标题（比用 URL 当标题友好得多）。
    let page_title = Selector::parse("title").ok()
        .and_then(|s| doc.select(&s).next().map(|e| e.text().collect::<String>().trim().to_string()))
        .filter(|s| !s.is_empty());
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();

    let push_url = |raw: &str, label: &str, out: &mut Vec<ResourceItem>, seen: &mut std::collections::HashSet<String>| {
        if raw.starts_with("magnet:") {
            if wants(kinds, ResourceKind::Book) && seen.insert(raw.to_string()) {
                let mut item = file_item(raw, "磁力资源", raw, "", ResourceKind::Book);
                item.download_url = None;
                item.magnet = Some(raw.to_string());
                out.push(item);
            }
            return;
        }
        if is_junk(raw) {
            return;
        }
        let abs = match &base_url {
            Some(b) => b.join(raw).map(|u| u.to_string()).unwrap_or_default(),
            None => raw.to_string(),
        };
        if abs.is_empty() || !abs.starts_with("http") || !seen.insert(abs.clone()) {
            return;
        }
        if let Some(ext) = ext_of(&abs) {
            let kind = ResourceKind::from_ext(&ext);
            if kind != ResourceKind::Other && wants(kinds, kind) {
                // 标题优先级：链接锚文本 > 页面标题 > 文件名。
                let t = label.trim();
                let title = if t.len() >= 4 {
                    t.to_string()
                } else {
                    page_title.clone().unwrap_or_else(|| abs.clone())
                };
                out.push(file_item(&abs, &title, &abs, &ext, kind));
            }
        }
    };

    for a in doc.select(&a_sel).take(400) {
        if let Some(href) = a.value().attr("href") {
            let label = a.text().collect::<String>();
            push_url(href, &label, &mut out, &mut seen);
        }
        if out.len() >= 60 {
            break;
        }
    }
    if wants(kinds, ResourceKind::Image) {
        for img in doc.select(&img_sel).take(200) {
            // 优先用高清原图属性，退回 src。
            let src = img.value().attr("data-src")
                .or_else(|| img.value().attr("data-original"))
                .or_else(|| img.value().attr("src"));
            if let Some(src) = src {
                let label = img.value().attr("alt").unwrap_or("");
                push_url(src, label, &mut out, &mut seen);
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

/// HEAD 校验：对 web 抓取到的直链做并发探测。
/// - 剔除明确失效(404/410)与实为 HTML 页面（扩展名误判）的链接；
/// - 用 Content-Type 校正类型（比扩展名可靠），并补全 size_hint。
/// 仅校验 web 抓取的直链；API 源、磁力、yt-dlp 直接放行。
pub async fn verify(items: Vec<ResourceItem>, kinds: &[ResourceKind]) -> Vec<ResourceItem> {
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
    let kinds_owned = kinds.to_vec();
    let futs = checks.into_iter().map(|it| {
        let cfg = cfg.clone();
        let kinds = kinds_owned.clone();
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
                    let ct = resp
                        .headers()
                        .get(reqwest::header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_string();
                    // Content-Type 表明这是 HTML 页面而非文件 → 扩展名误判，丢弃。
                    if is_html_ct(&ct) && !ct.is_empty() {
                        return None;
                    }
                    let mut it = it;
                    // 用 Content-Type 校正类型（更可靠），并做类型过滤。
                    if let Some(k) = kind_from_ct(&ct) {
                        if !wants(&kinds, k) {
                            return None;
                        }
                        it.kind = k;
                        if let Some(e) = ext_from_ct(&ct) {
                            it.format = Some(e.to_string());
                        }
                    }
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
