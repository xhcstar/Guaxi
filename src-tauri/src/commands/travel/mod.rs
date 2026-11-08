//! 旅行计划应用后端。
//!
//! 前端只负责表单、编排提示词和渲染；定位、POI、路线、天气、模型调用与磁盘存储都留在这里。
//! 这样高德/模型密钥不会进入 WebView，同时绕开前端 CSP 与跨域限制。
//!
//! 数据源是双通道：有高德 Web 服务 Key 时优先高德；没有 Key 或请求失败时自动降级到
//! Nominatim / Overpass / OSRM / Open-Meteo。降级只影响精度与公交能力，不影响应用可用性。

mod amap;
mod cache;
mod osm;
mod preferences;
mod providers;
mod share;
mod store;
mod weather;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Listener, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::commands::llm_provider;
use crate::commands::plugin_llm::{self, PluginLlmRequest};
use crate::commands::settingview;

pub(crate) const POI_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
/// 路况/路线不缓存太久：出发前重新规划时才不会拿几天前的拥堵信息。
pub(crate) const ROUTE_TTL: Duration = Duration::from_secs(2 * 3600);
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
    pub adcode: String,
    #[serde(default)]
    pub city_code: String,
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
    /// auto | amap | baidu | tencent | google | mapbox | osm
    #[serde(default)]
    pub map_provider: String,
    /// trip | weekend：周末模式会把候选扩到峪口、村镇、集市、徒步点等出游目的地。
    #[serde(default)]
    pub mode: String,
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
    /// gcj02 | bd09 | wgs84，地图显示按它选择瓦片或转换坐标
    #[serde(default)]
    pub coord_system: String,
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
pub struct RouteOption {
    /// 推荐 / 躲避拥堵 / 距离优先 / 方案 1
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub summary: String,
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
    pub polyline: String,
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
    /// 2-3 条可选路线；第一条是推荐方案。界面上不再展开逐段导航。
    #[serde(default)]
    pub alternatives: Vec<RouteOption>,
    #[serde(default)]
    pub from_lng: f64,
    #[serde(default)]
    pub from_lat: f64,
    #[serde(default)]
    pub to_lng: f64,
    #[serde(default)]
    pub to_lat: f64,
    #[serde(default)]
    pub from_name: String,
    #[serde(default)]
    pub to_name: String,
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
    #[serde(default)]
    pub provider: String,
    /// 是否为每段路线请求 2-3 条备选；周末班次草稿只用 1 条以减少调用。
    #[serde(default = "default_true")]
    pub alternatives: bool,
    pub days: Vec<RouteDayRequest>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteDayResult {
    pub date: String,
    /// legs[i] 对应 stops[i] -> stops[i + 1]
    pub legs: Vec<RouteLeg>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RouteWeatherPoint {
    #[serde(default)]
    pub name: String,
    pub lng: f64,
    pub lat: f64,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub time: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RouteWeatherRequest {
    /// auto | amap | baidu | tencent | google | mapbox | osm
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub season_date: String,
    #[serde(default)]
    pub points: Vec<RouteWeatherPoint>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RouteWeatherResult {
    pub provider: String,
    pub coord_system: String,
    pub fetched_at: String,
    pub season: String,
    pub season_tips: Vec<String>,
    pub points: Vec<weather::WeatherPoint>,
    pub notes: Vec<String>,
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
    /// 行迹的编排全部要求 JSON，默认开启。
    #[serde(default)]
    pub json_mode: bool,
    /// 结构化编排不需要思考链：关掉 reasoning，避免 token 被思考过程吃光。
    #[serde(default)]
    pub disable_reasoning: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelLlmResponse {
    pub text: String,
    pub provider: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelWebLlmRequest {
    pub provider: String,
    pub prompt: String,
    #[serde(default)]
    pub system: String,
    #[serde(default)]
    pub session_id: Option<String>,
    /// 等待网页模型回答的上限（毫秒）。调用方按场景缩短，缺省 240 秒。
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelWebLlmResponse {
    pub text: String,
    pub provider: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelLocalLlmRequest {
    pub provider: String,
    pub prompt: String,
    #[serde(default)]
    pub system: String,
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

/// 跨进程 SQLite 缓存：高德搜索只有约 100 次/天，必须在重启后仍然复用。
pub(crate) fn cache_get_ttl(key: &str) -> Option<Value> {
    cache::get(key, Duration::from_secs(0))
}

pub(crate) fn cache_put(key: &str, value: Value, ttl: Duration) {
    let _ = cache::put(key, &value, ttl);
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
        cache::bump(&url);
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
    cache::bump(url);
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

fn masked_map_keys() -> Value {
    let mut out = serde_json::Map::new();
    for provider in ["amap", "baidu", "tencent", "google", "mapbox"] {
        let key = settingview::get_travel_map_key(provider);
        let tail = key
            .chars()
            .rev()
            .take(4)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        out.insert(
            provider.to_string(),
            serde_json::json!({ "set": !key.is_empty(), "tail": tail }),
        );
    }
    Value::Object(out)
}

#[tauri::command]
pub fn travel_get_map_keys() -> Value {
    masked_map_keys()
}

#[tauri::command]
pub fn travel_set_map_key(provider: String, key: String) -> Result<Value, String> {
    let provider = provider.trim().to_lowercase();
    if !["amap", "baidu", "tencent", "google", "mapbox"].contains(&provider.as_str()) {
        return Err(format!("不支持的地图服务商：{provider}"));
    }
    settingview::set_travel_map_key_value(&provider, &key);
    Ok(masked_map_keys())
}

#[tauri::command]
pub fn travel_get_quota_usage() -> Result<Value, String> {
    cache::usage_today()
}

#[tauri::command]
pub fn travel_clear_cache() -> Result<(), String> {
    cache::clear()
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
            adcode: input.adcode.clone(),
            city_code: input.city_code.clone(),
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
    // 只按名字地理编码很容易跑偏（例如「兴庆小区」在西安和银川都有）。
    // 用户选地点时带了城市/行政区编码，就一定要用它把范围钉住（高德接受 adcode）。
    let city_hint = if !input.city.trim().is_empty() {
        input.city.trim().to_string()
    } else {
        input.adcode.trim().to_string()
    };
    let resolved = if key.trim().is_empty() {
        osm::geocode(&input.name, &input.city).await?
    } else {
        amap::geocode(key, &input.name, &city_hint).await?
    };
    let expected: String = input.adcode.trim().chars().take(4).collect();
    let actual: String = resolved.adcode.trim().chars().take(4).collect();
    if expected.len() == 4 && actual.len() == 4 && expected != actual {
        return Err(format!(
            "出发点「{}」解析到了{}，和设定的城市不一致。请重新选择出发点，或在设置里重新定位家庭地址。",
            input.name,
            if resolved.city.trim().is_empty() {
                resolved.name.clone()
            } else {
                resolved.city.clone()
            }
        ));
    }
    Ok(resolved)
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

/// 公交站、停车场、收费站这类“途经设施”不能当出游目的地/景点，
/// 但搜索「峪口」时经常会把同名公交站排在前面。
fn is_infra_poi(poi: &Poi) -> bool {
    const INFRA: &[&str] = &[
        "公交站",
        "地铁站",
        "停车场",
        "收费站",
        "加油站",
        "服务区",
        "充电站",
        "公共厕所",
        "售票处",
        "出入口",
        "火车站",
        "汽车站",
        "高速路口",
    ];
    let haystack = format!("{} {}", poi.name, poi.category);
    INFRA.iter().any(|keyword| haystack.contains(keyword))
}

/// 统一收尾候选地点：避开词过滤、按检索中心重算距离、**剔除超出半径的候选**、去重排序。
///
/// 高德的 around 搜索自带距离，但城市级文本搜索（景点兜底、必去地点）和 OSM 补点
/// 可能返回几十上百公里外的地点；半径是用户的硬约束，必须在进入模型之前就裁掉，
/// 否则模型会拿这些点排进行程。返回被剔除的数量，供调用方写进数据说明。
fn apply_poi_filters(
    mut items: Vec<Poi>,
    avoid: &[String],
    center_lng: f64,
    center_lat: f64,
    radius_m: f64,
) -> (Vec<Poi>, usize) {
    items.retain(|poi| !poi_matches_avoid(poi, avoid));
    // 途经设施（公交站/停车场等）不能被当成景点或周末目的地。
    items.retain(|poi| poi.kind != "sight" || !is_infra_poi(poi));
    for poi in items.iter_mut() {
        poi.distance = Some(haversine_km(center_lng, center_lat, poi.lng, poi.lat) * 1000.0);
    }
    let limit_m = radius_m + 500.0;
    let before = items.len();
    items.retain(|poi| poi.distance.unwrap_or(f64::MAX) <= limit_m);
    let dropped = before.saturating_sub(items.len());
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
    (items, dropped)
}

/// 按距离分环保留候选：半径越大，越不能让评分高的近处地点把远处的候选挤掉。
/// 每个环带保留评分靠前的若干条，保证从近郊到半径边缘都有地点可选。
fn pick_across_rings(items: Vec<Poi>, radius_m: f64, limit: usize) -> Vec<Poi> {
    if items.len() <= limit {
        return items;
    }
    let bands = 6usize;
    let per_band = (limit / bands).max(3);
    let mut out: Vec<Poi> = Vec::with_capacity(limit);
    let mut used = std::collections::HashSet::new();
    let key_of = |poi: &Poi| {
        if poi.id.trim().is_empty() {
            format!("{}|{:.5},{:.5}", poi.name, poi.lng, poi.lat)
        } else {
            poi.id.clone()
        }
    };
    for band in 0..bands {
        let lo = radius_m * band as f64 / bands as f64;
        let hi = radius_m * (band + 1) as f64 / bands as f64;
        let mut taken = 0usize;
        for poi in items.iter() {
            if taken >= per_band {
                break;
            }
            let distance = poi.distance.unwrap_or(0.0);
            if distance < lo || distance >= hi {
                continue;
            }
            if !used.insert(key_of(poi)) {
                continue;
            }
            out.push(poi.clone());
            taken += 1;
        }
    }
    for poi in items.iter() {
        if out.len() >= limit {
            break;
        }
        if used.insert(key_of(poi)) {
            out.push(poi.clone());
        }
    }
    out.truncate(limit);
    out
}

async fn collect_amap(request: &TravelRequest, key: &str) -> Result<TravelFacts, String> {
    let origin = resolve_place(&request.origin, key).await?;
    let destination = resolve_place(&request.destination, key).await?;
    if destination.lng.abs() < 0.0001 || destination.lat.abs() < 0.0001 {
        return Err("没有解析到目的地的坐标，请重新搜索目的地".to_string());
    }
    let weekend = request.mode.trim() == "weekend";
    // 用户可选 30/60/100 km；高德 around 接口最多支持 50 km，超出部分靠文本搜索补点，
    // 再统一按真实半径裁剪（见 apply_poi_filters）。
    let radius_m = (request.radius_km.unwrap_or(20.0) * 1000.0).clamp(3000.0, 150_000.0);
    let around_m = radius_m.min(50_000.0);
    // 周末要真的“出门”，餐饮/住宿候选也得覆盖到半径外圈，不能只搜家附近。
    let stay_radius_m = if weekend {
        radius_m.min(50_000.0)
    } else {
        radius_m.min(30_000.0)
    };

    let mut notes = Vec::new();
    let mut attractions = if weekend {
        amap::place_around_pages(
            key,
            destination.lng,
            destination.lat,
            around_m,
            "110000|110101|140100",
            "",
            2,
        )
        .await
        .unwrap_or_default()
    } else {
        amap::place_around(
            key,
            destination.lng,
            destination.lat,
            around_m,
            "110000|110101|140100",
            "",
        )
        .await
        .unwrap_or_default()
    };
    if attractions.len() < 6 {
        if let Ok(more) = amap::place_text(key, "景点", &destination.city).await {
            attractions.extend(more);
        }
    }
    // 半径超过高德 around 的 50 km 上限时，再用一次城市级文本搜索补候选，
    // 随后统一按真实半径过滤，避免 60/100 km 请求只拿到 50 km 内的点。
    if radius_m > around_m && attractions.len() < 24 {
        if let Ok(more) = amap::place_text(key, "风景名胜", &destination.city).await {
            attractions.extend(more);
        }
    }
    // 周末不是只逛“景点”：山野徒步、古镇集市、县城村庄都可能是目的地。
    // 两次文本检索各取 20 条，结果仍会按真实半径裁剪并按距离分环保留。
    if weekend {
        for keywords in [
            "峪口|古道|徒步|营地|水库|观景台",
            "古镇|古村|集市|乡镇|农家乐",
        ] {
            if let Ok(mut more) = amap::place_text(key, keywords, &destination.city).await {
                attractions.append(&mut more);
            }
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

    let mut food = if weekend {
        amap::place_around_pages(
            key,
            destination.lng,
            destination.lat,
            stay_radius_m,
            "050000",
            "",
            2,
        )
        .await
        .unwrap_or_default()
    } else {
        amap::place_around(
            key,
            destination.lng,
            destination.lat,
            stay_radius_m,
            "050000",
            "",
        )
        .await
        .unwrap_or_default()
    };
    let mut hotels = amap::place_around(
        key,
        destination.lng,
        destination.lat,
        stay_radius_m,
        "100000",
        "",
    )
    .await
    .unwrap_or_default();

    // 高德在小城市/新景点上偶尔会缺候选；用 OSM 补一份并转换到 GCJ-02，
    // 保证地图与路线仍然对齐。只在候选明显不足时触发，避免无谓消耗。
    if attractions.len() < 4 {
        if let Ok((sights, meals, stays)) =
            osm::overpass_pois(destination.lat, destination.lng, around_m).await
        {
            let convert = |mut poi: Poi| {
                let (lng, lat) = providers::coord::wgs84_to_gcj02(poi.lng, poi.lat);
                poi.lng = lng;
                poi.lat = lat;
                poi.source = "osm".to_string();
                poi.distance = Some(haversine_km(destination.lng, destination.lat, lng, lat) * 1000.0);
                poi
            };
            attractions.extend(sights.into_iter().map(convert));
            food.extend(meals.into_iter().map(convert));
            hotels.extend(stays.into_iter().map(convert));
            notes.push("高德候选较少，已用 OpenStreetMap 补充地点（坐标已转换为 GCJ-02）。".to_string());
        }
    }

    let (mut attractions, dropped_sights) =
        apply_poi_filters(attractions, &request.avoid, destination.lng, destination.lat, radius_m);
    let (mut food, dropped_food) =
        apply_poi_filters(food, &request.avoid, destination.lng, destination.lat, radius_m);
    let (mut hotels, dropped_hotels) =
        apply_poi_filters(hotels, &request.avoid, destination.lng, destination.lat, radius_m);
    let dropped = dropped_sights + dropped_food + dropped_hotels;
    if dropped > 0 {
        notes.push(format!(
            "有 {dropped} 个候选地点超出 {:.0} km 半径，已从候选里剔除。",
            radius_m / 1000.0
        ));
    }
    if weekend {
        attractions = pick_across_rings(attractions, radius_m, 48);
        food = pick_across_rings(food, radius_m, 48);
        hotels = pick_across_rings(hotels, radius_m, 16);
    } else {
        attractions.truncate(24);
        food.truncate(18);
        hotels.truncate(10);
    }

    let mut weather_days = Vec::new();
    if !destination.adcode.trim().is_empty() {
        if let Ok(items) = amap::weather(key, &destination.adcode).await {
            weather_days = items;
        }
    }
    let open_meteo = {
        let (weather_lng, weather_lat) = providers::coord::gcj02_to_wgs84(
            destination.lng,
            destination.lat,
        );
        weather::open_meteo(weather_lat, weather_lng, 16)
            .await
            .unwrap_or_default()
    };
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
        coord_system: "gcj02".to_string(),
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

    // Overpass 半径越大越慢，最多取 50 km：更大的半径需求只能给到 50 km 内的候选。
    let weekend = request.mode.trim() == "weekend";
    let radius_m = (request.radius_km.unwrap_or(20.0) * 1000.0).clamp(3000.0, 50_000.0);
    let (attractions, food, hotels) =
        osm::overpass_pois(destination.lat, destination.lng, radius_m).await?;
    let (mut attractions, dropped_sights) =
        apply_poi_filters(attractions, &request.avoid, destination.lng, destination.lat, radius_m);
    let (mut food, dropped_food) =
        apply_poi_filters(food, &request.avoid, destination.lng, destination.lat, radius_m);
    let (mut hotels, dropped_hotels) =
        apply_poi_filters(hotels, &request.avoid, destination.lng, destination.lat, radius_m);
    let dropped = dropped_sights + dropped_food + dropped_hotels;
    if weekend {
        attractions = pick_across_rings(attractions, radius_m, 48);
        food = pick_across_rings(food, radius_m, 48);
        hotels = pick_across_rings(hotels, radius_m, 16);
    } else {
        attractions.truncate(24);
        food.truncate(18);
        hotels.truncate(10);
    }

    let weather_days = weather::open_meteo(destination.lat, destination.lng, 16)
        .await
        .unwrap_or_default();

    let mut notes = vec![
        "未启用高德 Key，当前使用 OpenStreetMap / OSRM / Open-Meteo。POI 名称与营业信息可能不如高德完整，公交路线不可用。".to_string(),
    ];
    if dropped > 0 {
        notes.push(format!(
            "有 {dropped} 个候选地点超出 {:.0} km 半径，已从候选里剔除。",
            radius_m / 1000.0
        ));
    }
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
        coord_system: "wgs84".to_string(),
    })
}

async fn resolve_provider_place(
    provider: providers::MapProvider,
    key: &str,
    input: &PlaceRef,
) -> Result<Place, String> {
    if input.lng.abs() > 0.0001 && input.lat.abs() > 0.0001 {
        return Ok(Place {
            name: input.name.clone(),
            address: input.address.clone(),
            city: input.city.clone(),
            adcode: input.adcode.clone(),
            city_code: input.city_code.clone(),
            lng: input.lng,
            lat: input.lat,
            source: providers::label(provider).to_string(),
            ..Place::default()
        });
    }
    let city_hint = if !input.city.trim().is_empty() {
        input.city.trim().to_string()
    } else {
        input.adcode.trim().to_string()
    };
    match provider {
        providers::MapProvider::Baidu => providers::baidu::geocode(key, &input.name, &city_hint).await,        providers::MapProvider::Tencent => {
            providers::tencent::geocode(key, &input.name, &city_hint).await
        }
        providers::MapProvider::Google => providers::google::geocode(key, &input.name, &city_hint).await,
        providers::MapProvider::Mapbox => providers::mapbox::geocode(key, &input.name, &city_hint).await,
        _ => Err("不支持的地图服务商".to_string()),
    }
}

async fn provider_pois(
    provider: providers::MapProvider,
    key: &str,
    kind: &str,
    city: &str,
    lng: f64,
    lat: f64,
    radius_m: f64,
) -> Result<Vec<Poi>, String> {
    let keyword = match kind {
        "food" => "美食",
        "hotel" => "酒店",
        _ => "景点",
    };
    match provider {
        providers::MapProvider::Baidu => {
            providers::baidu::place_text(key, keyword, city, kind, lng, lat).await
        }
        providers::MapProvider::Tencent => {
            providers::tencent::place_text(key, keyword, city, kind, lng, lat).await
        }
        providers::MapProvider::Google => {
            let place_type = match kind {
                "food" => "restaurant",
                "hotel" => "lodging",
                _ => "tourist_attraction",
            };
            providers::google::place_nearby(key, lat, lng, radius_m, kind, place_type).await
        }
        providers::MapProvider::Mapbox => {
            providers::mapbox::place_text(key, keyword, city, kind, lng, lat).await
        }
        _ => Err("不支持的地图服务商".to_string()),
    }
}

async fn collect_provider(
    request: &TravelRequest,
    provider: providers::MapProvider,
    key: &str,
) -> Result<TravelFacts, String> {
    let origin = resolve_provider_place(provider, key, &request.origin).await?;
    let destination = resolve_provider_place(provider, key, &request.destination).await?;
    if destination.lng.abs() < 0.0001 || destination.lat.abs() < 0.0001 {
        return Err("没有解析到目的地的坐标，请重新搜索目的地".to_string());
    }
    let city = if destination.city.trim().is_empty() {
        destination.name.clone()
    } else {
        destination.city.clone()
    };
    let radius_m = (request.radius_km.unwrap_or(20.0) * 1000.0).clamp(3000.0, 150_000.0);
    let weekend = request.mode.trim() == "weekend";
    let stay_radius_m = if weekend {
        radius_m.min(50_000.0)
    } else {
        radius_m.min(30_000.0)
    };
    let mut notes = Vec::new();

    let mut attractions = provider_pois(
        provider,
        key,
        "sight",
        &city,
        destination.lng,
        destination.lat,
        radius_m,
    )
    .await
    .unwrap_or_default();
    let mut food = provider_pois(
        provider,
        key,
        "food",
        &city,
        destination.lng,
        destination.lat,
        stay_radius_m,
    )
    .await
    .unwrap_or_default();
    let mut hotels = provider_pois(
        provider,
        key,
        "hotel",
        &city,
        destination.lng,
        destination.lat,
        stay_radius_m,
    )
    .await
    .unwrap_or_default();

    // Mapbox 的 POI 检索能力有限；不足时补一份 OSM 候选，坐标同为 WGS84。
    if provider == providers::MapProvider::Mapbox && attractions.len() < 4 {
        if let Ok((sights, meals, stays)) =
            osm::overpass_pois(destination.lat, destination.lng, radius_m.min(50_000.0)).await
        {
            attractions.extend(sights);
            food.extend(meals);
            hotels.extend(stays);
            notes.push("Mapbox POI 不足，已补充 OpenStreetMap 候选地点。".to_string());
        }
    }

    let (mut attractions, dropped_sights) =
        apply_poi_filters(attractions, &request.avoid, destination.lng, destination.lat, radius_m);
    let (mut food, dropped_food) =
        apply_poi_filters(food, &request.avoid, destination.lng, destination.lat, radius_m);
    let (mut hotels, dropped_hotels) =
        apply_poi_filters(hotels, &request.avoid, destination.lng, destination.lat, radius_m);
    let dropped = dropped_sights + dropped_food + dropped_hotels;
    if dropped > 0 {
        notes.push(format!(
            "有 {dropped} 个候选地点超出 {:.0} km 半径，已从候选里剔除。",
            radius_m / 1000.0
        ));
    }
    if weekend {
        attractions = pick_across_rings(attractions, radius_m, 48);
        food = pick_across_rings(food, radius_m, 48);
        hotels = pick_across_rings(hotels, radius_m, 16);
    } else {
        attractions.truncate(24);
        food.truncate(18);
        hotels.truncate(10);
    }

    let (weather_lng, weather_lat) =
        providers::coord::to_wgs84(provider, destination.lng, destination.lat);
    let weather_days = weather::open_meteo(weather_lat, weather_lng, 16)
        .await
        .unwrap_or_default();

    notes.insert(
        0,
        format!(
            "{}：{} 个景点、{} 家餐厅、{} 家住宿。",
            providers::display_name(provider),
            attractions.len(),
            food.len(),
            hotels.len()
        ),
    );
    if provider == providers::MapProvider::Baidu || provider == providers::MapProvider::Tencent {
        if request.transport == "transit" {
            notes.push("该服务商的公交接口需要额外城市编码，公交路段会按驾车参考并在计划中标注。".to_string());
        }
    }

    Ok(TravelFacts {
        source: providers::label(provider).to_string(),
        origin,
        destination,
        attractions,
        food,
        hotels,
        weather: weather_days,
        notes,
        collected_at: chrono::Local::now().to_rfc3339(),
        coord_system: providers::coord_system(provider).to_string(),
    })
}

#[tauri::command]
pub async fn travel_collect_facts(request: TravelRequest) -> Result<TravelFacts, String> {
    let raw = if request.map_provider.trim().is_empty() {
        request.source.as_str()
    } else {
        request.map_provider.as_str()
    };
    let provider = providers::normalize(raw);
    match provider {
        providers::MapProvider::Osm => collect_osm(&request).await,
        providers::MapProvider::Amap => {
            let key = amap_key();
            if key.trim().is_empty() {
                return Err("未配置高德 Web 服务 Key。请在「行迹 → 设置」中填写，或把地图服务商切换为自动/OpenStreetMap。".to_string());
            }
            collect_amap(&request, &key).await
        }
        providers::MapProvider::Auto => {
            let key = amap_key();
            if key.trim().is_empty() {
                return collect_osm(&request).await;
            }
            match collect_amap(&request, &key).await {
                Ok(facts) => Ok(facts),
                Err(amap_error) => match collect_osm(&request).await {
                    Ok(mut facts) => {
                        facts.notes.insert(
                            0,
                            format!(
                                "高德请求失败，已自动降级到 OpenStreetMap：{}",
                                truncate(&amap_error, 160)
                            ),
                        );
                        Ok(facts)
                    }
                    Err(osm_error) => Err(format!(
                        "高德与 OpenStreetMap 均不可用。高德：{}；OSM：{}",
                        truncate(&amap_error, 160),
                        truncate(&osm_error, 160)
                    )),
                },
            }
        }
        other => {
            let key = providers::key(other);
            if key.trim().is_empty() {
                return Err(format!(
                    "未配置 {} Key。请在「行迹 → 设置 → 地图服务商」中填写，或切换回高德/OpenStreetMap。",
                    providers::display_name(other)
                ));
            }
            collect_provider(&request, other, &key).await
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
    let duration = distance / speed * 3600.0;
    let summary = format!(
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
    );
    let option = RouteOption {
        label: "估算".to_string(),
        summary: summary.clone(),
        distance,
        duration,
        ..RouteOption::default()
    };
    RouteLeg {
        mode: mode.to_string(),
        distance,
        duration,
        traffic: String::new(),
        summary,
        steps: vec![RouteStep {
            instruction: format!("从 {} 前往 {}（未获取到导航，按直线距离估算）", from.name, to.name),
            road: String::new(),
            distance,
            duration,
            ..RouteStep::default()
        }],
        alternatives: vec![option],
        from_lng: from.lng,
        from_lat: from.lat,
        to_lng: to.lng,
        to_lat: to.lat,
        from_name: from.name.clone(),
        to_name: to.name.clone(),
        warning: "路线服务暂不可用，已按直线距离估算时间，出发前请用地图 App 复核。".to_string(),
        ..RouteLeg::default()
    }
}

async fn provider_route(
    provider: providers::MapProvider,
    mode: &str,
    from: &RouteStop,
    to: &RouteStop,
    with_alternatives: bool,
) -> Result<RouteLeg, String> {
    let provider = match provider {
        providers::MapProvider::Auto => {
            if amap_key().trim().is_empty() {
                providers::MapProvider::Osm
            } else {
                providers::MapProvider::Amap
            }
        }
        other => other,
    };
    let key = if provider == providers::MapProvider::Amap {
        amap_key()
    } else {
        providers::key(provider)
    };
    match provider {
        providers::MapProvider::Amap => {
            if key.trim().is_empty() {
                return Err("未配置高德 Key".to_string());
            }
            amap::route(&key, mode, from, to, with_alternatives).await
        }
        providers::MapProvider::Osm => {
            osm::osrm_route(mode, from, to, with_alternatives).await
        }
        providers::MapProvider::Baidu => {
            if key.trim().is_empty() {
                return Err("未配置百度地图 Key".to_string());
            }
            providers::baidu::route(&key, mode, from, to).await
        }
        providers::MapProvider::Tencent => {
            if key.trim().is_empty() {
                return Err("未配置腾讯地图 Key".to_string());
            }
            providers::tencent::route(&key, mode, from, to).await
        }
        providers::MapProvider::Google => {
            if key.trim().is_empty() {
                return Err("未配置 Google Maps Key".to_string());
            }
            providers::google::route(&key, mode, from, to).await
        }
        providers::MapProvider::Mapbox => {
            if key.trim().is_empty() {
                return Err("未配置 Mapbox Token".to_string());
            }
            providers::mapbox::route(&key, mode, from, to).await
        }
        providers::MapProvider::Auto => unreachable!(),
    }
}

#[tauri::command]
pub async fn travel_build_routes(request: RoutePlanRequest) -> Result<Vec<RouteDayResult>, String> {
    let provider = providers::normalize(&request.provider);
    let mut results = Vec::new();

    for day in &request.days {
        let mut legs = Vec::new();
        for pair in day.stops.windows(2) {
            let from = &pair[0];
            let to = &pair[1];
            let distance_km = haversine_km(from.lng, from.lat, to.lng, to.lat);
            let mode = transport_mode(&request.transport, distance_km);
            let mut leg = match provider_route(provider, mode, from, to, request.alternatives).await {
                Ok(leg) => leg,
                Err(error) => {
                    let mut fallback = fallback_leg(from, to, mode);
                    fallback.warning = format!(
                        "{}路线不可用，已按直线距离估算：{}",
                        providers::display_name(provider),
                        truncate(&error, 120)
                    );
                    fallback
                }
            };
            if leg.mode.is_empty() {
                leg.mode = mode.to_string();
            }
            if leg.alternatives.is_empty() {
                leg.alternatives.push(RouteOption {
                    label: "推荐".to_string(),
                    summary: leg.summary.clone(),
                    distance: leg.distance,
                    duration: leg.duration,
                    tolls: leg.tolls,
                    fare: leg.fare,
                    traffic: leg.traffic.clone(),
                    polyline: leg.polyline.clone(),
                });
            }
            leg.from_lng = from.lng;
            leg.from_lat = from.lat;
            leg.to_lng = to.lng;
            leg.to_lat = to.lat;
            leg.from_name = from.name.clone();
            leg.to_name = to.name.clone();
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

// ── 命令：沿途天气 ─────────────────────────────────────────

/// 按路线采样点批量获取逐时天气。坐标统一转换为 WGS84 后再查询 Open-Meteo；
/// 途经点没有名称时，用高德（有 Key）或 Nominatim 逆地理编码补上地区名。
#[tauri::command]
pub async fn travel_route_weather(
    request: RouteWeatherRequest,
) -> Result<RouteWeatherResult, String> {
    if request.points.is_empty() {
        return Ok(RouteWeatherResult::default());
    }
    if request.points.len() > 40 {
        return Err("沿途天气采样点过多".to_string());
    }
    let requested = providers::normalize(&request.provider);
    let provider = match requested {
        providers::MapProvider::Auto => {
            if amap_key().trim().is_empty() {
                providers::MapProvider::Osm
            } else {
                providers::MapProvider::Amap
            }
        }
        other => other,
    };
    let season_date = if request.season_date.trim().is_empty() {
        request
            .points
            .iter()
            .find(|point| !point.date.trim().is_empty())
            .map(|point| point.date.clone())
            .unwrap_or_default()
    } else {
        request.season_date.clone()
    };

    let tasks = request.points.iter().map(|point| {
        let name = point.name.clone();
        let date = point.date.clone();
        let time = if point.time.trim().is_empty() {
            "12:00".to_string()
        } else {
            point.time.clone()
        };
        let (lng, lat) = providers::coord::to_wgs84(provider, point.lng, point.lat);
        async move {
            let mut weather_point = weather::point_weather(name, lat, lng, &date, &time).await;
            weather_point.lng = lng;
            weather_point.lat = lat;
            weather_point
        }
    });
    let mut points = futures_util::future::join_all(tasks).await;

    // 只给没有名字的途经点补地区名，最多 3 个，避免拖慢生成和触发限流。
    let mut named = 0;
    for (index, point) in points.iter_mut().enumerate() {
        if named >= 3 {
            break;
        }
        if !point.name.trim().is_empty() && !point.name.starts_with("途经") {
            continue;
        }
        let source = &request.points[index];
        let resolved = match provider {
            providers::MapProvider::Amap => {
                let key = amap_key();
                if key.trim().is_empty() {
                    None
                } else {
                    let (lng, lat) =
                        providers::coord::to_gcj02(provider, source.lng, source.lat);
                    amap::regeo(&key, lng, lat).await.ok()
                }
            }
            providers::MapProvider::Osm => osm::reverse(source.lat, source.lng).await.ok(),
            _ => None,
        };
        if let Some(place) = resolved {
            let label = if place.city.trim().is_empty() {
                truncate(place.name.trim(), 18)
            } else {
                place.city.clone()
            };
            if !label.trim().is_empty() {
                point.name = label;
                named += 1;
            }
        }
    }

    let mut notes = Vec::new();
    let unavailable = points.iter().filter(|point| !point.available).count();
    if unavailable > 0 {
        notes.push(format!(
            "{unavailable} 个采样点暂无逐时预报，界面标为「临近更新」"
        ));
    }
    if points.len() > 1 && provider == providers::MapProvider::Osm {
        notes.push("途经地区名称来自 OpenStreetMap，天气来自 Open-Meteo".to_string());
    }
    Ok(RouteWeatherResult {
        provider: providers::display_name(provider).to_string(),
        coord_system: "wgs84".to_string(),
        fetched_at: chrono::Local::now().to_rfc3339(),
        season: weather::season_label(&season_date),
        season_tips: weather::season_tips(&season_date),
        points,
        notes,
    })
}

// ── 命令：LLM ──────────────────────────────────────────────

#[tauri::command]
pub async fn travel_llm_complete(request: TravelLlmRequest) -> Result<TravelLlmResponse, String> {
    let prompt = request.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("prompt 不能为空".to_string());
    }
    tracing::info!(
        model_id = ?request.model_id,
        prompt_chars = prompt.chars().count(),
        "[travel-llm] API 模型请求开始"
    );
    let response = plugin_llm::complete(PluginLlmRequest {
        prompt,
        system: request.system,
        model_id: request.model_id,
        temperature: request.temperature.or(Some(0.35)),
        max_tokens: request.max_tokens.or(Some(4096)),
        json_mode: request.json_mode,
        disable_reasoning: request.disable_reasoning,
    })
    .await
    .map_err(|error| {
        tracing::warn!(model_id = ?request.model_id, error = %error, "[travel-llm] API 模型失败");
        error
    })?;
    tracing::info!(
        provider = ?response.provider,
        chars = response.text.chars().count(),
        "[travel-llm] API 模型返回"
    );
    Ok(TravelLlmResponse {
        text: response.text,
        provider: response.provider.unwrap_or_else(|| "未知模型".to_string()),
    })
}

#[tauri::command]
pub async fn travel_llm_local_complete(
    request: TravelLocalLlmRequest,
) -> Result<TravelLlmResponse, String> {
    let prompt = request.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("prompt 不能为空".to_string());
    }
    let question = if request.system.trim().is_empty() {
        prompt
    } else {
        format!("{}\n\n{}", request.system.trim(), prompt)
    };
    let provider = request.provider.trim().to_lowercase();
    let (answer, label) = match provider.as_str() {
        "qwen" => (
            crate::commands::chatview::get_answer_from_local_qwen(&question, &[], "travel-local")
                .await?,
            "本地 Qwen",
        ),
        _ => (
            crate::commands::chatview::get_answer_from_local_deepseek(
                &question,
                &[],
                "travel-local",
            )
            .await?,
            "本地 DeepSeek",
        ),
    };
    Ok(TravelLlmResponse {
        text: answer.answer,
        provider: label.to_string(),
    })
}

/// 通过已登录的网页模型（ChatGPT / DeepSeek / Kimi / 千问 / 豆包 / 元宝）完成一次补全。
///
/// 网页模型由 chatview 的 capture 脚本把回答推回 `web-model-chat-push`。这里只负责：
/// 确保窗口存在、发送一次 prompt、按 session_id 收集回答，直到回答稳定或超时。
/// 需要登录时会由本地服务把网页窗口显示出来，用户登录一次即可复用会话。
#[tauri::command]
pub async fn travel_llm_web_complete(
    app: AppHandle,
    request: TravelWebLlmRequest,
) -> Result<TravelWebLlmResponse, String> {
    let provider = request.provider.trim().to_lowercase();
    let model_id = crate::commands::chatview::web_provider_model_id(&provider)
        .ok_or_else(|| format!("不支持网页模型：{provider}"))?;
    let prompt = request.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("prompt 不能为空".to_string());
    }
    let session_id = request
        .session_id
        .unwrap_or_else(|| format!("travel-{}", uuid::Uuid::new_v4().simple()));
    let timeout_secs = request
        .timeout_ms
        .map(|ms| (ms / 1000).clamp(20, 240))
        .unwrap_or(240);
    let content = if request.system.trim().is_empty() {
        prompt.clone()
    } else {
        format!("{}\n\n{}", request.system.trim(), prompt)
    };
    tracing::info!(
        provider = %provider,
        session_id = %session_id,
        timeout_secs = timeout_secs,
        prompt_chars = content.chars().count(),
        "[travel-llm] 网页模型请求开始"
    );

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let listener = app.listen("web-model-chat-push", move |event| {
        let _ = tx.send(event.payload().to_string());
    });

    let result: Result<String, String> = async {
        crate::commands::chatview::prepare_web_model_window(app.clone(), provider.clone()).await?;
        tokio::time::sleep(Duration::from_millis(700)).await;
        crate::commands::chatview::web_model_send_message(
            app.clone(),
            crate::commands::chatview::WebModelSendRequest {
                provider: provider.clone(),
                model_id,
                topic_id: 0,
                session_id: session_id.clone(),
                content: content.clone(),
                attachments: Vec::new(),
            },
        )
        .await?;

        let mut last_text = String::new();
        let mut last_change = Instant::now();
        let mut need_login = false;
        let mut send_failed = false;
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            if Instant::now() >= deadline {
                break;
            }
            match tokio::time::timeout(Duration::from_millis(2500), rx.recv()).await {
                Ok(Some(payload)) => {
                    let Ok(value) = serde_json::from_str::<Value>(&payload) else {
                        continue;
                    };
                    if value.get("session_id").and_then(|item| item.as_str())
                        != Some(session_id.as_str())
                    {
                        continue;
                    }
                    let status = value
                        .get("status")
                        .and_then(|item| item.as_str())
                        .unwrap_or("");
                    if status == "need_login" {
                        need_login = true;
                    } else if status == "sent" {
                        // 发送成功：丢掉发送前抓到的旧回答，只等这次的新内容。
                        last_text.clear();
                        last_change = Instant::now();
                    } else if status == "ready" && need_login {
                        // 登录完成后把同一条 prompt 再发一次，避免首次发送落在登录页上。
                        need_login = false;
                        let _ = crate::commands::chatview::web_model_send_message(
                            app.clone(),
                            crate::commands::chatview::WebModelSendRequest {
                                provider: provider.clone(),
                                model_id,
                                topic_id: 0,
                                session_id: session_id.clone(),
                                content: content.clone(),
                                attachments: Vec::new(),
                            },
                        )
                        .await;
                    } else if status == "send_failed" && last_text.trim().is_empty() {
                        // 页面里没找到输入框/发送按钮：立刻报错，别再干等超时。
                        send_failed = true;
                        break;
                    }
                    if let Some(messages) = value.get("messages").and_then(|item| item.as_array()) {
                        for message in messages {
                            if message.get("role").and_then(|item| item.as_str()) != Some("assistant") {
                                continue;
                            }
                            let text = message
                                .get("content")
                                .and_then(|item| item.as_str())
                                .unwrap_or("")
                                .trim();
                            if !text.is_empty() && text != last_text {
                                last_text = text.to_string();
                                last_change = Instant::now();
                            }
                        }
                    }
                }
                Ok(None) => break,
                Err(_) => {
                    if !last_text.is_empty() && last_change.elapsed() >= Duration::from_secs(2) {
                        break;
                    }
                }
            }
        }

        if send_failed && last_text.trim().is_empty() {
            Err("网页模型页面里没有找到可用的输入框或发送按钮。请在「设置 → 网页模型」打开登录窗口，确认页面能正常输入并发送。".to_string())
        } else if last_text.trim().is_empty() {
            Err(format!(
                "网页模型 {timeout_secs} 秒内没有返回内容。请到「设置 → 网页模型」打开登录窗口，确认已登录且页面能正常对话。"
            ))
        } else {
            Ok(last_text)
        }
    }
    .await;

    app.unlisten(listener);
    match &result {
        Ok(text) => tracing::info!(
            provider = %provider,
            chars = text.chars().count(),
            "[travel-llm] 网页模型返回"
        ),
        Err(error) => tracing::warn!(
            provider = %provider,
            timeout_secs = timeout_secs,
            error = %error,
            "[travel-llm] 网页模型失败"
        ),
    }
    result.map(|text| TravelWebLlmResponse { text, provider })
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
pub fn travel_get_preferences(app: AppHandle) -> Result<Value, String> {
    preferences::load(&app)
}

#[tauri::command]
pub fn travel_set_preferences(app: AppHandle, preferences: Value) -> Result<Value, String> {
    preferences::save(&app, preferences)
}

#[tauri::command]
pub fn travel_reset_preferences(app: AppHandle) -> Result<Value, String> {
    preferences::save(&app, serde_json::json!({}))
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
