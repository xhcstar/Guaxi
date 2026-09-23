<template>
  <transition name="np-slide">
    <div v-if="store.nowPlayingVisible" class="now-playing" :style="bgStyle">
      <button class="np-close" @click="store.toggleNowPlaying()">
        <n-icon :component="ChevronDownOutline" :size="24" />
      </button>

      <div class="np-body">
        <div class="np-disc" :class="{ spinning: store.isPlaying }">
          <img v-if="track?.cover" :src="track.cover" alt="" />
          <div v-else class="np-disc-fallback">♪</div>
        </div>

        <div class="np-info">
          <div class="np-title">{{ track?.title || "未在播放" }}</div>
          <div class="np-artist">{{ track?.artist || "" }}</div>
          <div v-if="track?.album" class="np-album">专辑：{{ track.album }}</div>
          <div v-if="track?.source && track.source !== 'local'" class="np-source">来源：{{ sourceLabel }}</div>

          <div class="np-progress">
            <span class="np-time">{{ fmt(store.currentTime) }}</span>
            <n-slider class="np-slider" :value="store.currentTime" :max="store.duration || 0" :step="0.5" :tooltip="false" @update:value="onSeek" />
            <span class="np-time">{{ fmt(store.duration) }}</span>
          </div>

          <div class="np-controls">
            <button class="np-btn" @click="store.prev()"><n-icon :component="PlaySkipBackOutline" :size="26" /></button>
            <button class="np-btn np-play" @click="store.togglePlay()">
              <n-icon :component="store.isPlaying ? PauseOutline : PlayOutline" :size="34" />
            </button>
            <button class="np-btn" @click="store.next(false)"><n-icon :component="PlaySkipForwardOutline" :size="26" /></button>
          </div>
        </div>
      </div>
    </div>
  </transition>
</template>

<script setup>
import { computed } from "vue";
import { NIcon, NSlider } from "naive-ui";
import { ChevronDownOutline, PlayOutline, PauseOutline, PlaySkipBackOutline, PlaySkipForwardOutline } from "@vicons/ionicons5";
import { useMusicStore } from "../../../../stores/useMusicStore";

const store = useMusicStore();
const track = computed(() => store.currentTrack);
const sourceLabel = computed(() => {
  const map = { netease: "网易云", qq: "QQ音乐", kugou: "酷狗", bilibili: "B站", kuwo: "酷我", migu: "咪咕" };
  return map[track.value?.source] || track.value?.source || "";
});
// 阶段一用固定渐变；阶段二/三接入封面主色提取后替换为动态取色。
const bgStyle = computed(() => ({
  background: "radial-gradient(1200px 600px at 30% 20%, rgba(118,75,162,0.35), transparent), linear-gradient(160deg, #1f1c2c, #302b63 60%, #24243e)",
}));
function onSeek(v) { store.seek(v); }
function fmt(sec) {
  if (!sec || !isFinite(sec)) return "00:00";
  const s = Math.floor(sec % 60), m = Math.floor(sec / 60);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}
</script>
<!-- NP_STYLE -->

<style scoped>
.now-playing {
  position: absolute; inset: 0; z-index: 40;
  display: flex; flex-direction: column;
  color: #fff;
}
.np-close {
  position: absolute; top: 18px; left: 18px;
  width: 40px; height: 40px; border-radius: 50%;
  border: none; background: rgba(255,255,255,0.12); color: #fff; cursor: pointer;
  display: flex; align-items: center; justify-content: center;
}
.np-close:hover { background: rgba(255,255,255,0.22); }
.np-body { flex: 1; display: flex; align-items: center; justify-content: center; gap: 64px; padding: 40px; flex-wrap: wrap; }

.np-disc {
  width: 300px; height: 300px; border-radius: 50%;
  overflow: hidden; flex-shrink: 0;
  background: linear-gradient(135deg, #667eea, #764ba2);
  display: flex; align-items: center; justify-content: center;
  box-shadow: 0 20px 60px rgba(0,0,0,0.4);
  border: 10px solid rgba(0,0,0,0.25);
}
.np-disc img { width: 100%; height: 100%; object-fit: cover; }
.np-disc-fallback { font-size: 80px; color: rgba(255,255,255,0.85); }
.np-disc.spinning { animation: np-spin 18s linear infinite; }
@keyframes np-spin { to { transform: rotate(360deg); } }

.np-info { width: 420px; max-width: 90vw; }
.np-title { font-size: 30px; font-weight: 700; margin-bottom: 10px; }
.np-artist { font-size: 16px; opacity: 0.85; margin-bottom: 6px; }
.np-album, .np-source { font-size: 13px; opacity: 0.6; margin-bottom: 4px; }
.np-progress { display: flex; align-items: center; gap: 12px; margin: 28px 0 20px; }
.np-time { font-size: 12px; opacity: 0.75; width: 44px; text-align: center; font-variant-numeric: tabular-nums; }
.np-slider { flex: 1; }
.np-controls { display: flex; align-items: center; gap: 24px; }
.np-btn { border: none; background: transparent; color: #fff; cursor: pointer; display: flex; align-items: center; justify-content: center; width: 52px; height: 52px; border-radius: 50%; }
.np-btn:hover { background: rgba(255,255,255,0.12); }
.np-play { width: 68px; height: 68px; background: rgba(255,255,255,0.16); }
.np-play:hover { background: rgba(255,255,255,0.28); }

.np-slide-enter-active, .np-slide-leave-active { transition: transform 0.3s ease, opacity 0.3s ease; }
.np-slide-enter-from, .np-slide-leave-to { transform: translateY(100%); opacity: 0.4; }

/* NSlider 在深色沉浸页上的可读性 */
.np-info :deep(.n-slider .n-slider-rail) { background: rgba(255,255,255,0.2); }
</style>