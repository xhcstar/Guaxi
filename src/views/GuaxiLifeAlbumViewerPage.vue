<template>
  <div class="lav-window" :class="{ 'is-dark': isDark }">
    <div v-if="!current" class="lav-empty">
      <div class="empty-mark">✕</div>
      <div class="empty-text">没有可显示的影像</div>
    </div>

    <template v-else>
      <!-- 顶栏:信息 + 工具 -->
      <header class="lav-bar">
        <div class="lav-meta">
          <span class="m-time">{{ formatTs(current.taken_at) }}</span>
          <span class="m-sub">
            <span v-if="current.city" class="m-place">{{ current.city }}</span>
            <span v-if="isVideoCur && durationText" class="m-res">{{ durationText }}</span>
            <span v-else-if="resolutionText" class="m-res">{{ resolutionText }}</span>
            <span v-if="current.file_name" class="m-name" :title="current.file_name">{{ current.file_name }}</span>
          </span>
        </div>
        <div class="lav-counter">{{ cur + 1 }} <i>/</i> {{ list.length }}</div>
        <div class="lav-tools" v-if="!isVideoCur">
          <button class="vtool" title="缩小 (−)" @click="zoomOut">−</button>
          <button class="vtool" title="放大 (+)" @click="zoomIn">＋</button>
          <button class="vtool" :class="{ on: fitMode }" title="适应窗口" @click="fitToWindow">适应</button>
          <button class="vtool" :class="{ on: !fitMode }" title="原始大小 (1:1)" @click="actualSize">1:1</button>
          <button class="vtool" title="向左旋转" @click="rotateLeft">↺</button>
          <button class="vtool" title="向右旋转" @click="rotateRight">↻</button>
          <button class="vtool" title="重置 (0)" @click="resetView">重置</button>
        </div>
      </header>

      <!-- 舞台 -->
      <div
        ref="stageEl"
        class="lav-stage"
        :class="{ grabbing: dragging, pannable: canPan }"
        @wheel.prevent="onWheel"
        @mousedown="onStageDown"
        @dblclick="onDblClick"
      >
        <Transition name="lav-swap" mode="out-in">
          <video
            v-if="isVideoCur"
            :key="'v' + cur"
            class="lav-video"
            :src="toSrc(current.path)"
            controls
            autoplay
            playsinline
          />
          <img
            v-else
            :key="cur"
            class="lav-img"
            :src="toSrc(current.path)"
            :style="imgStyle"
            draggable="false"
            @load="onImgLoad"
          />
        </Transition>

        <button
          class="nav-arrow left"
          :disabled="cur <= 0"
          title="上一张 (←)"
          @click.stop="go(-1)"
        >‹</button>
        <button
          class="nav-arrow right"
          :disabled="cur >= list.length - 1"
          title="下一张 (→)"
          @click.stop="go(1)"
        >›</button>
      </div>

      <!-- 底部胶片条 -->
      <footer class="lav-film" ref="filmEl" @wheel.prevent="onFilmWheel">
        <button
          v-for="(p, i) in list"
          :key="p.path + i"
          class="film-cell"
          :class="{ active: i === cur }"
          :ref="(el) => setCellRef(el, i)"
          :title="formatTs(p.taken_at)"
          @click="jumpTo(i)"
        >
          <img v-if="!isVideo(p) || p.thumb" :src="thumbSrc(p)" loading="lazy" decoding="async" draggable="false" />
          <span v-else class="cell-video-ph"></span>
          <span v-if="isVideo(p)" class="cell-play">▶</span>
        </button>
      </footer>
    </template>
  </div>
</template>

<script setup>
/**
 * 旅行志照片查看器独立窗口(路由 /life-album-viewer)。
 *
 * 主窗口把整册照片(时间线倒序)+ 起始下标落到 Rust 待取槽位后开/唤醒本窗口;
 * 这里挂载时主动 take 一次,窗口已开着时 Rust 发 life-album-viewer:request 叫我们再取。
 * 全部翻页/缩放/旋转/平移都在本窗口内完成,不再回主窗口。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch, nextTick } from 'vue'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { createLogger } from '../utils/logger'

const props = defineProps({
  theme: { type: String, default: '' },
})

const logger = createLogger('LifeAlbumViewer')
const isDark = computed(() => props.theme === 'dark')

// ---------- 数据 ----------
const list = ref([])
const cur = ref(0)
const current = computed(() => list.value[cur.value] || null)

// ---------- 图片源 ----------
const toSrc = (p) => {
  const raw = String(p || '').trim()
  if (!raw) return ''
  if (/^(https?:|data:|blob:)/i.test(raw)) return raw
  try {
    const url = convertFileSrc(raw)
    return typeof url === 'string' && url ? url : raw
  } catch (e) {
    logger.warn('convertFileSrc failed:', e)
    return raw
  }
}
const thumbSrc = (p) => {
  if (isVideo(p)) return p?.thumb ? toSrc(p.thumb) : ''
  return toSrc(p?.thumb || p?.path)
}

// ---------- 视频判定 ----------
const VIDEO_EXTS = ['mp4', 'mov', 'm4v', 'webm']
const extOf = (p) => {
  const s = String(p || '')
  const i = s.lastIndexOf('.')
  return i >= 0 ? s.slice(i + 1).toLowerCase() : ''
}
const isVideo = (p) => (p?.media_type ? p.media_type === 'video' : VIDEO_EXTS.includes(extOf(p?.path)))
const isVideoCur = computed(() => isVideo(current.value))
const formatDuration = (sec) => {
  const s = Math.max(0, Math.round(Number(sec) || 0))
  if (!s) return ''
  const m = Math.floor(s / 60)
  const r = s % 60
  return `${m}:${String(r).padStart(2, '0')}`
}
const durationText = computed(() => formatDuration(current.value?.duration))

// ---------- 时间格式化 ----------
const parseTakenAt = (s) => {
  if (!s) return null
  const d = new Date(s)
  return Number.isNaN(d.getTime()) ? null : d
}
const pad = (n) => String(n).padStart(2, '0')
const formatTs = (s) => {
  const d = parseTakenAt(s)
  if (!d) return '未知时间'
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

const resolutionText = computed(() => {
  const p = current.value
  const w = Number(p?.width) || 0
  const h = Number(p?.height) || 0
  return w > 0 && h > 0 ? `${w} × ${h}` : ''
})

// ---------- 视图状态(缩放/旋转/平移) ----------
const stageEl = ref(null)
const stageSize = ref({ w: 0, h: 0 })
const imgNatural = ref({ w: 0, h: 0 })
const zoom = ref(1) // 1 = 适应窗口
const rotation = ref(0)
const pan = ref({ x: 0, y: 0 })
const fitMode = ref(true) // true=适应窗口, false=1:1

let stageResizeObserver = null

const onImgLoad = (e) => {
  const el = e?.target
  imgNatural.value = { w: Number(el?.naturalWidth) || 0, h: Number(el?.naturalHeight) || 0 }
}

// 适应窗口的基准缩放:把(考虑旋转后的)外接框塞进舞台。
const fitScale = computed(() => {
  const s = stageSize.value
  const img = imgNatural.value
  if (!s.w || !s.h || !img.w || !img.h) return 1
  const rad = (Math.abs(rotation.value) % 360) * (Math.PI / 180)
  const c = Math.abs(Math.cos(rad))
  const sn = Math.abs(Math.sin(rad))
  const bbW = img.w * c + img.h * sn
  const bbH = img.w * sn + img.h * c
  if (!bbW || !bbH) return 1
  const fit = Math.min(s.w / bbW, s.h / bbH)
  return Number.isFinite(fit) && fit > 0 ? fit : 1
})

// 1:1 相对适应基准的倍率。fitMode 下 zoom 直接乘 fitScale;actualSize 时 zoom 设为 1/fitScale。
const effectiveScale = computed(() => zoom.value * fitScale.value)
const canPan = computed(() => effectiveScale.value > fitScale.value + 1e-3)

const imgStyle = computed(() => ({
  transformOrigin: 'center center',
  transform: `translate(${pan.value.x}px, ${pan.value.y}px) scale(${effectiveScale.value}) rotate(${rotation.value}deg)`,
}))

const ZOOM_MIN = 0.1
const ZOOM_MAX = 12
const clampZoom = (z) => Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, z))

const resetView = () => {
  zoom.value = 1
  rotation.value = 0
  pan.value = { x: 0, y: 0 }
  fitMode.value = true
}
const fitToWindow = () => {
  zoom.value = 1
  pan.value = { x: 0, y: 0 }
  fitMode.value = true
}
const actualSize = () => {
  // 让实际缩放变成 1(原始像素):zoom * fitScale = 1。
  const fs = fitScale.value || 1
  zoom.value = clampZoom(1 / fs)
  pan.value = { x: 0, y: 0 }
  fitMode.value = false
}
const applyZoom = (factor) => {
  zoom.value = clampZoom(zoom.value * factor)
  fitMode.value = false
  if (!canPan.value) pan.value = { x: 0, y: 0 }
}
const zoomIn = () => applyZoom(1.15)
const zoomOut = () => applyZoom(1 / 1.15)
const rotateLeft = () => { rotation.value -= 90; pan.value = { x: 0, y: 0 } }
const rotateRight = () => { rotation.value += 90; pan.value = { x: 0, y: 0 } }

const onWheel = (e) => {
  if (isVideoCur.value) return
  if (e.deltaY < 0) zoomIn()
  else zoomOut()
}
const onDblClick = () => {
  if (isVideoCur.value) return
  if (fitMode.value) actualSize()
  else fitToWindow()
}

// 拖拽平移(仅在放大到超出适应尺寸时)
const dragging = ref(false)
let dragStart = null
const onStageDown = (e) => {
  if (isVideoCur.value || !canPan.value || e.button !== 0) return
  dragging.value = true
  dragStart = { x: e.clientX, y: e.clientY, px: pan.value.x, py: pan.value.y }
  window.addEventListener('mousemove', onDragMove)
  window.addEventListener('mouseup', onDragUp)
}
const onDragMove = (e) => {
  if (!dragging.value || !dragStart) return
  pan.value = { x: dragStart.px + (e.clientX - dragStart.x), y: dragStart.py + (e.clientY - dragStart.y) }
}
const onDragUp = () => {
  dragging.value = false
  dragStart = null
  window.removeEventListener('mousemove', onDragMove)
  window.removeEventListener('mouseup', onDragUp)
}

// ---------- 翻页 ----------
const go = (delta) => {
  const next = cur.value + delta
  if (next < 0 || next >= list.value.length) return
  cur.value = next
}
const jumpTo = (i) => {
  if (i < 0 || i >= list.value.length) return
  cur.value = i
}

// 用照片自带的宽高预置自然尺寸,避免换图瞬间 fitScale 用错基准导致缩放跳动。
const seedNatural = () => {
  const p = current.value
  const w = Number(p?.width) || 0
  const h = Number(p?.height) || 0
  imgNatural.value = { w, h }
}

// 预取相邻图片,让淡入切换时新图已解码完成,过渡更顺滑。
const preloadNeighbors = () => {
  for (const d of [1, -1]) {
    const p = list.value[cur.value + d]
    const src = p && toSrc(p.path)
    if (src) { const im = new Image(); im.src = src }
  }
}

// 切换照片时重置视图 + 用元数据预置自然尺寸 + 预取相邻 + 胶片条滚动到当前
watch(cur, () => {
  resetView()
  seedNatural()
  scrollFilmToCurrent()
  preloadNeighbors()
})

// ---------- 胶片条 ----------
const filmEl = ref(null)
const cellEls = {}
const setCellRef = (el, i) => {
  if (el) cellEls[i] = el
  else delete cellEls[i]
}
const scrollFilmToCurrent = async () => {
  await nextTick()
  const el = cellEls[cur.value]
  if (el?.scrollIntoView) {
    el.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'center' })
  }
}
// 胶片条是横向列表,鼠标滚轮默认只滚纵向;把竖向滚轮增量转成横向滚动。
const onFilmWheel = (e) => {
  const el = filmEl.value
  if (!el) return
  el.scrollLeft += (Math.abs(e.deltaY) > Math.abs(e.deltaX) ? e.deltaY : e.deltaX)
}

// ---------- 键盘 ----------
const onKey = (e) => {
  switch (e.key) {
    case 'ArrowLeft': e.preventDefault(); go(-1); break
    case 'ArrowRight': e.preventDefault(); go(1); break
    case '+': case '=': if (!isVideoCur.value) { e.preventDefault(); zoomIn() } break
    case '-': case '_': if (!isVideoCur.value) { e.preventDefault(); zoomOut() } break
    case '0': if (!isVideoCur.value) { e.preventDefault(); resetView() } break
    case 'r': case 'R': if (!isVideoCur.value) { e.preventDefault(); rotateRight() } break
    case 'Escape': e.preventDefault(); closeWindow(); break
    default: break
  }
}

// ---------- 窗口 ----------
const closeWindow = async () => {
  try {
    await getCurrentWindow().close()
  } catch (e) {
    logger.warn('close window failed:', e)
  }
}

// ---------- 请求交接 ----------
let unlisten = null
const pullRequest = async () => {
  try {
    const req = await invoke('life_album_viewer_take_request')
    if (req && Array.isArray(req.photos) && req.photos.length) {
      list.value = req.photos
      const idx = Number(req.index) || 0
      cur.value = Math.min(Math.max(0, idx), req.photos.length - 1)
      resetView()
      seedNatural()
      scrollFilmToCurrent()
      preloadNeighbors()
    }
  } catch (e) {
    logger.warn('take request failed:', e)
  }
}

const measureStage = () => {
  const el = stageEl.value
  if (!el) return
  const r = el.getBoundingClientRect()
  stageSize.value = { w: r.width, h: r.height }
}

onMounted(async () => {
  try {
    unlisten = await listen('life-album-viewer:request', () => { void pullRequest() })
  } catch (e) {
    logger.warn('listen failed:', e)
  }
  await pullRequest()
  await nextTick()
  measureStage()
  if (stageEl.value) {
    stageResizeObserver = new ResizeObserver(measureStage)
    stageResizeObserver.observe(stageEl.value)
  }
  window.addEventListener('keydown', onKey)
})

onBeforeUnmount(() => {
  if (typeof unlisten === 'function') unlisten()
  unlisten = null
  if (stageResizeObserver) { stageResizeObserver.disconnect(); stageResizeObserver = null }
  window.removeEventListener('keydown', onKey)
  window.removeEventListener('mousemove', onDragMove)
  window.removeEventListener('mouseup', onDragUp)
})
</script>

<style scoped>
/* 独立窗口根:用 100vh(与本项目其它独立窗口一致) */
.lav-window {
  --ink: #ecdfca;
  --ink-2: #b8a888;
  --accent: #d98b6f;
  --line: rgba(216, 180, 140, 0.18);
  --serif: Georgia, 'Songti SC', 'STSong', 'SimSun', serif;
  --sans: 'Avenir Next', 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', system-ui, sans-serif;
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100vh;
  max-height: 100vh;
  overflow: hidden;
  background: #14100c;
  color: var(--ink);
  font-family: var(--sans);
  user-select: none;
}

.lav-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: var(--ink-2);
}
.lav-empty .empty-mark {
  width: 46px; height: 46px; border-radius: 50%;
  border: 1px solid var(--line);
  display: flex; align-items: center; justify-content: center;
  font-size: 20px; opacity: 0.6;
}

/* 顶栏 */
.lav-bar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px 18px;
  background: rgba(20, 15, 10, 0.72);
  border-bottom: 1px solid var(--line);
  backdrop-filter: blur(8px);
  z-index: 3;
}
.lav-meta { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
.m-time { font-family: var(--serif); font-size: 15px; letter-spacing: 0.04em; }
.m-sub { display: flex; align-items: center; gap: 12px; font-size: 11px; color: var(--ink-2); min-width: 0; }
.m-place { letter-spacing: 0.16em; text-transform: uppercase; color: var(--accent); }
.m-res { font-variant-numeric: tabular-nums; }
.m-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 260px; }
.lav-counter {
  margin-left: auto;
  font-family: var(--serif);
  font-size: 15px;
  letter-spacing: 0.06em;
  font-variant-numeric: tabular-nums;
  color: var(--ink);
}
.lav-counter i { color: var(--ink-2); font-style: normal; margin: 0 3px; }
.lav-tools { display: flex; gap: 6px; }
.vtool {
  border: 1px solid var(--line);
  background: rgba(255, 255, 255, 0.04);
  color: var(--ink);
  font-family: var(--sans);
  font-size: 13px;
  min-width: 36px;
  height: 30px;
  padding: 0 10px;
  border-radius: 3px;
  cursor: pointer;
  transition: all 0.18s;
}
.vtool:hover { background: rgba(216, 140, 90, 0.28); border-color: var(--accent); color: #fff; }
.vtool.on { background: rgba(216, 140, 90, 0.2); border-color: var(--accent); color: #fff; }

/* 舞台 */
.lav-stage {
  flex: 1 1 auto;
  position: relative;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  background:
    radial-gradient(120% 120% at 50% 30%, rgba(60, 45, 30, 0.35), rgba(10, 7, 4, 0.9));
}
.lav-stage.pannable { cursor: grab; }
.lav-stage.grabbing { cursor: grabbing; }
.lav-img {
  width: auto;
  height: auto;
  max-width: none;
  max-height: none;
  display: block;
  will-change: transform, opacity;
  -webkit-user-drag: none;
}

/* 视频:直接铺在舞台内,原生 controls 播放(不套缩放/旋转) */
.lav-video {
  max-width: calc(100% - 40px);
  max-height: calc(100% - 40px);
  width: auto;
  height: auto;
  display: block;
  background: #000;
  border-radius: 3px;
  box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
  will-change: opacity;
}

/* 换图淡入淡出:旧图先淡出,新图淡入,配合相邻预取,过渡自然不闪黑 */
.lav-swap-enter-active { transition: opacity 0.32s ease; }
.lav-swap-leave-active { transition: opacity 0.18s ease; }
.lav-swap-enter-from,
.lav-swap-leave-to { opacity: 0; }

/* 导航箭头 */
.nav-arrow {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  width: 52px;
  height: 72px;
  border: 1px solid var(--line);
  background: rgba(20, 15, 10, 0.4);
  color: var(--ink);
  font-size: 38px;
  line-height: 1;
  border-radius: 4px;
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.2s, background 0.2s, border-color 0.2s;
  backdrop-filter: blur(4px);
}
.nav-arrow.left { left: 18px; }
.nav-arrow.right { right: 18px; }
.lav-stage:hover .nav-arrow { opacity: 0.85; }
.nav-arrow:hover { background: rgba(216, 140, 90, 0.35); border-color: var(--accent); opacity: 1; }
.nav-arrow:disabled { opacity: 0 !important; cursor: default; pointer-events: none; }

/* 胶片条 */
.lav-film {
  flex: 0 0 auto;
  display: flex;
  gap: 6px;
  padding: 10px 14px;
  overflow-x: auto;
  overflow-y: hidden;
  background: rgba(20, 15, 10, 0.72);
  border-top: 1px solid var(--line);
  scrollbar-width: thin;
  scrollbar-color: rgba(216, 180, 140, 0.3) transparent;
}
.lav-film::-webkit-scrollbar { height: 8px; }
.lav-film::-webkit-scrollbar-thumb { background: rgba(216, 180, 140, 0.3); border-radius: 4px; }
.film-cell {
  flex: 0 0 auto;
  position: relative;
  width: 68px;
  height: 68px;
  padding: 0;
  border: 1px solid transparent;
  border-radius: 3px;
  overflow: hidden;
  cursor: pointer;
  background: #000;
  opacity: 0.55;
  transition: opacity 0.18s, border-color 0.18s, transform 0.18s;
}
.film-cell img { width: 100%; height: 100%; object-fit: cover; display: block; }
.cell-video-ph { display: block; width: 100%; height: 100%; background: linear-gradient(135deg, #2a231c, #171310); }
.cell-play {
  position: absolute;
  top: 50%; left: 50%;
  transform: translate(-50%, -50%);
  width: 22px; height: 22px;
  display: flex; align-items: center; justify-content: center;
  border-radius: 50%;
  background: rgba(20, 15, 10, 0.5);
  border: 1px solid rgba(255, 255, 255, 0.85);
  color: #fff; font-size: 8px; padding-left: 2px;
  pointer-events: none;
}
.film-cell:hover { opacity: 0.85; }
.film-cell.active { opacity: 1; border-color: var(--accent); transform: translateY(-2px); }
</style>


