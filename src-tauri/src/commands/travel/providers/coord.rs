//! WGS84 / GCJ-02 / BD-09 坐标转换，供天气、跨服务商导航和地图显示使用。

use super::MapProvider;

const PI: f64 = std::f64::consts::PI;
const AXIS: f64 = 6378245.0;
const ECCENTRICITY: f64 = 0.00669342162296594323;

fn out_of_china(lng: f64, lat: f64) -> bool {
    lng < 72.004 || lng > 137.8347 || lat < 0.8293 || lat > 55.8271
}

fn transform_lat(lng: f64, lat: f64) -> f64 {
    let mut ret = -100.0 + 2.0 * lng + 3.0 * lat + 0.2 * lat * lat + 0.1 * lng * lat + 0.2 * (lng.abs()).sqrt();
    ret += ((20.0 * (6.0 * lng * PI).sin() + 20.0 * (2.0 * lng * PI).sin()) * 2.0) / 3.0;
    ret += ((20.0 * (lat * PI).sin() + 40.0 * ((lat / 3.0) * PI).sin()) * 2.0) / 3.0;
    ret += ((160.0 * ((lat / 12.0) * PI).sin() + 320.0 * ((lat * PI) / 30.0).sin()) * 2.0) / 3.0;
    ret
}

fn transform_lng(lng: f64, lat: f64) -> f64 {
    let mut ret = 300.0 + lng + 2.0 * lat + 0.1 * lng * lng + 0.1 * lng * lat + 0.1 * (lng.abs()).sqrt();
    ret += ((20.0 * (6.0 * lng * PI).sin() + 20.0 * (2.0 * lng * PI).sin()) * 2.0) / 3.0;
    ret += ((20.0 * (lng * PI).sin() + 40.0 * ((lng / 3.0) * PI).sin()) * 2.0) / 3.0;
    ret += ((150.0 * ((lng / 12.0) * PI).sin() + 300.0 * ((lng / 30.0) * PI).sin()) * 2.0) / 3.0;
    ret
}

pub fn wgs84_to_gcj02(lng: f64, lat: f64) -> (f64, f64) {
    if out_of_china(lng, lat) {
        return (lng, lat);
    }
    let mut d_lat = transform_lat(lng - 105.0, lat - 35.0);
    let mut d_lng = transform_lng(lng - 105.0, lat - 35.0);
    let rad_lat = (lat / 180.0) * PI;
    let mut magic = rad_lat.sin();
    magic = 1.0 - ECCENTRICITY * magic * magic;
    let sqrt_magic = magic.sqrt();
    d_lat = (d_lat * 180.0) / (((AXIS * (1.0 - ECCENTRICITY)) / (magic * sqrt_magic)) * PI);
    d_lng = (d_lng * 180.0) / ((AXIS / sqrt_magic) * rad_lat.cos() * PI);
    (lng + d_lng, lat + d_lat)
}

pub fn gcj02_to_wgs84(lng: f64, lat: f64) -> (f64, f64) {
    if out_of_china(lng, lat) {
        return (lng, lat);
    }
    let (g_lng, g_lat) = wgs84_to_gcj02(lng, lat);
    (lng * 2.0 - g_lng, lat * 2.0 - g_lat)
}

#[allow(dead_code)]
pub fn gcj02_to_bd09(lng: f64, lat: f64) -> (f64, f64) {
    let z = (lng * lng + lat * lat).sqrt() + 0.00002 * ((lat * PI * 3000.0) / 180.0).sin();
    let theta = lat.atan2(lng) + 0.000003 * ((lng * PI * 3000.0) / 180.0).cos();
    (z * theta.cos() + 0.0065, z * theta.sin() + 0.006)
}

pub fn bd09_to_gcj02(lng: f64, lat: f64) -> (f64, f64) {
    let x = lng - 0.0065;
    let y = lat - 0.006;
    let z = (x * x + y * y).sqrt() - 0.00002 * ((y * PI * 3000.0) / 180.0).sin();
    let theta = y.atan2(x) - 0.000003 * ((x * PI * 3000.0) / 180.0).cos();
    (z * theta.cos(), z * theta.sin())
}

pub fn bd09_to_wgs84(lng: f64, lat: f64) -> (f64, f64) {
    let (g_lng, g_lat) = bd09_to_gcj02(lng, lat);
    gcj02_to_wgs84(g_lng, g_lat)
}

pub fn to_wgs84(provider: MapProvider, lng: f64, lat: f64) -> (f64, f64) {
    match provider {
        MapProvider::Amap | MapProvider::Tencent => gcj02_to_wgs84(lng, lat),
        MapProvider::Baidu => bd09_to_wgs84(lng, lat),
        _ => (lng, lat),
    }
}
