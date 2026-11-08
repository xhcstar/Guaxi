//! 免 Key 的天气兜底：Open-Meteo 提供 16 天预报，正好补足高德只有 4 天的窗口。

use super::{cached_json, val_f64, val_text, WeatherDay, WEATHER_TTL};
use serde::{Deserialize, Serialize};

/// 路线沿途采样点的逐时天气：用于判断出发地、途经地区和目的地的实时天气差异。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WeatherPoint {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub lng: f64,
    #[serde(default)]
    pub lat: f64,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub time: String,
    #[serde(default)]
    pub weather: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub feels_like: Option<f64>,
    #[serde(default)]
    pub precipitation_probability: Option<f64>,
    #[serde(default)]
    pub precipitation: Option<f64>,
    #[serde(default)]
    pub wind_speed: Option<f64>,
    #[serde(default)]
    pub wind_direction: Option<f64>,
    /// 能见度（米），雾天判断用。
    #[serde(default)]
    pub visibility: Option<f64>,
    /// 采样点海拔（米），山区/高原提示用。
    #[serde(default)]
    pub elevation: Option<f64>,
    /// false 表示超出预报窗口或服务不可用，界面显示“临近更新”。
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub fetched_at: String,
}

fn code_text(code: i64) -> (&'static str, &'static str) {
    match code {
        0 => ("晴", "sun"),
        1 => ("大部晴朗", "sun"),
        2 => ("多云", "cloud"),
        3 => ("阴", "cloud"),
        45 | 48 => ("有雾", "fog"),
        51 | 53 | 55 => ("毛毛雨", "rain"),
        56 | 57 => ("冻毛毛雨", "rain"),
        61 => ("小雨", "rain"),
        63 => ("中雨", "rain"),
        65 => ("大雨", "rain"),
        66 | 67 => ("冻雨", "rain"),
        71 => ("小雪", "snow"),
        73 => ("中雪", "snow"),
        75 => ("大雪", "snow"),
        77 => ("米雪", "snow"),
        80 | 81 => ("阵雨", "rain"),
        82 => ("强阵雨", "rain"),
        85 | 86 => ("阵雪", "snow"),
        95 => ("雷阵雨", "storm"),
        96 | 99 => ("雷暴伴冰雹", "storm"),
        _ => ("天气未知", "cloud"),
    }
}

pub async fn open_meteo(lat: f64, lng: f64, days: u32) -> Result<Vec<WeatherDay>, String> {
    let days = days.clamp(1, 16);
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lng:.4}\
&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max,wind_speed_10m_max\
&timezone=auto&forecast_days={days}"
    );
    let value = cached_json(&url, WEATHER_TTL).await?;
    let daily = value
        .get("daily")
        .ok_or_else(|| "Open-Meteo 没有返回逐日预报".to_string())?;
    let dates = daily
        .get("time")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    let codes = daily
        .get("weather_code")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    let max_temps = daily
        .get("temperature_2m_max")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    let min_temps = daily
        .get("temperature_2m_min")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    let precipitation = daily
        .get("precipitation_probability_max")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    let wind = daily
        .get("wind_speed_10m_max")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::new();
    for (index, date) in dates.iter().enumerate() {
        let date = val_text(Some(date));
        if date.is_empty() {
            continue;
        }
        let code = codes
            .get(index)
            .and_then(|item| item.as_i64())
            .unwrap_or(2);
        let (text, icon) = code_text(code);
        out.push(WeatherDay {
            date,
            weekday: String::new(),
            day_weather: text.to_string(),
            night_weather: text.to_string(),
            day_temp: max_temps.get(index).and_then(|item| val_f64(Some(item))),
            night_temp: min_temps.get(index).and_then(|item| val_f64(Some(item))),
            wind: "风速".to_string(),
            wind_level: wind
                .get(index)
                .and_then(|item| val_f64(Some(item)))
                .map(|speed| format!("{speed:.0} km/h"))
                .unwrap_or_default(),
            precipitation: precipitation
                .get(index)
                .and_then(|item| val_f64(Some(item))),
            icon: icon.to_string(),
            source: "open-meteo".to_string(),
        });
    }
    Ok(out)
}

fn minutes_of(time: &str) -> i64 {
    let mut parts = time.split(':');
    let hour = parts
        .next()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(12)
        .clamp(0, 23);
    let minute = parts
        .next()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
        .clamp(0, 59);
    hour * 60 + minute
}

fn pick_hour(times: &[serde_json::Value], date: &str, time: &str) -> Option<usize> {
    let target = minutes_of(time);
    let mut best: Option<(usize, i64)> = None;
    for (index, item) in times.iter().enumerate() {
        let stamp = item.as_str().unwrap_or("");
        if !stamp.starts_with(date) {
            continue;
        }
        let minutes = stamp.get(11..16).map(minutes_of).unwrap_or(720);
        let diff = (minutes - target).abs();
        if best.map(|(_, value)| diff < value).unwrap_or(true) {
            best = Some((index, diff));
        }
    }
    best.map(|(index, _)| index)
}

fn hourly_f64(hourly: &serde_json::Value, key: &str, index: usize) -> Option<f64> {
    hourly
        .get(key)
        .and_then(|value| value.as_array())
        .and_then(|items| items.get(index))
        .and_then(|value| value.as_f64())
}

fn hourly_i64(hourly: &serde_json::Value, key: &str, index: usize) -> Option<i64> {
    hourly
        .get(key)
        .and_then(|value| value.as_array())
        .and_then(|items| items.get(index))
        .and_then(|value| value.as_i64())
}

/// 取某个坐标在指定日期与时刻的逐时天气。日期超出 16 天窗口时不发请求，
/// 直接返回 `available=false`，避免把旧快照当成实时预报。
pub async fn point_weather(
    name: String,
    lat: f64,
    lng: f64,
    date: &str,
    time: &str,
) -> WeatherPoint {
    let mut point = WeatherPoint {
        name,
        lng,
        lat,
        date: date.to_string(),
        time: time.to_string(),
        available: false,
        source: "open-meteo".to_string(),
        fetched_at: chrono::Local::now().to_rfc3339(),
        ..WeatherPoint::default()
    };
    if date.len() < 10 {
        point.message = "未指定日期".to_string();
        return point;
    }
    if let Ok(target) = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        let offset = (target - chrono::Local::now().date_naive()).num_days();
        if offset > 16 {
            point.message = "超过 16 天预报窗口，临近出发再更新".to_string();
            return point;
        }
        if offset < -1 {
            point.message = "日期已过".to_string();
            return point;
        }
    }
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lng:.4}\
&hourly=weather_code,temperature_2m,apparent_temperature,precipitation_probability,precipitation,wind_speed_10m,wind_direction_10m,visibility\
&timezone=auto&start_date={date}&end_date={date}"
    );
    let value = match cached_json(&url, WEATHER_TTL).await {
        Ok(value) => value,
        Err(error) => {
            point.message = format!("天气服务暂不可用：{}", super::truncate(&error, 80));
            return point;
        }
    };
    let Some(hourly) = value.get("hourly") else {
        point.message = "天气服务未返回逐时数据".to_string();
        return point;
    };
    let times = hourly
        .get("time")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let Some(index) = pick_hour(&times, date, time) else {
        point.message = "该日期暂无逐时预报".to_string();
        return point;
    };
    let code = hourly_i64(hourly, "weather_code", index).unwrap_or(2);
    let (text, icon) = code_text(code);
    point.weather = text.to_string();
    point.icon = icon.to_string();
    point.temperature = hourly_f64(hourly, "temperature_2m", index);
    point.feels_like = hourly_f64(hourly, "apparent_temperature", index);
    point.precipitation_probability = hourly_f64(hourly, "precipitation_probability", index);
    point.precipitation = hourly_f64(hourly, "precipitation", index);
    point.wind_speed = hourly_f64(hourly, "wind_speed_10m", index);
    point.wind_direction = hourly_f64(hourly, "wind_direction_10m", index);
    point.visibility = hourly_f64(hourly, "visibility", index);
    point.elevation = value.get("elevation").and_then(|item| val_f64(Some(item)));
    point.available = true;
    point
}

pub fn season_label(date: &str) -> String {
    let month = date
        .get(5..7)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    let (season, phase) = match month {
        3 => ("春季", "早春"),
        4 => ("春季", "仲春"),
        5 => ("春季", "暮春"),
        6 => ("夏季", "初夏"),
        7 => ("夏季", "盛夏"),
        8 => ("夏季", "夏末"),
        9 => ("秋季", "初秋"),
        10 => ("秋季", "金秋"),
        11 => ("秋季", "深秋"),
        12 => ("冬季", "初冬"),
        1 => ("冬季", "隆冬"),
        2 => ("冬季", "晚冬"),
        _ => return String::new(),
    };
    format!("{season} · {phase}（{month} 月）")
}

pub fn season_tips(date: &str) -> Vec<String> {
    let month = date
        .get(5..7)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    let tips: &[&str] = match month {
        3..=5 => &[
            "春季昼夜温差大，早晚加一件薄外套。",
            "春雨来得快，随身带折叠伞，户外点留室内备选。",
            "花期与踏青人流集中，热门景区尽量早到。",
        ],
        6..=8 => &[
            "夏季高温，11:00-15:00 减少暴晒，备足饮水。",
            "午后易有雷阵雨，户外行程留出室内备选。",
            "山区蚊虫多，穿长裤并带驱蚊用品。",
        ],
        9..=11 => &[
            "秋季昼夜温差大，清晨和山顶明显更冷。",
            "秋雨与晨雾会影响能见度，自驾注意车灯和车速。",
            "日落提前，下午的户外点尽量安排在 16:30 前。",
        ],
        12 | 1 | 2 => &[
            "冬季低温，羽绒服、手套和保暖鞋是刚需。",
            "山区和桥面可能结冰，自驾减速、避免急刹。",
            "日照时间短，尽量把户外行程放在中午前后。",
        ],
        _ => &[],
    };
    tips.iter().map(|item| item.to_string()).collect()
}

/// 高德前 4 天优先；Open-Meteo 补足后面的日期。
pub fn merge(amap_days: Vec<WeatherDay>, open_days: Vec<WeatherDay>) -> Vec<WeatherDay> {
    let mut by_date: std::collections::BTreeMap<String, WeatherDay> =
        std::collections::BTreeMap::new();
    for day in open_days {
        by_date.insert(day.date.clone(), day);
    }
    for day in amap_days {
        if !day.date.is_empty() {
            by_date.insert(day.date.clone(), day);
        }
    }
    by_date.into_values().collect()
}
