//! 旅行计划应用后端。
//!
//! 前端只负责表单、编排提示词和渲染；定位、POI、路线、天气、模型调用与磁盘存储都留在这里。
//! 这样高德/模型密钥不会进入 WebView，同时绕开前端 CSP 与跨域限制。
//!
//! 数据源是双通道：有高德 Web 服务 Key 时优先高德；没有 Key 或请求失败时自动降级到
//! Nominatim / Overpass / OSRM / Open-Meteo。降级只影响精度与公交能力，不影响应用可用性。

mod amap;
mod osm;
mod share;
mod store;
mod weather;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::commands::llm_provider;
use crate::commands::plugin_llm::{self, PluginLlmRequest};
use crate::commands::settingview;

pub(crate) const POI_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
pub(crate) const WEATHER_TTL: Duration = Duration::from_secs(30 * 60);
pub(crate) const GEO_TTL: Duration = Duration::from_secs(30 * 24 * 3600);
pub(crate) const HTTP_TIMEOUT: Duration = Duration::from_secs(25);

// ── 通用数据结构 ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub city: String,
    #[serde(default)]
    pub adcode: String,
    #[serde(default)]
    pub city_code: String,
    #[serde(default)]
    pub lng: f64,
    #[serde(default)]
    pub lat: f64,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlaceRef {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub city: String,
    #[serde(default)]
    pub lng: f64,
    #[serde(default)]
    pub lat: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Poi {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// amap | osm | ai
    #[serde(default)]
    pub source: String,
    /// sight | food | hotel
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub city: String,
    #[serde(default)]
    pub lng: f64,
    #[serde(default)]
    pub lat: f64,
    #[serde(default)]
    pub rating: Option<f64>,
    #[serde(default)]
    pub cost: Option<f64>,
    #[serde(default)]
    pub tel: String,
    #[serde(default)]
    pub distance: Option<f64>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WeatherDay {
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub weekday: String,
    #[serde(default)]
    pub day_weather: String,
    #[serde(default)]
    pub night_weather: String,
    #[serde(default)]
    pub day_temp: Option<f64>,
    #[serde(default)]
    pub night_temp: Option<f64>,
    #[serde(default)]
    pub wind: String,
    #[serde(default)]
    pub wind_level: String,
    #[serde(default)]
    pub precipitation: Option<f64>,
    /// sun | cloud | rain | snow | storm | fog
    #[serde(default)]
    pub icon: String,
    /// amap | open-meteo
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct TravelRequest {
    pub origin: PlaceRef,
    pub destination: PlaceRef,
    #[serde(default)]
    pub start_date: String,
    #[serde(default)]
    pub end_date: String,
    #[serde(default)]
    pub start_time: String,
    #[serde(default)]
    pub end_time: String,
    #[serde(default = "default_transport")]
    pub transport: String,
    #[serde(default = "default_budget")]
    pub budget_level: String,
    #[serde(default)]
    pub adults: u32,
    #[serde(default)]
    pub children: u32,
    #[serde(default)]
    pub elders: u32,
    #[serde(default)]
    pub interests: Vec<String>,
    #[serde(default)]
    pub pace: String,
    #[serde(default)]
    pub radius_km: Option<f64>,
    #[serde(default)]
    pub keywords: String,
    #[serde(default)]
    pub must_visit: Vec<String>,
    #[serde(default)]
    pub avoid: Vec<String>,
    /// auto | amap | osm
    #[serde(default)]
    pub source: String,
}

fn default_transport() -> String {
    "driving".to_string()
}

fn default_budget() -> String {
    "comfort".to_string()
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TravelFacts {
    pub source: String,
    pub origin: Place,
    pub destination: Place,
    pub attractions: Vec<Poi>,
    pub food: Vec<Poi>,
    pub hotels: Vec<Poi>,
    pub weather: Vec<WeatherDay>,
    pub notes: Vec<String>,
    pub collected_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RouteStep {
    #[serde(default)]
    pub instruction: String,
    #[serde(default)]
    pub road: String,
    #[serde(default)]
    pub distance: f64,
    #[serde(default)]
    pub duration: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RouteLeg {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub distance: f64,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub tolls: f64,
    #[serde(default)]
    pub fare: f64,
    #[serde(default)]
    pub traffic: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub steps: Vec<RouteStep>,
    #[serde(default)]
    pub polyline: String,
    #[serde(default)]
    pub warning: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct RouteStop {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub city: String,
    #[serde(default)]
    pub city_code: String,
    pub lng: f64,
    pub lat: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteDayRequest {
    #[serde(default)]
    pub date: String,
    pub stops: Vec<RouteStop>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutePlanRequest {
    #[serde(default)]
    pub transport: String,
    pub days: Vec<RouteDayRequest>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteDayResult {
    pub date: String,
    /// legs[i] 对应 stops[i] -> stops[i + 1]
    pub legs: Vec<RouteLeg>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticMapRequest {
    pub center_lng: f64,
    pub center_lat: f64,
    #[serde(default)]
    pub zoom: Option<u8>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub path: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelLlmRequest {
    pub prompt: String,
    #[serde(default)]
    pub system: String,
    #[serde(default)]
    pub model_id: Option<i32>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelLlmResponse {
    pub text: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelSettings {
    pub amap_key_set: bool,
    pub amap_key_tail: String,
    pub own_key_set: bool,
    pub album_key_set: bool,
    pub data_root: String,
    pub plan_count: usize,
    pub provider_label: String,
    pub source: String,
}

// ── 缓存与 HTTP ─────────────────────────────────────────────

type CacheMap = HashMap<String, (Instant, Value)>;

fn cache() -> &'static Mutex<CacheMap> {
    static CACHE: OnceLock<Mutex<CacheMap>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 命中未过期缓存时返回副本；过期条目顺手删除，缓存不会无限增长。
pub(crate) fn cache_get_ttl(key: &str) -> Option<Value> {
    let mut guard = cache().lock().ok()?;
    let expired = match guard.get(key) {
        Some((expires_at, _)) => Instant::now() >= *expires_at,
        None => return None,
    };
    if expired {
        guard.remove(key);
        return None;
    }
    guard.get(key).map(|(_, value)| value.clone())
}

pub(crate) fn cache_put(key: &str, value: Value, ttl: Duration) {
    if let Ok(mut guard) = cache().lock() {
        if guard.len() > 800 {
            guard.retain(|_, (expires_at, _)| Instant::now() < *expires_at);
            if guard.len() > 800 {
                guard.clear();
            }
        }
        guard.insert(key.to_string(), (Instant::now() + ttl, value));
    }
}

pub(crate) fn http_client() -> Result<reqwest::Client, String> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client.clone());
    }
    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent("GuaxiTravel/1.0 (desktop; travel planner)")
        .build()
        .map_err(|e| format!("创建网络客户端失败: {e}"))?;
    let _ = CLIENT.set(client.clone());
    Ok(client)
}

pub(crate) fn truncate(text: &str, max: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in text.chars().enumerate() {
        if idx >= max {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

pub(crate) fn cached_json(url: &str, ttl: Duration) -> impl std::future::Future<Output = Result<Value, String>> {
    let url = url.to_string();
    async move {
        if let Some(hit) = cache_get_ttl(&url) {
            return Ok(hit);
        }
        let response = http_client()?
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("网络请求失败: {e}"))?;
        let status = response.status();
        let text = response.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!(
                "服务返回 {}: {}",
                status.as_u16(),
                truncate(text.trim(), 240)
            ));
        }
        let value: Value =
            serde_json::from_str(&text).map_err(|e| format!("解析服务响应失败: {e}"))?;
        cache_put(&url, value.clone(), ttl);
        Ok(value)
    }
}

pub(crate) async fn cached_post_json(
    url: &str,
    form: Vec<(&str, String)>,
    ttl: Duration,
) -> Result<Value, String> {
    let key = format!("POST:{url}:{form:?}");
    if let Some(hit) = cache_get_ttl(&key) {
        return Ok(hit);
    }
    let response = http_client()?
        .post(url)
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {e}"))?;
    let status = response.status();
    let text = response.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!(
            "服务返回 {}: {}",
            status.as_u16(),
            truncate(text.trim(), 240)
        ));
    }
    let value: Value = serde_json::from_str(&text).map_err(|e| format!("解析服务响应失败: {e}"))?;
    cache_put(&key, value.clone(), ttl);
    Ok(value)
}

// ── 小工具 ─────────────────────────────────────────────────

pub(crate) fn val_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Array(items)) => items
            .first()
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .trim()
            .to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

pub(crate) fn val_f64(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse::<f64>().ok(),
        Some(Value::Array(items)) => items
            .first()
            .and_then(|item| match item {
                Value::Number(n) => n.as_f64(),
                Value::String(s) => s.trim().parse::<f64>().ok(),
                _ => None,
            }),
        _ => None,
    }
}

pub(crate) fn haversine_km(a_lng: f64, a_lat: f64, b_lng: f64, b_lat: f64) -> f64 {
    let radius = 6371.0_f64;
    let d_lat = (b_lat - a_lat).to_radians();
    let d_lng = (b_lng - a_lng).to_radians();
    let lat1 = a_lat.to_radians();
    let lat2 = b_lat.to_radians();
    let h = (d_lat / 2.0).sin().powi(2)
        + lat1.cos() * lat2.cos() * (d_lng / 2.0).sin().powi(2);
    2.0 * radius * h.sqrt().asin()
}

/// 高德/OSRM 的折线都是 "lng,lat;lng,lat" 字符串；拼起来时去掉重复端点。
pub(crate) fn coalesce_polyline(parts: &[String]) -> String {
    let mut points: Vec<&str> = Vec::new();
    for part in parts {
        for point in part.split(';') {
            let point = point.trim();
            if !point.is_empty() && points.last().copied() != Some(point) {
                points.push(point);
            }
        }
    }
    points.join(";")
}

// ── Key / 数据源 ─────────────────────────────────────────────

pub(crate) fn amap_key() -> String {
    settingview::get_travel_amap_key()
}

fn provider_label() -> String {
    for id in [11, 10, 1, 0, 2, 3, 4, 5] {
        if let Ok(provider) = llm_provider::resolve_chat(id) {
            return provider.label;
        }
    }
    "未配置模型".to_string()
}

fn settings_snapshot(app: &AppHandle) -> TravelSettings {
    let own = settingview::get_travel_amap_key_own();
    let album = settingview::get_life_album_amap_key();
    let effective = if !own.trim().is_empty() {
        own.clone()
    } else {
        album.clone()
    };
    let root = store::root(app)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let plan_count = store::list_plans(app).map(|items| items.len()).unwrap_or(0);
    TravelSettings {
        amap_key_set: !effective.trim().is_empty(),
        amap_key_tail: effective
            .trim()
            .chars()
            .rev()
            .take(4)
            .collect::<String>()
            .chars()
            .rev()
            .collect(),
        own_key_set: !own.trim().is_empty(),
        album_key_set: !album.trim().is_empty(),
        data_root: root,
        plan_count,
        provider_label: provider_label(),
        source: if effective.trim().is_empty() {
            "osm".to_string()
        } else {
            "amap".to_string()
        },
    }
}

// ── 命令：设置 ─────────────────────────────────────────────

#[tauri::command]
pub fn travel_get_settings(app: AppHandle) -> TravelSettings {
    settings_snapshot(&app)
}

#[tauri::command]
pub fn travel_set_amap_key(app: AppHandle, key: String) -> TravelSettings {
    settingview::set_travel_amap_key_value(&key);
    settings_snapshot(&app)
}

// ── 命令：定位与地点搜索 ─────────────────────────────────────

#[tauri::command]
pub async fn travel_ip_location() -> Result<Place, String> {
    let key = amap_key();
    if !key.trim().is_empty() {
        if let Ok(place) = amap::ip_location(&key).await {
            if place.lng.abs() > 0.0001 && place.lat.abs() > 0.0001 {
                if let Ok(detail) = amap::regeo(&key, place.lng, place.lat).await {
                    return Ok(Place {
                        id: detail.id,
                        name: if detail.name.trim().is_empty() {
                            place.name.clone()
                        } else {
                            detail.name
                        },
                        address: detail.address,
                        city: if detail.city.is_empty() {
                            place.city
                        } else {
                            detail.city
                        },
                        adcode: if detail.adcode.is_empty() {
                            place.adcode
                        } else {
                            detail.adcode
                        },
                        city_code: if detail.city_code.is_empty() {
                            place.city_code
                        } else {
                            detail.city_code
                        },
                        lng: place.lng,
                        lat: place.lat,
                        source: "amap".to_string(),
                    });
                }
                return Ok(place);
            }
        }
    }
    osm::ip_location().await
}

#[tauri::command]
pub async fn travel_search_places(keyword: String, city: Option<String>) -> Result<Vec<Place>, String> {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Ok(Vec::new());
    }
    let city = city.unwrap_or_default();
    let key = amap_key();
    if !key.trim().is_empty() {
        match amap::inputtips(&key, keyword, &city).await {
            Ok(items) if !items.is_empty() => return Ok(items),
            Ok(_) => {}
            Err(error) => {
                // 高德失败时不阻断用户，继续走免费源。
                if let Ok(items) = osm::nominatim_search(keyword, &city).await {
                    if !items.is_empty() {
                        return Ok(items);
                    }
                }
                return Err(error);
            }
        }
    }
    osm::nominatim_search(keyword, &city).await
}

// ── 命令：事实采集 ─────────────────────────────────────────

async fn resolve_place(input: &PlaceRef, key: &str) -> Result<Place, String> {
    if input.lng.abs() > 0.0001 && input.lat.abs() > 0.0001 {
        return Ok(Place {
            name: input.name.clone(),
            address: input.address.clone(),
            city: input.city.clone(),
            lng: input.lng,
            lat: input.lat,
            source: if key.trim().is_empty() {
                "osm".to_string()
            } else {
                "amap".to_string()
            },
            ..Place::default()
        });
    }
    if key.trim().is_empty() {
        return osm::geocode(&input.name, &input.city).await;
    }
    amap::geocode(key, &input.name, &input.city).await
}

fn dedupe_pois(items: &mut Vec<Poi>) {
    let mut seen = std::collections::HashSet::new();
    items.retain(|poi| {
        let key = if poi.id.trim().is_empty() {
            format!("{}|{:.4},{:.4}", poi.name, poi.lng, poi.lat)
        } else {
            poi.id.clone()
        };
        seen.insert(key)
    });
}

fn poi_matches_avoid(poi: &Poi, avoid: &[String]) -> bool {
    if avoid.is_empty() {
        return false;
    }
    let haystack = format!("{} {} {}", poi.name, poi.category, poi.address).to_lowercase();
    avoid
        .iter()
        .map(|item| item.trim().to_lowercase())
        .filter(|item| !item.is_empty())
        .any(|item| haystack.contains(&item))
}

fn apply_poi_filters(
    mut items: Vec<Poi>,
    avoid: &[String],
    center_lng: f64,
    center_lat: f64,
) -> Vec<Poi> {
    items.retain(|poi| !poi_matches_avoid(poi, avoid));
    for poi in items.iter_mut() {
        if poi.distance.is_none() {
            poi.distance = Some(haversine_km(center_lng, center_lat, poi.lng, poi.lat) * 1000.0);
        }
    }
    dedupe_pois(&mut items);
    items.sort_by(|a, b| {
        let rating = b.rating.unwrap_or(0.0).partial_cmp(&a.rating.unwrap_or(0.0)).unwrap();
        rating.then_with(|| {
            a.distance
                .unwrap_or(f64::MAX)
                .partial_cmp(&b.distance.unwrap_or(f64::MAX))
                .unwrap()
        })
    });
    items
}

async fn collect_amap(request: &TravelRequest, key: &str) -> Result<TravelFacts, String> {
    let origin = resolve_place(&request.origin, key).await?;
    let destination = resolve_place(&request.destination, key).await?;
    if destination.lng.abs() < 0.0001 || destination.lat.abs() < 0.0001 {
        return Err("没有解析到目的地的坐标，请重新搜索目的地".to_string());
    }
    let radius_m = (request.radius_km.unwrap_or(20.0) * 1000.0).clamp(3000.0, 50_000.0);

    let mut notes = Vec::new();
    let mut attractions = amap::place_around(
        key,
        destination.lng,
        destination.lat,
        radius_m,
        "110000|110101|140100",
        "",
    )
    .await
    .unwrap_or_default();
    if attractions.len() < 6 {
        if let Ok(more) = amap::place_text(key, "景点", &destination.city).await {
            attractions.extend(more);
        }
    }
    for must in &request.must_visit {
        if must.trim().is_empty() {
            continue;
        }
        if let Ok(mut more) = amap::place_text(key, must.trim(), &destination.city).await {
            attractions.append(&mut more);
        }
    }

    let food = amap::place_around(
        key,
        destination.lng,
        destination.lat,
        radius_m.min(10_000.0),
        "050000",
        "",
    )
    .await
    .unwrap_or_default();
    let hotels = amap::place_around(
        key,
        destination.lng,
        destination.lat,
        radius_m.min(20_000.0),
        "100000",
        "",
    )
    .await
    .unwrap_or_default();

    let mut attractions = apply_poi_filters(attractions, &request.avoid, destination.lng, destination.lat);
    let mut food = apply_poi_filters(food, &request.avoid, destination.lng, destination.lat);
    let mut hotels = apply_poi_filters(hotels, &request.avoid, destination.lng, destination.lat);
    attractions.truncate(24);
    food.truncate(18);
    hotels.truncate(10);

    let mut weather_days = Vec::new();
    if !destination.adcode.trim().is_empty() {
        if let Ok(items) = amap::weather(key, &destination.adcode).await {
            weather_days = items;
        }
    }
    let open_meteo = weather::open_meteo(destination.lat, destination.lng, 16)
        .await
        .unwrap_or_default();
    let weather_days = weather::merge(weather_days, open_meteo);

    notes.push(format!(
        "高德数据：{} 个景点、{} 家餐厅、{} 家住宿；POI 搜索按月配额缓存 7 天。",
        attractions.len(),
        food.len(),
        hotels.len()
    ));
    if weather_days.is_empty() {
        notes.push("天气服务暂不可用，出发前请再确认一次预报。".to_string());
    }

    Ok(TravelFacts {
        source: "amap".to_string(),
        origin,
        destination,
        attractions,
        food,
        hotels,
        weather: weather_days,
        notes,
        collected_at: chrono::Local::now().to_rfc3339(),
    })
}

async fn collect_osm(request: &TravelRequest) -> Result<TravelFacts, String> {
    let origin = if request.origin.lng.abs() > 0.0001 && request.origin.lat.abs() > 0.0001 {
        Place {
            name: request.origin.name.clone(),
            address: request.origin.address.clone(),
            city: request.origin.city.clone(),
            lng: request.origin.lng,
            lat: request.origin.lat,
            source: "osm".to_string(),
            ..Place::default()
        }
    } else {
        osm::geocode(&request.origin.name, &request.origin.city).await?
    };
    let destination = if request.destination.lng.abs() > 0.0001
        && request.destination.lat.abs() > 0.0001
    {
        Place {
            name: request.destination.name.clone(),
            address: request.destination.address.clone(),
            city: request.destination.city.clone(),
            lng: request.destination.lng,
            lat: request.destination.lat,
            source: "osm".to_string(),
            ..Place::default()
        }
    } else {
        osm::geocode(&request.destination.name, &request.destination.city).await?
    };
    if destination.lng.abs() < 0.0001 || destination.lat.abs() < 0.0001 {
        return Err("没有解析到目的地的坐标，请重新搜索目的地".to_string());
    }

    let radius_m = (request.radius_km.unwrap_or(20.0) * 1000.0).clamp(3000.0, 40_000.0);
    let (attractions, food, hotels) =
        osm::overpass_pois(destination.lat, destination.lng, radius_m).await?;
    let mut attractions = apply_poi_filters(attractions, &request.avoid, destination.lng, destination.lat);
    let mut food = apply_poi_filters(food, &request.avoid, destination.lng, destination.lat);
    let mut hotels = apply_poi_filters(hotels, &request.avoid, destination.lng, destination.lat);
    attractions.truncate(24);
    food.truncate(18);
    hotels.truncate(10);

    let weather_days = weather::open_meteo(destination.lat, destination.lng, 16)
        .await
        .unwrap_or_default();

    let mut notes = vec![
        "未启用高德 Key，当前使用 OpenStreetMap / OSRM / Open-Meteo。POI 名称与营业信息可能不如高德完整，公交路线不可用。".to_string(),
    ];
    if attractions.is_empty() {
        notes.push("周边 POI 检索结果较少，模型会基于目的地常识给出建议，出发前请核实。".to_string());
    }

    Ok(TravelFacts {
        source: "osm".to_string(),
        origin,
        destination,
        attractions,
        food,
        hotels,
        weather: weather_days,
        notes,
        collected_at: chrono::Local::now().to_rfc3339(),
    })
}

#[tauri::command]
pub async fn travel_collect_facts(request: TravelRequest) -> Result<TravelFacts, String> {
    let key = amap_key();
    let prefer_osm = request.source.eq_ignore_ascii_case("osm");
    let mut amap_error = String::new();

    if !prefer_osm && !key.trim().is_empty() {
        match collect_amap(&request, &key).await {
            Ok(facts) => return Ok(facts),
            Err(error) => {
                amap_error = error;
            }
        }
    }
    if request.source.eq_ignore_ascii_case("amap") && key.trim().is_empty() {
        return Err("未配置高德 Web 服务 Key。请在「行迹 → 设置」中填写，或把数据源切换为自动/OSM。".to_string());
    }

    match collect_osm(&request).await {
        Ok(mut facts) => {
            if !amap_error.is_empty() {
                facts.notes.insert(
                    0,
                    format!("高德请求失败，已自动降级到 OpenStreetMap：{}", truncate(&amap_error, 160)),
                );
            }
            Ok(facts)
        }
        Err(osm_error) => {
            if !amap_error.is_empty() {
                Err(format!(
                    "高德与 OpenStreetMap 均不可用。高德：{}；OSM：{}",
                    truncate(&amap_error, 160),
                    truncate(&osm_error, 160)
                ))
            } else {
                Err(osm_error)
            }
        }
    }
}

// ── 命令：路线 ─────────────────────────────────────────────

fn transport_mode(transport: &str, distance_km: f64) -> &'static str {
    match transport {
        "walking" => "walking",
        "transit" => "transit",
        "tour" => {
            if distance_km > 15.0 {
                "driving"
            } else {
                "walking"
            }
        }
        _ => "driving",
    }
}

fn fallback_leg(from: &RouteStop, to: &RouteStop, mode: &str) -> RouteLeg {
    let distance = haversine_km(from.lng, from.lat, to.lng, to.lat) * 1.25;
    let speed = match mode {
        "walking" => 4.5,
        "transit" => 22.0,
        _ => 42.0,
    };
    RouteLeg {
        mode: mode.to_string(),
        distance,
        duration: distance / speed * 3600.0,
        traffic: String::new(),
        summary: format!(
            "直线约 {:.1} km，按{}估算 {:.0} 分钟",
            distance,
            if mode == "walking" {
                "步行"
            } else if mode == "transit" {
                "公共交通"
            } else {
                "驾车"
            },
            distance / speed * 60.0
        ),
        steps: vec![RouteStep {
            instruction: format!("从 {} 前往 {}（未获取到逐段导航，按直线距离估算）", from.name, to.name),
            road: String::new(),
            distance,
            duration: distance / speed * 3600.0,
            ..RouteStep::default()
        }],
        warning: "路线服务暂不可用，已按直线距离估算时间，出发前请用高德/地图 App 复核。".to_string(),
        ..RouteLeg::default()
    }
}

#[tauri::command]
pub async fn travel_build_routes(request: RoutePlanRequest) -> Result<Vec<RouteDayResult>, String> {
    let key = amap_key();
    let mut results = Vec::new();

    for day in &request.days {
        let mut legs = Vec::new();
        for pair in day.stops.windows(2) {
            let from = &pair[0];
            let to = &pair[1];
            let distance_km = haversine_km(from.lng, from.lat, to.lng, to.lat);
            let mode = transport_mode(&request.transport, distance_km);
            let mut leg = if !key.trim().is_empty() {
                match amap::route(&key, mode, from, to).await {
                    Ok(leg) => leg,
                    Err(_) => osm::osrm_route(mode, from, to)
                        .await
                        .unwrap_or_else(|_| fallback_leg(from, to, mode)),
                }
            } else {
                osm::osrm_route(mode, from, to)
                    .await
                    .unwrap_or_else(|_| fallback_leg(from, to, mode))
            };
            if leg.mode.is_empty() {
                leg.mode = mode.to_string();
            }
            if leg.summary.is_empty() {
                leg.summary = format!(
                    "{} · {:.1} km · 约 {:.0} 分钟",
                    leg.mode,
                    leg.distance,
                    leg.duration / 60.0
                );
            }
            legs.push(leg);
        }
        results.push(RouteDayResult {
            date: day.date.clone(),
            legs,
        });
    }
    Ok(results)
}

// ── 命令：LLM ──────────────────────────────────────────────

#[tauri::command]
pub async fn travel_llm_complete(request: TravelLlmRequest) -> Result<TravelLlmResponse, String> {
    let prompt = request.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("prompt 不能为空".to_string());
    }
    let response = plugin_llm::complete(PluginLlmRequest {
        prompt,
        system: request.system,
        model_id: request.model_id,
        temperature: request.temperature.or(Some(0.35)),
        max_tokens: request.max_tokens.or(Some(4096)),
    })
    .await?;
    Ok(TravelLlmResponse {
        text: response.text,
        provider: response.provider.unwrap_or_else(|| "未知模型".to_string()),
    })
}

// ── 命令：静态地图与存储 ────────────────────────────────────

#[tauri::command]
pub async fn travel_static_map(request: StaticMapRequest) -> Result<String, String> {
    let key = amap_key();
    if key.trim().is_empty() {
        return Err("未配置高德 Key，无法生成静态地图".to_string());
    }
    amap::static_map(
        &key,
        request.center_lng,
        request.center_lat,
        request.zoom.unwrap_or(12),
        request.width.unwrap_or(750),
        request.height.unwrap_or(420),
        &request.path,
    )
    .await
}

#[tauri::command]
pub fn travel_save_plan(app: AppHandle, plan: Value) -> Result<Value, String> {
    store::save_plan(&app, plan)
}

#[tauri::command]
pub fn travel_list_plans(app: AppHandle) -> Result<Vec<Value>, String> {
    store::list_plans(&app)
}

#[tauri::command]
pub fn travel_get_plan(app: AppHandle, plan_id: String) -> Result<Value, String> {
    store::get_plan(&app, &plan_id)
}

#[tauri::command]
pub fn travel_delete_plan(app: AppHandle, plan_id: String) -> Result<(), String> {
    store::delete_plan(&app, &plan_id)
}

// ── 命令：分享与打印 ────────────────────────────────────────

#[tauri::command]
pub async fn travel_share_start(
    app: AppHandle,
    plan_id: String,
    html: String,
) -> Result<share::ShareStatus, String> {
    share::start(&app, &plan_id, &html).await
}

#[tauri::command]
pub async fn travel_share_stop() -> Result<(), String> {
    share::stop().await
}

#[tauri::command]
pub async fn travel_share_status() -> share::ShareStatus {
    share::status().await
}

#[tauri::command]
pub async fn travel_open_print_window(
    app: AppHandle,
    plan_id: String,
    theme: Option<String>,
) -> Result<(), String> {
    let plan_id = store::sanitize_id(&plan_id)?;
    let label = format!("app-travel-print-{}", plan_id);
    if let Some(window) = app.get_webview_window(&label) {
        window.show().map_err(|e| format!("显示打印窗口失败: {e}"))?;
        window.set_focus().ok();
        return Ok(());
    }
    let theme = theme
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let url = if theme == "dark" || theme == "light" {
        format!("/travel-print?planId={}&theme={}", plan_id, theme)
    } else {
        format!("/travel-print?planId={}", plan_id)
    };
    let window = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
        .title("行迹 · 打印行程")
        .inner_size(900.0, 1100.0)
        .min_inner_size(720.0, 800.0)
        .center()
        .visible(false)
        .build()
        .map_err(|e| format!("创建打印窗口失败: {e}"))?;
    tokio::time::sleep(Duration::from_millis(120)).await;
    window.show().map_err(|e| format!("显示打印窗口失败: {e}"))?;
    window.set_focus().ok();
    Ok(())
}
