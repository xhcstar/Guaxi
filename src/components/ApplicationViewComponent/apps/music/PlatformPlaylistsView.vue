<template>
  <div class="pp-view">
    <!-- 平台切换 -->
    <div class="pp-tabs">
      <button
        v-for="p in PLATFORMS"
        :key="p.key"
        class="pp-tab"
        :class="{ active: platform === p.key }"
        @click="switchPlatform(p.key)"
      >{{ p.label }}</button>
    </div>

    <!-- 未登录 -->
    <div v-if="!account.logged_in" class="pp-login">
      <div class="pp-login-icon">☁️</div>
      <div class="pp-login-title">登录{{ platformLabel }}，导入你的歌单</div>
      <div class="pp-login-actions">
        <n-button type="primary" :loading="loggingIn" @click="login">打开登录窗口</n-button>
        <n-button quaternary :loading="loading" @click="refreshAccount">我已登录，刷新</n-button>
      </div>
      <div class="pp-login-hint">在弹出的{{ platformLabel }}窗口里登录后，回来点“刷新”。</div>
    </div>

    <!-- 已登录 -->
    <template v-else>
      <div class="pp-head">
        <img v-if="account.avatar" class="pp-avatar" :src="account.avatar" alt="" />
        <div class="pp-user">{{ account.nickname || platformLabel + "用户" }}</div>
        <n-button size="tiny" quaternary :loading="loading" @click="loadPlaylists">刷新歌单</n-button>
      </div>

      <!-- 歌单网格 -->
      <div v-if="!selected" class="pp-grid">
        <div v-for="p in playlists" :key="p.id" class="pp-card" @click="openPlaylist(p)">
          <div class="pp-cover">
            <img v-if="p.cover" :src="p.cover + '?param=200y200'" alt="" />
            <span v-else>🎵</span>
          </div>
          <div class="pp-name" :title="p.name">{{ p.name }}</div>
          <div class="pp-count">{{ p.track_count }} 首</div>
        </div>
        <div v-if="!loading && !playlists.length" class="pp-empty">没有歌单</div>
      </div>

      <!-- 歌单详情 -->
      <div v-else class="pp-detail">
        <div class="pp-detail-head">
          <n-button size="small" quaternary @click="selected = null">← 返回</n-button>
          <span class="pp-detail-name">{{ selected.name }}</span>
          <span class="pp-detail-count">{{ tracks.length }} 首</span>
          <n-spin v-if="tracksLoading" size="small" />
        </div>
        <div class="pp-tracks">
          <div
            v-for="(t, i) in tracks"
            :key="t.platform + '_' + t.id"
            class="pp-row"
            :class="{ playing: isPlaying(t), resolving: resolvingId === t.id }"
            @dblclick="playTrack(t)"
          >
            <div class="c-idx">
              <span class="idx-num">{{ i + 1 }}</span>
              <button class="idx-play" @click.stop="playTrack(t)"><n-icon :component="PlayOutline" :size="16" /></button>
            </div>
            <div class="c-name">
              <span class="name-text" :title="t.name">{{ t.name }}</span>
              <span v-if="!t.playable" class="name-tag">VIP · 解灰</span>
            </div>
            <div class="c-artist" :title="t.artists">{{ t.artists }}</div>
            <div class="c-dur">{{ fmt(t.duration_ms) }}</div>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>
<!-- PP_SCRIPT -->

<script setup>
import { ref, computed, onMounted } from "vue";
import { NButton, NIcon, NSpin, useMessage } from "naive-ui";
import { PlayOutline } from "@vicons/ionicons5";
import { invoke } from "@tauri-apps/api/core";
import { createLogger } from "../../../../utils/logger";
import { useMusicStore } from "../../../../stores/useMusicStore";
import { resolveOnlineUrl, toPlayableTrack } from "./onlinePlay";

const logger = createLogger("MusicPlatformPlaylists");
const store = useMusicStore();
const message = useMessage();

// 各平台对应的后端命令名。切换平台即切换整套命令。
const PLATFORMS = [
  { key: "netease", label: "网易云", login: "music_login_netease", account: "music_netease_account", playlists: "music_netease_my_playlists", tracks: "music_netease_playlist_tracks" },
  { key: "qq", label: "QQ音乐", login: "music_login_qq", account: "music_qq_account", playlists: "music_qq_my_playlists", tracks: "music_qq_playlist_tracks" },
  { key: "kugou", label: "酷狗", login: "music_login_kugou", account: "music_kugou_account", playlists: "music_kugou_my_playlists", tracks: "music_kugou_playlist_tracks" },
];

const platform = ref("netease");
const cmds = computed(() => PLATFORMS.find((p) => p.key === platform.value));
const platformLabel = computed(() => cmds.value?.label || "");

const account = ref({ logged_in: false, nickname: "", avatar: "", uid: "" });
const playlists = ref([]);
const selected = ref(null);
const tracks = ref([]);
const loading = ref(false);
const tracksLoading = ref(false);
const loggingIn = ref(false);
const resolvingId = ref("");

function isPlaying(t) {
  return store.currentTrack?.id === `${t.platform}_${t.id}`;
}
function fmt(ms) {
  const sec = (ms || 0) / 1000;
  if (!sec) return "-";
  const s = Math.floor(sec % 60);
  const m = Math.floor(sec / 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}

function switchPlatform(key) {
  if (platform.value === key) return;
  platform.value = key;
  account.value = { logged_in: false, nickname: "", avatar: "", uid: "" };
  playlists.value = [];
  selected.value = null;
  tracks.value = [];
  refreshAccount();
}

async function refreshAccount() {
  loading.value = true;
  try {
    account.value = await invoke(cmds.value.account);
    if (account.value.logged_in) await loadPlaylists();
  } catch (e) {
    logger.warn("读取账号失败", e);
  } finally {
    loading.value = false;
  }
}

async function login() {
  loggingIn.value = true;
  try {
    await invoke(cmds.value.login);
    message.info("请在弹出的窗口完成登录，然后点“我已登录，刷新”");
  } catch (e) {
    logger.error("打开登录窗口失败", e);
    message.error("打开登录窗口失败");
  } finally {
    loggingIn.value = false;
  }
}

async function loadPlaylists() {
  loading.value = true;
  try {
    playlists.value = (await invoke(cmds.value.playlists)) || [];
  } catch (e) {
    logger.warn("加载歌单失败", e);
    message.warning("加载歌单失败，可能未登录");
  } finally {
    loading.value = false;
  }
}

async function openPlaylist(p) {
  selected.value = p;
  tracks.value = [];
  tracksLoading.value = true;
  try {
    tracks.value = (await invoke(cmds.value.tracks, { id: p.id })) || [];
  } catch (e) {
    logger.warn("加载歌单曲目失败", e);
    message.warning("加载歌单曲目失败");
  } finally {
    tracksLoading.value = false;
  }
}

async function playTrack(t) {
  if (resolvingId.value) return;
  resolvingId.value = t.id;
  try {
    const r = await resolveOnlineUrl(t);
    if (!r) {
      message.warning(`未找到可播放资源：${t.name}`);
      return;
    }
    store.setQueue([toPlayableTrack(t, r.url, r.source)], 0);
  } finally {
    resolvingId.value = "";
  }
}

onMounted(refreshAccount);
</script>
<!-- PP_STYLE -->

<style scoped>
.pp-view { height: 100%; display: flex; flex-direction: column; padding: 20px 24px; box-sizing: border-box; overflow: hidden; }
.pp-tabs { display: flex; gap: 6px; margin-bottom: 16px; flex-shrink: 0; }
.pp-tab {
  border: 1px solid var(--guaxi-border); background: transparent; cursor: pointer;
  padding: 5px 14px; border-radius: 999px; font-size: 13px; font-family: inherit;
  color: var(--guaxi-text-secondary);
}
.pp-tab:hover { background: rgba(128,128,128,0.14); }
.pp-tab.active { background: linear-gradient(135deg, rgba(102,126,234,0.18), rgba(118,75,162,0.18)); color: #7c5cd6; border-color: transparent; font-weight: 600; }
.pp-login { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 14px; color: var(--guaxi-text-secondary); }
.pp-login-icon { font-size: 52px; }
.pp-login-title { font-size: 16px; font-weight: 600; color: var(--guaxi-text-primary); }
.pp-login-actions { display: flex; gap: 10px; }
.pp-login-hint { font-size: 13px; }

.pp-head { display: flex; align-items: center; gap: 10px; margin-bottom: 16px; }
.pp-avatar { width: 32px; height: 32px; border-radius: 50%; object-fit: cover; }
.pp-user { font-size: 15px; font-weight: 700; color: var(--guaxi-text-primary); }

.pp-grid { flex: 1; overflow: auto; display: grid; grid-template-columns: repeat(auto-fill, minmax(130px, 1fr)); gap: 16px; align-content: start; }
.pp-card { cursor: pointer; }
.pp-cover { aspect-ratio: 1; border-radius: 10px; overflow: hidden; background: linear-gradient(135deg,#667eea,#764ba2); display: flex; align-items: center; justify-content: center; font-size: 28px; color: #fff; }
.pp-cover img { width: 100%; height: 100%; object-fit: cover; }
.pp-card:hover .pp-cover { filter: brightness(1.05); }
.pp-name { margin-top: 6px; font-size: 13px; font-weight: 500; color: var(--guaxi-text-primary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.pp-count { font-size: 12px; color: var(--guaxi-text-secondary); }
.pp-empty { color: var(--guaxi-text-secondary); grid-column: 1 / -1; text-align: center; padding: 40px 0; }

.pp-detail { flex: 1; display: flex; flex-direction: column; min-height: 0; }
.pp-detail-head { display: flex; align-items: center; gap: 10px; margin-bottom: 12px; }
.pp-detail-name { font-size: 16px; font-weight: 700; color: var(--guaxi-text-primary); }
.pp-detail-count { font-size: 13px; color: var(--guaxi-text-secondary); }
.pp-tracks { flex: 1; overflow: auto; }
.pp-row { display: grid; grid-template-columns: 44px 1fr 180px 56px; align-items: center; gap: 12px; padding: 0 10px; height: 44px; border-radius: 8px; font-size: 14px; color: var(--guaxi-text-primary); }
.pp-row:hover { background: rgba(128,128,128,0.14); }
.pp-row.playing .name-text { color: #a78bfa; font-weight: 600; }
.pp-row.resolving { opacity: 0.6; }
.c-idx { position: relative; display: flex; align-items: center; justify-content: center; color: var(--guaxi-text-secondary); }
.idx-play { display: none; border: none; background: transparent; color: inherit; cursor: pointer; align-items: center; }
.pp-row:hover .idx-num { display: none; }
.pp-row:hover .idx-play { display: flex; }
.c-name { display: flex; align-items: center; gap: 8px; min-width: 0; }
.name-text { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font-weight: 500; }
.name-tag { flex-shrink: 0; font-size: 10px; padding: 1px 6px; border-radius: 4px; color: #b45309; background: rgba(251,191,36,0.18); font-weight: 600; }
.c-artist { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--guaxi-text-secondary); }
.c-dur { text-align: right; color: var(--guaxi-text-secondary); font-variant-numeric: tabular-nums; }
</style>