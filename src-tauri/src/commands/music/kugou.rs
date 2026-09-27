//! 酷狗账号 provider：登录 + 我的歌单 + 歌单曲目。
//!
//! Cookie 复用 flipboard_render 的持久化浏览器 profile。
//! 说明(重要)：酷狗的用户歌单接口需要 md5 签名且版本敏感，以下实现按公开资料的
//! 移动端 v3 接口 + 常见签名写法编写，**未经真机验证**，很可能需要按实际返回联调。
//! 播放同样走 UNM 解灰(酷狗本身也是解灰源)。

use md5::{Digest, Md5};
use once_cell::sync::Lazy;
use serde_json::Value;

use super::{OnlineAccount, OnlinePlaylist, OnlineTrack};

const PROFILE: &str = "music_kugou";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36";
// 酷狗移动端签名盐(公开资料常见值)。若真机联调不通，优先怀疑此盐与参数集。
const SIGN_KEY: &str = "OIlwieks28dk2k092lksi2UIkp";

static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent(UA)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

async fn kugou_cookie() -> Option<String> {
    crate::commands::flipboard_render::cookie_header_for_url("https://www.kugou.com", PROFILE).await
}

fn cookie_val(cookie: &str, key: &str) -> Option<String> {
    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix(&format!("{key}=")) {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

// 签名：md5(SIGN_KEY + 按 key 升序拼接的 "kv" + SIGN_KEY)。参数值不带分隔符直接相连是酷狗常见写法。
fn sign(params: &[(&str, String)]) -> String {
    let mut sorted: Vec<&(&str, String)> = params.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    let mut s = String::from(SIGN_KEY);
    for (k, v) in sorted {
        s.push_str(k);
        s.push('=');
        s.push_str(v);
    }
    s.push_str(SIGN_KEY);
    let mut h = Md5::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

async fn get_json(url: &str, cookie: &str) -> Result<Value, String> {
    let resp = CLIENT
        .get(url)
        .header("Referer", "https://www.kugou.com/")
        .header("Cookie", cookie)
        .send()
        .await
        .map_err(|e| format!("酷狗请求失败: {e}"))?;
    let text = resp.text().await.map_err(|e| format!("酷狗响应读取失败: {e}"))?;
    serde_json::from_str::<Value>(&text).map_err(|e| format!("酷狗响应解析失败: {e}"))
}

#[tauri::command]
pub async fn music_login_kugou(app: tauri::AppHandle) -> Result<(), String> {
    crate::commands::flipboard_render::open_login_window(app, PROFILE, "https://www.kugou.com/")
        .await
        .map_err(|e| format!("打开酷狗登录窗口失败: {e}"))
}

#[tauri::command]
pub async fn music_kugou_account() -> Result<OnlineAccount, String> {
    let mut acc = OnlineAccount {
        platform: "kugou".into(),
        ..Default::default()
    };
    if let Some(cookie) = kugou_cookie().await {
        if let Some(uid) = cookie_val(&cookie, "KugooID").or_else(|| cookie_val(&cookie, "kg_user_id")) {
            acc.uid = uid;
            acc.logged_in = true;
            acc.nickname = cookie_val(&cookie, "KugooNickname").unwrap_or_default();
        }
    }
    Ok(acc)
}

fn now_ms() -> String {
    chrono::Utc::now().timestamp_millis().to_string()
}

#[tauri::command]
pub async fn music_kugou_my_playlists() -> Result<Vec<OnlinePlaylist>, String> {
    let cookie = kugou_cookie().await.ok_or("未登录酷狗")?;
    let userid = cookie_val(&cookie, "KugooID").ok_or("未登录酷狗")?;
    let token = cookie_val(&cookie, "token").or_else(|| cookie_val(&cookie, "t")).unwrap_or_default();
    let params = vec![
        ("appid", "1005".to_string()),
        ("clientver", "8000".to_string()),
        ("clienttime", now_ms()),
        ("userid", userid.clone()),
        ("token", token.clone()),
        ("page", "1".to_string()),
        ("pagesize", "100".to_string()),
        ("plat", "0".to_string()),
    ];
    let sig = sign(&params);
    let qs: Vec<String> = params.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let url = format!(
        "http://mobilecdnbj.kugou.com/api/v3/user/collect/list?{}&signature={sig}",
        qs.join("&")
    );
    let v = get_json(&url, &cookie).await?;
    let list = v
        .get("data")
        .and_then(|d| d.get("info"))
        .and_then(|l| l.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(list
        .iter()
        .map(|p| OnlinePlaylist {
            id: p
                .get("global_collection_id")
                .or_else(|| p.get("listid"))
                .map(|i| i.to_string().trim_matches('"').to_string())
                .unwrap_or_default(),
            platform: "kugou".into(),
            name: p.get("name").and_then(|n| n.as_str()).unwrap_or("").into(),
            cover: p.get("pic").and_then(|n| n.as_str()).unwrap_or("").into(),
            track_count: p.get("count").and_then(|n| n.as_i64()).unwrap_or(0),
        })
        .collect())
}

#[tauri::command]
pub async fn music_kugou_playlist_tracks(id: String) -> Result<Vec<OnlineTrack>, String> {
    let cookie = kugou_cookie().await.unwrap_or_default();
    let params = vec![
        ("appid", "1005".to_string()),
        ("clientver", "8000".to_string()),
        ("clienttime", now_ms()),
        ("global_collection_id", id.clone()),
        ("page", "1".to_string()),
        ("pagesize", "300".to_string()),
        ("plat", "0".to_string()),
    ];
    let sig = sign(&params);
    let qs: Vec<String> = params.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let url = format!(
        "http://pubsongscdn.kugou.com/v2/get_other_list_file?{}&signature={sig}",
        qs.join("&")
    );
    let v = get_json(&url, &cookie).await?;
    let songs = v
        .get("data")
        .and_then(|d| d.get("info"))
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(songs.iter().map(parse_song).collect())
}

// 酷狗常把 name 存成 "歌手 - 歌名"（filename），若没有单独字段则从中拆分。
fn parse_song(v: &Value) -> OnlineTrack {
    let raw = v
        .get("name")
        .or_else(|| v.get("filename"))
        .or_else(|| v.get("audio_name"))
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .to_string();
    let mut artist = v.get("singername").and_then(|n| n.as_str()).unwrap_or("").to_string();
    let mut name = v.get("songname").and_then(|n| n.as_str()).unwrap_or("").to_string();
    if name.is_empty() {
        if let Some((a, b)) = raw.split_once(" - ") {
            if artist.is_empty() {
                artist = a.trim().to_string();
            }
            name = b.trim().to_string();
        } else {
            name = raw;
        }
    }
    let dur_ms = v
        .get("timelen")
        .or_else(|| v.get("duration"))
        .and_then(|d| d.as_i64())
        .unwrap_or(0);
    OnlineTrack {
        id: v
            .get("hash")
            .or_else(|| v.get("audio_id"))
            .map(|i| i.to_string().trim_matches('"').to_string())
            .unwrap_or_default(),
        platform: "kugou".into(),
        name,
        artists: artist,
        album: v.get("album_name").and_then(|n| n.as_str()).unwrap_or("").into(),
        duration_ms: dur_ms,
        cover: String::new(),
        playable: false, // 一律走 UNM 解灰
        fee: 0,
    }
}