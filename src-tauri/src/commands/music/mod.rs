//! 音乐播放器后端。
//!
//! 分层：
//! - `local`   本地曲库（lofty 读标签 + SQLite）。
//! - `netease` 网易云 weapi 客户端（搜索 / 歌曲详情 / 取播放地址）。手写 weapi，避开 ncmapi 的 openssl 依赖。
//! - `resolve` 解灰引擎：原平台无权限/VIP 时，用 UnblockNeteaseMusic 的 Rust 引擎跨源(B站/酷我/酷狗)找可播副本。
//! - `stream`  流代理：给带防盗链的源补 Referer/UA，边下边喂前端 `<audio>`。

pub mod local;
pub mod netease;
pub mod qq;
pub mod kugou;
pub mod resolve;
pub mod stream;

use serde::{Deserialize, Serialize};

/// 统一的在线曲目形状，前端与各 provider 共用。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OnlineTrack {
    pub id: String,          // 平台内的歌曲 id
    pub platform: String,    // "netease" / "qq" / "kugou" ...
    pub name: String,
    pub artists: String,     // 逗号拼接的歌手名
    pub album: String,
    pub duration_ms: i64,
    pub cover: String,       // 封面 URL
    pub playable: bool,      // 原平台是否可直接播（false 时前端需走解灰）
    pub fee: i64,            // 网易云 fee 字段：1=VIP 8=免费 4=专辑付费 等
}

/// 平台歌单（列表项）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OnlinePlaylist {
    pub id: String,
    pub platform: String,
    pub name: String,
    pub cover: String,
    pub track_count: i64,
}

/// 平台账号信息。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OnlineAccount {
    pub platform: String,
    pub uid: String,
    pub nickname: String,
    pub avatar: String,
    pub logged_in: bool,
}
