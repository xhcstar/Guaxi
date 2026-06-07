use std::collections::{hash_map::DefaultHasher, HashSet};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, vec};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone)]
pub struct SiteSeedInput {
    pub base_url: String,
    pub category: String,
    pub include_keywords: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FeedBlueprint {
    pub name: String,
    pub url: String,
    pub source_type: String,
    pub include_keywords: Option<String>,
    pub remark: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SiteDiscoveryResult {
    pub feeds: Vec<FeedBlueprint>,
}

fn normalize_base_url(raw: &str) -> Result<Url, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("base_url 不能为空".to_string());
    }
    let url = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        Url::parse(trimmed).map_err(|e| e.to_string())?
    } else {
        Url::parse(&format!("https://{}", trimmed)).map_err(|e| e.to_string())?
    };
    let host = url.host_str().ok_or_else(|| "无效的主站 URL".to_string())?;
    Url::parse(&format!("{}://{}", url.scheme(), host)).map_err(|e| e.to_string())
}

fn host_label(host: &str) -> String {
    host.trim_start_matches("www.").to_string()
}

fn category_hint_tokens(category: &str) -> Vec<String> {
    match category.trim().to_lowercase().as_str() {
        "ai" => vec!["ai", "llm", "agent", "deep learning", "模型"]
            .into_iter()
            .map(String::from)
            .collect(),
        "technology" => vec!["engineering", "backend", "frontend", "cloud", "架构"]
            .into_iter()
            .map(String::from)
            .collect(),
        "medical" => vec!["clinical", "health", "medicine", "biotech", "诊疗"]
            .into_iter()
            .map(String::from)
            .collect(),
        "education" => vec!["learning", "teaching", "course", "study", "学习"]
            .into_iter()
            .map(String::from)
            .collect(),
        "finance" => vec!["market", "investment", "risk", "macro", "quant"]
            .into_iter()
            .map(String::from)
            .collect(),
        _ => Vec::new(),
    }
}

fn profile_templates(host: &str) -> Vec<(&'static str, &'static str)> {
    if host.contains("zhihu.com") {
        return vec![
            ("知乎热榜", "https://www.zhihu.com/api/v3/feed/topstory/hot-lists/total?limit=50"),
            ("知乎发现", "https://www.zhihu.com/explore"),
        ];
    }
    if host.contains("bilibili.com") {
        return vec![
            ("B站热门", "https://api.bilibili.com/x/web-interface/popular"),
            ("B站动态", "https://www.bilibili.com"),
        ];
    }
    if host.contains("github.com") {
        return vec![
            ("GitHub Trending", "https://github.com/trending"),
            ("GitHub Explore", "https://github.com/explore"),
        ];
    }
    if host.contains("reddit.com") {
        return vec![
            ("Reddit Hot", "https://www.reddit.com/hot/.rss"),
            ("Reddit New", "https://www.reddit.com/new/.rss"),
        ];
    }
    if host.contains("arxiv.org") {
        return vec![
            ("arXiv cs.AI", "https://export.arxiv.org/rss/cs.AI"),
            ("arXiv cs.LG", "https://export.arxiv.org/rss/cs.LG"),
        ];
    }
    if host.contains("news.ycombinator.com") {
        return vec![
            ("Hacker News Front", "https://hnrss.org/frontpage"),
            ("Hacker News Newest", "https://hnrss.org/newest"),
        ];
    }
    if host.contains("wikipedia.org") {
        return vec![
            ("Wikipedia Featured", "https://en.wikipedia.org/wiki/Wikipedia:Featured_articles"),
            ("Wikipedia Portal", "https://en.wikipedia.org/wiki/Portal:Contents"),
        ];
    }
    if host.contains("medium.com") || host.contains("substack.com") || host.contains("wordpress.com")
    {
        return vec![
            ("站点 Feed", "/feed"),
            ("站点首页", "/"),
        ];
    }
    vec![
        ("站点 Feed", "/rss"),
        ("站点 Feed 2", "/feed"),
        ("站点 Atom", "/atom.xml"),
        ("站点首页", "/"),
    ]
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

pub fn build_site_discovery(seed: SiteSeedInput) -> Result<SiteDiscoveryResult, String> {
    let base = normalize_base_url(&seed.base_url)?;
    let host = base
        .host_str()
        .ok_or_else(|| "无法解析主站 host".to_string())?
        .to_string();
    let name = host_label(&host);
    let mut urls = HashSet::new();
    let mut feeds = Vec::new();
    let mut include_words = category_hint_tokens(&seed.category);
    if let Some(extra) = seed.include_keywords {
        include_words.extend(
            extra
                .split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty()),
        );
    }
    let include_keywords = if include_words.is_empty() {
        None
    } else {
        Some(include_words.join(","))
    };

    for (label, raw) in profile_templates(&host) {
        let final_url = if raw.starts_with("http://") || raw.starts_with("https://") {
            raw.to_string()
        } else {
            base.join(raw)
                .map_err(|e| e.to_string())?
                .to_string()
        };
        if !urls.insert(final_url.clone()) {
            continue;
        }
        let source_type = infer_source_type(&final_url);
        feeds.push(FeedBlueprint {
            name: format!("{} · {}", name, label),
            url: final_url,
            source_type: source_type.clone(),
            include_keywords: include_keywords.clone(),
            remark: Some(format!(
                "site_profile={}, category={}, source_type={}",
                host, seed.category, source_type
            )),
        });
    }

    Ok(SiteDiscoveryResult { feeds })
}

pub fn compact_plain_text(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// @deprecated 旧的"片段拼接"正文组装：把图片降级成纯文本链接列表。
/// 已被 `flipboard_extract`（trafilatura/scraper，保留内联图片）取代，保留作为
/// 最终兜底，勿在新代码中使用。
#[allow(dead_code)]
pub fn assemble_readable_article(
    lead: &str,
    paragraphs: &[String],
    image_links: &[String],
    media_links: &[String],
    source_url: &str,
) -> String {
    let mut blocks = Vec::new();
    if !lead.trim().is_empty() {
        blocks.push(compact_plain_text(lead));
    }
    for p in paragraphs {
        let line = compact_plain_text(p);
        if line.len() >= 20 {
            blocks.push(line);
        }
    }
    if !image_links.is_empty() {
        blocks.push("图片资源链接：".to_string());
        for x in image_links {
            blocks.push(format!("- {}", x));
        }
    }
    if !media_links.is_empty() {
        blocks.push("多媒体资源链接：".to_string());
        for x in media_links {
            blocks.push(format!("- {}", x));
        }
    }
    blocks.push(format!("原文链接：{}", source_url));
    blocks.join("\n\n")
}

#[derive(Debug, Clone)]
pub struct SchedulerPolicy {
    pub max_concurrency: usize,
    pub min_interval_secs: i64,
    pub max_backoff_secs: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeedScheduleState {
    pub next_run_at: i64,
    pub last_attempt_at: i64,
    pub last_success_at: i64,
    pub consecutive_failures: u32,
    pub last_error: Option<String>,
    pub last_fingerprint: Option<String>,
    pub last_article_count: usize,
}

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn default_scheduler_policy() -> SchedulerPolicy {
    SchedulerPolicy {
        max_concurrency: 4,
        min_interval_secs: 60,
        max_backoff_secs: 60 * 60,
    }
}

pub fn feed_interval_secs(fetch_interval_minutes: Option<u32>) -> i64 {
    let m = fetch_interval_minutes.unwrap_or(60).max(1) as i64;
    (m * 60).max(default_scheduler_policy().min_interval_secs)
}

pub fn should_run_feed(state: Option<&FeedScheduleState>, now: i64, force: bool) -> bool {
    if force {
        return true;
    }
    match state {
        None => true,
        Some(s) => s.next_run_at <= now,
    }
}

pub fn next_backoff_secs(failures: u32, max_backoff_secs: i64) -> i64 {
    let exp = (failures.min(10)) as u32;
    let base = 30_i64 * 2_i64.pow(exp);
    base.min(max_backoff_secs).max(30)
}

pub fn update_schedule_after_success(
    state: &mut FeedScheduleState,
    now: i64,
    interval_secs: i64,
    article_count: usize,
    fingerprint: Option<String>,
) {
    state.last_attempt_at = now;
    state.last_success_at = now;
    state.consecutive_failures = 0;
    state.last_error = None;
    state.last_article_count = article_count;
    state.last_fingerprint = fingerprint;
    state.next_run_at = now + interval_secs;
}

pub fn update_schedule_after_failure(
    state: &mut FeedScheduleState,
    now: i64,
    max_backoff_secs: i64,
    error: String,
) {
    state.last_attempt_at = now;
    state.consecutive_failures = state.consecutive_failures.saturating_add(1);
    state.last_error = Some(error);
    let backoff = next_backoff_secs(state.consecutive_failures, max_backoff_secs);
    state.next_run_at = now + backoff;
}

pub fn articles_fingerprint(items: &[(&str, &str)]) -> String {
    let mut hasher = DefaultHasher::new();
    for (u, t) in items {
        u.hash(&mut hasher);
        t.hash(&mut hasher);
    }
    format!("{:x}", hasher.finish())
}

#[allow(dead_code)]
pub fn site_readability_paragraph_selectors(url: &str) -> Vec<&'static str> {
    let u = url.to_lowercase();
    if u.contains("zhihu.com") {
        return vec![
            ".RichText.ztext p",
            ".Post-RichTextContainer p",
            ".ContentItem p",
            "main p",
            "p",
        ];
    }
    if u.contains("bilibili.com") {
        return vec![
            ".desc-info-text p",
            ".article-container p",
            ".opus-module-content p",
            "main p",
            "p",
        ];
    }
    if u.contains("medium.com") || u.contains("substack.com") {
        return vec!["article p", ".post-content p", "main p", "p"];
    }
    vec![
        "article p",
        "main p",
        ".post-content p",
        ".entry-content p",
        ".article-content p",
        ".content p",
        ".markdown-body p",
        "p",
    ]
}

#[allow(dead_code)]
pub fn site_readability_image_selectors(url: &str) -> Vec<&'static str> {
    let u = url.to_lowercase();
    if u.contains("zhihu.com") {
        return vec!["article img", ".RichText img", "img"];
    }
    vec![
        "article img",
        "main img",
        ".post-content img",
        ".entry-content img",
        ".article-content img",
        "img",
    ]
}

#[allow(dead_code)]
pub fn is_noise_paragraph(text: &str) -> bool {
    let s = text.to_lowercase();
    s.is_empty()
        || s.len() < 20
        || s.contains("广告")
        || s.contains("赞助")
        || s.contains("cookie")
        || s.contains("隐私政策")
        || s.contains("相关阅读")
        || s.contains("推荐阅读")
        || s.contains("登录后")
        || s.contains("点击展开")
        || s.contains("版权所有")
}

#[derive(Debug, Clone)]
pub struct WebsiteFeedRecord {
    pub site_name: String,
    pub base_url: String,
    pub feed_url: String,
    pub category: String,
}

pub fn normalize_category_id_from_heading(raw: &str) -> String {
    let s = raw.trim().to_lowercase();
    if s.contains("综合") || s.contains("新闻") {
        return "all".to_string();
    }
    if s.contains("科技") {
        return "technology".to_string();
    }
    if s.contains("ai") || s.contains("ml") || s.contains("机器学习") || s.contains("人工智能") {
        return "ai".to_string();
    }
    if s.contains("计算机") || s.contains("编程") || s.contains("开发者") {
        return "computer".to_string();
    }
    if s.contains("科学") || s.contains("物理") {
        return "physics".to_string();
    }
    if s.contains("数学") {
        return "math".to_string();
    }
    if s.contains("金融") || s.contains("经济") {
        return "finance".to_string();
    }
    if s.contains("医学") || s.contains("医疗") {
        return "medicine".to_string();
    }
    if s.contains("商业") {
        return "business".to_string();
    }
    if s.contains("游戏") {
        return "entertainment".to_string();
    }
    if s.contains("教育") {
        return "education".to_string();
    }
    if s.contains("法律") {
        return "law".to_string();
    }
    if s.contains("生活") || s.contains("文化") {
        return "lifestyle".to_string();
    }
    "all".to_string()
}

fn extract_markdown_link_url(cell: &str) -> Option<String> {
    let v = cell.trim();
    if v.is_empty() {
        return None;
    }
    if let Some(start) = v.find("](") {
        let rest = &v[start + 2..];
        if let Some(end) = rest.find(')') {
            let url = rest[..end].trim();
            if url.starts_with("http://") || url.starts_with("https://") {
                return Some(url.to_string());
            }
        }
    }
    let lower = v.to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(v.to_string());
    }
    None
}

pub fn load_feed_records_from_website_markdown(path: &Path) -> Result<Vec<WebsiteFeedRecord>, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let mut current_category = "all".to_string();
    let mut seen = HashSet::new();

    for raw in content.lines() {
        let line = raw.trim();
        if line.starts_with('#') {
            current_category = normalize_category_id_from_heading(line);
            continue;
        }
        if !line.starts_with('|') {
            continue;
        }
        if line.contains("---") {
            continue;
        }
        let cols: Vec<&str> = line
            .trim_matches('|')
            .split('|')
            .map(|x| x.trim())
            .collect();
        if cols.len() < 3 {
            continue;
        }
        if cols[0].contains("网站") || cols[1].eq_ignore_ascii_case("url") {
            continue;
        }

        let site_name = cols[0].trim().to_string();
        if site_name.is_empty() {
            continue;
        }
        let base_url = extract_markdown_link_url(cols[1]).unwrap_or_default();
        let rss_url = extract_markdown_link_url(cols[2]).unwrap_or_default();
        let feed_url = if !rss_url.is_empty() { rss_url } else { base_url.clone() };
        if feed_url.is_empty() {
            continue;
        }

        let key = format!(
            "{}|{}|{}",
            site_name.to_lowercase(),
            feed_url.to_lowercase(),
            current_category
        );
        if !seen.insert(key) {
            continue;
        }
        out.push(WebsiteFeedRecord {
            site_name,
            base_url,
            feed_url,
            category: current_category.clone(),
        });
    }
    Ok(out)
}

pub fn classify_category_by_text_and_url(
    default_category: &str,
    title: &str,
    description: &str,
    url: &str,
    source: &str,
) -> String {
    let text = format!(
        "{} {} {} {}",
        title.to_lowercase(),
        description.to_lowercase(),
        url.to_lowercase(),
        source.to_lowercase()
    );
    let mut checks: Vec<(&str, &[&str])> = vec![
        ("ai", &[" ai ", "llm", "gpt", "deepmind", "openai", "machine learning", "机器学习", "人工智能"]),
        ("computer", &["programming", "developer", "github", "stack overflow", "rust", "python", "javascript", "编程", "开发者"]),
        ("technology", &["tech", "technology", "科技", "芯片", "cloud", "software"]),
        ("finance", &["finance", "market", "economy", "经济", "金融", "投资", "stock"]),
        ("medicine", &["medicine", "medical", "clinical", "health", "医学", "医疗"]),
        ("law", &["law", "legal", "court", "法律", "法案"]),
        ("education", &["education", "learning", "course", "教育", "学习"]),
        ("entertainment", &["game", "gaming", "esports", "游戏", "动漫", "娱乐", "bilibili"]),
        ("lifestyle", &["lifestyle", "culture", "life", "生活", "文化"]),
        ("business", &["business", "management", "创业", "商业"]),
        ("math", &["math", "algebra", "几何", "数学"]),
        ("physics", &["physics", "science", "nasa", "space", "科研", "科学", "物理"]),
    ];
    checks.sort_by_key(|(_, words)| usize::MAX - words.len());
    for (cat, words) in checks {
        if words.iter().any(|w| text.contains(w)) {
            return cat.to_string();
        }
    }
    default_category.to_string()
}
