//! Google Maps Web 服务适配（WGS84 坐标）。
//!
//! 显示时使用 OpenStreetMap 瓦片即可对齐，避免把 Google 地图脚本引入 WebView。

use serde_json::Value;

use super::{as_f64, as_text, build_url, decode_polyline, strip_html};
use crate::commands::travel::{
    cached_json, haversine_km, truncate, Place, Poi, RouteLeg, RouteOption, RouteStep, RouteStop,
    GEO_TTL, POI_TTL, ROUTE_TTL,
};

const BASE: &str = "https://maps.googleapis.com";

fn ensure_ok(value: &Value) -> Result<(), String> {
    let status = as_text(value.get("status"));
    if status == "OK" || status == "ZERO_RESULTS" {
        Ok(())
    } else {
        Err(format!(
            "Google Maps 返回 {status}: {}",
            truncate(&as_text(value.get("error_message")), 160)
        ))
    }
}

pub async fn geocode(key: &str, address: &str, city: &str) -> Result<Place, String> {
    let query = if city.trim().is_empty() {
        address.to_string()
    } else {
        format!("{address}, {city}")
    };
    let url = build_url(
        BASE,
        "/maps/api/geocode/json",
        &[
            ("address", query),
            ("language", "zh-CN".to_string()),
            ("key", key.to_string()),
        ],
    )?;
    let value = cached_json(&url, GEO_TTL).await?;
    ensure_ok(&value)?;
    let result = value
        .pointer("/results/0")
        .ok_or_else(|| format!("Google Maps 没有找到「{address}」"))?;
    let location = result
        .pointer("/geometry/location")
        .ok_or_else(|| "Google Maps 没有返回坐标".to_string())?;
    let lng = as_f64(location.get("lng")).ok_or_else(|| "Google 坐标缺少经度".to_string())?;
    let lat = as_f64(location.get("lat")).ok_or_else(|| "Google 坐标缺少纬度".to_string())?;
    let components = result
        .get("address_components")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    let find_component = |kind: &str| {
        components
            .iter()
            .find(|item| {
                item.get("types")
                    .and_then(|types| types.as_array())
                    .map(|types| types.iter().any(|t| t.as_str() == Some(kind)))
                    .unwrap_or(false)
            })
            .map(|item| as_text(item.get("long_name")))
            .unwrap_or_default()
    };
    let city_name = {
        let locality = find_component("locality");
        if locality.is_empty() {
            find_component("administrative_area_level_1")
        } else {
            locality
        }
    };
    Ok(Place {
        id: format!("google-{lng:.6},{lat:.6}"),
        name: {
            let formatted = as_text(result.get("formatted_address"));
            if formatted.is_empty() {
                address.to_string()
            } else {
                formatted
            }
        },
        address: as_text(result.get("formatted_address")),
        city: if city_name.is_empty() {
            city.to_string()
        } else {
            city_name
        },
        adcode: String::new(),
        city_code: String::new(),
        lng,
        lat,
        source: "google".to_string(),
    })
}

pub async fn place_nearby(
    key: &str,
    lat: f64,
    lng: f64,
    radius_m: f64,
    kind: &str,
    place_type: &str,
) -> Result<Vec<Poi>, String> {
    let url = build_url(
        BASE,
        "/maps/api/place/nearbysearch/json",
        &[
            ("location", format!("{lat:.6},{lng:.6}")),
            ("radius", format!("{:.0}", radius_m.clamp(1000.0, 50000.0))),
            ("type", place_type.to_string()),
            ("language", "zh-CN".to_string()),
            ("key", key.to_string()),
        ],
    )?;
    let value = cached_json(&url, POI_TTL).await?;
    ensure_ok(&value)?;
    let results = value
        .get("results")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for item in results {
        let location = item.get("geometry").and_then(|geo| geo.get("location"));
        let (Some(poi_lng), Some(poi_lat)) = (
            location.and_then(|item| as_f64(item.get("lng"))),
            location.and_then(|item| as_f64(item.get("lat"))),
        ) else {
            continue;
        };
        let name = as_text(item.get("name"));
        if name.is_empty() {
            continue;
        }
        out.push(Poi {
            id: format!("google-{name}-{poi_lng:.6},{poi_lat:.6}"),
            name,
            source: "google".to_string(),
            kind: kind.to_string(),
            category: {
                let first = item
                    .get("types")
                    .and_then(|types| types.as_array())
                    .and_then(|types| types.first())
                    .map(|value| as_text(Some(value)))
                    .unwrap_or_default();
                first
            },
            address: as_text(item.get("vicinity")),
            city: String::new(),
            lng: poi_lng,
            lat: poi_lat,
            rating: as_f64(item.get("rating")),
            cost: None,
            tel: String::new(),
            distance: Some(haversine_km(lng, lat, poi_lng, poi_lat) * 1000.0),
            tags: item
                .get("types")
                .and_then(|types| types.as_array())
                .map(|types| {
                    types
                        .iter()
                        .filter_map(|value| value.as_str().map(|s| s.to_string()))
                        .take(6)
                        .collect()
                })
                .unwrap_or_default(),
        });
    }
    Ok(out)
}

fn route_polyline(route: &Value) -> String {
    let encoded = route
        .pointer("/overview_polyline/points")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    decode_polyline(encoded)
        .into_iter()
        .map(|(lng, lat)| format!("{lng:.6},{lat:.6}"))
        .collect::<Vec<_>>()
        .join(";")
}

fn route_steps(route: &Value) -> Vec<RouteStep> {
    route
        .pointer("/legs/0/steps")
        .and_then(|item| item.as_array())
        .map(|steps| {
            steps
                .iter()
                .map(|step| RouteStep {
                    instruction: strip_html(&as_text(step.get("html_instructions"))),
                    road: as_text(step.get("short_name")),
                    distance: step
                        .pointer("/distance/value")
                        .and_then(|item| as_f64(Some(item)))
                        .unwrap_or(0.0),
                    duration: step
                        .pointer("/duration/value")
                        .and_then(|item| as_f64(Some(item)))
                        .unwrap_or(0.0),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub async fn route(
    key: &str,
    mode: &str,
    from: &RouteStop,
    to: &RouteStop,
) -> Result<RouteLeg, String> {
    let travel_mode = match mode {
        "walking" => "walking",
        "transit" => "transit",
        _ => "driving",
    };
    let url = build_url(
        BASE,
        "/maps/api/directions/json",
        &[
            ("origin", format!("{:.6},{:.6}", from.lat, from.lng)),
            ("destination", format!("{:.6},{:.6}", to.lat, to.lng)),
            ("mode", travel_mode.to_string()),
            ("alternatives", "true".to_string()),
            ("language", "zh-CN".to_string()),
            ("key", key.to_string()),
        ],
    )?;
    let value = cached_json(&url, ROUTE_TTL).await?;
    ensure_ok(&value)?;
    let routes = value
        .get("routes")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    if routes.is_empty() {
        return Err("Google Maps 没有返回路线".to_string());
    }
    let mode_label = match mode {
        "walking" => "步行",
        "transit" => "公共交通",
        _ => "驾车",
    };
    let mut options = Vec::new();
    let mut primary_steps = Vec::new();
    for (index, route) in routes.iter().take(3).enumerate() {
        let distance = route
            .pointer("/legs/0/distance/value")
            .and_then(|item| as_f64(Some(item)))
            .unwrap_or(0.0)
            / 1000.0;
        let duration = route
            .pointer("/legs/0/duration/value")
            .and_then(|item| as_f64(Some(item)))
            .unwrap_or(0.0);
        let label = if index == 0 {
            mode_label.to_string()
        } else {
            format!("{mode_label}备选 {index}")
        };
        if index == 0 {
            primary_steps = route_steps(route);
        }
        options.push(RouteOption {
            label: label.clone(),
            summary: format!("{label} · {distance:.1} km · 约 {:.0} 分钟", duration / 60.0),
            distance,
            duration,
            tolls: 0.0,
            fare: 0.0,
            traffic: String::new(),
            polyline: route_polyline(route),
        });
    }
    let primary = options[0].clone();
    Ok(RouteLeg {
        mode: mode.to_string(),
        distance: primary.distance,
        duration: primary.duration,
        tolls: 0.0,
        fare: 0.0,
        traffic: String::new(),
        summary: primary.summary.clone(),
        steps: primary_steps,
        polyline: primary.polyline.clone(),
        alternatives: options,
        ..RouteLeg::default()
    })
}
