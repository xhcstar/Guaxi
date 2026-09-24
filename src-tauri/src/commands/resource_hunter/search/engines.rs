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
    ]
}

fn is_foreign(id: &str) -> bool {
    engine_metas().iter().find(|e| e.id == id).map(|e| e.foreign).unwrap_or(true)
}

async fn fetch(cfg: &ProxyConfig, engine: &str, url: &str) -> Option<String> {
    let client = proxy::client_for_source(cfg, is_foreign(engine)).ok()?;
    let resp = client.get(url).send().await.ok()?;
    resp.text().await.ok()
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

pub async fn search(cfg: &ProxyConfig, engine: &str, query: &str, limit: usize) -> Vec<SearchHit> {
    let q = urlencoding::encode(query);
    let (url, selectors, decode): (String, Vec<&str>, DecodeKind) = match engine {
        "bing" => (format!("https://www.bing.com/search?q={q}"), vec!["li.b_algo h2 a", "h2 a"], DecodeKind::Direct),
        "bing_cn" => (format!("https://cn.bing.com/search?q={q}"), vec!["li.b_algo h2 a", "h2 a"], DecodeKind::Direct),
        "duckduckgo" => (format!("https://html.duckduckgo.com/html/?q={q}"), vec!["a.result__a"], DecodeKind::Ddg),
        "google" => (format!("https://www.google.com/search?q={q}&num=20"), vec!["div.yuRUbf > a", "a[href^=\"/url?q=\"]"], DecodeKind::GoogleUrl),
        "brave" => (format!("https://search.brave.com/search?q={q}"), vec!["a.result-header", ".snippet a[href]"], DecodeKind::Direct),
        "baidu" => (format!("https://www.baidu.com/s?wd={q}"), vec!["h3.t a", ".result h3 a", ".c-container h3 a"], DecodeKind::Direct),
        "sogou" => (format!("https://www.sogou.com/web?query={q}"), vec![".vrwrap h3 a", ".rb h3 a", "h3 a"], DecodeKind::SogouRel),
        "so360" => (format!("https://www.so.com/s?q={q}"), vec![".res-title a", ".res-list h3 a", "h3 a"], DecodeKind::Direct),
        _ => return vec![],
    };
    let body = match fetch(cfg, engine, &url).await {
        Some(b) => b,
        None => return vec![],
    };
    parse_hits(&url, &body, &selectors, decode, limit)
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

fn parse_hits(base: &str, html: &str, selectors: &[&str], decode: DecodeKind, limit: usize) -> Vec<SearchHit> {
    let doc = Html::parse_document(html);
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for s in selectors {
        let selector = match sel(s) {
            Some(x) => x,
            None => continue,
        };
        for a in doc.select(&selector) {
            let href = match a.value().attr("href") {
                Some(h) if !h.is_empty() && h != "#" => h,
                _ => continue,
            };
            let real = decode_href(base, href, decode);
            if !real.starts_with("http") || !seen.insert(real.clone()) {
                continue;
            }
            let title = a.text().collect::<String>().trim().to_string();
            out.push(SearchHit { title, url: real });
            if out.len() >= limit {
                return out;
            }
        }
        if !out.is_empty() {
            break;
        }
    }
    out
}

