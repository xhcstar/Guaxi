//! 目的地候选池：把一个城市/景区拆成可挑选的分类候选，供用户在编排前自己选。
//!
//! 这一层只做“检索 + 归一化”：高德按类别关键字检索并保留评分/人均，OSM 用一次
//! Overpass 查询覆盖全部类别。评分、推荐理由和自动挑选放在前端，权重随时可调，
//! 也便于用纯函数测试。

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::providers::{coord, MapProvider};
use super::{
    amap, amap_key, haversine_km, is_infra_poi, osm, poi_matches_avoid, resolve_place, truncate,
    weather, Place, PlaceRef, Poi, WeatherDay,
};

const DEFAULT_LIMIT_PER_CATEGORY: usize = 12;
const MAX_CANDIDATES: usize = 160;
const OSM_RADIUS_M: f64 = 25_000.0;

struct CategoryDef {
    key: &'static str,
    label: &'static str,
    types: &'static str,
    keywords: &'static str,
    kind: &'static str,
}

/// 类别就是用户提的那些出游类型；关键字用 `|` 合并，一次检索覆盖同类。
const CATEGORIES: &[CategoryDef] = &[
    CategoryDef {
        key: "nature",
        label: "自然风光",
        types: "110000",
        keywords: "风景区|湿地|森林公园|湖泊|峡谷|瀑布|草原|海滩",
        kind: "sight",
    },
    CategoryDef {
        key: "heritage",
        label: "人文古迹",
        types: "110000",
        keywords: "古镇|古村|遗址|故居|城墙|古塔|园林|古迹",
        kind: "sight",
    },
    CategoryDef {
        key: "museum",
        label: "博物馆",
        types: "140000",
        keywords: "博物馆|美术馆|展览馆|纪念馆|科技馆",
        kind: "sight",
    },
    CategoryDef {
        key: "landmark",
        label: "地标",
        types: "110000|190000",
        keywords: "地标|广场|观景台|大桥",
        kind: "sight",
    },
    CategoryDef {
        key: "street",
        label: "街区商业",
        types: "060000",
        keywords: "步行街|商业街|老街|历史街区",
        kind: "sight",
    },
    CategoryDef {
        key: "night",
        label: "夜市美食街",
        types: "050000",
        keywords: "夜市|美食街|小吃街",
        kind: "food",
    },
    CategoryDef {
        key: "market",
        label: "市场集市",
        types: "060000|070000",
        keywords: "市场|集市|菜市场",
        kind: "sight",
    },
    CategoryDef {
        key: "hike",
        label: "徒步户外",
        types: "110000",
        keywords: "古道|步道|徒步|登山|营地|栈道",
        kind: "sight",
    },
    CategoryDef {
        key: "family",
        label: "亲子",
        types: "110000|080000",
        keywords: "乐园|动物园|海洋馆|水族馆|植物园|亲子",
        kind: "sight",
    },
    CategoryDef {
        key: "temple",
        label: "寺庙",
        types: "110000",
        keywords: "寺|庙|道观|教堂",
        kind: "sight",
    },
    CategoryDef {
        key: "show",
        label: "演出剧场",
        types: "080000",
        keywords: "剧院|剧场|演出|音乐厅|影院",
        kind: "sight",
    },
    CategoryDef {
        key: "hotspring",
        label: "温泉",
        types: "110000|080000",
        keywords: "温泉|汤泉",
        kind: "sight",
    },
    CategoryDef {
        key: "food",
        label: "特色餐饮",
        types: "050000",
        keywords: "特色餐厅|本地菜|老字号|小吃",
        kind: "food",
    },
];

const HOTEL_TYPES: &str = "100000";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ExploreRequest {
    pub origin: PlaceRef,
    pub destination: PlaceRef,
    /// 省级目的地先由用户挑基地城市；为空时只返回范围与提示。
    #[serde(default)]
    pub bases: Vec<PlaceRef>,
    /// 为空表示扫描全部类别。
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub interests: Vec<String>,
    #[serde(default)]
    pub avoid: Vec<String>,
    #[serde(default)]
    pub must_visit: Vec<String>,
    #[serde(default)]
    pub budget_level: String,
    #[serde(default)]
    pub adults: u32,
    #[serde(default)]
    pub children: u32,
    #[serde(default)]
    pub elders: u32,
    #[serde(default)]
    pub pace: String,
    #[serde(default)]
    pub transport: String,
    #[serde(default)]
    pub start_date: String,
    #[serde(default)]
    pub end_date: String,
    #[serde(default)]
    pub limit_per_category: Option<usize>,
    /// auto | amap | baidu | tencent | google | mapbox | osm
    #[serde(default)]
    pub map_provider: String,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExploreBase {
    pub id: String,
    pub name: String,
    pub city: String,
    pub adcode: String,
    pub lng: f64,
    pub lat: f64,
    pub distance_km: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExploreCandidate {
    #[serde(flatten)]
    pub poi: Poi,
    pub category_key: String,
    pub category_label: String,
    pub base_id: String,
    pub base_name: String,
    pub distance_base_km: f64,
    pub distance_origin_km: f64,
    pub must: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExploreResult {
    /// poi | city | province
    pub scope: String,
    pub source: String,
    pub coord_system: String,
    pub origin: Place,
    pub destination: Place,
    pub bases: Vec<ExploreBase>,
    pub candidates: Vec<ExploreCandidate>,
    pub hotels: Vec<Poi>,
    pub weather: Vec<WeatherDay>,
    pub notes: Vec<String>,
    pub collected_at: String,
    pub total: usize,
    pub truncated: bool,
}

fn scope_of(place: &Place) -> &'static str {
    let name = place.name.as_str();
    let adcode = place.adcode.trim();
    if name.contains('省') || name.contains("自治区") || (adcode.len() == 6 && adcode.ends_with("0000")) {
        return "province";
    }
    if name.ends_with('市') || name.contains("地区") || name.contains("自治州") {
        return "city";
    }
    if adcode.len() == 6 && adcode.ends_with("00") {
        return "city";
    }
    if adcode.is_empty() {
        return "poi";
    }
    "poi"
}

fn candidate_from(
    mut poi: Poi,
    category: &CategoryDef,
    base: &Place,
    origin: &Place,
    must: bool,
) -> ExploreCandidate {
    let distance = haversine_km(base.lng, base.lat, poi.lng, poi.lat);
    poi.kind = category.kind.to_string();
    poi.distance = Some(distance * 1000.0);
    ExploreCandidate {
        distance_base_km: (distance * 10.0).round() / 10.0,
        distance_origin_km: (haversine_km(origin.lng, origin.lat, poi.lng, poi.lat) * 10.0).round()
            / 10.0,
        base_id: base.adcode.clone(),
        base_name: if base.name.is_empty() {
            base.city.clone()
        } else {
            base.name.clone()
        },
        category_key: if must {
            "must".to_string()
        } else {
            category.key.to_string()
        },
        category_label: if must {
            "必去".to_string()
        } else {
            category.label.to_string()
        },
        must,
        poi,
    }
}

fn normalize_name(name: &str) -> String {
    name.chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '·' && *ch != '-' && *ch != '（' && *ch != '）')
        .collect::<String>()
        .to_lowercase()
}

fn dedupe_candidates(items: Vec<ExploreCandidate>) -> Vec<ExploreCandidate> {
    let mut seen_id = HashSet::new();
    let mut seen_geo = HashSet::new();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let id = item.poi.id.trim().to_string();
        let geo = format!(
            "{}|{:.3},{:.3}",
            normalize_name(&item.poi.name),
            item.poi.lng,
            item.poi.lat
        );
        if !id.is_empty() && !seen_id.insert(id) {
            continue;
        }
        if !seen_geo.insert(geo) {
            continue;
        }
        out.push(item);
    }
    out
}

fn sort_by_quality(items: &mut [ExploreCandidate]) {
    items.sort_by(|a, b| {
        b.must.cmp(&a.must).then_with(|| {
            let rating = b
                .poi
                .rating
                .unwrap_or(0.0)
                .partial_cmp(&a.poi.rating.unwrap_or(0.0))
                .unwrap();
            rating.then_with(|| {
                a.distance_base_km
                    .partial_cmp(&b.distance_base_km)
                    .unwrap()
            })
        })
    });
}

fn filter_candidates(
    items: Vec<ExploreCandidate>,
    request: &ExploreRequest,
    base: &Place,
) -> Vec<ExploreCandidate> {
    let mut keep: Vec<ExploreCandidate> = items
        .into_iter()
        .filter(|item| !poi_matches_avoid(&item.poi, &request.avoid))
        .filter(|item| {
            item.category_key == "must"
                || item.poi.kind != "sight"
                || !is_infra_poi(&item.poi)
        })
        .filter(|item| {
            item.poi.lng.abs() > 0.0001
                && item.poi.lat.abs() > 0.0001
                && haversine_km(base.lng, base.lat, item.poi.lng, item.poi.lat) <= 80.0
        })
        .collect();
    // 必去点通常也会被某个类别搜到；去重时让“必去”版本留下。
    keep.sort_by_key(|item| !item.must);
    dedupe_candidates(keep)
}

async fn sweep_amap(key: &str, base: &Place, origin: &Place, request: &ExploreRequest) -> (Vec<ExploreCandidate>, Vec<Poi>, Vec<String>) {
    // 优先用城市名而不是区县 adcode：景点通常落在某个区，按区检索会把同城其他区漏掉。
    let city = if !base.city.trim().is_empty() {
        base.city.clone()
    } else if !base.adcode.trim().is_empty() {
        base.adcode.clone()
    } else {
        base.name.clone()
    };
    let mut candidates = Vec::new();
    let mut hotels = Vec::new();
    let mut notes = Vec::new();
    let mut failures = 0usize;
    let limit = request
        .limit_per_category
        .unwrap_or(DEFAULT_LIMIT_PER_CATEGORY)
        .clamp(4, 25);
    let only: Option<HashSet<&str>> = if request.categories.is_empty() {
        None
    } else {
        Some(request.categories.iter().map(|item| item.as_str()).collect())
    };

    for category in CATEGORIES {
        if let Some(filter) = &only {
            if !filter.contains(category.key) {
                continue;
            }
        }
        match amap::place_text_typed(key, category.keywords, &city, category.types, true).await {
            Ok(items) => {
                let mut bucket: Vec<ExploreCandidate> = items
                    .into_iter()
                    .map(|poi| candidate_from(poi, category, base, origin, false))
                    .collect();
                sort_by_quality(&mut bucket);
                candidates.extend(bucket.into_iter().take(limit));
            }
            Err(error) => {
                failures += 1;
                if failures == 1 {
                    notes.push(format!(
                        "{}检索失败，已跳过：{}",
                        category.label,
                        truncate(&error, 80)
                    ));
                }
            }
        }
    }

    if let Ok(items) = amap::place_text_typed(key, "", &city, HOTEL_TYPES, true).await {
        hotels.extend(items);
    }

    for must in &request.must_visit {
        let keyword = must.trim();
        if keyword.is_empty() {
            continue;
        }
        if let Ok(items) = amap::place_text_typed(key, keyword, &city, "", true).await {
            let marker = CategoryDef {
                key: "must",
                label: "必去",
                types: "",
                keywords: "",
                kind: "sight",
            };
            let bucket: Vec<ExploreCandidate> = items
                .into_iter()
                .take(3)
                .map(|poi| candidate_from(poi, &marker, base, origin, true))
                .collect();
            candidates.extend(bucket);
        }
    }

    (candidates, hotels, notes)
}

fn osm_category(name: &str, tags: &[String]) -> Option<(&'static str, &'static str, &'static str)> {
    let joined = tags.join(" ");
    let haystack = format!("{name} {joined}").to_lowercase();
    let has = |value: &str| haystack.contains(value);
    if haystack.contains("夜市") || haystack.contains("night_market") {
        return Some(("night", "夜市美食街", "food"));
    }
    if has("hotel") || has("guest_house") || has("hostel") {
        return Some(("hotel", "住宿", "hotel"));
    }
    if has("restaurant") || has("cafe") || has("food_court") {
        return Some(("food", "特色餐饮", "food"));
    }
    if has("museum") || has("gallery") || has("planetarium") {
        return Some(("museum", "博物馆", "sight"));
    }
    if has("zoo") || has("theme_park") || has("aquarium") || has("water_park") {
        return Some(("family", "亲子", "sight"));
    }
    if has("theatre") || has("cinema") || has("arts_centre") {
        return Some(("show", "演出剧场", "sight"));
    }
    if has("marketplace") || has("market") {
        return Some(("market", "市场集市", "sight"));
    }
    if has("mall") || has("department_store") {
        return Some(("street", "街区商业", "sight"));
    }
    if has("hot_spring") {
        return Some(("hotspring", "温泉", "sight"));
    }
    if has("historic") || has("monument") || has("memorial") || has("ruins") {
        return Some(("heritage", "人文古迹", "sight"));
    }
    if has("temple") || has("place_of_worship") || has("shrine") {
        return Some(("temple", "寺庙", "sight"));
    }
    if has("peak") || has("waterfall") || has("cave") || has("nature_reserve") || has("park") {
        return Some(("nature", "自然风光", "sight"));
    }
    if has("attraction") || has("viewpoint") || has("artwork") || has("monument") {
        return Some(("landmark", "地标", "sight"));
    }
    None
}

async fn sweep_osm(base: &Place, origin: &Place, request: &ExploreRequest) -> (Vec<ExploreCandidate>, Vec<Poi>, Vec<String>) {
    let mut notes = Vec::new();
    let (wgs_lng, wgs_lat) = coord::gcj02_to_wgs84(base.lng, base.lat);
    let items = match osm::explore_pois(wgs_lat, wgs_lng, OSM_RADIUS_M).await {
        Ok(items) => items,
        Err(error) => {
            notes.push(format!("OpenStreetMap 候选检索失败：{}", truncate(&error, 120)));
            Vec::new()
        }
    };
    let limit = request
        .limit_per_category
        .unwrap_or(DEFAULT_LIMIT_PER_CATEGORY)
        .clamp(4, 25);
    let mut buckets: std::collections::HashMap<&'static str, Vec<ExploreCandidate>> =
        std::collections::HashMap::new();
    let mut hotels = Vec::new();
    for poi in items {
        let name = poi.name.clone();
        let Some((key, label, kind)) = osm_category(&name, &poi.tags) else {
            continue;
        };
        let category = CategoryDef {
            key,
            label,
            types: "",
            keywords: "",
            kind,
        };
        let mut candidate = candidate_from(poi, &category, base, origin, false);
        // OSM 坐标是 WGS-84；统一转成 GCJ-02，和地图/路线保持同一坐标系。
        let (lng, lat) = coord::wgs84_to_gcj02(candidate.poi.lng, candidate.poi.lat);
        candidate.poi.lng = lng;
        candidate.poi.lat = lat;
        candidate.distance_base_km =
            (haversine_km(base.lng, base.lat, lng, lat) * 10.0).round() / 10.0;
        candidate.poi.distance = Some(candidate.distance_base_km * 1000.0);
        candidate.distance_origin_km =
            (haversine_km(origin.lng, origin.lat, lng, lat) * 10.0).round() / 10.0;
        if key == "hotel" {
            hotels.push(candidate.poi);
        } else {
            buckets.entry(key).or_default().push(candidate);
        }
    }
    let mut candidates = Vec::new();
    for (_, mut bucket) in buckets {
        sort_by_quality(&mut bucket);
        candidates.extend(bucket.into_iter().take(limit));
    }
    notes.push(
        "候选来自 OpenStreetMap：地点齐全但没有评分与人均，相关指标会标注为未核实。"
            .to_string(),
    );
    (candidates, hotels, notes)
}

fn dedupe_hotels(mut items: Vec<Poi>) -> Vec<Poi> {
    let mut seen = HashSet::new();
    items.retain(|poi| {
        let key = if poi.id.trim().is_empty() {
            format!("{}|{:.3},{:.3}", normalize_name(&poi.name), poi.lng, poi.lat)
        } else {
            poi.id.clone()
        };
        seen.insert(key)
    });
    items.sort_by(|a, b| {
        b.rating
            .unwrap_or(0.0)
            .partial_cmp(&a.rating.unwrap_or(0.0))
            .unwrap()
    });
    items.truncate(8);
    items
}

async fn collect_weather(key: &str, place: &Place) -> Vec<WeatherDay> {
    let mut from_amap = Vec::new();
    if !key.trim().is_empty() && !place.adcode.trim().is_empty() {
        if let Ok(items) = amap::weather(key, &place.adcode).await {
            from_amap = items;
        }
    }
    let (wgs_lng, wgs_lat) = coord::gcj02_to_wgs84(place.lng, place.lat);
    let from_open = weather::open_meteo(wgs_lat, wgs_lng, 16)
        .await
        .unwrap_or_default();
    weather::merge(from_amap, from_open)
}

pub async fn explore(
    request: &ExploreRequest,
    provider: MapProvider,
) -> Result<ExploreResult, String> {
    let key = amap_key();
    // 候选池只走两条成熟链路：高德（有 Key）或 OSM。百度的周边搜索、腾讯的
    // 区域检索都没有统一的分页/评分字段，混进来只会让评分口径不一致。
    let use_amap = !key.trim().is_empty()
        && !matches!(
            provider,
            MapProvider::Osm | MapProvider::Google | MapProvider::Mapbox
        );
    let origin = resolve_place(&request.origin, if use_amap { &key } else { "" }).await?;
    let destination = resolve_place(
        &request.destination,
        if use_amap { &key } else { "" },
    )
    .await?;
    if destination.lng.abs() < 0.0001 || destination.lat.abs() < 0.0001 {
        return Err("没有解析到目的地的坐标，请重新搜索目的地".to_string());
    }

    let scope = scope_of(&destination);
    let mut notes = Vec::new();
    let mut bases = Vec::new();
    let base_places: Vec<Place> = if scope == "province" {
        if request.bases.is_empty() {
            notes.push(
                "这是一个省级目的地。先添加 2–4 个基地城市，再按基地扫描候选，避免整天在路上。"
                    .to_string(),
            );
            Vec::new()
        } else {
            let mut resolved = Vec::new();
            for base in &request.bases {
                match resolve_place(base, if use_amap { &key } else { "" }).await {
                    Ok(place) => resolved.push(place),
                    Err(error) => notes.push(format!("基地「{}」解析失败：{}", base.name, truncate(&error, 80))),
                }
            }
            resolved
        }
    } else {
        vec![destination.clone()]
    };

    for base in &base_places {
        bases.push(ExploreBase {
            id: base.adcode.clone(),
            name: base.name.clone(),
            city: base.city.clone(),
            adcode: base.adcode.clone(),
            lng: base.lng,
            lat: base.lat,
            distance_km: (haversine_km(origin.lng, origin.lat, base.lng, base.lat) * 10.0).round()
                / 10.0,
        });
    }

    let mut candidates = Vec::new();
    let mut hotels = Vec::new();
    if !base_places.is_empty() {
        for base in &base_places {
            let (found, mut stays, mut base_notes) = if use_amap {
                sweep_amap(&key, base, &origin, request).await
            } else {
                sweep_osm(base, &origin, request).await
            };
            candidates.append(&mut filter_candidates(found, request, base));
            hotels.append(&mut stays);
            notes.append(&mut base_notes);
        }
    }

    if use_amap {
        notes.push(
            "候选由高德按类别检索：评分、人均来自高德，同城同类检索会缓存 7 天。".to_string(),
        );
    }

    let mut candidates = dedupe_candidates(candidates);
    let total_before_cap = candidates.len();
    sort_by_quality(&mut candidates);
    let truncated = total_before_cap > MAX_CANDIDATES;
    candidates.truncate(MAX_CANDIDATES);
    let total = candidates.len();
    let hotels = dedupe_hotels(hotels);

    let weather_place = base_places.first().cloned().unwrap_or_else(|| destination.clone());
    let weather_days = collect_weather(&key, &weather_place).await;

    if !base_places.is_empty() {
        notes.push(format!(
            "本次扫描到 {} 个候选地点、{} 家住宿。勾选的才会进入行程；没勾餐饮时会自动补 2 个备选。",
            total,
            hotels.len()
        ));
    }

    Ok(ExploreResult {
        scope: scope.to_string(),
        source: if use_amap { "amap" } else { "osm" }.to_string(),
        coord_system: "gcj02".to_string(),
        origin,
        destination,
        bases,
        candidates,
        hotels,
        weather: weather_days,
        notes,
        collected_at: chrono::Local::now().to_rfc3339(),
        total,
        truncated,
    })
}
