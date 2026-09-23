// 在线曲目的取址与播放：先问原平台(网易云)，拿不到(VIP/无版权)再走 UNM 跨源解灰，
// 最终 URL 统一经后端流代理(补 Referer/UA、支持 Range)喂给 <audio>。
// SearchView 与 PlatformPlaylistsView 共用，避免重复。
import { invoke } from "@tauri-apps/api/core";

const STREAM_PROXY = "http://127.0.0.1:8888/api/music/stream?url=";

// 解析出可播放地址。命中返回 { url(已代理), source }，全部失败返回 null。
export async function resolveOnlineUrl(t) {
  let url = null;
  let source = t.platform;
  try {
    url = await invoke("music_netease_song_url", { id: t.id, br: 320000 });
  } catch (_) {
    /* 转解灰 */
  }
  if (!url) {
    try {
      const artists = t.artists ? t.artists.split(",").map((s) => s.trim()) : [];
      const r = await invoke("music_resolve_unm", {
        name: t.name,
        artists,
        durationMs: t.duration_ms,
        enableFlac: false,
      });
      if (r && r.url) {
        url = r.url;
        source = r.source;
      }
    } catch (_) {
      /* ignore */
    }
  }
  if (!url) return null;
  return { url: `${STREAM_PROXY}${encodeURIComponent(url)}`, source };
}

// 把后端返回的 OnlineTrack + 解析结果，拼成播放器队列用的 track 形状。
export function toPlayableTrack(t, url, source) {
  return {
    id: `${t.platform}_${t.id}`,
    title: t.name,
    artist: t.artists,
    album: t.album,
    duration: (t.duration_ms || 0) / 1000,
    cover: t.cover,
    url,
    source,
  };
}
