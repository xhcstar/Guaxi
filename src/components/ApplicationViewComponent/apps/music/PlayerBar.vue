<template>
  <div class="player-bar">
    <audio
      ref="audioEl"
      :src="track?.url || ''"
      @timeupdate="onTimeUpdate"
      @loadedmetadata="onLoadedMeta"
      @ended="onEnded"
      @play="store.setPlaying(true)"
      @pause="store.setPlaying(false)"
      @error="onError"
    ></audio>

    <!-- 左：当前曲目 -->
    <div class="pb-track" @click="store.toggleNowPlaying()">
      <div class="pb-cover" :class="{ spinning: store.isPlaying }">
        <img v-if="track?.cover" :src="track.cover" alt="" />
        <span v-else class="pb-cover-fallback">♪</span>
      </div>
      <div class="pb-meta" v-if="track">
        <div class="pb-title">{{ track.title || "未知曲目" }}</div>
        <div class="pb-artist">{{ track.artist || "未知歌手" }}<span v-if="track.source && track.source !== 'local'" class="pb-source">· {{ sourceLabel }}</span></div>
      </div>
      <div class="pb-meta pb-meta-empty" v-else>
        <div class="pb-title">未在播放</div>
        <div class="pb-artist">选择一首歌开始</div>
      </div>
    </div>

    <!-- 中：控制 + 进度 -->
    <div class="pb-center">
      <div class="pb-controls">
        <button class="pb-btn pb-mode" :class="{ 'is-single': store.playMode === 'single' }" :title="store.playModeLabel" @click="store.cyclePlayMode()">
          <n-icon :component="modeIcon" :size="18" />
          <span v-if="store.playMode === 'single'" class="pb-mode-badge">1</span>
        </button>
        <button class="pb-btn" title="上一首" @click="store.prev()">
          <n-icon :component="PlaySkipBackOutline" :size="20" />
        </button>
        <button class="pb-btn pb-play" :title="store.isPlaying ? '暂停' : '播放'" @click="store.togglePlay()">
          <n-icon :component="store.isPlaying ? PauseOutline : PlayOutline" :size="26" />
        </button>
        <button class="pb-btn" title="下一首" @click="store.next(false)">
          <n-icon :component="PlaySkipForwardOutline" :size="20" />
        </button>
        <button class="pb-btn" title="播放队列" @click="store.toggleQueueDrawer()">
          <n-icon :component="ListOutline" :size="18" />
        </button>
      </div>
      <div class="pb-progress">
        <span class="pb-time">{{ fmt(store.currentTime) }}</span>
        <n-slider
          class="pb-slider"
          :value="store.currentTime"
          :max="store.duration || 0"
          :step="0.5"
          :tooltip="false"
          @update:value="onSeek"
        />
        <span class="pb-time">{{ fmt(store.duration) }}</span>
      </div>
    </div>

    <!-- 右：音量 -->
    <div class="pb-right">
      <button class="pb-btn" :title="store.muted ? '取消静音' : '静音'" @click="store.toggleMute()">
        <n-icon :component="volumeIcon" :size="18" />
      </button>
      <n-slider
        class="pb-vol"
        :value="store.muted ? 0 : store.volume * 100"
        :max="100"
        :tooltip="false"
        @update:value="onVolume"
      />
    </div>
  </div>
</template>
<!-- SCRIPT_PLACEHOLDER -->

<script setup>
import { ref, computed, watch, onMounted, onBeforeUnmount } from "vue";
import { NIcon, NSlider } from "naive-ui";
import {
  PlayOutline,
  PauseOutline,
  PlaySkipBackOutline,
  PlaySkipForwardOutline,
  ListOutline,
  RepeatOutline,
  ShuffleOutline,
  ArrowForwardOutline,
  VolumeHighOutline,
  VolumeLowOutline,
  VolumeMuteOutline,
} from "@vicons/ionicons5";
import { useMusicStore } from "../../../../stores/useMusicStore";

const store = useMusicStore();
const audioEl = ref(null);
const track = computed(() => store.currentTrack);

const sourceLabel = computed(() => {
  const map = { netease: "网易云", qq: "QQ音乐", kugou: "酷狗", bilibili: "B站", kuwo: "酷我", migu: "咪咕" };
  return map[track.value?.source] || track.value?.source || "";
});

const modeIcon = computed(() => {
  switch (store.playMode) {
    case "order": return ArrowForwardOutline;
    case "shuffle": return ShuffleOutline;
    default: return RepeatOutline; // list / single
  }
});

const volumeIcon = computed(() => {
  if (store.muted || store.volume === 0) return VolumeMuteOutline;
  if (store.volume < 0.5) return VolumeLowOutline;
  return VolumeHighOutline;
});

function fmt(sec) {
  if (!sec || !isFinite(sec)) return "00:00";
  const s = Math.floor(sec % 60);
  const m = Math.floor(sec / 60);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function onTimeUpdate() {
  if (audioEl.value && !seeking.value) store.reportTime(audioEl.value.currentTime);
}
function onLoadedMeta() {
  if (audioEl.value) store.reportDuration(audioEl.value.duration);
}
function onEnded() {
  store.next(true);
}
function onError() {
  // 播放失败(如解灰未命中/URL 失效)自动跳下一首
  if (track.value) store.next(true);
}

const seeking = ref(false);
function onSeek(val) {
  seeking.value = true;
  store.seek(val);
  if (audioEl.value) audioEl.value.currentTime = val;
  requestAnimationFrame(() => (seeking.value = false));
}
function onVolume(val) {
  store.setVolume(val / 100);
}

// 曲目切换：加载并按播放态自动播放
watch(
  () => track.value?.url,
  async (url) => {
    if (!audioEl.value) return;
    if (!url) {
      audioEl.value.pause();
      return;
    }
    audioEl.value.load();
    if (store.isPlaying) {
      try { await audioEl.value.play(); } catch (_) { /* 用户手势限制 */ }
    }
  }
);

watch(
  () => store.isPlaying,
  async (playing) => {
    if (!audioEl.value || !track.value) return;
    if (playing) {
      try { await audioEl.value.play(); } catch (_) {}
    } else {
      audioEl.value.pause();
    }
  }
);

watch([() => store.volume, () => store.muted], () => {
  if (audioEl.value) {
    audioEl.value.volume = store.volume;
    audioEl.value.muted = store.muted;
  }
});

// 外部 seek(如沉浸页拖动)同步到 audio 元素
watch(
  () => store._seekRequest,
  (req) => {
    if (req && audioEl.value) audioEl.value.currentTime = req.time;
  }
);

onMounted(() => {
  if (audioEl.value) {
    audioEl.value.volume = store.volume;
    audioEl.value.muted = store.muted;
  }
});
onBeforeUnmount(() => {
  if (audioEl.value) audioEl.value.pause();
});
</script>
<!-- STYLE_PLACEHOLDER -->

<style scoped>
.player-bar {
  display: flex;
  align-items: center;
  gap: 16px;
  height: 72px;
  padding: 0 20px;
  border-top: 1px solid var(--guaxi-border, #e5e7eb);
  background: var(--guaxi-panel-bg, #fff);
  box-sizing: border-box;
  flex-shrink: 0;
}

.pb-track {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 260px;
  min-width: 200px;
  cursor: pointer;
  user-select: none;
}
.pb-cover {
  width: 48px;
  height: 48px;
  border-radius: 8px;
  overflow: hidden;
  background: linear-gradient(135deg, #667eea, #764ba2);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.pb-cover img { width: 100%; height: 100%; object-fit: cover; }
.pb-cover-fallback { color: #fff; font-size: 22px; }
.pb-cover.spinning { animation: pb-spin 12s linear infinite; }
@keyframes pb-spin { to { transform: rotate(360deg); } }
.pb-meta { min-width: 0; }
.pb-meta-empty { opacity: 0.6; }
.pb-title {
  font-size: 14px; font-weight: 600;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  color: var(--guaxi-text-primary);
}
.pb-artist {
  font-size: 12px; color: var(--guaxi-text-secondary);
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.pb-source { margin-left: 4px; color: #a78bfa; }

.pb-center { flex: 1; display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.pb-controls { display: flex; align-items: center; justify-content: center; gap: 8px; }
.pb-btn {
  border: none; background: transparent; cursor: pointer;
  display: flex; align-items: center; justify-content: center;
  color: var(--guaxi-text-primary);
  width: 34px; height: 34px; border-radius: 50%;
  transition: background 0.15s, color 0.15s;
  position: relative;
}
.pb-btn:hover { background: rgba(128,128,128,0.14); }
.pb-play {
  width: 42px; height: 42px;
  background: linear-gradient(135deg, #667eea, #764ba2);
  color: #fff;
}
.pb-play:hover { filter: brightness(1.08); background: linear-gradient(135deg, #667eea, #764ba2); }
.pb-mode.is-single { color: #a78bfa; }
.pb-mode-badge {
  position: absolute; top: 2px; right: 2px;
  font-size: 9px; font-weight: 700; line-height: 1;
  background: #a78bfa; color: #fff;
  border-radius: 6px; padding: 1px 3px;
}

.pb-progress { display: flex; align-items: center; gap: 10px; }
.pb-time { font-size: 12px; color: var(--guaxi-text-secondary); width: 40px; text-align: center; flex-shrink: 0; font-variant-numeric: tabular-nums; }
.pb-slider { flex: 1; }

.pb-right { display: flex; align-items: center; gap: 6px; width: 150px; }
.pb-vol { flex: 1; }
</style>
