//! 中文书籍源：补上通用搜索引擎与英文向图书 API 覆盖不到的两类资源。
//! - 苦瓜书盘（kgbook.com）：中文 EPUB/AZW3/MOBI 下载站，两步跳转、页面 GBK。
//! - Standard Ebooks：精校公版书，详情页直接给出 epub/azw3 直链。
//! - GitHub 书籍仓库：仓库里存的 PDF/EPUB 往往只出现在文件树或 README 链接里，
//!   整仓 zip 的旧做法既慢又不是用户想要的那本书。
//! 网盘分享见 `pan_sources`。
use super::super::model::{ResourceItem, ResourceKind};
use super::super::proxy;
use super::make_id;
use regex::Regex;
use scraper::{Html, Selector};

fn client() -> Option<reqwest::Client> {
    proxy::client_for_source(&super::load_proxy(), true).ok()
}

/// 境内站（苦瓜书盘、夸克）直连；境外站走代理。
fn client_cn() -> Option<reqwest::Client> {
    proxy::client_for_source(&super::load_proxy(), false).ok()
}

/// 站点的 GBK 页面按 UTF-8 解出来会是乱码，扫文本前先解到正确编码。
/// 苦瓜书盘的响应头常写 `text/html` 甚至 `charset=utf-8`，与实际 GBK 内容不符，
/// 所以不能只信 Content-Type：先试严格 UTF-8，失败再按 GB18030（GBK 超集）解。
fn decode_body(bytes: &[u8], ct: &str) -> String {
    let lc = ct.to_ascii_lowercase();
    if lc.contains("gb2312") || lc.contains("gbk") || lc.contains("gb18030") {
        let (cow, _, _) = encoding_rs::GBK.decode(bytes);
        return cow.into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (cow, _, _) = encoding_rs::GBK.decode(bytes);
            cow.into_owned()
        }
    }
}

async fn get_text(client: &reqwest::Client, url: &str, referer: Option<&str>) -> Option<(String, String)> {
    let mut req = client.get(url);
    if let Some(r) = referer {
        req = req.header(reqwest::header::REFERER, r);
    }
    let resp = req.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = resp.bytes().await.ok()?;
    Some((decode_body(&bytes, &ct), ct))
}

/// 苦瓜书盘的搜索记录按服务端时间窗缓存，**并发**请求会互相覆盖（拿到彼此的结果）。
/// 同一进程内串行化并留出间隔，保证每次读到的 searchid 都属于自己那次查询。
static KGBOOK_GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
const KGBOOK_GAP: std::time::Duration = std::time::Duration::from_millis(900);

/// 苦瓜书盘：POST 搜索表单 → 302 到 `e/search/result/?searchid=...` → 解析结果列表。
/// 搜索结果只是详情页入口，真正的 epub/azw3/mobi 直链在详情页上，故先返回详情页条目，
/// 由 `resolve_items` 的专用解析层补齐直链。
///
/// 两个坑：
/// 1. 搜索记录按「服务端时间窗」缓存，**并发**请求会互相覆盖（拿到彼此的结果），故串行化；
/// 2. 关键词必须是 UTF-8，编成 GBK 服务端认不出，会回落到默认书目列表（1281 条无关书）。
pub async fn kgbook(kw: &str, limit: usize) -> Vec<ResourceItem> {
    // 整个「搜索 + 读结果页」必须独占，否则并发查询会互相覆盖服务端搜索记录。
    let _gate = KGBOOK_GATE.lock().await;
    let out = kgbook_locked(kw, limit).await;
    tokio::time::sleep(KGBOOK_GAP).await;
    out
}

async fn kgbook_locked(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let client = match client_cn() {
        Some(c) => c,
        None => return vec![],
    };
    // 表单字段全部走 UTF-8：站点虽然输出 GBK 页面，但搜索表单吃的是 UTF-8。
    // 把关键词编成 GBK 服务端认不出，会回落到默认书目列表（searchid 6600，1281 条），
    // 看起来"有结果"，其实全是无关书——所以这里只做标准 urlencode。
    let mut body = String::new();
    for (i, (k, v)) in [
        ("keyboard", kw),
        ("show", "title,booksay,bookwriter"),
        ("tbname", "download"),
        ("tempid", "1"),
        ("submit", "搜索"),
    ]
    .iter()
    .enumerate()
    {
        if i > 0 {
            body.push('&');
        }
        body.push_str(k);
        body.push('=');
        body.push_str(&urlencoding::encode(v));
    }
    // 自己跟随 302：需要拿到 searchid 才能校验结果归属。
    // 用直连通道（站点在境内）。
    let raw = match proxy::build_client_raw_no_redirect(None) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    for attempt in 0..3 {
        let resp = match raw
            .post("https://kgbook.com/e/search/index.php")
            .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(reqwest::header::REFERER, "https://kgbook.com/")
            .body(body.clone())
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => return vec![],
        };
        // 非 302：站内提示页（"没有搜索到相关的内容"），本次查询确实无结果。
        if resp.status() != reqwest::StatusCode::FOUND {
            return vec![];
        }
        let loc = match resp
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok())
        {
            Some(v) => v.to_string(),
            None => return vec![],
        };
        let result_url = match url::Url::parse("https://kgbook.com/e/search/")
            .ok()
            .and_then(|b| b.join(&loc).ok())
        {
            Some(u) => u.to_string(),
            None => return vec![],
        };
        let (html, _) = match get_text(&client, &result_url, Some("https://kgbook.com/")).await {
            Some(v) => v,
            None => return vec![],
        };
        // 结果页标题回显关键词：对不上就是读到了缓存窗口里别人的结果。
        let title = Regex::new(r"<title>([^<]*)</title>")
            .ok()
            .and_then(|re| re.captures(&html).map(|c| c[1].trim().to_string()))
            .unwrap_or_default();
        let title = html_unescape(&title);
        if !title.is_empty() && !title.contains(kw.trim()) {
            if attempt < 2 {
                tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
                continue;
            }
            return vec![];
        }
        return parse_kgbook(&html, &result_url, limit);
    }
    vec![]
}

/// 解析苦瓜书盘结果页：`<h1><a href=".../分类/ID.html">书名</a></h1>`。
/// 结果里混着站内导航链接（"苦瓜书盘"），按 URL 形态与标题过滤掉。
fn parse_kgbook(html: &str, base: &str, limit: usize) -> Vec<ResourceItem> {
    let doc = Html::parse_document(html);
    let sel = match Selector::parse("h1 > a[href]") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let page_re = Regex::new(r"^https?://kgbook\.com/[a-z]+/\d+\.html$").unwrap();
    let base_url = url::Url::parse(base).ok();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for a in doc.select(&sel) {
        let href = a.value().attr("href").unwrap_or("");
        let abs = if href.starts_with("http") {
            href.to_string()
        } else {
            match &base_url {
                Some(b) => b.join(href).map(|u| u.to_string()).unwrap_or_default(),
                None => String::new(),
            }
        };
        if !page_re.is_match(&abs) || !seen.insert(abs.clone()) {
            continue;
        }
        let title = a.text().collect::<String>().trim().to_string();
        if title.len() < 2 {
            continue;
        }
        let mut it = ResourceItem {
            id: make_id(&abs),
            title,
            kind: ResourceKind::Book,
            source: "苦瓜书盘".into(),
            source_url: abs,
            download_url: None,
            magnet: None,
            needs_ytdlp: false,
            needs_pan: false,
            format: None,
            thumbnail: None,
            author: None,
            published_at: None,
            summary: None,
            size_hint: None,
            relevance: 0.8,
            tags: vec![],
        };
        it.tags.push("中文".into());
        out.push(it);
        if out.len() >= limit {
            break;
        }
    }
    out
}

/// 解析苦瓜书盘详情页：`作者/格式/语言/大小` 四个 `<li>` + `GetDown?...pathid=N` 下载按钮。
/// 按钮 URL 有两种写法（`GetDown?...` 与 `GetDown/?...`），文案也不一定写格式，
/// 所以格式以 meta 里的「格式」字段为准，按钮只用来取下载地址。
pub async fn kgbook_detail(page_url: &str) -> Option<ResourceItem> {
    let client = client_cn()?;
    let (html, _) = get_text(&client, page_url, Some("https://kgbook.com/")).await?;
    let doc = Html::parse_document(&html);
    let title_sel = Selector::parse("h1.news_title").ok()?;
    let title = doc
        .select(&title_sel)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())?;
    let meta_re = Regex::new(r"<li>\s*(作者|格式|语言|大小)：\s*([^<]*)</li>").unwrap();
    let mut author = None;
    let mut fmt = None;
    let mut size = None;
    for c in meta_re.captures_iter(&html) {
        let v = c[2].trim().to_string();
        if v.is_empty() {
            continue;
        }
        match &c[1] {
            "作者" => author = Some(v),
            "格式" => fmt = Some(v.to_ascii_lowercase()),
            "大小" => size = Some(parse_size_hint(&v)),
            _ => {}
        }
    }
    // 按钮可能是 azw3/mobi/epub 三个，也可能只有一个「下载地址1」。
    let btn_re = Regex::new(r#"href="(https://kgbook\.com/e/DownSys/GetDown/?\?classid=[^"]*pathid=\d+)"[^>]*>([^<]*)</a>"#)
        .unwrap();
    let mut best: Option<(u8, String, String)> = None;
    for c in btn_re.captures_iter(&html) {
        let url = c[1].replace("&amp;", "&");
        let label = c[2].trim().to_string().to_ascii_lowercase();
        // 按格式优先级挑：epub > azw3/mobi > 其它（文案没写格式时排最后，仅作兜底）。
        let rank = if label.contains("epub") {
            0
        } else if label.contains("azw3") || label.contains("mobi") {
            1
        } else {
            2
        };
        if best.as_ref().map(|b| rank < b.0).unwrap_or(true) {
            best = Some((rank, url, label));
        }
    }
    let (_, dl, label) = best?;
    // 按钮文案带格式时以它为准（一个详情页可能只有某个格式的按钮），否则用 meta 的格式。
    let ext = ["epub", "azw3", "mobi", "pdf", "txt", "djvu"]
        .iter()
        .find(|e| label.contains(*e))
        .map(|e| e.to_string())
        .or(fmt);
    let mut it = ResourceItem {
        id: make_id(page_url),
        title,
        kind: ResourceKind::Book,
        source: "苦瓜书盘".into(),
        source_url: page_url.to_string(),
        download_url: Some(dl),
        magnet: None,
        needs_ytdlp: false,
        needs_pan: false,
        format: ext,
        thumbnail: None,
        author,
        published_at: None,
        summary: None,
        size_hint: size,
        relevance: 0.85,
        tags: vec!["中文".into()],
    };
    it.tags.dedup();
    Some(it)
}

/// "839.45 KB" / "48 MB" → 字节数（详情页只给人类可读的大小）。
fn parse_size_hint(s: &str) -> u64 {
    let re = Regex::new(r"(?i)([\d.]+)\s*(b|kb|mb|gb)").unwrap();
    let c = match re.captures(s) {
        Some(c) => c,
        None => return 0,
    };
    let n: f64 = c[1].parse().unwrap_or(0.0);
    let mul: f64 = match c[2].to_ascii_lowercase().as_str() {
        "gb" => 1024.0 * 1024.0 * 1024.0,
        "mb" => 1024.0 * 1024.0,
        "kb" => 1024.0,
        _ => 1.0,
    };
    (n * mul) as u64
}

/// Standard Ebooks：`?query=` 搜索页 → 每条书目详情页 → 详情页的 `.epub` 下载链接。
/// 公版书无版权风险且是精校版，比通用搜索捞到的 PDF 质量稳定得多。
pub async fn standard_ebooks(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let client = match client() {
        Some(c) => c,
        None => return vec![],
    };
    let url = format!("https://standardebooks.org/ebooks?query={}", urlencoding::encode(kw));
    let (html, _) = match get_text(&client, &url, None).await {
        Some(v) => v,
        None => return vec![],
    };
    // 列表项：`<li typeof="schema:Book" about="/ebooks/作者/书名[/译者]">`，
    // 每个 li 里有两个 schema:name span：先书名、后作者。
    let item_re = Regex::new(r#"(?s)<li typeof="schema:Book" about="(/ebooks/[^"]+)">(.*?)</li>"#).unwrap();
    let name_re = Regex::new(r#"(?s)<span property="schema:name">(.*?)</span>"#).unwrap();
    let mut out = Vec::new();
    for c in item_re.captures_iter(&html) {
        let path = &c[1];
        let block = &c[2];
        let mut names = name_re
            .captures_iter(block)
            .map(|m| m[1].trim().to_string())
            .filter(|s| !s.is_empty());
        let title = names.next().unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        let author = names.next();
        let page = format!("https://standardebooks.org{path}");
        out.push(ResourceItem {
            id: make_id(&page),
            title,
            kind: ResourceKind::Book,
            source: "StandardEbooks".into(),
            source_url: page,
            download_url: None,
            magnet: None,
            needs_ytdlp: false,
            needs_pan: false,
            format: Some("epub".into()),
            thumbnail: None,
            author,
            published_at: None,
            summary: None,
            size_hint: None,
            relevance: 0.84,
            tags: vec![],
        });
        if out.len() >= limit {
            break;
        }
    }
    out
}

/// 解析 Standard Ebooks 详情页里 `.epub` 直链（跳过 kepub/advanced 变体）。
pub async fn standard_ebooks_detail(page_url: &str) -> Option<String> {
    let client = client()?;
    let (html, _) = get_text(&client, page_url, None).await?;
    let re = Regex::new(r#"href="([^"]+\.epub)""#).unwrap();
    let mut best: Option<String> = None;
    for c in re.captures_iter(&html) {
        let href = &c[1];
        if href.contains("kepub") || href.contains("advanced") || href.contains("_advanced") {
            continue;
        }
        if !href.contains("/downloads/") {
            continue;
        }
        let abs = if href.starts_with("http") {
            href.to_string()
        } else {
            format!("https://standardebooks.org{href}")
        };
        best = Some(abs);
        break;
    }
    best
}

/// GitHub 书籍仓库：仓库里的 PDF/EPUB 只能靠文件树或 README 链接找出来。
/// 先用 HTML 版搜索页（免 API 限流）定位仓库，再读 README 抽直链；
/// README 里没有时，退到 `git/trees?recursive=1` 枚举文件树。
pub async fn github_books(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let client = match client() {
        Some(c) => c,
        None => return vec![],
    };
    let repos = github_repo_search(&client, kw, 6).await;
    if repos.is_empty() {
        return vec![];
    }
    let mut futs = Vec::new();
    for full in repos {
        let c = client.clone();
        futs.push(async move {
            let branch = github_default_branch(&c, &full).await;
            github_repo_book_files(&c, &full, &branch, limit).await
        });
    }
    let mut out = Vec::new();
    for group in futures_util::future::join_all(futs).await {
        out.extend(group);
    }
    out.truncate(limit.saturating_mul(2));
    out
}

/// 仓库搜索：优先 HTML 搜索页（不耗 API 配额），失败再退 API。
/// 返回仓库全名列表；分支名由调用方逐个查（见 `github_default_branch`）。
async fn github_repo_search(client: &reqwest::Client, kw: &str, want: usize) -> Vec<String> {
    let url = format!(
        "https://github.com/search?q={}&type=repositories",
        urlencoding::encode(&format!("{kw} book"))
    );
    let mut repos: Vec<String> = Vec::new();
    if let Some((html, _)) = get_text(client, &url, None).await {
        // 结果数据内嵌在 react-app.embeddedData 的 blackbirdSearchRoute.results 里：
        // 每条有 hl_name（owner/repo）与 repo.repository。用正则扫比解析整段 JSON 省内存，
        // 页面结构没变时这是最稳的取法。
        let name_re = Regex::new(r#""hl_name":"((?:[^"\\]|\\.)*)""#).unwrap();
        let mut seen = std::collections::HashSet::new();
        for c in name_re.captures_iter(&html) {
            // 内嵌 JSON 里高亮标签写作 <em>，先还原成 <em> 再去掉标签。
            let full = json_unescape(&c[1]);
            let full: String = Regex::new(r"<[^>]*>").unwrap().replace_all(&full, "").to_string();
            let full = full.trim().to_string();
            if full.is_empty() || !full.contains('/') || !seen.insert(full.clone()) {
                continue;
            }
            repos.push(full);
            if repos.len() >= want {
                break;
            }
        }
    }
    if repos.is_empty() {
        // HTML 搜索页被反爬时退回 API（未认证 10 次/分钟，够一次查询用）。
        let api = format!(
            "https://api.github.com/search/repositories?q={}&sort=stars&per_page={}",
            urlencoding::encode(&format!("{kw} book")),
            want
        );
        if let Some((body, _)) = get_text(client, &api, None).await {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                for r in json.get("items").and_then(|v| v.as_array()).into_iter().flatten() {
                    let full = r.get("full_name").and_then(|v| v.as_str()).unwrap_or("");
                    if !full.is_empty() {
                        repos.push(full.to_string());
                    }
                }
            }
        }
    }
    repos
}

/// 仓库真实默认分支：搜索结果页里只给了仓库名，分支名要单独问；
/// 写死 main 会让一半老仓库（用 master 的）直接 404。
///
/// 优先读仓库 HTML 里内嵌的 `defaultBranch`（不耗 GitHub API 配额）；
/// API 未认证只有 60 次/小时，是这类批量查询最先打满的资源。
async fn github_default_branch(client: &reqwest::Client, full: &str) -> String {
    let page = format!("https://github.com/{full}");
    if let Some((html, _)) = get_text(client, &page, None).await {
        if let Some(c) = Regex::new(r#""defaultBranch":"([^"]+)""#).unwrap().captures(&html) {
            return c[1].to_string();
        }
    }
    let api = format!("https://api.github.com/repos/{full}");
    if let Some((body, _)) = get_text(client, &api, None).await {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(br) = json.get("default_branch").and_then(|v| v.as_str()) {
                if !br.is_empty() {
                    return br.to_string();
                }
            }
        }
    }
    "main".to_string()
}

/// 从一个仓库里捞出可下载的书籍文件：先 README 链接，再文件树里的 pdf/epub。
async fn github_repo_book_files(
    client: &reqwest::Client,
    full: &str,
    branch: &str,
    limit: usize,
) -> Vec<ResourceItem> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let exts = ["pdf", "epub", "mobi", "azw3", "djvu"];
    let push = |title: String, url: String, fmt: &str, out: &mut Vec<ResourceItem>, seen: &mut std::collections::HashSet<String>| {
        if !seen.insert(url.clone()) {
            return;
        }
        out.push(ResourceItem {
            id: make_id(&url),
            title,
            kind: ResourceKind::Book,
            source: "GitHub".into(),
            source_url: format!("https://github.com/{full}"),
            download_url: Some(url),
            magnet: None,
            needs_ytdlp: false,
            needs_pan: false,
            format: Some(fmt.to_string()),
            thumbnail: None,
            author: None,
            published_at: None,
            summary: None,
            size_hint: None,
            relevance: 0.8,
            tags: vec!["开源".into()],
        });
    };

    // 1) README：书籍仓库通常把下载链接写在说明里（相对路径也常见）。
    //    收录三种写法：markdown 链接 `[书名](x.pdf)`、带徽章图片的链接
    //    `[![Download](badge.svg)](x.pdf)`、以及裸 URL（很多书仓直接贴链接）。
    let readme_re = Regex::new(r"(?i)\[([^\]\n]{1,160})\]\(([^)\s]+\.(pdf|epub|mobi|azw3|djvu))\)").unwrap();
    let bare_re = Regex::new(r"(?i)(https?://[^\s<>()\[\]]+\.(pdf|epub|mobi|azw3|djvu))").unwrap();
    for name in ["README.md", "readme.md", "README.MD"] {
        let raw = format!("https://raw.githubusercontent.com/{full}/{branch}/{name}");
        let (body, _) = match get_text(client, &raw, None).await {
            Some(v) => v,
            None => continue,
        };
        for c in readme_re.captures_iter(&body) {
            let label = c[1].trim();
            let href = c[2].trim();
            let ext = c[3].to_ascii_lowercase();
            let url = match normalize_raw_url(href, full, branch) {
                Some(u) => u,
                None => continue,
            };
            // 徽章链接的锚文本是图片（`![Download](badge.svg)`），不能当书名。
            let title = if label.starts_with("![") || label.starts_with("img ") {
                title_from_url(href)
            } else {
                strip_md_marks(label)
            };
            if title.is_empty() {
                continue;
            }
            push(title, url, &ext, &mut out, &mut seen);
            if out.len() >= limit {
                return out;
            }
        }
        // 裸 URL：标题从文件名还原（去扩展名、把 - 和 _ 换成空格）。
        for c in bare_re.captures_iter(&body) {
            let href = c[1].trim();
            let ext = c[2].to_ascii_lowercase();
            let url = match normalize_raw_url(href, full, branch) {
                Some(u) => u,
                None => continue,
            };
            let file = href.rsplit('/').next().unwrap_or(href);
            let title = title_from_url(file);
            if title.is_empty() {
                continue;
            }
            push(title, url, &ext, &mut out, &mut seen);
            if out.len() >= limit {
                return out;
            }
        }
        break;
    }

    // 2) 文件树：README 没列全时，再枚举仓库里的 pdf/epub。
    //    先走 git trees API（一次请求拿全仓）；配额打满时退回文件树网页，
    //    网页里内嵌的 items 同样带路径，够覆盖「书就在仓库里但 README 没写」的情况。
    if out.len() < limit {
        let mut paths: Vec<String> = Vec::new();
        let api = format!("https://api.github.com/repos/{full}/git/trees/{branch}?recursive=1");
        if let Some((body, _)) = get_text(client, &api, None).await {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                for t in json.get("tree").and_then(|v| v.as_array()).into_iter().flatten() {
                    if t.get("type").and_then(|v| v.as_str()) != Some("blob") {
                        continue;
                    }
                    if let Some(p) = t.get("path").and_then(|v| v.as_str()) {
                        paths.push(p.to_string());
                    }
                }
            }
        }
        if paths.is_empty() {
            let page = format!("https://github.com/{full}/tree/{branch}");
            if let Some((html, _)) = get_text(client, &page, None).await {
                let rel = Regex::new(r#"(?i)"path":"([^"]+\.(?:pdf|epub|mobi|azw3|djvu))""#).unwrap();
                for c in rel.captures_iter(&html) {
                    paths.push(c[1].to_string());
                }
            }
        }
        for path in paths {
            let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
            // 仓库里常见整站导出的 HTML 副本，只收书籍格式。
            if !exts.contains(&ext.as_str()) {
                continue;
            }
            let url = format!(
                "https://raw.githubusercontent.com/{full}/{branch}/{}",
                path.split('/').map(|s| urlencoding::encode(s)).collect::<Vec<_>>().join("/")
            );
            let title = path.rsplit('/').next().unwrap_or(&path).to_string();
            push(title, url, &ext, &mut out, &mut seen);
            if out.len() >= limit {
                break;
            }
        }
    }
    out
}

/// 把 README 里的各种相对/绝对写法归一成可直接下载的 raw 地址。
fn normalize_raw_url(href: &str, full: &str, branch: &str) -> Option<String> {
    if href.starts_with("http") {
        // github.com/o/r/blob/br/path → raw.githubusercontent.com/o/r/br/path
        if let Some(rest) = href.strip_prefix("https://github.com/") {
            let parts: Vec<&str> = rest.split('/').collect();
            if parts.len() > 4 && parts[2] == "blob" {
                return Some(format!(
                    "https://raw.githubusercontent.com/{}/{}/{}/{}",
                    parts[0],
                    parts[1],
                    parts[3],
                    parts[4..].join("/")
                ));
            }
        }
        return Some(href.to_string());
    }
    let rel = href.trim_start_matches("./").trim_start_matches('/');
    if rel.is_empty() {
        return None;
    }
    let encoded = rel
        .split('/')
        .map(|s| urlencoding::encode(s))
        .collect::<Vec<_>>()
        .join("/");
    Some(format!("https://raw.githubusercontent.com/{full}/{branch}/{encoded}"))
}

/// `**书名**`、`` `代码` `` 这类 markdown 装饰从标题里去掉。
fn strip_md_marks(s: &str) -> String {
    s.replace(['*', '`'], "").trim().to_string()
}

/// 从文件 URL 还原书名：取末段、去扩展名、解开 %20、把 - 和 _ 换成空格。
/// 返回空串表示「不像一本可下载的书」，调用方应丢弃。
fn title_from_url(href: &str) -> String {
    let seg = href.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    // `www.epub` 这类域名尾巴不是文件：末段没有真正的书名部分。
    if seg.starts_with("www.") {
        return String::new();
    }
    let seg = urlencoding::decode(seg).map(|c| c.into_owned()).unwrap_or_else(|_| seg.to_string());
    let stem = seg.rsplit_once('.').map(|(s, _)| s).unwrap_or(&seg);
    let t = stem.replace(['-', '_'], " ").trim().to_string();
    // 太长的不像书名（构建产物名等），直接丢掉。
    if t.len() > 160 {
        String::new()
    } else {
        t
    }
}

/// 反转义 JSON/HTML 片段里常见的实体（内嵌 JSON 里的短名一般只有这几种）。
fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// 内嵌 JSON 的 `<` 之类转义还原（GitHub 把 `<em>` 这么写）。
fn json_unescape(s: &str) -> String {
    let re = Regex::new(r"\\u([0-9a-fA-F]{4})").unwrap();
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    for c in re.captures_iter(s) {
        out.push_str(&s[last..c.get(0).map(|m| m.start()).unwrap_or(0)]);
        let code = u32::from_str_radix(&c[1], 16).unwrap_or(0);
        if let Some(ch) = char::from_u32(code) {
            out.push(ch);
        }
        last = c.get(0).map(|m| m.end()).unwrap_or(0);
    }
    out.push_str(&s[last..]);
    out.replace("\\\"", "\"").replace("\\\\", "\\")
}
// APPEND_BOOK_SOURCES

#[cfg(test)]
mod tests {
    use super::*;

    /// 苦瓜书盘结果页的真实片段（GBK 站点解码后的形态）：导航链接必须被过滤掉。
    #[test]
    fn kgbook_parses_result_rows_only() {
        let html = r#"
            <h1><a href="https://kgbook.com">苦瓜书盘</a></h1>
            <h1><a href="https://kgbook.com/renwensheke/1027.html" >论语译注（杨伯峻译注）</a></h1>
            <h1><a href="https://kgbook.com/gudianwenxue/848.html" >论语集注</a></h1>
        "#;
        let got = parse_kgbook(html, "https://kgbook.com/e/search/result/?searchid=1", 10);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].title, "论语译注（杨伯峻译注）");
        assert_eq!(got[0].source, "苦瓜书盘");
        // 搜索结果只有详情页，直链由解析层补。
        assert!(got[0].download_url.is_none());
    }

    /// "839.45 KB" 这类人类可读大小要换算成字节。
    #[test]
    fn size_hint_parses_units() {
        // 839.45 KiB = 859596.8 B，向下取整。
        assert_eq!(parse_size_hint("839.45 KB"), 859_596);
        assert_eq!(parse_size_hint("48 MB"), 50_331_648);
        assert_eq!(parse_size_hint("未知"), 0);
    }

    /// README 里的三种写法都要归一成可下载的 raw 地址。
    #[test]
    fn raw_url_normalizes_all_forms() {
        assert_eq!(
            normalize_raw_url("./PDF-file/Java.pdf", "o/r", "main").unwrap(),
            "https://raw.githubusercontent.com/o/r/main/PDF-file/Java.pdf"
        );
        assert_eq!(
            normalize_raw_url(
                "https://github.com/o/r/blob/master/complete-book-pdf/book.pdf",
                "o/r",
                "master"
            )
            .unwrap(),
            "https://raw.githubusercontent.com/o/r/master/complete-book-pdf/book.pdf"
        );
        // 已经是 raw 的绝对地址原样保留。
        assert_eq!(
            normalize_raw_url("https://example.com/a.epub", "o/r", "main").unwrap(),
            "https://example.com/a.epub"
        );
    }
}
