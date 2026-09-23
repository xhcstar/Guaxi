//! 通用搜索引擎适配器：抓结果页拿候选页面 URL，供嗅探器进一步扒文件链接。
//! 纯 reqwest + scraper，无需 API key。境内外引擎可分别开关。
use super::super::proxy;
use scraper::{Html, Selector};

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
}

fn client_for(url: &str) -> Option<reqwest::Client> {
    let cfg = super::load_proxy();
    proxy::build_client_for(&cfg, url).ok()
}

/// 已登记的引擎 id。默认集合在 aggregate 里选。
pub fn known_engines() -> Vec<&'static str> {
    vec!["duckduckgo", "bing"]
}

pub async fn search(engine: &str, query: &str, limit: usize) -> Vec<SearchHit> {
    match engine {
        "duckduckgo" => duckduckgo(query, limit).await,
        "bing" => bing(query, limit).await,
        _ => vec![],
    }
}

async fn duckduckgo(query: &str, limit: usize) -> Vec<SearchHit> {
    let url = format!("https://html.duckduckgo.com/html/?q={}", urlencoding::encode(query));
    let client = match client_for(&url) {
        Some(c) => c,
        None => return vec![],
    };
    let body = match client.get(&url).send().await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(_) => return vec![],
    };
    let doc = Html::parse_document(&body);
    let sel = Selector::parse("a.result__a").unwrap();
    let mut out = Vec::new();
    for a in doc.select(&sel).take(limit) {
        let href = a.value().attr("href").unwrap_or("");
        let real = extract_ddg_target(href);
        let title = a.text().collect::<String>().trim().to_string();
        if !real.is_empty() {
            out.push(SearchHit { title, url: real });
        }
    }
    out
}

/// DDG 结果链接常包一层 /l/?uddg=<编码真实URL>。
fn extract_ddg_target(href: &str) -> String {
    if let Some(idx) = href.find("uddg=") {
        let tail = &href[idx + 5..];
        let enc = tail.split('&').next().unwrap_or(tail);
        if let Ok(dec) = urlencoding::decode(enc) {
            return dec.to_string();
        }
    }
    if href.starts_with("http") {
        href.to_string()
    } else if href.starts_with("//") {
        format!("https:{href}")
    } else {
        String::new()
    }
}

async fn bing(query: &str, limit: usize) -> Vec<SearchHit> {
    let url = format!("https://www.bing.com/search?q={}", urlencoding::encode(query));
    let client = match client_for(&url) {
        Some(c) => c,
        None => return vec![],
    };
    let body = match client.get(&url).send().await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(_) => return vec![],
    };
    let doc = Html::parse_document(&body);
    let sel = Selector::parse("li.b_algo h2 a").unwrap();
    let mut out = Vec::new();
    for a in doc.select(&sel).take(limit) {
        let href = a.value().attr("href").unwrap_or("");
        let title = a.text().collect::<String>().trim().to_string();
        if href.starts_with("http") {
            out.push(SearchHit { title, url: href.to_string() });
        }
    }
    out
}
