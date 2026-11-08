//! Mapbox 适配（WGS84 坐标）。
//!
//! Mapbox 的 POI 检索能力依赖 Search API；这里用 Geocoding 的 poi 类型，
//! 数量不足时补一份 OpenStreetMap Overpass 结果，保证计划里仍有足够的候选地点。

use serde_json::Value;

use super::{as_f64, as_text, build_url};
use crate::commands::travel::{
    cached_json, haversine_km, truncate, Place, Poi, RouteLeg, RouteOption, RouteStep, RouteStop,
    GEO_TTL, POI_TTL, ROUTE_TTL,
};

const BASE: &str = "https://api.mapbox.com";

fn ensure_ok(value: &Value) -> Result<(), String> {
    let code = as_text(value.get("code"));
    if code == "Ok" {
        Ok(())
    } else {
        Err(format!(
            "Mapbox 返回 {code}: {}",
            truncate(&as_text(value.get("message")), 160)
        ))
    }
}

fn city_from_context(feature: &Value) -> String {
    let context = feature
        .get("context")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    for item in context {
        let id = as_text(item.get("id"));
        if id.starts_with("place.") || id.starts_with("district.") {
            let text = as_text(item.get("text"));
            if !text.is_empty() {
                return text;
            }
        }
    }
    as_text(feature.get("text"))
}

fn places_url(key: &str, query: &str, lng: f64, lat: f64, types: &str) -> Result<String, String> {
    let mut url = reqwest::Url::parse(&format!("{BASE}/geocoding/v5/mapbox.places/"))
        .map_err(|e| format!("构建 Mapbox 地址失败: {e}"))?;
    url.path_segments_mut()
        .map_err(|_| "构建 Mapbox 路径失败".to_string())?
        .push(&format!("{query}.json"));
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("access_token", key);
        pairs.append_pair("language", "zh");
        pairs.append_pair("limit", "8");
        pairs.append_pair("types", types);
        pairs.append_pair("proximity", &format!("{lng:.6},{lat:.6}"));
    }
    Ok(url.to_string())
}

pub async fn geocode(key: &str, address: &str, city: &str) -> Result<Place, String> {
    let query = if city.trim().is_empty() {
        address.to_string()
    } else {
        format!("{address}, {city}")
    };
    let url = places_url(key, &query, 0.0, 0.0, "place,address,poi")?;
    let value = cached_json(&url, GEO_TTL).await?;
    let feature = value
        .pointer("/features/0")
        .ok_or_else(|| format!("Mapbox 没有找到「{address}」"))?;
    let center = feature
        .get("center")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    let lng = center.first().and_then(|item| as_f64(Some(item)));
    let lat = center.get(1).and_then(|item| as_f64(Some(item)));
    let (Some(lng), Some(lat)) = (lng, lat) else {
        return Err("Mapbox 没有返回坐标".to_string());
    };
    let place_name = as_text(feature.get("place_name"));
    let context_city = city_from_context(feature);
    Ok(Place {
        id: format!("mapbox-{lng:.6},{lat:.6}"),
        name: {
            let text = as_text(feature.get("text"));
            if text.is_empty() {
                address.to_string()
            } else {
                text
            }
        },
        address: if place_name.is_empty() {
            address.to_string()
        } else {
            place_name
        },
        city: if context_city.is_empty() {
            city.to_string()
        } else {
            context_city
        },
        adcode: String::new(),
        city_code: String::new(),
        lng,
        lat,
        source: "mapbox".to_string(),
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
    let query = if city.trim().is_empty() {
        keyword.to_string()
    } else {
        format!("{keyword} {city}")
    };
    let url = places_url(key, &query, center_lng, center_lat, "poi")?;
    let value = cached_json(&url, POI_TTL).await?;
    let mut out = Vec::new();
    for feature in value
        .get("features")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default()
    {
        let center = feature
            .get("center")
            .and_then(|item| item.as_array())
            .cloned()
            .unwrap_or_default();
        let (Some(lng), Some(lat)) = (
            center.first().and_then(|item| as_f64(Some(item))),
            center.get(1).and_then(|item| as_f64(Some(item))),
        ) else {
            continue;
        };
        let name = as_text(feature.get("text"));
        if name.is_empty() {
            continue;
        }
        let props = feature.get("properties");
        out.push(Poi {
            id: as_text(feature.get("id")),
            name,
            source: "mapbox".to_string(),
            kind: kind.to_string(),
            category: props
                .map(|item| as_text(item.get("category")))
                .unwrap_or_default(),
            address: props
                .map(|item| as_text(item.get("address")))
                .unwrap_or_default(),
            city: city.to_string(),
            lng,
            lat,
            rating: None,
            cost: None,
            tel: String::new(),
            distance: Some(haversine_km(center_lng, center_lat, lng, lat) * 1000.0),
            tags: Vec::new(),
        });
    }
    Ok(out)
}

fn route_steps(route: &Value) -> Vec<RouteStep> {
    route
        .pointer("/legs/0/steps")
        .and_then(|item| item.as_array())
        .map(|steps| {
            steps
                .iter()
                .map(|step| RouteStep {
                    instruction: step
                        .pointer("/maneuver/instruction")
                        .map(|item| as_text(Some(item)))
                        .filter(|text| !text.is_empty())
                        .unwrap_or_else(|| as_text(step.get("name"))),
                    road: as_text(step.get("name")),
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
    let (profile, mode_label, warning) = match mode {
        "walking" => ("walking", "步行", String::new()),
        "transit" => (
            "driving",
            "公共交通",
            "Mapbox 不提供公交换乘，当前按驾车路线参考。".to_string(),
        ),
        _ => ("driving", "驾车", String::new()),
    };
    let coords = format!(
        "{:.6},{:.6};{:.6},{:.6}",
        from.lng, from.lat, to.lng, to.lat
    );
    let url = build_url(
        BASE,
        &format!("/directions/v5/mapbox/{profile}/{coords}"),
        &[
            ("alternatives", "true".to_string()),
            ("geometries", "geojson".to_string()),
            ("steps", "true".to_string()),
            ("language", "zh".to_string()),
            ("access_token", key.to_string()),
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
        return Err("Mapbox 没有返回路线".to_string());
    }
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
        let polyline = route
            .pointer("/geometry/coordinates")
            .and_then(|item| item.as_array())
            .map(|points| {
                points
                    .iter()
                    .filter_map(|point| {
                        let pair = point.as_array()?;
                        Some(format!("{:.6},{:.6}", pair.first()?.as_f64()?, pair.get(1)?.as_f64()?))
                    })
                    .collect::<Vec<_>>()
                    .join(";")
            })
            .unwrap_or_default();
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
            polyline,
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
        warning,
        ..RouteLeg::default()
    })
}
