//! QQ 音乐账号 provider：登录 + 我的歌单 + 歌单曲目。
//!
//! Cookie 复用 flipboard_render 的持久化浏览器 profile(同网易云做法)。
//! 用无签名的 fcg 接口(带 Referer y.qq.com 即可),不依赖逆向签名。
//! 播放仍走 UNM 解灰(QQ 本身也是解灰源之一),这里只负责把「你账号里的歌单」拉进来。

use once_cell::sync::Lazy;
use serde_json::Value;

use super::{OnlineAccount, OnlinePlaylist, OnlineTrack};

const PROFILE: &str = "music_qq";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36";

static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent(UA)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

async fn qq_cookie() -> Option<String> {
    crate::commands::flipboard_render::cookie_header_for_url("https://y.qq.com", PROFILE).await
}

// 从 Cookie 里取 QQ 号(uin)。cookie 里形如 `uin=o0123456789` 或纯数字。
fn uin_from_cookie(cookie: &str) -> Option<String> {
    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix("uin=") {
            let digits: String = v.chars().filter(|c| c.is_ascii_digit()).collect();
            let trimmed = digits.trim_start_matches('0');
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

async fn get_json(url: &str, cookie: &str) -> Result<Value, String> {
    let resp = CLIENT
        .get(url)
        .header("Referer", "https://y.qq.com/")
        .header("Cookie", cookie)
        .send()
        .await
        .map_err(|e| format!("QQ 请求失败: {e}"))?;
    let text = resp.text().await.map_err(|e| format!("QQ 响应读取失败: {e}"))?;
    serde_json::from_str::<Value>(&text).map_err(|e| format!("QQ 响应解析失败: {e}"))
}

#[tauri::command]
pub async fn music_login_qq() -> Result<(), String> {
    crate::commands::flipboard_render::open_login_window(PROFILE, "https://y.qq.com/")
        .await
        .map_err(|e| format!("打开 QQ 音乐登录窗口失败: {e}"))
}

#[tauri::command]
pub async fn music_qq_account() -> Result<OnlineAccount, String> {
    let mut acc = OnlineAccount {
        platform: "qq".into(),
        ..Default::default()
    };
    if let Some(cookie) = qq_cookie().await {
        if let Some(uin) = uin_from_cookie(&cookie) {
            acc.uid = uin;
            acc.logged_in = true;
        }
    }
    Ok(acc)
}

#[tauri::command]
pub async fn music_qq_my_playlists() -> Result<Vec<OnlinePlaylist>, String> {
    let cookie = qq_cookie().await.ok_or("未登录 QQ 音乐")?;
    let uin = uin_from_cookie(&cookie).ok_or("未登录 QQ 音乐")?;
    let url = format!(
        "https://c.y.qq.com/rsc/fcgi-bin/fcg_user_created_diss?hostuin={uin}&sin=0&size=200&g_tk=5381&loginUin={uin}&format=json&inCharset=utf-8&outCharset=utf-8&platform=yqq.json"
    );
    let v = get_json(&url, &cookie).await?;
    let list = v
        .get("data")
        .and_then(|d| d.get("disslist"))
        .and_then(|l| l.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(list
        .iter()
        .map(|p| OnlinePlaylist {
            id: p.get("dissid").map(|i| i.to_string().trim_matches('"').to_string()).unwrap_or_default(),
            platform: "qq".into(),
            name: p.get("title").and_then(|n| n.as_str()).unwrap_or("").into(),
            cover: p.get("picurl").and_then(|n| n.as_str()).unwrap_or("").into(),
            track_count: p.get("song_cnt").and_then(|n| n.as_i64()).unwrap_or(0),
        })
        .collect())
}

#[tauri::command]
pub async fn music_qq_playlist_tracks(id: String) -> Result<Vec<OnlineTrack>, String> {
    let cookie = qq_cookie().await.unwrap_or_default();
    let url = format!(
        "https://c.y.qq.com/qzone/fcg-bin/fcg_ucc_getcdinfo_byids_cp.fcg?type=1&json=1&utf8=1&onlysong=0&disstid={id}&format=json&g_tk=5381&loginUin=0&hostUin=0&inCharset=utf8&outCharset=utf-8&platform=yqq.json"
    );
    let v = get_json(&url, &cookie).await?;
    let songs = v
        .get("cdlist")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|cd| cd.get("songlist"))
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(songs.iter().map(parse_song).collect())
}

fn parse_song(v: &Value) -> OnlineTrack {
    let artists = v
        .get("singer")
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    // QQ 用秒(interval)，统一转毫秒。
    let dur_ms = v.get("interval").and_then(|d| d.as_i64()).unwrap_or(0) * 1000;
    let id = v
        .get("songmid")
        .and_then(|m| m.as_str())
        .map(|s| s.to_string())
        .or_else(|| v.get("songid").map(|i| i.to_string().trim_matches('"').to_string()))
        .unwrap_or_default();
    OnlineTrack {
        id,
        platform: "qq".into(),
        name: v.get("songname").and_then(|n| n.as_str()).unwrap_or("").into(),
        artists,
        album: v.get("albumname").and_then(|n| n.as_str()).unwrap_or("").into(),
        duration_ms: dur_ms,
        cover: String::new(),
        // QQ 曲目一律走 UNM 解灰(无原平台直链命令)，标为需解灰。
        playable: false,
        fee: 0,
    }
}