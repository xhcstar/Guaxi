use chrono::Utc;
use crate::commands::flipboard_pipeline::{
    articles_fingerprint, build_site_discovery,
    classify_category_by_text_and_url, compact_plain_text, default_scheduler_policy,
    feed_interval_secs, load_feed_records_from_website_markdown,
    should_run_feed,
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
use tauri::{command, AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::Semaphore;
use futures_util::StreamExt;
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

// ===== 抓取任务取消（用户可随时终止，避免后台操作卡住或跑错配置无法停止）=====
static CANCEL_ALL_TASKS: AtomicBool = AtomicBool::new(false);
static CANCELLED_FEED_IDS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));
// 每个 host 本会话内是否已主动弹出过登录窗口（避免反复弹窗）。
static LOGIN_PROMPTED_HOSTS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// 某个 feed 的抓取是否被请求终止。
fn is_task_cancelled(feed_id: &str) -> bool {
    if CANCEL_ALL_TASKS.load(Ordering::Relaxed) {
        return true;
    }
    CANCELLED_FEED_IDS
        .lock()
        .map(|s| s.contains(feed_id.trim()))
        .unwrap_or(false)
}

/// 新一轮抓取开始时清除历史的“停止”标记，使停止只作用于当时在跑的任务。
fn reset_task_cancellation() {
    CANCEL_ALL_TASKS.store(false, Ordering::Relaxed);
    if let Ok(mut s) = CANCELLED_FEED_IDS.lock() {
        s.clear();
    }
}

/// 清除单个 feed 的停止标记（重爬 / 重新启动该源时调用）。
fn clear_feed_cancel(feed_id: &str) {
    if let Ok(mut s) = CANCELLED_FEED_IDS.lock() {
        s.remove(feed_id.trim());
    }
}

/// 命中登录墙时，本会话内对该 host 仅主动弹出一次登录窗口（非阻塞）。
fn maybe_prompt_login(url: &str) {
    let host = match Url::parse(url.trim()).ok().and_then(|u| u.host_str().map(|h| h.to_string())) {
        Some(h) => h,
        None => return,
    };
    {
        let mut set = match LOGIN_PROMPTED_HOSTS.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        if set.contains(&host) {
            return;
        }
        set.insert(host.clone());
    }
    let login_url = url.trim().to_string();
    let profile = host.clone();
    // 后台弹出可见登录窗口，不阻塞抓取流程。
    tokio::spawn(async move {
        info!("flipboard: auto-opening login window for host {}", profile);
        if let Err(e) =
            crate::commands::flipboard_render::open_login_window(&profile, &login_url).await
        {
            warn!("flipboard: auto login window failed for {}: {}", profile, e);
        }
    });
}

/// 终止指定 feed 的抓取任务。
#[command]
pub async fn stop_feed_task(feed_id: String) -> Result<(), String> {
    if let Ok(mut s) = CANCELLED_FEED_IDS.lock() {
        s.insert(feed_id.trim().to_string());
    }
    info!("flipboard: stop_feed_task requested for {}", feed_id);
    Ok(())
}

/// 终止全部抓取任务。
#[command]
pub async fn stop_all_feed_tasks() -> Result<(), String> {
    CANCEL_ALL_TASKS.store(true, Ordering::Relaxed);
    info!("flipboard: stop_all_feed_tasks requested");
    Ok(())
}
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

/// 预置源 schema 版本。每次更换内置源清单时 +1，会触发一次性迁移：
/// 清掉旧的机器生成源（builtin-/auto-/website-），写入新的 curated 源。
/// 用户手动添加（f*/sd*）的源不受影响。
const FEEDS_SCHEMA_VERSION: u32 = 3;

fn flipboard_feeds_version_path() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let base_dir = exe_path
        .parent()
        .ok_or_else(|| "无法获取可执行文件目录".to_string())?;
    let config_dir = base_dir.join("config");
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    }
    Ok(config_dir.join("flipboard_feeds_version.txt"))
}

fn read_feeds_schema_version() -> u32 {
    flipboard_feeds_version_path()
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0)
}

fn write_feeds_schema_version(version: u32) {
    if let Ok(path) = flipboard_feeds_version_path() {
        if let Err(e) = fs::write(&path, version.to_string()) {
            warn!("flipboard: write feeds schema version failed: {}", e);
        }
    }
}

/// 机器生成的预置源 id 前缀（区别于用户手动添加的 f*/sd*）。
fn is_preconfigured_feed_id(id: &str) -> bool {
    let id = id.trim();
    id.starts_with("builtin-") || id.starts_with("auto-") || id.starts_with("website-")
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
        CREATE TABLE IF NOT EXISTS crawl_queue (
            feed_id TEXT NOT NULL,
            url TEXT NOT NULL,
            depth INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'pending',
            kind TEXT NOT NULL DEFAULT 'page',
            discovered_at INTEGER NOT NULL DEFAULT 0,
            fetched_at INTEGER NOT NULL DEFAULT 0,
            error TEXT,
            PRIMARY KEY(feed_id, url)
        );
        CREATE INDEX IF NOT EXISTS idx_crawl_queue_status
            ON crawl_queue(feed_id, status, depth);
        "#,
    )
    .map_err(|e| e.to_string())?;
    ensure_article_columns(&conn)?;
    Ok(conn)
}

/// 向后兼容迁移：旧 article_cache 表缺少工业级管线新增列时，逐列 ADD COLUMN。
/// SQLite 无 `ADD COLUMN IF NOT EXISTS`，故先用 PRAGMA table_info 探测已有列。
/// 新列只能追加在末尾，否则 row_to_article 的按位读取会错位。
fn ensure_article_columns(conn: &Connection) -> Result<(), String> {
    let mut existing: HashSet<String> = HashSet::new();
    {
        let mut stmt = conn
            .prepare("PRAGMA table_info(article_cache)")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| e.to_string())?;
        for r in rows {
            if let Ok(name) = r {
                existing.insert(name);
            }
        }
    }
    // (列名, 列定义)，顺序须与 SELECT / row_to_article 末尾顺序一致。
    let columns: &[(&str, &str)] = &[
        ("clean_content_html", "TEXT NOT NULL DEFAULT ''"),
        ("clean_content_markdown", "TEXT NOT NULL DEFAULT ''"),
        ("lead_image", "TEXT"),
        ("content_images", "TEXT NOT NULL DEFAULT '[]'"),
        ("relevance_score", "INTEGER NOT NULL DEFAULT -1"),
        ("tags", "TEXT NOT NULL DEFAULT '[]'"),
        ("summary", "TEXT NOT NULL DEFAULT ''"),
        ("render_mode", "TEXT NOT NULL DEFAULT ''"),
    ];
    for (name, decl) in columns {
        if !existing.contains(*name) {
            conn.execute(
                &format!("ALTER TABLE article_cache ADD COLUMN {} {}", name, decl),
                [],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_article_cache_relevance \
         ON article_cache(relevance_score DESC, updated_at DESC);",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn save_articles_to_cache(feed_id: &str, items: &[Article]) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }
    let mut conn = open_article_cache_db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let now = unix_now();
    for a in items {
        let content_images_json = serde_json::to_string(&a.content_images).unwrap_or_else(|_| "[]".to_string());
        let tags_json = serde_json::to_string(&a.tags).unwrap_or_else(|_| "[]".to_string());
        tx.execute(
            r#"
            INSERT INTO article_cache
                (id, feed_id, title, description, content, source, url, image_url, published_at, category, author, read_count, likes, updated_at,
                 clean_content_html, clean_content_markdown, lead_image, content_images, relevance_score, tags, summary, render_mode)
            VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                 ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)
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
                updated_at=excluded.updated_at,
                clean_content_html=excluded.clean_content_html,
                clean_content_markdown=excluded.clean_content_markdown,
                lead_image=excluded.lead_image,
                content_images=excluded.content_images,
                relevance_score=excluded.relevance_score,
                tags=excluded.tags,
                summary=excluded.summary,
                render_mode=excluded.render_mode
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
                now,
                a.clean_content_html,
                a.clean_content_markdown,
                a.lead_image,
                content_images_json,
                a.relevance_score,
                tags_json,
                a.summary,
                a.render_mode
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

/// 把深解析(hydrate)后的完整内容回写到所有匹配该 URL 的缓存行（按 url 更新，
/// 无需 feed_id）。让"二次打开秒开"且卡片列表后续直接拿到完整图文。
fn persist_hydrated_article(a: &Article) -> Result<(), String> {
    let url = a.url.trim();
    if url.is_empty() {
        return Ok(());
    }
    let conn = open_article_cache_db()?;
    let content_images_json =
        serde_json::to_string(&a.content_images).unwrap_or_else(|_| "[]".to_string());
    let tags_json = serde_json::to_string(&a.tags).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        r#"
        UPDATE article_cache SET
            title = ?2,
            description = ?3,
            content = ?4,
            image_url = ?5,
            category = ?6,
            author = ?7,
            clean_content_html = ?8,
            clean_content_markdown = ?9,
            lead_image = ?10,
            content_images = ?11,
            summary = ?12,
            render_mode = ?13,
            relevance_score = CASE WHEN ?14 >= 0 THEN ?14 ELSE relevance_score END,
            tags = CASE WHEN ?15 = '[]' THEN tags ELSE ?15 END
        WHERE url = ?1
        "#,
        params![
            url,
            a.title,
            a.description,
            a.content,
            a.image_url,
            a.category,
            a.author,
            a.clean_content_html,
            a.clean_content_markdown,
            a.lead_image,
            content_images_json,
            a.summary,
            a.render_mode,
            a.relevance_score,
            tags_json,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 后台预热：对一批文章里"内容薄 + 相关性高"的前 max 篇做深解析(hydrate)并就地替换、
/// 回写缓存。每个 feed 每 tick 仅预热少量，避免拖慢调度。
async fn preheat_thin_articles(list: &mut [Article], feed: &FeedSource, max: usize) {
    if max == 0 || list.is_empty() {
        return;
    }
    let interest = feed.interest_profile.as_deref();
    let mut candidates: Vec<usize> = list
        .iter()
        .enumerate()
        .filter(|(_, a)| {
            a.clean_content_html.trim().is_empty() && a.content.trim().chars().count() < 200
        })
        .map(|(i, _)| i)
        .collect();
    // 相关性高者优先预热。
    candidates.sort_by_key(|&i| std::cmp::Reverse(local_relevance_score(&list[i], interest)));
    for &i in candidates.iter().take(max) {
        match hydrate_article_readable_content(&list[i]).await {
            Ok(enriched) => {
                let _ = persist_hydrated_article(&enriched);
                list[i] = enriched;
            }
            Err(e) => {
                warn!("flipboard: preheat hydrate failed for {}: {}", list[i].url, e);
            }
        }
    }
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

/// 启动清理：删除所有未收藏的缓存文章，仅保留 keep_urls（用户收藏）。
/// 由前端在启动时携带收藏 URL 列表调用，避免缓存无限增长占用磁盘。
#[command]
pub async fn purge_unbookmarked_articles(keep_urls: Vec<String>) -> Result<usize, String> {
    let conn = open_article_cache_db()?;

    let keep: Vec<String> = keep_urls
        .into_iter()
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty())
        .collect();

    let removed = if keep.is_empty() {
        conn.execute("DELETE FROM article_cache", [])
            .map_err(|e| e.to_string())?
    } else {
        // 构造 IN 占位符 (?1, ?2, ...)，按 url 保留收藏项。
        let placeholders = (1..=keep.len())
            .map(|i| format!("?{}", i))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "DELETE FROM article_cache WHERE url NOT IN ({})",
            placeholders
        );
        let params_vec: Vec<&dyn rusqlite::ToSql> =
            keep.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        conn.execute(&sql, params_vec.as_slice())
            .map_err(|e| e.to_string())?
    };

    // 同步内存中的 ARTICLES（仅保留收藏的，避免重启前内存仍持旧数据）。
    {
        let keep_set: HashSet<String> = keep.iter().map(|u| u.to_lowercase()).collect();
        let mut lock = ARTICLES.lock().unwrap();
        lock.retain(|a| keep_set.contains(&a.url.trim().to_lowercase()));
    }

    info!(
        "flipboard: purge_unbookmarked_articles removed {} rows, kept {} bookmarked",
        removed,
        keep.len()
    );
    Ok(removed)
}

fn row_to_article(row: &rusqlite::Row<'_>) -> rusqlite::Result<Article> {
    // 列顺序须与各 SELECT 一致：
    // 12 clean_content_html, 13 clean_content_markdown, 14 lead_image,
    // 15 content_images, 16 relevance_score, 17 tags, 18 summary, 19 render_mode
    let content_images_json: String = row.get(15).unwrap_or_default();
    let tags_json: String = row.get(17).unwrap_or_default();
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
        clean_content_html: row.get(12).unwrap_or_default(),
        clean_content_markdown: row.get(13).unwrap_or_default(),
        lead_image: row.get(14).ok(),
        content_images: serde_json::from_str(&content_images_json).unwrap_or_default(),
        relevance_score: row.get(16).unwrap_or(-1),
        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        summary: row.get(18).unwrap_or_default(),
        render_mode: row.get(19).unwrap_or_default(),
        login_required: false,
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
            "SELECT id,title,description,content,source,url,image_url,published_at,category,author,read_count,likes,clean_content_html,clean_content_markdown,lead_image,content_images,relevance_score,tags,summary,render_mode \
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
        "SELECT id,title,description,content,source,url,image_url,published_at,category,author,read_count,likes,clean_content_html,clean_content_markdown,lead_image,content_images,relevance_score,tags,summary,render_mode \
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
                "SELECT id,title,description,content,source,url,image_url,published_at,category,author,read_count,likes,clean_content_html,clean_content_markdown,lead_image,content_images,relevance_score,tags,summary,render_mode \
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
                "SELECT id,title,description,content,source,url,image_url,published_at,category,author,read_count,likes,clean_content_html,clean_content_markdown,lead_image,content_images,relevance_score,tags,summary,render_mode \
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
                        ..Default::default()
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

/// 从配置文件加载订阅源；如果文件不存在则返回默认订阅源。
/// 返回值第二项表示是否需要把结果写回磁盘（首次初始化或发生了迁移时为 true）。
fn load_feeds_from_config() -> Result<(Vec<FeedSource>, bool), String> {
    let path = flipboard_config_path()?;

    // 没有配置文件：全新安装，直接使用 curated 默认源。
    if !path.exists() {
        info!(
            "flipboard: feeds config not found, use curated default feeds: {}",
            path.display()
        );
        let mut feeds = get_default_feeds();
        merge_feeds_from_website_markdown(&mut feeds);
        return Ok((feeds, true));
    }

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut feeds: Vec<FeedSource> =
        serde_json::from_str(&content).map_err(|e| e.to_string())?;

    let stored_version = read_feeds_schema_version();
    let migrating = stored_version < FEEDS_SCHEMA_VERSION;

    if migrating {
        // 一次性迁移：清掉所有旧的机器生成源（rsshub/知乎/小红书/微信等），
        // 保留用户手动添加的源（f*/sd*），再写入新的 curated 源。
        let before = feeds.len();
        feeds.retain(|f| !is_preconfigured_feed_id(&f.id));
        let purged = before - feeds.len();

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

        let mut added = 0usize;
        for d in get_default_feeds() {
            let key = format!(
                "{}|{}|{}",
                d.url.trim().to_lowercase(),
                d.category.trim().to_lowercase(),
                d.source_type.trim().to_lowercase()
            );
            if existing_keys.insert(key) {
                feeds.push(d);
                added += 1;
            }
        }

        info!(
            "flipboard: migrated feeds schema {} -> {} (purged {} legacy preset feeds, added {} curated feeds)",
            stored_version, FEEDS_SCHEMA_VERSION, purged, added
        );
    }
    // 非迁移路径：不再无条件合并默认源，尊重用户后续的删除/启停操作。

    merge_feeds_from_website_markdown(&mut feeds);

    Ok((feeds, migrating))
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
        Ok((list, should_persist)) => {
            info!("flipboard: loaded {} feeds from config", list.len());
            *feeds_lock = list;
            if should_persist {
                if let Err(se) = save_feeds_to_config(&feeds_lock) {
                    warn!("flipboard: persist feeds after migration failed: {}", se);
                }
                write_feeds_schema_version(FEEDS_SCHEMA_VERSION);
            }
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
            write_feeds_schema_version(FEEDS_SCHEMA_VERSION);
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
    // ===== 工业级抓取管线新增字段（均带 serde default，向后兼容旧 JSON / 旧缓存行）=====
    /// 清洗后的正文 HTML（内联 <img>，URL 已绝对化）。前端优先渲染此字段。
    #[serde(default)]
    pub clean_content_html: String,
    /// 清洗后的正文 Markdown（供 KB 导出 / LLM 输入）。
    #[serde(default)]
    pub clean_content_markdown: String,
    /// 头图（区别于 image_url 缩略图）。
    #[serde(default)]
    pub lead_image: Option<String>,
    /// 正文内联图片 URL（有序）。
    #[serde(default)]
    pub content_images: Vec<String>,
    /// 相关性分数 0-100；-1 表示未评分。
    #[serde(default = "default_relevance_score")]
    pub relevance_score: i32,
    /// LLM / 规则生成的标签。
    #[serde(default)]
    pub tags: Vec<String>,
    /// 摘要。
    #[serde(default)]
    pub summary: String,
    /// 渲染来源："http" | "chrome" | "rss"。
    #[serde(default)]
    pub render_mode: String,
    /// 命中登录墙：正文抓取被登录/验证页拦截，需用户登录后重试。
    #[serde(default)]
    pub login_required: bool,
}

fn default_relevance_score() -> i32 {
    -1
}

impl Default for Article {
    fn default() -> Self {
        Article {
            id: String::new(),
            title: String::new(),
            description: String::new(),
            content: String::new(),
            source: String::new(),
            url: String::new(),
            image_url: None,
            published_at: String::new(),
            category: String::new(),
            author: None,
            read_count: 0,
            likes: 0,
            clean_content_html: String::new(),
            clean_content_markdown: String::new(),
            lead_image: None,
            content_images: Vec::new(),
            relevance_score: -1,
            tags: Vec::new(),
            summary: String::new(),
            render_mode: String::new(),
            login_required: false,
        }
    }
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
    // ===== 工业级抓取管线新增（均带 serde default，向后兼容旧 feeds.json）=====
    /// 用户的兴趣描述，用于 LLM/本地相关性打分。
    #[serde(default)]
    pub interest_profile: Option<String>,
    /// 相关性阈值 0-100；低于该分的文章被丢弃。0 = 不过滤。
    #[serde(default)]
    pub relevance_threshold: i32,
    /// 爬虫配置（JSON 字符串：max_depth/max_pages_per_tick/...），仅 site_crawl 用。
    #[serde(default)]
    pub crawl_cfg: Option<String>,
    /// 渲染会话 profile 名（登录墙复用）。
    #[serde(default)]
    pub render_profile: Option<String>,
    /// 是否对该源启用 LLM 增强（摘要/标签/清洗）。
    #[serde(default)]
    pub llm_enhance: bool,
}

impl Default for FeedSource {
    fn default() -> Self {
        FeedSource {
            id: String::new(),
            name: String::new(),
            url: String::new(),
            category: "all".to_string(),
            enabled: true,
            source_type: String::new(),
            fetch_interval: None,
            include_keywords: None,
            exclude_keywords: None,
            remark: None,
            interest_profile: None,
            relevance_threshold: 0,
            crawl_cfg: None,
            render_profile: None,
            llm_enhance: false,
        }
    }
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
/// 经过实测验证的高质量内置源（单一数据源）：(显示名, URL, source_type, category)。
/// 旧的 rsshub/知乎/小红书/微信等低质量与登录墙源已全部移除。
fn curated_feeds() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    vec![
        // ===== 科技 technology =====
        ("少数派", "https://sspai.com/feed", "rss", "technology"),
        ("爱范儿", "https://www.ifanr.com/feed", "rss", "technology"),
        ("36氪", "https://36kr.com/feed", "rss", "technology"),
        ("The Verge", "https://www.theverge.com/rss/index.xml", "rss", "technology"),
        ("Ars Technica", "https://feeds.arstechnica.com/arstechnica/index", "rss", "technology"),
        ("TechCrunch", "https://techcrunch.com/feed/", "rss", "technology"),
        ("Wired", "https://www.wired.com/feed/rss", "rss", "technology"),
        ("MIT Technology Review", "https://www.technologyreview.com/feed/", "rss", "technology"),
        ("Engadget", "https://www.engadget.com/rss.xml", "rss", "technology"),
        ("阮一峰周刊", "https://www.ruanyifeng.com/blog/atom.xml", "rss", "technology"),
        // ===== 计算机 computer =====
        ("Hacker News", "https://hnrss.org/frontpage", "rss", "computer"),
        ("Lobsters", "https://lobste.rs/rss", "rss", "computer"),
        ("InfoQ", "https://feed.infoq.com/", "rss", "computer"),
        ("Martin Fowler", "https://martinfowler.com/feed.atom", "rss", "computer"),
        ("The Pragmatic Engineer", "https://blog.pragmaticengineer.com/rss/", "rss", "computer"),
        ("CSS-Tricks", "https://css-tricks.com/feed/", "rss", "computer"),
        ("Smashing Magazine", "https://www.smashingmagazine.com/feed/", "rss", "computer"),
        ("Julia Evans", "https://jvns.ca/atom.xml", "rss", "computer"),
        ("GitHub Trending", "https://github.com/trending", "spider", "computer"),
        ("酷壳 CoolShell", "https://coolshell.cn/feed", "rss", "computer"),
        // ===== AI =====
        ("arXiv cs.AI", "https://rss.arxiv.org/rss/cs.AI", "rss", "ai"),
        ("arXiv cs.LG", "https://rss.arxiv.org/rss/cs.LG", "rss", "ai"),
        ("arXiv cs.CL", "https://rss.arxiv.org/rss/cs.CL", "rss", "ai"),
        ("Hugging Face Blog", "https://huggingface.co/blog/feed.xml", "rss", "ai"),
        ("BAIR Blog", "https://bair.berkeley.edu/blog/feed.xml", "rss", "ai"),
        ("The Gradient", "https://thegradient.pub/rss/", "rss", "ai"),
        ("Import AI", "https://importai.substack.com/feed", "rss", "ai"),
        ("MIT News AI", "https://news.mit.edu/topic/mitartificial-intelligence2-rss.xml", "rss", "ai"),
        ("Machine Learning Mastery", "https://machinelearningmastery.com/feed/", "rss", "ai"),
        ("Lil'Log", "https://lilianweng.github.io/index.xml", "rss", "ai"),
        // ===== 数学 math =====
        ("arXiv math", "https://rss.arxiv.org/rss/math", "rss", "math"),
        ("Terence Tao", "https://terrytao.wordpress.com/feed/", "rss", "math"),
        ("Quanta Magazine", "https://www.quantamagazine.org/feed/", "rss", "math"),
        ("Gowers's Weblog", "https://gowers.wordpress.com/feed/", "rss", "math"),
        ("Math ∩ Programming", "https://www.jeremykun.com/index.xml", "rss", "math"),
        ("AMS Blog on Math Blogs", "https://blogs.ams.org/blogonmathblogs/feed/", "rss", "math"),
        ("John Baez Azimuth", "https://johncarlosbaez.wordpress.com/feed/", "rss", "math"),
        ("3Blue1Brown", "https://www.youtube.com/feeds/videos.xml?channel_id=UCYO_jab_esuFRV4b17AJtAw", "rss", "math"),
        ("Numberphile", "https://www.youtube.com/feeds/videos.xml?user=numberphile", "rss", "math"),
        ("Wikipedia 数学门户", "https://en.wikipedia.org/wiki/Portal:Mathematics", "spider", "math"),
        // ===== 物理 physics =====
        ("arXiv astro-ph", "https://rss.arxiv.org/rss/astro-ph", "rss", "physics"),
        ("arXiv quant-ph", "https://rss.arxiv.org/rss/quant-ph", "rss", "physics"),
        ("arXiv hep-th", "https://rss.arxiv.org/rss/hep-th", "rss", "physics"),
        ("Physics World", "https://physicsworld.com/feed/", "rss", "physics"),
        ("APS Physics", "http://feeds.aps.org/rss/recent/physics.xml", "rss", "physics"),
        ("Quanta Magazine (物理)", "https://www.quantamagazine.org/feed/", "rss", "physics"),
        ("Sean Carroll", "https://www.preposterousuniverse.com/blog/feed/", "rss", "physics"),
        ("Shtetl-Optimized", "https://scottaaronson.blog/?feed=rss2", "rss", "physics"),
        ("Symmetry Magazine", "https://www.symmetrymagazine.org/feed", "rss", "physics"),
        ("NASA APOD", "https://apod.nasa.gov/apod.rss", "rss", "physics"),
        // ===== 医学 medicine =====
        ("The Lancet", "https://www.thelancet.com/rssfeed/lancet_current.xml", "rss", "medicine"),
        ("BMJ", "https://www.bmj.com/rss/current.xml", "rss", "medicine"),
        ("STAT News", "https://www.statnews.com/feed/", "rss", "medicine"),
        ("ScienceDaily 健康医学", "https://rss.sciencedaily.com/health_medicine.xml", "rss", "medicine"),
        ("Nature Medicine", "https://www.nature.com/nm.rss", "rss", "medicine"),
        ("Medscape", "https://www.medscape.com/cx/rssfeeds/2700.xml", "rss", "medicine"),
        ("KevinMD", "https://www.kevinmd.com/feed", "rss", "medicine"),
        ("Harvard Health", "https://www.health.harvard.edu/blog/feed", "rss", "medicine"),
        ("NEJM", "https://www.nejm.org/action/showFeed?jc=nejm&type=etoc&feed=rss", "rss", "medicine"),
        ("MedlinePlus What's New", "https://medlineplus.gov/feeds/whatsnew_en.xml", "rss", "medicine"),
        ("CDC MMWR", "https://tools.cdc.gov/api/v2/resources/media/342778.rss", "rss", "medicine"),
        // ===== 商业 business =====
        ("Knowledge at Wharton", "https://knowledge.wharton.upenn.edu/feed/", "rss", "business"),
        ("McKinsey Insights", "https://www.mckinsey.com/insights/rss", "rss", "business"),
        ("钛媒体", "https://www.tmtpost.com/feed", "rss", "business"),
        ("Stratechery", "https://stratechery.com/feed/", "rss", "business"),
        ("36氪 (商业)", "https://36kr.com/feed", "rss", "business"),
        ("VentureBeat", "https://feeds.feedburner.com/venturebeat/SZYF", "rss", "business"),
        ("Business Insider", "https://www.businessinsider.com/rss", "rss", "business"),
        ("Inc.com", "https://www.inc.com/rss/", "rss", "business"),
        ("Bloomberg Business", "https://feeds.bloomberg.com/business/news.rss", "rss", "business"),
        ("HBS Working Knowledge", "https://hbswk.hbs.edu/rss/all.xml", "rss", "business"),
        ("Forbes Business", "https://www.forbes.com/business/feed/", "rss", "business"),
        ("36氪快讯", "https://36kr.com/feed-newsflash", "rss", "business"),
        // ===== 金融 finance =====
        ("CNBC Top News", "https://www.cnbc.com/id/100003114/device/rss/rss.html", "rss", "finance"),
        ("Marginal Revolution", "https://marginalrevolution.com/feed", "rss", "finance"),
        ("A Wealth of Common Sense", "https://awealthofcommonsense.com/feed/", "rss", "finance"),
        ("Calculated Risk", "https://www.calculatedriskblog.com/feeds/posts/default", "rss", "finance"),
        ("The Big Picture", "https://ritholtz.com/feed/", "rss", "finance"),
        ("Federal Reserve", "https://www.federalreserve.gov/feeds/press_all.xml", "rss", "finance"),
        ("Investing.com", "https://www.investing.com/rss/news.rss", "rss", "finance"),
        ("Seeking Alpha", "https://seekingalpha.com/feed.xml", "rss", "finance"),
        ("The Economist Finance", "https://www.economist.com/finance-and-economics/rss.xml", "rss", "finance"),
        ("MarketWatch Top Stories", "https://feeds.content.dowjones.io/public/rss/mw_topstories", "rss", "finance"),
        ("Financial Times Home", "https://www.ft.com/rss/home", "rss", "finance"),
        ("WSJ Markets", "https://feeds.a.dj.com/rss/RSSMarketsMain.xml", "rss", "finance"),
        // ===== 法律 law =====
        ("SCOTUSblog", "https://www.scotusblog.com/feed/", "rss", "law"),
        ("Volokh Conspiracy", "https://reason.com/volokh/feed/", "rss", "law"),
        ("Above the Law", "https://abovethelaw.com/feed/", "rss", "law"),
        ("JURIST", "https://www.jurist.org/news/feed/", "rss", "law"),
        ("EFF Deeplinks", "https://www.eff.org/rss/updates.xml", "rss", "law"),
        ("Harvard Law Review", "https://harvardlawreview.org/feed/", "rss", "law"),
        ("Balkinization", "https://balkin.blogspot.com/feeds/posts/default", "rss", "law"),
        ("SCOTUSblog Petitions", "https://www.scotusblog.com/category/cases-in-the-pipeline/feed/", "rss", "law"),
        ("SCOTUSblog Featured", "https://www.scotusblog.com/category/featured/feed/", "rss", "law"),
        ("Reason", "https://reason.com/feed/", "rss", "law"),
        ("ACLU News", "https://www.aclu.org/news/feed", "rss", "law"),
        ("Just Security", "https://www.justsecurity.org/feed/", "rss", "law"),
        // ===== 教育 education =====
        ("EdSurge", "https://www.edsurge.com/articles_rss", "rss", "education"),
        ("Inside Higher Ed", "https://www.insidehighered.com/rss.xml", "rss", "education"),
        ("Khan Academy Blog", "https://blog.khanacademy.org/feed/", "rss", "education"),
        ("The Hechinger Report", "https://hechingerreport.org/feed/", "rss", "education"),
        ("eLearning Industry", "https://elearningindustry.com/feed", "rss", "education"),
        ("TeachThought", "https://www.teachthought.com/feed/", "rss", "education"),
        ("Education Next", "https://www.educationnext.org/feed/", "rss", "education"),
        ("Class Central", "https://www.classcentral.com/report/feed/", "rss", "education"),
        ("MindShift", "https://ww2.kqed.org/mindshift/feed/", "rss", "education"),
        ("Higher Ed Dive", "https://www.highereddive.com/feeds/news/", "rss", "education"),
        // ===== 生活 lifestyle =====
        ("少数派 (生活)", "https://sspai.com/feed", "rss", "lifestyle"),
        ("Lifehacker", "https://lifehacker.com/rss", "rss", "lifestyle"),
        ("Mark Manson", "https://markmanson.net/feed", "rss", "lifestyle"),
        ("The Kitchn", "https://www.thekitchn.com/main.rss", "rss", "lifestyle"),
        ("Apartment Therapy", "https://www.apartmenttherapy.com/main.rss", "rss", "lifestyle"),
        ("Longreads", "https://longreads.com/feed/", "rss", "lifestyle"),
        ("The Art of Manliness", "https://www.artofmanliness.com/feed/", "rss", "lifestyle"),
        ("NYT Well", "https://rss.nytimes.com/services/xml/rss/nyt/Well.xml", "rss", "lifestyle"),
        ("mindbodygreen", "https://www.mindbodygreen.com/rss.xml", "rss", "lifestyle"),
        ("Lifehack", "https://lifehack.org/feed", "rss", "lifestyle"),
        ("Becoming Minimalist", "https://www.becomingminimalist.com/feed/", "rss", "lifestyle"),
        ("Nutrition Stripped", "https://nutritionstripped.com/feed/", "rss", "lifestyle"),
        ("Breaking Muscle", "https://breakingmuscle.com/feed/", "rss", "lifestyle"),
        // ===== 游戏/娱乐 entertainment =====
        ("IGN", "https://feeds.ign.com/ign/all", "rss", "entertainment"),
        ("GameSpot", "https://www.gamespot.com/feeds/news/", "rss", "entertainment"),
        ("Polygon", "https://www.polygon.com/rss/index.xml", "rss", "entertainment"),
        ("Kotaku", "https://kotaku.com/rss", "rss", "entertainment"),
        ("Rock Paper Shotgun", "https://www.rockpapershotgun.com/feed", "rss", "entertainment"),
        ("PC Gamer", "https://www.pcgamer.com/rss/", "rss", "entertainment"),
        ("Eurogamer", "https://www.eurogamer.net/feed", "rss", "entertainment"),
        ("机核 GCORES", "https://www.gcores.com/rss", "rss", "entertainment"),
        ("Variety", "https://variety.com/feed/", "rss", "entertainment"),
        ("The Hollywood Reporter", "https://www.hollywoodreporter.com/feed/", "rss", "entertainment"),

        // ===== 知识性平台 / 问答 / 百科（区别于资讯新闻）=====
        // 计算机 / 编程 Q&A
        ("Stack Overflow 问答", "https://stackoverflow.com/feeds", "rss", "computer"),
        ("Server Fault 问答", "https://serverfault.com/feeds", "rss", "computer"),
        ("Computer Science SE", "https://cs.stackexchange.com/feeds", "rss", "computer"),
        // AI / 数据科学 Q&A
        ("Cross Validated (统计/ML)", "https://stats.stackexchange.com/feeds", "rss", "ai"),
        ("Data Science SE", "https://datascience.stackexchange.com/feeds", "rss", "ai"),
        ("AI Stack Exchange", "https://ai.stackexchange.com/feeds", "rss", "ai"),
        // 数学 Q&A + 研究
        ("Mathematics SE 问答", "https://math.stackexchange.com/feeds", "rss", "math"),
        ("MathOverflow 研究级问答", "https://mathoverflow.net/feeds", "rss", "math"),
        // 物理 Q&A
        ("Physics SE 问答", "https://physics.stackexchange.com/feeds", "rss", "physics"),
        ("Astronomy SE 问答", "https://astronomy.stackexchange.com/feeds", "rss", "physics"),
        // 医学 / 健康 Q&A
        ("Health/Medical SE", "https://medicalsciences.stackexchange.com/feeds", "rss", "medicine"),
        // 法律 Q&A
        ("Law Stack Exchange 问答", "https://law.stackexchange.com/feeds", "rss", "law"),
        // 商业 / 创业 Q&A
        ("Workplace SE", "https://workplace.stackexchange.com/feeds", "rss", "business"),
        // 金融 Q&A
        ("Personal Finance SE", "https://money.stackexchange.com/feeds", "rss", "finance"),
        ("Quant Finance SE", "https://quant.stackexchange.com/feeds", "rss", "finance"),
        // 教育 / 通识 / 百科 / 哲学
        ("斯坦福哲学百科 (SEP)", "https://plato.stanford.edu/rss/sep.xml", "rss", "education"),
        ("Wikipedia 每日精选", "https://en.wikipedia.org/w/api.php?action=featuredfeed&feed=featured&feedformat=atom", "rss", "education"),
        ("Wikipedia 历史上的今天", "https://en.wikipedia.org/w/api.php?action=featuredfeed&feed=onthisday&feedformat=atom", "rss", "education"),
        ("Aeon 思想长文", "https://aeon.co/feed.rss", "rss", "education"),
        ("Big Think", "https://bigthink.com/feed/", "rss", "education"),
        ("Open Culture", "https://www.openculture.com/feed", "rss", "education"),
        ("Arts & Letters Daily", "https://aldaily.com/feed/", "rss", "education"),
        ("Stephen Wolfram Writings", "https://writings.stephenwolfram.com/feed/", "rss", "education"),
        // 综合知识 / 科普
        ("Smithsonian Magazine", "https://www.smithsonianmag.com/rss/latest_articles/", "rss", "lifestyle"),
        ("ScienceAlert", "https://www.sciencealert.com/feed", "rss", "physics"),
        // 知识社区（需登录/动态渲染，走 spider；命中登录墙会引导登录）
        ("知乎 - 专栏", "https://zhuanlan.zhihu.com/", "spider", "education"),
        ("Quora", "https://www.quora.com/", "spider", "education"),
    ]
}

fn get_default_feeds() -> Vec<FeedSource> {
    let mut default_feeds = Vec::new();
    let mut per_category_idx: HashMap<&str, usize> = HashMap::new();

    for (name, url, source_type, category) in curated_feeds() {
        let idx = per_category_idx.entry(category).or_insert(0);
        default_feeds.push(FeedSource {
            id: format!("builtin-{}-{}", category, idx),
            name: name.to_string(),
            url: url.to_string(),
            category: category.to_string(),
            enabled: true,
            source_type: source_type.to_string(),
            fetch_interval: Some(60),
            include_keywords: None,
            exclude_keywords: None,
            remark: Some("curated_builtin".to_string()),
            ..Default::default()
        });
        *idx += 1;
    }

    default_feeds
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PlatformFeedTemplate {
    name: String,
    url: String,
    source_type: String,
    include_keywords: Option<String>,
    exclude_keywords: Option<String>,
    remark: Option<String>,
}

fn build_default_feeds_for_platform(_platform: &str) -> Vec<PlatformFeedTemplate> {
    // 旧的 rsshub / 知乎 / B站 / 小红书 / 微信 / Quora 等低质量与登录墙平台模板已全部移除，
    // 内置源统一由 curated_feeds() 提供。简易“按平台生成”入口因此不再产出内置模板。
    Vec::new()
}

#[command]
pub async fn ensure_category_default_feeds(category: String, force: Option<bool>) -> Result<usize, String> {
    ensure_feeds_initialized();
    let force = force.unwrap_or(false);

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

    let mut feeds_lock = FEEDS.lock().unwrap();
    let has_covering_feed = feeds_lock.iter().any(|f| {
        f.enabled
            && (f.category.trim() == normalized_category.as_str() || f.category.trim() == "all")
    });
    if has_covering_feed && !force {
        return Ok(0);
    }

    let mut existing_keys: HashSet<String> = feeds_lock
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

    let mut created = 0usize;
    let mut seq = 0i64;
    for (name, url, source_type, cat) in curated_feeds() {
        if cat != normalized_category {
            continue;
        }
        let key = format!(
            "{}|{}|{}",
            url.trim().to_lowercase(),
            normalized_category.to_lowercase(),
            source_type.trim().to_lowercase()
        );
        if !existing_keys.insert(key) {
            continue;
        }

        feeds_lock.push(FeedSource {
            id: format!(
                "auto-{}-{}",
                normalized_category,
                Utc::now().timestamp_millis() + seq
            ),
            name: name.to_string(),
            url: url.to_string(),
            category: normalized_category.clone(),
            enabled: true,
            source_type: source_type.to_string(),
            fetch_interval: Some(60),
            include_keywords: None,
            exclude_keywords: None,
            remark: Some(format!(
                "auto_generated_for_category={}",
                normalized_category
            )),
            ..Default::default()
        });
        created += 1;
        seq += 1;
    }

    if created > 0 {
        save_feeds_to_config(&feeds_lock)?;
        info!(
            "flipboard: ensure_category_default_feeds created {} feeds for category {}",
            created, normalized_category
        );
    } else {
        warn!(
            "flipboard: ensure_category_default_feeds no templates created for category {}",
            normalized_category
        );
    }
    Ok(created)
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
            ..Default::default()
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
pub async fn get_article_detail(
    id: String,
    url: Option<String>,
    force: Option<bool>,
) -> Result<Article, String> {
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
    let force_reparse = force.unwrap_or(false);
    let need_hydrate = force_reparse
        || content.is_empty()
        || content.len() < 180
        || (!desc.is_empty() && content == desc);

    if !need_hydrate {
        return Ok(base_article);
    }

    match hydrate_article_readable_content(&base_article).await {
        Ok(enriched) => {
            {
                let mut lock = ARTICLES.lock().unwrap();
                if let Some(slot) = lock.iter_mut().find(|a| a.id == id_trim || a.url == enriched.url) {
                    *slot = enriched.clone();
                } else {
                    lock.push(enriched.clone());
                }
            }
            // 回写缓存：二次打开秒开 + 卡片列表后续直接有完整图文。
            if let Err(e) = persist_hydrated_article(&enriched) {
                warn!("flipboard: persist hydrated article failed: {}", e);
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

            // 搜索匹配（覆盖标题/摘要/正文/来源/LLM摘要/标签）。
            article.title.to_lowercase().contains(&query_lower)
                || article.description.to_lowercase().contains(&query_lower)
                || article.content.to_lowercase().contains(&query_lower)
                || article.source.to_lowercase().contains(&query_lower)
                || article.summary.to_lowercase().contains(&query_lower)
                || article
                    .tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(&query_lower))
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
                ..Default::default()
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
    #[serde(default)]
    pub interest_profile: Option<String>,
    #[serde(default)]
    pub relevance_threshold: Option<i32>,
    #[serde(default)]
    pub crawl_cfg: Option<String>,
    #[serde(default)]
    pub render_profile: Option<String>,
    #[serde(default)]
    pub llm_enhance: Option<bool>,
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
        interest_profile: input.interest_profile,
        relevance_threshold: input.relevance_threshold.unwrap_or(0),
        crawl_cfg: input.crawl_cfg,
        render_profile: input.render_profile,
        llm_enhance: input.llm_enhance.unwrap_or(false),
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
                        ..Default::default()
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
            ..Default::default()
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
        .or_else(|| website_markdown_candidates().into_iter().find(|p| p.exists()));
    let Some(path) = path else {
        info!("flipboard: website markdown not found, skip import");
        return Ok(Vec::new());
    };

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
            ..Default::default()
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
        target.interest_profile = input.interest_profile;
        if let Some(threshold) = input.relevance_threshold {
            target.relevance_threshold = threshold;
        }
        target.crawl_cfg = input.crawl_cfg;
        target.render_profile = input.render_profile;
        if let Some(enhance) = input.llm_enhance {
            target.llm_enhance = enhance;
        }

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

/// 批量删除订阅源。返回实际删除的数量；只保存一次配置。
#[command]
pub async fn delete_feeds(ids: Vec<String>) -> Result<usize, String> {
    ensure_feeds_initialized();
    let id_set: HashSet<String> = ids.into_iter().map(|s| s.trim().to_string()).collect();
    if id_set.is_empty() {
        return Ok(0);
    }

    let mut feeds_lock = FEEDS.lock().unwrap();
    let before = feeds_lock.len();
    feeds_lock.retain(|f| !id_set.contains(f.id.trim()));
    let removed = before - feeds_lock.len();

    if removed > 0 {
        if let Err(e) = save_feeds_to_config(&feeds_lock) {
            warn!("flipboard: save_feeds_to_config failed after delete_feeds: {}", e);
        }
    }
    info!("flipboard: delete_feeds removed {}, remaining {}", removed, feeds_lock.len());
    Ok(removed)
}

/// 批量启用/禁用订阅源。返回受影响的数量；只保存一次配置。
#[command]
pub async fn set_feeds_enabled(ids: Vec<String>, enabled: bool) -> Result<usize, String> {
    ensure_feeds_initialized();
    let id_set: HashSet<String> = ids.into_iter().map(|s| s.trim().to_string()).collect();
    if id_set.is_empty() {
        return Ok(0);
    }

    let mut feeds_lock = FEEDS.lock().unwrap();
    let mut changed = 0usize;
    for f in feeds_lock.iter_mut() {
        if id_set.contains(f.id.trim()) && f.enabled != enabled {
            f.enabled = enabled;
            changed += 1;
        }
    }
    if changed > 0 {
        if let Err(e) = save_feeds_to_config(&feeds_lock) {
            warn!("flipboard: save_feeds_to_config failed after set_feeds_enabled: {}", e);
        }
    }
    info!("flipboard: set_feeds_enabled changed {} to {}", changed, enabled);
    Ok(changed)
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

    // 关键词预筛（include/exclude）。
    let pre_filtered: Vec<Article> = if include.is_empty() && exclude.is_empty() {
        articles
    } else {
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
    };

    // 相关性打分 + 阈值过滤（P2 本地优先；P6 由 LLM 覆盖）。
    apply_relevance(pre_filtered, feed)
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

/// 本地相关性评分（0-100），基于兴趣描述里的关键词命中。
/// 这是 P2 的"本地优先"实现；P6 会用 LLM 打分覆盖更精细的语义相关性。
/// 无兴趣描述时返回 -1（未评分，不参与过滤）。
fn local_relevance_score(article: &Article, interest_profile: Option<&str>) -> i32 {
    let profile = match interest_profile {
        Some(p) if !p.trim().is_empty() => p.trim().to_lowercase(),
        _ => return -1,
    };
    // 把兴趣描述拆成词元（中英文混合：按空白/标点/常见分隔切分）。
    let tokens: Vec<String> = profile
        .split(|c: char| c.is_whitespace() || ",.，。、；;/|".contains(c))
        .map(|s| s.trim())
        .filter(|s| s.chars().count() >= 2)
        .map(|s| s.to_string())
        .collect();
    if tokens.is_empty() {
        return -1;
    }

    let title = article.title.to_lowercase();
    let description = article.description.to_lowercase();
    let content = article.content.to_lowercase();

    let mut hit_tokens = 0usize;
    let mut weighted = 0usize;
    for t in &tokens {
        let in_title = title.contains(t);
        let in_desc = description.contains(t);
        let in_content = content.contains(t);
        if in_title || in_desc || in_content {
            hit_tokens += 1;
        }
        weighted += if in_title { 5 } else { 0 };
        weighted += if in_desc { 3 } else { 0 };
        weighted += if in_content { 1 } else { 0 };
    }

    if hit_tokens == 0 {
        return 0;
    }
    // 覆盖率（命中词元比例）占主导，再叠加加权命中强度。
    let coverage = (hit_tokens as f64 / tokens.len() as f64) * 70.0;
    let intensity = (weighted as f64).min(30.0); // 上限 30
    (coverage + intensity).round().clamp(0.0, 100.0) as i32
}

/// 为一批文章打本地相关性分，并按阈值过滤。阈值 <=0 时只打分不过滤。
fn apply_relevance(mut articles: Vec<Article>, feed: &FeedSource) -> Vec<Article> {
    let profile = feed.interest_profile.as_deref();
    let threshold = feed.relevance_threshold;
    for a in articles.iter_mut() {
        let score = local_relevance_score(a, profile);
        a.relevance_score = score;
    }
    if threshold > 0 {
        articles.retain(|a| a.relevance_score < 0 || a.relevance_score >= threshold);
    }
    articles
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
    if base != "all" {
        return base;
    }
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

// 仍被站点发现等路径以及 P3 爬虫层使用。
#[allow(dead_code)]
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

#[allow(dead_code)]
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

/// 用站点 host 作为渲染会话 profile，使同站文章复用同一份持久登录态。
fn render_profile_for_url(url: &str) -> String {
    Url::parse(url.trim())
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_else(|| "default".to_string())
}

/// 登录墙启发式判断：抽取后正文极薄，且页面/URL 命中登录、验证、人机校验等特征。
fn looks_like_login_wall(html: &str, plain_text: &str, final_url: &str) -> bool {
    if plain_text.chars().filter(|c| !c.is_whitespace()).count() >= 300 {
        return false;
    }
    let hay = format!("{} {}", html, final_url).to_lowercase();
    const SIGNS: &[&str] = &[
        "请登录",
        "登录后",
        "登录查看",
        "登录后继续",
        "扫码登录",
        "请先登录",
        "需要登录",
        "captcha",
        "验证码",
        "人机验证",
        "安全验证",
        "您的访问出现异常",
        "sign in to continue",
        "please log in",
        "please sign in",
        "log in to continue",
        "/login",
        "/signin",
        "accounts.google.com",
        "as a paid subscriber",
        "subscribe to read",
    ];
    SIGNS.iter().any(|s| hay.contains(s))
}

async fn hydrate_article_readable_content(article: &Article) -> Result<Article, String> {
    // 混合抓取：HTTP 优先，JS 站/空壳/登录墙升级到无头 Chrome，并复用该站持久登录态。
    let profile = render_profile_for_url(&article.url);
    let rendered = crate::commands::flipboard_render::fetch_page(
        &HTTP_CLIENT,
        article.url.trim(),
        false,
        &profile,
    )
    .await?;
    let html = rendered.html;
    let final_url = rendered.final_url;

    // 工业级抽取：trafilatura（主）/ scraper（降级），保留内联图片。
    // worker 协议是同步 stdin/stdout，放到阻塞线程池执行。
    let base_url = article.url.clone();
    let html_for_extract = html.clone();
    let extracted = tokio::task::spawn_blocking(move || {
        crate::commands::flipboard_extract::extract(&html_for_extract, &base_url)
    })
    .await
    .map_err(|e| format!("extract task join failed: {e}"))?;

    let mut enriched = article.clone();

    match extracted {
        Ok(ex) => {
            if let Some(t) = ex.title.as_ref() {
                if !t.is_empty() && (enriched.title == "无标题" || enriched.title.trim().len() < 4) {
                    enriched.title = t.clone();
                }
            }
            enriched.clean_content_html = ex.clean_html.clone();
            enriched.clean_content_markdown = ex.markdown.clone();
            enriched.content_images = ex.images.clone();
            enriched.lead_image = ex.lead_image.clone();
            enriched.render_mode = ex.engine.to_string();
            if let Some(by) = ex.byline {
                if enriched.author.is_none() && !by.is_empty() {
                    enriched.author = Some(by);
                }
            }
            // content 作为纯文本镜像（搜索/旧字段兼容）。
            if !ex.plain_text.is_empty() {
                enriched.content = ex.plain_text.clone();
            }
            // description：优先 excerpt，其次正文截断。
            let desc = ex
                .excerpt
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    if !ex.plain_text.is_empty() {
                        Some(trim_to_chars(&ex.plain_text, 180))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| article.description.clone());
            enriched.description = desc;
            if enriched.image_url.is_none() {
                enriched.image_url = ex.lead_image.or_else(|| ex.images.first().cloned());
            }
        }
        Err(e) => {
            // 抽取彻底失败：退回到旧的 meta 描述兜底，避免空白。
            warn!("flipboard: extract failed for {}: {}", article.url, e);
            let document = Html::parse_document(&html);
            let lead = extract_meta_content(&document, "meta[property='og:description']")
                .or_else(|| extract_meta_content(&document, "meta[name='description']"))
                .unwrap_or_default();
            if !lead.is_empty() {
                enriched.description = lead;
            }
        }
    }

    enriched.category = classify_category_by_text_and_url(
        &enriched.category,
        &enriched.title,
        &enriched.description,
        &enriched.url,
        &enriched.source,
    );

    // 登录墙判定：正文抽取后仍极薄且命中登录/验证特征，标记需登录供前端引导。
    enriched.login_required = looks_like_login_wall(&html, &enriched.content, &final_url);
    if enriched.login_required {
        warn!(
            "flipboard: login wall detected for {} (final={})",
            article.url, final_url
        );
        // 首次命中即主动弹出登录窗口（每 host 一次），用户登录后再次打开即可正常抓取。
        maybe_prompt_login(&article.url);
    }

    Ok(enriched)
}

#[command]
pub async fn fetch_knowledge_from_feed_ids(
    feed_ids: Vec<String>,
    topic: Option<String>,
    category: Option<String>,
    limit: Option<usize>,
    force: Option<bool>,
) -> Result<Vec<Article>, String> {
    ensure_feeds_initialized();
    // 仅在用户主动强制刷新时清除历史“停止”标记；普通加载不清，使停止能持续生效。
    if force == Some(true) {
        reset_task_cancellation();
    }

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
    let normalized_category = category
        .as_ref()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty() && c != "all");
    let category_ref = normalized_category.as_deref();
    let topic_tokens: Vec<String> = topic_text
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(|x| x.trim().to_lowercase())
        .filter(|x| !x.is_empty())
        .collect();

    let feed_id_list: Vec<String> = feeds.iter().map(|f| f.id.clone()).collect();
    let mut out = load_cached_articles_by_feed_ids(&feed_id_list, category_ref, max_limit).unwrap_or_default();
    let now = unix_now();

    for feed in &feeds {
        // 用户随时可终止：命中则跳过剩余源，前端立即解除等待。
        if is_task_cancelled(&feed.id) {
            continue;
        }
        if !force {
            let has_cache = has_cached_articles_for_feed(&feed.id).unwrap_or(false);
            let has_category_cache = if let Some(cat) = category_ref {
                load_cached_articles_by_feed_ids(&vec![feed.id.clone()], Some(cat), 1)
                    .map(|list| !list.is_empty())
                    .unwrap_or(false)
            } else {
                has_cache
            };
            let due = should_run_feed(read_scheduler_state(&feed.id).as_ref(), now, false);
            if has_cache && has_category_cache && !due {
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
    if let Some(cat) = category_ref {
        out.retain(|a| a.category.trim() == cat || a.category.trim() == "all");
    }

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
            ..Default::default()
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
                ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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

// ===== 整站深度抓取（site_crawl）爬虫管线 =====

#[derive(Debug, Clone)]
struct CrawlConfig {
    max_depth: u32,
    max_pages_per_tick: u32,
    max_queue: u32,
    same_domain_only: bool,
    rate_delay_ms: u64,
    respect_robots: bool,
}

impl Default for CrawlConfig {
    fn default() -> Self {
        CrawlConfig {
            max_depth: 3,
            max_pages_per_tick: 16,
            max_queue: 2000,
            same_domain_only: true,
            rate_delay_ms: 400,
            respect_robots: true,
        }
    }
}

/// 单次 tick 内并行抓取/解析的页面数（在 rate_delay 之上再叠加并发以提速）。
const CRAWL_PAGE_CONCURRENCY: usize = 4;

impl CrawlConfig {
    /// 从 feed.crawl_cfg(JSON) 解析配置，缺省回退默认。
    fn from_feed(feed: &FeedSource) -> Self {
        let mut cfg = CrawlConfig::default();
        if let Some(raw) = feed.crawl_cfg.as_deref() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) {
                if let Some(x) = v.get("max_depth").and_then(|x| x.as_u64()) {
                    cfg.max_depth = x.clamp(1, 8) as u32;
                }
                if let Some(x) = v.get("max_pages_per_tick").and_then(|x| x.as_u64()) {
                    cfg.max_pages_per_tick = x.clamp(1, 64) as u32;
                }
                if let Some(x) = v.get("max_queue").and_then(|x| x.as_u64()) {
                    cfg.max_queue = x.clamp(10, 100_000) as u32;
                }
                if let Some(x) = v.get("same_domain_only").and_then(|x| x.as_bool()) {
                    cfg.same_domain_only = x;
                }
                if let Some(x) = v.get("rate_delay_ms").and_then(|x| x.as_u64()) {
                    cfg.rate_delay_ms = x.clamp(0, 60_000);
                }
                if let Some(x) = v.get("respect_robots").and_then(|x| x.as_bool()) {
                    cfg.respect_robots = x;
                }
            }
        }
        cfg
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CrawlQueueStats {
    pub pending: u32,
    pub done: u32,
    pub error: u32,
    pub total: u32,
}

#[derive(Debug, Clone)]
struct CrawlItem {
    url: String,
    depth: u32,
}

/// 把种子 URL 写入队列（depth 0）。
fn seed_crawl_queue(conn: &Connection, feed_id: &str, seed_url: &str) -> Result<(), String> {
    let now = unix_now();
    conn.execute(
        "INSERT OR IGNORE INTO crawl_queue (feed_id, url, depth, status, kind, discovered_at) \
         VALUES (?1, ?2, 0, 'pending', 'page', ?3)",
        params![feed_id, seed_url, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 队列里某 feed 的待抓取条数。
fn crawl_queue_pending_count(conn: &Connection, feed_id: &str) -> Result<u32, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM crawl_queue WHERE feed_id = ?1 AND status = 'pending'",
        params![feed_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|n| n as u32)
    .map_err(|e| e.to_string())
}

fn crawl_queue_total(conn: &Connection, feed_id: &str) -> Result<u32, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM crawl_queue WHERE feed_id = ?1",
        params![feed_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|n| n as u32)
    .map_err(|e| e.to_string())
}

/// 取出最多 budget 条 pending，并在事务内标记 fetching，避免并发重复处理。
fn claim_pending(conn: &mut Connection, feed_id: &str, budget: u32) -> Result<Vec<CrawlItem>, String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let items: Vec<CrawlItem> = {
        let mut stmt = tx
            .prepare(
                "SELECT url, depth FROM crawl_queue \
                 WHERE feed_id = ?1 AND status = 'pending' \
                 ORDER BY depth ASC, discovered_at ASC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![feed_id, budget as i64], |row| {
                Ok(CrawlItem {
                    url: row.get(0)?,
                    depth: row.get::<_, i64>(1)? as u32,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };
    let now = unix_now();
    for item in &items {
        tx.execute(
            "UPDATE crawl_queue SET status = 'fetching', fetched_at = ?3 \
             WHERE feed_id = ?1 AND url = ?2",
            params![feed_id, item.url, now],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(items)
}

fn mark_crawl_done(conn: &Connection, feed_id: &str, url: &str) {
    let _ = conn.execute(
        "UPDATE crawl_queue SET status = 'done', error = NULL WHERE feed_id = ?1 AND url = ?2",
        params![feed_id, url],
    );
}

fn mark_crawl_error(conn: &Connection, feed_id: &str, url: &str, err: &str) {
    let _ = conn.execute(
        "UPDATE crawl_queue SET status = 'error', error = ?3 WHERE feed_id = ?1 AND url = ?2",
        params![feed_id, url, err],
    );
}

/// 把页面内发现的链接入队（同域过滤 + 噪声过滤 + 深度/队列上限）。
fn enqueue_links(
    conn: &Connection,
    feed_id: &str,
    parent_depth: u32,
    base: &Url,
    links: &[String],
    cfg: &CrawlConfig,
) -> usize {
    if parent_depth + 1 > cfg.max_depth {
        return 0;
    }
    let total = crawl_queue_total(conn, feed_id).unwrap_or(0);
    if total >= cfg.max_queue {
        return 0;
    }
    let mut budget_left = (cfg.max_queue - total) as usize;
    let now = unix_now();
    let mut added = 0usize;
    let base_host = base.host_str().unwrap_or("").to_lowercase();

    for raw in links {
        if budget_left == 0 {
            break;
        }
        let abs = match resolve_url(base, raw) {
            Some(u) => u,
            None => continue,
        };
        let parsed = match Url::parse(&abs) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            continue;
        }
        if cfg.same_domain_only {
            let host = parsed.host_str().unwrap_or("").to_lowercase();
            if host != base_host && !host.ends_with(&format!(".{}", base_host)) {
                continue;
            }
        }
        if is_noise_crawl_path(parsed.path()) {
            continue;
        }
        // 规范化：去掉 fragment 与常见跟踪参数。
        let normalized = normalize_crawl_url(&parsed);
        let kind = classify_crawl_url(&normalized);
        let res = conn.execute(
            "INSERT OR IGNORE INTO crawl_queue (feed_id, url, depth, status, kind, discovered_at) \
             VALUES (?1, ?2, ?3, 'pending', ?4, ?5)",
            params![feed_id, normalized, (parent_depth + 1) as i64, kind, now],
        );
        if let Ok(n) = res {
            if n > 0 {
                added += 1;
                budget_left -= 1;
            }
        }
    }
    added
}

/// 去掉 fragment 和 utm_* 等跟踪参数，便于去重。
fn normalize_crawl_url(parsed: &Url) -> String {
    let mut u = parsed.clone();
    u.set_fragment(None);
    let kept: Vec<(String, String)> = u
        .query_pairs()
        .filter(|(k, _)| {
            let k = k.to_lowercase();
            !(k.starts_with("utm_") || k == "spm" || k == "ref" || k == "from")
        })
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if kept.is_empty() {
        u.set_query(None);
    } else {
        let qs = kept
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        u.set_query(Some(&qs));
    }
    u.to_string()
}

/// 启发式判断 URL 像文章页还是导航页。
fn classify_crawl_url(url: &str) -> &'static str {
    let path = Url::parse(url)
        .ok()
        .map(|u| u.path().to_lowercase())
        .unwrap_or_default();
    // 含数字 id 段 / 带 slug 的较深路径 → 文章页。
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let has_id = segs.iter().any(|s| s.chars().any(|c| c.is_ascii_digit()) && s.len() >= 4);
    let has_slug = segs.iter().any(|s| s.contains('-') && s.len() >= 8);
    if has_id || has_slug || segs.len() >= 3 {
        "article"
    } else {
        "page"
    }
}

/// 爬虫噪声路径过滤：登录/标签/分类/搜索/静态资源等不抓正文。
fn is_noise_crawl_path(path: &str) -> bool {
    let p = path.to_lowercase();
    p.starts_with("/tag/")
        || p.starts_with("/tags/")
        || p.starts_with("/category/")
        || p.starts_with("/categories/")
        || p.starts_with("/login")
        || p.starts_with("/signin")
        || p.starts_with("/signup")
        || p.starts_with("/register")
        || p.starts_with("/search")
        || p.starts_with("/cart")
        || p.starts_with("/account")
        || p.starts_with("/wp-admin")
        || p.starts_with("/wp-login")
        || p.ends_with(".css")
        || p.ends_with(".js")
        || p.ends_with(".png")
        || p.ends_with(".jpg")
        || p.ends_with(".jpeg")
        || p.ends_with(".gif")
        || p.ends_with(".svg")
        || p.ends_with(".ico")
        || p.ends_with(".pdf")
        || p.ends_with(".zip")
}

/// 抓取 robots.txt 并解析 sitemap，返回发现的 URL（best-effort）。
/// 不持有数据库连接，避免跨 await 的 !Send 问题；入队由调用方完成。
async fn expand_from_sitemap(base: &Url, cfg: &CrawlConfig) -> Vec<String> {
    // robots.txt
    let robots_url = match base.join("/robots.txt") {
        Ok(u) => u,
        Err(_) => return Vec::new(),
    };
    let mut sitemaps: Vec<String> = Vec::new();
    if let Ok(resp) = HTTP_CLIENT.get(robots_url).send().await {
        if resp.status().is_success() {
            if let Ok(text) = resp.text().await {
                for line in text.lines() {
                    let l = line.trim();
                    if l.to_lowercase().starts_with("sitemap:") {
                        if let Some(idx) = l.find(':') {
                            let url = l[idx + 1..].trim().to_string();
                            if url.starts_with("http") {
                                sitemaps.push(url);
                            }
                        }
                    }
                }
            }
        }
    }
    // 默认尝试 /sitemap.xml
    if sitemaps.is_empty() {
        if let Ok(u) = base.join("/sitemap.xml") {
            sitemaps.push(u.to_string());
        }
    }

    let mut discovered: Vec<String> = Vec::new();
    for sm in sitemaps.iter().take(5) {
        collect_sitemap_locs(sm, &mut discovered, 0).await;
        if discovered.len() >= cfg.max_queue as usize {
            break;
        }
    }
    discovered
}

/// 递归解析 sitemap（含 sitemap-index 一层），收集 <loc>。
async fn collect_sitemap_locs(sitemap_url: &str, out: &mut Vec<String>, depth: u32) {
    if depth > 1 || out.len() >= 5000 {
        return;
    }
    let resp = match HTTP_CLIENT.get(sitemap_url).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => return,
    };
    let body = match resp.text().await {
        Ok(b) => b,
        Err(_) => return,
    };
    let is_index = body.contains("<sitemapindex");
    // 简单提取 <loc>...</loc>
    let mut locs = Vec::new();
    for cap in body.split("<loc>").skip(1) {
        if let Some(end) = cap.find("</loc>") {
            let loc = cap[..end].trim().to_string();
            if loc.starts_with("http") {
                locs.push(loc);
            }
        }
    }
    if is_index {
        for loc in locs.into_iter().take(20) {
            Box::pin(collect_sitemap_locs(&loc, out, depth + 1)).await;
            if out.len() >= 5000 {
                break;
            }
        }
    } else {
        out.extend(locs);
    }
}

/// 提取页面内的所有链接（<a href>）。
fn extract_page_links(html: &str, base: &Url) -> Vec<String> {
    let document = Html::parse_document(html);
    let mut links = Vec::new();
    if let Ok(sel) = Selector::parse("a[href]") {
        for a in document.select(&sel) {
            if let Some(href) = a.value().attr("href") {
                if let Some(abs) = resolve_url(base, href) {
                    links.push(abs);
                }
            }
        }
    }
    links
}

/// 一次调度 tick 内对某个 site_crawl feed 的抓取：取预算条 URL，渲染→抽取→相关性，
/// 并把页内链接入队。返回本批新文章。
async fn crawl_tick_for_feed(feed: &FeedSource) -> Result<Vec<Article>, String> {
    // 该源被请求终止：消费一次性标记并跳过本轮。
    if is_task_cancelled(&feed.id) {
        clear_feed_cancel(&feed.id);
        return Ok(Vec::new());
    }
    let cfg = CrawlConfig::from_feed(feed);
    let base = Url::parse(feed.url.trim())
        .or_else(|_| Url::parse(&format!("https://{}", feed.url.trim())))
        .map_err(|e| format!("invalid seed url: {}", e))?;

    // 1. 队列为空：种子 + sitemap 展开。
    //    注意：Connection 是 !Sync，绝不能跨 .await 持有，否则 future 非 Send。
    let need_sitemap = {
        let conn = open_article_cache_db()?;
        let pending = crawl_queue_pending_count(&conn, &feed.id).unwrap_or(0);
        let total = crawl_queue_total(&conn, &feed.id).unwrap_or(0);
        if total == 0 {
            seed_crawl_queue(&conn, &feed.id, &normalize_crawl_url(&base))?;
        }
        pending == 0 && total <= 1
    };
    if need_sitemap {
        // 首次：尝试 sitemap 展开（best-effort）。先异步收集，再开连接入队。
        let discovered = expand_from_sitemap(&base, &cfg).await;
        if !discovered.is_empty() {
            let conn = open_article_cache_db()?;
            enqueue_links(&conn, &feed.id, 0, &base, &discovered, &cfg);
        }
    }

    // 2. 取预算条 pending。
    let items = {
        let mut conn = open_article_cache_db()?;
        claim_pending(&mut conn, &feed.id, cfg.max_pages_per_tick)?
    };
    if items.is_empty() {
        return Ok(Vec::new());
    }

    // 3. 并发处理本批页面（在 rate_delay 之上再叠加 CRAWL_PAGE_CONCURRENCY 并发提速）。
    // profile 默认按站点 host 区分，使其与"自动登录"窗口保存 cookie 的目录一致，登录后即可复用。
    let render_profile = feed
        .render_profile
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| render_profile_for_url(&feed.url));
    let base_ref = &base;
    let cfg_ref = &cfg;
    let profile_ref = render_profile.as_str();
    let produced: Vec<Article> = futures_util::stream::iter(items.into_iter())
        .map(|item| async move {
            process_crawl_item(feed, base_ref, cfg_ref, profile_ref, item).await
        })
        .buffer_unordered(CRAWL_PAGE_CONCURRENCY)
        .filter_map(|x| async move { x })
        .collect()
        .await;

    Ok(produced)
}

/// 处理单个爬取页面：渲染→抽取→相关性/LLM→入队链接→标记队列状态。
/// 自行处理所有错误（标记 crawl error），返回 None；产出文章时返回 Some(Article)。
/// 不跨 await 持有 SQLite 连接（!Sync），保证 future Send，可并发。
async fn process_crawl_item(
    feed: &FeedSource,
    base: &Url,
    cfg: &CrawlConfig,
    render_profile: &str,
    item: CrawlItem,
) -> Option<Article> {
    // 用户已请求终止该任务：立即跳过，不再渲染抓取。
    if is_task_cancelled(&feed.id) {
        return None;
    }
    if cfg.rate_delay_ms > 0 {
        sleep(Duration::from_millis(cfg.rate_delay_ms)).await;
    }
    let _ = cfg.respect_robots; // robots 在 sitemap 阶段已用；逐页校验后续补强。

    let page_url = item.url.clone();
    // 混合抓取：HTTP 优先，JS 站/空壳升级无头 Chrome 渲染。
    let rendered = crate::commands::flipboard_render::fetch_page(
        &HTTP_CLIENT,
        &page_url,
        false,
        render_profile,
    )
    .await;
    let (html, render_mode) = match rendered {
        Ok(page) => {
            if page.blocked {
                // 命中登录墙/验证码：标记队列并记录健康，提示用户为该源登录。
                if let Ok(conn) = open_article_cache_db() {
                    mark_crawl_error(
                        &conn,
                        &feed.id,
                        &page_url,
                        "blocked: 需要登录或人工验证（请为该源点击\"登录\"）",
                    );
                }
                record_feed_health_failure(
                    feed,
                    "blocked",
                    "命中登录墙/验证码，请为该源登录后重试".to_string(),
                );
                // 首次命中即主动弹出登录窗口（每 host 一次），无需用户手动找入口。
                maybe_prompt_login(&page_url);
                return None;
            }
            (page.html, page.mode)
        }
        Err(e) => {
            if let Ok(conn) = open_article_cache_db() {
                mark_crawl_error(&conn, &feed.id, &page_url, &format!("render: {}", e));
            }
            return None;
        }
    };

    // 入队页内链接（BFS 扩展）。
    let links = extract_page_links(&html, base);
    if let Ok(conn) = open_article_cache_db() {
        enqueue_links(&conn, &feed.id, item.depth, base, &links, cfg);
    }

    // 导航页只扩链接、不抽正文。
    if !item.kind_is_article() {
        if let Ok(conn) = open_article_cache_db() {
            mark_crawl_done(&conn, &feed.id, &page_url);
        }
        return None;
    }

    let base_url = page_url.clone();
    let html_for_extract = html.clone();
    let extracted = match tokio::task::spawn_blocking(move || {
        crate::commands::flipboard_extract::extract(&html_for_extract, &base_url)
    })
    .await
    {
        Ok(r) => r,
        Err(e) => {
            if let Ok(conn) = open_article_cache_db() {
                mark_crawl_error(&conn, &feed.id, &page_url, &format!("extract join: {e}"));
            }
            return None;
        }
    };

    match extracted {
        Ok(ex) if !ex.clean_html.trim().is_empty() => {
            let mut article = Article {
                id: format!("crawl-{:x}", md5_like_hash(&page_url)),
                title: ex.title.clone().unwrap_or_else(|| "无标题".to_string()),
                description: ex
                    .excerpt
                    .clone()
                    .unwrap_or_else(|| trim_to_chars(&ex.plain_text, 180)),
                content: ex.plain_text.clone(),
                source: feed.name.clone(),
                url: page_url.clone(),
                image_url: ex.lead_image.clone().or_else(|| ex.images.first().cloned()),
                published_at: ex
                    .published
                    .clone()
                    .unwrap_or_else(|| Utc::now().to_rfc3339()),
                category: feed.category.clone(),
                author: ex.byline.clone(),
                clean_content_html: ex.clean_html.clone(),
                clean_content_markdown: ex.markdown.clone(),
                lead_image: ex.lead_image.clone(),
                content_images: ex.images.clone(),
                render_mode: format!("{}+{}", render_mode, ex.engine),
                ..Default::default()
            };
            article.category = classify_category_by_text_and_url(
                &article.category,
                &article.title,
                &article.description,
                &article.url,
                &article.source,
            );

            // LLM 增强（可选）：摘要 / 标签 / 相关性 / 二次清洗。
            if feed.llm_enhance && crate::commands::flipboard_llm::has_budget() {
                let interest = feed.interest_profile.clone().unwrap_or_default();
                if let Some(llm) = crate::commands::flipboard_llm::enhance(
                    &article.title,
                    &article.clean_content_markdown,
                    &interest,
                )
                .await
                {
                    if !llm.summary.is_empty() {
                        article.summary = llm.summary;
                    }
                    if !llm.tags.is_empty() {
                        article.tags = llm.tags;
                    }
                    if llm.relevance >= 0 {
                        article.relevance_score = llm.relevance;
                    }
                    if let Some(cleaned) = llm.cleaned_markdown {
                        article.clean_content_markdown = cleaned.clone();
                        article.clean_content_html =
                            crate::commands::flipboard_extract::markdown_to_html(&cleaned, &page_url);
                    }
                }
            }
            // 本地相关性兜底（LLM 未给分时）。
            if article.relevance_score < 0 {
                article.relevance_score =
                    local_relevance_score(&article, feed.interest_profile.as_deref());
            }
            // 阈值过滤。
            if feed.relevance_threshold > 0
                && article.relevance_score >= 0
                && article.relevance_score < feed.relevance_threshold
            {
                if let Ok(conn) = open_article_cache_db() {
                    mark_crawl_done(&conn, &feed.id, &page_url);
                }
                return None;
            }

            if let Ok(conn) = open_article_cache_db() {
                mark_crawl_done(&conn, &feed.id, &page_url);
            }
            Some(article)
        }
        Ok(_) => {
            if let Ok(conn) = open_article_cache_db() {
                mark_crawl_done(&conn, &feed.id, &page_url);
            }
            None
        }
        Err(e) => {
            if let Ok(conn) = open_article_cache_db() {
                mark_crawl_error(&conn, &feed.id, &page_url, &e);
            }
            None
        }
    }
}

/// 轻量 URL 哈希，用于生成稳定的文章 id（非加密）。
fn md5_like_hash(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// 查询某 feed 的爬虫队列统计。
fn read_crawl_queue_stats(feed_id: &str) -> Result<CrawlQueueStats, String> {
    let conn = open_article_cache_db()?;
    let mut stats = CrawlQueueStats::default();
    let mut stmt = conn
        .prepare("SELECT status, COUNT(*) FROM crawl_queue WHERE feed_id = ?1 GROUP BY status")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![feed_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u32))
        })
        .map_err(|e| e.to_string())?;
    for r in rows {
        if let Ok((status, count)) = r {
            stats.total += count;
            match status.as_str() {
                "pending" | "fetching" => stats.pending += count,
                "done" => stats.done += count,
                "error" => stats.error += count,
                _ => {}
            }
        }
    }
    Ok(stats)
}

/// 创建一个整站深度抓取源并把种子 URL 入队。
#[command]
pub async fn start_site_crawl(
    seed_url: String,
    category: Option<String>,
    interest: Option<String>,
    crawl_cfg: Option<String>,
    render_profile: Option<String>,
) -> Result<FeedSource, String> {
    ensure_feeds_initialized();

    let base = Url::parse(seed_url.trim())
        .or_else(|_| Url::parse(&format!("https://{}", seed_url.trim())))
        .map_err(|e| format!("无效主站 URL: {}", e))?;
    let host = base.host_str().unwrap_or("site").to_string();
    let category = category
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "all".to_string());

    let feed = FeedSource {
        id: format!("crawl-{:x}", md5_like_hash(&format!("{}|{}", base, category))),
        name: format!("整站抓取 · {}", host),
        url: base.to_string(),
        category,
        enabled: true,
        source_type: "site_crawl".to_string(),
        fetch_interval: Some(30),
        interest_profile: interest.filter(|s| !s.trim().is_empty()),
        crawl_cfg: crawl_cfg.filter(|s| !s.trim().is_empty()),
        render_profile: render_profile.filter(|s| !s.trim().is_empty()),
        remark: Some(format!("site_crawl_seed={}", base)),
        ..Default::default()
    };

    {
        let mut feeds_lock = FEEDS.lock().unwrap();
        if let Some(existing) = feeds_lock.iter_mut().find(|f| f.id == feed.id) {
            *existing = feed.clone();
        } else {
            feeds_lock.push(feed.clone());
        }
        if let Err(e) = save_feeds_to_config(&feeds_lock) {
            warn!("flipboard: save_feeds_to_config failed after start_site_crawl: {}", e);
        }
    }

    // 立刻种子入队（首次 tick 会做 sitemap 展开）。
    {
        let conn = open_article_cache_db()?;
        seed_crawl_queue(&conn, &feed.id, &normalize_crawl_url(&base))?;
    }

    info!("flipboard: start_site_crawl created feed {} for {}", feed.id, base);
    Ok(feed)
}

/// 查询整站抓取进度。
#[command]
pub async fn get_crawl_progress(feed_id: String) -> Result<CrawlQueueStats, String> {
    read_crawl_queue_stats(feed_id.trim())
}

/// 重置某 feed 的爬虫队列（清空已发现链接，便于重爬）。
#[command]
pub async fn reset_crawl_queue(feed_id: String) -> Result<(), String> {
    // 重爬即重新启用该源：清除其“停止”标记。
    clear_feed_cancel(&feed_id);
    let conn = open_article_cache_db()?;
    conn.execute(
        "DELETE FROM crawl_queue WHERE feed_id = ?1",
        params![feed_id.trim()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 确保无头 Chromium 就绪（首次会触发下载），返回可执行路径。
#[command]
pub async fn ensure_chromium_ready() -> Result<String, String> {
    crate::commands::flipboard_render::ensure_chromium_ready().await
}

/// 打开可见登录窗口供用户手动登录一次；会话(cookie)持久化到该 profile。
/// profile 一般传 feed.id 或站点名。
#[command]
pub async fn open_login_session(profile: String, login_url: String) -> Result<(), String> {
    crate::commands::flipboard_render::open_login_window(profile.trim(), login_url.trim())
        .await
        .map_err(|e| {
            warn!("flipboard: open_login_session failed: {}", e);
            if e.contains("launch chrome failed") || e.contains("status") {
                "无法启动浏览器登录窗口。请确认已安装 Chrome 或 Edge（程序会优先使用系统浏览器）；\
                 知乎/小红书等站点有强反爬风控，即使登录也可能无法稳定抓取，建议改用 RSS 源。"
                    .to_string()
            } else {
                e
            }
        })
}

impl CrawlItem {
    fn kind_is_article(&self) -> bool {
        classify_crawl_url(&self.url) == "article"
    }
}

async fn collect_articles_for_feed(feed: &FeedSource) -> Result<Vec<Article>, String> {
    let source_type = feed.source_type.trim().to_lowercase();

    // 整站深度抓取：走专门的爬虫管线（持久化队列 + BFS + 抽取）。
    if source_type == "site_crawl" {
        return match crawl_tick_for_feed(feed).await {
            Ok(list) => {
                let out = filter_articles_by_keywords(list, feed);
                record_feed_health_success(feed, "site_crawl", out.len());
                Ok(out)
            }
            Err(e) => {
                record_feed_health_failure(feed, "site_crawl", e.clone());
                Ok(Vec::new())
            }
        };
    }

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
            let res = match collect_articles_for_feed(&effective_feed).await {
                Ok(mut list) => {
                    // 后台预热（两者结合模型）：非整站源对最相关的少量"内容薄"文章
                    // 提前深解析，让用户点开即有现成全文。site_crawl 已产出全文，跳过。
                    if effective_feed.source_type.trim().to_lowercase() != "site_crawl" {
                        preheat_thin_articles(&mut list, &effective_feed, 2).await;
                    }
                    Ok(list)
                }
                Err(e) => Err(e),
            };
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
        ..Default::default()
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
        let article = get_article_detail(id.to_string(), None, None).await?;
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

/// 在内置浏览器窗口里打开文章原网页（真实页面，人类友好浏览）。
/// 复用各处已用的 WebviewWindowBuilder + External URL 模式。
#[command]
pub async fn open_article_in_browser(app: AppHandle, url: String) -> Result<(), String> {
    let url = url.trim().to_string();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("无效的文章 URL".to_string());
    }
    let external = url
        .parse()
        .map_err(|e| format!("解析 URL 失败: {}", e))?;
    let label = format!("article-browser-{:x}", md5_like_hash(&url));

    if let Some(w) = app.get_webview_window(&label) {
        w.show().ok();
        w.set_focus().ok();
        return Ok(());
    }

    let source_url = url.clone();
    let app_for_nav = app.clone();
    let window = WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(external))
        .title(&url)
        .inner_size(1100.0, 800.0)
        .min_inner_size(820.0, 560.0)
        // 注入双击采集脚本：在外部网页里双击段落/图片/视频即可发送到主窗口的「我的收集」。
        .initialization_script(EXTERNAL_DBLCLICK_SCRIPT)
        // 拦截 guaxinote:// 伪导航：解析采集载荷 → 转发事件给主窗口 → 取消导航。
        .on_navigation(move |nav_url| {
            let s = nav_url.as_str();
            if let Some(rest) = s.strip_prefix("guaxinote://capture/") {
                if let Ok(decoded) = urlencoding_decode(rest) {
                    if let Ok(mut payload) =
                        serde_json::from_str::<serde_json::Value>(&decoded)
                    {
                        if let Some(obj) = payload.as_object_mut() {
                            obj.insert(
                                "sourceUrl".to_string(),
                                serde_json::Value::String(source_url.clone()),
                            );
                        }
                        let _ = app_for_nav.emit("flipboard-external-note", payload);
                    }
                }
                return false; // 取消导航，页面保持原样。
            }
            true
        })
        .build()
        .map_err(|e| format!("创建浏览器窗口失败: {}", e))?;
    window.set_focus().ok();
    Ok(())
}

/// 注入到“浏览器打开原文”窗口的脚本：双击段落/图片/视频 → 通过 guaxinote:// 伪导航
/// 把采集内容发回主程序。不向外部页面暴露任何 Tauri 命令，安全可控。
const EXTERNAL_DBLCLICK_SCRIPT: &str = r#"
(function () {
  if (window.__guaxiNoteCaptureInstalled) return;
  window.__guaxiNoteCaptureInstalled = true;

  // 注入交互视觉：文字选中高亮 + 段落/句子悬停高亮，提示可双击采集。
  function injectStyle() {
    try {
      if (document.getElementById('__guaxi_capture_style')) return;
      var st = document.createElement('style');
      st.id = '__guaxi_capture_style';
      st.textContent =
        '::selection{background:rgba(37,99,235,.28)!important;color:inherit!important;}' +
        '.__guaxi_hover{outline:2px solid rgba(37,99,235,.55)!important;' +
        'outline-offset:2px;background:rgba(37,99,235,.06)!important;cursor:copy!important;border-radius:4px;}';
      (document.head || document.documentElement).appendChild(st);
    } catch (e) {}
  }
  if (document.head) injectStyle();
  document.addEventListener('DOMContentLoaded', injectStyle);

  function toast(text, ok) {
    try {
      var d = document.createElement('div');
      d.textContent = text;
      d.style.cssText = 'position:fixed;right:20px;bottom:20px;z-index:2147483647;'
        + 'background:' + (ok === false ? 'rgba(239,68,68,.96)' : 'rgba(34,197,94,.96)') + ';'
        + 'color:#fff;padding:8px 16px;border-radius:999px;'
        + 'font-size:13px;box-shadow:0 8px 24px rgba(22,163,74,.45);font-family:sans-serif;';
      document.body.appendChild(d);
      setTimeout(function(){ d.remove(); }, 1400);
    } catch (e) {}
  }

  var blocks = { P:1, H1:1, H2:1, H3:1, H4:1, H5:1, LI:1, BLOCKQUOTE:1, PRE:1, FIGURE:1, ARTICLE:1, SECTION:1, TD:1 };
  function nearestBlock(node) {
    var el = node;
    while (el && el !== document.body && !blocks[el.tagName]) el = el.parentElement;
    return (el && el !== document.body) ? el : null;
  }

  // 悬停高亮当前块。
  var hovered = null;
  document.addEventListener('mouseover', function (ev) {
    var b = nearestBlock(ev.target);
    if (b === hovered) return;
    if (hovered) hovered.classList.remove('__guaxi_hover');
    hovered = b;
    if (hovered) hovered.classList.add('__guaxi_hover');
  }, true);
  document.addEventListener('mouseout', function () {
    if (hovered) { hovered.classList.remove('__guaxi_hover'); hovered = null; }
  }, true);

  function send(payload) {
    try {
      var enc = encodeURIComponent(JSON.stringify(payload));
      // 顶层导航到自定义 scheme：Rust 的 on_navigation 会拦截并取消，页面不会真跳转。
      // （iframe 子框架的未知 scheme 不会触发 on_navigation，故必须用顶层导航。）
      window.location.href = 'guaxinote://capture/' + enc;
      toast('已添加到「我的收集」');
    } catch (e) { toast('采集失败', false); }
  }

  document.addEventListener('dblclick', function (ev) {
    var sel = (window.getSelection && window.getSelection().toString() || '').trim();
    if (sel) { send({ type: 'text', content: sel, originalMarkdown: sel }); return; }

    var t = ev.target;
    if (t && t.tagName === 'IMG' && t.src) {
      send({ type: 'image', content: t.src, originalMarkdown: '![' + (t.alt || '图片') + '](' + t.src + ')' });
      return;
    }
    var el = nearestBlock(t);
    if (!el) return;

    var media = el.querySelector && el.querySelector('iframe[src], video source[src], video[src]');
    var murl = media && (media.src || (media.getAttribute && media.getAttribute('src')));
    if (murl) { send({ type: 'video', content: murl, originalMarkdown: '[▶ 视频](' + murl + ')' }); return; }

    var text = (el.innerText || el.textContent || '').trim();
    var img = el.querySelector && el.querySelector('img[src]');
    if (img && img.src && text.length < 12) {
      send({ type: 'image', content: img.src, originalMarkdown: '![' + (img.alt || '图片') + '](' + img.src + ')' });
      return;
    }
    if (text) { send({ type: 'text', content: text, originalMarkdown: text }); }
  }, true);
})();
"#;

/// 轻量 percent-decode（只处理 %XX 与 + 号），用于解析 guaxinote:// 载荷。
fn urlencoding_decode(s: &str) -> Result<String, String> {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                match (hi, lo) {
                    (Some(h), Some(l)) => {
                        out.push((h * 16 + l) as u8);
                        i += 3;
                    }
                    _ => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|e| e.to_string())
}
