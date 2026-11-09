//! 高德 Web 服务 API 适配。
//!
//! 全部请求由 Rust 发出：Key 不进 WebView，也不需要给前端 CSP 放行 `restapi.amap.com`。
//! 响应字段里高德经常用 `[]` 表示缺失字符串（例如 city、address、tel），统一用
//! `val_text` 兜底，避免前端拿到数组类型后渲染成 [object Object]。

use base64::engine::general_purpose;
use base64::Engine as _;
use serde_json::Value;
use std::time::Duration;

use super::{
    cached_json, coalesce_polyline, haversine_km, truncate, val_f64, val_text, Place, Poi,
    RouteLeg, RouteOption, RouteStep, RouteStop, WeatherDay, GEO_TTL, POI_TTL, WEATHER_TTL,
};

const AMAP_BASE: &str = "https://restapi.amap.com";

fn build_url(path: &str, params: &[(&str, String)]) -> Result<String, String> {
    let mut url = reqwest::Url::parse(&format!("{AMAP_BASE}{path}"))
        .map_err(|e| format!("构建高德请求地址失败: {e}"))?;
    {
        let mut query = url.query_pairs_mut();
        for (key, value) in params {
            query.append_pair(key, value);
        }
    }
    Ok(url.to_string())
}

fn ensure_ok(value: &Value) -> Result<(), String> {
    let status = value.get("status").and_then(|item| item.as_str()).unwrap_or("");
    if status == "1" {
        return Ok(());
    }
    Err(format!(
        "高德接口返回 {}（{}）",
        val_text(value.get("info")),
        val_text(value.get("infocode"))
    ))
}

/// 行政区层级落在 `district` 里（“省份+城市+区县”连写），而 `place/text` 的 `city`
/// 参数只认城市级名称或 adcode：原样传整串路径会静默返回 0 条候选。
const MUNICIPALITIES: [&str; 4] = ["北京市", "上海市", "天津市", "重庆市"];
const PROVINCE_SUFFIXES: [&str; 3] = ["特别行政区", "自治区", "省"];
// 按“后缀结束位置最早”取，避免把「伊犁哈萨克自治州伊宁市」误判成伊宁市。
const CITY_SUFFIXES: [&str; 5] = ["自治州", "地区", "盟", "市", "州"];

/// 从高德的 `district` 串收敛出城市级名称；解析不出来时回退到调用方给的城市提示。
pub(crate) fn city_from_district(district: &str, fallback: &str) -> String {
    let text = district.trim();
    if text.is_empty() {
        return fallback.trim().to_string();
    }
    for city in MUNICIPALITIES {
        if text.starts_with(city) {
            return city.to_string();
        }
    }
    let rest = match PROVINCE_SUFFIXES
        .iter()
        .filter_map(|suffix| text.find(suffix).map(|pos| pos + suffix.len()))
        .min()
    {
        Some(end) if end < text.len() => &text[end..],
        _ => text,
    };
    let mut best: Option<usize> = None;
    for suffix in CITY_SUFFIXES {
        if let Some(end) = rest.find(suffix).map(|pos| pos + suffix.len()) {
            if end < rest.len() {
                best = Some(best.map_or(end, |current| current.min(end)));
            }
        }
    }
    match best {
        Some(end) => rest[..end].to_string(),
        None => rest.to_string(),
    }
}

fn parse_location(raw: &str) -> Option<(f64, f64)> {
    let mut parts = raw.split(',');
    let lng = parts.next()?.trim().parse::<f64>().ok()?;
    let lat = parts.next()?.trim().parse::<f64>().ok()?;
    if lng.abs() < 0.0001 && lat.abs() < 0.0001 {
        return None;
    }
    Some((lng, lat))
}

fn place_from_json(value: &Value) -> Option<Place> {
    let (lng, lat) = parse_location(&val_text(value.get("location")))?;
    let province = val_text(value.get("province"));
    let raw_city = val_text(value.get("city"));
    let district = val_text(value.get("district"));
    // 直辖市的 city 字段为空，这时用省名兜底（「北京市」），按城市检索才不会落空。
    let city = if raw_city.is_empty() { province.clone() } else { raw_city };
    let adcode = val_text(value.get("adcode"));
    let city_code = val_text(value.get("citycode"));
    let formatted = val_text(value.get("formatted_address"));
    let name = if formatted.is_empty() {
        val_text(value.get("name"))
    } else {
        formatted
    };
    Some(Place {
        id: format!("amap-{adcode}-{lng:.6},{lat:.6}"),
        name,
        address: {
            let township = val_text(value.get("township"));
            [province, city.clone(), district, township]
                .into_iter()
                .filter(|item| !item.trim().is_empty())
                .collect::<Vec<_>>()
                .join("")
        },
        city,
        adcode,
        city_code,
        lng,
        lat,
        source: "amap".to_string(),
    })
}

fn poi_kind(typecode: &str) -> &'static str {
    if typecode.starts_with("05") {
        "food"
    } else if typecode.starts_with("10") {
        "hotel"
    } else {
        "sight"
    }
}

fn poi_from_json(value: &Value, center: Option<(f64, f64)>) -> Option<Poi> {
    let (lng, lat) = parse_location(&val_text(value.get("location")))?;
    let name = val_text(value.get("name"));
    if name.trim().is_empty() {
        return None;
    }
    let typecode = val_text(value.get("typecode"));
    let kind = poi_kind(&typecode).to_string();
    let type_name = val_text(value.get("type"));
    let category = type_name
        .split(';')
        .filter(|part| !part.trim().is_empty())
        .take(2)
        .collect::<Vec<_>>()
        .join(" · ");
    let tags: Vec<String> = type_name
        .split(';')
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .take(6)
        .collect();
    let biz = value.get("biz_ext");
    let rating = biz.and_then(|item| val_f64(item.get("rating")));
    let cost = biz.and_then(|item| val_f64(item.get("cost")));
    let distance = value
        .get("distance")
        .and_then(|item| val_f64(Some(item)))
        .or_else(|| center.map(|(clng, clat)| haversine_km(clng, clat, lng, lat) * 1000.0));
    Some(Poi {
        id: val_text(value.get("id")),
        name,
        source: "amap".to_string(),
        kind,
        category,
        address: val_text(value.get("address")),
        city: {
            let city = val_text(value.get("cityname"));
            if city.is_empty() {
                val_text(value.get("adname"))
            } else {
                city
            }
        },
        lng,
        lat,
        rating,
        cost,
        tel: val_text(value.get("tel")),
        distance,
        tags,
    })
}

pub async fn geocode(key: &str, address: &str, city: &str) -> Result<Place, String> {
    let url = build_url(
        "/v3/geocode/geo",
        &[
            ("key", key.to_string()),
            ("address", address.to_string()),
            ("city", city.to_string()),
        ],
    )?;
    let value = cached_json(&url, GEO_TTL).await?;
    ensure_ok(&value)?;
    let geocodes = value
        .get("geocodes")
        .and_then(|item| item.as_array())
        .ok_or_else(|| "高德没有返回地理编码结果".to_string())?;
    let first = geocodes
        .first()
        .ok_or_else(|| format!("没有找到「{}」的坐标，请换一个更具体的地址", address))?;
    place_from_json(first)
        .ok_or_else(|| format!("「{}」的坐标格式无法解析", address))
}

/// 省/自治区下的地级行政区：用户只知道「新疆」时，先把保底城市摆出来。
///
/// 高德对直辖市返回的是「北京城区」「上海城区」这类虚拟节点，它们不是可检索的城市，
/// 所以直辖市的省级查询直接返回省本身，让上层按城市检索。
pub async fn districts(key: &str, keyword: &str) -> Result<Vec<Place>, String> {
    let url = build_url(
        "/v3/config/district",
        &[
            ("key", key.to_string()),
            ("keywords", keyword.to_string()),
            ("subdistrict", "1".to_string()),
            ("extensions", "base".to_string()),
        ],
    )?;
    let value = cached_json(&url, GEO_TTL.min(Duration::from_secs(24 * 3600))).await?;
    ensure_ok(&value)?;
    let mut out = Vec::new();
    for district in value
        .get("districts")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default()
    {
        let name = val_text(district.get("name"));
        let adcode = val_text(district.get("adcode"));
        let level = val_text(district.get("level"));
        let (Some((lng, lat)), false) = (parse_location(&val_text(district.get("center"))), name.is_empty())
        else {
            continue;
        };
        if level == "country" {
            continue;
        }
        if level == "province" && MUNICIPALITIES.iter().any(|item| name.starts_with(item)) {
            out.push(Place {
                id: format!("amap-{adcode}-{lng:.6},{lat:.6}"),
                name: name.clone(),
                address: name.clone(),
                city: name.clone(),
                adcode: adcode.clone(),
                city_code: String::new(),
                lng,
                lat,
                source: "amap".to_string(),
            });
            continue;
        }
        for child in district
            .get("districts")
            .and_then(|item| item.as_array())
            .cloned()
            .unwrap_or_default()
        {
            let child_name = val_text(child.get("name"));
            let child_level = val_text(child.get("level"));
            if child_name.is_empty() || !matches!(child_level.as_str(), "city" | "district") {
                continue;
            }
            let Some((child_lng, child_lat)) = parse_location(&val_text(child.get("center"))) else {
                continue;
            };
            let child_adcode = val_text(child.get("adcode"));
            out.push(Place {
                id: format!("amap-{child_adcode}-{child_lng:.6},{child_lat:.6}"),
                name: child_name.clone(),
                address: format!("{name}{child_name}"),
                city: child_name,
                adcode: child_adcode,
                city_code: String::new(),
                lng: child_lng,
                lat: child_lat,
                source: "amap".to_string(),
            });
        }
    }
    Ok(out)
}

pub async fn regeo(key: &str, lng: f64, lat: f64) -> Result<Place, String> {
    let url = build_url(
        "/v3/geocode/regeo",
        &[
            ("key", key.to_string()),
            ("location", format!("{lng:.6},{lat:.6}")),
            ("extensions", "base".to_string()),
        ],
    )?;
    let value = cached_json(&url, GEO_TTL).await?;
    ensure_ok(&value)?;
    let regeocode = value
        .get("regeocode")
        .ok_or_else(|| "高德没有返回逆地理编码结果".to_string())?;
    let component = regeocode.get("addressComponent");
    let mut place = Place {
        id: format!("amap-regeo-{lng:.6},{lat:.6}"),
        name: val_text(regeocode.get("formatted_address")),
        address: val_text(regeocode.get("formatted_address")),
        city: component
            .map(|item| {
                let city = val_text(item.get("city"));
                if city.is_empty() {
                    val_text(item.get("province"))
                } else {
                    city
                }
            })
            .unwrap_or_default(),
        adcode: component
            .map(|item| val_text(item.get("adcode")))
            .unwrap_or_default(),
        city_code: component
            .map(|item| val_text(item.get("citycode")))
            .unwrap_or_default(),
        lng,
        lat,
        source: "amap".to_string(),
    };
    if place.name.trim().is_empty() {
        place.name = format!("{:.4}, {:.4} 附近", lng, lat);
    }
    Ok(place)
}

pub async fn inputtips(key: &str, keyword: &str, city: &str) -> Result<Vec<Place>, String> {
    let url = build_url(
        "/v3/assistant/inputtips",
        &[
            ("key", key.to_string()),
            ("keywords", keyword.to_string()),
            ("city", city.to_string()),
            ("datatype", "all".to_string()),
        ],
    )?;
    let value = cached_json(&url, GEO_TTL).await?;
    ensure_ok(&value)?;
    let tips = value
        .get("tips")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for tip in tips {
        let name = val_text(tip.get("name"));
        if name.trim().is_empty() {
            continue;
        }
        let Some((lng, lat)) = parse_location(&val_text(tip.get("location"))) else {
            continue;
        };
        let district = val_text(tip.get("district"));
        let adcode = val_text(tip.get("adcode"));
        // inputtips 的 `city` 字段常年返回空数组，真正的省市县信息都在 `district`
        // 里（连写）。直接把它塞进 `city` 会让后续按城市检索全部落空。
        let place_city = city_from_district(&district, city);
        out.push(Place {
            id: val_text(tip.get("id")),
            name,
            address: {
                let address = val_text(tip.get("address"));
                if address.is_empty() {
                    district.clone()
                } else {
                    address
                }
            },
            city: place_city,
            adcode,
            city_code: String::new(),
            lng,
            lat,
            source: "amap".to_string(),
        });
    }
    Ok(out)
}

pub async fn place_around(
    key: &str,
    lng: f64,
    lat: f64,
    radius_m: f64,
    types: &str,
    keyword: &str,
) -> Result<Vec<Poi>, String> {
    place_around_page(key, lng, lat, radius_m, types, keyword, 1).await
}

/// 周末半径大、候选多是常态，翻两页才能让外圈也有足够选择；每页仍走 7 天缓存。
pub async fn place_around_pages(
    key: &str,
    lng: f64,
    lat: f64,
    radius_m: f64,
    types: &str,
    keyword: &str,
    pages: u32,
) -> Result<Vec<Poi>, String> {
    let mut out = Vec::new();
    let mut last_error = String::new();
    for page in 1..=pages.clamp(1, 3) {
        match place_around_page(key, lng, lat, radius_m, types, keyword, page).await {
            Ok(mut items) => out.append(&mut items),
            Err(error) => {
                last_error = error;
                break;
            }
        }
    }
    if out.is_empty() && !last_error.is_empty() {
        return Err(last_error);
    }
    Ok(out)
}

async fn place_around_page(
    key: &str,
    lng: f64,
    lat: f64,
    radius_m: f64,
    types: &str,
    keyword: &str,
    page: u32,
) -> Result<Vec<Poi>, String> {
    let url = build_url(
        "/v3/place/around",
        &[
            ("key", key.to_string()),
            ("location", format!("{lng:.6},{lat:.6}")),
            ("radius", format!("{:.0}", radius_m)),
            ("types", types.to_string()),
            ("keywords", keyword.to_string()),
            ("offset", "25".to_string()),
            ("page", page.to_string()),
            ("extensions", "all".to_string()),
            ("sortrule", "weight".to_string()),
        ],
    )?;
    let value = cached_json(&url, POI_TTL).await?;
    ensure_ok(&value)?;
    let pois = value
        .get("pois")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(pois
        .iter()
        .filter_map(|item| poi_from_json(item, Some((lng, lat))))
        .collect())
}

pub async fn place_text(key: &str, keyword: &str, city: &str) -> Result<Vec<Poi>, String> {
    place_text_typed(key, keyword, city, "", false).await
}

/// 候选池按类别检索：`types` 收窄大类，`keywords` 用 `|` 合并同类词；
/// `city_limit` 保证城市级扫描不会把邻市的同名地点混进来。
pub async fn place_text_typed(
    key: &str,
    keyword: &str,
    city: &str,
    types: &str,
    city_limit: bool,
) -> Result<Vec<Poi>, String> {
    let mut params: Vec<(&str, String)> = vec![
        ("key", key.to_string()),
        ("keywords", keyword.to_string()),
        ("city", city.to_string()),
        (
            "citylimit",
            if city_limit { "true" } else { "false" }.to_string(),
        ),
        ("offset", "25".to_string()),
        ("page", "1".to_string()),
        ("extensions", "all".to_string()),
    ];
    if !types.trim().is_empty() {
        params.push(("types", types.to_string()));
    }
    let url = build_url("/v3/place/text", &params)?;
    let value = cached_json(&url, POI_TTL).await?;
    ensure_ok(&value)?;
    let pois = value
        .get("pois")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(pois
        .iter()
        .filter_map(|item| poi_from_json(item, None))
        .collect())
}

pub async fn weather(key: &str, adcode: &str) -> Result<Vec<WeatherDay>, String> {
    let url = build_url(
        "/v3/weather/weatherInfo",
        &[
            ("key", key.to_string()),
            ("city", adcode.to_string()),
            ("extensions", "all".to_string()),
        ],
    )?;
    let value = cached_json(&url, WEATHER_TTL).await?;
    ensure_ok(&value)?;
    let casts = value
        .get("forecasts")
        .and_then(|item| item.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.get("casts"))
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(casts
        .iter()
        .map(|cast| {
            let day_weather = val_text(cast.get("dayweather"));
            let night_weather = val_text(cast.get("nightweather"));
            WeatherDay {
                date: val_text(cast.get("date")),
                weekday: val_text(cast.get("week")),
                day_weather: day_weather.clone(),
                night_weather,
                day_temp: val_f64(cast.get("daytemp")),
                night_temp: val_f64(cast.get("nighttemp")),
                wind: val_text(cast.get("daywind")),
                wind_level: val_text(cast.get("daypower")),
                precipitation: None,
                icon: weather_icon(&day_weather).to_string(),
                source: "amap".to_string(),
            }
        })
        .collect())
}

fn weather_icon(text: &str) -> &'static str {
    if text.contains("雷") {
        "storm"
    } else if text.contains("雪") {
        "snow"
    } else if text.contains("雨") {
        "rain"
    } else if text.contains("雾") || text.contains("霾") {
        "fog"
    } else if text.contains("云") || text.contains("阴") {
        "cloud"
    } else {
        "sun"
    }
}

fn summarize_traffic(steps: &[Value]) -> String {
    let mut smooth = 0;
    let mut slow = 0;
    let mut jam = 0;
    let mut severe = 0;
    for step in steps {
        let Some(tmcs) = step.get("tmcs").and_then(|item| item.as_array()) else {
            continue;
        };
        for tmc in tmcs {
            let status = val_text(tmc.get("status"));
            if status.contains("严重") {
                severe += 1;
            } else if status.contains("拥堵") {
                jam += 1;
            } else if status.contains("缓行") {
                slow += 1;
            } else if status.contains("畅通") {
                smooth += 1;
            }
        }
    }
    if severe > 0 {
        format!("严重拥堵 {severe} 段")
    } else if jam > 0 {
        format!("拥堵 {jam} 段")
    } else if slow > 0 {
        format!("缓行 {slow} 段")
    } else if smooth > 0 {
        "全程畅通".to_string()
    } else {
        String::new()
    }
}

fn steps_from_path(path: &Value) -> Vec<RouteStep> {
    path.get("steps")
        .and_then(|item| item.as_array())
        .map(|items| {
            items
                .iter()
                .map(|step| RouteStep {
                    instruction: val_text(step.get("instruction")),
                    road: val_text(step.get("road")),
                    distance: val_f64(step.get("distance")).unwrap_or(0.0),
                    duration: val_f64(step.get("duration")).unwrap_or(0.0),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn polyline_from_steps(path: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(steps) = path.get("steps").and_then(|item| item.as_array()) {
        for step in steps {
            let polyline = val_text(step.get("polyline"));
            if !polyline.trim().is_empty() {
                parts.push(polyline.trim_matches(';').to_string());
            }
        }
    }
    coalesce_polyline(&parts)
}

fn path_summary(path: &Value, mode: &str) -> String {
    let distance = val_f64(path.get("distance")).unwrap_or(0.0) / 1000.0;
    let duration = val_f64(path.get("duration")).unwrap_or(0.0) / 60.0;
    let tolls = val_f64(path.get("tolls")).unwrap_or(0.0);
    let label = match mode {
        "walking" => "步行",
        "transit" => "公共交通",
        _ => "驾车",
    };
    if mode == "driving" && tolls > 0.0 {
        format!("{label} · {distance:.1} km · 约 {duration:.0} 分钟 · 过路费 {tolls:.0} 元")
    } else {
        format!("{label} · {distance:.1} km · 约 {duration:.0} 分钟")
    }
}

fn option_from_path(path: &Value, mode: &str, label: &str) -> RouteOption {
    let steps_json = path
        .get("steps")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    RouteOption {
        label: label.to_string(),
        summary: path_summary(path, mode),
        distance: val_f64(path.get("distance")).unwrap_or(0.0) / 1000.0,
        duration: val_f64(path.get("duration")).unwrap_or(0.0),
        tolls: val_f64(path.get("tolls")).unwrap_or(0.0),
        fare: 0.0,
        traffic: if mode == "driving" {
            summarize_traffic(&steps_json)
        } else {
            String::new()
        },
        polyline: polyline_from_steps(path),
    }
}

async fn driving_one(
    key: &str,
    origin: &str,
    destination: &str,
    strategy: &str,
    label: &str,
) -> Result<(RouteOption, Vec<RouteStep>), String> {
    let url = build_url(
        "/v3/direction/driving",
        &[
            ("key", key.to_string()),
            ("origin", origin.to_string()),
            ("destination", destination.to_string()),
            ("strategy", strategy.to_string()),
            ("extensions", "all".to_string()),
        ],
    )?;
    let value = cached_json(&url, super::ROUTE_TTL).await?;
    ensure_ok(&value)?;
    let path = value
        .pointer("/route/paths/0")
        .ok_or_else(|| "高德没有返回驾车路线".to_string())?;
    Ok((
        option_from_path(path, "driving", label),
        steps_from_path(path),
    ))
}

fn transit_option(
    transit: &Value,
    label: &str,
    from: &RouteStop,
    to: &RouteStop,
) -> (RouteOption, Vec<RouteStep>) {
    let mut steps = Vec::new();
    let mut polylines = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    if let Some(segments) = transit.get("segments").and_then(|item| item.as_array()) {
        for segment in segments {
            if let Some(walking) = segment.get("walking") {
                if let Some(items) = walking.get("steps").and_then(|item| item.as_array()) {
                    for step in items {
                        steps.push(RouteStep {
                            instruction: val_text(step.get("instruction")),
                            road: val_text(step.get("road")),
                            distance: val_f64(step.get("distance")).unwrap_or(0.0),
                            duration: val_f64(step.get("duration")).unwrap_or(0.0),
                        });
                        let polyline = val_text(step.get("polyline"));
                        if !polyline.trim().is_empty() {
                            polylines.push(polyline.trim_matches(';').to_string());
                        }
                    }
                }
            }
            if let Some(bus) = segment.get("bus") {
                if let Some(buslines) = bus.get("buslines").and_then(|item| item.as_array()) {
                    if let Some(line) = buslines.first() {
                        let name = val_text(line.get("name"));
                        let departure = line
                            .get("departure_stop")
                            .map(|item| val_text(item.get("name")))
                            .unwrap_or_default();
                        let arrival = line
                            .get("arrival_stop")
                            .map(|item| val_text(item.get("name")))
                            .unwrap_or_default();
                        let via = val_f64(line.get("via_num")).unwrap_or(0.0);
                        let duration = val_f64(line.get("duration")).unwrap_or(0.0);
                        if !name.trim().is_empty() {
                            lines.push(name.clone());
                        }
                        steps.push(RouteStep {
                            instruction: format!(
                                "乘坐 {name}，从 {departure} 上车，{arrival} 下车（{via:.0} 站，约 {:.0} 分钟）",
                                duration / 60.0
                            ),
                            road: name,
                            distance: val_f64(line.get("distance")).unwrap_or(0.0),
                            duration,
                        });
                        let polyline = val_text(line.get("polyline"));
                        if !polyline.trim().is_empty() {
                            polylines.push(polyline.trim_matches(';').to_string());
                        }
                    }
                }
            }
            if let Some(railway) = segment.get("railway") {
                let name = val_text(railway.get("name"));
                if !name.trim().is_empty() {
                    lines.push(name.clone());
                    steps.push(RouteStep {
                        instruction: format!(
                            "乘坐 {}，从 {} 到 {}",
                            name,
                            railway
                                .get("departure_stop")
                                .map(|item| val_text(item.get("name")))
                                .unwrap_or_default(),
                            railway
                                .get("arrival_stop")
                                .map(|item| val_text(item.get("name")))
                                .unwrap_or_default()
                        ),
                        road: name,
                        distance: val_f64(railway.get("distance")).unwrap_or(0.0),
                        duration: val_f64(railway.get("time")).unwrap_or(0.0),
                    });
                }
                let polyline = val_text(railway.get("polyline"));
                if !polyline.trim().is_empty() {
                    polylines.push(polyline.trim_matches(';').to_string());
                }
            }
            if let Some(taxi) = segment.get("taxi") {
                let distance = val_f64(taxi.get("distance")).unwrap_or(0.0);
                steps.push(RouteStep {
                    instruction: format!(
                        "打车约 {:.1} km，约 {:.0} 分钟",
                        distance / 1000.0,
                        val_f64(taxi.get("drivetime")).unwrap_or(0.0) / 60.0
                    ),
                    road: "出租车".to_string(),
                    distance,
                    duration: val_f64(taxi.get("drivetime")).unwrap_or(0.0),
                });
            }
        }
    }
    let distance = val_f64(transit.get("distance"))
        .map(|value| value / 1000.0)
        .unwrap_or_else(|| haversine_km(from.lng, from.lat, to.lng, to.lat) * 1.2);
    let duration = val_f64(transit.get("duration")).unwrap_or(0.0);
    let fare = val_f64(transit.get("cost")).unwrap_or(0.0);
    let line_text = if lines.is_empty() {
        "公共交通".to_string()
    } else {
        format!("公共交通 · {}", lines.join(" → "))
    };
    let summary = format!(
        "{line_text} · {distance:.1} km · 约 {:.0} 分钟{}",
        duration / 60.0,
        if fare > 0.0 {
            format!(" · 票价 {fare:.0} 元")
        } else {
            String::new()
        }
    );
    (
        RouteOption {
            label: label.to_string(),
            summary,
            distance,
            duration,
            tolls: 0.0,
            fare,
            traffic: String::new(),
            polyline: coalesce_polyline(&polylines),
        },
        steps,
    )
}

pub async fn route(
    key: &str,
    mode: &str,
    from: &RouteStop,
    to: &RouteStop,
    with_alternatives: bool,
) -> Result<RouteLeg, String> {
    let origin = format!("{:.6},{:.6}", from.lng, from.lat);
    let destination = format!("{:.6},{:.6}", to.lng, to.lat);
    match mode {
        "walking" => {
            let url = build_url(
                "/v3/direction/walking",
                &[
                    ("key", key.to_string()),
                    ("origin", origin),
                    ("destination", destination),
                ],
            )?;
            let value = cached_json(&url, super::ROUTE_TTL).await?;
            ensure_ok(&value)?;
            let path = value
                .pointer("/route/paths/0")
                .ok_or_else(|| "高德没有返回步行路线".to_string())?;
            let option = option_from_path(path, "walking", "步行");
            Ok(RouteLeg {
                mode: "walking".to_string(),
                distance: option.distance,
                duration: option.duration,
                tolls: 0.0,
                fare: 0.0,
                traffic: String::new(),
                summary: option.summary.clone(),
                steps: steps_from_path(path),
                polyline: option.polyline.clone(),
                alternatives: vec![option],
                ..RouteLeg::default()
            })
        }
        "transit" => {
            let city = if from.city_code.trim().is_empty() {
                from.city.clone()
            } else {
                from.city_code.clone()
            };
            let cityd = if to.city_code.trim().is_empty() {
                to.city.clone()
            } else {
                to.city_code.clone()
            };
            let url = build_url(
                "/v3/direction/transit/integrated",
                &[
                    ("key", key.to_string()),
                    ("origin", origin),
                    ("destination", destination),
                    ("city", city),
                    ("cityd", cityd),
                    ("strategy", "0".to_string()),
                    ("nightflag", "0".to_string()),
                ],
            )?;
            let value = cached_json(&url, super::ROUTE_TTL).await?;
            ensure_ok(&value)?;
            let transits = value
                .pointer("/route/transits")
                .and_then(|item| item.as_array())
                .cloned()
                .unwrap_or_default();
            if transits.is_empty() {
                return Err("高德没有返回公交换乘方案".to_string());
            }
            let mut options = Vec::new();
            let mut primary_steps = Vec::new();
            let transit_limit = if with_alternatives { 3 } else { 1 };
            for (index, transit) in transits.iter().take(transit_limit).enumerate() {
                let label = format!("方案 {}", index + 1);
                let (option, steps) = transit_option(transit, &label, from, to);
                if options.is_empty() {
                    primary_steps = steps;
                }
                options.push(option);
            }
            let primary = options[0].clone();
            Ok(RouteLeg {
                mode: "transit".to_string(),
                distance: primary.distance,
                duration: primary.duration,
                tolls: 0.0,
                fare: primary.fare,
                traffic: String::new(),
                summary: primary.summary.clone(),
                steps: primary_steps,
                polyline: primary.polyline.clone(),
                alternatives: options,
                ..RouteLeg::default()
            })
        }
        _ => {
            let mut options: Vec<RouteOption> = Vec::new();
            let mut primary_steps = Vec::new();
            let mut last_error = String::new();
            let strategies: &[(&str, &str)] = if with_alternatives {
                &[("0", "推荐"), ("4", "躲避拥堵"), ("2", "距离优先")]
            } else {
                &[("0", "推荐")]
            };
            for (strategy, label) in strategies.iter().copied() {
                match driving_one(key, &origin, &destination, strategy, label).await {
                    Ok((option, steps)) => {
                        let duplicate = options.iter().any(|item| {
                            (!item.polyline.is_empty() && item.polyline == option.polyline)
                                || ((item.distance - option.distance).abs() < 0.05
                                    && (item.duration - option.duration).abs() < 30.0)
                        });
                        if duplicate {
                            continue;
                        }
                        if options.is_empty() {
                            primary_steps = steps;
                        }
                        options.push(option);
                    }
                    Err(error) => {
                        if options.is_empty() {
                            last_error = error;
                        }
                    }
                }
            }
            if options.is_empty() {
                return Err(if last_error.is_empty() {
                    "高德没有返回驾车路线".to_string()
                } else {
                    last_error
                });
            }
            let primary = options[0].clone();
            Ok(RouteLeg {
                mode: "driving".to_string(),
                distance: primary.distance,
                duration: primary.duration,
                tolls: primary.tolls,
                fare: 0.0,
                traffic: primary.traffic.clone(),
                summary: primary.summary.clone(),
                steps: primary_steps,
                polyline: primary.polyline.clone(),
                alternatives: options,
                ..RouteLeg::default()
            })
        }
    }
}

pub async fn ip_location(key: &str) -> Result<Place, String> {
    let url = build_url("/v3/ip", &[("key", key.to_string())])?;
    let value = cached_json(&url, Duration::from_secs(3600)).await?;
    ensure_ok(&value)?;
    let province = val_text(value.get("province"));
    let mut city = val_text(value.get("city"));
    if city.is_empty() {
        city = province.clone();
    }
    if city.trim().is_empty() {
        return Err("高德 IP 定位没有返回城市".to_string());
    }
    let rectangle = val_text(value.get("rectangle"));
    let center = rectangle
        .split(';')
        .filter_map(|pair| parse_location(pair))
        .fold(None::<(f64, f64)>, |acc, item| match acc {
            None => Some(item),
            Some((lng, lat)) => Some(((lng + item.0) / 2.0, (lat + item.1) / 2.0)),
        });
    if let Some((lng, lat)) = center {
        return Ok(Place {
            id: format!("amap-ip-{}", val_text(value.get("adcode"))),
            name: city.clone(),
            address: format!("{province}{city}"),
            city,
            adcode: val_text(value.get("adcode")),
            city_code: String::new(),
            lng,
            lat,
            source: "amap".to_string(),
        });
    }
    let mut place = geocode(key, &city, "").await?;
    place.name = city;
    Ok(place)
}

pub async fn static_map(
    key: &str,
    center_lng: f64,
    center_lat: f64,
    zoom: u8,
    width: u32,
    height: u32,
    path: &[Vec<f64>],
) -> Result<String, String> {
    let width = width.clamp(320, 1024);
    let height = height.clamp(200, 1024);
    let mut url = format!(
        "{AMAP_BASE}/v3/staticmap?key={key}&location={center_lng:.6},{center_lat:.6}&zoom={zoom}&size={width}*{height}&scale=2"
    );
    if path.len() >= 2 {
        let step = (path.len() / 60).max(1);
        let points = path
            .iter()
            .enumerate()
            .filter(|(index, _)| index % step == 0)
            .filter_map(|(_, item)| {
                if item.len() >= 2 {
                    Some(format!("{:.6},{:.6}", item[0], item[1]))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(";");
        if !points.is_empty() {
            // 高德的 paths 参数依赖裸逗号/分号，手动拼 URL，避免被 query 编码器转义。
            // 白色描边 + 绿色主线：公园、道路本身也是彩色，纯细线会混在一起。
            url.push_str(&format!(
                "&paths=9,0xFFFFFF,1,,:{points}|6,0x0F7A3D,1,,:{points}"
            ));
        }
    }
    let response = super::http_client()?
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("请求静态地图失败: {e}"))?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|item| item.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    if !status.is_success() || !content_type.starts_with("image/") {
        let text = String::from_utf8_lossy(&bytes);
        return Err(format!(
            "高德静态地图返回异常: {}",
            truncate(text.trim(), 160)
        ));
    }
    let encoded = general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{content_type};base64,{encoded}"))
}
