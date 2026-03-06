use scraper::{Html, Selector};
use std::collections::HashSet;
use url::Url;

#[derive(Debug, Clone)]
pub struct DiscoveredCategoryFeed {
    pub category: String,
    pub label: String,
    pub url: String,
    pub source_type: String,
    pub include_keywords: Option<String>,
}

fn infer_source_type(url: &str) -> String {
    let u = url.to_lowercase();
    if u.contains("/api/") || u.contains("api.") {
        return "spider".to_string();
    }
    if u.ends_with(".xml") || u.contains("rss") || u.contains("atom") || u.contains("hnrss.org") {
        return "rss".to_string();
    }
    if u.contains("zhihu.com") {
        return "zhihu".to_string();
    }
    if u.contains("bilibili.com") {
        return "bilibili".to_string();
    }
    "spider".to_string()
}

fn classify_category_from_label_and_url(label: &str, url: &str) -> String {
    let t = format!("{} {}", label.to_lowercase(), url.to_lowercase());
    if t.contains("ai")
        || t.contains("ml")
        || t.contains("llm")
        || t.contains("人工智能")
        || t.contains("机器学习")
    {
        return "ai".to_string();
    }
    if t.contains("tech")
        || t.contains("科技")
        || t.contains("hardware")
        || t.contains("software")
        || t.contains("芯片")
    {
        return "technology".to_string();
    }
    if t.contains("dev")
        || t.contains("program")
        || t.contains("code")
        || t.contains("github")
        || t.contains("编程")
        || t.contains("开发")
    {
        return "computer".to_string();
    }
    if t.contains("finance")
        || t.contains("econom")
        || t.contains("market")
        || t.contains("金融")
        || t.contains("经济")
    {
        return "finance".to_string();
    }
    if t.contains("medicine")
        || t.contains("medical")
        || t.contains("health")
        || t.contains("医学")
        || t.contains("医疗")
    {
        return "medicine".to_string();
    }
    if t.contains("law") || t.contains("legal") || t.contains("法律") {
        return "law".to_string();
    }
    if t.contains("education") || t.contains("course") || t.contains("教育") || t.contains("学习") {
        return "education".to_string();
    }
    if t.contains("game")
        || t.contains("gaming")
        || t.contains("动漫")
        || t.contains("游戏")
        || t.contains("entertainment")
    {
        return "entertainment".to_string();
    }
    if t.contains("lifestyle")
        || t.contains("culture")
        || t.contains("生活")
        || t.contains("文化")
    {
        return "lifestyle".to_string();
    }
    if t.contains("business") || t.contains("商业") || t.contains("management") || t.contains("创业") {
        return "business".to_string();
    }
    if t.contains("math") || t.contains("数学") {
        return "math".to_string();
    }
    if t.contains("physics")
        || t.contains("science")
        || t.contains("space")
        || t.contains("物理")
        || t.contains("科学")
    {
        return "physics".to_string();
    }
    "all".to_string()
}

fn is_noise_link(url: &Url) -> bool {
    let p = url.path().to_lowercase();
    p == "/"
        || p.is_empty()
        || p.starts_with("/about")
        || p.starts_with("/privacy")
        || p.starts_with("/terms")
        || p.starts_with("/login")
        || p.starts_with("/signup")
        || p.starts_with("/register")
        || p.starts_with("/account")
}

fn feed_candidates(category_url: &Url) -> Vec<String> {
    let u = category_url.to_string();
    let lower = u.to_lowercase();
    if lower.contains("rss") || lower.contains("atom") || lower.ends_with(".xml") {
        return vec![u];
    }
    let mut out = Vec::new();
    out.push(u.clone());
    if let Ok(v) = category_url.join("rss") {
        out.push(v.to_string());
    }
    if let Ok(v) = category_url.join("feed") {
        out.push(v.to_string());
    }
    if let Ok(v) = category_url.join("atom.xml") {
        out.push(v.to_string());
    }
    out
}

pub fn discover_site_category_feeds_from_html(base: &Url, html: &str) -> Vec<DiscoveredCategoryFeed> {
    let document = Html::parse_document(html);
    let selectors = vec![
        "nav a[href]",
        "header a[href]",
        ".nav a[href]",
        ".menu a[href]",
        ".category a[href]",
        "a[href]",
    ];
    let mut links = Vec::new();
    let mut link_seen = HashSet::new();

    for sel in selectors {
        let Ok(selector) = Selector::parse(sel) else {
            continue;
        };
        for a in document.select(&selector) {
            let href = a.value().attr("href").unwrap_or("").trim();
            if href.is_empty() || href.starts_with('#') || href.starts_with("javascript:") {
                continue;
            }
            let Ok(url) = Url::options().base_url(Some(base)).parse(href) else {
                continue;
            };
            if !url.scheme().starts_with("http") {
                continue;
            }
            let base_host = base.host_str().unwrap_or("");
            let host = url.host_str().unwrap_or("");
            if base_host.is_empty() || host.is_empty() {
                continue;
            }
            if host != base_host && !host.ends_with(&format!(".{}", base_host)) {
                continue;
            }
            if is_noise_link(&url) {
                continue;
            }
            let label = a.text().collect::<String>().trim().to_string();
            if label.is_empty() || label.len() > 60 {
                continue;
            }
            let key = format!("{}|{}", label.to_lowercase(), url.as_str().to_lowercase());
            if !link_seen.insert(key) {
                continue;
            }
            links.push((label, url));
            if links.len() >= 120 {
                break;
            }
        }
        if links.len() >= 120 {
            break;
        }
    }

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (label, category_url) in links {
        let category = classify_category_from_label_and_url(&label, category_url.as_str());
        if category == "all" {
            continue;
        }
        for candidate in feed_candidates(&category_url) {
            let key = format!("{}|{}", category, candidate.to_lowercase());
            if !seen.insert(key) {
                continue;
            }
            out.push(DiscoveredCategoryFeed {
                category: category.clone(),
                label: label.clone(),
                url: candidate.clone(),
                source_type: infer_source_type(&candidate),
                include_keywords: Some(label.clone()),
            });
        }
    }

    out
}
