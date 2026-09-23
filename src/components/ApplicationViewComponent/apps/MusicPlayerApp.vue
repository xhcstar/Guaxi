<template>
  <div class="music-app" :class="{ dark: isDark }">
    <div class="music-body">
      <!-- 左侧导航 -->
      <aside class="music-nav">
        <div class="nav-brand">
          <div class="nav-brand-logo">🎵</div>
          <div class="nav-brand-name">音乐</div>
        </div>

        <div class="nav-scroll">
          <div v-for="sec in navSections" :key="sec.title" class="nav-section">
            <div class="nav-section-title">{{ sec.title }}</div>
            <div
              v-for="item in sec.items"
              :key="item.key"
              class="nav-item"
              :class="{ active: activeView === item.key }"
              @click="activeView = item.key"
            >
              <n-icon class="nav-item-icon" :component="item.icon" :size="18" />
              <span class="nav-item-label">{{ item.label }}</span>
            </div>
          </div>
        </div>
      </aside>

      <!-- 右侧主区 -->
      <div class="music-main">
        <header class="music-top">
          <div class="top-search">
            <n-icon :component="SearchOutline" :size="16" class="top-search-icon" />
            <input
              v-model="searchKeyword"
              class="top-search-input"
              placeholder="搜索音乐、歌手、歌单，回车搜索"
              @keyup.enter="onSearch"
            />
          </div>
          <div class="top-account">
            <n-button size="small" quaternary @click="onLoginPlaceholder">
              <template #icon><n-icon :component="PersonCircleOutline" /></template>
              登录平台
            </n-button>
          </div>
        </header>

        <section class="music-content">
          <LibraryView v-if="activeView === 'local'" :tracks="localTracks" @import="importLocal" @import-dir="importDir" />
          <SearchView v-else-if="activeView === 'search'" :results="searchResults" :keyword="searchSubmitted" :loading="searchLoading" />
          <PlatformPlaylistsView v-else-if="activeView === 'platform-playlists'" />
          <div v-else class="view-placeholder">
            <div class="ph-icon">{{ placeholderInfo.icon }}</div>
            <div class="ph-title">{{ placeholderInfo.title }}</div>
            <div class="ph-desc">{{ placeholderInfo.desc }}</div>
          </div>

          <QueueDrawer />
          <NowPlaying />
        </section>

        <PlayerBar />
      </div>
    </div>
  </div>
</template>
<!-- MUSIC_SCRIPT -->

<script setup>
import { ref, computed, onMounted } from "vue";
import { NButton, NIcon } from "naive-ui";
import {
  SearchOutline, PersonCircleOutline,
  MusicalNotesOutline, TimeOutline, HeartOutline,
  RadioOutline, CompassOutline, AlbumsOutline, CloudOutline,
} from "@vicons/ionicons5";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { createLogger } from "../../../utils/logger";
import { useSettingsStore } from "../../../stores/settingsStore";
import LibraryView from "./music/LibraryView.vue";
import SearchView from "./music/SearchView.vue";
import PlatformPlaylistsView from "./music/PlatformPlaylistsView.vue";
import PlayerBar from "./music/PlayerBar.vue";
import QueueDrawer from "./music/QueueDrawer.vue";
import NowPlaying from "./music/NowPlaying.vue";

const logger = createLogger("MusicPlayerApp");
const settingsStore = useSettingsStore();
const isDark = computed(() => settingsStore.appearanceTheme === "dark");

const activeView = ref("local");
const searchKeyword = ref("");
const searchSubmitted = ref("");
const searchResults = ref([]);
const searchLoading = ref(false);
const localTracks = ref([]);

const navSections = [
  {
    title: "在线音乐",
    items: [
      { key: "search", label: "搜索", icon: SearchOutline },
      { key: "discover", label: "发现推荐", icon: CompassOutline },
      { key: "radio", label: "电台", icon: RadioOutline },
    ],
  },
  {
    title: "我的音乐",
    items: [
      { key: "local", label: "本地曲库", icon: MusicalNotesOutline },
      { key: "recent", label: "最近播放", icon: TimeOutline },
      { key: "favorites", label: "我的收藏", icon: HeartOutline },
    ],
  },
  {
    title: "歌单",
    items: [
      { key: "my-playlists", label: "创建的歌单", icon: AlbumsOutline },
      { key: "platform-playlists", label: "平台歌单", icon: CloudOutline },
    ],
  },
];

const placeholderMap = {
  search: { icon: "🔍", title: "在线搜索", desc: "全网搜索 + VIP/无版权跨源解灰，将在阶段三接入。" },
  discover: { icon: "🧭", title: "发现推荐", desc: "登录平台后展示个性化推荐，敬请期待。" },
  radio: { icon: "📻", title: "电台", desc: "电台功能规划中。" },
  recent: { icon: "🕘", title: "最近播放", desc: "播放历史将在本地库落地后展示。" },
  favorites: { icon: "❤️", title: "我的收藏", desc: "收藏功能将随歌曲管理一并上线。" },
  "my-playlists": { icon: "📚", title: "创建的歌单", desc: "本地歌单管理即将上线。" },
  "platform-playlists": { icon: "☁️", title: "平台歌单", desc: "登录网易云/QQ/酷狗后可导入你的歌单（阶段三/四）。" },
};
const placeholderInfo = computed(() => placeholderMap[activeView.value] || { icon: "🎵", title: "即将上线", desc: "" });

function mapTrack(t) {
  return {
    id: t.id,
    title: t.title,
    artist: t.artist,
    album: t.album,
    duration: t.duration,
    cover: t.cover_path ? convertFileSrc(t.cover_path) : "",
    url: convertFileSrc(t.path),
    source: "local",
    path: t.path,
  };
}

function mergeTracks(list) {
  const map = new Map(localTracks.value.map((t) => [t.path, t]));
  for (const t of list) map.set(t.path, mapTrack(t));
  localTracks.value = [...map.values()];
}

async function loadLibrary() {
  try {
    const rows = await invoke("music_local_list");
    localTracks.value = (rows || []).map(mapTrack);
  } catch (err) {
    logger.error("加载本地曲库失败", err);
  }
}

async function importLocal() {
  try {
    const selected = await open({
      multiple: true,
      filters: [{ name: "音频文件", extensions: ["mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "wma", "ape"] }],
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    const rows = await invoke("music_import_files", { paths });
    mergeTracks(rows || []);
    logger.info("导入本地音乐", { count: (rows || []).length });
  } catch (err) {
    logger.error("导入本地音乐失败", err);
  }
}

async function importDir() {
  try {
    const dir = await open({ directory: true, multiple: false });
    if (!dir) return;
    const rows = await invoke("music_import_dir", { dirPath: dir });
    mergeTracks(rows || []);
    logger.info("导入音乐文件夹", { dir, count: (rows || []).length });
  } catch (err) {
    logger.error("导入音乐文件夹失败", err);
  }
}

onMounted(loadLibrary);

async function onSearch() {
  const kw = searchKeyword.value.trim();
  if (!kw) return;
  activeView.value = "search";
  searchSubmitted.value = kw;
  searchLoading.value = true;
  try {
    const rows = await invoke("music_netease_search", { keyword: kw, limit: 50, offset: 0 });
    searchResults.value = rows || [];
  } catch (err) {
    logger.error("在线搜索失败", err);
    searchResults.value = [];
  } finally {
    searchLoading.value = false;
  }
}
function onLoginPlaceholder() {
  activeView.value = "platform-playlists";
}
</script>
<!-- MUSIC_STYLE -->

<style scoped>
.music-app {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--guaxi-content-bg, #f7f8fa);
  color: var(--guaxi-text-primary);
  overflow: hidden;
}
.music-body { flex: 1; display: flex; min-height: 0; }

/* 左侧导航 */
.music-nav {
  width: 210px;
  flex-shrink: 0;
  background: var(--guaxi-panel-bg, #fff);
  border-right: 1px solid var(--guaxi-border, #e5e7eb);
  display: flex;
  flex-direction: column;
}
.nav-brand { display: flex; align-items: center; gap: 10px; padding: 18px 18px 10px; }
.nav-brand-logo { font-size: 22px; }
.nav-brand-name { font-size: 17px; font-weight: 700; }
.nav-scroll { flex: 1; overflow: auto; padding: 8px 10px; }
.nav-section { margin-bottom: 14px; }
.nav-section-title { font-size: 12px; color: var(--guaxi-text-secondary); font-weight: 600; padding: 6px 10px; }
.nav-item {
  display: flex; align-items: center; gap: 10px;
  padding: 8px 10px; border-radius: 8px; cursor: pointer;
  font-size: 14px; color: var(--guaxi-text-primary);
}
.nav-item:hover { background: rgba(128,128,128,0.14); }
.nav-item.active { background: linear-gradient(135deg, rgba(102,126,234,0.16), rgba(118,75,162,0.16)); color: #7c5cd6; font-weight: 600; }
.nav-item-icon { color: inherit; }

/* 右侧主区 */
.music-main { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.music-top {
  height: 56px; flex-shrink: 0;
  display: flex; align-items: center; justify-content: space-between;
  padding: 0 20px; gap: 16px;
  border-bottom: 1px solid var(--guaxi-border, #eee);
}
.top-search {
  display: flex; align-items: center; gap: 8px;
  background: rgba(128,128,128,0.14);
  border-radius: 999px; padding: 7px 14px; width: 360px; max-width: 46vw;
}
.top-search-icon { color: var(--guaxi-text-secondary); flex-shrink: 0; }
.top-search-input { border: none; background: transparent; outline: none; flex: 1; font-family: inherit; font-size: 14px; color: var(--guaxi-text-primary); }
.top-search-input::placeholder { color: var(--guaxi-text-secondary); }

.music-content { flex: 1; position: relative; min-height: 0; overflow: hidden; }
.view-placeholder {
  height: 100%; display: flex; flex-direction: column;
  align-items: center; justify-content: center; gap: 10px; color: var(--guaxi-text-secondary);
}
.ph-icon { font-size: 52px; }
.ph-title { font-size: 18px; font-weight: 600; color: var(--guaxi-text-primary); }
.ph-desc { font-size: 14px; max-width: 420px; text-align: center; }
</style>