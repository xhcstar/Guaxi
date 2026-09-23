<template>
  <div class="lib-view">
    <div class="lib-head">
      <div class="lib-head-left">
        <h2 class="lib-title">本地曲库</h2>
        <span class="lib-count">{{ tracks.length }} 首</span>
      </div>
      <div class="lib-head-actions">
        <n-button size="small" type="primary" :disabled="!tracks.length" @click="playAll">
          <template #icon><n-icon :component="PlayOutline" /></template>
          播放全部
        </n-button>
        <n-button size="small" @click="$emit('import')">
          <template #icon><n-icon :component="AddOutline" /></template>
          导入文件
        </n-button>
        <n-button size="small" @click="$emit('import-dir')">
          <template #icon><n-icon :component="FolderOpenOutline" /></template>
          导入文件夹
        </n-button>
      </div>
    </div>

    <div v-if="!tracks.length" class="lib-empty">
      <div class="lib-empty-icon">🎧</div>
      <div class="lib-empty-text">还没有本地音乐</div>
      <n-button size="small" type="primary" @click="$emit('import')">选择音频文件导入</n-button>
    </div>

    <div v-else class="lib-table">
      <div class="lib-row lib-row-head">
        <div class="c-idx">#</div>
        <div class="c-title">标题</div>
        <div class="c-artist">歌手</div>
        <div class="c-album">专辑</div>
        <div class="c-dur">时长</div>
      </div>
      <div
        v-for="(t, i) in tracks"
        :key="t.id"
        class="lib-row"
        :class="{ playing: isPlaying(t) }"
        @dblclick="play(i)"
      >
        <div class="c-idx">
          <span class="idx-num">{{ i + 1 }}</span>
          <button class="idx-play" @click.stop="play(i)">
            <n-icon :component="isPlaying(t) ? VolumeHighOutline : PlayOutline" :size="16" />
          </button>
        </div>
        <div class="c-title" :title="t.title">{{ t.title }}</div>
        <div class="c-artist" :title="t.artist">{{ t.artist || "未知歌手" }}</div>
        <div class="c-album" :title="t.album">{{ t.album || "-" }}</div>
        <div class="c-dur">{{ fmt(t.duration) }}</div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { NButton, NIcon } from "naive-ui";
import { PlayOutline, AddOutline, VolumeHighOutline, FolderOpenOutline } from "@vicons/ionicons5";
import { useMusicStore } from "../../../../stores/useMusicStore";

const props = defineProps({
  tracks: { type: Array, default: () => [] },
});
defineEmits(["import", "import-dir"]);

const store = useMusicStore();

function isPlaying(t) {
  return store.currentTrack?.id === t.id;
}
function play(i) {
  store.setQueue(props.tracks, i);
}
function playAll() {
  store.setQueue(props.tracks, 0);
}
function fmt(sec) {
  if (!sec || !isFinite(sec)) return "-";
  const s = Math.floor(sec % 60);
  const m = Math.floor(sec / 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}
</script>
<!-- LIB_STYLE_PLACEHOLDER -->

<style scoped>
.lib-view { display: flex; flex-direction: column; height: 100%; padding: 20px 24px; box-sizing: border-box; overflow: hidden; }
.lib-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px; }
.lib-head-left { display: flex; align-items: baseline; gap: 10px; }
.lib-title { margin: 0; font-size: 20px; font-weight: 700; color: var(--guaxi-text-primary); }
.lib-count { font-size: 13px; color: var(--guaxi-text-secondary); }
.lib-head-actions { display: flex; gap: 8px; }

.lib-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; color: var(--guaxi-text-secondary); }
.lib-empty-icon { font-size: 48px; }
.lib-empty-text { font-size: 14px; }

.lib-table { flex: 1; overflow: auto; }
.lib-row {
  display: grid;
  grid-template-columns: 48px 1fr 160px 160px 64px;
  align-items: center;
  gap: 12px;
  padding: 0 10px;
  height: 44px;
  border-radius: 8px;
  font-size: 14px;
  color: var(--guaxi-text-primary);
}
.lib-row:not(.lib-row-head):hover { background: rgba(128,128,128,0.14); cursor: default; }
.lib-row-head {
  height: 34px; font-size: 12px; color: var(--guaxi-text-secondary); font-weight: 600;
  border-bottom: 1px solid var(--guaxi-border, #eee); border-radius: 0; position: sticky; top: 0;
  background: var(--guaxi-content-bg, #fff); z-index: 1;
}
.lib-row.playing { color: #a78bfa; font-weight: 600; }
.c-idx { position: relative; display: flex; align-items: center; justify-content: center; color: var(--guaxi-text-secondary); }
.idx-play { display: none; border: none; background: transparent; color: inherit; cursor: pointer; align-items: center; }
.lib-row:hover .idx-num { display: none; }
.lib-row:hover .idx-play { display: flex; }
.lib-row.playing .idx-num { display: none; }
.lib-row.playing .idx-play { display: flex; color: #a78bfa; }
.c-title { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font-weight: 500; }
.c-artist, .c-album { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--guaxi-text-secondary); }
.c-dur { text-align: right; color: var(--guaxi-text-secondary); font-variant-numeric: tabular-nums; }
</style>