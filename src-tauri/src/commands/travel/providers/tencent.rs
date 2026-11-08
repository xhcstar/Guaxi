//! 腾讯位置服务适配（GCJ-02 坐标）。

use serde_json::Value;

use super::{as_f64, as_text, build_url};
use crate::commands::travel::{
    cached_json, haversine_km, truncate, Place, Poi, RouteLeg, RouteOption, RouteStep, RouteStop,
    GEO_TTL, POI_TTL,
};

const BASE: &str = "https://apis.map.qq.com";

fn status_ok(value: &Value) -> bool {
    value.get("status").and_then(|item| item.as_i64()) == Some(0)
}

fn error_text(value: &Value) -> String {
    format!(
        "腾讯地图返回 status={}: {}",
        value.get("status").and_then(|item| item.as_i64()).unwrap_or(-1),
        truncate(&as_text(value.get("message")), 140)
    )
}

pub async fn geocode(key: &str, address: &str, city: &str) -> Result<Place, String> {
    let url = build_url(
        BASE,
        "/ws/geocoder/v1/",
        &[
            ("address", address.to_string()),
            ("region", city.to_string()),
            ("key", key.to_string()),
        ],
    )?;
    let value = cached_json(&url, GEO_TTL).await?;
    if !status_ok(&value) {
        return Err(error_text(&value));
    }
    let result = value
        .get("result")
        .ok_or_else(|| "腾讯地图没有返回坐标".to_string())?;
    let location = result
        .get("location")
        .ok_or_else(|| "腾讯地图没有返回坐标".to_string())?;
    let lng = as_f64(location.get("lng")).ok_or_else(|| "腾讯坐标缺少经度".to_string())?;
    let lat = as_f64(location.get("lat")).ok_or_else(|| "腾讯坐标缺少纬度".to_string())?;
    let ad_info = result.get("ad_info");
    let city_name = ad_info
        .map(|item| {
            let city = as_text(item.get("city"));
            if city.is_empty() {
                as_text(item.get("province"))
            } else {
                city
            }
        })
        .unwrap_or_else(|| city.to_string());
    Ok(Place {
        id: format!("tencent-{lng:.6},{lat:.6}"),
        name: {
            let title = as_text(result.get("title"));
            if title.is_empty() {
                address.to_string()
            } else {
                title
            }
        },
        address: address.to_string(),
        city: city_name,
        adcode: ad_info.map(|item| as_text(item.get("adcode"))).unwrap_or_default(),
        city_code: String::new(),
        lng,
        lat,
        source: "tencent".to_string(),
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
        "/ws/place/v1/search",
        &[
            ("keyword", keyword.to_string()),
            ("boundary", format!("region({city},0)")),
            ("page_size", "20".to_string()),
            ("key", key.to_string()),
        ],
    )?;
    let value = cached_json(&url, POI_TTL).await?;
    if !status_ok(&value) {
        return Err(error_text(&value));
    }
    let items = value
        .get("data")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for item in items {
        let location = item.get("location");
        let (Some(lng), Some(lat)) = (
            location.and_then(|item| as_f64(item.get("lng"))),
            location.and_then(|item| as_f64(item.get("lat"))),
        ) else {
            continue;
        };
        let name = as_text(item.get("title"));
        if name.is_empty() {
            continue;
        }
        let poi_city = item
            .get("ad_info")
            .and_then(|info| info.get("city"))
            .map(|value| as_text(Some(value)))
            .unwrap_or_default();
        out.push(Poi {
            id: format!("tencent-{name}-{lng:.6},{lat:.6}"),
            name,
            source: "tencent".to_string(),
            kind: kind.to_string(),
            category: as_text(item.get("category")),
            address: as_text(item.get("address")),
            city: if poi_city.is_empty() {
                city.to_string()
            } else {
                poi_city
            },
            lng,
            lat,
            rating: None,
            cost: None,
            tel: as_text(item.get("tel")),
            distance: Some(haversine_km(center_lng, center_lat, lng, lat) * 1000.0),
            tags: Vec::new(),
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

pub async fn route(
    key: &str,
    mode: &str,
    from: &RouteStop,
    to: &RouteStop,
) -> Result<RouteLeg, String> {
    let endpoint = match mode {
        "walking" => "walking",
        "transit" => "transit",
        _ => "driving",
    };
    let url = build_url(
        BASE,
        &format!("/ws/direction/v1/{endpoint}/"),
        &[
            ("from", format!("{:.6},{:.6}", from.lat, from.lng)),
            ("to", format!("{:.6},{:.6}", to.lat, to.lng)),
            ("key", key.to_string()),
        ],
    )?;
    let value = cached_json(&url, POI_TTL).await?;
    if !status_ok(&value) {
        return Err(error_text(&value));
    }
    let routes = value
        .pointer("/result/routes")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    if routes.is_empty() {
        return Err("腾讯地图没有返回路线".to_string());
    }
    let mode_label = match mode {
        "walking" => "步行",
        "transit" => "公共交通",
        _ => "驾车",
    };
    let mut options = Vec::new();
    let mut primary_steps = Vec::new();
    for (index, route) in routes.iter().take(3).enumerate() {
        let distance = as_f64(route.get("distance")).unwrap_or(0.0) / 1000.0;
        let duration = as_f64(route.get("duration")).unwrap_or(0.0);
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
            tolls: as_f64(route.get("toll")).unwrap_or(0.0),
            fare: 0.0,
            traffic: String::new(),
            polyline: String::new(),
        });
    }
    let primary = options[0].clone();
    Ok(RouteLeg {
        mode: mode.to_string(),
        distance: primary.distance,
        duration: primary.duration,
        tolls: primary.tolls,
        fare: 0.0,
        traffic: String::new(),
        summary: primary.summary.clone(),
        steps: primary_steps,
        polyline: String::new(),
        alternatives: options,
        ..RouteLeg::default()
    })
}
