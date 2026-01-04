use serde::Serialize;
use std::fs;
use std::io::BufReader;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use exif::{In, Tag, Value};
use lru::LruCache;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rayon::prelude::*;
use uuid::Uuid;
use walkdir::WalkDir;

#[derive(Debug, Serialize, Clone)]
pub struct LifeAlbumPhoto {
    pub path: String,
    pub original_path: String,
    pub file_name: String,
    pub taken_at: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Debug, Serialize, Clone)]
pub struct LifeAlbumPhotoMeta {
    pub path: String,
    pub taken_at: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

static GEO_HTTP: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent("guaxi-life-album/0.1")
        .build()
        .expect("build reqwest client failed")
});

static GEO_CACHE: Lazy<Mutex<LruCache<String, String>>> = Lazy::new(|| {
    Mutex::new(LruCache::new(
        NonZeroUsize::new(4096).expect("geo cache size"),
    ))
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
    matches!(
        ext.to_lowercase().as_str(),
        "jpg" | "jpeg" | "png" | "webp"
    )
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
        Value::Ascii(v) => v
            .get(0)
            .map(|b| String::from_utf8_lossy(b).trim().to_string()),
        _ => None,
    }
}

fn exif_rational_vec(field: &exif::Field) -> Option<Vec<exif::Rational>> {
    match &field.value {
        Value::Rational(v) => Some(v.clone()),
        _ => None,
    }
}

fn read_exif_meta(path: &Path) -> (Option<String>, Option<f64>, Option<f64>) {
    let Ok(file) = fs::File::open(path) else {
        return (None, None, None);
    };

    let mut reader = BufReader::new(file);
    let Ok(exif) = exif::Reader::new().read_from_container(&mut reader) else {
        return (None, None, None);
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

    (taken_at, latitude, longitude)
}

fn fallback_file_time(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    let t = meta.modified().ok().or_else(|| meta.created().ok())?;
    let dt: DateTime<Local> = t.into();
    Some(dt.to_rfc3339())
}

fn scan_one_photo(src: &Path) -> LifeAlbumPhoto {
    let file_name = src
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let taken_at = fallback_file_time(src);

    let p = src.to_string_lossy().to_string();
    LifeAlbumPhoto {
        path: p.clone(),
        original_path: p,
        file_name,
        taken_at,
        latitude: None,
        longitude: None,
    }
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

    let mut out: Vec<LifeAlbumPhoto> = files.par_iter().map(|p| scan_one_photo(p)).collect();

    out.sort_by(|a, b| b.taken_at.cmp(&a.taken_at));

    if let Some(m) = max_count {
        out.truncate(m as usize);
    }

    Ok(out)
}

#[tauri::command]
pub fn life_album_enrich_files(paths: Vec<String>) -> Result<Vec<LifeAlbumPhotoMeta>, String> {
    let out: Vec<LifeAlbumPhotoMeta> = paths
        .par_iter()
        .map(|p| {
            let path = normalize_path(p);
            if !path.is_file() {
                return None;
            }

            let (mut taken_at, latitude, longitude) = read_exif_meta(&path);
            if taken_at.is_none() {
                taken_at = fallback_file_time(&path);
            }

            Some(LifeAlbumPhotoMeta {
                path: path.to_string_lossy().to_string(),
                taken_at,
                latitude,
                longitude,
            })
        })
        .filter_map(|v| v)
        .collect();

    Ok(out)
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
        if t.is_empty() {
            continue;
        }
        if out.last().map(|v| v == &t).unwrap_or(false) {
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

#[tauri::command]
pub async fn life_album_reverse_geocode(latitude: f64, longitude: f64) -> Result<String, String> {
    if !latitude.is_finite() || !longitude.is_finite() {
        return Err("经纬度无效".to_string());
    }

    let key = format!("{:.4},{:.4}", latitude, longitude);

    if let Some(v) = GEO_CACHE.lock().get(&key).cloned() {
        return Ok(v);
    }

    let url = format!(
        "https://nominatim.openstreetmap.org/reverse?format=jsonv2&lat={}&lon={}&zoom=12&accept-language=zh-CN",
        latitude, longitude
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

    let place = join_place(vec![country, province, city, county]);

    GEO_CACHE.lock().put(key, place.clone());
    Ok(place)
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

    let (mut taken_at, latitude, longitude) = read_exif_meta(src);
    if taken_at.is_none() {
        taken_at = fallback_file_time(src);
    }

    Ok(LifeAlbumPhoto {
        path: dest.to_string_lossy().to_string(),
        original_path: src.to_string_lossy().to_string(),
        file_name,
        taken_at,
        latitude,
        longitude,
    })
}

#[tauri::command]
pub fn life_album_import_files(paths: Vec<String>) -> Result<Vec<LifeAlbumPhoto>, String> {
    let mut out: Vec<LifeAlbumPhoto> = Vec::new();

    for p in paths {
        let path = normalize_path(&p);
        if !path.is_file() {
            continue;
        }
        if !is_supported_photo(&path) {
            continue;
        }
        out.push(import_one_photo(&path)?);
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

    let mut out: Vec<LifeAlbumPhoto> = Vec::new();

    for entry in WalkDir::new(&dir_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        if !is_supported_photo(p) {
            continue;
        }
        out.push(import_one_photo(p)?);
    }

    out.sort_by(|a, b| b.taken_at.cmp(&a.taken_at));
    Ok(out)
}