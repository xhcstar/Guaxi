<template>
  <div class="journal">
    <!-- 顶部报头 -->
    <header class="masthead">
      <div class="masthead-left">
        <div class="kicker">TRAVEL · JOURNAL</div>
        <h1 class="wordmark">旅行志</h1>
        <div v-if="albumDir" class="dateline" :title="albumDir">{{ albumDir }}</div>
      </div>
      <div class="masthead-right">
        <button class="link-btn" :disabled="loading" @click="pickDir">选择相册目录</button>
        <span class="dot-sep">·</span>
        <button class="link-btn" :disabled="loading" @click="importFiles">导入照片</button>
      </div>
    </header>

    <!-- 视图切换 + 统计 -->
    <div class="controlbar">
      <div class="segmented" role="tablist">
        <button
          class="seg"
          :class="{ active: viewMode === 'timeline' }"
          role="tab"
          @click="viewMode = 'timeline'"
        >时间线</button>
        <button
          class="seg"
          :class="{ active: viewMode === 'location' }"
          role="tab"
          @click="viewMode = 'location'"
        >地图</button>
        <span class="seg-ink" :class="viewMode"></span>
      </div>
      <div class="counts">
        <span class="count-num">{{ photos.length }}</span> 帧影像
        <template v-if="geoPhotosAll.length">
          <span class="dot-sep">·</span>
          <span class="count-num">{{ geoPhotosAll.length }}</span> 处足迹
        </template>
      </div>
    </div>

    <!-- 状态 -->
    <div v-if="loading" class="state">
      <span class="spinner"></span>
      <span>正在翻阅相册…</span>
    </div>

    <div v-else-if="!photos.length" class="state empty">
      <div class="empty-mark">◍</div>
      <div class="empty-title">相册还是空白页</div>
      <div class="empty-hint">选择一个相册目录，或导入照片，开始你的旅行志。</div>
    </div>

    <!-- 时间线 -->
    <div v-else-if="viewMode === 'timeline'" class="timeline">
      <!-- 左侧时间轴脊柱 -->
      <nav class="spine">
        <div class="spine-line"></div>
        <button
          v-for="s in spine"
          :key="s.key"
          class="spine-node"
          :class="{ active: activeMonth === s.key }"
          @click="scrollToMonth(s.key)"
        >
          <span class="spine-dot"></span>
          <span class="spine-year" v-if="s.showYear">{{ s.year }}</span>
          <span class="spine-month">{{ s.monthShort }}</span>
        </button>
      </nav>

      <!-- 主区：月为章节的非对称网格 -->
      <div ref="scrollEl" class="stream" @scroll.passive="onStreamScroll">
        <section
          v-for="g in timelineGroups"
          :key="g.key"
          class="chapter"
          :data-month="g.key"
          :ref="setChapterRef"
        >
          <div class="chapter-head">
            <div class="chapter-time">
              <span class="chapter-month">{{ g.monthLabel }}</span>
              <span class="chapter-year">{{ g.year }}</span>
            </div>
            <div class="chapter-meta">
              <span v-if="g.place" class="chapter-place">{{ g.place }}</span>
              <span class="chapter-count">{{ g.items.length }} 帧</span>
            </div>
            <div class="chapter-rule"></div>
          </div>

          <div class="mosaic">
            <figure
              v-for="(p, i) in g.items"
              :key="p.path"
              class="frame"
              :class="{ wide: isWide(p), tall: isTall(p), hero: i === 0 && isWide(p) }"
              :style="tileStyle(p)"
              @click="openDetail(p)"
            >
              <img class="frame-img" :src="thumbSrc(p)" loading="lazy" decoding="async" />
              <figcaption class="frame-cap">
                <span class="cap-time">{{ formatClock(p.taken_at) }}</span>
                <span v-if="cityFor(p)" class="cap-place">{{ cityFor(p) }}</span>
              </figcaption>
            </figure>
          </div>
        </section>

        <div v-if="photos.length > visibleLimit" class="load-more">
          <button class="ghost-btn" @click="loadMore">展开更早的记忆</button>
          <div class="load-more-hint">已翻阅 {{ visibleLimit }} / {{ photos.length }}</div>
        </div>
      </div>
    </div>

    <!-- 地图 -->
    <div v-else class="atlas">
      <div class="map-frame">
        <div ref="mapEl" class="map"></div>

        <div class="geo-hud" @click.stop>
          <span class="geo-dot" :class="{ on: !!amapKey }"></span>
          <span class="geo-text">{{ amapKey ? '高德地名服务' : '免费多源反查' }}</span>
          <button class="geo-edit" @click="toggleKeyEditor">{{ amapKey ? '更换 Key' : '数据源设置' }}</button>
          <div v-if="showKeyEditor" class="geo-pop">
            <div class="geo-hint">
              默认已启用免费多源反查（BigDataCloud → Photon → OSM），无需 Key、支持中文；结果永久缓存，之后秒开。
            </div>
            <input
              v-model="amapKeyInput"
              class="geo-input"
              placeholder="可选：高德 Web 服务 Key（留空即用免费源）"
              spellcheck="false"
              @keyup.enter="saveAmapKey"
            />
            <button class="geo-save" @click="saveAmapKey">保存</button>
            <button class="geo-clear" :disabled="clearingGeo" @click="clearGeoCache">
              {{ clearingGeo ? '正在重新反查…' : '清空地名缓存并重新反查' }}
            </button>
          </div>
        </div>

        <div v-if="!geoPhotosAll.length" class="map-empty">
          <div class="empty-mark">✕</div>
          <div>这些照片还没有留下坐标</div>
        </div>
      </div>

      <div class="atlas-chapters">
        <article v-for="c in cityChapters" :key="c.key" class="city-card">
          <header class="city-head" @click="flyTo(c)">
            <span class="city-name">{{ c.title }}</span>
            <span class="city-count">{{ c.items.length }}</span>
          </header>
          <div class="city-strip">
            <div
              v-for="p in c.items.slice(0, 24)"
              :key="p.path"
              class="strip-thumb"
              @click="openDetail(p)"
            >
              <img :src="thumbSrc(p)" loading="lazy" decoding="async" />
            </div>
          </div>
        </article>
      </div>
    </div>

    <!-- 全屏查看器 -->
    <n-modal
      v-model:show="detailVisible"
      preset="card"
      class="viewer-modal"
      style="width: 100vw; height: 100vh; max-width: 100vw; max-height: 100vh; margin: 0;"
    >
      <div
        v-if="detail"
        ref="viewerStageEl"
        class="viewer"
        @click="detailVisible = false"
        @wheel.prevent="onViewerWheel"
      >
        <div class="viewer-bar" @click.stop>
          <div class="viewer-info">
            <span class="viewer-time">{{ formatTs(detail.taken_at) }}</span>
            <span v-if="cityFor(detail)" class="viewer-place">{{ cityFor(detail) }}</span>
          </div>
          <div class="viewer-tools">
            <button class="vtool" @click="viewerZoomOut">−</button>
            <button class="vtool" @click="viewerZoomIn">＋</button>
            <button class="vtool" @click="viewerRotateLeft">↺</button>
            <button class="vtool" @click="viewerRotateRight">↻</button>
            <button class="vtool" @click="viewerReset">重置</button>
            <button class="vtool close" @click="detailVisible = false">退出</button>
          </div>
        </div>
        <img
          class="viewer-img"
          :src="toSrc(detail.path)"
          :style="viewerImgStyle"
          @load="onViewerImgLoad"
          @click.stop
        />
      </div>
    </n-modal>
  </div>
</template>

<script setup>
import { createLogger } from '../../../utils/logger'
import { computed, ref, watch, nextTick, onMounted, onUnmounted } from 'vue'
import { createDiscreteApi } from 'naive-ui'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

import 'leaflet/dist/leaflet.css'
import 'leaflet.markercluster/dist/MarkerCluster.css'
import 'leaflet.markercluster/dist/MarkerCluster.Default.css'
import L from 'leaflet'
import 'leaflet.markercluster'
import markerIcon2x from 'leaflet/dist/images/marker-icon-2x.png'
import markerIcon from 'leaflet/dist/images/marker-icon.png'
import markerShadow from 'leaflet/dist/images/marker-shadow.png'

const logger = createLogger('LifeAlbumApp')

delete L.Icon.Default.prototype._getIconUrl
L.Icon.Default.mergeOptions({
  iconRetinaUrl: markerIcon2x,
  iconUrl: markerIcon,
  shadowUrl: markerShadow
})

const { message } = createDiscreteApi(['message'])

const loading = ref(false)
const viewMode = ref('timeline')
const photos = ref([])
const albumDir = ref('')
const visibleLimit = ref(400)

const detailVisible = ref(false)
const detail = ref(null)

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
const thumbSrc = (p) => toSrc(p?.thumb || p?.path)

// ---------- 时间 ----------
const parseTakenAt = (s) => {
  if (!s) return null
  const d = new Date(s)
  return Number.isNaN(d.getTime()) ? null : d
}
const MONTHS = ['一月','二月','三月','四月','五月','六月','七月','八月','九月','十月','十一月','十二月']
const monthKey = (p) => {
  const d = parseTakenAt(p?.taken_at)
  if (!d) return 'unknown'
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`
}
const formatClock = (s) => {
  const d = parseTakenAt(s)
  if (!d) return ''
  return `${String(d.getMonth() + 1).padStart(2, '0')}.${String(d.getDate()).padStart(2, '0')} ${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}
const formatTs = (s) => {
  const d = parseTakenAt(s)
  if (!d) return '未知时间'
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')} ${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}

const sortByTimeDesc = (a, b) =>
  (parseTakenAt(b?.taken_at)?.getTime() ?? 0) - (parseTakenAt(a?.taken_at)?.getTime() ?? 0)

// ---------- 坐标 / 城市 ----------
const cityByGeoKey = ref({})
const coordKey = (p) => {
  const lat = p?.latitude, lon = p?.longitude
  if (typeof lat !== 'number' || typeof lon !== 'number') return 'unknown'
  return `${lat.toFixed(4)},${lon.toFixed(4)}`
}
const geoKey3 = (p) => {
  const lat = p?.latitude, lon = p?.longitude
  if (typeof lat !== 'number' || typeof lon !== 'number') return 'unknown'
  return `${lat.toFixed(3)},${lon.toFixed(3)}`
}
const cityFor = (p) => {
  const k = geoKey3(p)
  if (k === 'unknown') return ''
  return cityByGeoKey.value[k] || ''
}

// ---------- 瓦片尺寸（保留原始宽高比，杂志式错落） ----------
const tileStyle = (p) => {
  const w = Number(p?.width) || 0
  const h = Number(p?.height) || 0
  if (w > 0 && h > 0) return { aspectRatio: `${w} / ${h}` }
  return { aspectRatio: '4 / 3' }
}
const isWide = (p) => {
  const w = Number(p?.width) || 0, h = Number(p?.height) || 0
  return w > 0 && h > 0 && w / h >= 1.7
}
const isTall = (p) => {
  const w = Number(p?.width) || 0, h = Number(p?.height) || 0
  return w > 0 && h > 0 && h / w >= 1.5
}

const visiblePhotos = computed(() => photos.value.slice(0, visibleLimit.value))
const geoPhotosAll = computed(() =>
  photos.value.filter((p) => typeof p?.latitude === 'number' && typeof p?.longitude === 'number')
)

// ---------- 时间线分组（按月成章） ----------
const mostCommonCity = (items) => {
  const tally = {}
  for (const p of items) {
    const c = cityFor(p)
    if (c) tally[c] = (tally[c] || 0) + 1
  }
  let best = '', n = 0
  for (const [c, cnt] of Object.entries(tally)) if (cnt > n) { best = c; n = cnt }
  return best
}

const timelineGroups = computed(() => {
  const m = new Map()
  for (const p of visiblePhotos.value) {
    const k = monthKey(p)
    if (!m.has(k)) m.set(k, [])
    m.get(k).push(p)
  }
  const keys = Array.from(m.keys()).sort((a, b) => {
    if (a === 'unknown') return 1
    if (b === 'unknown') return -1
    return a < b ? 1 : -1
  })
  return keys.map((k) => {
    const items = m.get(k).slice().sort(sortByTimeDesc)
    if (k === 'unknown') {
      return { key: k, year: '', monthLabel: '未标注时间', place: mostCommonCity(items), items }
    }
    const [y, mm] = k.split('-')
    return {
      key: k,
      year: y,
      monthLabel: MONTHS[Number(mm) - 1] || mm,
      place: mostCommonCity(items),
      items
    }
  })
})

const spine = computed(() => {
  let lastYear = null
  return timelineGroups.value
    .filter((g) => g.key !== 'unknown')
    .map((g) => {
      const showYear = g.year !== lastYear
      lastYear = g.year
      const mm = Number(g.key.split('-')[1])
      return { key: g.key, year: g.year, monthShort: `${mm}月`, showYear }
    })
})

const activeMonth = ref('')
const scrollEl = ref(null)
const chapterEls = {}
const setChapterRef = (el) => {
  if (el && el.dataset && el.dataset.month) chapterEls[el.dataset.month] = el
}
let spy = null
const setupSpy = async () => {
  await nextTick()
  if (spy) spy.disconnect()
  if (!scrollEl.value) return
  spy = new IntersectionObserver(
    (entries) => {
      const vis = entries
        .filter((e) => e.isIntersecting)
        .sort((a, b) => b.intersectionRatio - a.intersectionRatio)[0]
      if (vis?.target?.dataset?.month) activeMonth.value = vis.target.dataset.month
    },
    { root: scrollEl.value, threshold: [0.1, 0.5], rootMargin: '-10% 0px -70% 0px' }
  )
  for (const el of Object.values(chapterEls)) spy.observe(el)
}
const scrollToMonth = (key) => {
  chapterEls[key]?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}
const onStreamScroll = () => {}

// ---------- 城市章节（地图视图） ----------
const cityChapters = computed(() => {
  const m = new Map()
  for (const p of geoPhotosAll.value) {
    const title = cityFor(p) || '定位中…'
    const key = cityFor(p) || coordKey(p)
    if (!m.has(key)) m.set(key, { key, title, items: [], lat: p.latitude, lon: p.longitude })
    m.get(key).items.push(p)
  }
  return Array.from(m.values())
    .map((c) => ({ ...c, items: c.items.slice().sort(sortByTimeDesc) }))
    .sort((a, b) => b.items.length - a.items.length)
})

// ---------- 地名批量反查（命中缓存秒回，未命中走高德批量，结果落库） ----------
const geocodingBusy = ref(false)
const ensureCities = async () => {
  if (geocodingBusy.value) return
  const seen = new Set()
  const coords = []
  for (const p of geoPhotosAll.value) {
    const k = geoKey3(p)
    if (k === 'unknown' || cityByGeoKey.value[k] || seen.has(k)) continue
    seen.add(k)
    coords.push([p.latitude, p.longitude])
  }
  if (!coords.length) return
  geocodingBusy.value = true
  try {
    const map = await invoke('life_album_reverse_geocode_batch', { coords })
    if (map && typeof map === 'object') {
      cityByGeoKey.value = { ...cityByGeoKey.value, ...map }
    }
  } catch (e) {
    logger.error('批量地名反查失败:', e)
  } finally {
    geocodingBusy.value = false
  }
}

// ---------- 高德 Key（国内地名反查加速，可选） ----------
const amapKey = ref('')
const amapKeyInput = ref('')
const showKeyEditor = ref(false)
const loadAmapKey = async () => {
  try {
    const k = await invoke('get_life_album_amap_key_cmd')
    amapKey.value = typeof k === 'string' ? k : ''
    amapKeyInput.value = amapKey.value
  } catch (e) {
    logger.warn('读取高德 Key 失败:', e)
  }
}
const toggleKeyEditor = () => {
  showKeyEditor.value = !showKeyEditor.value
  if (showKeyEditor.value) amapKeyInput.value = amapKey.value
}
const saveAmapKey = async () => {
  const key = String(amapKeyInput.value || '').trim()
  try {
    await invoke('set_life_album_amap_key', { key })
    amapKey.value = key
    showKeyEditor.value = false
    message.success(key ? '已启用高德地名服务' : '已清除高德 Key')
  } catch (e) {
    logger.error('保存高德 Key 失败:', e)
    message.error('保存失败')
  }
}
const clearingGeo = ref(false)
const clearGeoCache = async () => {
  if (clearingGeo.value) return
  clearingGeo.value = true
  try {
    const n = await invoke('life_album_clear_geocode_cache')
    cityByGeoKey.value = {}
    message.success(`已清空 ${n} 条地名缓存，正在重新反查…`)
    await ensureCities()
    if (viewMode.value === 'location') refreshMapMarkers()
  } catch (e) {
    logger.error('清空地名缓存失败:', e)
    message.error('清空失败')
  } finally {
    clearingGeo.value = false
  }
}

// ---------- 地图（高德底图 + marker 聚合） ----------
const mapEl = ref(null)
let map = null
let clusterLayer = null

const teardownMap = () => {
  if (!map) return
  map.remove()
  map = null
  clusterLayer = null
}

const clusterIcon = (cluster) => {
  const n = cluster.getChildCount()
  const size = n < 10 ? 34 : n < 100 ? 42 : 52
  return L.divIcon({
    html: `<div class="cl-badge"><span>${n}</span></div>`,
    className: 'cl-wrap',
    iconSize: L.point(size, size)
  })
}
const photoIcon = () =>
  L.divIcon({ html: '<div class="pin"></div>', className: 'pin-wrap', iconSize: L.point(18, 18) })

const ensureMap = async () => {
  await nextTick()
  if (!mapEl.value || map) return

  map = L.map(mapEl.value, { zoomControl: true, attributionControl: false })

  const amap = L.tileLayer(
    'https://webrd0{s}.is.autonavi.com/appmaptile?lang=zh_cn&size=1&scale=1&style=8&x={x}&y={y}&z={z}',
    { subdomains: ['1', '2', '3', '4'], maxZoom: 19 }
  )
  const osm = L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', { maxZoom: 19 })
  let fellBack = false
  amap.on('tileerror', () => {
    if (fellBack) return
    fellBack = true
    try { map.removeLayer(amap) } catch {}
    osm.addTo(map)
  })
  amap.addTo(map)

  clusterLayer = L.markerClusterGroup({
    iconCreateFunction: clusterIcon,
    showCoverageOnHover: false,
    maxClusterRadius: 48,
    spiderfyOnMaxZoom: true
  })
  map.addLayer(clusterLayer)
}

const avgCoord = (items) => {
  const pts = items.filter(
    (p) => typeof p?.latitude === 'number' && typeof p?.longitude === 'number'
  )
  if (!pts.length) return null
  const s = pts.reduce((a, p) => ({ lat: a.lat + p.latitude, lon: a.lon + p.longitude }), { lat: 0, lon: 0 })
  return { lat: s.lat / pts.length, lon: s.lon / pts.length }
}

const refreshMapMarkers = async () => {
  if (viewMode.value !== 'location' || !geoPhotosAll.value.length) return
  await ensureMap()
  if (!map || !clusterLayer) return

  clusterLayer.clearLayers()

  // 按 4 位小数（约 11m）聚成点，每点一个 marker，交给 markercluster 做视觉聚合。
  const spots = new Map()
  for (const p of geoPhotosAll.value) {
    const k = coordKey(p)
    if (k === 'unknown') continue
    if (!spots.has(k)) spots.set(k, [])
    spots.get(k).push(p)
  }

  const bounds = []
  const markers = []
  for (const [, items] of spots) {
    const c = avgCoord(items)
    if (!c) continue
    const first = items.slice().sort(sortByTimeDesc)[0]
    const label = `${cityFor(first) || '未定位'} · ${items.length} 帧`
    const mk = L.marker([c.lat, c.lon], { icon: photoIcon() })
    mk.bindTooltip(label, { direction: 'top', offset: [0, -10] })
    mk.on('click', () => first && openDetail(first))
    markers.push(mk)
    bounds.push([c.lat, c.lon])
  }
  clusterLayer.addLayers(markers)

  await nextTick()
  map.invalidateSize()
  if (bounds.length === 1) map.setView(bounds[0], 12)
  else if (bounds.length > 1) map.fitBounds(bounds, { padding: [40, 40], maxZoom: 14 })
}

const flyTo = (c) => {
  if (viewMode.value !== 'location' || !map) return
  if (typeof c?.lat === 'number' && typeof c?.lon === 'number') map.setView([c.lat, c.lon], 13)
}

// ---------- 全屏查看器 ----------
const viewerStageEl = ref(null)
let viewerResizeObserver = null
const viewerZoom = ref(1)
const viewerRotation = ref(0)
const viewerStageSize = ref({ w: 0, h: 0 })
const viewerImgNatural = ref({ w: 0, h: 0 })

const onViewerImgLoad = (e) => {
  const el = e?.target
  viewerImgNatural.value = { w: Number(el?.naturalWidth) || 0, h: Number(el?.naturalHeight) || 0 }
}
const fitScale = computed(() => {
  const s = viewerStageSize.value, img = viewerImgNatural.value
  if (!s.w || !s.h || !img.w || !img.h) return 1
  const rad = (Math.abs(viewerRotation.value) % 360) * (Math.PI / 180)
  const c = Math.abs(Math.cos(rad)), sn = Math.abs(Math.sin(rad))
  const bbW = img.w * c + img.h * sn, bbH = img.w * sn + img.h * c
  if (!bbW || !bbH) return 1
  const fit = Math.min(s.w / bbW, s.h / bbH)
  return Number.isFinite(fit) && fit > 0 ? fit : 1
})
const viewerReady = computed(() => {
  const s = viewerStageSize.value, img = viewerImgNatural.value
  return Boolean(s.w && s.h && img.w && img.h)
})
const viewerImgStyle = computed(() => {
  const z = Math.min(8, Math.max(0.1, viewerZoom.value))
  return {
    transformOrigin: 'center center',
    transform: `scale(${z * fitScale.value}) rotate(${viewerRotation.value}deg)`,
    opacity: viewerReady.value ? 1 : 0
  }
})
const viewerReset = () => { viewerZoom.value = 1; viewerRotation.value = 0 }
const viewerZoomIn = () => { viewerZoom.value = Math.min(8, viewerZoom.value * 1.12) }
const viewerZoomOut = () => { viewerZoom.value = Math.max(0.1, viewerZoom.value / 1.12) }
const viewerRotateLeft = () => { viewerRotation.value -= 90 }
const viewerRotateRight = () => { viewerRotation.value += 90 }
const onViewerWheel = (e) => { e?.deltaY < 0 ? viewerZoomIn() : viewerZoomOut() }

const openDetail = (p) => {
  detail.value = p
  viewerReset()
  detailVisible.value = true
}

// ---------- 加载 / 增强 ----------
const enriched = new Set()
let enrichAllToken = 0

const enrichVisible = async () => {
  const batch = visiblePhotos.value
    .map((p) => p?.path)
    .filter((p) => typeof p === 'string' && p && !enriched.has(p))
    .slice(0, 200)
  if (!batch.length) return
  for (const p of batch) enriched.add(p)
  try {
    const metas = await invoke('life_album_enrich_files', { paths: batch })
    mergeMetas(metas)
  } catch (e) {
    logger.error('EXIF 增强失败:', e)
  }
  ensureCities()
}

const mergeMetas = (metas) => {
  const map = new Map((Array.isArray(metas) ? metas : []).map((m) => [m.path, m]))
  if (!map.size) return
  photos.value = photos.value.map((p) => {
    const m = map.get(p.path)
    if (!m) return p
    return {
      ...p,
      taken_at: m.taken_at ?? p.taken_at,
      latitude: m.latitude ?? p.latitude,
      longitude: m.longitude ?? p.longitude,
      thumb: m.thumb ?? p.thumb,
      width: m.width ?? p.width,
      height: m.height ?? p.height
    }
  })
}

const enrichAll = async () => {
  const token = ++enrichAllToken
  const allPaths = photos.value.map((p) => p?.path).filter((p) => p && !enriched.has(p))
  const chunk = 300
  for (let i = 0; i < allPaths.length; i += chunk) {
    if (token !== enrichAllToken) return
    const batch = allPaths.slice(i, i + chunk)
    for (const p of batch) enriched.add(p)
    const metas = await invoke('life_album_enrich_files', { paths: batch }).catch((e) => {
      logger.error('EXIF 全量增强失败:', e)
      return []
    })
    if (token !== enrichAllToken) return
    mergeMetas(metas)
    await new Promise((r) => setTimeout(r, 0))
  }
  if (token === enrichAllToken) ensureCities()
}

const loadMore = () => {
  visibleLimit.value += 400
  enrichVisible()
}

const loadDir = async (dirPath, persist) => {
  enrichAllToken += 1
  albumDir.value = String(dirPath || '')
  visibleLimit.value = 400
  enriched.clear()
  cityByGeoKey.value = {}

  if (persist) {
    try { await invoke('set_life_album_album_dir', { path: albumDir.value }) }
    catch (e) { logger.error('保存相册目录失败:', e) }
  }

  loading.value = true
  try {
    const list = await invoke('life_album_scan_dir', { dirPath: albumDir.value })
    photos.value = Array.isArray(list) ? list : []
    message.success(`已加载 ${photos.value.length} 张照片`)
    enrichVisible()
    enrichAll()
  } catch (e) {
    logger.error('加载目录失败:', e)
    message.error('加载目录失败')
  } finally {
    loading.value = false
  }
}

const pickDir = async () => {
  const selected = await open({ directory: true, multiple: false })
  if (!selected) return
  await loadDir(selected, true)
}

const importFiles = async () => {
  try {
    const selected = await open({
      multiple: true,
      filters: [{ name: 'Images', extensions: ['jpg', 'jpeg', 'png', 'webp'] }]
    })
    if (!selected) return
    const paths = Array.isArray(selected) ? selected : [selected]
    loading.value = true
    const list = await invoke('life_album_import_files', { paths })
    photos.value = Array.isArray(list) ? list : []
    visibleLimit.value = 400
    enriched.clear()
    cityByGeoKey.value = {}
    message.success(`已导入 ${photos.value.length} 张照片`)
    ensureCities()
  } catch (e) {
    logger.error('导入失败:', e)
    message.error('导入失败')
  } finally {
    loading.value = false
  }
}

// ---------- 联动 ----------
watch(viewMode, async (v) => {
  if (v === 'location') {
    await ensureCities()
    refreshMapMarkers()
  } else if (v === 'timeline') {
    teardownMap()
    setupSpy()
  }
})
watch(photos, () => {
  if (viewMode.value === 'location') refreshMapMarkers()
  else setupSpy()
})
watch(cityByGeoKey, () => {
  if (viewMode.value === 'location') refreshMapMarkers()
})

watch(detailVisible, async (v) => {
  if (!v) {
    if (viewerResizeObserver) { viewerResizeObserver.disconnect(); viewerResizeObserver = null }
    viewerStageSize.value = { w: 0, h: 0 }
    viewerImgNatural.value = { w: 0, h: 0 }
    return
  }
  await nextTick()
  const el = viewerStageEl.value
  if (!el) return
  const upd = () => {
    const r = el.getBoundingClientRect()
    viewerStageSize.value = { w: r.width, h: r.height }
  }
  upd()
  viewerResizeObserver = new ResizeObserver(upd)
  viewerResizeObserver.observe(el)
})

onMounted(async () => {
  setupSpy()
  loadAmapKey()
  try {
    const dir = await invoke('get_life_album_album_dir')
    if (dir) await loadDir(dir, false)
  } catch (e) {
    logger.error('读取相册目录失败:', e)
  }
})
onUnmounted(() => {
  teardownMap()
  if (spy) spy.disconnect()
})
</script>

<style scoped>
.journal {
  --paper: #f5efe4;
  --paper-2: #efe7d8;
  --card: #fbf7ef;
  --ink: #2a2521;
  --ink-2: #6d6355;
  --ink-3: #9a8f7d;
  --accent: #b1533a;
  --accent-2: #d98b6f;
  --gold: #a8814a;
  --line: #ddd0bb;
  --line-2: #e8ddca;
  --serif: Georgia, 'Songti SC', 'STSong', 'SimSun', serif;
  --sans: 'Avenir Next', 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', system-ui, sans-serif;

  position: relative;
  height: 100%;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  padding: 22px 26px 0;
  overflow: hidden;
  color: var(--ink);
  font-family: var(--sans);
  background:
    radial-gradient(1100px 460px at 108% -12%, #fcf7ed 0%, rgba(252, 247, 237, 0) 60%),
    radial-gradient(900px 500px at -10% 110%, #f0e7d6 0%, rgba(240, 231, 214, 0) 55%),
    var(--paper);
}
.journal::before {
  content: '';
  position: absolute;
  inset: 0;
  pointer-events: none;
  z-index: 0;
  opacity: 0.5;
  mix-blend-mode: multiply;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='140' height='140'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='2' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='0.05'/%3E%3C/svg%3E");
}
.journal > * { position: relative; z-index: 1; }

/* 报头 */
.masthead {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  padding-bottom: 14px;
}
.kicker {
  font-size: 11px;
  letter-spacing: 0.42em;
  color: var(--accent);
  font-weight: 600;
  margin-bottom: 2px;
}
.wordmark {
  margin: 0;
  font-family: var(--serif);
  font-size: 34px;
  font-weight: 600;
  letter-spacing: 0.14em;
  line-height: 1;
}
.dateline {
  margin-top: 6px;
  font-size: 12px;
  color: var(--ink-3);
  max-width: 44vw;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.masthead-right { display: flex; align-items: center; gap: 10px; }
.link-btn {
  border: 0;
  background: none;
  font-family: var(--sans);
  font-size: 13px;
  color: var(--ink-2);
  cursor: pointer;
  padding: 2px 0;
  border-bottom: 1px solid transparent;
  transition: color 0.18s, border-color 0.18s;
}
.link-btn:hover { color: var(--accent); border-color: var(--accent); }
.link-btn:disabled { opacity: 0.4; cursor: default; }
.dot-sep { color: var(--ink-3); }
/* 控制栏 */
.controlbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 0 14px;
  border-bottom: 1px solid var(--line);
}
.segmented { position: relative; display: inline-flex; gap: 2px; }
.seg {
  position: relative;
  border: 0;
  background: none;
  cursor: pointer;
  font-family: var(--serif);
  font-size: 16px;
  letter-spacing: 0.08em;
  color: var(--ink-3);
  padding: 4px 14px 8px;
  transition: color 0.2s;
}
.seg.active { color: var(--ink); }
.seg:hover { color: var(--accent); }
.seg-ink {
  position: absolute;
  bottom: 0;
  height: 2px;
  width: 44px;
  background: var(--accent);
  border-radius: 2px;
  transition: transform 0.32s cubic-bezier(0.6, 0, 0.2, 1);
}
.seg-ink.timeline { transform: translateX(14px); }
.seg-ink.location { transform: translateX(96px); }
.counts { font-size: 12px; color: var(--ink-2); letter-spacing: 0.03em; }
.count-num { font-family: var(--serif); font-size: 15px; color: var(--accent); }

/* 状态 */
.state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--ink-2);
}
.spinner {
  width: 22px; height: 22px; border-radius: 50%;
  border: 2px solid var(--line); border-top-color: var(--accent);
  animation: spin 0.8s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }
.empty-mark { font-size: 40px; color: var(--accent-2); }
.empty-title { font-family: var(--serif); font-size: 20px; color: var(--ink); }
.empty-hint { font-size: 13px; color: var(--ink-3); }

/* 时间线 */
.timeline { flex: 1; min-height: 0; display: flex; gap: 22px; padding-top: 16px; }
.spine {
  position: relative;
  width: 74px;
  flex: 0 0 74px;
  overflow-y: auto;
  padding: 4px 0 20px;
  scrollbar-width: none;
}
.spine::-webkit-scrollbar { display: none; }
.spine-line {
  position: absolute; left: 12px; top: 0; bottom: 0;
  width: 1px; background: linear-gradient(var(--line), transparent);
}
.spine-node {
  position: relative;
  display: flex; flex-direction: column; align-items: flex-start;
  gap: 1px; border: 0; background: none; cursor: pointer;
  padding: 8px 0 8px 26px; width: 100%; text-align: left;
}
.spine-dot {
  position: absolute; left: 8px; top: 12px;
  width: 8px; height: 8px; border-radius: 50%;
  background: var(--paper); border: 1.5px solid var(--line);
  transition: all 0.2s;
}
.spine-node.active .spine-dot { background: var(--accent); border-color: var(--accent); transform: scale(1.15); }
.spine-year { font-family: var(--serif); font-size: 15px; color: var(--ink); }
.spine-month { font-size: 11px; color: var(--ink-3); }
.spine-node.active .spine-month { color: var(--accent); }
.stream {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: 0 8px 40px 0;
  scrollbar-width: thin;
}
.chapter { margin-bottom: 34px; content-visibility: auto; contain-intrinsic-size: 1px 640px; }
.chapter-head {
  position: sticky;
  top: 0;
  z-index: 3;
  padding: 6px 0 12px;
  margin-bottom: 14px;
  background: linear-gradient(var(--paper) 62%, rgba(245, 239, 228, 0));
}
.chapter-time { display: flex; align-items: baseline; gap: 10px; }
.chapter-month {
  font-family: var(--serif);
  font-size: 30px;
  font-weight: 600;
  letter-spacing: 0.06em;
  color: var(--ink);
}
.chapter-year { font-family: var(--serif); font-size: 18px; color: var(--accent); letter-spacing: 0.12em; }
.chapter-meta { display: flex; align-items: center; gap: 12px; margin-top: 3px; }
.chapter-place {
  font-size: 12px; letter-spacing: 0.14em; text-transform: uppercase; color: var(--ink-2);
}
.chapter-count { font-size: 12px; color: var(--ink-3); }
.chapter-rule { margin-top: 10px; height: 1px; background: var(--line); }

/* 杂志式错落瀑布（CSS 多列，保留原始宽高比） */
.mosaic { columns: 4 200px; column-gap: 12px; }
@media (max-width: 1400px) { .mosaic { columns: 3 200px; } }
@media (max-width: 980px) { .mosaic { columns: 2 180px; } }
.frame {
  position: relative;
  break-inside: avoid;
  margin: 0 0 12px;
  border-radius: 3px;
  overflow: hidden;
  background: var(--card);
  box-shadow: 0 1px 2px rgba(60, 45, 30, 0.08);
  cursor: pointer;
  transition: transform 0.28s cubic-bezier(0.2, 0.7, 0.2, 1), box-shadow 0.28s;
  animation: rise 0.5s both;
}
@keyframes rise { from { opacity: 0; transform: translateY(10px); } to { opacity: 1; transform: none; } }
.frame::after {
  content: '';
  position: absolute; inset: 0;
  border: 1px solid rgba(255, 255, 255, 0.5);
  border-radius: 3px; pointer-events: none;
}
.frame-img { display: block; width: 100%; height: auto; }
.frame.wide { outline: 3px solid var(--card); }
.frame:hover { transform: translateY(-4px); box-shadow: 0 14px 30px rgba(60, 40, 25, 0.2); }
.frame-cap {
  position: absolute;
  left: 0; right: 0; bottom: 0;
  padding: 22px 10px 8px;
  display: flex; align-items: center; justify-content: space-between; gap: 8px;
  background: linear-gradient(transparent, rgba(30, 22, 16, 0.7));
  color: #f6efe2;
  opacity: 0; transform: translateY(6px);
  transition: opacity 0.24s, transform 0.24s;
}
.frame:hover .frame-cap { opacity: 1; transform: none; }
.cap-time { font-size: 11px; letter-spacing: 0.05em; font-variant-numeric: tabular-nums; }
.cap-place { font-size: 11px; color: var(--accent-2); max-width: 60%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.load-more { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 18px 0 8px; }
.ghost-btn {
  border: 1px solid var(--line);
  background: var(--card);
  font-family: var(--serif); font-size: 14px; letter-spacing: 0.06em;
  color: var(--ink); padding: 8px 22px; border-radius: 2px; cursor: pointer;
  transition: all 0.2s;
}
.ghost-btn:hover { border-color: var(--accent); color: var(--accent); }
.load-more-hint { font-size: 11px; color: var(--ink-3); }
/* 地图视图 */
.atlas { flex: 1; min-height: 0; display: flex; gap: 20px; padding: 16px 0 20px; }
.map-frame {
  position: relative;
  flex: 1 1 64%;
  border-radius: 4px;
  overflow: hidden;
  background: var(--paper-2);
  box-shadow: 0 2px 10px rgba(60, 45, 30, 0.12);
  border: 1px solid var(--line);
}
.map { position: absolute; inset: 0; }
.map-empty {
  position: absolute; inset: 0;
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  gap: 10px; color: var(--ink-2);
  background: rgba(245, 239, 228, 0.86); backdrop-filter: blur(4px);
}
.atlas-chapters {
  flex: 0 0 34%;
  min-width: 260px;
  overflow-y: auto;
  padding-right: 6px;
  display: flex; flex-direction: column; gap: 16px;
  scrollbar-width: thin;
}
.city-card { border-bottom: 1px solid var(--line); padding-bottom: 14px; }
.city-head {
  display: flex; align-items: baseline; justify-content: space-between;
  cursor: pointer; margin-bottom: 8px;
}
.city-name {
  font-family: var(--serif); font-size: 18px; color: var(--ink);
  border-bottom: 1px solid transparent; transition: color 0.18s, border-color 0.18s;
}
.city-head:hover .city-name { color: var(--accent); border-color: var(--accent); }
.city-count {
  font-size: 11px; color: #fff; background: var(--accent);
  border-radius: 10px; padding: 1px 8px; font-variant-numeric: tabular-nums;
}
.city-strip { display: flex; gap: 6px; overflow-x: auto; padding-bottom: 4px; scrollbar-width: thin; }
.strip-thumb {
  flex: 0 0 auto; width: 72px; height: 72px; border-radius: 3px; overflow: hidden;
  cursor: pointer; box-shadow: 0 1px 2px rgba(60, 45, 30, 0.12);
}
.strip-thumb img { width: 100%; height: 100%; object-fit: cover; transition: transform 0.3s; }
.strip-thumb:hover img { transform: scale(1.08); }

/* 查看器 */
.viewer-modal :deep(.n-card) { height: 100vh; display: flex; flex-direction: column; }
.viewer-modal :deep(.n-card__content) { flex: 1; min-height: 0; padding: 0; position: relative; }
.viewer {
  position: absolute; inset: 0;
  display: flex; align-items: center; justify-content: center;
  background: #14100c; overflow: hidden;
}
.viewer-bar {
  position: absolute; top: 14px; left: 14px; right: 14px; z-index: 2;
  display: flex; align-items: center; justify-content: space-between;
  padding: 10px 14px; border-radius: 4px;
  background: rgba(20, 15, 10, 0.55);
  border: 1px solid rgba(216, 180, 140, 0.16);
  backdrop-filter: blur(8px);
}
.viewer-info { display: flex; flex-direction: column; gap: 2px; color: #f0e7d6; }
.viewer-time { font-family: var(--serif); font-size: 15px; letter-spacing: 0.04em; }
.viewer-place { font-size: 11px; letter-spacing: 0.16em; text-transform: uppercase; color: var(--accent-2); }
.viewer-tools { display: flex; gap: 6px; }
.vtool {
  border: 1px solid rgba(216, 180, 140, 0.22); background: rgba(255, 255, 255, 0.04);
  color: #ecdfca; font-family: var(--sans); font-size: 13px; min-width: 34px; height: 30px;
  padding: 0 10px; border-radius: 3px; cursor: pointer; transition: all 0.18s;
}
.vtool:hover { background: rgba(216, 140, 90, 0.28); border-color: var(--accent-2); color: #fff; }
.vtool.close { border-color: rgba(200, 90, 60, 0.5); color: #f0b8a6; }
.vtool.close:hover { background: var(--accent); color: #fff; }
.viewer-img {
  width: auto; height: auto; max-width: none; max-height: none;
  display: block; will-change: transform; user-select: none;
}
/* 地图上的地名服务 HUD */
.geo-hud {
  position: absolute;
  top: 12px;
  left: 12px;
  z-index: 500;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  border-radius: 3px;
  background: rgba(251, 247, 239, 0.92);
  border: 1px solid var(--line);
  box-shadow: 0 2px 8px rgba(60, 45, 30, 0.14);
  backdrop-filter: blur(6px);
}
.geo-dot { width: 7px; height: 7px; border-radius: 50%; background: var(--ink-3); }
.geo-dot.on { background: #4c9a5a; box-shadow: 0 0 0 3px rgba(76, 154, 90, 0.18); }
.geo-text { font-size: 12px; color: var(--ink-2); letter-spacing: 0.02em; }
.geo-edit {
  border: 0; background: none; cursor: pointer;
  font-size: 12px; color: var(--accent);
  border-bottom: 1px solid transparent; padding: 0 0 1px;
}
.geo-edit:hover { border-color: var(--accent); }
.geo-pop {
  position: absolute; top: calc(100% + 8px); left: 0;
  width: 300px; padding: 12px;
  background: var(--card); border: 1px solid var(--line); border-radius: 4px;
  box-shadow: 0 12px 28px rgba(60, 40, 20, 0.22);
  display: flex; flex-wrap: wrap; gap: 8px;
}
.geo-input {
  flex: 1 1 190px; min-width: 0;
  border: 1px solid var(--line); border-radius: 3px;
  padding: 7px 9px; font-size: 12px; font-family: var(--sans);
  background: #fff; color: var(--ink); outline: none;
}
.geo-input:focus { border-color: var(--accent); }
.geo-save {
  border: 0; background: var(--accent); color: #fff;
  font-size: 12px; padding: 0 14px; border-radius: 3px; cursor: pointer;
}
.geo-save:hover { background: #9c452f; }
.geo-hint { flex: 1 1 100%; font-size: 11px; line-height: 1.5; color: var(--ink-3); }
.geo-clear {
  flex: 1 1 100%;
  border: 1px solid var(--line);
  background: none;
  color: var(--ink-2);
  font-family: var(--sans);
  font-size: 12px;
  padding: 6px 0;
  border-radius: 3px;
  cursor: pointer;
  transition: color 0.18s, border-color 0.18s;
}
.geo-clear:hover:not(:disabled) { color: var(--accent); border-color: var(--accent); }
.geo-clear:disabled { opacity: 0.5; cursor: default; }

/* 地图 marker / 聚合（leaflet 注入的 DOM，用 :deep 穿透） */
.map :deep(.pin-wrap) { background: none; border: 0; }
.map :deep(.pin) {
  width: 16px; height: 16px; border-radius: 50% 50% 50% 0;
  transform: rotate(-45deg);
  background: var(--accent);
  border: 2px solid #fbf7ef;
  box-shadow: 0 2px 5px rgba(40, 20, 10, 0.4);
}
.map :deep(.cl-wrap) { background: none; border: 0; }
.map :deep(.cl-badge) {
  width: 100%; height: 100%; border-radius: 50%;
  display: flex; align-items: center; justify-content: center;
  color: #fff; font-family: var(--sans); font-weight: 600; font-size: 13px;
  background: radial-gradient(circle at 35% 30%, var(--accent-2), var(--accent));
  border: 2px solid rgba(251, 247, 239, 0.9);
  box-shadow: 0 3px 10px rgba(60, 30, 15, 0.35);
}
.map :deep(.leaflet-container) { background: var(--paper-2); font-family: var(--sans); }
</style>
