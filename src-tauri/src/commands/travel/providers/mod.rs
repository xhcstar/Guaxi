//! 多地图服务商适配层。
//!
//! 支持：高德（已有）、腾讯、百度、Google、Mapbox、OpenStreetMap（已有）。
//! 每个服务商的坐标系不同：高德/腾讯 GCJ-02、百度 BD-09、Google/Mapbox/OSM WGS84。
//! 计划里保存数据源的原始坐标，并记录 `coordSystem`；地图组件按坐标系选择瓦片或转换。

pub mod baidu;
pub mod coord;
pub mod google;
pub mod mapbox;
pub mod tencent;

use crate::commands::settingview;

use super::{val_f64, val_text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapProvider {
    Auto,
    Amap,
    Osm,
    Baidu,
    Tencent,
    Google,
    Mapbox,
}

pub fn normalize(value: &str) -> MapProvider {
    match value.trim().to_lowercase().as_str() {
        "amap" | "gaode" => MapProvider::Amap,
        "osm" | "openstreetmap" => MapProvider::Osm,
        "baidu" => MapProvider::Baidu,
        "tencent" | "qq" => MapProvider::Tencent,
        "google" => MapProvider::Google,
        "mapbox" => MapProvider::Mapbox,
        _ => MapProvider::Auto,
    }
}

pub fn label(provider: MapProvider) -> &'static str {
    match provider {
        MapProvider::Amap => "amap",
        MapProvider::Osm => "osm",
        MapProvider::Baidu => "baidu",
        MapProvider::Tencent => "tencent",
        MapProvider::Google => "google",
        MapProvider::Mapbox => "mapbox",
        MapProvider::Auto => "auto",
    }
}

pub fn display_name(provider: MapProvider) -> &'static str {
    match provider {
        MapProvider::Amap => "高德地图",
        MapProvider::Osm => "OpenStreetMap",
        MapProvider::Baidu => "百度地图",
        MapProvider::Tencent => "腾讯地图",
        MapProvider::Google => "Google Maps",
        MapProvider::Mapbox => "Mapbox",
        MapProvider::Auto => "自动",
    }
}

pub fn coord_system(provider: MapProvider) -> &'static str {
    match provider {
        MapProvider::Amap | MapProvider::Tencent => "gcj02",
        MapProvider::Baidu => "bd09",
        _ => "wgs84",
    }
}

pub fn key(provider: MapProvider) -> String {
    settingview::get_travel_map_key(label(provider))
}

pub(crate) fn build_url(base: &str, path: &str, params: &[(&str, String)]) -> Result<String, String> {
    let mut url = reqwest::Url::parse(&format!("{base}{path}"))
        .map_err(|e| format!("构建地图请求地址失败: {e}"))?;
    {
        let mut query = url.query_pairs_mut();
        for (key, value) in params {
            query.append_pair(key, value);
        }
    }
    Ok(url.to_string())
}

pub(crate) fn as_text(value: Option<&serde_json::Value>) -> String {
    val_text(value)
}

pub(crate) fn as_f64(value: Option<&serde_json::Value>) -> Option<f64> {
    val_f64(value)
}

/// Google Directions 使用 polyline5；这里复用标准解码算法。
pub(crate) fn decode_polyline(encoded: &str) -> Vec<(f64, f64)> {
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

pub(crate) fn strip_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .trim()
        .to_string()
}
