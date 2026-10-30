//! 通用搜索引擎适配器：抓结果页拿候选页面 URL，供嗅探器进一步扒文件链接。
//! 纯 reqwest + scraper。境内引擎(百度/搜狗/360/必应CN)直连，境外引擎(Bing/DDG/Google/Brave)
//! 走代理(系统或用户配置)。每个引擎多套 selector 兜底，尽量抗改版。
use super::super::model::ProxyConfig;
use super::super::proxy;
use scraper::{Html, Selector};
use url::Url;

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    /// 结果页给出的摘要（部分引擎有）。对话侧直接把它当联网上下文，
    /// 没有摘要时才需要进一步抓正文。
    pub snippet: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EngineMeta {
    pub id: &'static str,
    pub label: &'static str,
    /// 境外引擎：大陆需代理才可达。
    pub foreign: bool,
    /// 是否默认启用。
    pub default_on: bool,
}

/// 引擎注册表。前端据此渲染开关与「境外/代理」标记。
pub fn engine_metas() -> Vec<EngineMeta> {
    vec![
        EngineMeta { id: "baidu", label: "百度", foreign: false, default_on: true },
        EngineMeta { id: "sogou", label: "搜狗", foreign: false, default_on: true },
        EngineMeta { id: "so360", label: "360", foreign: false, default_on: false },
        EngineMeta { id: "bing_cn", label: "必应(国内)", foreign: false, default_on: true },
        EngineMeta { id: "bing", label: "Bing", foreign: true, default_on: true },
        EngineMeta { id: "duckduckgo", label: "DuckDuckGo", foreign: true, default_on: true },
        EngineMeta { id: "google", label: "Google", foreign: true, default_on: false },
        EngineMeta { id: "brave", label: "Brave", foreign: true, default_on: false },
        EngineMeta { id: "mojeek", label: "Mojeek", foreign: true, default_on: false },
        EngineMeta { id: "searxng", label: "SearXNG(自建/公共)", foreign: true, default_on: false },
    ]
}

fn is_foreign(id: &str) -> bool {
    engine_metas().iter().find(|e| e.id == id).map(|e| e.foreign).unwrap_or(true)
}

async fn fetch(cfg: &ProxyConfig, engine: &str, url: &str) -> Option<String> {
    let client = proxy::client_for_source(cfg, is_foreign(engine)).ok()?;
    // 拟真浏览器请求头：降低被反爬识别/返回验证页的概率。
    let referer = url.split('/').take(3).collect::<Vec<_>>().join("/");
    // 瞬时失败(限流/网络抖动)时退避重试一次。
    for attempt in 0..2 {
        let req = client
            .get(url)
            .header(reqwest::header::ACCEPT,
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header(reqwest::header::ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en;q=0.8")
            .header(reqwest::header::REFERER, referer.clone())
            .header("Upgrade-Insecure-Requests", "1");
        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(t) = resp.text().await {
                    if !t.is_empty() { return Some(t); }
                }
            }
            _ => {}
        }
        if attempt == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        }
    }
    None
}

/// SearXNG 元搜索：聚合 Google/Bing/DDG 等多引擎，返回干净 JSON，远比单引擎抓 HTML 稳。
/// 实例地址由用户在设置里配置（`searxng_url`，可指自建或公共实例）。
async fn search_searxng(cfg: &ProxyConfig, query: &str, limit: usize) -> Vec<SearchHit> {
    let base = match super::super::store::get_setting("searxng_url").ok().flatten() {
        Some(u) if !u.trim().is_empty() => u.trim().trim_end_matches('/').to_string(),
        _ => return vec![],
    };
    let url = format!("{base}/search?q={}&format=json&safesearch=0", urlencoding::encode(query));
    let client = match proxy::client_for_source(cfg, true) { Ok(c) => c, Err(_) => return vec![] };
    let json: serde_json::Value = match client.get(&url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send().await
    {
        Ok(r) => match r.json().await { Ok(j) => j, Err(_) => return vec![] },
        Err(_) => return vec![],
    };
    let mut out = Vec::new();
    if let Some(arr) = json.get("results").and_then(|v| v.as_array()) {
        for r in arr.iter().take(limit) {
            let url = r.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if !url.starts_with("http") { continue; }
            let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let snippet = r.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
            out.push(SearchHit { title, url, snippet });
        }
    }
    out
}

fn sel(s: &str) -> Option<Selector> {
    Selector::parse(s).ok()
}

fn abs(base: &str, href: &str) -> String {
    if href.starts_with("http") {
        return href.to_string();
    }
    Url::parse(base)
        .ok()
        .and_then(|b| b.join(href).ok())
        .map(|u| u.to_string())
        .unwrap_or_default()
}

// PLACEHOLDER_DISPATCH

pub fn known_engines() -> Vec<&'static str> {
    engine_metas().into_iter().filter(|e| e.default_on).map(|e| e.id).collect()
}

/// 单个引擎的结果页结构：结果卡片 / 标题链接 / 摘要。
///
/// 等对话链路也要摘要，所以这里按“卡片”抓而不是只抓全页锚点；卡片选择器失效时
/// `parse_hits` 会退回旧的锚点抓法，只少摘要、不至于整页无结果。
struct EngineSpec {
    url: String,
    item: &'static [&'static str],
    link: &'static [&'static str],
    snippet: &'static [&'static str],
    decode: DecodeKind,
}

pub async fn search(cfg: &ProxyConfig, engine: &str, query: &str, limit: usize) -> Vec<SearchHit> {
    // SearXNG 走 JSON 接口，单独处理。
    if engine == "searxng" {
        return search_searxng(cfg, query, limit).await;
    }
    let q = urlencoding::encode(query);
    let spec = match engine {
        "bing" => EngineSpec {
            url: format!("https://www.bing.com/search?q={q}"),
            item: &["li.b_algo", "main ol li"],
            link: &["h2 a", "a[href^=\"http\"]"],
            snippet: &[".b_caption p", "p"],
            decode: DecodeKind::Direct,
        },
        "bing_cn" => EngineSpec {
            url: format!("https://cn.bing.com/search?q={q}"),
            item: &["li.b_algo", "main ol li"],
            link: &["h2 a", "a[href^=\"http\"]"],
            snippet: &[".b_caption p", "p"],
            decode: DecodeKind::Direct,
        },
        "duckduckgo" => EngineSpec {
            url: format!("https://html.duckduckgo.com/html/?q={q}"),
            item: &[".result", ".web-result"],
            link: &["a.result__a", "a[href]"],
            snippet: &[".result__snippet", ".result-snippet"],
            decode: DecodeKind::Ddg,
        },
        "google" => EngineSpec {
            url: format!("https://www.google.com/search?q={q}&num=20"),
            item: &["div.g", "div.MjjYud"],
            link: &["div.yuRUbf > a", "a[href^=\"/url?q=\"]"],
            snippet: &["div.VwiC3b", "div.IsZvec", "span.aCOpRe"],
            decode: DecodeKind::GoogleUrl,
        },
        "brave" => EngineSpec {
            url: format!("https://search.brave.com/search?q={q}"),
            item: &[".snippet", ".snippet[data-type=\"web\"]"],
            link: &["a.result-header", ".snippet a[href]"],
            snippet: &[".snippet-description", ".snippet-content"],
            decode: DecodeKind::Direct,
        },
        "mojeek" => EngineSpec {
            url: format!("https://www.mojeek.com/search?q={q}"),
            item: &["ul.results-standard li", ".results-standard li", "li"],
            link: &["a.ob", ".results-standard li h2 a", "h2 a"],
            snippet: &["p.s", ".s", "p"],
            decode: DecodeKind::Direct,
        },
        "baidu" => EngineSpec {
            url: format!("https://www.baidu.com/s?wd={q}"),
            item: &["div.result", "div.c-container", "div.result-op"],
            link: &["h3.t a", ".result h3 a", ".c-container h3 a"],
            snippet: &[".c-abstract", ".c-span-last", "span.content-right_8Zs40", ".c-color-text"],
            decode: DecodeKind::Direct,
        },
        "sogou" => EngineSpec {
            url: format!("https://www.sogou.com/web?query={q}"),
            item: &[".vrwrap", ".rb"],
            link: &[".vrwrap h3 a", ".rb h3 a", "h3 a"],
            snippet: &[".text-layout", ".str-text-info", ".ft", ".space-txt"],
            decode: DecodeKind::SogouRel,
        },
        "so360" => EngineSpec {
            url: format!("https://www.so.com/s?q={q}"),
            item: &[".res-list", "li.res-list"],
            link: &[".res-title a", ".res-list h3 a", "h3 a"],
            snippet: &[".res-desc", "p.res-desc", "p"],
            decode: DecodeKind::Direct,
        },
        _ => return vec![],
    };
    let body = match fetch(cfg, engine, &spec.url).await {
        Some(b) => b,
        None => return vec![],
    };
    parse_hits(&spec.url, &body, &spec, limit)
}

#[derive(Clone, Copy)]
enum DecodeKind {
    Direct,
    Ddg,
    GoogleUrl,
    SogouRel,
}

fn decode_href(base: &str, href: &str, kind: DecodeKind) -> String {
    match kind {
        DecodeKind::Ddg => {
            if let Some(idx) = href.find("uddg=") {
                let tail = &href[idx + 5..];
                let enc = tail.split('&').next().unwrap_or(tail);
                if let Ok(dec) = urlencoding::decode(enc) {
                    return dec.to_string();
                }
            }
            abs(base, href)
        }
        DecodeKind::GoogleUrl => {
            if let Some(idx) = href.find("/url?q=") {
                let tail = &href[idx + 7..];
                let enc = tail.split('&').next().unwrap_or(tail);
                if let Ok(dec) = urlencoding::decode(enc) {
                    return dec.to_string();
                }
            }
            abs(base, href)
        }
        DecodeKind::SogouRel => abs("https://www.sogou.com", href),
        DecodeKind::Direct => abs(base, href),
    }
}

fn clean_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn first_selector(list: &[&'static str]) -> Option<Selector> {
    list.iter().find_map(|s| sel(s))
}

/// 在结果卡片内按候选选择器取第一段文本。
fn first_text(node: &scraper::ElementRef<'_>, selectors: &[&'static str]) -> String {
    for raw in selectors {
        let Some(selector) = sel(raw) else { continue };
        if let Some(el) = node.select(&selector).next() {
            let text = clean_text(&el.text().collect::<Vec<_>>().join(" "));
            if !text.is_empty() {
                return text;
            }
        }
    }
    String::new()
}

fn parse_hits(base: &str, html: &str, spec: &EngineSpec, limit: usize) -> Vec<SearchHit> {
    let doc = Html::parse_document(html);
    let mut out: Vec<SearchHit> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut try_push = |out: &mut Vec<SearchHit>, title: String, url: String, snippet: String| {
        if title.is_empty() || !url.starts_with("http") || !seen.insert(url.clone()) {
            return;
        }
        out.push(SearchHit { title, url, snippet });
    };

    // 1) 按结果卡片抓标题 / 链接 / 摘要。
    if let Some(item_sel) = first_selector(spec.item) {
        let link_sel = first_selector(spec.link);
        for item in doc.select(&item_sel) {
            let (href, anchor_text) = match &link_sel {
                Some(s) => match item.select(s).next() {
                    Some(a) => (a.value().attr("href").unwrap_or("").to_string(), a.text().collect::<String>()),
                    None => continue,
                },
                None => continue,
            };
            if href.is_empty() || href == "#" {
                continue;
            }
            let real = decode_href(base, &href, spec.decode);
            let title = clean_text(&anchor_text);
            let snippet = first_text(&item, spec.snippet);
            try_push(&mut out, title, real, snippet);
            if out.len() >= limit {
                return out;
            }
        }
    }

    // 2) 兜底：卡片选择器失效时退回全页锚点抓法（没有摘要）。
    if out.is_empty() {
        for raw in spec.link {
            let Some(selector) = sel(raw) else { continue };
            for a in doc.select(&selector) {
                let href = match a.value().attr("href") {
                    Some(h) if !h.is_empty() && h != "#" => h,
                    _ => continue,
                };
                let real = decode_href(base, href, spec.decode);
                let title = clean_text(&a.text().collect::<String>());
                try_push(&mut out, title, real, String::new());
                if out.len() >= limit {
                    return out;
                }
            }
            if !out.is_empty() {
                break;
            }
        }
    }

    out
}

