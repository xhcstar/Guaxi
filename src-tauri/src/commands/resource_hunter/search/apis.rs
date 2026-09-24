//! 领域富集站 API 适配器：直接查结构化接口，拿到干净的文件直链，
//! 精度远高于 SERP 抓取。全部走境外代理路由(client_for_source foreign=true)，无需 key。
use super::super::model::{ResourceItem, ResourceKind};
use super::super::{proxy, ytdlp_fetch};
use super::make_id;
use scraper::{Html, Selector};
use tauri::AppHandle;

fn client() -> Option<reqwest::Client> {
    proxy::client_for_source(&super::load_proxy(), true).ok()
}

/// 境内直连客户端（百度等国内源不走代理）。
fn client_cn() -> Option<reqwest::Client> {
    proxy::client_for_source(&super::load_proxy(), false).ok()
}

fn item(id_src: &str, title: String, kind: ResourceKind, source: &str, page: String,
        dl: Option<String>, fmt: Option<&str>, rel: f32) -> ResourceItem {
    ResourceItem {
        id: make_id(id_src),
        title,
        kind,
        source: source.into(),
        source_url: page,
        download_url: dl,
        magnet: None,
        needs_ytdlp: false,
        format: fmt.map(|s| s.to_string()),
        thumbnail: None,
        author: None,
        published_at: None,
        summary: None,
        size_hint: None,
        relevance: rel,
        tags: vec![],
    }
}

async fn get_json(url: &str) -> Option<serde_json::Value> {
    let c = client()?;
    c.get(url).send().await.ok()?.json().await.ok()
}

// PLACEHOLDER_APIS

/// GitHub 仓库搜索 → 默认分支 zip 直链。代码领域。
pub async fn github(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://api.github.com/search/repositories?q={}&sort=stars&per_page={}",
        urlencoding::encode(kw), limit.min(30)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    if let Some(arr) = json.get("items").and_then(|v| v.as_array()) {
        for r in arr {
            let full = r.get("full_name").and_then(|v| v.as_str()).unwrap_or("");
            if full.is_empty() { continue; }
            let branch = r.get("default_branch").and_then(|v| v.as_str()).unwrap_or("master");
            let page = r.get("html_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let dl = format!("https://github.com/{full}/archive/refs/heads/{branch}.zip");
            let desc = r.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let title = if desc.is_empty() { full.to_string() } else { format!("{full} — {desc}") };
            out.push(item(&dl, title, ResourceKind::Code, "GitHub", page, Some(dl.clone()), Some("zip"), 0.85));
        }
    }
    out
}

/// Project Gutenberg（Gutendex）→ epub/txt 直链。公版书籍。
pub async fn gutendex(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!("https://gutendex.com/books?search={}", urlencoding::encode(kw));
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    if let Some(arr) = json.get("results").and_then(|v| v.as_array()) {
        for b in arr.iter().take(limit) {
            let title = b.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if title.is_empty() { continue; }
            let fmts = b.get("formats").and_then(|v| v.as_object());
            let (dl, ext) = fmts.and_then(|m| {
                m.get("application/epub+zip").and_then(|v| v.as_str()).map(|s| (s.to_string(), "epub"))
                    .or_else(|| m.get("application/x-mobipocket-ebook").and_then(|v| v.as_str()).map(|s| (s.to_string(), "mobi")))
                    .or_else(|| m.get("text/plain; charset=utf-8").and_then(|v| v.as_str()).map(|s| (s.to_string(), "txt")))
            }).map(|(u, e)| (Some(u), Some(e))).unwrap_or((None, None));
            let page = format!("https://www.gutenberg.org/ebooks/{}", b.get("id").and_then(|v| v.as_u64()).unwrap_or(0));
            out.push(item(&page, title, ResourceKind::Book, "Gutenberg", page.clone(), dl, ext, 0.75));
        }
    }
    out
}

/// 百度图片搜索 → 直连、返回可用图片 URL（境内首选图片源，无需代理）。
pub async fn baidu_image(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://image.baidu.com/search/acjson?tn=resultjson_com&ipn=rj&word={}&pn=0&rn={}",
        urlencoding::encode(kw),
        limit.min(30)
    );
    let client = match client_cn() { Some(c) => c, None => return vec![] };
    let body = match client.get(&url).header("Referer", "https://image.baidu.com/").send().await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(_) => return vec![],
    };
    let json: serde_json::Value = match serde_json::from_str(&body) { Ok(j) => j, Err(_) => return vec![] };
    let mut out = Vec::new();
    if let Some(arr) = json.get("data").and_then(|v| v.as_array()) {
        for r in arr {
            let dl = r.get("middleURL").and_then(|v| v.as_str())
                .or_else(|| r.get("thumbURL").and_then(|v| v.as_str()))
                .or_else(|| r.get("hoverURL").and_then(|v| v.as_str()))
                .map(|s| s.to_string());
            let dl = match dl { Some(d) if !d.is_empty() => d, _ => continue };
            let title = r.get("fromPageTitleEnc").and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{kw} 图片"));
            let mut it = item(&dl, title, ResourceKind::Image, "百度图片",
                r.get("fromURL").and_then(|v| v.as_str()).unwrap_or(&dl).to_string(),
                Some(dl.clone()), Some("jpg"), 0.72);
            it.thumbnail = r.get("thumbURL").and_then(|v| v.as_str()).map(|s| s.to_string());
            out.push(it);
        }
    }
    out
}

/// Openverse → CC 图片 / 音频 直链。
pub async fn openverse(kw: &str, media: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!("https://api.openverse.org/v1/{}/?q={}&page_size={}", media, urlencoding::encode(kw), limit.min(30));
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let kind = if media == "audio" { ResourceKind::Audio } else { ResourceKind::Image };
    let mut out = Vec::new();
    if let Some(arr) = json.get("results").and_then(|v| v.as_array()) {
        for r in arr {
            let dl = r.get("url").and_then(|v| v.as_str()).map(|s| s.to_string());
            let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("Openverse 资源").to_string();
            let page = r.get("foreign_landing_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let key = dl.clone().unwrap_or_else(|| page.clone());
            if key.is_empty() { continue; }
            let mut it = item(&key, title, kind, "Openverse", page, dl, None, 0.7);
            it.thumbnail = r.get("thumbnail").and_then(|v| v.as_str()).map(|s| s.to_string());
            out.push(it);
        }
    }
    out
}

/// Library Genesis 搜索 → 书籍详情/下载页 + md5 直链（best-effort，镜像不稳时返回空）。
pub async fn libgen(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!("https://libgen.is/search.php?req={}&res=50&column=def", urlencoding::encode(kw));
    let client = match client() { Some(c) => c, None => return vec![] };
    let body = match client.get(&url).send().await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(_) => return vec![],
    };
    let doc = Html::parse_document(&body);
    let sel = match Selector::parse("a[href*='md5=']") { Ok(s) => s, Err(_) => return vec![] };
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for a in doc.select(&sel) {
        let href = a.value().attr("href").unwrap_or("");
        let title = a.text().collect::<String>().trim().to_string();
        // 只取书名链接（有意义的标题文本），跳过镜像小链接。
        if title.len() < 4 { continue; }
        let md5 = match href.split("md5=").nth(1) {
            Some(m) => m.split('&').next().unwrap_or("").to_string(),
            None => continue,
        };
        if md5.len() < 32 || !seen.insert(md5.clone()) { continue; }
        let page = format!("https://libgen.is/book/index.php?md5={md5}");
        let dl = format!("https://libgen.is/get.php?md5={md5}");
        out.push(item(&md5, title, ResourceKind::Book, "LibGen", page, Some(dl), None, 0.68));
        if out.len() >= limit { break; }
    }
    out
}

#[cfg(windows)]
fn no_window(cmd: &mut tokio::process::Command) {
    cmd.creation_flags(0x0800_0000);
}
#[cfg(not(windows))]
fn no_window(_cmd: &mut tokio::process::Command) {}

/// yt-dlp 直接搜索视频/音频。yt-dlp 缺失时按需联网下载（首次较慢）；
/// 若本会话已尝试下载失败(无代理等)，则不再重试，避免每次搜索都卡在下载超时。
static YTDLP_UNAVAILABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub async fn ytsearch(app: &AppHandle, kw: &str, media: &str, limit: usize) -> Vec<ResourceItem> {
    use std::sync::atomic::Ordering;
    if YTDLP_UNAVAILABLE.load(Ordering::Relaxed) {
        return vec![];
    }
    let bin = match ytdlp_fetch::ensure_ytdlp(app).await {
        Ok(p) => p,
        Err(_) => {
            YTDLP_UNAVAILABLE.store(true, Ordering::Relaxed);
            return vec![];
        }
    };
    let kind = if media == "audio" { ResourceKind::Audio } else { ResourceKind::Video };
    let query = format!("ytsearch{}:{}", limit.min(20), kw);
    let mut cmd = tokio::process::Command::new(&bin);
    cmd.arg(&query).arg("--flat-playlist").arg("--dump-json").arg("--no-warnings");
    if let Some(px) = proxy::proxy_arg(&super::load_proxy()) {
        cmd.arg("--proxy").arg(px);
    }
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
    no_window(&mut cmd);
    let out = match tokio::time::timeout(std::time::Duration::from_secs(25), cmd.output()).await {
        Ok(Ok(o)) => o,
        _ => return vec![],
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut items = Vec::new();
    for line in text.lines() {
        let v: serde_json::Value = match serde_json::from_str(line) { Ok(x) => x, Err(_) => continue };
        let title = v.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let page = v.get("url").and_then(|x| x.as_str())
            .or_else(|| v.get("webpage_url").and_then(|x| x.as_str()))
            .map(|s| {
                if s.starts_with("http") { s.to_string() }
                else { format!("https://www.youtube.com/watch?v={s}") }
            });
        let (page, title) = match (page, title.is_empty()) {
            (Some(p), false) => (p, title),
            _ => continue,
        };
        let mut it = item(&page, title, kind, "YouTube", page.clone(), None, None, 0.78);
        it.needs_ytdlp = true;
        items.push(it);
    }
    items
}

/// Semantic Scholar → 开放获取 PDF。补充论文广度。
pub async fn semantic_scholar(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://api.semanticscholar.org/graph/v1/paper/search?query={}&fields=title,openAccessPdf,year&limit={}",
        urlencoding::encode(kw), limit.min(30)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    if let Some(arr) = json.get("data").and_then(|v| v.as_array()) {
        for p in arr {
            let title = p.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let pdf = p.get("openAccessPdf").and_then(|o| o.get("url")).and_then(|v| v.as_str()).map(|s| s.to_string());
            if title.is_empty() || pdf.is_none() { continue; }
            let src = pdf.clone().unwrap();
            out.push(item(&src, title, ResourceKind::Paper, "SemanticScholar", src.clone(), pdf, Some("pdf"), 0.72));
        }
    }
    out
}

