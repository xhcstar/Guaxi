//! 地图数据的本地 SQLite 缓存与每日额度统计。
//!
//! 高德个人认证的关键字/周边搜索只有约 100 次/天，路线与地理编码约 5000 次/天。
//! 这层缓存让「同一城市 + 同一分类」的检索跨进程复用，避免反复消耗配额；
//! 同时记录每日实际发出的请求数，设置页可以直观看到还剩多少额度。

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::store;

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|item| item.as_secs() as i64)
        .unwrap_or(0)
}

fn open() -> Result<Connection, String> {
    let path: PathBuf = store::travel_root()?.join("cache.sqlite3");
    let conn = Connection::open(path).map_err(|e| format!("打开地图缓存失败: {e}"))?;
    conn.busy_timeout(Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        CREATE TABLE IF NOT EXISTS cache (
          key TEXT PRIMARY KEY,
          provider TEXT NOT NULL,
          kind TEXT NOT NULL,
          url TEXT NOT NULL,
          payload TEXT NOT NULL,
          created_at INTEGER NOT NULL,
          expires_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_cache_expires ON cache(expires_at);
        CREATE TABLE IF NOT EXISTS quota (
          day TEXT NOT NULL,
          provider TEXT NOT NULL,
          kind TEXT NOT NULL,
          count INTEGER NOT NULL DEFAULT 0,
          PRIMARY KEY (day, provider, kind)
        );
        "#,
    )
    .map_err(|e| format!("初始化地图缓存失败: {e}"))?;
    Ok(conn)
}

pub(crate) fn provider_from_url(url: &str) -> &'static str {
    if url.contains("restapi.amap.com") {
        "amap"
    } else if url.contains("api.map.baidu.com") {
        "baidu"
    } else if url.contains("apis.map.qq.com") {
        "tencent"
    } else if url.contains("maps.googleapis.com") {
        "google"
    } else if url.contains("api.mapbox.com") {
        "mapbox"
    } else if url.contains("open-meteo.com") {
        "open-meteo"
    } else {
        "osm"
    }
}

pub(crate) fn kind_from_url(url: &str) -> &'static str {
    if url.contains("weatherInfo") || url.contains("open-meteo.com") {
        "weather"
    } else if url.contains("/direction") || url.contains("/directions") || url.contains("/route/") {
        "route"
    } else if url.contains("/staticmap") || url.contains("/staticimage") {
        "static"
    } else if url.contains("/geocod")
        || url.contains("/regeo")
        || url.contains("/geocoder")
        || url.contains("/assistant/inputtips")
        || url.contains("/v3/ip")
        || url.contains("/maps/api/geocode")
        || url.contains("nominatim")
    {
        "geocode"
    } else if url.contains("/place") || url.contains("overpass") {
        "poi"
    } else {
        "other"
    }
}

pub fn get(key: &str, ttl: Duration) -> Option<Value> {
    let _ = ttl;
    let conn = open().ok()?;
    let row = conn
        .query_row(
            "SELECT payload, expires_at FROM cache WHERE key = ?1",
            params![key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .ok()?;
    let (payload, expires_at) = row?;
    if expires_at <= now_secs() {
        let _ = conn.execute("DELETE FROM cache WHERE key = ?1", params![key]);
        return None;
    }
    serde_json::from_str(&payload).ok()
}

pub fn put(key: &str, value: &Value, ttl: Duration) -> Result<(), String> {
    let conn = open()?;
    let now = now_secs();
    let expires = now + ttl.as_secs().max(1) as i64;
    let payload = serde_json::to_string(value).map_err(|e| e.to_string())?;
    let provider = provider_from_url(key);
    let kind = kind_from_url(key);
    conn.execute(
        "INSERT INTO cache (key, provider, kind, url, payload, created_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(key) DO UPDATE SET payload=excluded.payload, created_at=excluded.created_at, expires_at=excluded.expires_at",
        params![key, provider, kind, key, payload, now, expires],
    )
    .map_err(|e| format!("写入地图缓存失败: {e}"))?;
    // 顺手清理过期超过一周的旧记录，避免缓存无限增长。
    let _ = conn.execute(
        "DELETE FROM cache WHERE expires_at < ?1",
        params![now - 7 * 24 * 3600],
    );
    Ok(())
}

pub fn bump(url: &str) {
    let Ok(conn) = open() else {
        return;
    };
    let day = chrono::Local::now().format("%Y-%m-%d").to_string();
    let provider = provider_from_url(url);
    let kind = kind_from_url(url);
    let _ = conn.execute(
        "INSERT INTO quota (day, provider, kind, count) VALUES (?1, ?2, ?3, 1)
         ON CONFLICT(day, provider, kind) DO UPDATE SET count = count + 1",
        params![day, provider, kind],
    );
}

pub fn usage_today() -> Result<Value, String> {
    let conn = open()?;
    let day = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut stmt = conn
        .prepare("SELECT provider, kind, count FROM quota WHERE day = ?1 ORDER BY provider, kind")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![day], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut providers: Map<String, Value> = Map::new();
    for row in rows.flatten() {
        let (provider, kind, count) = row;
        let entry = providers
            .entry(provider)
            .or_insert_with(|| json!({}));
        if let Some(object) = entry.as_object_mut() {
            object.insert(kind, json!(count));
        }
    }
    let cache_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM cache", [], |row| row.get(0))
        .unwrap_or(0);
    Ok(json!({
        "day": day,
        "providers": Value::Object(providers),
        "cacheCount": cache_count,
        "limits": {
            "amap": { "poi": 100, "route": 5000, "geocode": 5000, "weather": 5000 }
        }
    }))
}

pub fn clear() -> Result<(), String> {
    let conn = open()?;
    conn.execute("DELETE FROM cache", [])
        .map_err(|e| format!("清理地图缓存失败: {e}"))?;
    Ok(())
}
