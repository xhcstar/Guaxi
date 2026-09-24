use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::io::BufReader;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use exif::{In, Tag, Value};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ExtendedColorType};
use lru::LruCache;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rayon::prelude::*;
use rusqlite::Connection;
use uuid::Uuid;
use walkdir::WalkDir;

/// 缩略图最长边（像素）。480 兼顾网格清晰度（含视网膜屏）与体积（jpeg q82 约 30–60KB）。
const THUMB_MAX_EDGE: u32 = 480;
const THUMB_QUALITY: u8 = 82;

#[derive(Debug, Serialize, Clone, Default)]
pub struct LifeAlbumPhoto {
    pub path: String,
    pub original_path: String,
    pub file_name: String,
    pub taken_at: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub thumb: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct LifeAlbumPhotoMeta {
    pub path: String,
    pub taken_at: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub thumb: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Clone, Default)]
struct CachedMeta {
    mtime: i64,
    taken_at: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    thumb: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

static GEO_HTTP: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent("guaxi-life-album/0.1")
        .build()
        .expect("build reqwest client failed")
});

static GEO_CACHE: Lazy<Mutex<LruCache<String, String>>> = Lazy::new(|| {
    Mutex::new(LruCache::new(NonZeroUsize::new(8192).expect("geo cache size")))
});

fn normalize_path(path: &str) -> PathBuf {
    use std::path::Component;

    let mut s = path.trim().replace('\\', "/");

    if s.starts_with('/') && s.len() >= 4 {
        let bytes = s.as_bytes();
        if bytes[2] == b':' && bytes[3] == b'/' {
            s = s[1..].to_string();
        }
    }

    let mut out = PathBuf::new();
    let mut anchored = false;
    let mut depth = 0usize;

    for comp in Path::new(&s).components() {
        match comp {
            Component::Prefix(p) => {
                out.push(p.as_os_str());
                anchored = true;
                depth = 0;
            }
            Component::RootDir => {
                out.push(Path::new("/"));
                anchored = true;
                depth = 0;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if depth > 0 {
                    out.pop();
                    depth -= 1;
                } else if !anchored {
                    out.push("..");
                }
            }
            Component::Normal(part) => {
                out.push(part);
                depth += 1;
            }
        }
    }

    out
}

fn is_supported_photo(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
        return false;
    };
    matches!(ext.to_lowercase().as_str(), "jpg" | "jpeg" | "png" | "webp")
}

fn life_album_dir() -> Result<PathBuf, String> {
    let root = crate::commands::settingview::get_image_bed_root();
    if root.trim().is_empty() {
        return Err("未配置图片床路径，请先在设置中配置".to_string());
    }
    let root = PathBuf::from(root);
    let dir = root.join("images").join("life-album");
    fs::create_dir_all(&dir).map_err(|e| format!("创建相册目录失败: {}", e))?;
    Ok(dir)
}

#[tauri::command]
pub fn life_album_get_internal_root_dir() -> Result<String, String> {
    Ok(life_album_dir()?.to_string_lossy().to_string())
}

fn album_cache_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let base = exe
        .parent()
        .ok_or_else(|| "无法获取可执行文件目录".to_string())?;
    let dir = base.join("repos").join("life_album_cache");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn thumbs_dir() -> Result<PathBuf, String> {
    let dir = album_cache_dir()?.join("thumbs");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn open_cache_db() -> Result<Connection, String> {
    let path = album_cache_dir()?.join("life_album_cache.sqlite3");
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        PRAGMA temp_store=MEMORY;
        CREATE TABLE IF NOT EXISTS photo_meta (
            path TEXT PRIMARY KEY,
            mtime INTEGER NOT NULL,
            taken_at TEXT,
            latitude REAL,
            longitude REAL,
            thumb TEXT,
            width INTEGER,
            height INTEGER
        );
        CREATE TABLE IF NOT EXISTS geo_cache (
            key TEXT PRIMARY KEY,
            place TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        );
        "#,
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

fn load_meta_map(conn: &Connection) -> HashMap<String, CachedMeta> {
    let mut out = HashMap::new();
    let Ok(mut stmt) = conn.prepare(
        "SELECT path, mtime, taken_at, latitude, longitude, thumb, width, height FROM photo_meta",
    ) else {
        return out;
    };
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            CachedMeta {
                mtime: r.get(1)?,
                taken_at: r.get(2)?,
                latitude: r.get(3)?,
                longitude: r.get(4)?,
                thumb: r.get(5)?,
                width: r.get::<_, Option<i64>>(6)?.map(|v| v as u32),
                height: r.get::<_, Option<i64>>(7)?.map(|v| v as u32),
            },
        ))
    });
    if let Ok(rows) = rows {
        for row in rows.flatten() {
            out.insert(row.0, row.1);
        }
    }
    out
}

fn file_mtime(path: &Path) -> i64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn rational_to_f64(r: exif::Rational) -> Option<f64> {
    if r.denom == 0 {
        return None;
    }
    Some(r.num as f64 / r.denom as f64)
}

fn gps_dms_to_decimal(dms: &[exif::Rational], reference: &str) -> Option<f64> {
    if dms.len() < 3 {
        return None;
    }
    let d = rational_to_f64(dms[0])?;
    let m = rational_to_f64(dms[1])?;
    let s = rational_to_f64(dms[2])?;
    let mut v = d + (m / 60.0) + (s / 3600.0);
    let r = reference.trim().to_uppercase();
    if r == "S" || r == "W" {
        v = -v;
    }
    Some(v)
}

fn exif_ascii(field: &exif::Field) -> Option<String> {
    match &field.value {
        Value::Ascii(v) => v.get(0).map(|b| String::from_utf8_lossy(b).trim().to_string()),
        _ => None,
    }
}

fn exif_rational_vec(field: &exif::Field) -> Option<Vec<exif::Rational>> {
    match &field.value {
        Value::Rational(v) => Some(v.clone()),
        _ => None,
    }
}

/// 返回 (taken_at, latitude, longitude, orientation)。orientation 为 EXIF 1..=8，缺省 1。
fn read_exif_meta(path: &Path) -> (Option<String>, Option<f64>, Option<f64>, u32) {
    let Ok(file) = fs::File::open(path) else {
        return (None, None, None, 1);
    };
    let mut reader = BufReader::new(file);
    let Ok(exif) = exif::Reader::new().read_from_container(&mut reader) else {
        return (None, None, None, 1);
    };

    let taken_at = exif
        .get_field(Tag::DateTimeOriginal, In::PRIMARY)
        .or_else(|| exif.get_field(Tag::DateTime, In::PRIMARY))
        .and_then(exif_ascii)
        .and_then(|s| NaiveDateTime::parse_from_str(&s, "%Y:%m:%d %H:%M:%S").ok())
        .and_then(|naive| Local.from_local_datetime(&naive).single())
        .map(|dt| dt.to_rfc3339());

    let lat_ref = exif
        .get_field(Tag::GPSLatitudeRef, In::PRIMARY)
        .and_then(exif_ascii)
        .unwrap_or_default();
    let lon_ref = exif
        .get_field(Tag::GPSLongitudeRef, In::PRIMARY)
        .and_then(exif_ascii)
        .unwrap_or_default();

    let latitude = exif
        .get_field(Tag::GPSLatitude, In::PRIMARY)
        .and_then(exif_rational_vec)
        .and_then(|v| gps_dms_to_decimal(&v, &lat_ref));
    let longitude = exif
        .get_field(Tag::GPSLongitude, In::PRIMARY)
        .and_then(exif_rational_vec)
        .and_then(|v| gps_dms_to_decimal(&v, &lon_ref));

    let orientation = exif
        .get_field(Tag::Orientation, In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .filter(|v| (1..=8).contains(v))
        .unwrap_or(1);

    (taken_at, latitude, longitude, orientation)
}

fn fallback_file_time(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    let t = meta.modified().ok().or_else(|| meta.created().ok())?;
    let dt: DateTime<Local> = t.into();
    Some(dt.to_rfc3339())
}

fn apply_orientation(img: DynamicImage, orientation: u32) -> DynamicImage {
    match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

fn thumb_key(path: &str, mtime: i64) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(path.as_bytes());
    hasher.update(&mtime.to_le_bytes());
    hasher.finalize().to_hex().to_string()
}

/// 生成（或复用）缩略图，返回 (缩略图绝对路径, 宽, 高)。失败返回 None。
fn ensure_thumb(path: &Path, mtime: i64, orientation: u32) -> Option<(String, u32, u32)> {
    let abs = path.to_string_lossy().to_string();
    let key = thumb_key(&abs, mtime);
    let dir = thumbs_dir().ok()?;
    let sub = dir.join(&key[0..2]);
    let _ = fs::create_dir_all(&sub);
    let out = sub.join(format!("{}.jpg", key));

    if out.is_file() {
        let (w, h) = image::image_dimensions(&out).unwrap_or((0, 0));
        return Some((out.to_string_lossy().to_string(), w, h));
    }

    let img = image::open(path).ok()?;
    let img = apply_orientation(img, orientation);
    let thumb = img.thumbnail(THUMB_MAX_EDGE, THUMB_MAX_EDGE);
    let (w, h) = (thumb.width(), thumb.height());
    let rgb = thumb.to_rgb8();

    let tmp = sub.join(format!("{}.tmp.jpg", key));
    {
        let mut f = fs::File::create(&tmp).ok()?;
        let mut enc = JpegEncoder::new_with_quality(&mut f, THUMB_QUALITY);
        enc.encode(rgb.as_raw(), w, h, ExtendedColorType::Rgb8).ok()?;
    }
    fs::rename(&tmp, &out).ok()?;
    Some((out.to_string_lossy().to_string(), w, h))
}

/// 从磁盘读取一个照片的完整元数据（EXIF + 缩略图）。用于缓存未命中路径，可在 rayon 并行调用。
fn compute_meta(path: &Path) -> LifeAlbumPhotoMeta {
    let (mut taken_at, latitude, longitude, orientation) = read_exif_meta(path);
    if taken_at.is_none() {
        taken_at = fallback_file_time(path);
    }
    let (thumb, width, height) = match ensure_thumb(path, file_mtime(path), orientation) {
        Some((t, w, h)) => (Some(t), Some(w), Some(h)),
        None => (None, None, None),
    };
    LifeAlbumPhotoMeta {
        path: path.to_string_lossy().to_string(),
        taken_at,
        latitude,
        longitude,
        thumb,
        width,
        height,
    }
}

fn upsert_meta(conn: &Connection, path: &str, mtime: i64, m: &LifeAlbumPhotoMeta) {
    let _ = conn.execute(
        "INSERT INTO photo_meta (path, mtime, taken_at, latitude, longitude, thumb, width, height)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(path) DO UPDATE SET
            mtime=excluded.mtime, taken_at=excluded.taken_at,
            latitude=excluded.latitude, longitude=excluded.longitude,
            thumb=excluded.thumb, width=excluded.width, height=excluded.height",
        rusqlite::params![
            path,
            mtime,
            m.taken_at,
            m.latitude,
            m.longitude,
            m.thumb,
            m.width.map(|v| v as i64),
            m.height.map(|v| v as i64),
        ],
    );
}

#[tauri::command]
pub fn life_album_scan_dir(
    dir_path: String,
    max_count: Option<u32>,
) -> Result<Vec<LifeAlbumPhoto>, String> {
    let dir_path = normalize_path(&dir_path);
    if !dir_path.is_dir() {
        return Err("路径不是目录".to_string());
    }

    let files: Vec<PathBuf> = WalkDir::new(&dir_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .map(|e| e.into_path())
        .filter(|p| p.is_file() && is_supported_photo(p))
        .collect();

    // 一次性载入缓存，命中且 mtime 未变的照片直接带出缩略图/EXIF —— 冷启动秒开。
    let cache = open_cache_db().ok().map(|c| load_meta_map(&c)).unwrap_or_default();

    let mut out: Vec<LifeAlbumPhoto> = files
        .par_iter()
        .map(|p| {
            let abs = p.to_string_lossy().to_string();
            let mtime = file_mtime(p);
            let file_name = p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            if let Some(c) = cache.get(&abs) {
                if c.mtime == mtime {
                    return LifeAlbumPhoto {
                        path: abs.clone(),
                        original_path: abs,
                        file_name,
                        taken_at: c.taken_at.clone(),
                        latitude: c.latitude,
                        longitude: c.longitude,
                        thumb: c.thumb.clone(),
                        width: c.width,
                        height: c.height,
                    };
                }
            }

            LifeAlbumPhoto {
                path: abs.clone(),
                original_path: abs,
                file_name,
                taken_at: fallback_file_time(p),
                ..Default::default()
            }
        })
        .collect();

    out.sort_by(|a, b| b.taken_at.cmp(&a.taken_at));
    if let Some(m) = max_count {
        out.truncate(m as usize);
    }
    Ok(out)
}

#[tauri::command]
pub fn life_album_enrich_files(paths: Vec<String>) -> Result<Vec<LifeAlbumPhotoMeta>, String> {
    let conn = open_cache_db()?;
    let cache = load_meta_map(&conn);

    // 命中：mtime 未变且已有缩略图，直接复用；未命中：稍后并行计算。
    let mut hits: Vec<LifeAlbumPhotoMeta> = Vec::new();
    let mut misses: Vec<(PathBuf, i64)> = Vec::new();

    for p in &paths {
        let path = normalize_path(p);
        if !path.is_file() {
            continue;
        }
        let abs = path.to_string_lossy().to_string();
        let mtime = file_mtime(&path);
        if let Some(c) = cache.get(&abs) {
            if c.mtime == mtime && c.thumb.as_deref().map(|t| Path::new(t).is_file()).unwrap_or(false)
            {
                hits.push(LifeAlbumPhotoMeta {
                    path: abs,
                    taken_at: c.taken_at.clone(),
                    latitude: c.latitude,
                    longitude: c.longitude,
                    thumb: c.thumb.clone(),
                    width: c.width,
                    height: c.height,
                });
                continue;
            }
        }
        misses.push((path, mtime));
    }

    let computed: Vec<(LifeAlbumPhotoMeta, i64)> = misses
        .par_iter()
        .map(|(path, mtime)| (compute_meta(path), *mtime))
        .collect();

    for (m, mtime) in &computed {
        upsert_meta(&conn, &m.path, *mtime, m);
    }

    let mut out = hits;
    out.extend(computed.into_iter().map(|(m, _)| m));
    Ok(out)
}

fn import_one_photo(src: &Path) -> Result<LifeAlbumPhoto, String> {
    let file_name = src
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("jpg")
        .to_lowercase();

    let now = Local::now();
    let dir = life_album_dir()?
        .join(now.format("%Y").to_string())
        .join(now.format("%m").to_string());
    fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let dest = dir.join(format!("{}.{}", Uuid::new_v4(), ext));
    fs::copy(src, &dest).map_err(|e| format!("复制文件失败: {}", e))?;

    let meta = compute_meta(&dest);
    Ok(LifeAlbumPhoto {
        path: dest.to_string_lossy().to_string(),
        original_path: src.to_string_lossy().to_string(),
        file_name,
        taken_at: meta.taken_at,
        latitude: meta.latitude,
        longitude: meta.longitude,
        thumb: meta.thumb,
        width: meta.width,
        height: meta.height,
    })
}

#[tauri::command]
pub fn life_album_import_files(paths: Vec<String>) -> Result<Vec<LifeAlbumPhoto>, String> {
    let conn = open_cache_db().ok();
    let mut out: Vec<LifeAlbumPhoto> = Vec::new();
    for p in paths {
        let path = normalize_path(&p);
        if !path.is_file() || !is_supported_photo(&path) {
            continue;
        }
        let photo = import_one_photo(&path)?;
        if let Some(c) = &conn {
            upsert_meta(
                c,
                &photo.path,
                file_mtime(Path::new(&photo.path)),
                &LifeAlbumPhotoMeta {
                    path: photo.path.clone(),
                    taken_at: photo.taken_at.clone(),
                    latitude: photo.latitude,
                    longitude: photo.longitude,
                    thumb: photo.thumb.clone(),
                    width: photo.width,
                    height: photo.height,
                },
            );
        }
        out.push(photo);
    }
    out.sort_by(|a, b| b.taken_at.cmp(&a.taken_at));
    Ok(out)
}

#[tauri::command]
pub fn life_album_import_dir(dir_path: String) -> Result<Vec<LifeAlbumPhoto>, String> {
    let dir_path = normalize_path(&dir_path);
    if !dir_path.is_dir() {
        return Err("路径不是目录".to_string());
    }
    let conn = open_cache_db().ok();
    let mut out: Vec<LifeAlbumPhoto> = Vec::new();
    for entry in WalkDir::new(&dir_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if !p.is_file() || !is_supported_photo(p) {
            continue;
        }
        let photo = import_one_photo(p)?;
        if let Some(c) = &conn {
            upsert_meta(
                c,
                &photo.path,
                file_mtime(Path::new(&photo.path)),
                &LifeAlbumPhotoMeta {
                    path: photo.path.clone(),
                    taken_at: photo.taken_at.clone(),
                    latitude: photo.latitude,
                    longitude: photo.longitude,
                    thumb: photo.thumb.clone(),
                    width: photo.width,
                    height: photo.height,
                },
            );
        }
        out.push(photo);
    }
    out.sort_by(|a, b| b.taken_at.cmp(&a.taken_at));
    Ok(out)
}

// ---------- 地名反查（多源 + 持久化）----------
// 逻辑：先查 sqlite geo_cache（永久，秒开）→ 内存 LRU → 高德批量 regeo（配置了 key，国内快且中文）
// → Nominatim 单条兜底。结果一律落库，之后每次都命中缓存，不再联网。

const GCJ_A: f64 = 6378245.0;
const GCJ_EE: f64 = 0.006_693_421_622_965_943;

fn out_of_china(lat: f64, lon: f64) -> bool {
    !(72.004..=137.8347).contains(&lon) || !(0.8293..=55.8271).contains(&lat)
}

fn transform_lat(x: f64, y: f64) -> f64 {
    let mut ret = -100.0 + 2.0 * x + 3.0 * y + 0.2 * y * y + 0.1 * x * y + 0.2 * x.abs().sqrt();
    ret += (20.0 * (6.0 * x * std::f64::consts::PI).sin() + 20.0 * (2.0 * x * std::f64::consts::PI).sin()) * 2.0 / 3.0;
    ret += (20.0 * (y * std::f64::consts::PI).sin() + 40.0 * (y / 3.0 * std::f64::consts::PI).sin()) * 2.0 / 3.0;
    ret += (160.0 * (y / 12.0 * std::f64::consts::PI).sin() + 320.0 * (y * std::f64::consts::PI / 30.0).sin()) * 2.0 / 3.0;
    ret
}

fn transform_lon(x: f64, y: f64) -> f64 {
    let mut ret = 300.0 + x + 2.0 * y + 0.1 * x * x + 0.1 * x * y + 0.1 * x.abs().sqrt();
    ret += (20.0 * (6.0 * x * std::f64::consts::PI).sin() + 20.0 * (2.0 * x * std::f64::consts::PI).sin()) * 2.0 / 3.0;
    ret += (20.0 * (x * std::f64::consts::PI).sin() + 40.0 * (x / 3.0 * std::f64::consts::PI).sin()) * 2.0 / 3.0;
    ret += (150.0 * (x / 12.0 * std::f64::consts::PI).sin() + 300.0 * (x / 30.0 * std::f64::consts::PI).sin()) * 2.0 / 3.0;
    ret
}

/// WGS-84（照片 EXIF 用）→ GCJ-02（高德/腾讯用）。国外坐标原样返回。
fn wgs84_to_gcj02(lat: f64, lon: f64) -> (f64, f64) {
    if out_of_china(lat, lon) {
        return (lat, lon);
    }
    let dlat = transform_lat(lon - 105.0, lat - 35.0);
    let dlon = transform_lon(lon - 105.0, lat - 35.0);
    let radlat = lat / 180.0 * std::f64::consts::PI;
    let mut magic = radlat.sin();
    magic = 1.0 - GCJ_EE * magic * magic;
    let sqrtmagic = magic.sqrt();
    let dlat = (dlat * 180.0) / ((GCJ_A * (1.0 - GCJ_EE)) / (magic * sqrtmagic) * std::f64::consts::PI);
    let dlon = (dlon * 180.0) / (GCJ_A / sqrtmagic * radlat.cos() * std::f64::consts::PI);
    (lat + dlat, lon + dlon)
}

fn geo_key(lat: f64, lon: f64) -> String {
    format!("{:.3},{:.3}", lat, lon)
}

fn geo_db_get(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT place FROM geo_cache WHERE key=?1",
        [key],
        |r| r.get::<_, String>(0),
    )
    .ok()
}

fn geo_db_put(conn: &Connection, key: &str, place: &str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let _ = conn.execute(
        "INSERT INTO geo_cache (key, place, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET place=excluded.place, updated_at=excluded.updated_at",
        rusqlite::params![key, place, now],
    );
}

fn pick_addr_str(addr: Option<&serde_json::Value>, keys: &[&str]) -> Option<String> {
    let a = addr?;
    for k in keys {
        if let Some(s) = a.get(*k).and_then(|v| v.as_str()) {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

fn join_place(parts: Vec<Option<String>>) -> String {
    let mut out: Vec<String> = Vec::new();
    for p in parts.into_iter().flatten() {
        let t = p.trim().to_string();
        if t.is_empty() || out.last().map(|v| v == &t).unwrap_or(false) {
            continue;
        }
        out.push(t);
    }
    if out.is_empty() {
        "未知位置".to_string()
    } else {
        out.join(" ")
    }
}

/// 高德 regeo 的 addressComponent → "省 市 区"。空串/空数组字段跳过。
fn amap_place(component: &serde_json::Value) -> String {
    let s = |k: &str| -> Option<String> {
        component
            .get(k)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    join_place(vec![s("province"), s("city"), s("district")])
}

/// 高德批量逆地理编码（每请求最多 20 个点）。coords 为 WGS-84。返回与输入等长、对齐的地名。
async fn amap_reverse_batch(key: &str, coords: &[(f64, f64)]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::with_capacity(coords.len());
    for chunk in coords.chunks(20) {
        let loc = chunk
            .iter()
            .map(|(lat, lon)| {
                let (glat, glon) = wgs84_to_gcj02(*lat, *lon);
                format!("{:.6},{:.6}", glon, glat)
            })
            .collect::<Vec<_>>()
            .join("|");

        let url = format!(
            "https://restapi.amap.com/v3/geocode/regeo?key={}&location={}&batch=true&extensions=base",
            key, loc
        );

        let resp = GEO_HTTP
            .get(url)
            .send()
            .await
            .map_err(|e| format!("高德反查失败: {}", e))?
            .json::<serde_json::Value>()
            .await
            .map_err(|e| format!("高德反查解析失败: {}", e))?;

        if resp.get("status").and_then(|v| v.as_str()) != Some("1") {
            let info = resp.get("info").and_then(|v| v.as_str()).unwrap_or("未知错误");
            return Err(format!("高德返回错误: {}", info));
        }

        let arr = resp
            .get("regeocodes")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        for i in 0..chunk.len() {
            let place = arr
                .get(i)
                .and_then(|r| r.get("addressComponent"))
                .map(amap_place)
                .unwrap_or_else(|| "未知位置".to_string());
            out.push(place);
        }
    }
    Ok(out)
}

async fn nominatim_reverse(lat: f64, lon: f64) -> Result<String, String> {
    let url = format!(
        "https://nominatim.openstreetmap.org/reverse?format=jsonv2&lat={}&lon={}&zoom=12&accept-language=zh-CN",
        lat, lon
    );
    let resp = GEO_HTTP
        .get(url)
        .send()
        .await
        .map_err(|e| format!("反查失败: {}", e))?
        .error_for_status()
        .map_err(|e| format!("反查失败: {}", e))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("反查失败: {}", e))?;
    let addr = resp.get("address");
    let country = pick_addr_str(addr, &["country"]);
    let province = pick_addr_str(addr, &["state", "province", "region"]);
    let city = pick_addr_str(addr, &["city", "town"]);
    let county = pick_addr_str(addr, &["county", "city_district", "district", "suburb"]);
    Ok(join_place(vec![country, province, city, county]))
}

fn place_is_good(p: &str) -> bool {
    let t = p.trim();
    !t.is_empty() && t != "未知位置"
}

fn opt_str(v: &serde_json::Value, k: &str) -> Option<String> {
    v.get(k)
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// BigDataCloud 免费客户端逆地理（无需 key，支持中文，国内可用，无硬性限速）。
async fn bigdatacloud_reverse(lat: f64, lon: f64) -> Result<String, String> {
    let url = format!(
        "https://api.bigdatacloud.net/data/reverse-geocode-client?latitude={}&longitude={}&localityLanguage=zh",
        lat, lon
    );
    let resp = GEO_HTTP
        .get(url)
        .send()
        .await
        .map_err(|e| format!("bigdatacloud 失败: {}", e))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("bigdatacloud 解析失败: {}", e))?;
    let province = opt_str(&resp, "principalSubdivision");
    let city = opt_str(&resp, "city");
    let locality = opt_str(&resp, "locality");
    let country = opt_str(&resp, "countryName");
    let place = join_place(vec![province, city, locality]);
    if place_is_good(&place) {
        Ok(place)
    } else {
        Ok(join_place(vec![country]))
    }
}

/// Photon（komoot）免费逆地理（无需 key，基于 OSM，lang=default 返回当地语言）。
async fn photon_reverse(lat: f64, lon: f64) -> Result<String, String> {
    let url = format!(
        "https://photon.komoot.io/reverse?lat={}&lon={}&lang=default",
        lat, lon
    );
    let resp = GEO_HTTP
        .get(url)
        .send()
        .await
        .map_err(|e| format!("photon 失败: {}", e))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("photon 解析失败: {}", e))?;
    let props = resp
        .get("features")
        .and_then(|f| f.as_array())
        .and_then(|a| a.first())
        .and_then(|f| f.get("properties"));
    let Some(props) = props else {
        return Ok(String::new());
    };
    let province = opt_str(props, "state");
    let city = opt_str(props, "city").or_else(|| opt_str(props, "name"));
    let county = opt_str(props, "county").or_else(|| opt_str(props, "district"));
    Ok(join_place(vec![province, city, county]))
}

/// 免费多源链：依次尝试 BigDataCloud → Photon → Nominatim，取首个可用结果。
async fn free_reverse(lat: f64, lon: f64) -> String {
    if let Ok(p) = bigdatacloud_reverse(lat, lon).await {
        if place_is_good(&p) {
            return p;
        }
    }
    if let Ok(p) = photon_reverse(lat, lon).await {
        if place_is_good(&p) {
            return p;
        }
    }
    if let Ok(p) = nominatim_reverse(lat, lon).await {
        if place_is_good(&p) {
            return p;
        }
    }
    "未知位置".to_string()
}

async fn reverse_geocode_impl(coords: Vec<(f64, f64)>) -> Result<HashMap<String, String>, String> {
    let mut result: HashMap<String, String> = HashMap::new();

    // 按 3 位小数去重（约 110m），大幅减少请求数。
    let mut uniq: Vec<(String, (f64, f64))> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (lat, lon) in coords {
        if !lat.is_finite() || !lon.is_finite() {
            continue;
        }
        let key = geo_key(lat, lon);
        if seen.insert(key.clone()) {
            uniq.push((key, (lat, lon)));
        }
    }

    // L1 内存 LRU + L2 sqlite（永久）。命中即不再联网。连接不跨 await —— 用完即 drop。
    let mut misses: Vec<(String, (f64, f64))> = Vec::new();
    {
        let conn = open_cache_db().ok();
        for (key, coord) in uniq {
            if let Some(v) = GEO_CACHE.lock().get(&key).cloned() {
                result.insert(key, v);
                continue;
            }
            if let Some(c) = &conn {
                if let Some(v) = geo_db_get(c, &key) {
                    GEO_CACHE.lock().put(key.clone(), v.clone());
                    result.insert(key, v);
                    continue;
                }
            }
            misses.push((key, coord));
        }
    }

    if misses.is_empty() {
        return Ok(result);
    }

    // 联网反查（含 await）：只收集 (key, place)，不碰数据库连接。
    let mut resolved: Vec<(String, String)> = Vec::new();
    let amap_key = crate::commands::settingview::get_life_album_amap_key()
        .trim()
        .to_string();

    let mut done_via_amap = false;
    if !amap_key.is_empty() {
        let pts: Vec<(f64, f64)> = misses.iter().map(|(_, c)| *c).collect();
        match amap_reverse_batch(&amap_key, &pts).await {
            Ok(places) => {
                for ((key, _), place) in misses.iter().zip(places.into_iter()) {
                    resolved.push((key.clone(), place));
                }
                done_via_amap = true;
            }
            Err(e) => {
                tracing::warn!("高德批量反查失败，回退免费多源: {}", e);
            }
        }
    }

    if !done_via_amap {
        // 免费多源（BigDataCloud → Photon → Nominatim），限并发 5 条，明显快于逐条串行。
        use futures_util::stream::StreamExt;
        let more: Vec<(String, String)> = futures_util::stream::iter(misses.into_iter())
            .map(|(key, (lat, lon))| async move { (key, free_reverse(lat, lon).await) })
            .buffer_unordered(5)
            .collect()
            .await;
        resolved.extend(more);
    }

    // 落库（同步块，连接不跨 await）。下次秒开。
    {
        let conn = open_cache_db().ok();
        for (key, place) in &resolved {
            GEO_CACHE.lock().put(key.clone(), place.clone());
            if let Some(c) = &conn {
                geo_db_put(c, key, place);
            }
            result.insert(key.clone(), place.clone());
        }
    }

    Ok(result)
}

/// 清空地名反查缓存（sqlite geo_cache 表 + 内存 LRU）。之后重新反查会用当前配置的数据源（如高德）。
#[tauri::command]
pub fn life_album_clear_geocode_cache() -> Result<u64, String> {
    GEO_CACHE.lock().clear();
    let conn = open_cache_db()?;
    let n = conn
        .execute("DELETE FROM geo_cache", [])
        .map_err(|e| e.to_string())?;
    Ok(n as u64)
}

#[tauri::command]
pub async fn life_album_reverse_geocode(latitude: f64, longitude: f64) -> Result<String, String> {
    if !latitude.is_finite() || !longitude.is_finite() {
        return Err("经纬度无效".to_string());
    }
    let key = geo_key(latitude, longitude);
    let map = reverse_geocode_impl(vec![(latitude, longitude)]).await?;
    Ok(map.get(&key).cloned().unwrap_or_else(|| "未知位置".to_string()))
}

/// 批量反查：前端一次把所有唯一坐标传进来，命中缓存的秒回，未命中的走高德批量。
#[tauri::command]
pub async fn life_album_reverse_geocode_batch(
    coords: Vec<[f64; 2]>,
) -> Result<HashMap<String, String>, String> {
    let pairs: Vec<(f64, f64)> = coords
        .into_iter()
        .filter(|c| c[0].is_finite() && c[1].is_finite())
        .map(|c| (c[0], c[1]))
        .collect();
    reverse_geocode_impl(pairs).await
}

