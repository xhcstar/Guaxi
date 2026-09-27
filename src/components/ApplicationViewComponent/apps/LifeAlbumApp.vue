<template>
  <div class="journal">
    <!-- 顶部报头 -->
    <header class="masthead">
      <div class="masthead-lead">
        <div class="kicker">TRAVEL · JOURNAL</div>
        <div class="title-row">
          <h1 class="wordmark">旅行志</h1>
        </div>
      </div>
      <div class="actions">
        <button class="icon-btn" :disabled="loading" title="选择相册目录" @click="pickDir">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z" />
          </svg>
        </button>
        <button class="icon-btn" :disabled="loading" title="导入照片" @click="importFiles">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 12V6a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h7" />
            <circle cx="8" cy="9" r="1.6" />
            <path d="M3 16l4-3 3.6 2.7" />
            <path d="M19 15v6M16 18h6" />
          </svg>
        </button>
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
              <img v-if="!isVideo(p) || p.thumb" class="frame-img" :src="thumbSrc(p)" loading="lazy" decoding="async" />
              <div v-else class="frame-img video-placeholder"></div>
              <template v-if="isVideo(p)">
                <span class="play-badge">▶</span>
                <span v-if="formatDuration(p.duration)" class="dur-chip">{{ formatDuration(p.duration) }}</span>
              </template>
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
              <img v-if="!isVideo(p) || p.thumb" :src="thumbSrc(p)" loading="lazy" decoding="async" />
              <div v-else class="strip-video-ph"></div>
              <span v-if="isVideo(p)" class="strip-play">▶</span>
            </div>
          </div>
        </article>
      </div>
    </div>
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
const VIDEO_EXTS = ['mp4', 'mov', 'm4v', 'webm']
const extOf = (p) => {
  const s = String(p || '')
  const i = s.lastIndexOf('.')
  return i >= 0 ? s.slice(i + 1).toLowerCase() : ''
}
// 视频判定:优先看后端给的 media_type,老数据/裸对象按扩展名兜底。
const isVideo = (p) => (p?.media_type ? p.media_type === 'video' : VIDEO_EXTS.includes(extOf(p?.path)))
// 缩略图源:视频只认已生成的海报(没海报就返回空串走占位),绝不把视频路径塞进 <img> 变裂图;
// 图片保持 thumb || 原图。
const thumbSrc = (p) => {
  if (isVideo(p)) return p?.thumb ? toSrc(p.thumb) : ''
  return toSrc(p?.thumb || p?.path)
}
const formatDuration = (sec) => {
  const s = Math.max(0, Math.round(Number(sec) || 0))
  if (!s) return ''
  const m = Math.floor(s / 60)
  const r = s % 60
  return `${m}:${String(r).padStart(2, '0')}`
}

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

// ---------- 全屏查看器（独立窗口） ----------
// 整册按时间线倒序串成一条，供查看器窗口左右翻页（与时间线阅读顺序一致）。
const orderedPhotos = computed(() => photos.value.slice().sort(sortByTimeDesc))

const openDetail = (p) => {
  if (!p?.path) return
  const ordered = orderedPhotos.value
  let index = ordered.findIndex((x) => x.path === p.path)
  if (index < 0) index = 0
  const payload = ordered.map((x) => ({
    path: x.path,
    thumb: x.thumb ?? null,
    taken_at: x.taken_at ?? null,
    latitude: typeof x.latitude === 'number' ? x.latitude : null,
    longitude: typeof x.longitude === 'number' ? x.longitude : null,
    width: typeof x.width === 'number' ? x.width : null,
    height: typeof x.height === 'number' ? x.height : null,
    file_name: x.file_name || '',
    city: cityFor(x) || '',
    media_type: x.media_type || (isVideo(x) ? 'video' : 'image'),
    duration: typeof x.duration === 'number' ? x.duration : null
  }))
  invoke('open_life_album_viewer_window', {
    request: { photos: payload, index },
    theme: 'dark'
  }).catch((e) => {
    logger.error('打开查看器窗口失败:', e)
    message.error('打开查看器失败')
  })
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
  posterVisible()
}

const mergeMetas = (metas) => {
  const map = new Map((Array.isArray(metas) ? metas : []).map((m) => [m.path, m]))
  if (!map.size) return
  // 原地 patch 命中的照片对象，而不是整数组重建。整数组重建会让 timelineGroups / geoPhotosAll
  // 全量重算、并触发 watch(photos) → setupSpy 的渲染风暴（每批 enrich 都要重排 400 个瓦片）。
  // Vue 的深响应会精确地只更新真正用到这些字段的绑定。
  for (const p of photos.value) {
    const m = map.get(p.path)
    if (!m) continue
    if (m.taken_at != null) p.taken_at = m.taken_at
    if (m.latitude != null) p.latitude = m.latitude
    if (m.longitude != null) p.longitude = m.longitude
    if (m.thumb != null) p.thumb = m.thumb
    if (m.width != null) p.width = m.width
    if (m.height != null) p.height = m.height
    if (m.media_type != null) p.media_type = m.media_type
    if (m.duration != null) p.duration = m.duration
  }
}

// ---------- 视频海报(前端抽首帧) ----------
// Rust 无 ffmpeg,视频海报只能靠 WebView 解码:离屏 <video> seek 到 ~0.1s → <canvas>
// 截图 → JPEG 回传 Rust 落盘缓存。能出海报的视频恰好就是能播放的视频,行为一致。
const POSTER_EDGE = 480 // 海报短边像素
const posterTried = new Set() // 本会话已尝试(成功/失败)过的视频路径,不重复抽帧
const posterQueue = []
let posterActive = 0
const POSTER_CONCURRENCY = 2

const generateVideoPoster = (p) =>
  new Promise((resolve) => {
    const src = toSrc(p?.path)
    if (!src) return resolve(false)
    const video = document.createElement('video')
    video.muted = true
    video.preload = 'metadata'
    video.crossOrigin = 'anonymous'
    video.playsInline = true
    let done = false
    const cleanup = () => {
      video.removeAttribute('src')
      try { video.load() } catch {}
    }
    const fail = () => {
      if (done) return
      done = true
      cleanup()
      resolve(false)
    }
    const timer = setTimeout(fail, 15000)
    video.addEventListener('loadeddata', () => {
      // seek 一点点,避开某些视频第 0 帧是纯黑的情况。
      try { video.currentTime = Math.min(0.1, (video.duration || 1) / 2) } catch { fail() }
    })
    video.addEventListener('seeked', async () => {
      if (done) return
      try {
        const vw = video.videoWidth || 0
        const vh = video.videoHeight || 0
        if (!vw || !vh) return fail()
        const scale = POSTER_EDGE / Math.min(vw, vh)
        const cw = Math.round(vw * Math.min(1, scale))
        const ch = Math.round(vh * Math.min(1, scale))
        const canvas = document.createElement('canvas')
        canvas.width = cw
        canvas.height = ch
        const ctx = canvas.getContext('2d')
        ctx.drawImage(video, 0, 0, cw, ch)
        const dataUrl = canvas.toDataURL('image/jpeg', 0.82)
        const duration = Number.isFinite(video.duration) ? video.duration : null
        clearTimeout(timer)
        done = true
        cleanup()
        const thumb = await invoke('life_album_save_video_poster', {
          path: p.path,
          jpegBase64: dataUrl,
          width: cw,
          height: ch,
          duration
        })
        // 原地 patch:海报出现 + 尺寸/时长补齐(触发瓦片重排为正确宽高比)。
        for (const x of photos.value) {
          if (x.path === p.path) {
            if (typeof thumb === 'string' && thumb) x.thumb = thumb
            x.width = vw
            x.height = vh
            if (duration != null) x.duration = duration
            x.media_type = 'video'
            break
          }
        }
        resolve(true)
      } catch (e) {
        logger.warn('生成视频海报失败:', p?.path, e)
        clearTimeout(timer)
        fail()
      }
    })
    video.addEventListener('error', () => { clearTimeout(timer); fail() })
    video.src = src
  })

const pumpPosterQueue = () => {
  while (posterActive < POSTER_CONCURRENCY && posterQueue.length) {
    const p = posterQueue.shift()
    posterActive += 1
    generateVideoPoster(p).finally(() => {
      posterActive -= 1
      pumpPosterQueue()
    })
  }
}

// 只给可见范围内、还没海报、且本会话没尝试过的视频排队抽帧。
const posterVisible = () => {
  for (const p of visiblePhotos.value) {
    if (!isVideo(p) || p.thumb || posterTried.has(p.path)) continue
    posterTried.add(p.path)
    posterQueue.push(p)
  }
  pumpPosterQueue()
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
      filters: [
        { name: 'Media', extensions: ['jpg', 'jpeg', 'png', 'webp', 'mp4', 'mov', 'm4v', 'webm'] },
        { name: 'Images', extensions: ['jpg', 'jpeg', 'png', 'webp'] },
        { name: 'Videos', extensions: ['mp4', 'mov', 'm4v', 'webm'] }
      ]
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
// photos 现在按元素原地更新（见 mergeMetas），watch(photos) 不再随 enrich 逐批触发。
// 改为监听派生结果：章节结构变化 → 重建滚动监听；地理点变化 → 刷新地图标记。
// 首次加载 / 导入目录（整数组重建）也会让这两个计算属性变化，从而完成初始化。
watch(timelineGroups, () => {
  if (viewMode.value === 'timeline') setupSpy()
  posterVisible()
})
watch(geoPhotosAll, () => {
  if (viewMode.value === 'location') refreshMapMarkers()
})
watch(cityByGeoKey, () => {
  if (viewMode.value === 'location') refreshMapMarkers()
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
  padding: 14px 26px 0;
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
  align-items: flex-start;
  justify-content: space-between;
  padding-bottom: 8px;
}
.kicker {
  font-size: 11px;
  letter-spacing: 0.42em;
  color: var(--accent);
  font-weight: 600;
  margin-bottom: 3px;
}
.title-row {
  display: flex;
  align-items: center;
  gap: 16px;
}
.wordmark {
  margin: 0;
  font-family: var(--serif);
  font-size: 30px;
  font-weight: 600;
  letter-spacing: 0.14em;
  line-height: 1;
}
.actions { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }
.icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  padding: 0;
  border: 1px solid var(--line);
  border-radius: 3px;
  background: var(--card);
  color: var(--ink-2);
  cursor: pointer;
  transition: color 0.18s, border-color 0.18s, background 0.18s, transform 0.18s;
}
.icon-btn:hover { color: var(--accent); border-color: var(--accent); transform: translateY(-1px); }
.icon-btn:disabled { opacity: 0.4; cursor: default; transform: none; }
.icon-btn svg { width: 18px; height: 18px; }
.dot-sep { color: var(--ink-3); }
/* 控制栏 */
.controlbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 2px 0 10px;
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
  padding: 4px 14px 10px;
  transition: color 0.2s;
}
.seg.active { color: var(--ink); }
.seg:hover { color: var(--accent); }
.seg::after {
  content: '';
  position: absolute;
  left: 14px;
  right: 14px;
  bottom: 2px;
  height: 2px;
  border-radius: 2px;
  background: var(--accent);
  transform: scaleX(0);
  transform-origin: center;
  transition: transform 0.28s cubic-bezier(0.6, 0, 0.2, 1);
}
.seg.active::after { transform: scaleX(1); }
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
.timeline { flex: 1; min-height: 0; display: flex; gap: 22px; padding-top: 10px; }
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
  padding: 0 10px 40px 0;
  scrollbar-width: thin;
  scrollbar-color: var(--line) transparent;
  scrollbar-gutter: stable;
}
.stream::-webkit-scrollbar { width: 10px; }
.stream::-webkit-scrollbar-track { background: transparent; }
.stream::-webkit-scrollbar-thumb {
  background: var(--line);
  border-radius: 6px;
  border: 3px solid transparent;
  background-clip: padding-box;
}
.stream::-webkit-scrollbar-thumb:hover {
  background: var(--ink-3);
  background-clip: padding-box;
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
.frame-img.video-placeholder {
  aspect-ratio: 4 / 3;
  background: linear-gradient(135deg, #2a231c, #171310);
}
/* 视频角标:播放三角 + 时长胶囊 */
.play-badge {
  position: absolute;
  top: 50%; left: 50%;
  transform: translate(-50%, -50%);
  width: 42px; height: 42px;
  display: flex; align-items: center; justify-content: center;
  border-radius: 50%;
  background: rgba(20, 15, 10, 0.5);
  border: 1.5px solid rgba(255, 255, 255, 0.85);
  color: #fff;
  font-size: 15px;
  padding-left: 3px;
  backdrop-filter: blur(2px);
  pointer-events: none;
  transition: background 0.2s, transform 0.2s;
}
.frame:hover .play-badge { background: rgba(177, 83, 58, 0.72); transform: translate(-50%, -50%) scale(1.08); }
.dur-chip {
  position: absolute;
  right: 6px; top: 6px;
  padding: 1px 6px;
  border-radius: 3px;
  background: rgba(20, 15, 10, 0.72);
  color: #f6efe2;
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.02em;
  pointer-events: none;
}
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
.atlas { flex: 1; min-height: 0; display: flex; gap: 20px; padding: 10px 0 20px; }
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
  position: relative;
  flex: 0 0 auto; width: 72px; height: 72px; border-radius: 3px; overflow: hidden;
  cursor: pointer; box-shadow: 0 1px 2px rgba(60, 45, 30, 0.12);
}
.strip-thumb img { width: 100%; height: 100%; object-fit: cover; transition: transform 0.3s; }
.strip-thumb:hover img { transform: scale(1.08); }
.strip-video-ph { width: 100%; height: 100%; background: linear-gradient(135deg, #2a231c, #171310); }
.strip-play {
  position: absolute;
  top: 50%; left: 50%;
  transform: translate(-50%, -50%);
  width: 24px; height: 24px;
  display: flex; align-items: center; justify-content: center;
  border-radius: 50%;
  background: rgba(20, 15, 10, 0.5);
  border: 1px solid rgba(255, 255, 255, 0.85);
  color: #fff; font-size: 9px; padding-left: 2px;
  pointer-events: none;
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
