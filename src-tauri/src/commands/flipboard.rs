use chrono::Utc;
use crate::commands::flipboard_pipeline::{
    articles_fingerprint, assemble_readable_article, build_site_discovery,
    classify_category_by_text_and_url, compact_plain_text, default_scheduler_policy,
    feed_interval_secs, is_noise_paragraph, load_feed_records_from_website_markdown,
    site_readability_image_selectors, site_readability_paragraph_selectors, should_run_feed,
    unix_now, update_schedule_after_failure, update_schedule_after_success, FeedScheduleState,
    SiteSeedInput,
};
use crate::commands::site_category_discovery::discover_site_category_feeds_from_html;
use once_cell::sync::Lazy;
use rusqlite::{params, Connection, OptionalExtension};
use rss::Channel;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{command, AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};
use url::Url;
use reqwest::header::{
    HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, CACHE_CONTROL, CONNECTION, ETAG,
    IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED, PRAGMA, UPGRADE_INSECURE_REQUESTS,
};

// 全局状态
static ARTICLES: Lazy<Mutex<Vec<Article>>> = Lazy::new(|| Mutex::new(Vec::new()));
static CATEGORIES: Lazy<Mutex<Vec<Category>>> = Lazy::new(|| Mutex::new(get_default_categories()));
static FEEDS: Lazy<Mutex<Vec<FeedSource>>> = Lazy::new(|| Mutex::new(Vec::new()));
static FEEDS_INITIALIZED: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
static FEED_HEALTH: Lazy<Mutex<HashMap<String, FeedHealth>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static FEED_SCHEDULE: Lazy<Mutex<HashMap<String, FeedScheduleState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static FEED_HTTP_CACHE: Lazy<Mutex<HashMap<String, FeedHttpCacheState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static SCHEDULER_LOOP_STARTED: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
static SCHEDULER_ENABLED: AtomicBool = AtomicBool::new(true);
static LAST_CACHE_CLEANUP_AT: Lazy<Mutex<i64>> = Lazy::new(|| Mutex::new(0));
const CACHE_RETENTION_SECONDS: i64 = 60 * 60 * 24 * 7;
const CACHE_CLEANUP_INTERVAL_SECONDS: i64 = 60 * 60 * 24 * 7;
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

fn scheduler_state_path() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let base_dir = exe_path
        .parent()
        .ok_or_else(|| "无法获取可执行文件目录".to_string())?;
    let config_dir = base_dir.join("config");
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    }
    Ok(config_dir.join("flipboard_scheduler_state.json"))
}

fn feed_http_cache_path() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let base_dir = exe_path
        .parent()
        .ok_or_else(|| "无法获取可执行文件目录".to_string())?;
    let config_dir = base_dir.join("config");
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    }
    Ok(config_dir.join("flipboard_http_cache.json"))
}

fn article_cache_db_path() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let base_dir = exe_path
        .parent()
        .ok_or_else(|| "无法获取可执行文件目录".to_string())?;
    let repos_dir = base_dir.join("repos");
    if !repos_dir.exists() {
        fs::create_dir_all(&repos_dir).map_err(|e| e.to_string())?;
    }
    Ok(repos_dir.join("flipboard_cache.sqlite3"))
}

fn open_article_cache_db() -> Result<Connection, String> {
    let path = article_cache_db_path()?;
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        PRAGMA temp_store=MEMORY;
        CREATE TABLE IF NOT EXISTS article_cache (
            id TEXT NOT NULL,
            feed_id TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            source TEXT NOT NULL DEFAULT '',
            url TEXT NOT NULL,
            image_url TEXT,
            published_at TEXT NOT NULL DEFAULT '',
            category TEXT NOT NULL DEFAULT 'all',
            author TEXT,
            read_count INTEGER NOT NULL DEFAULT 0,
            likes INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY(feed_id, url)
        );
        CREATE INDEX IF NOT EXISTS idx_article_cache_category_time
            ON article_cache(category, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_article_cache_feed_time
            ON article_cache(feed_id, updated_at DESC);
        "#,
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

fn save_articles_to_cache(feed_id: &str, items: &[Article]) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }
    let mut conn = open_article_cache_db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let now = unix_now();
    for a in items {
        tx.execute(
            r#"
            INSERT INTO article_cache
                (id, feed_id, title, description, content, source, url, image_url, published_at, category, author, read_count, likes, updated_at)
            VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ON CONFLICT(feed_id, url) DO UPDATE SET
                id=excluded.id,
                title=excluded.title,
                description=excluded.description,
                content=excluded.content,
                source=excluded.source,
                image_url=excluded.image_url,
                published_at=excluded.published_at,
                category=excluded.category,
                author=excluded.author,
                read_count=excluded.read_count,
                likes=excluded.likes,
                updated_at=excluded.updated_at
            "#,
            params![
                a.id,
                feed_id,
                a.title,
                a.description,
                a.content,
                a.source,
                a.url,
                a.image_url,
                a.published_at,
                a.category,
                a.author,
                a.read_count,
                a.likes,
                now
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

fn cleanup_old_cached_articles_if_due(now: i64) -> Result<(), String> {
    {
        let guard = LAST_CACHE_CLEANUP_AT
            .lock()
            .map_err(|e| format!("lock LAST_CACHE_CLEANUP_AT failed: {}", e))?;
        if *guard > 0 && now - *guard < CACHE_CLEANUP_INTERVAL_SECONDS {
            return Ok(());
        }
    }

    let cutoff = now - CACHE_RETENTION_SECONDS;
    let conn = open_article_cache_db()?;
    let removed = conn
        .execute(
            "DELETE FROM article_cache WHERE updated_at > 0 AND updated_at < ?1",
            params![cutoff],
        )
        .map_err(|e| e.to_string())?;

    {
        let mut guard = LAST_CACHE_CLEANUP_AT
            .lock()
            .map_err(|e| format!("lock LAST_CACHE_CLEANUP_AT failed: {}", e))?;
        *guard = now;
    }

    info!(
        "flipboard cache cleanup finished: removed={}, cutoff={}",
        removed, cutoff
    );
    Ok(())
}

fn row_to_article(row: &rusqlite::Row<'_>) -> rusqlite::Result<Article> {
    Ok(Article {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        content: row.get(3)?,
        source: row.get(4)?,
        url: row.get(5)?,
        image_url: row.get(6)?,
        published_at: row.get(7)?,
        category: row.get(8)?,
        author: row.get(9)?,
        read_count: row.get(10)?,
        likes: row.get(11)?,
    })
}

fn load_cached_articles_by_feed_ids(
    feed_ids: &[String],
    category: Option<&str>,
    limit: usize,
) -> Result<Vec<Article>, String> {
    if feed_ids.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let conn = open_article_cache_db()?;
    let mut out = Vec::new();
    let category = category.unwrap_or("").trim().to_string();
    for feed_id in feed_ids {
        let mut sql = String::from(
            "SELECT id,title,description,content,source,url,image_url,published_at,category,author,read_count,likes \
             FROM article_cache WHERE feed_id = ?1",
        );
        if !category.is_empty() && category != "all" {
            sql.push_str(" AND (category = ?2 OR category = 'all')");
            sql.push_str(" ORDER BY updated_at DESC LIMIT ?3");
            let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![feed_id, category, limit as i64], row_to_article)
                .map_err(|e| e.to_string())?;
            for x in rows {
                if let Ok(a) = x {
                    out.push(a);
                }
            }
            continue;
        }
        sql.push_str(" ORDER BY updated_at DESC LIMIT ?2");
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![feed_id, limit as i64], row_to_article)
            .map_err(|e| e.to_string())?;
        for x in rows {
            if let Ok(a) = x {
                out.push(a);
            }
        }
    }
    Ok(dedup_articles_by_identity(out))
}

fn has_cached_articles_for_feed(feed_id: &str) -> Result<bool, String> {
    let conn = open_article_cache_db()?;
    let mut stmt = conn
        .prepare("SELECT 1 FROM article_cache WHERE feed_id = ?1 LIMIT 1")
        .map_err(|e| e.to_string())?;
    let found = stmt
        .query_row(params![feed_id], |_| Ok(1_i64))
        .optional()
        .map_err(|e| e.to_string())?
        .is_some();
    Ok(found)
}

fn load_cached_articles(
    category: Option<&str>,
    topic: Option<&str>,
    limit: usize,
) -> Result<Vec<Article>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let conn = open_article_cache_db()?;
    let category = category.unwrap_or("").trim().to_string();
    let mut sql = String::from(
        "SELECT id,title,description,content,source,url,image_url,published_at,category,author,read_count,likes \
         FROM article_cache WHERE 1=1",
    );
    if !category.is_empty() && category != "all" {
        sql.push_str(" AND (category = ?1 OR category = 'all')");
    }
    sql.push_str(" ORDER BY updated_at DESC LIMIT ");
    sql.push_str(&limit.to_string());
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = if !category.is_empty() && category != "all" {
        stmt.query_map(params![category], row_to_article)
            .map_err(|e| e.to_string())?
    } else {
        stmt.query_map([], row_to_article).map_err(|e| e.to_string())?
    };
    let mut out = Vec::new();
    for x in rows {
        if let Ok(a) = x {
            out.push(a);
        }
    }
    if let Some(topic_text) = topic {
        let tokens = topic_tokens_from_text(topic_text);
        if !tokens.is_empty() {
            out.retain(|a| score_article_for_topic(a, &tokens) > 0);
            out.sort_by(|a, b| {
                score_article_for_topic(b, &tokens)
                    .cmp(&score_article_for_topic(a, &tokens))
                    .then_with(|| b.published_at.cmp(&a.published_at))
            });
        }
    }
    out.truncate(limit);
    Ok(out)
}

fn load_cached_article_detail(id: &str, url: Option<&str>) -> Result<Option<Article>, String> {
    let conn = open_article_cache_db()?;
    if !id.trim().is_empty() {
        let mut stmt = conn
            .prepare(
                "SELECT id,title,description,content,source,url,image_url,published_at,category,author,read_count,likes \
                 FROM article_cache WHERE id = ?1 ORDER BY updated_at DESC LIMIT 1",
            )
            .map_err(|e| e.to_string())?;
        let found = stmt
            .query_row(params![id.trim()], row_to_article)
            .optional()
            .map_err(|e| e.to_string())?;
        if found.is_some() {
            return Ok(found);
        }
    }
    if let Some(url_text) = url.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let mut stmt = conn
            .prepare(
                "SELECT id,title,description,content,source,url,image_url,published_at,category,author,read_count,likes \
                 FROM article_cache WHERE url = ?1 ORDER BY updated_at DESC LIMIT 1",
            )
            .map_err(|e| e.to_string())?;
        let found = stmt
            .query_row(params![url_text], row_to_article)
            .optional()
            .map_err(|e| e.to_string())?;
        return Ok(found);
    }
    Ok(None)
}

fn website_markdown_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd.join("docs").join("website.md"));
        out.push(cwd.join("..").join("docs").join("website.md"));
    }
    out.push(PathBuf::from("e:\\Record\\source\\Vue\\guaxi\\docs\\website.md"));
    out
}

fn merge_feeds_from_website_markdown(feeds: &mut Vec<FeedSource>) {
    let mut existing_keys: HashSet<String> = feeds
        .iter()
        .map(|f| format!("{}|{}", f.url.trim().to_lowercase(), f.category.trim().to_lowercase()))
        .collect();

    for p in website_markdown_candidates() {
        if !p.exists() {
            continue;
        }
        match load_feed_records_from_website_markdown(&p) {
            Ok(records) => {
                let mut added = 0usize;
                for (idx, r) in records.into_iter().enumerate() {
                    let category = r.category.trim().to_string();
                    let key = format!("{}|{}", r.feed_url.trim().to_lowercase(), category);
                    if !existing_keys.insert(key) {
                        continue;
                    }
                    let host = Url::parse(&r.base_url)
                        .ok()
                        .and_then(|u| u.host_str().map(|h| h.to_string()))
                        .unwrap_or_else(|| "site".to_string());
                    let source_type = infer_source_type_from_url(&r.feed_url);
                    feeds.push(FeedSource {
                        id: format!("website-{}-{}-{}", category, host.replace('.', "-"), idx),
                        name: r.site_name.clone(),
                        url: r.feed_url.clone(),
                        category: category.clone(),
                        enabled: true,
                        source_type,
                        fetch_interval: Some(60),
                        include_keywords: None,
                        exclude_keywords: None,
                        remark: Some(format!("website.md:{}|base={}", category, r.base_url)),
                    });
                    added += 1;
                }
                info!(
                    "flipboard: merged {} feeds from website markdown: {}",
                    added,
                    p.display()
                );
                return;
            }
            Err(e) => {
                warn!(
                    "flipboard: parse website markdown failed: {}, err = {}",
                    p.display(),
                    e
                );
            }
        }
    }
}

/// 从配置文件加载订阅源；如果文件不存在则返回默认订阅源
fn load_feeds_from_config() -> Result<Vec<FeedSource>, String> {
    let path = flipboard_config_path()?;
    let mut feeds: Vec<FeedSource> = if !path.exists() {
        info!(
            "flipboard: feeds config not found, use default feeds: {}",
            path.display()
        );
        get_default_feeds()
    } else {
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())?
    };
    let defaults = get_default_feeds();
    let mut existing_keys: HashSet<String> = feeds
        .iter()
        .map(|f| {
            format!(
                "{}|{}|{}",
                f.url.trim().to_lowercase(),
                f.category.trim().to_lowercase(),
                f.source_type.trim().to_lowercase()
            )
        })
        .collect();

    let mut merged_count = 0usize;
    for d in defaults {
        let key = format!(
            "{}|{}|{}",
            d.url.trim().to_lowercase(),
            d.category.trim().to_lowercase(),
            d.source_type.trim().to_lowercase()
        );
        if existing_keys.insert(key) {
            feeds.push(d);
            merged_count += 1;
        }
    }

    if merged_count > 0 {
        info!(
            "flipboard: merged {} builtin feeds into current config snapshot",
            merged_count
        );
    }

    merge_feeds_from_website_markdown(&mut feeds);

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

fn save_scheduler_state_to_config() -> Result<(), String> {
    let path = scheduler_state_path()?;
    let lock = FEED_SCHEDULE.lock().unwrap();
    let json = serde_json::to_string_pretty(&*lock).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

fn load_scheduler_state_from_config() -> Result<(), String> {
    let path = scheduler_state_path()?;
    if !path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let data: HashMap<String, FeedScheduleState> =
        serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let mut lock = FEED_SCHEDULE.lock().unwrap();
    *lock = data;
    Ok(())
}

fn save_http_cache_to_config() -> Result<(), String> {
    let path = feed_http_cache_path()?;
    let lock = FEED_HTTP_CACHE.lock().unwrap();
    let json = serde_json::to_string_pretty(&*lock).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

fn load_http_cache_from_config() -> Result<(), String> {
    let path = feed_http_cache_path()?;
    if !path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let data: HashMap<String, FeedHttpCacheState> =
        serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let mut lock = FEED_HTTP_CACHE.lock().unwrap();
    *lock = data;
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

    if let Err(e) = load_scheduler_state_from_config() {
        warn!("flipboard: load scheduler state failed: {}", e);
    }
    if let Err(e) = load_http_cache_from_config() {
        warn!("flipboard: load http cache failed: {}", e);
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeedScheduleSnapshot {
    pub feed_id: String,
    pub next_run_at: i64,
    pub last_attempt_at: i64,
    pub last_success_at: i64,
    pub consecutive_failures: u32,
    pub last_error: Option<String>,
    pub last_article_count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SchedulerFeedReport {
    pub feed_id: String,
    pub feed_name: String,
    pub status: String,
    pub method: String,
    pub article_count: usize,
    pub next_run_at: i64,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SchedulerTickResult {
    pub run_at: i64,
    pub total_feeds: usize,
    pub due_feeds: usize,
    pub executed_feeds: usize,
    pub article_count: usize,
    pub reports: Vec<SchedulerFeedReport>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FeedHttpCacheState {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub last_fetched_at: i64,
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
        Category {
            id: "ai".to_string(),
            name: "AI".to_string(),
            icon: "Sparkles".to_string(),
            color: "#6366F1".to_string(),
        },
        Category {
            id: "finance".to_string(),
            name: "金融".to_string(),
            icon: "Cash".to_string(),
            color: "#14B8A6".to_string(),
        },
        Category {
            id: "medicine".to_string(),
            name: "医学".to_string(),
            icon: "Medkit".to_string(),
            color: "#EF4444".to_string(),
        },
        Category {
            id: "law".to_string(),
            name: "法律".to_string(),
            icon: "Scale".to_string(),
            color: "#A855F7".to_string(),
        },
        Category {
            id: "education".to_string(),
            name: "教育".to_string(),
            icon: "School".to_string(),
            color: "#F59E0B".to_string(),
        },
    ]
}

// 获取默认RSS源
fn get_default_feeds() -> Vec<FeedSource> {
    let platform_category_pairs: Vec<(&str, &str)> = vec![
        ("zhihu", "technology"),
        ("zhihu", "business"),
        ("zhihu", "finance"),
        ("zhihu", "ai"),
        ("zhihu", "computer"),
        ("zhihu", "education"),
        ("zhihu", "lifestyle"),
        ("bilibili", "all"),
        ("bilibili", "entertainment"),
        ("bilibili", "education"),
        ("quora", "technology"),
        ("quora", "business"),
        ("quora", "finance"),
        ("quora", "law"),
        ("quora", "education"),
        ("quora", "medicine"),
        ("xiaohongshu", "lifestyle"),
        ("xiaohongshu", "entertainment"),
        ("wechat", "technology"),
        ("wechat", "business"),
        ("wechat", "medicine"),
        ("wechat", "education"),
        ("arxiv", "ai"),
        ("arxiv", "computer"),
        ("arxiv", "math"),
        ("arxiv", "physics"),
        ("hackernews", "technology"),
        ("hackernews", "computer"),
        ("github_trending", "technology"),
        ("github_trending", "computer"),
        ("wikipedia", "all"),
        ("wikipedia", "math"),
        ("wikipedia", "physics"),
        ("wikipedia", "law"),
        ("nature", "medicine"),
        ("nature", "physics"),
        ("who", "medicine"),
        ("coursera", "education"),
    ];

    let mut default_feeds = Vec::new();

    for (platform, category) in platform_category_pairs {
        let templates = build_default_feeds_for_platform(platform);
        for (idx, t) in templates.into_iter().enumerate() {
            default_feeds.push(FeedSource {
                id: format!("builtin-{}-{}-{}", platform, category, idx),
                name: format!("{} ({})", t.name, category),
                url: t.url,
                category: category.to_string(),
                enabled: true,
                source_type: t.source_type,
                fetch_interval: Some(60),
                include_keywords: t.include_keywords,
                exclude_keywords: t.exclude_keywords,
                remark: t.remark,
            });
        }
    }

    default_feeds
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
        "arxiv" => vec![
            PlatformFeedTemplate {
                name: "arXiv - CS.AI".to_string(),
                url: "https://rss.arxiv.org/rss/cs.AI".to_string(),
                source_type: "rss".to_string(),
                include_keywords: None,
                exclude_keywords: None,
                remark: Some("自动生成的 arXiv 人工智能论文源".to_string()),
            },
            PlatformFeedTemplate {
                name: "arXiv - CS.LG".to_string(),
                url: "https://rss.arxiv.org/rss/cs.LG".to_string(),
                source_type: "rss".to_string(),
                include_keywords: None,
                exclude_keywords: None,
                remark: Some("自动生成的 arXiv 机器学习论文源".to_string()),
            },
        ],
        "hackernews" => vec![PlatformFeedTemplate {
            name: "Hacker News".to_string(),
            url: "https://hnrss.org/frontpage".to_string(),
            source_type: "rss".to_string(),
            include_keywords: None,
            exclude_keywords: None,
            remark: Some("自动生成的 Hacker News 前沿技术源".to_string()),
        }],
        "github_trending" => vec![PlatformFeedTemplate {
            name: "GitHub Trending".to_string(),
            url: "https://github.com/trending".to_string(),
            source_type: "spider".to_string(),
            include_keywords: None,
            exclude_keywords: None,
            remark: Some("自动生成的 GitHub 热门仓库网页探索源".to_string()),
        }],
        "wikipedia" => vec![PlatformFeedTemplate {
            name: "Wikipedia - Featured".to_string(),
            url: "https://en.wikipedia.org/wiki/Wikipedia:Featured_articles".to_string(),
            source_type: "spider".to_string(),
            include_keywords: None,
            exclude_keywords: None,
            remark: Some("自动生成的 Wikipedia 精选条目网页探索源".to_string()),
        }],
        "nature" => vec![PlatformFeedTemplate {
            name: "Nature - Latest Research".to_string(),
            url: "https://www.nature.com/nature/research-articles".to_string(),
            source_type: "spider".to_string(),
            include_keywords: None,
            exclude_keywords: None,
            remark: Some("自动生成的 Nature 最新研究网页探索源".to_string()),
        }],
        "who" => vec![PlatformFeedTemplate {
            name: "WHO - News".to_string(),
            url: "https://www.who.int/news".to_string(),
            source_type: "spider".to_string(),
            include_keywords: None,
            exclude_keywords: None,
            remark: Some("自动生成的 WHO 医学与公共卫生网页探索源".to_string()),
        }],
        "coursera" => vec![PlatformFeedTemplate {
            name: "Coursera Blog".to_string(),
            url: "https://blog.coursera.org/".to_string(),
            source_type: "spider".to_string(),
            include_keywords: None,
            exclude_keywords: None,
            remark: Some("自动生成的课程与学习趋势网页探索源".to_string()),
        }],
        "quora" => vec![PlatformFeedTemplate {
            name: "Quora - Discover".to_string(),
            url: "https://www.quora.com/".to_string(),
            source_type: "spider".to_string(),
            include_keywords: None,
            exclude_keywords: None,
            remark: Some("自动生成的 Quora 问题洞察网页探索源".to_string()),
        }],
        "xiaohongshu" => vec![PlatformFeedTemplate {
            name: "小红书 - 发现".to_string(),
            url: "https://www.xiaohongshu.com/explore".to_string(),
            source_type: "spider".to_string(),
            include_keywords: None,
            exclude_keywords: None,
            remark: Some("自动生成的小红书内容发现网页探索源".to_string()),
        }],
        "wechat" => vec![PlatformFeedTemplate {
            name: "微信公众号 - 搜索".to_string(),
            url: "https://weixin.sogou.com/".to_string(),
            source_type: "spider".to_string(),
            include_keywords: None,
            exclude_keywords: None,
            remark: Some("自动生成的微信公众号入口网页探索源".to_string()),
        }],
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
pub async fn get_article_detail(id: String, url: Option<String>) -> Result<Article, String> {
    let id_trim = id.trim().to_string();
    let url_trim = url
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let mut base_article = {
        let articles_lock = ARTICLES.lock().unwrap();
        articles_lock
            .iter()
            .find(|article| {
                article.id == id_trim
                    || url_trim
                        .as_deref()
                        .map(|u| article.url == *u)
                        .unwrap_or(false)
            })
            .cloned()
    };

    if base_article.is_none() {
        base_article = load_cached_article_detail(&id_trim, url_trim.as_deref())?;
    }

    let base_article = base_article.ok_or_else(|| "Article not found".to_string())?;

    {
        let mut lock = ARTICLES.lock().unwrap();
        if lock.iter().all(|a| a.id != base_article.id && a.url != base_article.url) {
            lock.push(base_article.clone());
        }
    };

    let content = base_article.content.trim();
    let desc = base_article.description.trim();
    let need_hydrate = content.is_empty() || content.len() < 180 || (!desc.is_empty() && content == desc);

    if !need_hydrate {
        return Ok(base_article);
    }

    match hydrate_article_readable_content(&base_article).await {
        Ok(enriched) => {
            let mut lock = ARTICLES.lock().unwrap();
            if let Some(slot) = lock.iter_mut().find(|a| a.id == id_trim || a.url == enriched.url) {
                *slot = enriched.clone();
            } else {
                lock.push(enriched.clone());
            }
            Ok(enriched)
        }
        Err(e) => {
            warn!(
                "flipboard: hydrate article detail failed, id = {}, url = {}, err = {}",
                id_trim, base_article.url, e
            );
            Ok(base_article)
        }
    }
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

    let source_type = {
        let st = input.source_type.trim().to_string();
        if st.is_empty() {
            infer_source_type_from_url(&url)
        } else {
            st
        }
    };
    let name = {
        let n = input.name.trim().to_string();
        if n.is_empty() {
            infer_feed_name_from_url(&url)
        } else {
            n
        }
    };

    let new_feed = FeedSource {
        id: format!("f{}", Utc::now().timestamp_millis()),
        name,
        url,
        category,
        enabled: input.enabled.unwrap_or(true),
        source_type,
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
pub async fn add_site(seed_url: String, category: String) -> Result<Vec<FeedSource>, String> {
    ensure_feeds_initialized();
    let base = Url::parse(seed_url.trim())
        .or_else(|_| Url::parse(&format!("https://{}", seed_url.trim())))
        .map_err(|e| format!("无效主站 URL: {}", e))?;
    let host = base
        .host_str()
        .ok_or_else(|| "无效主站 host".to_string())?
        .to_string();

    let mut discovered_feeds = Vec::new();
    match HTTP_CLIENT.get(base.clone()).send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(html) = resp.text().await {
                let categories = discover_site_category_feeds_from_html(&base, &html);
                for (idx, x) in categories.into_iter().enumerate() {
                    discovered_feeds.push(FeedSource {
                        id: format!("sd-{}-{}-{}", x.category, host.replace('.', "-"), idx),
                        name: format!("{} · {}", host, x.label),
                        url: x.url,
                        category: x.category,
                        enabled: true,
                        source_type: x.source_type,
                        fetch_interval: Some(60),
                        include_keywords: x.include_keywords,
                        exclude_keywords: None,
                        remark: Some(format!("site_category_discovery={}", base)),
                    });
                }
            }
        }
        Ok(resp) => {
            warn!(
                "flipboard: site_category_discovery request non-success, url={}, status={}",
                base,
                resp.status()
            );
        }
        Err(e) => {
            warn!("flipboard: site_category_discovery request failed, url={}, err={}", base, e);
        }
    }

    if discovered_feeds.is_empty() {
        let seed = SiteSeedInput {
            base_url: seed_url.clone(),
            category: category.trim().to_string(),
            include_keywords: None,
        };
        let discovered = build_site_discovery(seed)?;
        discovered_feeds.extend(discovered.feeds.into_iter().map(|b| FeedSource {
            id: format!("f{}", Utc::now().timestamp_millis()),
            name: b.name,
            url: b.url,
            category: category.trim().to_string(),
            enabled: true,
            source_type: b.source_type,
            fetch_interval: Some(60),
            include_keywords: b.include_keywords,
            exclude_keywords: None,
            remark: b.remark,
        }));
    }

    let mut feeds_lock = FEEDS.lock().unwrap();
    let mut existing_keys: HashSet<String> = feeds_lock
        .iter()
        .map(|f| {
            format!(
                "{}|{}",
                f.url.trim().to_lowercase(),
                f.category.trim().to_lowercase()
            )
        })
        .collect();

    let mut created = Vec::new();
    for feed in discovered_feeds {
        let key = format!(
            "{}|{}",
            feed.url.trim().to_lowercase(),
            feed.category.trim().to_lowercase()
        );
        if feed.url.trim().is_empty() || !existing_keys.insert(key) {
            continue;
        }
        feeds_lock.push(feed.clone());
        created.push(feed);
    }

    if let Err(e) = save_feeds_to_config(&feeds_lock) {
        warn!("flipboard: save_feeds_to_config failed after add_site: {}", e);
    }

    Ok(created)
}

#[command]
pub async fn import_feeds_from_website_markdown(path: Option<String>) -> Result<Vec<FeedSource>, String> {
    ensure_feeds_initialized();

    let path = path
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| website_markdown_candidates().into_iter().find(|p| p.exists()))
        .ok_or_else(|| "未找到 website markdown 文件".to_string())?;

    let records = load_feed_records_from_website_markdown(&path)?;
    if records.is_empty() {
        return Ok(Vec::new());
    }

    let mut feeds_lock = FEEDS.lock().unwrap();
    let mut existing_keys: HashSet<String> = feeds_lock
        .iter()
        .map(|f| format!("{}|{}", f.url.trim().to_lowercase(), f.category.trim().to_lowercase()))
        .collect();

    let mut created = Vec::new();
    for (idx, r) in records.into_iter().enumerate() {
        let key = format!(
            "{}|{}",
            r.feed_url.trim().to_lowercase(),
            r.category.trim().to_lowercase()
        );
        if !existing_keys.insert(key) {
            continue;
        }
        let host = Url::parse(&r.base_url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_else(|| "site".to_string());
        let source_type = infer_source_type_from_url(&r.feed_url);
        let feed = FeedSource {
            id: format!("md-{}-{}-{}", r.category, host.replace('.', "-"), idx),
            name: r.site_name,
            url: r.feed_url,
            category: r.category,
            enabled: true,
            source_type,
            fetch_interval: Some(60),
            include_keywords: None,
            exclude_keywords: None,
            remark: Some(format!("imported_from={}", path.display())),
        };
        feeds_lock.push(feed.clone());
        created.push(feed);
    }

    if !created.is_empty() {
        let _ = save_feeds_to_config(&feeds_lock);
    }
    Ok(created)
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

fn score_article_for_topic(article: &Article, topic_tokens: &[String]) -> usize {
    if topic_tokens.is_empty() {
        return 0;
    }

    let title = article.title.to_lowercase();
    let description = article.description.to_lowercase();
    let content = article.content.to_lowercase();
    let source = article.source.to_lowercase();

    topic_tokens
        .iter()
        .map(|token| {
            let t = token.as_str();
            let mut score = 0;
            score += title.matches(t).count() * 4;
            score += description.matches(t).count() * 3;
            score += content.matches(t).count();
            score += source.matches(t).count() * 2;
            score
        })
        .sum()
}

fn normalize_category_filter(category: &Option<String>) -> Option<String> {
    category
        .as_ref()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty() && c != "all")
}

fn classify_article_category(feed: &FeedSource, title: &str, description: &str, article_url: &str) -> String {
    let base = if feed.category.trim().is_empty() {
        "all".to_string()
    } else {
        feed.category.trim().to_string()
    };
    classify_category_by_text_and_url(&base, title, description, article_url, &feed.name)
}

fn select_enabled_feeds_by_category(feeds: &[FeedSource], category: &Option<String>) -> Vec<FeedSource> {
    let normalized = normalize_category_filter(category);
    match normalized {
        Some(ref cat) => feeds
            .iter()
            .filter(|f| f.enabled && (f.category == *cat || f.category == "all"))
            .cloned()
            .collect(),
        None => feeds.iter().filter(|f| f.enabled).cloned().collect(),
    }
}

fn dedup_articles_by_identity(mut articles: Vec<Article>) -> Vec<Article> {
    let mut seen = HashSet::new();
    articles.retain(|a| {
        let key = format!(
            "{}|{}",
            a.url.trim().to_lowercase(),
            a.title.trim().to_lowercase()
        );
        seen.insert(key)
    });
    articles
}

fn merge_incremental_articles(existing: Vec<Article>, incoming: Vec<Article>, limit: Option<usize>) -> Vec<Article> {
    let mut merged = existing;
    merged.extend(incoming);
    merged = dedup_articles_by_identity(merged);
    merged.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    if let Some(l) = limit {
        merged.truncate(l);
    }
    merged
}

fn read_scheduler_state(feed_id: &str) -> Option<FeedScheduleState> {
    let lock = FEED_SCHEDULE.lock().unwrap();
    lock.get(feed_id).cloned()
}

fn write_scheduler_state(feed_id: &str, state: FeedScheduleState) {
    let mut lock = FEED_SCHEDULE.lock().unwrap();
    lock.insert(feed_id.to_string(), state);
    drop(lock);
    let _ = save_scheduler_state_to_config();
}

fn read_http_cache(key: &str) -> FeedHttpCacheState {
    let lock = FEED_HTTP_CACHE.lock().unwrap();
    lock.get(key).cloned().unwrap_or_default()
}

fn write_http_cache(key: &str, state: FeedHttpCacheState) {
    let mut lock = FEED_HTTP_CACHE.lock().unwrap();
    lock.insert(key.to_string(), state);
    drop(lock);
    let _ = save_http_cache_to_config();
}

fn topic_tokens_from_text(topic: &str) -> Vec<String> {
    topic
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(|x| x.trim().to_lowercase())
        .filter(|x| !x.is_empty())
        .collect()
}

fn score_article_for_preference(article: &Article, tokens: &[String], preferred_sources: &[String]) -> usize {
    let mut score = score_article_for_topic(article, tokens);
    if !preferred_sources.is_empty() {
        let source = article.source.to_lowercase();
        for s in preferred_sources {
            let needle = s.trim().to_lowercase();
            if !needle.is_empty() && source.contains(&needle) {
                score += 10;
            }
        }
    }
    if article.image_url.is_some() {
        score += 2;
    }
    score
}

fn aggregate_articles_for_note(
    articles: &[Article],
    topic: &str,
    preferred_sources: &[String],
    max_items: usize,
) -> Vec<Article> {
    let tokens = topic_tokens_from_text(topic);
    let mut list: Vec<Article> = dedup_articles_by_identity(articles.to_vec());
    list.sort_by(|a, b| {
        score_article_for_preference(b, &tokens, preferred_sources)
            .cmp(&score_article_for_preference(a, &tokens, preferred_sources))
            .then_with(|| b.published_at.cmp(&a.published_at))
    });
    list.truncate(max_items.max(1));
    list
}

fn infer_source_type_from_url(url: &str) -> String {
    let u = url.to_lowercase();
    if u.contains("zhihu.com") {
        return "zhihu".to_string();
    }
    if u.contains("bilibili.com") {
        return "bilibili".to_string();
    }
    if u.contains("/rss") || u.contains("/feed") || u.ends_with(".xml") || u.contains("atom") {
        return "rss".to_string();
    }
    "spider".to_string()
}

fn infer_feed_name_from_url(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string()))
        .unwrap_or_else(|| "知识源".to_string())
}

fn trim_to_chars(text: &str, max_chars: usize) -> String {
    let mut s = String::new();
    for (i, ch) in text.chars().enumerate() {
        if i >= max_chars {
            break;
        }
        s.push(ch);
    }
    s
}

fn sanitize_file_stem(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else if ch.is_whitespace() {
            out.push('_');
        } else {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "note".to_string()
    } else {
        out
    }
}

fn build_kb_note_path(kb_name: &str, file_stem: &str) -> Result<PathBuf, String> {
    let root = crate::commands::settingview::get_knowledge_bases_root();
    let root_dir = PathBuf::from(root);
    let kb_dir = root_dir.join(kb_name);
    if !kb_dir.exists() {
        return Err("知识库不存在或路径无效".to_string());
    }
    let notes_dir = kb_dir.join("Collected");
    if !notes_dir.exists() {
        fs::create_dir_all(&notes_dir).map_err(|e| e.to_string())?;
        let _ = crate::commands::kbmeta::after_create_path(&notes_dir, true, None);
    }
    Ok(notes_dir.join(format!("{}.md", file_stem)))
}

fn write_kb_markdown_note(path: &PathBuf, content: &str) -> Result<String, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, content).map_err(|e| e.to_string())?;
    let _ = crate::commands::kbmeta::after_create_path(path, false, Some(content));
    let _ = crate::commands::kbmeta::after_write_file(path, content);
    Ok(path.to_string_lossy().to_string())
}

fn resolve_url(base: &Url, raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() || value.starts_with("javascript:") || value.starts_with("data:") {
        return None;
    }
    Url::options()
        .base_url(Some(base))
        .parse(value)
        .ok()
        .and_then(|u| {
            if u.scheme().starts_with("http") {
                Some(u.to_string())
            } else {
                None
            }
        })
}

fn extract_meta_content(document: &Html, selector_text: &str) -> Option<String> {
    let selector = Selector::parse(selector_text).ok()?;
    document
        .select(&selector)
        .next()
        .and_then(|el| el.value().attr("content"))
        .map(compact_plain_text)
        .filter(|v| !v.is_empty())
}

fn looks_like_media_link(url: &str) -> bool {
    let u = url.to_lowercase();
    u.contains("youtube.com")
        || u.contains("youtu.be")
        || u.contains("bilibili.com")
        || u.contains("vimeo.com")
        || u.contains("xiaohongshu.com")
        || u.contains("zhihu.com/video")
        || u.contains("podcast")
        || u.ends_with(".mp4")
        || u.ends_with(".mp3")
        || u.ends_with(".m3u8")
        || u.ends_with(".webm")
}

async fn hydrate_article_readable_content(article: &Article) -> Result<Article, String> {
    let parsed = Url::parse(article.url.trim()).map_err(|e| e.to_string())?;
    let resp = HTTP_CLIENT.get(parsed.clone()).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("fetch article html failed: {}", resp.status()));
    }
    let html = resp.text().await.map_err(|e| e.to_string())?;
    let document = Html::parse_document(&html);

    let page_title = Selector::parse("h1")
        .ok()
        .and_then(|sel| {
            document
                .select(&sel)
                .next()
                .map(|el| compact_plain_text(&el.text().collect::<String>()))
        })
        .filter(|v| !v.is_empty());

    let lead = article
        .description
        .trim()
        .to_string()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");
    let lead = if lead.is_empty() {
        extract_meta_content(&document, "meta[property='og:description']")
            .or_else(|| extract_meta_content(&document, "meta[name='description']"))
            .unwrap_or_default()
    } else {
        lead
    };

    let text_selectors = site_readability_paragraph_selectors(article.url.as_str());
    let mut paragraphs = Vec::new();
    let mut text_seen = HashSet::new();
    for sel_text in &text_selectors {
        let selector = match Selector::parse(sel_text) {
            Ok(s) => s,
            Err(_) => continue,
        };
        for el in document.select(&selector) {
            let text = compact_plain_text(&el.text().collect::<String>());
            if is_noise_paragraph(&text) {
                continue;
            }
            let key = text.to_lowercase();
            if !text_seen.insert(key) {
                continue;
            }
            paragraphs.push(text);
            if paragraphs.len() >= 24 {
                break;
            }
        }
        if paragraphs.len() >= 24 {
            break;
        }
    }

    let mut image_links = Vec::new();
    let mut image_seen = HashSet::new();
    for img_sel_text in &site_readability_image_selectors(article.url.as_str()) {
        if let Ok(img_sel) = Selector::parse(img_sel_text) {
            for img in document.select(&img_sel) {
                let src = img
                    .value()
                    .attr("src")
                    .or_else(|| img.value().attr("data-src"))
                    .or_else(|| img.value().attr("data-original"))
                    .unwrap_or("");
                if let Some(abs) = resolve_url(&parsed, src) {
                    if image_seen.insert(abs.clone()) {
                        image_links.push(abs);
                    }
                }
                if image_links.len() >= 6 {
                    break;
                }
            }
            if image_links.len() >= 6 {
                break;
            }
        }
    }

    let mut media_links = Vec::new();
    let mut media_seen = HashSet::new();
    if let Ok(a_sel) = Selector::parse("a[href]") {
        for a in document.select(&a_sel) {
            let href = a.value().attr("href").unwrap_or("");
            if let Some(abs) = resolve_url(&parsed, href) {
                if looks_like_media_link(&abs) && media_seen.insert(abs.clone()) {
                    media_links.push(abs);
                }
            }
            if media_links.len() >= 8 {
                break;
            }
        }
    }

    if let Ok(iframe_sel) = Selector::parse("iframe[src], video[src], source[src]") {
        for node in document.select(&iframe_sel) {
            let src = node.value().attr("src").unwrap_or("");
            if let Some(abs) = resolve_url(&parsed, src) {
                if looks_like_media_link(&abs) && media_seen.insert(abs.clone()) {
                    media_links.push(abs);
                }
            }
            if media_links.len() >= 8 {
                break;
            }
        }
    }

    let content = assemble_readable_article(&lead, &paragraphs, &image_links, &media_links, &article.url);
    let desc = if !lead.is_empty() {
        lead
    } else if !content.is_empty() {
        trim_to_chars(&content, 180)
    } else {
        article.description.clone()
    };

    let mut enriched = article.clone();
    if let Some(t) = page_title {
        if !t.is_empty() && (enriched.title == "无标题" || enriched.title.trim().len() < 4) {
            enriched.title = t;
        }
    }
    enriched.description = desc;
    enriched.content = content;
    enriched.category = classify_category_by_text_and_url(
        &enriched.category,
        &enriched.title,
        &enriched.description,
        &enriched.url,
        &enriched.source,
    );
    if enriched.image_url.is_none() {
        enriched.image_url = image_links.first().cloned();
    }
    Ok(enriched)
}

#[command]
pub async fn fetch_knowledge_from_feed_ids(
    feed_ids: Vec<String>,
    topic: Option<String>,
    limit: Option<usize>,
    force: Option<bool>,
) -> Result<Vec<Article>, String> {
    ensure_feeds_initialized();

    let feed_id_set: HashSet<String> = feed_ids
        .into_iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();

    if feed_id_set.is_empty() {
        return Ok(Vec::new());
    }

    let feeds: Vec<FeedSource> = {
        let feeds_lock = FEEDS.lock().unwrap();
        feeds_lock
            .iter()
            .filter(|f| f.enabled && feed_id_set.contains(&f.id))
            .cloned()
            .collect()
    };

    let topic_text = topic.unwrap_or_default().trim().to_string();
    let force = force.unwrap_or(false);
    let max_limit = limit.unwrap_or(80).max(1);
    let topic_tokens: Vec<String> = topic_text
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(|x| x.trim().to_lowercase())
        .filter(|x| !x.is_empty())
        .collect();

    let feed_id_list: Vec<String> = feeds.iter().map(|f| f.id.clone()).collect();
    let mut out = load_cached_articles_by_feed_ids(&feed_id_list, None, max_limit).unwrap_or_default();
    let now = unix_now();

    for feed in &feeds {
        if !force {
            let has_cache = has_cached_articles_for_feed(&feed.id).unwrap_or(false);
            let due = should_run_feed(read_scheduler_state(&feed.id).as_ref(), now, false);
            if has_cache && !due {
                continue;
            }
        }

        let mut selected_feed = feed.clone();
        if !topic_text.is_empty() {
            selected_feed.include_keywords =
                Some(match selected_feed.include_keywords.as_deref() {
                    Some(existing) if !existing.trim().is_empty() => {
                        format!("{},{}", existing.trim(), topic_text)
                    }
                    _ => topic_text.clone(),
                });
        }

        match collect_articles_for_feed(&selected_feed).await {
            Ok(mut list) => {
                let _ = save_articles_to_cache(&feed.id, &list);
                out.append(&mut list)
            }
            Err(e) => warn!(
                "flipboard: fetch_knowledge_from_feed_ids failed, feed={}, err={}",
                feed.url, e
            ),
        }
    }

    out = dedup_articles_by_identity(out);

    if !topic_tokens.is_empty() {
        out.retain(|a| score_article_for_topic(a, &topic_tokens) > 0);
        out.sort_by(|a, b| {
            score_article_for_topic(b, &topic_tokens)
                .cmp(&score_article_for_topic(a, &topic_tokens))
                .then_with(|| b.published_at.cmp(&a.published_at))
        });
    } else {
        out.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    }

    out.truncate(max_limit);

    {
        let mut lock = ARTICLES.lock().unwrap();
        let mut merged = lock.clone();
        merged.extend(out.clone());
        *lock = dedup_articles_by_identity(merged);
    }

    Ok(out)
}

#[command]
pub async fn get_cached_articles(
    category: Option<String>,
    topic: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<Article>, String> {
    ensure_feeds_initialized();
    let max_limit = limit.unwrap_or(80).max(1);
    let category_ref = category.as_deref();
    let topic_ref = topic.as_deref();
    let list = load_cached_articles(category_ref, topic_ref, max_limit)?;
    {
        let mut lock = ARTICLES.lock().unwrap();
        let mut merged = lock.clone();
        merged.extend(list.clone());
        *lock = dedup_articles_by_identity(merged);
    }
    Ok(list)
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

    let cache_key = format!("{}|rss|{}", feed.id, parsed);
    let cache_state = read_http_cache(&cache_key);
    let mut req = HTTP_CLIENT.get(parsed.clone());
    if let Some(etag) = cache_state.etag.as_deref() {
        req = req.header(IF_NONE_MATCH, etag);
    }
    if let Some(last_modified) = cache_state.last_modified.as_deref() {
        req = req.header(IF_MODIFIED_SINCE, last_modified);
    }

    let resp = req.send().await.map_err(|e| {
        error!("flipboard: request feed {} failed: {}", parsed, e);
        e.to_string()
    })?;

    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        let mut state = read_http_cache(&cache_key);
        state.last_fetched_at = unix_now();
        write_http_cache(&cache_key, state);
        info!("flipboard: feed not modified: {}", parsed);
        return Ok(Vec::new());
    }

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!(
            "flipboard: feed {} returned non-success: {} {}",
            parsed, status, body
        );
        return Err(format!("feed {} returned status {}", parsed, status));
    }

    let etag = resp
        .headers()
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let last_modified = resp
        .headers()
        .get(LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

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
            category: classify_article_category(feed, &title, &description, &link),
            title,
            description,
            content,
            source: feed.name.clone(),
            url: link,
            image_url: None,
            published_at,
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

    write_http_cache(
        &cache_key,
        FeedHttpCacheState {
            etag,
            last_modified,
            last_fetched_at: unix_now(),
        },
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

    let cache_key = format!("{}|html|{}", feed.id, parsed);
    let cache_state = read_http_cache(&cache_key);
    let mut req = HTTP_CLIENT.get(parsed.clone());
    if let Some(etag) = cache_state.etag.as_deref() {
        req = req.header(IF_NONE_MATCH, etag);
    }
    if let Some(last_modified) = cache_state.last_modified.as_deref() {
        req = req.header(IF_MODIFIED_SINCE, last_modified);
    }

    let resp = req.send().await.map_err(|e| {
        error!("flipboard: request html {} failed: {}", parsed, e);
        e.to_string()
    })?;

    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        let mut state = read_http_cache(&cache_key);
        state.last_fetched_at = unix_now();
        write_http_cache(&cache_key, state);
        info!("flipboard: html not modified: {}", parsed);
        return Ok(Vec::new());
    }

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!(
            "flipboard: html {} returned non-success: {} {}",
            parsed, status, body
        );
        return Err(format!("html {} returned status {}", parsed, status));
    }

    let etag = resp
        .headers()
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let last_modified = resp
        .headers()
        .get(LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

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
                category: classify_article_category(feed, &title, "", &link),
                title,
                description: String::new(),
                content: String::new(),
                source: feed.name.clone(),
                url: link,
                image_url: None,
                published_at: now.clone(),
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

    write_http_cache(
        &cache_key,
        FeedHttpCacheState {
            etag,
            last_modified,
            last_fetched_at: unix_now(),
        },
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
            category: classify_article_category(feed, &title, &desc, &link),
            title,
            description: desc.clone(),
            content: desc,
            source: feed.name.clone(),
            url: link,
            image_url: None,
            published_at: now.clone(),
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
            category: classify_article_category(feed, &title, &desc, &link),
            title,
            description: desc.clone(),
            content: desc,
            source: feed.name.clone(),
            url: link,
            image_url,
            published_at: now.clone(),
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

    let cache_key = format!("json|{}", url);
    let cache_state = read_http_cache(&cache_key);
    let mut req = HTTP_CLIENT.get(url.clone());
    if let Some(etag) = cache_state.etag.as_deref() {
        req = req.header(IF_NONE_MATCH, etag);
    }
    if let Some(last_modified) = cache_state.last_modified.as_deref() {
        req = req.header(IF_MODIFIED_SINCE, last_modified);
    }

    let resp = req.send().await.map_err(|e| {
        error!("flipboard: request json {} failed: {}", url, e);
        e.to_string()
    })?;

    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Err("not_modified".to_string());
    }

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!("flipboard: json {} returned non-success: {} {}", url, status, body);
        return Err(format!("json {} returned status {}", url, status));
    }

    let etag = resp
        .headers()
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let last_modified = resp
        .headers()
        .get(LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let text = resp.text().await.map_err(|e| {
        error!("flipboard: read json {} body failed: {}", url, e);
        e.to_string()
    })?;

    let value = serde_json::from_str::<serde_json::Value>(&text).map_err(|e| {
        error!("flipboard: parse json {} failed: {}", url, e);
        e.to_string()
    })?;
    write_http_cache(
        &cache_key,
        FeedHttpCacheState {
            etag,
            last_modified,
            last_fetched_at: unix_now(),
        },
    );
    Ok(value)
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
            category: classify_article_category(feed, &title, &desc, &link),
            title,
            description: desc.clone(),
            content: desc,
            source: feed.name.clone(),
            url: link,
            image_url: None,
            published_at: now.clone(),
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
            category: classify_article_category(feed, &title, &desc, &link),
            title,
            description: desc.clone(),
            content: desc,
            source: feed.name.clone(),
            url: link,
            image_url,
            published_at: now.clone(),
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
            Err(e) => {
                if e == "not_modified" {
                    return Ok(CrawlResult {
                        method: "not_modified",
                        articles: Vec::new(),
                    });
                }
                warn!("flipboard: zhihu api url request failed, url = {}, err = {}", feed.url, e)
            }
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
            Err(e) => {
                if e == "not_modified" {
                    return Ok(CrawlResult {
                        method: "not_modified",
                        articles: Vec::new(),
                    });
                }
                warn!("flipboard: bilibili api url request failed, url = {}, err = {}", feed.url, e)
            }
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
        "spider" => {
            return match fetch_html_and_extract_articles(feed).await {
                Ok(list) => {
                    let out = filter_articles_by_keywords(list, feed);
                    record_feed_health_success(feed, "spider_html", out.len());
                    Ok(out)
                }
                Err(e) => {
                    record_feed_health_failure(feed, "spider_html", e.clone());
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
pub async fn get_feed_schedule_status() -> Result<Vec<FeedScheduleSnapshot>, String> {
    ensure_feeds_initialized();
    let feeds: Vec<FeedSource> = {
        let lock = FEEDS.lock().unwrap();
        lock.clone()
    };
    let schedule = FEED_SCHEDULE.lock().unwrap();
    let mut out = Vec::new();
    for f in feeds {
        let s = schedule
            .get(&f.id)
            .cloned()
            .unwrap_or_default();
        out.push(FeedScheduleSnapshot {
            feed_id: f.id.clone(),
            next_run_at: s.next_run_at,
            last_attempt_at: s.last_attempt_at,
            last_success_at: s.last_success_at,
            consecutive_failures: s.consecutive_failures,
            last_error: s.last_error,
            last_article_count: s.last_article_count,
        });
    }
    Ok(out)
}

#[command]
pub async fn run_feed_scheduler_tick(
    category: Option<String>,
    topic: Option<String>,
    force: Option<bool>,
    limit: Option<usize>,
) -> Result<SchedulerTickResult, String> {
    ensure_feeds_initialized();
    let force = force.unwrap_or(false);
    let now = unix_now();
    if let Err(e) = cleanup_old_cached_articles_if_due(now) {
        warn!("flipboard cache cleanup failed: {}", e);
    }
    let policy = default_scheduler_policy();
    let all_feeds: Vec<FeedSource> = {
        let lock = FEEDS.lock().unwrap();
        lock.clone()
    };
    let feeds = select_enabled_feeds_by_category(&all_feeds, &category);
    if feeds.is_empty() {
        return Ok(SchedulerTickResult {
            run_at: now,
            total_feeds: 0,
            due_feeds: 0,
            executed_feeds: 0,
            article_count: 0,
            reports: Vec::new(),
        });
    }

    let mut due = Vec::new();
    let mut reports = Vec::new();
    for f in feeds {
        let state = read_scheduler_state(&f.id);
        if should_run_feed(state.as_ref(), now, force) {
            due.push(f);
        } else if let Some(s) = state {
            reports.push(SchedulerFeedReport {
                feed_id: f.id.clone(),
                feed_name: f.name.clone(),
                status: "skipped".to_string(),
                method: "scheduler".to_string(),
                article_count: 0,
                next_run_at: s.next_run_at,
                error: None,
            });
        }
    }

    let due_count = due.len();
    let sem = std::sync::Arc::new(Semaphore::new(policy.max_concurrency.max(1)));
    let topic_text = topic.unwrap_or_default();
    let mut handles = Vec::new();
    for feed in due {
        let sem = sem.clone();
        let t = topic_text.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.map_err(|e| e.to_string())?;
            let mut effective_feed = feed.clone();
            if !t.trim().is_empty() {
                effective_feed.include_keywords = Some(match effective_feed.include_keywords.as_deref() {
                    Some(existing) if !existing.trim().is_empty() => format!("{},{}", existing.trim(), t),
                    _ => t.clone(),
                });
            }
            let res = collect_articles_for_feed(&effective_feed).await;
            Ok::<(FeedSource, Result<Vec<Article>, String>), String>((feed, res))
        }));
    }

    let mut new_articles = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok((feed, Ok(list)))) => {
                let fingerprint = {
                    let tuples: Vec<(&str, &str)> = list
                        .iter()
                        .take(30)
                        .map(|a| (a.url.as_str(), a.title.as_str()))
                        .collect();
                    Some(articles_fingerprint(&tuples))
                };
                let mut s = read_scheduler_state(&feed.id).unwrap_or_default();
                let interval = feed_interval_secs(feed.fetch_interval);
                if list.is_empty() {
                    update_schedule_after_success(&mut s, now, interval, 0, fingerprint);
                } else {
                    update_schedule_after_success(&mut s, now, interval, list.len(), fingerprint);
                    let _ = save_articles_to_cache(&feed.id, &list);
                    new_articles.extend(list);
                }
                let next_run_at = s.next_run_at;
                let error = s.last_error.clone();
                let count = s.last_article_count;
                write_scheduler_state(&feed.id, s);
                reports.push(SchedulerFeedReport {
                    feed_id: feed.id.clone(),
                    feed_name: feed.name.clone(),
                    status: if count > 0 { "success" } else { "empty" }.to_string(),
                    method: "scheduled".to_string(),
                    article_count: count,
                    next_run_at,
                    error,
                });
            }
            Ok(Ok((feed, Err(e)))) => {
                let mut s = read_scheduler_state(&feed.id).unwrap_or_default();
                update_schedule_after_failure(&mut s, now, policy.max_backoff_secs, e.clone());
                let next_run_at = s.next_run_at;
                write_scheduler_state(&feed.id, s);
                reports.push(SchedulerFeedReport {
                    feed_id: feed.id.clone(),
                    feed_name: feed.name.clone(),
                    status: "failed".to_string(),
                    method: "scheduled".to_string(),
                    article_count: 0,
                    next_run_at,
                    error: Some(e),
                });
            }
            Ok(Err(e)) => {
                let err = e.to_string();
                reports.push(SchedulerFeedReport {
                    feed_id: "unknown".to_string(),
                    feed_name: "unknown".to_string(),
                    status: "failed".to_string(),
                    method: "scheduled".to_string(),
                    article_count: 0,
                    next_run_at: now + 60,
                    error: Some(err),
                });
            }
            Err(e) => {
                let err = e.to_string();
                reports.push(SchedulerFeedReport {
                    feed_id: "unknown".to_string(),
                    feed_name: "unknown".to_string(),
                    status: "failed".to_string(),
                    method: "scheduled".to_string(),
                    article_count: 0,
                    next_run_at: now + 60,
                    error: Some(err),
                });
            }
        }
    }

    let merged = {
        let existing = {
            let lock = ARTICLES.lock().unwrap();
            lock.clone()
        };
        merge_incremental_articles(existing, new_articles, limit)
    };
    {
        let mut lock = ARTICLES.lock().unwrap();
        *lock = merged.clone();
    }

    Ok(SchedulerTickResult {
        run_at: now,
        total_feeds: reports.len(),
        due_feeds: due_count,
        executed_feeds: reports
            .iter()
            .filter(|r| r.status == "success" || r.status == "empty" || r.status == "failed")
            .count(),
        article_count: merged.len(),
        reports,
    })
}

pub fn start_feed_scheduler_loop() {
    let mut started = SCHEDULER_LOOP_STARTED.lock().unwrap();
    if *started {
        return;
    }
    *started = true;

    tokio::spawn(async move {
        loop {
            if !SCHEDULER_ENABLED.load(Ordering::Relaxed) {
                sleep(Duration::from_secs(10)).await;
                continue;
            }
            let _ = run_feed_scheduler_tick(None, None, Some(false), Some(120)).await;
            sleep(Duration::from_secs(15)).await;
        }
    });
}

#[command]
pub async fn set_feed_scheduler_enabled(enabled: bool) -> Result<bool, String> {
    SCHEDULER_ENABLED.store(enabled, Ordering::Relaxed);
    Ok(enabled)
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
    ensure_feeds_initialized();

    let max_limit = limit.unwrap_or(80).max(1);
    let cached = load_cached_articles(category.as_deref(), None, max_limit).unwrap_or_default();
    if cached.len() >= max_limit.min(40) {
        return Ok(cached);
    }

    let all_feeds: Vec<FeedSource> = {
        let feeds_lock = FEEDS.lock().unwrap();
        feeds_lock.clone()
    };
    let feeds = select_enabled_feeds_by_category(&all_feeds, &category);

    if feeds.is_empty() {
        warn!("flipboard: no enabled feeds, fallback to mock articles");
        let mut articles = generate_mock_articles();
        if let Some(ref cat) = normalize_category_filter(&category) {
            articles.retain(|a| &a.category == cat);
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

    if let Some(ref cat) = normalize_category_filter(&category) {
        all_articles.retain(|a| &a.category == cat);
    }

    all_articles = dedup_articles_by_identity(all_articles);
    all_articles.sort_by(|a, b| b.published_at.cmp(&a.published_at));

    all_articles.truncate(max_limit);

    if !all_articles.is_empty() {
        let _ = save_articles_to_cache("all", &all_articles);
    }

    let mut lock = ARTICLES.lock().unwrap();
    *lock = all_articles.clone();

    info!(
        "flipboard: refresh_articles_from_feeds done, article_count = {}",
        all_articles.len()
    );

    Ok(all_articles)
}

#[command]
pub async fn refresh_topic_knowledge_from_feeds(
    topic: String,
    category: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<Article>, String> {
    let topic = topic.trim().to_string();
    if topic.is_empty() {
        return refresh_articles_from_feeds(category, limit).await;
    }

    ensure_feeds_initialized();

    let max_limit = limit.unwrap_or(80).max(1);
    let cached = load_cached_articles(category.as_deref(), Some(topic.as_str()), max_limit).unwrap_or_default();
    if cached.len() >= max_limit.min(40) {
        return Ok(cached);
    }

    let all_feeds: Vec<FeedSource> = {
        let feeds_lock = FEEDS.lock().unwrap();
        feeds_lock.clone()
    };
    let feeds = select_enabled_feeds_by_category(&all_feeds, &category);

    let topic_tokens: Vec<String> = topic
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(|x| x.trim().to_lowercase())
        .filter(|x| !x.is_empty())
        .collect();

    if feeds.is_empty() {
        let mut articles = generate_mock_articles();
        if let Some(ref cat) = normalize_category_filter(&category) {
            articles.retain(|a| &a.category == cat);
        }
        if !topic_tokens.is_empty() {
            articles.retain(|a| score_article_for_topic(a, &topic_tokens) > 0);
            articles.sort_by(|a, b| {
                score_article_for_topic(b, &topic_tokens)
                    .cmp(&score_article_for_topic(a, &topic_tokens))
                    .then_with(|| b.published_at.cmp(&a.published_at))
            });
        }
        if let Some(limit) = limit {
            articles.truncate(limit);
        }
        let mut lock = ARTICLES.lock().unwrap();
        *lock = articles.clone();
        return Ok(articles);
    }

    info!(
        "flipboard: refresh_topic_knowledge_from_feeds start, topic = {}, feed_count = {}",
        topic,
        feeds.len()
    );

    let mut all_articles = Vec::new();

    for feed in &feeds {
        let mut topic_feed = feed.clone();
        topic_feed.include_keywords = Some(match topic_feed.include_keywords.as_deref() {
            Some(existing) if !existing.trim().is_empty() => {
                format!("{},{}", existing.trim(), topic)
            }
            _ => topic.clone(),
        });

        match collect_articles_for_feed(&topic_feed).await {
            Ok(mut list) => {
                all_articles.append(&mut list);
            }
            Err(e) => {
                warn!(
                    "flipboard: collect topic feed {} failed, topic = {}, err = {}",
                    feed.url, topic, e
                );
            }
        }
    }

    if let Some(ref cat) = normalize_category_filter(&category) {
        all_articles.retain(|a| &a.category == cat);
    }

    all_articles = dedup_articles_by_identity(all_articles);
    if !topic_tokens.is_empty() {
        all_articles.retain(|a| score_article_for_topic(a, &topic_tokens) > 0);
        all_articles.sort_by(|a, b| {
            score_article_for_topic(b, &topic_tokens)
                .cmp(&score_article_for_topic(a, &topic_tokens))
                .then_with(|| b.published_at.cmp(&a.published_at))
        });
    } else {
        all_articles.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    }

    all_articles.truncate(max_limit);

    let mut lock = ARTICLES.lock().unwrap();
    *lock = all_articles.clone();

    info!(
        "flipboard: refresh_topic_knowledge_from_feeds done, topic = {}, article_count = {}",
        topic,
        all_articles.len()
    );

    Ok(all_articles)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneratedNoteResult {
    pub file_path: String,
    pub item_count: usize,
}

#[command]
pub async fn create_kb_note_from_topic(
    kb_name: String,
    topic: String,
    category: Option<String>,
    max_items: Option<usize>,
    preferred_sources: Option<Vec<String>>,
    hydrate: Option<bool>,
) -> Result<GeneratedNoteResult, String> {
    let kb_name = kb_name.trim().to_string();
    if kb_name.is_empty() {
        return Err("kb_name 不能为空".to_string());
    }
    let topic = topic.trim().to_string();
    if topic.is_empty() {
        return Err("topic 不能为空".to_string());
    }

    let hydrate = hydrate.unwrap_or(false);
    let max_items = max_items.unwrap_or(8).max(1).min(20);
    let preferred_sources = preferred_sources.unwrap_or_default();

    let mut items: Vec<Article> = {
        let lock = ARTICLES.lock().unwrap();
        lock.clone()
    };
    if items.is_empty() {
        items = refresh_topic_knowledge_from_feeds(topic.clone(), category.clone(), Some(80)).await?;
    }
    if let Some(ref cat) = normalize_category_filter(&category) {
        items.retain(|a| &a.category == cat);
    }

    let mut selected = aggregate_articles_for_note(&items, &topic, &preferred_sources, max_items);
    if hydrate {
        let mut hydrated_list = Vec::new();
        for a in selected {
            let base = if a.content.trim().is_empty() || a.content.trim().len() < 180 {
                match hydrate_article_readable_content(&a).await {
                    Ok(x) => x,
                    Err(_) => a,
                }
            } else {
                a
            };
            hydrated_list.push(base);
        }
        selected = hydrated_list;
    }

    let mut sources: Vec<String> = selected
        .iter()
        .map(|a| a.source.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    sources.sort();
    sources.dedup();

    let mut md = String::new();
    md.push_str("# ");
    md.push_str(&topic);
    md.push_str("\n\n");
    md.push_str("生成时间：");
    md.push_str(&Utc::now().to_rfc3339());
    md.push_str("\n\n");
    if !sources.is_empty() {
        md.push_str("来源：\n");
        for s in &sources {
            md.push_str("- ");
            md.push_str(s);
            md.push_str("\n");
        }
        md.push_str("\n");
    }

    for (idx, a) in selected.iter().enumerate() {
        if idx > 0 {
            md.push_str("\n---\n\n");
        }
        md.push_str("## ");
        md.push_str(a.title.trim());
        md.push_str("\n\n");
        if !a.source.trim().is_empty() {
            md.push_str("来源：");
            md.push_str(a.source.trim());
            md.push_str("\n\n");
        }
        if let Some(img) = &a.image_url {
            if !img.trim().is_empty() {
                md.push_str("![](");
                md.push_str(img.trim());
                md.push_str(")\n\n");
            }
        }
        md.push_str("原文：");
        md.push_str(a.url.trim());
        md.push_str("\n\n");
        let content = a.content.trim();
        if !content.is_empty() {
            md.push_str(&trim_to_chars(content, 8000));
            md.push_str("\n");
        } else if !a.description.trim().is_empty() {
            md.push_str(a.description.trim());
            md.push_str("\n");
        }
    }

    let file_stem = sanitize_file_stem(&topic);
    let path = build_kb_note_path(&kb_name, &file_stem)?;
    let file_path = write_kb_markdown_note(&path, &md)?;
    Ok(GeneratedNoteResult {
        file_path,
        item_count: selected.len(),
    })
}

#[command]
pub async fn create_kb_note_from_article_ids(
    kb_name: String,
    title: String,
    article_ids: Vec<String>,
) -> Result<GeneratedNoteResult, String> {
    let kb_name = kb_name.trim().to_string();
    if kb_name.is_empty() {
        return Err("kb_name 不能为空".to_string());
    }
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err("title 不能为空".to_string());
    }
    if article_ids.is_empty() {
        return Err("article_ids 不能为空".to_string());
    }

    let mut md = String::new();
    md.push_str("# ");
    md.push_str(&title);
    md.push_str("\n\n");
    md.push_str("生成时间：");
    md.push_str(&Utc::now().to_rfc3339());
    md.push_str("\n\n");

    let mut count = 0usize;
    for (idx, id) in article_ids.iter().enumerate() {
        let article = get_article_detail(id.to_string(), None).await?;
        if idx > 0 {
            md.push_str("\n---\n\n");
        }
        md.push_str("## ");
        md.push_str(article.title.trim());
        md.push_str("\n\n");
        if let Some(img) = &article.image_url {
            if !img.trim().is_empty() {
                md.push_str("![](");
                md.push_str(img.trim());
                md.push_str(")\n\n");
            }
        }
        md.push_str("原文：");
        md.push_str(article.url.trim());
        md.push_str("\n\n");
        if !article.content.trim().is_empty() {
            md.push_str(&trim_to_chars(article.content.trim(), 8000));
            md.push_str("\n");
        } else if !article.description.trim().is_empty() {
            md.push_str(article.description.trim());
            md.push_str("\n");
        }
        count += 1;
    }

    let file_stem = sanitize_file_stem(&title);
    let path = build_kb_note_path(&kb_name, &file_stem)?;
    let file_path = write_kb_markdown_note(&path, &md)?;
    Ok(GeneratedNoteResult { file_path, item_count: count })
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
