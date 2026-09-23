//! 解灰引擎：给一首歌（名 + 歌手 + 时长），跨源(B站/酷我/酷狗)找一个可播的音频直链。
//!
//! UNM 0.4 API 用法（读 crate 源码确认）：
//! - `Executor::register(Cow<'static,str>, Arc<dyn Engine+Send+Sync>)`
//! - `Executor::search(&[EngineId], &Song, &Context) -> SongSearchInformation`（内部已做 pre-retrieve）
//! - `Executor::retrieve(&info, &Context) -> RetrievedSongInfo { source, url }`
//! - `Song` / `Artist` / `Context` 均 `#[non_exhaustive]` 且实现 `Default`，用「Default + 字段赋值」构造。

use std::borrow::Cow;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use unm_engine::executor::Executor;
use unm_types::{Artist, Context, SearchMode, Song};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveResult {
    pub url: String,
    pub source: String,
}

fn build_executor() -> Executor {
    let mut ex = Executor::new();
    ex.register(
        Cow::Borrowed(unm_engine_bilibili::ENGINE_ID),
        Arc::new(unm_engine_bilibili::BilibiliEngine),
    );
    ex.register(
        Cow::Borrowed(unm_engine_kuwo::ENGINE_ID),
        Arc::new(unm_engine_kuwo::KuwoEngine),
    );
    ex.register(
        Cow::Borrowed(unm_engine_kugou::ENGINE_ID),
        Arc::new(unm_engine_kugou::KugouEngine),
    );
    ex
}

/// 跨源解灰。命中返回可播直链与来源，全源未命中返回 None。
pub async fn resolve_unm(
    name: &str,
    artists: &[String],
    duration_ms: Option<i64>,
    enable_flac: bool,
) -> Option<ResolveResult> {
    let ex = build_executor();
    let engine_ids = [
        Cow::Borrowed(unm_engine_bilibili::ENGINE_ID),
        Cow::Borrowed(unm_engine_kuwo::ENGINE_ID),
        Cow::Borrowed(unm_engine_kugou::ENGINE_ID),
    ];

    let mut song = Song::default();
    song.name = name.to_string();
    song.duration = duration_ms;
    song.artists = artists
        .iter()
        .filter(|a| !a.trim().is_empty())
        .map(|a| {
            let mut ar = Artist::default();
            ar.name = a.clone();
            ar
        })
        .collect();

    let mut ctx = Context::default();
    ctx.enable_flac = enable_flac;
    ctx.search_mode = SearchMode::FastFirst;

    match ex.search(&engine_ids, &song, &ctx).await {
        Ok(info) => match ex.retrieve(&info, &ctx).await {
            Ok(r) => Some(ResolveResult {
                url: r.url,
                source: r.source.to_string(),
            }),
            Err(e) => {
                warn!("解灰 retrieve 失败 {name}: {e}");
                None
            }
        },
        Err(e) => {
            info!("解灰未命中 {name}: {e}");
            None
        }
    }
}

#[tauri::command]
pub async fn music_resolve_unm(
    name: String,
    artists: Vec<String>,
    duration_ms: Option<i64>,
    enable_flac: Option<bool>,
) -> Result<Option<ResolveResult>, String> {
    Ok(resolve_unm(&name, &artists, duration_ms, enable_flac.unwrap_or(false)).await)
}
