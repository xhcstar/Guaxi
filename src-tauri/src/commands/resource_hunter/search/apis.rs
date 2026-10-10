//! 领域富集站 API 适配器：直接查结构化接口，拿到干净的文件直链，
//! 精度远高于 SERP 抓取。全部走境外代理路由(client_for_source foreign=true)，无需 key。
use super::super::model::{ResourceItem, ResourceKind};
use super::super::{proxy, ytdlp_fetch};
use super::make_id;
use futures_util::future::join_all;
use scraper::{Html, Selector};
use tauri::AppHandle;

fn client() -> Option<reqwest::Client> {
    proxy::client_for_source(&super::load_proxy(), true).ok()
}

/// 境内直连客户端（百度等国内源不走代理）。
fn client_cn() -> Option<reqwest::Client> {
    proxy::client_for_source(&super::load_proxy(), false).ok()
}

/// 读取用户在设置里配置的第三方 API Key（未配置返回 None，对应源自动跳过）。
/// 存储为 `api_keys` 设置项下的 JSON 对象，键名见各源实现。
pub(crate) fn api_key(name: &str) -> Option<String> {
    let raw = super::store::get_setting("api_keys").ok().flatten()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    json.get(name)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Unpaywall 需要一个联系邮箱（非密钥）；用户可配，否则用占位。
fn unpaywall_email() -> String {
    api_key("unpaywall_email").unwrap_or_else(|| "resource-hunter@example.com".into())
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

/// 识别 Cloudflare/反爬挑战页或空壳页：返回的是 JS 质询而非真实结果时应跳过换镜像，
/// 而不是把挑战页当成“无结果”静默吞掉（旧版书源召回塌陷的主因之一）。
fn looks_blocked(body: &str) -> bool {
    if body.len() < 500 {
        return true;
    }
    let lc = body.to_ascii_lowercase();
    lc.contains("just a moment")
        || lc.contains("cf-chl")
        || lc.contains("challenge-platform")
        || lc.contains("enable javascript and cookies")
        || lc.contains("attention required")
        || lc.contains("/cdn-cgi/challenge")
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

/// Library Genesis 搜索 → 书籍详情/下载页 + md5 直链。
/// 单个镜像常年不稳，这里按序尝试多个镜像域，首个返回结果者胜出（best-effort）。
pub async fn libgen(kw: &str, limit: usize) -> Vec<ResourceItem> {
    // 常见活跃镜像；顺序尝试，命中即止。不同镜像 HTML 结构一致（Gen 经典表格 + md5= 链接）。
    const MIRRORS: [&str; 8] = [
        "libgen.is", "libgen.rs", "libgen.st", "libgen.gs", "libgen.la", "libgen.vg",
        "libgen.gl", "libgen.pm",
    ];
    let client = match client() { Some(c) => c, None => return vec![] };
    for host in MIRRORS {
        let url = format!("https://{host}/search.php?req={}&res=50&column=def", urlencoding::encode(kw));
        let body = match client.get(&url).send().await {
            Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
            _ => continue,
        };
        if looks_blocked(&body) { continue; }
        let out = parse_libgen(&body, host, limit);
        if !out.is_empty() { return out; }
    }
    vec![]
}

/// 解析 LibGen 经典搜索结果页：抽 md5= 链接与书名。
fn parse_libgen(body: &str, host: &str, limit: usize) -> Vec<ResourceItem> {
    let doc = Html::parse_document(body);
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
        let page = format!("https://{host}/book/index.php?md5={md5}");
        let dl = format!("https://{host}/get.php?md5={md5}");
        out.push(item(&md5, title, ResourceKind::Book, "LibGen", page, Some(dl), None, 0.68));
        if out.len() >= limit { break; }
    }
    out
}

/// Anna's Archive（全网最大影子图书馆聚合：libgen+z-lib+sci-hub+IA）→ 书籍发现。
/// 直链需会员/慢速通道，这里只给详情页（供发现与跳转），已能极大提升书籍召回。
/// 多域名兜底：主域被墙/CF 拦截时切换镜像。
pub async fn annas_archive(kw: &str, limit: usize) -> Vec<ResourceItem> {
    // .se/.li 镜像通常比主域 .org 更少被 CF 质询，优先尝试。
    const HOSTS: [&str; 4] = ["annas-archive.se", "annas-archive.li", "annas-archive.org", "annas-archive.gs"];
    let client = match client() { Some(c) => c, None => return vec![] };
    for host in HOSTS {
        let url = format!("https://{host}/search?q={}", urlencoding::encode(kw));
        let body = match client.get(&url).send().await {
            Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
            _ => continue,
        };
        if looks_blocked(&body) { continue; }
        let out = parse_annas(&body, host, limit);
        if !out.is_empty() { return out; }
    }
    vec![]
}

/// 解析 Anna's Archive 搜索结果：抽 /md5/<hash> 卡片与标题。
/// 结果卡有时被包在 HTML 注释里（懒加载），一并扫描注释文本。
fn parse_annas(body: &str, host: &str, limit: usize) -> Vec<ResourceItem> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    // 卡片链接形如 href="/md5/<32hex>"，标题在其后的元素文本里。
    // 直接扫原文，稳过 DOM 变动 + 注释懒加载两种情况。
    let mut rest = body;
    while let Some(pos) = rest.find("/md5/") {
        let after = &rest[pos + 5..];
        let hash: String = after.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        rest = &after[hash.len().min(after.len())..];
        if hash.len() < 16 || !seen.insert(hash.clone()) { continue; }
        // 标题：取该 md5 链接附近向后一段里的可读文本（h3/div 文本），best-effort。
        let title = extract_annas_title(rest).unwrap_or_else(|| format!("Anna's Archive · {}", &hash[..hash.len().min(8)]));
        let page = format!("https://{host}/md5/{hash}");
        out.push(item(&hash, title, ResourceKind::Book, "AnnasArchive", page, None, None, 0.7));
        if out.len() >= limit { break; }
    }
    out
}

/// 从 /md5/ 链接后的 HTML 片段里提取首个较长的可读文本作为书名。
fn extract_annas_title(frag: &str) -> Option<String> {
    // 取接下来 ~1200 字符里，标签之间最长的一段纯文本（书名通常最长）。
    let window = &frag[..frag.len().min(1200)];
    let mut best = String::new();
    let mut cur = String::new();
    let mut in_tag = false;
    for ch in window.chars() {
        match ch {
            '<' => { in_tag = true; if cur.trim().len() > best.trim().len() { best = cur.clone(); } cur.clear(); }
            '>' => in_tag = false,
            _ if !in_tag => cur.push(ch),
            _ => {}
        }
    }
    let t = best.trim();
    if t.len() >= 4 { Some(t.chars().take(160).collect()) } else { None }
}

/// Invidious（YouTube 隐私前端）→ 视频发现，keyless JSON，无需本地 yt-dlp 二进制即可召回。
/// 下载仍走 yt-dlp（needs_ytdlp），但发现阶段不再受二进制缺失/搜索超时拖累。
/// 多实例兜底：公共实例常轮换，逐个尝试直到拿到结果。
pub async fn invidious(kw: &str, limit: usize) -> Vec<ResourceItem> {
    const INSTANCES: [&str; 5] = [
        "https://inv.nadeko.net",
        "https://invidious.nerdvpn.de",
        "https://yewtu.be",
        "https://invidious.jing.rocks",
        "https://inv.tux.pizza",
    ];
    let client = match client() { Some(c) => c, None => return vec![] };
    for base in INSTANCES {
        let url = format!("{base}/api/v1/search?q={}&type=video&sort_by=relevance", urlencoding::encode(kw));
        let resp = match client.get(&url).send().await {
            Ok(r) if r.status().is_success() => r, _ => continue,
        };
        let json: serde_json::Value = match resp.json().await { Ok(j) => j, Err(_) => continue };
        let arr = match json.as_array() { Some(a) => a, None => continue };
        let mut out = Vec::new();
        for r in arr.iter().take(limit) {
            if r.get("type").and_then(|v| v.as_str()) != Some("video") { continue; }
            let vid = r.get("videoId").and_then(|v| v.as_str()).unwrap_or("");
            let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if vid.is_empty() || title.is_empty() { continue; }
            let page = format!("https://www.youtube.com/watch?v={vid}");
            let mut it = item(&page, title, ResourceKind::Video, "YouTube", page.clone(), None, None, 0.78);
            it.needs_ytdlp = true;
            it.author = r.get("author").and_then(|v| v.as_str()).map(|s| s.to_string());
            it.thumbnail = r.get("videoThumbnails").and_then(|v| v.as_array())
                .and_then(|a| a.iter().find(|t| t.get("quality").and_then(|q| q.as_str()) == Some("medium")).or_else(|| a.first()))
                .and_then(|t| t.get("url")).and_then(|v| v.as_str()).map(|s| s.to_string());
            out.push(it);
        }
        if !out.is_empty() { return out; }
    }
    vec![]
}

/// 哔哩哔哩视频搜索（境内直连，keyless）→ 视频页，下载走 yt-dlp。中文视频主力召回源。
/// B 站有风控：需浏览器 UA + Referer + buvid3 cookie；伪造一个 buvid3 即可 best-effort 放行。
pub async fn bilibili_search(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let client = match client_cn() { Some(c) => c, None => return vec![] };
    let url = format!(
        "https://api.bilibili.com/x/web-interface/search/type?search_type=video&keyword={}&page=1",
        urlencoding::encode(kw)
    );
    // 伪造 buvid3（B 站接受任意 <uuid>infoc 形态），规避 -412 风控。
    let buvid = format!("{}infoc", make_id(&format!("{kw}-buvid")).to_uppercase());
    let resp = match client
        .get(&url)
        .header("Referer", "https://www.bilibili.com/")
        .header("Cookie", format!("buvid3={buvid}"))
        .send()
        .await
    {
        Ok(r) => r, Err(_) => return vec![],
    };
    let json: serde_json::Value = match resp.json().await { Ok(j) => j, Err(_) => return vec![] };
    let arr = match json.get("data").and_then(|d| d.get("result")).and_then(|v| v.as_array()) {
        Some(a) => a.clone(), None => return vec![],
    };
    let mut out = Vec::new();
    for r in arr.iter().take(limit) {
        let bvid = r.get("bvid").and_then(|v| v.as_str()).unwrap_or("");
        if bvid.is_empty() { continue; }
        // 标题含 <em class="keyword"> 高亮标签，去标签。
        let raw = r.get("title").and_then(|v| v.as_str()).unwrap_or("");
        let title = strip_html_tags(raw);
        if title.is_empty() { continue; }
        let page = format!("https://www.bilibili.com/video/{bvid}");
        let mut it = item(&page, title, ResourceKind::Video, "Bilibili", page.clone(), None, None, 0.76);
        it.needs_ytdlp = true;
        it.author = r.get("author").and_then(|v| v.as_str()).map(|s| s.to_string());
        it.thumbnail = r.get("pic").and_then(|v| v.as_str()).map(|s| {
            if s.starts_with("//") { format!("https:{s}") } else { s.to_string() }
        });
        out.push(it);
    }
    out
}

/// 去除简单 HTML 标签（B 站标题的 <em> 高亮等），返回纯文本。
fn strip_html_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.trim().chars().take(160).collect()
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

/// Crossref → 学术论文元数据 + 可能的 PDF 直链。覆盖面极广，补充论文广度。
pub async fn crossref(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://api.crossref.org/works?query={}&rows={}&select=title,author,issued,link,DOI,URL",
        urlencoding::encode(kw), limit.min(30)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    if let Some(arr) = json.get("message").and_then(|m| m.get("items")).and_then(|v| v.as_array()) {
        for w in arr {
            let title = w.get("title").and_then(|v| v.as_array()).and_then(|a| a.first())
                .and_then(|v| v.as_str()).unwrap_or("").to_string();
            if title.is_empty() { continue; }
            let pdf = w.get("link").and_then(|v| v.as_array()).and_then(|arr| {
                arr.iter().find(|l| l.get("content-type").and_then(|c| c.as_str()) == Some("application/pdf"))
                    .and_then(|l| l.get("URL")).and_then(|v| v.as_str()).map(|s| s.to_string())
            });
            let page = w.get("URL").and_then(|v| v.as_str())
                .or_else(|| w.get("DOI").and_then(|v| v.as_str())).unwrap_or("").to_string();
            let author = w.get("author").and_then(|v| v.as_array()).and_then(|a| a.first()).map(|a| {
                let given = a.get("given").and_then(|v| v.as_str()).unwrap_or("");
                let family = a.get("family").and_then(|v| v.as_str()).unwrap_or("");
                format!("{given} {family}").trim().to_string()
            }).filter(|s| !s.is_empty());
            let year = w.get("issued").and_then(|i| i.get("date-parts"))
                .and_then(|v| v.as_array()).and_then(|a| a.first())
                .and_then(|v| v.as_array()).and_then(|a| a.first())
                .and_then(|v| v.as_u64()).map(|y| y.to_string());
            let src = pdf.clone().unwrap_or_else(|| page.clone());
            if src.is_empty() { continue; }
            let mut it = item(&src, title, ResourceKind::Paper, "Crossref", page,
                pdf.clone(), pdf.as_ref().map(|_| "pdf"), if pdf.is_some() { 0.72 } else { 0.58 });
            it.author = author;
            it.published_at = year;
            out.push(it);
        }
    }
    out
}

/// DOAJ → 开放获取期刊论文（多数带全文/PDF）。
pub async fn doaj(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://doaj.org/api/search/articles/{}?pageSize={}",
        urlencoding::encode(kw), limit.min(30)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    if let Some(arr) = json.get("results").and_then(|v| v.as_array()) {
        for r in arr {
            let bj = match r.get("bibjson") { Some(b) => b, None => continue };
            let title = bj.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if title.is_empty() { continue; }
            let full = bj.get("link").and_then(|v| v.as_array()).and_then(|arr| {
                arr.iter().find(|l| l.get("type").and_then(|c| c.as_str()) == Some("fulltext"))
                    .and_then(|l| l.get("url")).and_then(|v| v.as_str()).map(|s| s.to_string())
            });
            let is_pdf = full.as_ref().map(|u| u.to_ascii_lowercase().ends_with(".pdf")).unwrap_or(false);
            let page = full.clone().unwrap_or_default();
            let author = bj.get("author").and_then(|v| v.as_array()).and_then(|a| a.first())
                .and_then(|a| a.get("name")).and_then(|v| v.as_str()).map(|s| s.to_string());
            let year = bj.get("year").and_then(|v| v.as_str()).map(|s| s.to_string());
            let src = full.clone().unwrap_or_else(|| title.clone());
            let mut it = item(&src, title, ResourceKind::Paper, "DOAJ", page,
                if is_pdf { full.clone() } else { None }, if is_pdf { Some("pdf") } else { None },
                if is_pdf { 0.7 } else { 0.55 });
            it.author = author;
            it.published_at = year;
            out.push(it);
        }
    }
    out
}
// PLACEHOLDER_APIS_MORE

/// 解析 archive.org 条目的首个可用文件直链（按期望扩展名优先级挑选）。
/// 返回 (下载直链, 扩展名, 字节大小)。
async fn ia_first_file(id: &str, want_exts: &[&str]) -> Option<(String, String, Option<u64>)> {
    let meta = get_json(&format!("https://archive.org/metadata/{id}")).await?;
    // 借阅馆藏/受限/暗馆条目直链会返回 401 页面，跳过避免下到垃圾 HTML。
    let restricted = meta.get("metadata")
        .and_then(|m| m.get("access-restricted-item"))
        .and_then(|v| v.as_str()) == Some("true");
    let dark = meta.get("is_dark").and_then(|v| v.as_bool()) == Some(true);
    if restricted || dark {
        return None;
    }
    let files = meta.get("files")?.as_array()?;
    for ext in want_exts {
        for f in files {
            let name = f.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let lc = name.to_ascii_lowercase();
            if lc.ends_with(&format!(".{ext}")) && !lc.contains("_thumb") && !lc.starts_with("__ia") {
                let size = f.get("size").and_then(|v| v.as_str()).and_then(|s| s.parse::<u64>().ok());
                let url = format!("https://archive.org/download/{id}/{}", urlencoding::encode(name));
                return Some((url, ext.to_string(), size));
            }
        }
    }
    None
}

/// Internet Archive → 音频/视频/文本的直链。海量公开资源，无需 key。
pub async fn internet_archive(kw: &str, mediatype: &str, kind: ResourceKind, limit: usize) -> Vec<ResourceItem> {
    let q = format!("{kw} AND mediatype:{mediatype}");
    let url = format!(
        "https://archive.org/advancedsearch.php?q={}&fl[]=identifier&fl[]=title&rows={}&page=1&output=json",
        urlencoding::encode(&q), limit.min(20)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let docs = match json.get("response").and_then(|r| r.get("docs")).and_then(|v| v.as_array()) {
        Some(d) => d.clone(), None => return vec![],
    };
    let exts: &[&str] = match kind {
        ResourceKind::Audio => &["mp3", "flac", "m4a", "ogg"],
        ResourceKind::Video => &["mp4", "mkv", "webm", "ogv"],
        _ => &["pdf", "epub", "txt"],
    };
    let futs = docs.into_iter().map(|d| async move {
        let id = d.get("identifier").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let title = d.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if id.is_empty() || title.is_empty() { return None; }
        let (dl, ext, size) = ia_first_file(&id, exts).await?;
        let page = format!("https://archive.org/details/{id}");
        let mut it = item(&dl, title, kind, "InternetArchive", page, Some(dl.clone()), Some(ext.as_str()), 0.66);
        it.size_hint = size;
        Some(it)
    });
    join_all(futs).await.into_iter().flatten().collect()
}

/// Open Library → 书籍元数据；有 archive.org 全文的解析出真实直链，否则给详情页。
pub async fn open_library(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://openlibrary.org/search.json?q={}&limit={}&fields=title,author_name,first_publish_year,ia,cover_i,key",
        urlencoding::encode(kw), limit.min(20)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let docs = match json.get("docs").and_then(|v| v.as_array()) {
        Some(d) => d.clone(), None => return vec![],
    };
    let futs = docs.into_iter().map(|d| async move {
        let title = d.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if title.is_empty() { return None; }
        let key = d.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let page = format!("https://openlibrary.org{key}");
        let author = d.get("author_name").and_then(|v| v.as_array()).and_then(|a| a.first())
            .and_then(|v| v.as_str()).map(|s| s.to_string());
        let year = d.get("first_publish_year").and_then(|v| v.as_u64()).map(|y| y.to_string());
        let ia = d.get("ia").and_then(|v| v.as_array()).and_then(|a| a.first()).and_then(|v| v.as_str());
        let (dl, ext, size, rel) = if let Some(id) = ia {
            match ia_first_file(id, &["pdf", "epub", "txt"]).await {
                Some((u, e, s)) => (Some(u), Some(e), s, 0.72),
                None => (None, None, None, 0.55),
            }
        } else {
            (None, None, None, 0.55)
        };
        let src = dl.clone().unwrap_or_else(|| page.clone());
        let mut it = item(&src, title, ResourceKind::Book, "OpenLibrary", page,
            dl, ext.as_deref(), rel);
        it.author = author;
        it.published_at = year;
        it.size_hint = size;
        if let Some(cid) = d.get("cover_i").and_then(|v| v.as_u64()) {
            it.thumbnail = Some(format!("https://covers.openlibrary.org/b/id/{cid}-M.jpg"));
        }
        Some(it)
    });
    join_all(futs).await.into_iter().flatten().collect()
}

/// Wikimedia Commons → 自由许可图片直链（含缩略图）。境外图片主力源。
pub async fn wikimedia_commons(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://commons.wikimedia.org/w/api.php?action=query&generator=search&gsrsearch={}\
         &gsrnamespace=6&gsrlimit={}&prop=imageinfo&iiprop=url|size|mime&iiurlwidth=320&format=json&origin=*",
        urlencoding::encode(kw), limit.min(30)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    if let Some(pages) = json.get("query").and_then(|q| q.get("pages")).and_then(|v| v.as_object()) {
        for (_, p) in pages {
            let title = p.get("title").and_then(|v| v.as_str()).unwrap_or("")
                .trim_start_matches("File:").to_string();
            let info = match p.get("imageinfo").and_then(|v| v.as_array()).and_then(|a| a.first()) {
                Some(i) => i, None => continue,
            };
            let mime = info.get("mime").and_then(|v| v.as_str()).unwrap_or("");
            if !mime.starts_with("image/") || mime.contains("svg") { continue; }
            let dl = match info.get("url").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u.to_string(), _ => continue,
            };
            let page = info.get("descriptionurl").and_then(|v| v.as_str()).unwrap_or(&dl).to_string();
            let ext = mime.rsplit('/').next().map(|s| if s == "jpeg" { "jpg" } else { s });
            let mut it = item(&dl, title, ResourceKind::Image, "Wikimedia", page, Some(dl.clone()), ext, 0.7);
            it.thumbnail = info.get("thumburl").and_then(|v| v.as_str()).map(|s| s.to_string());
            it.size_hint = info.get("size").and_then(|v| v.as_u64());
            out.push(it);
        }
    }
    out
}

// ============ T1 扩展：更多结构化直链源（论文/书籍/图片/音频/代码）============

/// Unpaywall → 开放获取论文的合法 PDF 直链。DOI 级权威，召回大量免费全文。
pub async fn unpaywall(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://api.unpaywall.org/v2/search?query={}&email={}",
        urlencoding::encode(kw), urlencoding::encode(&unpaywall_email())
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    if let Some(arr) = json.get("results").and_then(|v| v.as_array()) {
        for r in arr.iter().take(limit) {
            let resp = match r.get("response") { Some(x) => x, None => continue };
            let title = resp.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if title.is_empty() { continue; }
            let oa = resp.get("best_oa_location");
            let pdf = oa.and_then(|o| o.get("url_for_pdf")).and_then(|v| v.as_str()).map(|s| s.to_string());
            let landing = oa.and_then(|o| o.get("url")).and_then(|v| v.as_str())
                .or_else(|| resp.get("doi_url").and_then(|v| v.as_str())).unwrap_or("").to_string();
            let src = pdf.clone().unwrap_or_else(|| landing.clone());
            if src.is_empty() { continue; }
            let mut it = item(&src, title, ResourceKind::Paper, "Unpaywall", landing,
                pdf.clone(), pdf.as_ref().map(|_| "pdf"), if pdf.is_some() { 0.9 } else { 0.6 });
            it.published_at = resp.get("year").and_then(|v| v.as_u64()).map(|y| y.to_string());
            out.push(it);
        }
    }
    out
}

/// Europe PMC → 生物医学+全学科论文，带全文 PDF 链接（PMC OA）。
pub async fn europepmc(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://www.ebi.ac.uk/europepmc/webservices/rest/search?query={}&format=json&pageSize={}&resultType=core",
        urlencoding::encode(kw), limit.min(25)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    if let Some(arr) = json.get("resultList").and_then(|r| r.get("result")).and_then(|v| v.as_array()) {
        for r in arr {
            let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if title.is_empty() { continue; }
            let pdf = r.get("fullTextUrlList").and_then(|f| f.get("fullTextUrl")).and_then(|v| v.as_array())
                .and_then(|arr| arr.iter().find(|u| u.get("documentStyle").and_then(|d| d.as_str()) == Some("pdf"))
                    .or_else(|| arr.first()))
                .and_then(|u| u.get("url")).and_then(|v| v.as_str()).map(|s| s.to_string());
            let doi = r.get("doi").and_then(|v| v.as_str()).map(|d| format!("https://doi.org/{d}"));
            let landing = pdf.clone().or(doi).unwrap_or_default();
            let src = pdf.clone().unwrap_or_else(|| landing.clone());
            if src.is_empty() { continue; }
            let is_pdf = pdf.as_ref().map(|u| u.to_ascii_lowercase().contains("pdf")).unwrap_or(false);
            let mut it = item(&src, title, ResourceKind::Paper, "EuropePMC", landing,
                if is_pdf { pdf.clone() } else { None }, if is_pdf { Some("pdf") } else { None },
                if is_pdf { 0.82 } else { 0.58 });
            it.author = r.get("authorString").and_then(|v| v.as_str()).map(|s| s.to_string());
            it.published_at = r.get("pubYear").and_then(|v| v.as_str()).map(|s| s.to_string());
            out.push(it);
        }
    }
    out
}
// PLACEHOLDER_T1_PAPERS

/// 带自定义请求头的 JSON GET（用于需要 Authorization/API-Key 头的源）。
async fn get_json_hdr(url: &str, header: &str, value: &str) -> Option<serde_json::Value> {
    let c = client()?;
    c.get(url).header(header, value).send().await.ok()?.json().await.ok()
}

/// CORE → 全球最大开放获取论文聚合，直给 PDF。需 API Key（免费申请，设置里填 "core"）。
pub async fn core_api(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let key = match api_key("core") { Some(k) => k, None => return vec![] };
    let url = format!(
        "https://api.core.ac.uk/v3/search/works?q={}&limit={}",
        urlencoding::encode(kw), limit.min(25)
    );
    let json = match get_json_hdr(&url, "Authorization", &format!("Bearer {key}")).await {
        Some(j) => j, None => return vec![],
    };
    let mut out = Vec::new();
    if let Some(arr) = json.get("results").and_then(|v| v.as_array()) {
        for r in arr {
            let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if title.is_empty() { continue; }
            let pdf = r.get("downloadUrl").and_then(|v| v.as_str()).map(|s| s.to_string())
                .filter(|s| !s.is_empty());
            let landing = r.get("doi").and_then(|v| v.as_str()).map(|d| format!("https://doi.org/{d}"))
                .or_else(|| pdf.clone()).unwrap_or_default();
            let src = pdf.clone().unwrap_or_else(|| landing.clone());
            if src.is_empty() { continue; }
            let mut it = item(&src, title, ResourceKind::Paper, "CORE", landing,
                pdf.clone(), pdf.as_ref().map(|_| "pdf"), if pdf.is_some() { 0.84 } else { 0.6 });
            it.published_at = r.get("yearPublished").and_then(|v| v.as_u64()).map(|y| y.to_string());
            out.push(it);
        }
    }
    out
}

/// DOAB（开放获取图书目录）→ 合法整本 PDF 直链。
pub async fn doab(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://directory.doabooks.org/rest/search?query={}&expand=metadata,bitstreams",
        urlencoding::encode(kw)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let arr = match json.as_array() { Some(a) => a.clone(), None => return vec![] };
    let mut out = Vec::new();
    for b in arr.iter().take(limit) {
        let meta = b.get("metadata").and_then(|v| v.as_array());
        let field = |name: &str| meta.and_then(|m| m.iter()
            .find(|x| x.get("key").and_then(|k| k.as_str()) == Some(name))
            .and_then(|x| x.get("value")).and_then(|v| v.as_str()).map(|s| s.to_string()));
        let title = field("dc.title").unwrap_or_default();
        if title.is_empty() { continue; }
        let handle = b.get("handle").and_then(|v| v.as_str()).unwrap_or("");
        let page = format!("https://directory.doabooks.org/handle/{handle}");
        let pdf = b.get("bitstreams").and_then(|v| v.as_array())
            .and_then(|arr| arr.iter().find(|s| s.get("mimeType").and_then(|m| m.as_str()) == Some("application/pdf")))
            .and_then(|s| s.get("retrieveLink").and_then(|v| v.as_str()))
            .map(|l| format!("https://directory.doabooks.org{l}"));
        let src = pdf.clone().unwrap_or_else(|| page.clone());
        let mut it = item(&src, title, ResourceKind::Book, "DOAB", page,
            pdf.clone(), pdf.as_ref().map(|_| "pdf"), if pdf.is_some() { 0.8 } else { 0.6 });
        it.author = field("dc.contributor.author");
        out.push(it);
    }
    out
}
// PLACEHOLDER_T1_BOOKS

/// LibriVox → 公版有声书整本 zip（mp3）直链。音频领域强召回。
pub async fn librivox(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://librivox.org/api/feed/audiobooks/?title=^{}&format=json&limit={}",
        urlencoding::encode(kw), limit.min(20)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    if let Some(arr) = json.get("books").and_then(|v| v.as_array()) {
        for b in arr {
            let title = b.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if title.is_empty() { continue; }
            let zip = b.get("url_zip_file").and_then(|v| v.as_str()).map(|s| s.to_string());
            let page = b.get("url_librivox").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let src = zip.clone().unwrap_or_else(|| page.clone());
            if src.is_empty() { continue; }
            let author = b.get("authors").and_then(|v| v.as_array()).and_then(|a| a.first()).map(|a| {
                let f = a.get("first_name").and_then(|v| v.as_str()).unwrap_or("");
                let l = a.get("last_name").and_then(|v| v.as_str()).unwrap_or("");
                format!("{f} {l}").trim().to_string()
            }).filter(|s| !s.is_empty());
            let mut it = item(&src, title, ResourceKind::Audio, "LibriVox", page,
                zip.clone(), zip.as_ref().map(|_| "zip"), if zip.is_some() { 0.78 } else { 0.55 });
            it.author = author;
            out.push(it);
        }
    }
    out
}

/// Jamendo → 自由授权音乐 mp3 直链。需 client_id（设置里填 "jamendo"）。
pub async fn jamendo(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let key = match api_key("jamendo") { Some(k) => k, None => return vec![] };
    let url = format!(
        "https://api.jamendo.com/v3.0/tracks/?client_id={}&format=json&limit={}&search={}&audioformat=mp32",
        key, limit.min(30), urlencoding::encode(kw)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    if let Some(arr) = json.get("results").and_then(|v| v.as_array()) {
        for r in arr {
            let title = r.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let dl = r.get("audiodownload").and_then(|v| v.as_str()).map(|s| s.to_string())
                .filter(|s| !s.is_empty());
            let dl = match dl { Some(d) => d, None => continue };
            if title.is_empty() { continue; }
            let page = r.get("shareurl").and_then(|v| v.as_str()).unwrap_or(&dl).to_string();
            let mut it = item(&dl, title, ResourceKind::Audio, "Jamendo", page, Some(dl.clone()), Some("mp3"), 0.76);
            it.author = r.get("artist_name").and_then(|v| v.as_str()).map(|s| s.to_string());
            it.thumbnail = r.get("image").and_then(|v| v.as_str()).map(|s| s.to_string());
            out.push(it);
        }
    }
    out
}
// PLACEHOLDER_T1_AUDIO

/// 大都会艺术博物馆开放馆藏 → 高清艺术图片直链（公有领域）。keyless。
pub async fn met_museum(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://collectionapi.metmuseum.org/public/collection/v1/search?q={}&hasImages=true",
        urlencoding::encode(kw)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let ids: Vec<u64> = json.get("objectIDs").and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_u64()).take(limit.min(15)).collect())
        .unwrap_or_default();
    let futs = ids.into_iter().map(|id| async move {
        let o = get_json(&format!("https://collectionapi.metmuseum.org/public/collection/v1/objects/{id}")).await?;
        let dl = o.get("primaryImage").and_then(|v| v.as_str()).filter(|s| !s.is_empty())?.to_string();
        let title = o.get("title").and_then(|v| v.as_str()).filter(|s| !s.is_empty())?.to_string();
        let page = o.get("objectURL").and_then(|v| v.as_str()).unwrap_or(&dl).to_string();
        let mut it = item(&dl, title, ResourceKind::Image, "MetMuseum", page, Some(dl.clone()), Some("jpg"), 0.75);
        it.thumbnail = o.get("primaryImageSmall").and_then(|v| v.as_str()).map(|s| s.to_string());
        it.author = o.get("artistDisplayName").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
        Some(it)
    });
    join_all(futs).await.into_iter().flatten().collect()
}

/// NASA 影像库 → 图片直链。keyless。
pub async fn nasa_images(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!("https://images-api.nasa.gov/search?q={}&media_type=image", urlencoding::encode(kw));
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    if let Some(arr) = json.get("collection").and_then(|c| c.get("items")).and_then(|v| v.as_array()) {
        for r in arr.iter().take(limit) {
            let data = r.get("data").and_then(|v| v.as_array()).and_then(|a| a.first());
            let title = data.and_then(|d| d.get("title")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let dl = r.get("links").and_then(|v| v.as_array()).and_then(|a| a.first())
                .and_then(|l| l.get("href")).and_then(|v| v.as_str()).map(|s| s.to_string());
            let dl = match dl { Some(d) => d, None => continue };
            if title.is_empty() { continue; }
            let mut it = item(&dl, title, ResourceKind::Image, "NASA", dl.clone(), Some(dl.clone()), Some("jpg"), 0.72);
            it.thumbnail = Some(dl);
            it.summary = data.and_then(|d| d.get("description")).and_then(|v| v.as_str())
                .map(|s| s.chars().take(200).collect());
            out.push(it);
        }
    }
    out
}

/// Pexels → 高质量图片/视频直链。需 API Key（设置里填 "pexels"）。
pub async fn pexels(kw: &str, media: &str, limit: usize) -> Vec<ResourceItem> {
    let key = match api_key("pexels") { Some(k) => k, None => return vec![] };
    let (url, is_video) = if media == "video" {
        (format!("https://api.pexels.com/videos/search?query={}&per_page={}", urlencoding::encode(kw), limit.min(20)), true)
    } else {
        (format!("https://api.pexels.com/v1/search?query={}&per_page={}", urlencoding::encode(kw), limit.min(20)), false)
    };
    let json = match get_json_hdr(&url, "Authorization", &key).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    if is_video {
        if let Some(arr) = json.get("videos").and_then(|v| v.as_array()) {
            for r in arr {
                let files = r.get("video_files").and_then(|v| v.as_array());
                let dl = files.and_then(|f| f.iter().max_by_key(|x| x.get("width").and_then(|w| w.as_u64()).unwrap_or(0)))
                    .and_then(|f| f.get("link")).and_then(|v| v.as_str()).map(|s| s.to_string());
                let dl = match dl { Some(d) => d, None => continue };
                let page = r.get("url").and_then(|v| v.as_str()).unwrap_or(&dl).to_string();
                let title = format!("Pexels 视频 · {}", r.get("id").and_then(|v| v.as_u64()).unwrap_or(0));
                let mut it = item(&dl, title, ResourceKind::Video, "Pexels", page, Some(dl.clone()), Some("mp4"), 0.75);
                it.thumbnail = r.get("image").and_then(|v| v.as_str()).map(|s| s.to_string());
                out.push(it);
            }
        }
    } else if let Some(arr) = json.get("photos").and_then(|v| v.as_array()) {
        for r in arr {
            let dl = r.get("src").and_then(|s| s.get("original")).and_then(|v| v.as_str()).map(|s| s.to_string());
            let dl = match dl { Some(d) => d, None => continue };
            let page = r.get("url").and_then(|v| v.as_str()).unwrap_or(&dl).to_string();
            let title = r.get("alt").and_then(|v| v.as_str()).filter(|s| !s.is_empty())
                .map(|s| s.to_string()).unwrap_or_else(|| format!("{kw} 图片"));
            let mut it = item(&dl, title, ResourceKind::Image, "Pexels", page, Some(dl.clone()), Some("jpg"), 0.75);
            it.thumbnail = r.get("src").and_then(|s| s.get("medium")).and_then(|v| v.as_str()).map(|s| s.to_string());
            it.author = r.get("photographer").and_then(|v| v.as_str()).map(|s| s.to_string());
            out.push(it);
        }
    }
    out
}
// PLACEHOLDER_T1_IMAGES

/// crates.io → Rust 库 .crate 源码包直链。代码领域。keyless。
pub async fn crates_io(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!("https://crates.io/api/v1/crates?q={}&per_page={}", urlencoding::encode(kw), limit.min(20));
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    if let Some(arr) = json.get("crates").and_then(|v| v.as_array()) {
        for c in arr {
            let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let ver = c.get("newest_version").and_then(|v| v.as_str()).unwrap_or("");
            if name.is_empty() || ver.is_empty() { continue; }
            let dl = format!("https://crates.io/api/v1/crates/{name}/{ver}/download");
            let page = format!("https://crates.io/crates/{name}");
            let desc = c.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let title = if desc.is_empty() { format!("{name} {ver}") } else { format!("{name} — {desc}") };
            out.push(item(&dl, title, ResourceKind::Code, "crates.io", page, Some(dl.clone()), Some("crate"), 0.8));
        }
    }
    out
}

/// npm registry → 包 tarball 直链。代码领域。keyless。
pub async fn npm_pkg(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!("https://registry.npmjs.org/-/v1/search?text={}&size={}", urlencoding::encode(kw), limit.min(20));
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    if let Some(arr) = json.get("objects").and_then(|v| v.as_array()) {
        for o in arr {
            let p = match o.get("package") { Some(x) => x, None => continue };
            let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let ver = p.get("version").and_then(|v| v.as_str()).unwrap_or("");
            if name.is_empty() || ver.is_empty() { continue; }
            // 非 scoped 包可拼出 tarball 直链；scoped(@a/b) 结构不同，退回 npm 详情页。
            let (dl, ext) = if name.starts_with('@') {
                (None, None)
            } else {
                (Some(format!("https://registry.npmjs.org/{name}/-/{name}-{ver}.tgz")), Some("tgz"))
            };
            let page = p.get("links").and_then(|l| l.get("npm")).and_then(|v| v.as_str())
                .map(|s| s.to_string()).unwrap_or_else(|| format!("https://www.npmjs.com/package/{name}"));
            let desc = p.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let title = if desc.is_empty() { format!("{name} {ver}") } else { format!("{name} — {desc}") };
            let src = dl.clone().unwrap_or_else(|| page.clone());
            out.push(item(&src, title, ResourceKind::Code, "npm", page, dl, ext, if ext.is_some() { 0.78 } else { 0.6 }));
        }
    }
    out
}

/// GitLab 公共项目搜索 → 默认分支归档 zip 直链。代码领域。keyless。
pub async fn gitlab(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://gitlab.com/api/v4/projects?search={}&order_by=star_count&sort=desc&per_page={}",
        urlencoding::encode(kw), limit.min(20)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    if let Some(arr) = json.as_array() {
        for r in arr {
            let web = r.get("web_url").and_then(|v| v.as_str()).unwrap_or("");
            let path = r.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let branch = r.get("default_branch").and_then(|v| v.as_str()).unwrap_or("master");
            if web.is_empty() || path.is_empty() { continue; }
            let full = r.get("path_with_namespace").and_then(|v| v.as_str()).unwrap_or(path);
            let dl = format!("{web}/-/archive/{branch}/{path}-{branch}.zip");
            let desc = r.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let title = if desc.is_empty() { full.to_string() } else { format!("{full} — {desc}") };
            out.push(item(&dl, title, ResourceKind::Code, "GitLab", web.to_string(), Some(dl.clone()), Some("zip"), 0.78));
        }
    }
    out
}

// ============ T1 扩展（第二批）：论文/数据集/图片/影音/代码 更多结构化源 ============

/// PubMed Central（E-utilities，keyless）→ 开放获取生物医学全文，直给 PMC PDF。
pub async fn pubmed(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let es = format!(
        "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi?db=pmc&retmode=json&retmax={}&term={}",
        limit.min(25), urlencoding::encode(kw)
    );
    let json = match get_json(&es).await { Some(j) => j, None => return vec![] };
    let ids: Vec<String> = json.get("esearchresult").and_then(|r| r.get("idlist"))
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    if ids.is_empty() { return vec![]; }
    let sum = format!(
        "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi?db=pmc&retmode=json&id={}",
        ids.join(",")
    );
    let sj = match get_json(&sum).await { Some(j) => j, None => return vec![] };
    let result = match sj.get("result") { Some(r) => r, None => return vec![] };
    let mut out = Vec::new();
    for id in &ids {
        let rec = match result.get(id) { Some(r) => r, None => continue };
        let title = rec.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if title.is_empty() { continue; }
        let page = format!("https://www.ncbi.nlm.nih.gov/pmc/articles/PMC{id}/");
        let dl = format!("https://www.ncbi.nlm.nih.gov/pmc/articles/PMC{id}/pdf/");
        let mut it = item(&page, title, ResourceKind::Paper, "PubMed", page.clone(), Some(dl), Some("pdf"), 0.9);
        it.published_at = rec.get("pubdate").and_then(|v| v.as_str()).map(|s| s.to_string());
        out.push(it);
    }
    out
}

/// DBLP（keyless）→ 计算机文献元数据最全，ee 常为 PDF/出版商直链。
pub async fn dblp(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!("https://dblp.org/search/publ/api?q={}&format=json&h={}", urlencoding::encode(kw), limit.min(30));
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let hits = json.get("result").and_then(|r| r.get("hits")).and_then(|h| h.get("hit")).and_then(|v| v.as_array());
    let mut out = Vec::new();
    for h in hits.into_iter().flatten() {
        let info = match h.get("info") { Some(i) => i, None => continue };
        let title = info.get("title").and_then(|v| v.as_str()).unwrap_or("").trim_end_matches('.').to_string();
        if title.is_empty() { continue; }
        let ee = info.get("ee").and_then(|v| v.as_str()).map(|s| s.to_string());
        let page = ee.clone().or_else(|| info.get("url").and_then(|v| v.as_str()).map(|s| s.to_string())).unwrap_or_default();
        if page.is_empty() { continue; }
        let dl = ee.filter(|u| u.ends_with(".pdf"));
        let mut it = item(&page, title, ResourceKind::Paper, "DBLP", page.clone(), dl.clone(), dl.as_ref().map(|_| "pdf"), 0.8);
        it.published_at = info.get("year").and_then(|v| v.as_str()).map(|s| s.to_string());
        out.push(it);
    }
    out
}

/// HAL（法国开放科学，keyless）→ fileMain_s 为 PDF 直链。
pub async fn hal(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://api.archives-ouvertes.fr/search/?q={}&fl=label_s,uri_s,fileMain_s&rows={}&wt=json",
        urlencoding::encode(kw), limit.min(25)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let docs = json.get("response").and_then(|r| r.get("docs")).and_then(|v| v.as_array());
    let mut out = Vec::new();
    for d in docs.into_iter().flatten() {
        let title = d.get("label_s").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if title.is_empty() { continue; }
        let page = d.get("uri_s").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let dl = d.get("fileMain_s").and_then(|v| v.as_str()).map(|s| s.to_string());
        let src = dl.clone().unwrap_or_else(|| page.clone());
        if src.is_empty() { continue; }
        out.push(item(&src, title, ResourceKind::Paper, "HAL", page, dl.clone(), dl.as_ref().map(|_| "pdf"), if dl.is_some() { 0.88 } else { 0.6 }));
    }
    out
}
// APPEND_NEW_SOURCES

/// Zenodo（CERN，keyless）→ 科研数据集/软件/论文，files[].links.self 为文件直链。
pub async fn zenodo(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!("https://zenodo.org/api/records?q={}&size={}", urlencoding::encode(kw), limit.min(25));
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let hits = json.get("hits").and_then(|h| h.get("hits")).and_then(|v| v.as_array());
    let mut out = Vec::new();
    for r in hits.into_iter().flatten() {
        let meta = r.get("metadata");
        let title = meta.and_then(|m| m.get("title")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        if title.is_empty() { continue; }
        let page = r.get("links").and_then(|l| l.get("self_html")).and_then(|v| v.as_str())
            .or_else(|| r.get("links").and_then(|l| l.get("html")).and_then(|v| v.as_str()))
            .unwrap_or("").to_string();
        let file = r.get("files").and_then(|v| v.as_array()).and_then(|a| a.first());
        let dl = file.and_then(|f| f.get("links")).and_then(|l| l.get("self")).and_then(|v| v.as_str()).map(|s| s.to_string());
        let ext = file.and_then(|f| f.get("key")).and_then(|v| v.as_str())
            .and_then(|k| k.rsplit('.').next()).map(|s| s.to_string());
        let src = dl.clone().unwrap_or_else(|| page.clone());
        if src.is_empty() { continue; }
        let mut it = item(&src, title, ResourceKind::Dataset, "Zenodo", page, dl, ext.as_deref(), 0.85);
        it.size_hint = file.and_then(|f| f.get("size")).and_then(|v| v.as_u64());
        out.push(it);
    }
    out
}

/// Hugging Face（keyless）→ 模型(Code)/数据集(Dataset) 仓库发现。
pub async fn huggingface(kw: &str, which: &str, kind: ResourceKind, limit: usize) -> Vec<ResourceItem> {
    let (api, prefix) = if which == "datasets" {
        ("datasets", "datasets/")
    } else {
        ("models", "")
    };
    let url = format!("https://huggingface.co/api/{api}?search={}&limit={}", urlencoding::encode(kw), limit.min(25));
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    for r in json.as_array().into_iter().flatten() {
        let id = r.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if id.is_empty() { continue; }
        let page = format!("https://huggingface.co/{prefix}{id}");
        // 仓库非单文件，给详情页（可 git clone）；下载由前端跳转/后续解析处理。
        out.push(item(&page, id.to_string(), kind, "HuggingFace", page.clone(), None, None, 0.74));
    }
    out
}

/// Harvard Dataverse（keyless）→ 数据集，access API 可整包下载。
pub async fn dataverse(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://dataverse.harvard.edu/api/search?q={}&type=dataset&per_page={}",
        urlencoding::encode(kw), limit.min(25)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let items = json.get("data").and_then(|d| d.get("items")).and_then(|v| v.as_array());
    let mut out = Vec::new();
    for d in items.into_iter().flatten() {
        let title = d.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let gid = d.get("global_id").and_then(|v| v.as_str()).unwrap_or("");
        if title.is_empty() || gid.is_empty() { continue; }
        let page = d.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let dl = format!("https://dataverse.harvard.edu/api/access/dataset/:persistentId/?persistentId={}", urlencoding::encode(gid));
        out.push(item(gid, title, ResourceKind::Dataset, "Dataverse", page, Some(dl), Some("zip"), 0.82));
    }
    out
}

/// Codeberg（Gitea，keyless）→ 仓库默认分支归档 zip 直链。代码领域。
pub async fn codeberg(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!("https://codeberg.org/api/v1/repos/search?q={}&limit={}", urlencoding::encode(kw), limit.min(20));
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    for r in json.get("data").and_then(|v| v.as_array()).into_iter().flatten() {
        let full = r.get("full_name").and_then(|v| v.as_str()).unwrap_or("");
        let html = r.get("html_url").and_then(|v| v.as_str()).unwrap_or("");
        if full.is_empty() || html.is_empty() { continue; }
        let branch = r.get("default_branch").and_then(|v| v.as_str()).unwrap_or("main");
        let dl = format!("{html}/archive/{branch}.zip");
        let desc = r.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let title = if desc.is_empty() { full.to_string() } else { format!("{full} — {desc}") };
        out.push(item(&dl, title, ResourceKind::Code, "Codeberg", html.to_string(), Some(dl.clone()), Some("zip"), 0.8));
    }
    out
}
// APPEND_NEW_SOURCES2

/// 芝加哥艺术博物馆（keyless，CC0）→ IIIF 高清图直链。
pub async fn art_institute(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://api.artic.edu/api/v1/artworks/search?q={}&limit={}&fields=id,title,image_id",
        urlencoding::encode(kw), limit.min(25)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    for d in json.get("data").and_then(|v| v.as_array()).into_iter().flatten() {
        let title = d.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let img = d.get("image_id").and_then(|v| v.as_str()).unwrap_or("");
        if title.is_empty() || img.is_empty() { continue; }
        let id = d.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
        let dl = format!("https://www.artic.edu/iiif/2/{img}/full/843,/0/default.jpg");
        let page = format!("https://www.artic.edu/artworks/{id}");
        let mut it = item(&dl, title, ResourceKind::Image, "ArtInstitute", page, Some(dl.clone()), Some("jpg"), 0.82);
        it.thumbnail = Some(format!("https://www.artic.edu/iiif/2/{img}/full/200,/0/default.jpg"));
        out.push(it);
    }
    out
}

/// 克利夫兰艺术博物馆（keyless，CC0）→ 图片直链。
pub async fn cleveland_art(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://openaccess-api.clevelandart.org/api/artworks/?q={}&limit={}&has_image=1",
        urlencoding::encode(kw), limit.min(25)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    for d in json.get("data").and_then(|v| v.as_array()).into_iter().flatten() {
        let title = d.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if title.is_empty() { continue; }
        let imgs = d.get("images");
        let dl = imgs.and_then(|i| i.get("web")).and_then(|w| w.get("url")).and_then(|v| v.as_str())
            .or_else(|| imgs.and_then(|i| i.get("print")).and_then(|w| w.get("url")).and_then(|v| v.as_str()))
            .map(|s| s.to_string());
        let dl = match dl { Some(d) => d, None => continue };
        let page = d.get("url").and_then(|v| v.as_str()).unwrap_or(&dl).to_string();
        out.push(item(&dl, title, ResourceKind::Image, "ClevelandArt", page, Some(dl.clone()), Some("jpg"), 0.82));
    }
    out
}

/// Unsplash（需 key "unsplash"，默认不启用）→ 高清免费图片。
pub async fn unsplash(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let key = match api_key("unsplash") { Some(k) => k, None => return vec![] };
    let url = format!("https://api.unsplash.com/search/photos?query={}&per_page={}", urlencoding::encode(kw), limit.min(20));
    let json = match get_json_hdr(&url, "Authorization", &format!("Client-ID {key}")).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    for r in json.get("results").and_then(|v| v.as_array()).into_iter().flatten() {
        let dl = r.get("urls").and_then(|u| u.get("full")).and_then(|v| v.as_str()).map(|s| s.to_string());
        let dl = match dl { Some(d) => d, None => continue };
        let page = r.get("links").and_then(|l| l.get("html")).and_then(|v| v.as_str()).unwrap_or(&dl).to_string();
        let title = r.get("alt_description").and_then(|v| v.as_str()).filter(|s| !s.is_empty())
            .map(|s| s.to_string()).unwrap_or_else(|| format!("{kw} 图片"));
        let mut it = item(&dl, title, ResourceKind::Image, "Unsplash", page, Some(dl.clone()), Some("jpg"), 0.8);
        it.thumbnail = r.get("urls").and_then(|u| u.get("small")).and_then(|v| v.as_str()).map(|s| s.to_string());
        it.author = r.get("user").and_then(|u| u.get("name")).and_then(|v| v.as_str()).map(|s| s.to_string());
        out.push(it);
    }
    out
}

/// Pixabay（需 key "pixabay"，默认不启用）→ 免费图片/视频直链。
pub async fn pixabay(kw: &str, media: &str, limit: usize) -> Vec<ResourceItem> {
    let key = match api_key("pixabay") { Some(k) => k, None => return vec![] };
    let is_video = media == "video";
    let url = if is_video {
        format!("https://pixabay.com/api/videos/?key={key}&q={}&per_page={}", urlencoding::encode(kw), limit.min(20).max(3))
    } else {
        format!("https://pixabay.com/api/?key={key}&q={}&per_page={}", urlencoding::encode(kw), limit.min(20).max(3))
    };
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    for r in json.get("hits").and_then(|v| v.as_array()).into_iter().flatten() {
        let page = r.get("pageURL").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if is_video {
            let dl = r.get("videos").and_then(|v| v.get("large")).and_then(|l| l.get("url")).and_then(|v| v.as_str()).map(|s| s.to_string());
            let dl = match dl { Some(d) => d, None => continue };
            let title = r.get("tags").and_then(|v| v.as_str()).unwrap_or(kw).to_string();
            out.push(item(&dl, title, ResourceKind::Video, "Pixabay", page.clone(), Some(dl.clone()), Some("mp4"), 0.78));
        } else {
            let dl = r.get("largeImageURL").and_then(|v| v.as_str()).map(|s| s.to_string());
            let dl = match dl { Some(d) => d, None => continue };
            let title = r.get("tags").and_then(|v| v.as_str()).unwrap_or(kw).to_string();
            let mut it = item(&dl, title, ResourceKind::Image, "Pixabay", page.clone(), Some(dl.clone()), Some("jpg"), 0.78);
            it.thumbnail = r.get("previewURL").and_then(|v| v.as_str()).map(|s| s.to_string());
            out.push(it);
        }
    }
    out
}

/// ccMixter（keyless）→ 知识共享授权音乐直链。
pub async fn ccmixter(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!("https://ccmixter.org/api/query?f=json&search={}&limit={}", urlencoding::encode(kw), limit.min(25));
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    for r in json.as_array().into_iter().flatten() {
        let title = r.get("upload_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if title.is_empty() { continue; }
        let page = r.get("file_page_url").and_then(|v| v.as_str())
            .or_else(|| r.get("artist_page_url").and_then(|v| v.as_str())).unwrap_or("").to_string();
        let file = r.get("files").and_then(|v| v.as_array()).and_then(|a| a.first());
        let dl = file.and_then(|f| f.get("download_url")).and_then(|v| v.as_str()).map(|s| s.to_string());
        let dl = match dl { Some(d) => d, None => continue };
        let ext = dl.rsplit('.').next().filter(|e| e.len() <= 4).map(|s| s.to_string());
        let mut it = item(&dl, title, ResourceKind::Audio, "ccMixter", page, Some(dl.clone()), ext.as_deref(), 0.78);
        it.author = r.get("user_name").and_then(|v| v.as_str()).map(|s| s.to_string());
        out.push(it);
    }
    out
}

/// PeerTube Sepia Search（keyless，联邦）→ 去中心化视频，交 yt-dlp 下载。
pub async fn peertube(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!("https://sepiasearch.org/api/v1/search/videos?search={}&count={}", urlencoding::encode(kw), limit.min(20));
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    for r in json.get("data").and_then(|v| v.as_array()).into_iter().flatten() {
        let title = r.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let watch = r.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if title.is_empty() || watch.is_empty() { continue; }
        let mut it = item(&watch, title, ResourceKind::Video, "PeerTube", watch.clone(), None, None, 0.72);
        it.needs_ytdlp = true;
        it.thumbnail = r.get("thumbnailUrl").and_then(|v| v.as_str()).map(|s| s.to_string());
        out.push(it);
    }
    out
}

/// Dailymotion（keyless 搜索）→ 视频，交 yt-dlp 下载。国际主流平台补充。
pub async fn dailymotion(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://api.dailymotion.com/videos?search={}&fields=id,title,url,thumbnail_360_url&limit={}",
        urlencoding::encode(kw), limit.min(20)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    for r in json.get("list").and_then(|v| v.as_array()).into_iter().flatten() {
        let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let watch = r.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if title.is_empty() || watch.is_empty() { continue; }
        let mut it = item(&watch, title, ResourceKind::Video, "Dailymotion", watch.clone(), None, None, 0.72);
        it.needs_ytdlp = true;
        it.thumbnail = r.get("thumbnail_360_url").and_then(|v| v.as_str()).map(|s| s.to_string());
        out.push(it);
    }
    out
}

/// 史密森尼开放获取（需 key "smithsonian"，api.data.gov，默认不启用）→ CC0 文物/高清图。
pub async fn smithsonian(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let key = match api_key("smithsonian") { Some(k) => k, None => return vec![] };
    let url = format!(
        "https://api.si.edu/openaccess/api/v1.0/search?q={}&rows={}&api_key={}",
        urlencoding::encode(kw), limit.min(25), urlencoding::encode(&key)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let rows = json.get("response").and_then(|r| r.get("rows")).and_then(|v| v.as_array());
    let mut out = Vec::new();
    for r in rows.into_iter().flatten() {
        let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if title.is_empty() { continue; }
        let dnr = r.get("content").and_then(|c| c.get("descriptiveNonRepeating"));
        let media = dnr.and_then(|d| d.get("online_media")).and_then(|m| m.get("media")).and_then(|v| v.as_array());
        let first = match media.and_then(|a| a.first()) { Some(m) => m, None => continue };
        let dl = match first.get("content").and_then(|v| v.as_str()) {
            Some(d) if d.starts_with("http") => d.to_string(), _ => continue,
        };
        let page = dnr.and_then(|d| d.get("record_link")).and_then(|v| v.as_str()).unwrap_or(&dl).to_string();
        let mut it = item(&dl, title, ResourceKind::Image, "Smithsonian", page, Some(dl.clone()), Some("jpg"), 0.82);
        it.thumbnail = first.get("thumbnail").and_then(|v| v.as_str()).map(|s| s.to_string());
        out.push(it);
    }
    out
}

/// Europeana（需 key "europeana"，默认不启用）→ 欧洲文化遗产海量图片/媒体直链。
pub async fn europeana(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let key = match api_key("europeana") { Some(k) => k, None => return vec![] };
    let url = format!(
        "https://api.europeana.eu/record/v2/search.json?wskey={}&query={}&rows={}&media=true&thumbnail=true",
        urlencoding::encode(&key), urlencoding::encode(kw), limit.min(25)
    );
    let json = match get_json(&url).await { Some(j) => j, None => return vec![] };
    let mut out = Vec::new();
    for r in json.get("items").and_then(|v| v.as_array()).into_iter().flatten() {
        let title = r.get("title").and_then(|v| v.as_array()).and_then(|a| a.first())
            .and_then(|v| v.as_str()).unwrap_or("").to_string();
        if title.is_empty() { continue; }
        let dl = r.get("edmIsShownBy").and_then(|v| v.as_array()).and_then(|a| a.first())
            .and_then(|v| v.as_str()).map(|s| s.to_string());
        let dl = match dl { Some(d) => d, None => continue };
        let page = r.get("guid").and_then(|v| v.as_str()).unwrap_or(&dl).to_string();
        let mut it = item(&dl, title, ResourceKind::Image, "Europeana", page, Some(dl.clone()), None, 0.8);
        it.thumbnail = r.get("edmPreview").and_then(|v| v.as_array()).and_then(|a| a.first())
            .and_then(|v| v.as_str()).map(|s| s.to_string());
        out.push(it);
    }
    out
}

