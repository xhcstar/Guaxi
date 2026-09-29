//! 网易云 weapi 客户端（手写加密，避开 ncmapi 的 openssl 依赖）。
//!
//! weapi 加密流程（业界公开算法）：
//! 1. params = base64( AES-128-CBC(明文JSON, key=PRESET_KEY, iv=IV) )
//! 2. params = base64( AES-128-CBC(上一步, key=随机16位, iv=IV) )
//! 3. encSecKey = 文本型RSA( reverse(随机16位), 公钥 )  （无填充，左补零到 256 hex）
//! 加密用纯 Rust：aes + cbc(CBC/PKCS7) + num-bigint(RSA modpow)。

use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockEncryptMut, KeyIvInit};
use aes::Aes128;
use num_bigint::BigUint;
use rand::Rng;
use serde_json::Value;
use tracing::warn;

use super::{OnlineAccount, OnlinePlaylist, OnlineTrack};

type Aes128CbcEnc = cbc::Encryptor<Aes128>;

const PRESET_KEY: &[u8; 16] = b"0CoJUm6Qyw8W8jud";
const IV: &[u8; 16] = b"0102030405060708";
const BASE62: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const PUBKEY_MOD: &[u8] = b"00e0b509f6259df8642dbc35662901477df22677ec152b5ff68ace615bb7b725152b3ab17a876aea8a5aa76d2e417629ec4ee341f56135fccf695280104e0312ecbda92557c93870114af6c9d05c4f7f0c3685b7a46bee255932575cce10b424d813cfe4875d3e82047b97ddef52741d546b8e289dc6935b3ece0462db0a22b8e7";
const PUBKEY_EXP: &[u8] = b"010001";

fn aes_cbc_b64(data: &[u8], key: &[u8; 16]) -> String {
    use base64::Engine as _;
    let enc = Aes128CbcEnc::new_from_slices(key, IV).expect("AES key/iv 长度固定 16");
    let ct = enc.encrypt_padded_vec_mut::<Pkcs7>(data);
    base64::engine::general_purpose::STANDARD.encode(ct)
}

// 文本型 RSA：把反转后的密钥当大整数做 m^e mod n，输出左补零的 256 位 hex。
fn rsa_encrypt(secret: &str) -> String {
    let reversed: String = secret.chars().rev().collect();
    let m = BigUint::parse_bytes(hex::encode(reversed.as_bytes()).as_bytes(), 16)
        .unwrap_or_else(BigUint::default);
    let n = BigUint::parse_bytes(PUBKEY_MOD, 16).unwrap_or_else(BigUint::default);
    let e = BigUint::parse_bytes(PUBKEY_EXP, 16).unwrap_or_else(BigUint::default);
    let c = m.modpow(&e, &n);
    format!("{:0>256}", c.to_str_radix(16))
}

fn rand_secret() -> String {
    let mut rng = rand::rng();
    (0..16)
        .map(|_| BASE62[rng.random_range(0..BASE62.len())] as char)
        .collect()
}

/// 返回 weapi 的 (params, encSecKey)。
fn weapi_encrypt(payload: &str) -> (String, String) {
    let secret = rand_secret();
    let secret_arr: [u8; 16] = secret.as_bytes().try_into().unwrap();
    let step1 = aes_cbc_b64(payload.as_bytes(), PRESET_KEY);
    let params = aes_cbc_b64(step1.as_bytes(), &secret_arr);
    let enc_sec_key = rsa_encrypt(&secret);
    (params, enc_sec_key)
}

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36";

async fn weapi_post(path: &str, payload: Value, cookie: Option<&str>) -> Result<Value, String> {
    let (params, enc_sec_key) = weapi_encrypt(&payload.to_string());
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("https://music.163.com{path}"))
        .header("Referer", "https://music.163.com")
        .header("User-Agent", UA)
        .header("Cookie", cookie.unwrap_or("os=pc; appver=8.9.70"))
        .form(&[("params", params), ("encSecKey", enc_sec_key)])
        .send()
        .await
        .map_err(|e| format!("weapi 请求失败: {e}"))?;
    resp.json::<Value>()
        .await
        .map_err(|e| format!("weapi 响应解析失败: {e}"))
}

fn parse_song(v: &Value) -> OnlineTrack {
    let artists = v
        .get("ar")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let album = v.get("al");
    let fee = v.get("fee").and_then(|f| f.as_i64()).unwrap_or(0);
    OnlineTrack {
        id: v
            .get("id")
            .map(|i| i.to_string().trim_matches('"').to_string())
            .unwrap_or_default(),
        platform: "netease".into(),
        name: v.get("name").and_then(|n| n.as_str()).unwrap_or("").into(),
        artists,
        album: album
            .and_then(|a| a.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .into(),
        duration_ms: v.get("dt").and_then(|d| d.as_i64()).unwrap_or(0),
        cover: album
            .and_then(|a| a.get("picUrl"))
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .into(),
        // fee: 0/8=免费可播，1=VIP，4=专辑付费。非免费即需走解灰。
        playable: fee == 0 || fee == 8,
        fee,
    }
}

#[tauri::command]
pub async fn music_netease_search(
    keyword: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<OnlineTrack>, String> {
    let payload = serde_json::json!({
        "s": keyword,
        "type": 1,
        "limit": limit.unwrap_or(30),
        "offset": offset.unwrap_or(0),
        "total": true,
        "csrf_token": "",
    });
    let v = weapi_post("/weapi/cloudsearch/get/web", payload, None).await?;
    let songs = v
        .get("result")
        .and_then(|r| r.get("songs"))
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(songs.iter().map(parse_song).collect())
}

#[tauri::command]
pub async fn music_netease_song_url(
    id: String,
    br: Option<i64>,
    cookie: Option<String>,
) -> Result<Option<String>, String> {
    let payload = serde_json::json!({
        "ids": format!("[{id}]"),
        "br": br.unwrap_or(320_000),
        "csrf_token": "",
    });
    let v = weapi_post(
        "/weapi/song/enhance/player/url",
        payload,
        cookie.as_deref(),
    )
    .await?;
    let url = v
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|a| a.first())
        .and_then(|d| d.get("url"))
        .and_then(|u| u.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    if url.is_none() {
        warn!("网易云取播放地址为空（可能 VIP/无版权），id={id}");
    }
    Ok(url)
}

// ── 登录 + 我的歌单 ────────────────────────────────────────────────
//
// Cookie 复用 flipboard_render 的持久化浏览器 profile：登录一次后 cookie 落盘到
// 该 profile 的 user_data_dir，之后 cookie_header_for_url 直接读出，无需自建加密存储。

const NETEASE_PROFILE: &str = "music_netease";

async fn netease_cookie() -> Option<String> {
    crate::commands::flipboard_render::cookie_header_for_url("https://music.163.com", NETEASE_PROFILE)
        .await
}

/// 打开可见登录窗口，用户扫码/输密码登录一次；cookie 落盘该 profile。
#[tauri::command]
pub async fn music_login_netease(app: tauri::AppHandle) -> Result<(), String> {
    crate::commands::flipboard_render::open_login_window(app, NETEASE_PROFILE, "https://music.163.com/")
        .await
        .map_err(|e| format!("打开网易云登录窗口失败: {e}"))
}

/// 读取当前登录账号；未登录返回 logged_in=false。
#[tauri::command]
pub async fn music_netease_account() -> Result<OnlineAccount, String> {
    let cookie = netease_cookie().await;
    let mut acc = OnlineAccount {
        platform: "netease".into(),
        ..Default::default()
    };
    let Some(cookie) = cookie else { return Ok(acc) };
    let v = weapi_post("/weapi/w/nuser/account/get", serde_json::json!({}), Some(&cookie)).await?;
    if let Some(id) = v.get("account").and_then(|a| a.get("id")).and_then(|i| i.as_i64()) {
        acc.uid = id.to_string();
        acc.logged_in = id != 0;
        if let Some(p) = v.get("profile") {
            acc.nickname = p.get("nickname").and_then(|n| n.as_str()).unwrap_or("").into();
            acc.avatar = p.get("avatarUrl").and_then(|n| n.as_str()).unwrap_or("").into();
        }
    }
    Ok(acc)
}

/// 拉取「我的歌单 + 收藏的歌单」。
#[tauri::command]
pub async fn music_netease_my_playlists() -> Result<Vec<OnlinePlaylist>, String> {
    let cookie = netease_cookie().await.ok_or("未登录网易云")?;
    let acc = music_netease_account().await?;
    if !acc.logged_in {
        return Err("未登录网易云".into());
    }
    let payload = serde_json::json!({ "uid": acc.uid, "limit": 1000, "offset": 0, "csrf_token": "" });
    let v = weapi_post("/weapi/user/playlist", payload, Some(&cookie)).await?;
    let list = v.get("playlist").and_then(|p| p.as_array()).cloned().unwrap_or_default();
    Ok(list
        .iter()
        .map(|p| OnlinePlaylist {
            id: p.get("id").map(|i| i.to_string().trim_matches('"').to_string()).unwrap_or_default(),
            platform: "netease".into(),
            name: p.get("name").and_then(|n| n.as_str()).unwrap_or("").into(),
            cover: p.get("coverImgUrl").and_then(|n| n.as_str()).unwrap_or("").into(),
            track_count: p.get("trackCount").and_then(|n| n.as_i64()).unwrap_or(0),
        })
        .collect())
}

/// 首页「推荐歌单」（无需登录；登录后会带上个性化）。
/// 走 /weapi/personalized/playlist，返回 result 数组。
#[tauri::command]
pub async fn music_netease_recommend_playlists() -> Result<Vec<OnlinePlaylist>, String> {
    let cookie = netease_cookie().await;
    let payload = serde_json::json!({ "limit": 30, "total": true, "n": 1000, "csrf_token": "" });
    let v = weapi_post("/weapi/personalized/playlist", payload, cookie.as_deref()).await?;
    let list = v.get("result").and_then(|p| p.as_array()).cloned().unwrap_or_default();
    Ok(list
        .iter()
        .map(|p| OnlinePlaylist {
            id: p.get("id").map(|i| i.to_string().trim_matches('"').to_string()).unwrap_or_default(),
            platform: "netease".into(),
            name: p.get("name").and_then(|n| n.as_str()).unwrap_or("").into(),
            cover: p.get("picUrl").and_then(|n| n.as_str()).unwrap_or("").into(),
            // 推荐歌单接口用 playCount（播放量）而非曲目数；没有曲目数时先置 0，进入详情再取真实曲目。
            track_count: p
                .get("trackCount")
                .or_else(|| p.get("playCount"))
                .and_then(|n| n.as_i64())
                .unwrap_or(0),
        })
        .collect())
}

/// 读取某歌单的曲目列表。
#[tauri::command]
pub async fn music_netease_playlist_tracks(id: String) -> Result<Vec<OnlineTrack>, String> {
    let cookie = netease_cookie().await;
    let payload = serde_json::json!({ "id": id, "n": 1000, "s": 8, "csrf_token": "" });
    let v = weapi_post("/weapi/v6/playlist/detail", payload, cookie.as_deref()).await?;
    let tracks = v
        .get("playlist")
        .and_then(|p| p.get("tracks"))
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(tracks.iter().map(parse_song).collect())
}