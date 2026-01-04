use serde::{Deserialize, Serialize};
use tauri::command;
use chrono::Utc;
use std::sync::Mutex;
use once_cell::sync::Lazy;

// 全局状态
static ARTICLES: Lazy<Mutex<Vec<Article>>> = Lazy::new(|| Mutex::new(Vec::new()));
static CATEGORIES: Lazy<Mutex<Vec<Category>>> = Lazy::new(|| Mutex::new(get_default_categories()));
static FEEDS: Lazy<Mutex<Vec<FeedSource>>> = Lazy::new(|| Mutex::new(get_default_feeds()));

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
            id: "news".to_string(),
            name: "新闻".to_string(),
            icon: "Newspaper".to_string(),
            color: "#EF4444".to_string(),
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
            id: "sports".to_string(),
            name: "体育".to_string(),
            icon: "Trophy".to_string(),
            color: "#F59E0B".to_string(),
        },
        Category {
            id: "entertainment".to_string(),
            name: "娱乐".to_string(),
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
            id: "science".to_string(),
            name: "科学".to_string(),
            icon: "Beaker".to_string(),
            color: "#06B6D4".to_string(),
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
        ("人工智能在医疗领域的突破性进展", "探索AI如何彻底改变医疗行业", "technology"),
        ("量子计算的未来：超越传统计算", "量子计算机的最新研究进展", "technology"),
        ("5G与物联网的深度融合", "新一代通信技术如何连接万物", "technology"),
        ("区块链技术的新应用场景", "区块链在数字身份和供应链管理中的应用", "technology"),
        ("元宇宙：数字世界的未来", "虚拟现实和增强现实的融合", "technology"),
    ];
    
    // 新闻类文章
    let news_articles = vec![
        ("全球气候变化峰会达成新协议", "各国就减排目标达成共识", "news"),
        ("国际关系新动态", "多边合作与区域发展", "news"),
        ("经济政策调整影响市场", "货币政策对全球经济的影响", "news"),
        ("社会民生政策解读", "最新政策对民众生活的影响", "news"),
    ];
    
    // 商业类文章
    let business_articles = vec![
        ("初创企业融资趋势分析", "2024年投资风向标", "business"),
        ("数字化转型：企业必修课", "传统企业如何拥抱数字化", "business"),
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
pub async fn get_articles(category: Option<String>, limit: Option<usize>) -> Result<Vec<Article>, String> {
    let mut articles_lock = ARTICLES.lock().unwrap();
    
    // 如果是第一次获取，生成模拟数据
    if articles_lock.is_empty() {
        *articles_lock = generate_mock_articles();
    }
    
    let mut filtered_articles = if let Some(cat) = category {
        if cat == "all" {
            articles_lock.clone()
        } else {
            articles_lock.iter()
                .filter(|article| article.category == cat)
                .cloned()
                .collect()
        }
    } else {
        articles_lock.clone()
    };
    
    // 按发布时间排序（最新的在前）
    filtered_articles.sort_by(|a, b| {
        b.published_at.cmp(&a.published_at)
    });
    
    // 限制数量
    if let Some(limit) = limit {
        filtered_articles.truncate(limit);
    }
    
    Ok(filtered_articles)
}

#[command]
pub async fn get_article_detail(id: String) -> Result<Article, String> {
    let articles_lock = ARTICLES.lock().unwrap();
    
    articles_lock.iter()
        .find(|article| article.id == id)
        .cloned()
        .ok_or_else(|| "Article not found".to_string())
}

#[command]
pub async fn search_articles(query: String, category: Option<String>) -> Result<Vec<Article>, String> {
    let articles_lock = ARTICLES.lock().unwrap();
    let query_lower = query.to_lowercase();
    
    let results: Vec<Article> = articles_lock.iter()
        .filter(|article| {
            // 按类别筛选
            if let Some(cat) = &category {
                if article.category != *cat {
                    return false;
                }
            }
            
            // 搜索匹配
            article.title.to_lowercase().contains(&query_lower) ||
            article.description.to_lowercase().contains(&query_lower) ||
            article.content.to_lowercase().contains(&query_lower) ||
            article.source.to_lowercase().contains(&query_lower)
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
    let feeds_lock = FEEDS.lock().unwrap();
    Ok(feeds_lock.clone())
}

#[command]
pub async fn add_feed(url: String, name: String, category: String) -> Result<(), String> {
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
        id: (feeds_lock.len() + 1).to_string(),
        name,
        url,
        category,
        enabled: true,
    };
    
    feeds_lock.push(new_feed);
    Ok(())
}

#[command]
pub async fn fetch_rss_feed(_url: String) -> Result<Vec<Article>, String> {
    // 在实际应用中，这里会从RSS源获取真实数据
    // 由于网络请求需要异步处理，这里返回模拟数据
    Ok(Vec::new())
}

#[allow(deprecated)]
#[command]
pub async fn get_featured_articles() -> Result<Vec<Article>, String> {
    let articles_lock = ARTICLES.lock().unwrap();
    
    let mut featured: Vec<Article> = articles_lock.iter()
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
    let current_article = articles_lock.iter()
        .find(|a| a.id == article_id)
        .ok_or_else(|| "Article not found".to_string())?;
    
    // 推荐同类别文章
    let recommendations: Vec<Article> = articles_lock.iter()
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