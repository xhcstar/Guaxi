//! 百度地图 Web 服务适配（BD-09 坐标）。
//!
//! 百度公交接口需要城市编码；这里没有可靠的编码映射，因此公交模式会退化为驾车参考并显式提示。

use serde_json::Value;

use super::{as_f64, as_text, build_url};
use crate::commands::travel::{
    cached_json, coalesce_polyline, haversine_km, truncate, val_text, Place, Poi, RouteLeg,
    RouteOption, RouteStep, RouteStop, GEO_TTL, POI_TTL, ROUTE_TTL,
};

const BASE: &str = "https://api.map.baidu.com";

fn status_ok(value: &Value) -> bool {
    value.get("status").and_then(|item| item.as_i64()) == Some(0)
}

fn error_text(value: &Value) -> String {
    let message = as_text(value.get("message"));
    let status = value.get("status").and_then(|item| item.as_i64()).unwrap_or(-1);
    format!("百度地图返回 status={status}: {}", truncate(&message, 140))
}

pub async fn geocode(key: &str, address: &str, city: &str) -> Result<Place, String> {
    let url = build_url(
        BASE,
        "/geocoding/v3/",
        &[
            ("address", address.to_string()),
            ("city", city.to_string()),
            ("output", "json".to_string()),
            ("ak", key.to_string()),
        ],
    )?;
    let value = cached_json(&url, GEO_TTL).await?;
    if !status_ok(&value) {
        return Err(error_text(&value));
    }
    let location = value
        .pointer("/result/location")
        .ok_or_else(|| "百度没有返回坐标".to_string())?;
    let lng = as_f64(location.get("lng")).ok_or_else(|| "百度坐标缺少经度".to_string())?;
    let lat = as_f64(location.get("lat")).ok_or_else(|| "百度坐标缺少纬度".to_string())?;
    Ok(Place {
        id: format!("baidu-{lng:.6},{lat:.6}"),
        name: address.to_string(),
        address: address.to_string(),
        city: city.to_string(),
        adcode: String::new(),
        city_code: String::new(),
        lng,
        lat,
        source: "baidu".to_string(),
    })
}

pub async fn place_text(
    key: &str,
    keyword: &str,
    city: &str,
    kind: &str,
    center_lng: f64,
    center_lat: f64,
) -> Result<Vec<Poi>, String> {
    let url = build_url(
        BASE,
        "/place/v2/search",
        &[
            ("query", keyword.to_string()),
            ("region", city.to_string()),
            ("output", "json".to_string()),
            ("ak", key.to_string()),
            ("page_size", "20".to_string()),
            ("scope", "2".to_string()),
        ],
    )?;
    let value = cached_json(&url, POI_TTL).await?;
    if !status_ok(&value) {
        return Err(error_text(&value));
    }
    let results = value
        .get("results")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for item in results {
        let Some(location) = item.get("location") else {
            continue;
        };
        let (Some(lng), Some(lat)) = (
            as_f64(location.get("lng")),
            as_f64(location.get("lat")),
        ) else {
            continue;
        };
        let name = as_text(item.get("name"));
        if name.is_empty() {
            continue;
        }
        let detail = item.get("detail_info");
        let rating = detail.and_then(|item| as_f64(item.get("overall_rating")));
        let cost = detail.and_then(|item| as_f64(item.get("price")));
        let category = detail
            .map(|item| {
                let tag = as_text(item.get("tag"));
                if tag.is_empty() {
                    as_text(item.get("type"))
                } else {
                    tag
                }
            })
            .unwrap_or_else(|| keyword.to_string());
        out.push(Poi {
            id: format!("baidu-{name}-{lng:.6},{lat:.6}"),
            name,
            source: "baidu".to_string(),
            kind: kind.to_string(),
            category,
            address: as_text(item.get("address")),
            city: {
                let city = as_text(item.get("city"));
                if city.is_empty() {
                    as_text(item.get("area"))
                } else {
                    city
                }
            },
            lng,
            lat,
            rating,
            cost,
            tel: detail.map(|item| as_text(item.get("phone"))).unwrap_or_default(),
            distance: Some(haversine_km(center_lng, center_lat, lng, lat) * 1000.0),
            tags: detail
                .map(|item| {
                    let tag = as_text(item.get("tag"));
                    if tag.is_empty() {
                        Vec::new()
                    } else {
                        tag.split(';').map(|part| part.trim().to_string()).collect()
                    }
                })
                .unwrap_or_default(),
        });
    }
    Ok(out)
}

fn route_steps(route: &Value) -> Vec<RouteStep> {
    route
        .get("steps")
        .and_then(|item| item.as_array())
        .map(|items| {
            items
                .iter()
                .map(|step| RouteStep {
                    instruction: as_text(step.get("instruction")),
                    road: as_text(step.get("road_name")),
                    distance: as_f64(step.get("distance")).unwrap_or(0.0),
                    duration: as_f64(step.get("duration")).unwrap_or(0.0),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn route_polyline(route: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(steps) = route.get("steps").and_then(|item| item.as_array()) {
        for step in steps {
            let path = as_text(step.get("path"));
            if !path.trim().is_empty() {
                parts.push(path.trim_matches(';').to_string());
            }
        }
    }
    coalesce_polyline(&parts)
}

pub async fn route(
    key: &str,
    mode: &str,
    from: &RouteStop,
    to: &RouteStop,
) -> Result<RouteLeg, String> {
    let (endpoint, mode_label, warning) = match mode {
        "walking" => ("walking", "步行", String::new()),
        "transit" => (
            "driving",
            "公共交通",
            "百度公交接口需要城市编码，当前按驾车路线参考。".to_string(),
        ),
        _ => ("driving", "驾车", String::new()),
    };
    let url = build_url(
        BASE,
        &format!("/directionlite/v1/{endpoint}"),
        &[
            ("origin", format!("{:.6},{:.6}", from.lat, from.lng)),
            ("destination", format!("{:.6},{:.6}", to.lat, to.lng)),
            ("ak", key.to_string()),
        ],
    )?;
    let value = cached_json(&url, ROUTE_TTL).await?;
    if !status_ok(&value) {
        return Err(error_text(&value));
    }
    let routes = value
        .pointer("/result/routes")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    if routes.is_empty() {
        return Err("百度没有返回路线".to_string());
    }
    let mut options = Vec::new();
    let mut primary_steps = Vec::new();
    for (index, route) in routes.iter().take(3).enumerate() {
        let distance = as_f64(route.get("distance")).unwrap_or(0.0) / 1000.0;
        let duration = as_f64(route.get("duration")).unwrap_or(0.0);
        let tolls = as_f64(route.get("toll")).unwrap_or(0.0);
        let label = if index == 0 {
            mode_label.to_string()
        } else {
            format!("{mode_label}备选 {index}")
        };
        let summary = format!("{label} · {distance:.1} km · 约 {:.0} 分钟", duration / 60.0);
        if index == 0 {
            primary_steps = route_steps(route);
        }
        options.push(RouteOption {
            label,
            summary,
            distance,
            duration,
            tolls,
            fare: 0.0,
            traffic: String::new(),
            polyline: route_polyline(route),
        });
    }
    let primary = options[0].clone();
    Ok(RouteLeg {
        mode: if mode == "transit" {
            "transit".to_string()
        } else {
            mode.to_string()
        },
        distance: primary.distance,
        duration: primary.duration,
        tolls: primary.tolls,
        fare: 0.0,
        traffic: String::new(),
        summary: primary.summary.clone(),
        steps: primary_steps,
        polyline: primary.polyline.clone(),
        alternatives: options,
        warning,
        ..RouteLeg::default()
    })
}

#[allow(dead_code)]
fn text(value: &Value) -> String {
    val_text(Some(value))
}
