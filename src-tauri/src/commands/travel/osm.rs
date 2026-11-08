//! OpenStreetMap 系兜底：Nominatim 地理编码、Overpass POI、OSRM 路线、ipwho.is IP 定位。
//!
//! 这条链路不需要任何 Key，但在国内的数据覆盖和公交能力弱于高德，因此界面上会明确标注
//! 数据源；POI 名称缺失或营业信息不全时，交给模型补文案、由用户出发前核实。

use serde_json::Value;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::{
    cached_json, cached_post_json, coalesce_polyline, haversine_km, truncate, val_f64, val_text,
    Place, Poi, RouteLeg, RouteStep, RouteStop, GEO_TTL, POI_TTL,
};

const NOMINATIM: &str = "https://nominatim.openstreetmap.org";
const OVERPASS: &str = "https://overpass-api.de/api/interpreter";
const OSRM: &str = "https://router.project-osrm.org";

fn build_url(base: &str, path: &str, params: &[(&str, String)]) -> Result<String, String> {
    let mut url = reqwest::Url::parse(&format!("{base}{path}"))
        .map_err(|e| format!("构建 OSM 请求地址失败: {e}"))?;
    {
        let mut query = url.query_pairs_mut();
        for (key, value) in params {
            query.append_pair(key, value);
        }
    }
    Ok(url.to_string())
}

async fn throttle_nominatim() {
    static LAST: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    let lock = LAST.get_or_init(|| Mutex::new(None));
    let wait = {
        let mut guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match *guard {
            Some(at) => {
                let elapsed = at.elapsed();
                if elapsed < Duration::from_millis(1100) {
                    Some(Duration::from_millis(1100) - elapsed)
                } else {
                    *guard = Some(Instant::now());
                    None
                }
            }
            None => {
                *guard = Some(Instant::now());
                None
            }
        }
    };
    if let Some(wait) = wait {
        tokio::time::sleep(wait).await;
        if let Ok(mut guard) = lock.lock() {
            *guard = Some(Instant::now());
        }
    }
}

fn city_from_address(address: &Value) -> String {
    for key in ["city", "town", "village", "county", "state_district", "state"] {
        let value = val_text(address.get(key));
        if !value.is_empty() {
            return value;
        }
    }
    String::new()
}

pub async fn nominatim_search(keyword: &str, city: &str) -> Result<Vec<Place>, String> {
    throttle_nominatim().await;
    let query = if city.trim().is_empty() {
        keyword.to_string()
    } else {
        format!("{keyword}，{city}")
    };
    let url = build_url(
        NOMINATIM,
        "/search",
        &[
            ("format", "jsonv2".to_string()),
            ("q", query),
            ("accept-language", "zh-CN,zh,en".to_string()),
            ("limit", "8".to_string()),
            ("addressdetails", "1".to_string()),
        ],
    )?;
    let value = cached_json(&url, GEO_TTL).await?;
    let items = value
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for item in items {
        let (Some(lng), Some(lat)) = (
            val_f64(item.get("lon")),
            val_f64(item.get("lat")),
        ) else {
            continue;
        };
        let address = item.get("address");
        let display_name = val_text(item.get("display_name"));
        let mut name = val_text(item.get("name"));
        if name.is_empty() {
            name = display_name
                .split(',')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
        }
        if name.is_empty() {
            continue;
        }
        let place_city = address.map(city_from_address).unwrap_or_default();
        out.push(Place {
            id: format!(
                "osm-{}-{}",
                val_text(item.get("osm_type")),
                val_text(item.get("osm_id"))
            ),
            name,
            address: display_name,
            city: place_city,
            adcode: String::new(),
            city_code: String::new(),
            lng,
            lat,
            source: "osm".to_string(),
        });
    }
    Ok(out)
}

pub async fn geocode(address: &str, city: &str) -> Result<Place, String> {
    let items = nominatim_search(address, city).await?;
    items
        .into_iter()
        .next()
        .ok_or_else(|| format!("没有找到「{}」的坐标，请换一个更具体的地址", address))
}

pub async fn reverse(lat: f64, lng: f64) -> Result<Place, String> {
    throttle_nominatim().await;
    let url = build_url(
        NOMINATIM,
        "/reverse",
        &[
            ("format", "jsonv2".to_string()),
            ("lat", format!("{lat:.6}")),
            ("lon", format!("{lng:.6}")),
            ("accept-language", "zh-CN,zh,en".to_string()),
            ("addressdetails", "1".to_string()),
        ],
    )?;
    let value = cached_json(&url, GEO_TTL).await?;
    let display_name = val_text(value.get("display_name"));
    let address = value.get("address");
    let city = address.map(city_from_address).unwrap_or_default();
    let name = if display_name.is_empty() {
        format!("{lat:.4}, {lng:.4} 附近")
    } else {
        display_name.clone()
    };
    Ok(Place {
        id: format!("osm-reverse-{lat:.6},{lng:.6}"),
        name,
        address: display_name,
        city,
        adcode: String::new(),
        city_code: String::new(),
        lng,
        lat,
        source: "osm".to_string(),
    })
}

fn poi_kind(tags: &Value) -> &'static str {
    let tourism = val_text(tags.get("tourism"));
    if tourism.contains("hotel") || tourism.contains("guest_house") || tourism.contains("hostel") {
        return "hotel";
    }
    let amenity = val_text(tags.get("amenity"));
    if amenity.contains("restaurant") || amenity.contains("cafe") {
        return "food";
    }
    "sight"
}

fn poi_from_element(element: &Value, center: (f64, f64)) -> Option<Poi> {
    let tags = element.get("tags")?;
    let name = val_text(tags.get("name:zh"));
    let name = if name.is_empty() {
        val_text(tags.get("name"))
    } else {
        name
    };
    if name.trim().is_empty() {
        return None;
    }
    let (lng, lat) = if let (Some(lng), Some(lat)) = (
        val_f64(element.get("lon")),
        val_f64(element.get("lat")),
    ) {
        (lng, lat)
    } else if let Some(center_value) = element.get("center") {
        (
            val_f64(center_value.get("lon"))?,
            val_f64(center_value.get("lat"))?,
        )
    } else {
        return None;
    };
    let kind = poi_kind(tags).to_string();
    let category = {
        let tourism = val_text(tags.get("tourism"));
        let amenity = val_text(tags.get("amenity"));
        if !tourism.is_empty() {
            tourism
        } else {
            amenity
        }
    };
    let street = val_text(tags.get("addr:street"));
    let house = val_text(tags.get("addr:housenumber"));
    let address = [street, house]
        .into_iter()
        .filter(|item| !item.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    Some(Poi {
        id: format!(
            "osm-{}-{}",
            val_text(element.get("type")),
            val_text(element.get("id"))
        ),
        name,
        source: "osm".to_string(),
        kind,
        category,
        address,
        city: val_text(tags.get("addr:city")),
        lng,
        lat,
        rating: val_f64(tags.get("stars")),
        cost: None,
        tel: {
            let phone = val_text(tags.get("phone"));
            if phone.is_empty() {
                val_text(tags.get("contact:phone"))
            } else {
                phone
            }
        },
        distance: Some(haversine_km(center.0, center.1, lng, lat) * 1000.0),
        tags: [
            "cuisine",
            "tourism",
            "amenity",
            "historic",
            "leisure",
            "opening_hours",
            "website",
        ]
        .iter()
        .filter_map(|key| {
            let value = val_text(tags.get(key));
            if value.is_empty() {
                None
            } else {
                Some(format!("{key}: {value}"))
            }
        })
        .collect(),
    })
}

pub async fn overpass_pois(
    lat: f64,
    lng: f64,
    radius_m: f64,
) -> Result<(Vec<Poi>, Vec<Poi>, Vec<Poi>), String> {
    let radius = radius_m.clamp(3000.0, 40_000.0) as i64;
    let query = format!(
        r#"[out:json][timeout:25];
(
  nwr(around:{radius},{lat:.6},{lng:.6})["tourism"~"attraction|museum|viewpoint|zoo|theme_park|gallery"];
  nwr(around:{radius},{lat:.6},{lng:.6})["amenity"~"restaurant|cafe"];
  nwr(around:{radius},{lat:.6},{lng:.6})["tourism"~"hotel|guest_house|hostel"];
);
out center tags 90;"#
    );
    let value = cached_post_json(
        OVERPASS,
        vec![("data", query)],
        POI_TTL,
    )
    .await?;
    let elements = value
        .get("elements")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    let mut sights = Vec::new();
    let mut food = Vec::new();
    let mut hotels = Vec::new();
    for element in elements {
        let Some(poi) = poi_from_element(&element, (lng, lat)) else {
            continue;
        };
        match poi.kind.as_str() {
            "hotel" => hotels.push(poi),
            "food" => food.push(poi),
            _ => sights.push(poi),
        }
    }
    Ok((sights, food, hotels))
}

fn decode_polyline(encoded: &str) -> Vec<(f64, f64)> {
    let bytes = encoded.as_bytes();
    let mut index = 0usize;
    let mut lat = 0i64;
    let mut lng = 0i64;
    let mut out = Vec::new();
    while index < bytes.len() {
        let mut shift = 0u32;
        let mut result = 0i64;
        loop {
            if index >= bytes.len() {
                return out;
            }
            let byte = bytes[index] as i64 - 63;
            index += 1;
            result |= (byte & 0x1f) << shift;
            shift += 5;
            if byte < 0x20 {
                break;
            }
        }
        let delta_lat = if result & 1 != 0 {
            !(result >> 1)
        } else {
            result >> 1
        };
        lat += delta_lat;

        let mut shift = 0u32;
        let mut result = 0i64;
        loop {
            if index >= bytes.len() {
                return out;
            }
            let byte = bytes[index] as i64 - 63;
            index += 1;
            result |= (byte & 0x1f) << shift;
            shift += 5;
            if byte < 0x20 {
                break;
            }
        }
        let delta_lng = if result & 1 != 0 {
            !(result >> 1)
        } else {
            result >> 1
        };
        lng += delta_lng;
        out.push((lng as f64 / 1e5, lat as f64 / 1e5));
    }
    out
}

fn modifier_text(modifier: &str) -> &'static str {
    match modifier {
        "left" => "左转",
        "right" => "右转",
        "slight left" => "稍向左",
        "slight right" => "稍向右",
        "sharp left" => "急左转",
        "sharp right" => "急右转",
        "uturn" => "掉头",
        "straight" => "直行",
        _ => "",
    }
}

fn instruction_for(step: &Value) -> String {
    let maneuver = step.get("maneuver");
    let kind = maneuver
        .map(|item| val_text(item.get("type")))
        .unwrap_or_default();
    let modifier = maneuver
        .map(|item| val_text(item.get("modifier")))
        .unwrap_or_default();
    let road = val_text(step.get("name"));
    let distance = val_f64(step.get("distance")).unwrap_or(0.0);
    let modifier = modifier_text(&modifier);
    let road_text = if road.trim().is_empty() {
        "未命名道路".to_string()
    } else {
        road
    };
    let base = match kind.as_str() {
        "depart" => format!("从起点出发，沿{road_text}行驶"),
        "arrive" => "到达终点".to_string(),
        "turn" => format!("{modifier}进入{road_text}"),
        "continue" | "new name" => format!("沿{road_text}继续行驶"),
        "merge" => format!("汇入{road_text}"),
        "fork" => format!("在{road_text}走{modifier}岔路"),
        "roundabout" | "rotary" => format!("进入环岛，沿{road_text}行驶"),
        "end of road" => format!("在道路尽头{modifier}进入{road_text}"),
        "notification" => format!("注意：{road_text}"),
        _ => format!("沿{road_text}行驶"),
    };
    if kind == "arrive" {
        return base;
    }
    if distance > 0.0 {
        format!("{base}，约 {distance:.0} 米")
    } else {
        base
    }
}

pub async fn osrm_route(mode: &str, from: &RouteStop, to: &RouteStop) -> Result<RouteLeg, String> {
    let (profile, label, warning) = match mode {
        "walking" => ("foot", "步行", String::new()),
        "transit" => (
            "driving",
            "公共交通",
            "OpenStreetMap 不提供公交换乘，以下按驾车路线参考。".to_string(),
        ),
        _ => ("driving", "驾车", String::new()),
    };
    let coords = format!(
        "{:.6},{:.6};{:.6},{:.6}",
        from.lng, from.lat, to.lng, to.lat
    );
    let url = build_url(
        OSRM,
        &format!("/route/v1/{profile}/{coords}"),
        &[
            ("overview", "full".to_string()),
            ("geometries", "polyline".to_string()),
            ("steps", "true".to_string()),
        ],
    )?;
    let mut result = cached_json(&url, POI_TTL).await;
    if result.is_err() && profile == "foot" {
        // 公共 OSRM 实例不一定开放 foot profile，退回驾车。
        let url = build_url(
            OSRM,
            &format!("/route/v1/driving/{coords}"),
            &[
                ("overview", "full".to_string()),
                ("geometries", "polyline".to_string()),
                ("steps", "true".to_string()),
            ],
        )?;
        result = cached_json(&url, POI_TTL).await;
    }
    let value = result?;
    if val_text(value.get("code")) != "Ok" {
        return Err(format!(
            "OSRM 路线服务返回 {}",
            truncate(&val_text(value.get("message")), 120)
        ));
    }
    let route = value
        .pointer("/routes/0")
        .ok_or_else(|| "OSRM 没有返回路线".to_string())?;
    let distance = val_f64(route.get("distance")).unwrap_or(0.0) / 1000.0;
    let duration = val_f64(route.get("duration")).unwrap_or(0.0);
    let mut steps = Vec::new();
    let mut polylines = Vec::new();
    if let Some(legs) = route.get("legs").and_then(|item| item.as_array()) {
        for leg in legs {
            if let Some(items) = leg.get("steps").and_then(|item| item.as_array()) {
                for step in items {
                    steps.push(RouteStep {
                        instruction: instruction_for(step),
                        road: val_text(step.get("name")),
                        distance: val_f64(step.get("distance")).unwrap_or(0.0),
                        duration: val_f64(step.get("duration")).unwrap_or(0.0),
                    });
                    let geometry = val_text(step.get("geometry"));
                    if !geometry.trim().is_empty() {
                        let points = decode_polyline(&geometry)
                            .into_iter()
                            .map(|(lng, lat)| format!("{lng:.6},{lat:.6}"))
                            .collect::<Vec<_>>()
                            .join(";");
                        polylines.push(points);
                    }
                }
            }
        }
    }
    let full_polyline = {
        let geometry = val_text(route.get("geometry"));
        if !geometry.trim().is_empty() {
            decode_polyline(&geometry)
                .into_iter()
                .map(|(lng, lat)| format!("{lng:.6},{lat:.6}"))
                .collect::<Vec<_>>()
                .join(";")
        } else {
            coalesce_polyline(&polylines)
        }
    };
    Ok(RouteLeg {
        mode: if mode == "transit" {
            "transit".to_string()
        } else {
            mode.to_string()
        },
        distance,
        duration,
        tolls: 0.0,
        fare: 0.0,
        traffic: String::new(),
        summary: format!("{label} · {distance:.1} km · 约 {:.0} 分钟", duration / 60.0),
        steps,
        polyline: full_polyline,
        warning,
    })
}

async fn ipwho() -> Result<Value, String> {
    let url = "https://ipwho.is/?lang=zh-CN";
    cached_json(url, Duration::from_secs(3600)).await
}

async fn ipapi() -> Result<Value, String> {
    let url = "https://ipapi.co/json/";
    cached_json(url, Duration::from_secs(3600)).await
}

pub async fn ip_location() -> Result<Place, String> {
    let last_error;
    match ipwho().await {
        Ok(value) => {
            let success = value
                .get("success")
                .and_then(|item| item.as_bool())
                .unwrap_or(false);
            if success {
                let city = val_text(value.get("city"));
                let (Some(lng), Some(lat)) = (
                    val_f64(value.get("longitude")),
                    val_f64(value.get("latitude")),
                ) else {
                    return Err("IP 定位没有返回坐标".to_string());
                };
                if !city.trim().is_empty() {
                    if let Ok(reverse_place) = reverse(lat, lng).await {
                        return Ok(Place {
                            id: format!("osm-ip-{lng:.4},{lat:.4}"),
                            name: if reverse_place.name.trim().is_empty() {
                                city.clone()
                            } else {
                                reverse_place.name
                            },
                            address: reverse_place.address,
                            city: if reverse_place.city.trim().is_empty() {
                                city
                            } else {
                                reverse_place.city
                            },
                            lng,
                            lat,
                            source: "osm".to_string(),
                            ..Place::default()
                        });
                    }
                    return Ok(Place {
                        id: format!("osm-ip-{lng:.4},{lat:.4}"),
                        name: city.clone(),
                        address: city.clone(),
                        city,
                        lng,
                        lat,
                        source: "osm".to_string(),
                        ..Place::default()
                    });
                }
            }
            last_error = "ipwho.is 没有返回城市".to_string();
        }
        Err(error) => last_error = error,
    }

    match ipapi().await {
        Ok(value) => {
            let city = val_text(value.get("city"));
            let (Some(lng), Some(lat)) = (
                val_f64(value.get("longitude")),
                val_f64(value.get("latitude")),
            ) else {
                return Err("IP 定位没有返回坐标".to_string());
            };
            if city.trim().is_empty() {
                return Err("IP 定位没有返回城市".to_string());
            }
            Ok(Place {
                id: format!("osm-ip-{lng:.4},{lat:.4}"),
                name: city.clone(),
                address: [val_text(value.get("region")), city.clone()]
                    .into_iter()
                    .filter(|item| !item.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join(" "),
                city,
                lng,
                lat,
                source: "osm".to_string(),
                ..Place::default()
            })
        }
        Err(error) => Err(if last_error.is_empty() {
            error
        } else {
            format!("{last_error}；{error}")
        }),
    }
}
