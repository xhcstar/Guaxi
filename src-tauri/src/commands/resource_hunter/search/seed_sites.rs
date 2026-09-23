//! 富集站精确源：直接查开放 API，拿到干净的文件直链。
//! 一期接 arXiv（论文 PDF）与 OpenAlex（开放获取 PDF）。
use super::super::model::{ResourceItem, ResourceKind};
use super::super::proxy;
use super::make_id;
use regex::Regex;

fn client_for(url: &str) -> Option<reqwest::Client> {
    let cfg = super::load_proxy();
    proxy::build_client_for(&cfg, url).ok()
}

/// arXiv：Atom XML，正则抽 entry 的标题/摘要/pdf 链接。
pub async fn arxiv(keyword: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "http://export.arxiv.org/api/query?search_query=all:{}&max_results={}",
        urlencoding::encode(keyword),
        limit.min(50)
    );
    let client = match client_for(&url) {
        Some(c) => c,
        None => return vec![],
    };
    let body = match client.get(&url).send().await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(_) => return vec![],
    };
    let entry_re = Regex::new(r"(?s)<entry>(.*?)</entry>").unwrap();
    let title_re = Regex::new(r"(?s)<title>(.*?)</title>").unwrap();
    let sum_re = Regex::new(r"(?s)<summary>(.*?)</summary>").unwrap();
    let pdf_re = Regex::new(r#"<link title="pdf" href="([^"]+)""#).unwrap();
    let id_re = Regex::new(r"(?s)<id>(.*?)</id>").unwrap();
    let author_re = Regex::new(r"(?s)<name>(.*?)</name>").unwrap();

    let mut out = Vec::new();
    for cap in entry_re.captures_iter(&body) {
        let block = &cap[1];
        let title = title_re
            .captures(block)
            .map(|c| c[1].trim().replace('\n', " "))
            .unwrap_or_default();
        let pdf = pdf_re.captures(block).map(|c| c[1].to_string());
        let page = id_re
            .captures(block)
            .map(|c| c[1].trim().to_string())
            .unwrap_or_default();
        let download_url = pdf.map(|u| if u.ends_with(".pdf") { u } else { format!("{u}.pdf") });
        let src = download_url.clone().unwrap_or_else(|| page.clone());
        out.push(ResourceItem {
            id: make_id(&src),
            title: if title.is_empty() { "arXiv 论文".into() } else { title },
            kind: ResourceKind::Paper,
            source: "arXiv".into(),
            source_url: page,
            download_url,
            magnet: None,
            needs_ytdlp: false,
            format: Some("pdf".into()),
            thumbnail: None,
            author: author_re.captures(block).map(|c| c[1].trim().to_string()),
            published_at: None,
            summary: sum_re.captures(block).map(|c| c[1].trim().replace('\n', " ")),
            size_hint: None,
            relevance: 0.8,
            tags: vec!["paper".into()],
        });
    }
    out
}

/// OpenAlex：JSON，取每条的开放获取 PDF 链接。
pub async fn openalex(keyword: &str, limit: usize) -> Vec<ResourceItem> {
    let url = format!(
        "https://api.openalex.org/works?search={}&per-page={}",
        urlencoding::encode(keyword),
        limit.min(50)
    );
    let client = match client_for(&url) {
        Some(c) => c,
        None => return vec![],
    };
    let json: serde_json::Value = match client.get(&url).send().await {
        Ok(r) => r.json().await.unwrap_or(serde_json::Value::Null),
        Err(_) => return vec![],
    };
    let mut out = Vec::new();
    if let Some(results) = json.get("results").and_then(|v| v.as_array()) {
        for w in results {
            let title = w.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let pdf = w
                .get("best_oa_location")
                .and_then(|l| l.get("pdf_url"))
                .and_then(|v| v.as_str())
                .or_else(|| {
                    w.get("open_access")
                        .and_then(|o| o.get("oa_url"))
                        .and_then(|v| v.as_str())
                })
                .map(|s| s.to_string());
            let page = w.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if title.is_empty() {
                continue;
            }
            let src = pdf.clone().unwrap_or_else(|| page.clone());
            out.push(ResourceItem {
                id: make_id(&src),
                title,
                kind: ResourceKind::Paper,
                source: "OpenAlex".into(),
                source_url: page,
                download_url: pdf,
                magnet: None,
                needs_ytdlp: false,
                format: Some("pdf".into()),
                thumbnail: None,
                author: None,
                published_at: w
                    .get("publication_date")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                summary: None,
                size_hint: None,
                relevance: 0.7,
                tags: vec!["paper".into()],
            });
        }
    }
    out
}
