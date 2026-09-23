<template>
  <div class="search-view">
    <div class="sv-head">
      <h2 class="sv-title">搜索结果</h2>
      <span v-if="keyword" class="sv-kw">“{{ keyword }}”</span>
      <span v-if="!loading" class="sv-count">{{ results.length }} 首</span>
      <n-spin v-if="loading" size="small" class="sv-spin" />
    </div>

    <div v-if="!loading && !results.length" class="sv-empty">
      <div class="sv-empty-icon">🔍</div>
      <div>没有搜索结果，换个关键词试试</div>
    </div>

    <div v-else class="sv-table">
      <div class="sv-row sv-row-head">
        <div class="c-idx">#</div>
        <div class="c-name">标题</div>
        <div class="c-artist">歌手</div>
        <div class="c-album">专辑</div>
        <div class="c-dur">时长</div>
      </div>
      <div
        v-for="(t, i) in results"
        :key="t.platform + '_' + t.id"
        class="sv-row"
        :class="{ playing: isPlaying(t), resolving: resolvingId === t.id }"
        @dblclick="resolveAndPlay(t)"
      >
        <div class="c-idx">
          <span class="idx-num">{{ i + 1 }}</span>
          <button class="idx-play" @click.stop="resolveAndPlay(t)">
            <n-icon :component="PlayOutline" :size="16" />
          </button>
        </div>
        <div class="c-name">
          <span class="name-text" :title="t.name">{{ t.name }}</span>
          <span v-if="!t.playable" class="name-tag">VIP · 解灰</span>
        </div>
        <div class="c-artist" :title="t.artists">{{ t.artists }}</div>
        <div class="c-album" :title="t.album">{{ t.album }}</div>
        <div class="c-dur">{{ fmt(t.duration_ms) }}</div>
      </div>
    </div>
  </div>
</template>
<!-- SV_SCRIPT -->

<script setup>
import { ref } from "vue";
import { NIcon, NSpin, useMessage } from "naive-ui";
import { PlayOutline } from "@vicons/ionicons5";
import { createLogger } from "../../../../utils/logger";
import { useMusicStore } from "../../../../stores/useMusicStore";
import { resolveOnlineUrl, toPlayableTrack } from "./onlinePlay";

const props = defineProps({
  results: { type: Array, default: () => [] },
  keyword: { type: String, default: "" },
  loading: { type: Boolean, default: false },
});

const logger = createLogger("MusicSearchView");
const store = useMusicStore();
const message = useMessage();
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

// 取播放地址：先问原平台，拿不到(VIP/无版权)再走 UNM 跨源解灰。统一经流代理。
async function resolveAndPlay(t) {
  if (resolvingId.value) return;
  resolvingId.value = t.id;
  try {
    const r = await resolveOnlineUrl(t);
    if (!r) {
      message.warning(`未找到可播放资源：${t.name}`);
      return;
    }
    store.setQueue([toPlayableTrack(t, r.url, r.source)], 0);
  } catch (e) {
    logger.warn("解析播放地址失败", e);
    message.warning(`播放失败：${t.name}`);
  } finally {
    resolvingId.value = "";
  }
}
</script>
<!-- SV_STYLE -->

<style scoped>
.search-view { display: flex; flex-direction: column; height: 100%; padding: 20px 24px; box-sizing: border-box; overflow: hidden; }
.sv-head { display: flex; align-items: baseline; gap: 10px; margin-bottom: 16px; }
.sv-title { margin: 0; font-size: 20px; font-weight: 700; color: var(--guaxi-text-primary); }
.sv-kw { font-size: 14px; color: #a78bfa; }
.sv-count { font-size: 13px; color: var(--guaxi-text-secondary); }
.sv-spin { margin-left: 6px; }
.sv-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; color: var(--guaxi-text-secondary); }
.sv-empty-icon { font-size: 44px; }

.sv-table { flex: 1; overflow: auto; }
.sv-row {
  display: grid;
  grid-template-columns: 48px 1fr 180px 180px 60px;
  align-items: center; gap: 12px; padding: 0 10px; height: 46px;
  border-radius: 8px; font-size: 14px; color: var(--guaxi-text-primary);
}
.sv-row:not(.sv-row-head):hover { background: rgba(128,128,128,0.14); }
.sv-row-head {
  height: 34px; font-size: 12px; color: var(--guaxi-text-secondary); font-weight: 600;
  border-bottom: 1px solid var(--guaxi-border, #eee); border-radius: 0;
  position: sticky; top: 0; background: var(--guaxi-content-bg, #fff); z-index: 1;
}
.sv-row.playing .c-name .name-text { color: #a78bfa; font-weight: 600; }
.sv-row.resolving { opacity: 0.6; }
.c-idx { position: relative; display: flex; align-items: center; justify-content: center; color: var(--guaxi-text-secondary); }
.idx-play { display: none; border: none; background: transparent; color: inherit; cursor: pointer; align-items: center; }
.sv-row:hover .idx-num { display: none; }
.sv-row:hover .idx-play { display: flex; }
.c-name { display: flex; align-items: center; gap: 8px; min-width: 0; }
.name-text { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font-weight: 500; }
.name-tag {
  flex-shrink: 0; font-size: 10px; padding: 1px 6px; border-radius: 4px;
  color: #b45309; background: rgba(251,191,36,0.18); font-weight: 600;
}
.c-artist, .c-album { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--guaxi-text-secondary); }
.c-dur { text-align: right; color: var(--guaxi-text-secondary); font-variant-numeric: tabular-nums; }
</style>