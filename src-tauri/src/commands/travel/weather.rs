//! 免 Key 的天气兜底：Open-Meteo 提供 16 天预报，正好补足高德只有 4 天的窗口。

use super::{cached_json, val_f64, val_text, WeatherDay, WEATHER_TTL};

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
