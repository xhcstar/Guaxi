//! 智能助手联网检索：多引擎并发 + 正文摘录。
//!
//! 旧实现按 Bing → Google → DDG → Python Agent 的顺序串行回退，单条查询最坏要等
//! 四次超时；拿回来的又只有标题，模型经常“有链接、没内容”。这里改成：
//!
//! 1. 境内/境外引擎并发抓，复用资源嗅探器里那套引擎适配器与代理分流
//!    （`resource_hunter::search::engines`），不再另写一套页面解析；
//! 2. 用户配置了 SearXNG 时优先用它的 JSON 结果（自带摘要、一次聚合多引擎）；
//! 3. 对前几条结果并发抓正文、抽可读文本作为摘要，给模型真正的资料。
//!
//! 每条查询的总耗时上限 ≈ 引擎 6s + 正文 5s（全部并发），不会再出现几十秒的空转。

use crate::commands::resource_hunter::model::ProxyConfig;
use crate::commands::resource_hunter::proxy;
use crate::commands::resource_hunter::search::engines;
use crate::commands::resource_hunter::search::load_proxy;
use crate::commands::resource_hunter::store;
use futures_util::future::join_all;
use scraper::{Html, Selector};
use serde::Serialize;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{info, warn};
use url::Url;

#[derive(Serialize, Clone, Debug)]
pub struct WebContextItem {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// 参与并发抓取的引擎。境内在前：中文问题命中率更高，境外再补英文资料。
const ENGINES: [&str; 5] = ["bing_cn", "bing", "duckduckgo", "baidu", "sogou"];
const ENGINE_TIMEOUT: Duration = Duration::from_secs(5);
const SEARXNG_TIMEOUT: Duration = Duration::from_secs(4);
const PAGE_TIMEOUT: Duration = Duration::from_secs(4);
/// 最多给几条结果抓正文（更多只增加延迟，模型 prompt 也放不下）。
const ENRICH_LIMIT: usize = 3;
const SNIPPET_CHARS: usize = 900;

pub async fn search(query: &str, limit: usize) -> Vec<WebContextItem> {
    let started = Instant::now();
    let cfg = load_proxy();

    let (engine_groups, searx) = tokio::join!(
        join_all(ENGINES.iter().map(|engine| {
            let cfg = &cfg;
            async move {
                match timeout(ENGINE_TIMEOUT, engines::search(cfg, engine, query, 10)).await {
                    Ok(hits) => hits,
                    Err(_) => {
                        info!(engine, "[web_search] engine timeout");
                        Vec::new()
                    }
                }
            }
        })),
        timeout(SEARXNG_TIMEOUT, searxng(&cfg, query, limit)),
    );

    let searx_hits = searx.unwrap_or_default();
    let mut candidates: Vec<WebContextItem> = searx_hits
        .iter()
        .map(|h| WebContextItem {
            title: h.title.clone(),
            url: h.url.clone(),
            snippet: h.snippet.clone(),
        })
        .collect();

    for group in engine_groups {
        for hit in group {
            candidates.push(WebContextItem {
                title: hit.title,
                url: hit.url,
                snippet: hit.snippet,
            });
        }
    }

    // 去重：同一篇文章常被多个引擎重复返回。
    let mut seen: HashSet<String> = HashSet::new();
    candidates.retain(|item| {
        let key = normalize_url(&item.url);
        !key.is_empty() && seen.insert(key)
    });

    // 只做“按相关度排序”，不因为词面不重合就整条丢弃——
    // 旧实现就是在这里把中文问题/改写过的 query 全部过滤成 0 条。
    let tokens = relevance_tokens(query);
    let mut scored: Vec<(i32, usize, WebContextItem)> = candidates
        .into_iter()
        .enumerate()
        .map(|(idx, item)| (score_item(&item, &tokens), idx, item))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let mut items: Vec<WebContextItem> = scored.into_iter().take(limit).map(|(_, _, i)| i).collect();

    // 摘要太短的补正文：前几条并发抓，失败就保留引擎给的摘要。
    let enrich_count = items.len().min(ENRICH_LIMIT);
    if enrich_count > 0 {
        let cfg_ref = &cfg;
        let excerpts = join_all(items[..enrich_count].iter().map(|item| {
            let url = item.url.clone();
            async move {
                match timeout(PAGE_TIMEOUT, fetch_page_text(cfg_ref, &url)).await {
                    Ok(Some(text)) => Some(text),
                    _ => None,
                }
            }
        }))
        .await;

        for (item, excerpt) in items.iter_mut().zip(excerpts) {
            let Some(text) = excerpt else { continue };
            if text.chars().count() > item.snippet.chars().count() {
                item.snippet = truncate_chars(&text, SNIPPET_CHARS);
            }
        }
    }

    for item in items.iter_mut() {
        item.snippet = truncate_chars(item.snippet.trim(), SNIPPET_CHARS);
    }

    info!(
        query_len = query.len(),
        results = items.len(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        "[web_search] finished"
    );
    items
}

async fn searxng(
    cfg: &ProxyConfig,
    query: &str,
    limit: usize,
) -> Vec<engines::SearchHit> {
    let Ok(Some(base)) = store::get_setting("searxng_url") else {
        return Vec::new();
    };
    let base = base.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return Vec::new();
    }

    let Ok(client) = proxy::client_for_source(cfg, true) else {
        return Vec::new();
    };
    let url = format!(
        "{base}/search?q={}&format=json&safesearch=0",
        urlencoding::encode(query)
    );
    let payload = match client
        .get(&url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(json) => json,
            Err(err) => {
                warn!(error = %err, "[web_search] searxng json parse failed");
                return Vec::new();
            }
        },
        Ok(resp) => {
            warn!(status = %resp.status(), "[web_search] searxng http error");
            return Vec::new();
        }
        Err(err) => {
            warn!(error = %err, "[web_search] searxng request failed");
            return Vec::new();
        }
    };

    let mut out = Vec::new();
    if let Some(results) = payload.get("results").and_then(|v| v.as_array()) {
        for item in results.iter().take(limit) {
            let url = item.get("url").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            if !url.starts_with("http") {
                continue;
            }
            out.push(engines::SearchHit {
                title: item.get("title").and_then(|v| v.as_str()).unwrap_or("").trim().to_string(),
                url,
                snippet: item.get("content").and_then(|v| v.as_str()).unwrap_or("").trim().to_string(),
            });
        }
    }
    out
}

fn normalize_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match Url::parse(trimmed) {
        Ok(mut url) => {
            url.set_fragment(None);
            let mut key = url.to_string();
            if url.path().len() > 1 {
                key = key.trim_end_matches('/').to_string();
            }
            key.to_lowercase()
        }
        Err(_) => String::new(),
    }
}

fn is_cjk(ch: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&ch)
}

/// 查询词面 token：中文按二元组、英文按单词。只用于排序，不用来丢弃结果。
fn relevance_tokens(query: &str) -> Vec<String> {
    let mut tokens: HashSet<String> = HashSet::new();
    let mut cjk_buf = String::new();
    let mut ascii_buf = String::new();

    let flush_cjk = |buf: &mut String, set: &mut HashSet<String>| {
        let chars: Vec<char> = buf.chars().collect();
        if chars.len() >= 2 {
            set.insert(buf.to_lowercase());
        }
        for window in chars.windows(2) {
            set.insert(window.iter().collect::<String>());
        }
        buf.clear();
    };
    let flush_ascii = |buf: &mut String, set: &mut HashSet<String>| {
        let cleaned = buf.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        if cleaned.len() >= 2 {
            set.insert(cleaned.to_lowercase());
        }
        buf.clear();
    };

    for ch in query.chars() {
        if is_cjk(ch) {
            flush_ascii(&mut ascii_buf, &mut tokens);
            cjk_buf.push(ch);
        } else if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '+' | ':') {
            flush_cjk(&mut cjk_buf, &mut tokens);
            ascii_buf.push(ch);
        } else {
            flush_cjk(&mut cjk_buf, &mut tokens);
            flush_ascii(&mut ascii_buf, &mut tokens);
        }
    }
    flush_cjk(&mut cjk_buf, &mut tokens);
    flush_ascii(&mut ascii_buf, &mut tokens);
    tokens.into_iter().collect()
}

fn score_item(item: &WebContextItem, tokens: &[String]) -> i32 {
    let hay = format!("{} {} {}", item.title, item.snippet, item.url).to_lowercase();
    let mut score = 0;
    for token in tokens {
        if token.is_empty() {
            continue;
        }
        if hay.contains(token) {
            score += if token.chars().any(is_cjk) { 3 } else { 2 };
        }
    }
    // 标题命中比摘要命中更重要。
    let title = item.title.to_lowercase();
    for token in tokens {
        if !token.is_empty() && title.contains(token) {
            score += 2;
        }
    }
    // “搜索引擎自己的跳转页”基本没有正文可抓，排后面。
    if item.url.contains("baidu.com/link") || item.url.contains("sogou.com/link") {
        score -= 2;
    }
    score
}

async fn fetch_page_text(cfg: &ProxyConfig, url: &str) -> Option<String> {
    if !url.starts_with("http") {
        return None;
    }
    let client = proxy::build_client_for(cfg, url).ok()?;
    let resp = client
        .get(url)
        .header(
            reqwest::header::ACCEPT,
            "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8",
        )
        .header(reqwest::header::ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en;q=0.8")
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36",
        )
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }
    if let Some(len) = resp.content_length() {
        if len > 4 * 1024 * 1024 {
            return None;
        }
    }
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();
    if !content_type.is_empty() && !content_type.contains("html") && !content_type.contains("text") {
        return None;
    }

    let body = resp.text().await.ok()?;
    let text = extract_readable_text(&body, SNIPPET_CHARS);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// 轻量正文抽取：优先正文容器里的段落，退到全页段落，再退到 meta description。
/// 只要给模型当摘要，不需要完整的分块/去导航逻辑（那套在 flipboard_extract）。
fn extract_readable_text(html: &str, limit: usize) -> String {
    let doc = Html::parse_document(html);
    let mut out = String::new();

    for raw in ["article p", "main p", "div.article p", "div.content p", "p"] {
        let Ok(selector) = Selector::parse(raw) else { continue };
        for node in doc.select(&selector) {
            let text = squash(&node.text().collect::<Vec<_>>().join(" "));
            if text.chars().count() < 16 {
                continue;
            }
            if out.chars().count() + text.chars().count() > limit {
                return out.trim().to_string();
            }
            out.push_str(&text);
            out.push(' ');
            if out.chars().count() >= limit {
                return out.trim().to_string();
            }
        }
        if out.chars().count() >= 120 {
            return out.trim().to_string();
        }
    }

    if out.is_empty() {
        if let Ok(selector) = Selector::parse("meta[name=\"description\"]") {
            if let Some(meta) = doc.select(&selector).next() {
                if let Some(content) = meta.value().attr("content") {
                    return truncate_chars(&squash(content), limit);
                }
            }
        }
    }
    out.trim().to_string()
}

fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(text: &str, max: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in text.chars().enumerate() {
        if idx >= max {
            break;
        }
        out.push(ch);
    }
    out
}
