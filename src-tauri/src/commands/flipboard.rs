use chrono::Utc;
use once_cell::sync::Lazy;
use rss::Channel;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{command, AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tracing::{error, info, warn};
use url::Url;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, CACHE_CONTROL, CONNECTION, PRAGMA, UPGRADE_INSECURE_REQUESTS};

// 全局状态
static ARTICLES: Lazy<Mutex<Vec<Article>>> = Lazy::new(|| Mutex::new(Vec::new()));
static CATEGORIES: Lazy<Mutex<Vec<Category>>> = Lazy::new(|| Mutex::new(get_default_categories()));
static FEEDS: Lazy<Mutex<Vec<FeedSource>>> = Lazy::new(|| Mutex::new(Vec::new()));
static FEEDS_INITIALIZED: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
static FEED_HEALTH: Lazy<Mutex<HashMap<String, FeedHealth>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    let mut default_headers = HeaderMap::new();
    default_headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"));
    default_headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"));
    default_headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    default_headers.insert(PRAGMA, HeaderValue::from_static("no-cache"));
    default_headers.insert(CONNECTION, HeaderValue::from_static("keep-alive"));
    default_headers.insert(UPGRADE_INSECURE_REQUESTS, HeaderValue::from_static("1"));

    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0 Safari/537.36")
        .default_headers(default_headers)
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .expect("build reqwest client failed")
});

/// 计算 flipboard 订阅源配置文件路径：可执行文件同级目录下的 config/flipboard_feeds.json
fn flipboard_config_path() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let base_dir = exe_path
        .parent()
        .ok_or_else(|| "无法获取可执行文件目录".to_string())?;
    let config_dir = base_dir.join("config");

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    }

    Ok(config_dir.join("flipboard_feeds.json"))
}

/// 从配置文件加载订阅源；如果文件不存在则返回默认订阅源
fn load_feeds_from_config() -> Result<Vec<FeedSource>, String> {
    let path = flipboard_config_path()?;
    if !path.exists() {
        info!(
            "flipboard: feeds config not found, use default feeds: {}",
            path.display()
        );
        return Ok(get_default_feeds());
    }

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let feeds: Vec<FeedSource> = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(feeds)
}

/// 将当前订阅源写回配置文件
fn save_feeds_to_config(feeds: &[FeedSource]) -> Result<(), String> {
    let path = flipboard_config_path()?;
    let json = serde_json::to_string_pretty(feeds).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    info!(
        "flipboard: feeds config saved, count = {}, path = {}",
        feeds.len(),
        path.display()
    );
    Ok(())
}

/// 确保 FEEDS 已经从配置文件初始化（只在第一次访问时加载）
fn ensure_feeds_initialized() {
    let mut initialized = FEEDS_INITIALIZED.lock().unwrap();
    if *initialized {
        return;
    }

    let mut feeds_lock = FEEDS.lock().unwrap();

    match load_feeds_from_config() {
        Ok(list) => {
            info!("flipboard: loaded {} feeds from config", list.len());
            *feeds_lock = list;
        }
        Err(e) => {
            warn!(
                "flipboard: load feeds config failed, use default feeds: {}",
                e
            );
            *feeds_lock = get_default_feeds();
            if let Err(se) = save_feeds_to_config(&feeds_lock) {
                warn!("flipboard: save default feeds config failed: {}", se);
            }
        }
    }

    *initialized = true;
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Article {
    pub id: String,
    pub title: String,
    pub description: String,
    pub content: String,
    pub source: String,
    pub url: String,
    pub image_url: Option<String>,
    pub published_at: String,
    pub category: String,
    pub author: Option<String>,
    pub read_count: u32,
    pub likes: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub color: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeedSource {
    pub id: String,
    pub name: String,
    pub url: String,
    pub category: String,
    pub enabled: bool,
    #[serde(default)]
    pub source_type: String,
    #[serde(default)]
    pub fetch_interval: Option<u32>,
    #[serde(default)]
    pub include_keywords: Option<String>,
    #[serde(default)]
    pub exclude_keywords: Option<String>,
    #[serde(default)]
    pub remark: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeedHealth {
    pub feed_id: String,
    pub feed_name: String,
    pub url: String,
    pub category: String,
    pub source_type: String,
    pub last_attempt_at: String,
    pub last_success_at: Option<String>,
    pub last_method: Option<String>,
    pub last_item_count: u32,
    pub last_error: Option<String>,
}

fn record_feed_health_success(feed: &FeedSource, method: &str, item_count: usize) {
    let now = Utc::now().to_rfc3339();
    let mut lock = FEED_HEALTH.lock().unwrap();

    lock.insert(
        feed.id.clone(),
        FeedHealth {
            feed_id: feed.id.clone(),
            feed_name: feed.name.clone(),
            url: feed.url.clone(),
            category: feed.category.clone(),
            source_type: feed.source_type.clone(),
            last_attempt_at: now.clone(),
            last_success_at: Some(now),
            last_method: Some(method.to_string()),
            last_item_count: item_count as u32,
            last_error: None,
        },
    );

    info!(
        "flipboard: feed health success, id = {}, method = {}, items = {}",
        feed.id, method, item_count
    );
}

fn record_feed_health_failure(feed: &FeedSource, method: &str, err: String) {
    let now = Utc::now().to_rfc3339();
    let mut lock = FEED_HEALTH.lock().unwrap();

    let prev_success = lock.get(&feed.id).and_then(|h| h.last_success_at.clone());

    lock.insert(
        feed.id.clone(),
        FeedHealth {
            feed_id: feed.id.clone(),
            feed_name: feed.name.clone(),
            url: feed.url.clone(),
            category: feed.category.clone(),
            source_type: feed.source_type.clone(),
            last_attempt_at: now,
            last_success_at: prev_success,
            last_method: Some(method.to_string()),
            last_item_count: 0,
            last_error: Some(err.clone()),
        },
    );

    warn!(
        "flipboard: feed health failure, id = {}, method = {}, err = {}",
        feed.id, method, err
    );
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchQuery {
    pub query: String,
    pub category: Option<String>,
    pub limit: Option<usize>,
}

// 获取默认分类
fn get_default_categories() -> Vec<Category> {
    vec![
        Category {
            id: "all".to_string(),
            name: "全部".to_string(),
            icon: "Globe".to_string(),
            color: "#3B82F6".to_string(),
        },
        Category {
            id: "technology".to_string(),
            name: "科技".to_string(),
            icon: "Chip".to_string(),
            color: "#8B5CF6".to_string(),
        },
        Category {
            id: "business".to_string(),
            name: "商业".to_string(),
            icon: "TrendingUp".to_string(),
            color: "#10B981".to_string(),
        },
        Category {
            id: "entertainment".to_string(),
            name: "游戏".to_string(),
            icon: "Film".to_string(),
            color: "#EC4899".to_string(),
        },
        Category {
            id: "lifestyle".to_string(),
            name: "生活".to_string(),
            icon: "Heart".to_string(),
            color: "#F97316".to_string(),
        },
        Category {
            id: "computer".to_string(),
            name: "计算机".to_string(),
            icon: "Chip".to_string(),
            color: "#0EA5E9".to_string(),
        },
        Category {
            id: "math".to_string(),
            name: "数学".to_string(),
            icon: "Calculator".to_string(),
            color: "#22C55E".to_string(),
        },
        Category {
            id: "physics".to_string(),
            name: "物理".to_string(),
            icon: "Flash".to_string(),
            color: "#F97316".to_string(),
        },
    ]
}

// 获取默认RSS源
fn get_default_feeds() -> Vec<FeedSource> {
    vec![]
}

#[derive(Debug, Clone)]
struct PlatformFeedTemplate {
    name: String,
    url: String,
    source_type: String,
    include_keywords: Option<String>,
    exclude_keywords: Option<String>,
    remark: Option<String>,
}

fn build_default_feeds_for_platform(platform: &str) -> Vec<PlatformFeedTemplate> {
    match platform {
        "zhihu" => vec![
            PlatformFeedTemplate {
                name: "知乎 - 热榜".to_string(),
                url: "https://rsshub.app/zhihu/hotlist".to_string(),
                source_type: "zhihu".to_string(),
                include_keywords: None,
                exclude_keywords: None,
                remark: Some("自动生成的知乎综合热榜信息源".to_string()),
            },
            PlatformFeedTemplate {
                name: "知乎 - 日报精选".to_string(),
                url: "https://rsshub.app/zhihu/daily".to_string(),
                source_type: "zhihu".to_string(),
                include_keywords: None,
                exclude_keywords: None,
                remark: Some("自动生成的知乎日报精选信息源".to_string()),
            },
        ],
        "bilibili" => vec![
            PlatformFeedTemplate {
                name: "B站 - 综合热门".to_string(),
                url: "https://rsshub.app/bilibili/popular/all".to_string(),
                source_type: "bilibili".to_string(),
                include_keywords: None,
                exclude_keywords: None,
                remark: Some("自动生成的 B 站综合热门信息源".to_string()),
            },
            PlatformFeedTemplate {
                name: "B站 - 日排行".to_string(),
                url: "https://rsshub.app/bilibili/ranking/0/0/1".to_string(),
                source_type: "bilibili".to_string(),
                include_keywords: None,
                exclude_keywords: None,
                remark: Some("自动生成的 B 站日排行信息源".to_string()),
            },
        ],
        _ => Vec::new(),
    }
}

// 生成模拟文章数据
fn generate_mock_articles() -> Vec<Article> {
    let mut articles = Vec::new();

    // 科技类文章
    let tech_articles = vec![
        (
            "人工智能在医疗领域的突破性进展",
            "探索AI如何彻底改变医疗行业",
            "technology",
        ),
        (
            "量子计算的未来：超越传统计算",
            "量子计算机的最新研究进展",
            "technology",
        ),
        (
            "5G与物联网的深度融合",
            "新一代通信技术如何连接万物",
            "technology",
        ),
        (
            "区块链技术的新应用场景",
            "区块链在数字身份和供应链管理中的应用",
            "technology",
        ),
        (
            "元宇宙：数字世界的未来",
            "虚拟现实和增强现实的融合",
            "technology",
        ),
    ];

    // 新闻类文章
    let news_articles = vec![
        (
            "全球气候变化峰会达成新协议",
            "各国就减排目标达成共识",
            "news",
        ),
        ("国际关系新动态", "多边合作与区域发展", "news"),
        ("经济政策调整影响市场", "货币政策对全球经济的影响", "news"),
        ("社会民生政策解读", "最新政策对民众生活的影响", "news"),
    ];

    // 商业类文章
    let business_articles = vec![
        ("初创企业融资趋势分析", "2024年投资风向标", "business"),
        (
            "数字化转型：企业必修课",
            "传统企业如何拥抱数字化",
            "business",
        ),
        ("ESG投资成为主流", "可持续投资的重要性", "business"),
    ];

    // 体育类文章
    let sports_articles = vec![
        ("2024年体育盛事展望", "奥运会和世界杯的筹备进展", "sports"),
        ("运动员训练科学化", "科技如何提升运动表现", "sports"),
    ];

    let all_articles: Vec<(&str, &str, &str)> = tech_articles
        .into_iter()
        .chain(news_articles)
        .chain(business_articles)
        .chain(sports_articles)
        .collect();

    for (i, (title, description, category)) in all_articles.into_iter().enumerate() {
        let id = (i + 1).to_string();
        let content = format!("{} 详细内容...这是文章的完整内容，包含更多的细节和信息。这篇文章探讨了相关主题的各个方面，为读者提供全面的视角。\n\n在当前的背景下，这个话题具有重要的意义。我们需要从多个角度来分析这个问题，以便更好地理解其影响和潜在的发展趋势。\n\n总之，这是一个值得深入探讨的话题。", description);

        let sources = vec!["新华网", "人民网", "央视新闻", "华尔街日报", "BBC", "CNN"];
        let authors = vec!["张三", "李四", "王五", "赵六", "钱七"];

        articles.push(Article {
            id,
            title: title.to_string(),
            description: description.to_string(),
            content,
            source: sources[i % sources.len()].to_string(),
            url: format!("https://example.com/article/{}", i + 1),
            image_url: Some(format!("https://picsum.photos/600/400?random={}", i)),
            published_at: Utc::now()
                .checked_sub_signed(chrono::Duration::hours(i as i64 * 2))
                .unwrap()
                .to_rfc3339(),
            category: category.to_string(),
            author: Some(authors[i % authors.len()].to_string()),
            read_count: (i * 100) as u32,
            likes: (i * 50) as u32,
        });
    }

    articles
}

#[command]
pub async fn get_articles(
    category: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<Article>, String> {
    let mut articles_lock = ARTICLES.lock().unwrap();

    // 如果是第一次获取，生成模拟数据
    if articles_lock.is_empty() {
        *articles_lock = generate_mock_articles();
    }

    let mut filtered_articles = if let Some(cat) = category {
        if cat == "all" {
            articles_lock.clone()
        } else {
            articles_lock
                .iter()
                .filter(|article| article.category == cat)
                .cloned()
                .collect()
        }
    } else {
        articles_lock.clone()
    };

    // 按发布时间排序（最新的在前）
    filtered_articles.sort_by(|a, b| b.published_at.cmp(&a.published_at));

    // 限制数量
    if let Some(limit) = limit {
        filtered_articles.truncate(limit);
    }

    Ok(filtered_articles)
}

#[command]
pub async fn get_article_detail(id: String) -> Result<Article, String> {
    let articles_lock = ARTICLES.lock().unwrap();

    articles_lock
        .iter()
        .find(|article| article.id == id)
        .cloned()
        .ok_or_else(|| "Article not found".to_string())
}

#[command]
pub async fn search_articles(
    query: String,
    category: Option<String>,
) -> Result<Vec<Article>, String> {
    let articles_lock = ARTICLES.lock().unwrap();
    let query_lower = query.to_lowercase();

    let results: Vec<Article> = articles_lock
        .iter()
        .filter(|article| {
            // 按类别筛选
            if let Some(cat) = &category {
                if article.category != *cat {
                    return false;
                }
            }

            // 搜索匹配
            article.title.to_lowercase().contains(&query_lower)
                || article.description.to_lowercase().contains(&query_lower)
                || article.content.to_lowercase().contains(&query_lower)
                || article.source.to_lowercase().contains(&query_lower)
        })
        .cloned()
        .collect();

    Ok(results)
}

#[command]
pub async fn get_categories() -> Result<Vec<Category>, String> {
    let categories_lock = CATEGORIES.lock().unwrap();
    Ok(categories_lock.clone())
}

#[command]
pub async fn get_feeds() -> Result<Vec<FeedSource>, String> {
    ensure_feeds_initialized();
    let feeds_lock = FEEDS.lock().unwrap();
    info!("flipboard: get_feeds, count = {}", feeds_lock.len());
    Ok(feeds_lock.clone())
}

#[command]
pub async fn get_feed_health() -> Result<Vec<FeedHealth>, String> {
    ensure_feeds_initialized();

    let enabled_ids: HashSet<String> = {
        let feeds_lock = FEEDS.lock().unwrap();
        feeds_lock
            .iter()
            .filter(|f| f.enabled)
            .map(|f| f.id.clone())
            .collect()
    };

    let mut list: Vec<FeedHealth> = {
        let lock = FEED_HEALTH.lock().unwrap();
        lock.values().cloned().collect()
    };

    list.retain(|h| enabled_ids.contains(&h.feed_id));
    list.sort_by(|a, b| a.feed_name.cmp(&b.feed_name));

    info!("flipboard: get_feed_health, count = {}", list.len());

    Ok(list)
}

#[command]
pub async fn generate_feeds_for_platforms(
    platforms: Vec<String>,
    category: String,
) -> Result<Vec<FeedSource>, String> {
    ensure_feeds_initialized();

    let normalized_category = category.trim().to_string();
    if normalized_category.is_empty() {
        return Err("category 不能为空".to_string());
    }

    {
        let categories_lock = CATEGORIES.lock().unwrap();
        if !categories_lock
            .iter()
            .any(|cat| cat.id == normalized_category)
            && normalized_category != "all"
        {
            return Err("Invalid category".to_string());
        }
    }

    let normalized_platforms: Vec<String> = platforms
        .into_iter()
        .map(|p| p.trim().to_lowercase())
        .filter(|p| !p.is_empty())
        .collect();

    if normalized_platforms.is_empty() {
        return Err("platforms 不能为空".to_string());
    }

    let mut feeds_lock = FEEDS.lock().unwrap();

    let mut existing_urls: HashSet<String> = feeds_lock
        .iter()
        .map(|f| f.url.trim().to_lowercase())
        .collect();

    let mut created = Vec::new();

    for platform in &normalized_platforms {
        let templates = build_default_feeds_for_platform(platform);
        if templates.is_empty() {
            warn!(
                "flipboard: no default feeds for platform {}, skip simple generate",
                platform
            );
            continue;
        }

        for t in templates {
            let url_norm = t.url.trim().to_lowercase();
            if url_norm.is_empty() || existing_urls.contains(&url_norm) {
                continue;
            }

            let feed = FeedSource {
                id: format!("f{}", Utc::now().timestamp_millis()),
                name: t.name,
                url: t.url,
                category: normalized_category.clone(),
                enabled: true,
                source_type: t.source_type,
                fetch_interval: Some(60),
                include_keywords: t.include_keywords,
                exclude_keywords: t.exclude_keywords,
                remark: t.remark,
            };

            existing_urls.insert(url_norm);
            feeds_lock.push(feed.clone());
            created.push(feed);
        }
    }

    if created.is_empty() {
        return Err("未生成任何新的信息源，请尝试使用高级配置".to_string());
    }

    if let Err(e) = save_feeds_to_config(&feeds_lock) {
        warn!(
            "flipboard: save_feeds_to_config failed after generate_feeds_for_platforms: {}",
            e
        );
    }

    info!(
        "flipboard: generate_feeds_for_platforms success, created = {}, category = {}, platforms = {:?}",
        created.len(),
        normalized_category,
        normalized_platforms
    );

    Ok(created)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FeedSourceInput {
    pub id: Option<String>,
    pub name: String,
    pub source_type: String,
    pub url: String,
    pub category: String,
    pub fetch_interval: Option<u32>,
    pub include_keywords: Option<String>,
    pub exclude_keywords: Option<String>,
    pub remark: Option<String>,
    pub enabled: Option<bool>,
}

#[command]
pub async fn add_feed(input: FeedSourceInput) -> Result<(), String> {
    ensure_feeds_initialized();
    let mut feeds_lock = FEEDS.lock().unwrap();

    let url = input.url.trim().to_string();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Invalid URL format".to_string());
    }

    if feeds_lock
        .iter()
        .any(|feed| feed.url.trim().eq_ignore_ascii_case(&url))
    {
        return Err("Feed already exists".to_string());
    }

    let category = input.category.trim().to_string();
    let categories_lock = CATEGORIES.lock().unwrap();
    if !categories_lock.iter().any(|cat| cat.id == category) && category != "all" {
        return Err("Invalid category".to_string());
    }

    let new_feed = FeedSource {
        id: format!("f{}", Utc::now().timestamp_millis()),
        name: input.name.trim().to_string(),
        url,
        category,
        enabled: input.enabled.unwrap_or(true),
        source_type: input.source_type.trim().to_string(),
        fetch_interval: input.fetch_interval,
        include_keywords: input.include_keywords,
        exclude_keywords: input.exclude_keywords,
        remark: input.remark,
    };

    feeds_lock.push(new_feed);

    if let Err(e) = save_feeds_to_config(&feeds_lock) {
        warn!(
            "flipboard: save_feeds_to_config failed after add_feed: {}",
            e
        );
    }

    info!("flipboard: add_feed success, total = {}", feeds_lock.len());

    Ok(())
}

#[command]
pub async fn set_feed_enabled(id: String, enabled: bool) -> Result<(), String> {
    ensure_feeds_initialized();

    let mut feeds_lock = FEEDS.lock().unwrap();

    let (target_id, target_enabled) = {
        let target = feeds_lock
            .iter_mut()
            .find(|f| f.id == id)
            .ok_or_else(|| "Feed not found".to_string())?;

        target.enabled = enabled;

        (target.id.clone(), target.enabled)
    };

    if let Err(e) = save_feeds_to_config(&feeds_lock) {
        warn!(
            "flipboard: save_feeds_to_config failed after set_feed_enabled: {}",
            e
        );
    }

    info!(
        "flipboard: set_feed_enabled success, id = {}, enabled = {}",
        target_id, target_enabled
    );

    Ok(())
}

#[command]
pub async fn update_feed(input: FeedSourceInput) -> Result<(), String> {
    ensure_feeds_initialized();

    let id = input
        .id
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "id 不能为空".to_string())?;

    let url = input.url.trim().to_string();
    let name = input.name.trim().to_string();
    let category = input.category.trim().to_string();

    if name.is_empty() {
        return Err("name 不能为空".to_string());
    }

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Invalid URL format".to_string());
    }

    {
        let categories_lock = CATEGORIES.lock().unwrap();
        if !categories_lock.iter().any(|cat| cat.id == category) && category != "all" {
            return Err("Invalid category".to_string());
        }
    }

    let mut feeds_lock = FEEDS.lock().unwrap();

    if feeds_lock
        .iter()
        .any(|f| f.id != id && f.url.trim().eq_ignore_ascii_case(&url))
    {
        return Err("Feed already exists".to_string());
    }

    let target_id = {
        let target = feeds_lock
            .iter_mut()
            .find(|f| f.id == id)
            .ok_or_else(|| "Feed not found".to_string())?;

        target.url = url;
        target.name = name;
        target.category = category;
        if let Some(enabled) = input.enabled {
            target.enabled = enabled;
        }
        if !input.source_type.trim().is_empty() {
            target.source_type = input.source_type.trim().to_string();
        }
        target.fetch_interval = input.fetch_interval;
        target.include_keywords = input.include_keywords;
        target.exclude_keywords = input.exclude_keywords;
        target.remark = input.remark;

        target.id.clone()
    };

    if let Err(e) = save_feeds_to_config(&feeds_lock) {
        warn!(
            "flipboard: save_feeds_to_config failed after update_feed: {}",
            e
        );
    }

    info!("flipboard: update_feed success, id = {}", target_id);

    Ok(())
}

#[command]
pub async fn delete_feed(id: String) -> Result<(), String> {
    ensure_feeds_initialized();

    let mut feeds_lock = FEEDS.lock().unwrap();
    let before = feeds_lock.len();
    feeds_lock.retain(|f| f.id != id);

    if feeds_lock.len() == before {
        return Err("Feed not found".to_string());
    }

    if let Err(e) = save_feeds_to_config(&feeds_lock) {
        warn!(
            "flipboard: save_feeds_to_config failed after delete_feed: {}",
            e
        );
    }

    info!(
        "flipboard: delete_feed success, remaining = {}",
        feeds_lock.len()
    );

    Ok(())
}

fn split_keywords(raw: &str) -> Vec<String> {
    raw.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

fn filter_articles_by_keywords(articles: Vec<Article>, feed: &FeedSource) -> Vec<Article> {
    let include = feed
        .include_keywords
        .as_deref()
        .map(split_keywords)
        .unwrap_or_default();
    let exclude = feed
        .exclude_keywords
        .as_deref()
        .map(split_keywords)
        .unwrap_or_default();

    if include.is_empty() && exclude.is_empty() {
        return articles;
    }

    articles
        .into_iter()
        .filter(|a| {
            let text = format!(
                "{} {} {}",
                a.title.to_lowercase(),
                a.description.to_lowercase(),
                a.content.to_lowercase()
            );

            if !include.is_empty() && !include.iter().any(|k| text.contains(k)) {
                return false;
            }

            if exclude.iter().any(|k| text.contains(k)) {
                return false;
            }

            true
        })
        .collect()
}

async fn fetch_rss_feed_internal(feed: &FeedSource) -> Result<Vec<Article>, String> {
    let url = feed.url.trim();
    if url.is_empty() {
        return Err("feed url is empty".to_string());
    }

    let parsed = Url::parse(url).map_err(|e| {
        error!("flipboard: invalid feed url {}: {}", url, e);
        format!("invalid feed url: {}", e)
    })?;

    info!("flipboard: fetching rss feed: {}", parsed);

    let resp = HTTP_CLIENT.get(parsed.clone()).send().await.map_err(|e| {
        error!("flipboard: request feed {} failed: {}", parsed, e);
        e.to_string()
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!(
            "flipboard: feed {} returned non-success: {} {}",
            parsed, status, body
        );
        return Err(format!("feed {} returned status {}", parsed, status));
    }

    let bytes = resp.bytes().await.map_err(|e| {
        error!("flipboard: read feed {} body failed: {}", parsed, e);
        e.to_string()
    })?;

    let cursor = std::io::Cursor::new(bytes);
    let channel = Channel::read_from(cursor).map_err(|e| {
        error!("flipboard: parse rss channel {} failed: {}", parsed, e);
        e.to_string()
    })?;

    let now = Utc::now().to_rfc3339();
    let mut articles = Vec::new();

    for (idx, item) in channel.items().iter().enumerate() {
        let title = item.title().unwrap_or("无标题").to_string();
        let description = item.description().unwrap_or("").to_string();
        let link = item.link().unwrap_or(url).to_string();
        let content = item
            .content()
            .or_else(|| item.description())
            .unwrap_or("")
            .to_string();

        let published_at = item
            .pub_date()
            .map(|d| d.to_string())
            .unwrap_or_else(|| now.clone());

        let id = item
            .guid()
            .map(|g| g.value().to_string())
            .unwrap_or_else(|| format!("{}-{}", feed.id, idx));

        articles.push(Article {
            id,
            title,
            description,
            content,
            source: feed.name.clone(),
            url: link,
            image_url: None,
            published_at,
            category: feed.category.clone(),
            author: None,
            read_count: 0,
            likes: 0,
        });
    }

    info!(
        "flipboard: fetched {} items from feed {} ({})",
        articles.len(),
        feed.name,
        feed.url
    );

    Ok(articles)
}

async fn fetch_html_and_extract_articles(feed: &FeedSource) -> Result<Vec<Article>, String> {
    let url = feed.url.trim();
    if url.is_empty() {
        return Err("feed url is empty".to_string());
    }

    let parsed = Url::parse(url).map_err(|e| {
        error!("flipboard: invalid html url {}: {}", url, e);
        format!("invalid html url: {}", e)
    })?;

    let base_host = parsed.host_str().unwrap_or("").to_string();

    info!(
        "flipboard: fetching html page: {} (host = {})",
        parsed, base_host
    );

    let resp = HTTP_CLIENT.get(parsed.clone()).send().await.map_err(|e| {
        error!("flipboard: request html {} failed: {}", parsed, e);
        e.to_string()
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!(
            "flipboard: html {} returned non-success: {} {}",
            parsed, status, body
        );
        return Err(format!("html {} returned status {}", parsed, status));
    }

    let body = resp.text().await.map_err(|e| {
        error!("flipboard: read html {} body failed: {}", parsed, e);
        e.to_string()
    })?;

    let document = Html::parse_document(&body);

    let selectors = vec![
        "article a",
        "h1 a, h2 a, h3 a",
        ".entry-title a, .post-title a, .title a",
        ".post a",
        "a",
    ];

    let mut seen = HashSet::new();
    let mut articles = Vec::new();
    let now = Utc::now().to_rfc3339();

    let is_same_site = |base: &Url, candidate: &Url| {
        let base_host = base.host_str().unwrap_or("");
        let cand_host = candidate.host_str().unwrap_or("");
        if base_host.is_empty() || cand_host.is_empty() {
            return false;
        }
        cand_host == base_host || cand_host.ends_with(&format!(".{}", base_host))
    };

    let is_noise_path = |u: &Url| {
        let p = u.path().to_lowercase();
        p == "/" || p.is_empty() || p.starts_with("/tag/") || p.starts_with("/tags/")
            || p.starts_with("/category/")
            || p.starts_with("/categories/")
            || p.starts_with("/about")
            || p.starts_with("/privacy")
            || p.starts_with("/terms")
            || p.starts_with("/login")
            || p.starts_with("/signup")
            || p.starts_with("/register")
    };

    for sel in selectors {
        let selector = match Selector::parse(sel) {
            Ok(s) => s,
            Err(e) => {
                warn!("flipboard: invalid css selector {}: {}", sel, e);
                continue;
            }
        };

        for element in document.select(&selector) {
            if articles.len() >= 50 {
                break;
            }

            let href_raw = element.value().attr("href").unwrap_or("").trim();
            if href_raw.is_empty() || href_raw.starts_with('#') {
                continue;
            }
            if href_raw.starts_with("javascript:")
                || href_raw.starts_with("mailto:")
                || href_raw.starts_with("tel:")
            {
                continue;
            }

            let resolved = Url::options().base_url(Some(&parsed)).parse(href_raw);
            let link_url = match resolved {
                Ok(u) => u,
                Err(_) => continue,
            };

            if !link_url.scheme().starts_with("http") {
                continue;
            }

            if !is_same_site(&parsed, &link_url) {
                continue;
            }

            if is_noise_path(&link_url) {
                continue;
            }

            let link = link_url.to_string();
            if !seen.insert(link.clone()) {
                continue;
            }

            let title_attr = element.value().attr("title").unwrap_or("").trim();
            let text = if !title_attr.is_empty() {
                title_attr.to_string()
            } else {
                element.text().collect::<String>().trim().to_string()
            };

            let title = text
                .split_whitespace()
                .collect::<Vec<&str>>()
                .join(" ");

            if title.len() < 8 || title.len() > 120 {
                continue;
            }

            let id = format!("{}-html-{}", feed.id, articles.len());

            articles.push(Article {
                id,
                title,
                description: String::new(),
                content: String::new(),
                source: feed.name.clone(),
                url: link,
                image_url: None,
                published_at: now.clone(),
                category: feed.category.clone(),
                author: None,
                read_count: 0,
                likes: 0,
            });
        }

        if articles.len() >= 20 {
            break;
        }
    }

    info!(
        "flipboard: fetched {} items from html {} ({})",
        articles.len(),
        feed.name,
        feed.url
    );

    Ok(articles)
}

async fn fetch_zhihu_fallback(feed: &FeedSource) -> Result<Vec<Article>, String> {
    let api_url = "https://www.zhihu.com/api/v3/feed/topstory/hot-lists/total?limit=50";

    info!(
        "flipboard: zhihu fallback using api {}, feed = {} ({})",
        api_url,
        feed.name,
        feed.url
    );

    let resp = HTTP_CLIENT.get(api_url).send().await.map_err(|e| {
        error!("flipboard: zhihu api request failed: {}", e);
        e.to_string()
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!(
            "flipboard: zhihu api {} returned non-success: {} {}",
            api_url, status, body
        );
        return Err(format!("zhihu api returned status {}", status));
    }

    let text = resp.text().await.map_err(|e| {
        error!("flipboard: read zhihu api body failed: {}", e);
        e.to_string()
    })?;

    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        error!("flipboard: parse zhihu api json failed: {}", e);
        e.to_string()
    })?;

    let data = v
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| "zhihu api json missing data array".to_string())?;

    let now = Utc::now().to_rfc3339();
    let mut articles = Vec::new();

    for (idx, item) in data.iter().enumerate() {
        let target = item.get("target").unwrap_or(&serde_json::Value::Null);
        let title = target
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("无标题")
            .to_string();
        let desc = target
            .get("excerpt")
            .or_else(|| target.get("desc"))
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();
        let raw_url = target
            .get("url")
            .and_then(|u| u.as_str())
            .unwrap_or("");
        let link = if raw_url.starts_with("http://") || raw_url.starts_with("https://") {
            raw_url.to_string()
        } else if raw_url.is_empty() {
            "https://www.zhihu.com/hot".to_string()
        } else {
            format!("https://www.zhihu.com{}", raw_url)
        };

        let id = item
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}-zhihu-{}", feed.id, idx));

        articles.push(Article {
            id,
            title,
            description: desc.clone(),
            content: desc,
            source: feed.name.clone(),
            url: link,
            image_url: None,
            published_at: now.clone(),
            category: feed.category.clone(),
            author: None,
            read_count: 0,
            likes: 0,
        });

        if articles.len() >= 50 {
            break;
        }
    }

    info!(
        "flipboard: zhihu fallback fetched {} items for feed {}",
        articles.len(),
        feed.name
    );

    Ok(articles)
}

async fn fetch_bilibili_fallback(feed: &FeedSource) -> Result<Vec<Article>, String> {
    let api_url = "https://api.bilibili.com/x/web-interface/popular";

    info!(
        "flipboard: bilibili fallback using api {}, feed = {} ({})",
        api_url,
        feed.name,
        feed.url
    );

    let resp = HTTP_CLIENT
        .get(api_url)
        .query(&[("ps", "50"), ("pn", "1")])
        .send()
        .await
        .map_err(|e| {
            error!("flipboard: bilibili api request failed: {}", e);
            e.to_string()
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!(
            "flipboard: bilibili api {} returned non-success: {} {}",
            api_url, status, body
        );
        return Err(format!("bilibili api returned status {}", status));
    }

    let text = resp.text().await.map_err(|e| {
        error!("flipboard: read bilibili api body failed: {}", e);
        e.to_string()
    })?;

    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        error!("flipboard: parse bilibili api json failed: {}", e);
        e.to_string()
    })?;

    let data = v
        .get("data")
        .and_then(|d| d.get("list"))
        .and_then(|d| d.as_array())
        .ok_or_else(|| "bilibili api json missing data.list array".to_string())?;

    let now = Utc::now().to_rfc3339();
    let mut articles = Vec::new();

    for (idx, item) in data.iter().enumerate() {
        let title = item
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("无标题")
            .to_string();
        let desc = item
            .get("desc")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();
        let bvid = item
            .get("bvid")
            .and_then(|b| b.as_str())
            .unwrap_or("");
        let short = item
            .get("short_link_v2")
            .and_then(|s| s.as_str())
            .unwrap_or("");
        let link = if !short.is_empty() {
            short.to_string()
        } else if !bvid.is_empty() {
            format!("https://www.bilibili.com/video/{}", bvid)
        } else {
            "https://www.bilibili.com".to_string()
        };

        let id = item
            .get("aid")
            .and_then(|v| v.as_i64())
            .map(|v| v.to_string())
            .unwrap_or_else(|| format!("{}-bilibili-{}", feed.id, idx));

        let author = item
            .get("owner")
            .and_then(|o| o.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string());

        let likes = item
            .get("stat")
            .and_then(|s| s.get("like"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let views = item
            .get("stat")
            .and_then(|s| s.get("view"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let image_url = item
            .get("pic")
            .and_then(|p| p.as_str())
            .map(|s| s.to_string());

        articles.push(Article {
            id,
            title,
            description: desc.clone(),
            content: desc,
            source: feed.name.clone(),
            url: link,
            image_url,
            published_at: now.clone(),
            category: feed.category.clone(),
            author,
            read_count: views as u32,
            likes: likes as u32,
        });

        if articles.len() >= 50 {
            break;
        }
    }

    info!(
        "flipboard: bilibili fallback fetched {} items for feed {}",
        articles.len(),
        feed.name
    );

    Ok(articles)
}

struct CrawlResult {
    method: &'static str,
    articles: Vec<Article>,
}

fn is_probably_json_api(url: &Url) -> bool {
    let host = url.host_str().unwrap_or("");
    let path = url.path();
    host.starts_with("api.")
        || (host.contains("zhihu.com") && path.contains("/api/"))
        || (host.contains("bilibili.com") && path.contains("/x/"))
        || (host.contains("bilibili.com") && path.contains("/web-interface/"))
}

async fn fetch_json_value(url: &Url) -> Result<serde_json::Value, String> {
    info!("flipboard: fetch json {}", url);

    let resp = HTTP_CLIENT.get(url.clone()).send().await.map_err(|e| {
        error!("flipboard: request json {} failed: {}", url, e);
        e.to_string()
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!("flipboard: json {} returned non-success: {} {}", url, status, body);
        return Err(format!("json {} returned status {}", url, status));
    }

    let text = resp.text().await.map_err(|e| {
        error!("flipboard: read json {} body failed: {}", url, e);
        e.to_string()
    })?;

    serde_json::from_str::<serde_json::Value>(&text).map_err(|e| {
        error!("flipboard: parse json {} failed: {}", url, e);
        e.to_string()
    })
}

fn extract_zhihu_articles_from_json(feed: &FeedSource, v: &serde_json::Value) -> Result<Vec<Article>, String> {
    let data = v
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| "zhihu json missing data array".to_string())?;

    let now = Utc::now().to_rfc3339();
    let mut articles = Vec::new();

    for (idx, item) in data.iter().enumerate() {
        let target = item.get("target").unwrap_or(item);
        let title = target
            .get("title")
            .or_else(|| target.get("name"))
            .and_then(|t| t.as_str())
            .unwrap_or("无标题")
            .trim()
            .to_string();

        if title.len() < 4 {
            continue;
        }

        let desc = target
            .get("excerpt")
            .or_else(|| target.get("desc"))
            .or_else(|| target.get("description"))
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let raw_url = target
            .get("url")
            .or_else(|| target.get("link"))
            .and_then(|u| u.as_str())
            .unwrap_or("");

        let link = if raw_url.starts_with("http://") || raw_url.starts_with("https://") {
            raw_url.to_string()
        } else if raw_url.is_empty() {
            feed.url.clone()
        } else {
            format!("https://www.zhihu.com{}", raw_url)
        };

        let id = item
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}-zhihu-json-{}", feed.id, idx));

        articles.push(Article {
            id,
            title,
            description: desc.clone(),
            content: desc,
            source: feed.name.clone(),
            url: link,
            image_url: None,
            published_at: now.clone(),
            category: feed.category.clone(),
            author: None,
            read_count: 0,
            likes: 0,
        });

        if articles.len() >= 50 {
            break;
        }
    }

    Ok(articles)
}

fn extract_bilibili_articles_from_json(feed: &FeedSource, v: &serde_json::Value) -> Result<Vec<Article>, String> {
    let data = v.get("data").ok_or_else(|| "bilibili json missing data".to_string())?;
    let list = data
        .get("list")
        .and_then(|d| d.as_array())
        .or_else(|| data.get("archives").and_then(|d| d.as_array()))
        .ok_or_else(|| "bilibili json missing data.list/data.archives".to_string())?;

    let now = Utc::now().to_rfc3339();
    let mut articles = Vec::new();

    for (idx, item) in list.iter().enumerate() {
        let title = item
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("无标题")
            .trim()
            .to_string();

        if title.len() < 4 {
            continue;
        }

        let desc = item
            .get("desc")
            .or_else(|| item.get("dynamic"))
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let bvid = item.get("bvid").and_then(|b| b.as_str()).unwrap_or("");
        let short = item
            .get("short_link_v2")
            .or_else(|| item.get("short_link"))
            .and_then(|s| s.as_str())
            .unwrap_or("");

        let link = if !short.is_empty() {
            short.to_string()
        } else if !bvid.is_empty() {
            format!("https://www.bilibili.com/video/{}", bvid)
        } else {
            "https://www.bilibili.com".to_string()
        };

        let id = item
            .get("aid")
            .and_then(|v| v.as_i64())
            .map(|v| v.to_string())
            .unwrap_or_else(|| format!("{}-bili-json-{}", feed.id, idx));

        let author = item
            .get("owner")
            .and_then(|o| o.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string());

        let likes = item
            .get("stat")
            .and_then(|s| s.get("like"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let views = item
            .get("stat")
            .and_then(|s| s.get("view"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let image_url = item.get("pic").and_then(|p| p.as_str()).map(|s| s.to_string());

        articles.push(Article {
            id,
            title,
            description: desc.clone(),
            content: desc,
            source: feed.name.clone(),
            url: link,
            image_url,
            published_at: now.clone(),
            category: feed.category.clone(),
            author,
            read_count: views as u32,
            likes: likes as u32,
        });

        if articles.len() >= 50 {
            break;
        }
    }

    Ok(articles)
}

async fn crawl_zhihu_when_rss_unavailable(feed: &FeedSource) -> Result<CrawlResult, String> {
    let parsed = Url::parse(feed.url.trim()).map_err(|e| format!("invalid url: {}", e))?;

    if is_probably_json_api(&parsed) {
        match fetch_json_value(&parsed).await {
            Ok(v) => match extract_zhihu_articles_from_json(feed, &v) {
                Ok(list) if !list.is_empty() => {
                    return Ok(CrawlResult {
                        method: "zhihu_api_url",
                        articles: list,
                    });
                }
                Ok(_) => warn!("flipboard: zhihu api url extracted empty, url = {}", feed.url),
                Err(e) => warn!("flipboard: zhihu api url extract failed, url = {}, err = {}", feed.url, e),
            },
            Err(e) => warn!("flipboard: zhihu api url request failed, url = {}, err = {}", feed.url, e),
        }
    }

    match fetch_html_and_extract_articles(feed).await {
        Ok(list) if !list.is_empty() => Ok(CrawlResult {
            method: "html",
            articles: list,
        }),
        _ => {
            let list = fetch_zhihu_fallback(feed).await?;
            Ok(CrawlResult {
                method: "zhihu_api_hot",
                articles: list,
            })
        }
    }
}

async fn crawl_bilibili_when_rss_unavailable(feed: &FeedSource) -> Result<CrawlResult, String> {
    let parsed = Url::parse(feed.url.trim()).map_err(|e| format!("invalid url: {}", e))?;

    if is_probably_json_api(&parsed) {
        match fetch_json_value(&parsed).await {
            Ok(v) => match extract_bilibili_articles_from_json(feed, &v) {
                Ok(list) if !list.is_empty() => {
                    return Ok(CrawlResult {
                        method: "bilibili_api_url",
                        articles: list,
                    });
                }
                Ok(_) => warn!("flipboard: bilibili api url extracted empty, url = {}", feed.url),
                Err(e) => warn!("flipboard: bilibili api url extract failed, url = {}, err = {}", feed.url, e),
            },
            Err(e) => warn!("flipboard: bilibili api url request failed, url = {}, err = {}", feed.url, e),
        }
    }

    match fetch_html_and_extract_articles(feed).await {
        Ok(list) if !list.is_empty() => Ok(CrawlResult {
            method: "html",
            articles: list,
        }),
        _ => {
            let list = fetch_bilibili_fallback(feed).await?;
            Ok(CrawlResult {
                method: "bilibili_api_popular",
                articles: list,
            })
        }
    }
}

async fn collect_articles_for_feed(feed: &FeedSource) -> Result<Vec<Article>, String> {
    let source_type = feed.source_type.trim().to_lowercase();

    match source_type.as_str() {
        "zhihu" => {
            let rss_result = fetch_rss_feed_internal(feed).await;
            match rss_result {
                Ok(list) if !list.is_empty() => {
                    let out = filter_articles_by_keywords(list, feed);
                    record_feed_health_success(feed, "rss", out.len());
                    return Ok(out);
                }
                Ok(_) => {
                    warn!(
                        "flipboard: zhihu rss returned empty, use crawler for url, feed = {}",
                        feed.url
                    );
                }
                Err(e) => {
                    warn!(
                        "flipboard: zhihu rss failed, use crawler for url, feed = {}, err = {}",
                        feed.url, e
                    );
                }
            }

            return match crawl_zhihu_when_rss_unavailable(feed).await {
                Ok(res) => {
                    let out = filter_articles_by_keywords(res.articles, feed);
                    record_feed_health_success(feed, res.method, out.len());
                    Ok(out)
                }
                Err(e) => {
                    record_feed_health_failure(feed, "zhihu_crawler", e.clone());
                    Ok(Vec::new())
                }
            };
        }
        "bilibili" => {
            let rss_result = fetch_rss_feed_internal(feed).await;
            match rss_result {
                Ok(list) if !list.is_empty() => {
                    let out = filter_articles_by_keywords(list, feed);
                    record_feed_health_success(feed, "rss", out.len());
                    return Ok(out);
                }
                Ok(_) => {
                    warn!(
                        "flipboard: bilibili rss returned empty, use crawler for url, feed = {}",
                        feed.url
                    );
                }
                Err(e) => {
                    warn!(
                        "flipboard: bilibili rss failed, use crawler for url, feed = {}, err = {}",
                        feed.url, e
                    );
                }
            }

            return match crawl_bilibili_when_rss_unavailable(feed).await {
                Ok(res) => {
                    let out = filter_articles_by_keywords(res.articles, feed);
                    record_feed_health_success(feed, res.method, out.len());
                    Ok(out)
                }
                Err(e) => {
                    record_feed_health_failure(feed, "bilibili_crawler", e.clone());
                    Ok(Vec::new())
                }
            };
        }
        _ => {}
    }

    if source_type.is_empty() || source_type == "rss" {
        match fetch_rss_feed_internal(feed).await {
            Ok(list) => {
                let out = filter_articles_by_keywords(list, feed);
                record_feed_health_success(feed, "rss", out.len());
                Ok(out)
            }
            Err(e) => {
                record_feed_health_failure(feed, "rss", e.clone());
                Err(e)
            }
        }
    } else {
        let rss_result = fetch_rss_feed_internal(feed).await;
        match rss_result {
            Ok(list) if !list.is_empty() => {
                let out = filter_articles_by_keywords(list, feed);
                record_feed_health_success(feed, "rss", out.len());
                Ok(out)
            }
            Ok(_) => match fetch_html_and_extract_articles(feed).await {
                Ok(list) => {
                    let out = filter_articles_by_keywords(list, feed);
                    record_feed_health_success(feed, "html", out.len());
                    Ok(out)
                }
                Err(e) => {
                    record_feed_health_failure(feed, "html", e.clone());
                    Ok(Vec::new())
                }
            },
            Err(e) => match fetch_html_and_extract_articles(feed).await {
                Ok(list) => {
                    let out = filter_articles_by_keywords(list, feed);
                    record_feed_health_success(feed, "html", out.len());
                    Ok(out)
                }
                Err(e2) => {
                    record_feed_health_failure(feed, "html", format!("rss_err = {}; html_err = {}", e, e2));
                    Ok(Vec::new())
                }
            },
        }
    }
}

#[command]
pub async fn fetch_rss_feed(url: String) -> Result<Vec<Article>, String> {
    let feed = FeedSource {
        id: "temporary".to_string(),
        name: "临时RSS源".to_string(),
        url,
        category: "all".to_string(),
        enabled: true,
        source_type: "rss".to_string(),
        fetch_interval: Some(60),
        include_keywords: None,
        exclude_keywords: None,
        remark: None,
    };
    fetch_rss_feed_internal(&feed).await
}

#[command]
pub async fn refresh_articles_from_feeds(
    category: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<Article>, String> {
    // 确保订阅源从配置文件初始化
    ensure_feeds_initialized();

    let feeds: Vec<FeedSource> = {
        let feeds_lock = FEEDS.lock().unwrap();
        feeds_lock.iter().filter(|f| f.enabled).cloned().collect()
    };

    if feeds.is_empty() {
        warn!("flipboard: no enabled feeds, fallback to mock articles");
        let mut articles = generate_mock_articles();
        if let Some(ref cat) = category {
            if cat != "all" {
                articles.retain(|a| &a.category == cat);
            }
        }
        if let Some(limit) = limit {
            articles.truncate(limit);
        }
        let mut lock = ARTICLES.lock().unwrap();
        *lock = articles.clone();
        return Ok(articles);
    }

    info!(
        "flipboard: refresh_articles_from_feeds start, feed_count = {}",
        feeds.len()
    );

    let mut all_articles = Vec::new();

    for feed in &feeds {
        match collect_articles_for_feed(feed).await {
            Ok(mut list) => {
                all_articles.append(&mut list);
            }
            Err(e) => {
                warn!("flipboard: collect feed {} failed: {}", feed.url, e);
            }
        }
    }

    if let Some(ref cat) = category {
        if cat != "all" {
            all_articles.retain(|a| &a.category == cat);
        }
    }

    all_articles.sort_by(|a, b| b.published_at.cmp(&a.published_at));

    if let Some(limit) = limit {
        all_articles.truncate(limit);
    }

    let mut lock = ARTICLES.lock().unwrap();
    *lock = all_articles.clone();

    info!(
        "flipboard: refresh_articles_from_feeds done, article_count = {}",
        all_articles.len()
    );

    Ok(all_articles)
}

#[allow(deprecated)]
#[command]
pub async fn get_featured_articles() -> Result<Vec<Article>, String> {
    let articles_lock = ARTICLES.lock().unwrap();

    let mut featured: Vec<Article> = articles_lock
        .iter()
        .filter(|article| article.image_url.is_some())
        .take(5)
        .cloned()
        .collect();

    // 随机排序
    use rand::seq::SliceRandom;
    use rand::thread_rng;
    let mut rng = thread_rng();
    featured.shuffle(&mut rng);

    Ok(featured)
}

#[command]
pub async fn get_recommendations(article_id: String) -> Result<Vec<Article>, String> {
    let articles_lock = ARTICLES.lock().unwrap();

    // 查找当前文章
    let current_article = articles_lock
        .iter()
        .find(|a| a.id == article_id)
        .ok_or_else(|| "Article not found".to_string())?;

    // 推荐同类别文章
    let recommendations: Vec<Article> = articles_lock
        .iter()
        .filter(|a| a.id != article_id && a.category == current_article.category)
        .take(4)
        .cloned()
        .collect();

    Ok(recommendations)
}

#[command]
pub async fn get_popular_articles() -> Result<Vec<Article>, String> {
    let articles_lock = ARTICLES.lock().unwrap();

    let mut popular: Vec<Article> = articles_lock.clone();
    popular.sort_by(|a, b| b.read_count.cmp(&a.read_count));
    popular.truncate(10);

    Ok(popular)
}

#[command]
pub async fn create_flipboard_feed_window(
    app: AppHandle,
    label: String,
    title: String,
) -> Result<(), String> {
    let label = label.trim().to_string();
    if label.is_empty() {
        return Err("label 不能为空".to_string());
    }

    if let Some(w) = app.get_webview_window(&label) {
        info!(
            "flipboard: feed manager window already exists, label = {}",
            label
        );
        w.show().map_err(|e| format!("显示窗口失败: {}", e))?;
        w.set_focus().ok();
        return Ok(());
    }

    let url = WebviewUrl::App("/flipboard-feeds".into());

    info!(
        "flipboard: create feed manager window label={} title={} url={:?}",
        label, title, url
    );

    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(900.0, 680.0)
        .min_inner_size(720.0, 520.0)
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
    window.set_focus().ok();

    Ok(())
}
