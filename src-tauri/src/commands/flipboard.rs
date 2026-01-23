use chrono::Utc;
use once_cell::sync::Lazy;
use rss::Channel;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{command, AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tracing::{error, info, warn};
use url::Url;

// 全局状态
static ARTICLES: Lazy<Mutex<Vec<Article>>> = Lazy::new(|| Mutex::new(Vec::new()));
static CATEGORIES: Lazy<Mutex<Vec<Category>>> = Lazy::new(|| Mutex::new(get_default_categories()));
static FEEDS: Lazy<Mutex<Vec<FeedSource>>> = Lazy::new(|| Mutex::new(Vec::new()));
static FEEDS_INITIALIZED: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent("guaxi-flipboard/0.1")
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
    vec![
        FeedSource {
            id: "1".to_string(),
            name: "BBC新闻".to_string(),
            url: "http://feeds.bbci.co.uk/news/rss.xml".to_string(),
            category: "news".to_string(),
            enabled: true,
        },
        FeedSource {
            id: "2".to_string(),
            name: "TechCrunch".to_string(),
            url: "https://techcrunch.com/feed/".to_string(),
            category: "technology".to_string(),
            enabled: true,
        },
        FeedSource {
            id: "3".to_string(),
            name: "哈佛商业评论".to_string(),
            url: "https://hbr.org/feed".to_string(),
            category: "business".to_string(),
            enabled: true,
        },
    ]
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
    // 确保从配置文件加载订阅源
    ensure_feeds_initialized();
    let feeds_lock = FEEDS.lock().unwrap();
    info!("flipboard: get_feeds, count = {}", feeds_lock.len());
    Ok(feeds_lock.clone())
}

#[command]
pub async fn add_feed(url: String, name: String, category: String) -> Result<(), String> {
    // 确保订阅源已从配置文件初始化
    ensure_feeds_initialized();
    let mut feeds_lock = FEEDS.lock().unwrap();

    // 检查URL是否有效
    if !url.starts_with("http") {
        return Err("Invalid URL format".to_string());
    }

    // 检查是否已存在
    if feeds_lock.iter().any(|feed| feed.url == url) {
        return Err("Feed already exists".to_string());
    }

    // 检查分类是否存在
    let categories_lock = CATEGORIES.lock().unwrap();
    if !categories_lock.iter().any(|cat| cat.id == category) && category != "all" {
        return Err("Invalid category".to_string());
    }

    let new_feed = FeedSource {
        id: format!("f{}", Utc::now().timestamp_millis()),
        name,
        url,
        category,
        enabled: true,
    };

    feeds_lock.push(new_feed);

    // 写入配置文件，保证订阅源持久化
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
pub async fn update_feed(
    id: String,
    url: String,
    name: String,
    category: String,
    enabled: bool,
) -> Result<(), String> {
    ensure_feeds_initialized();

    let url = url.trim().to_string();
    let name = name.trim().to_string();
    let category = category.trim().to_string();

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
        target.enabled = enabled;

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

#[command]
pub async fn fetch_rss_feed(url: String) -> Result<Vec<Article>, String> {
    let feed = FeedSource {
        id: "temporary".to_string(),
        name: "临时RSS源".to_string(),
        url,
        category: "all".to_string(),
        enabled: true,
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
        match fetch_rss_feed_internal(feed).await {
            Ok(mut list) => {
                all_articles.append(&mut list);
            }
            Err(e) => {
                warn!("flipboard: fetch feed {} failed: {}", feed.url, e);
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
