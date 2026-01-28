<template>
  <div class="album-root">
    <div class="album-toolbar">
      <div class="album-title">
        <div class="album-title-main">相册</div>
        <div v-if="albumDir" class="album-title-sub" :title="albumDir">{{ albumDir }}</div>
      </div>
      <div class="album-actions">
        <n-button size="small" tertiary :loading="loading" @click="pickDir">选择相册目录</n-button>
        <n-button size="small" secondary :loading="loading" @click="importFiles">导入照片</n-button>
      </div>
    </div>

    <div class="album-subbar">
      <n-radio-group v-model:value="viewMode" size="small">
        <n-radio-button value="timeline">时间线</n-radio-button>
        <n-radio-button value="location">地理位置</n-radio-button>
      </n-radio-group>
      <div class="album-stats">{{ photos.length }} 张</div>
    </div>

    <div v-if="loading" class="album-loading">
      <n-spin size="small" />
      <span class="album-loading-text">处理中...</span>
    </div>

    <div v-else-if="!photos.length" class="album-empty">
      <n-empty description="请选择相册目录或导入照片" />
    </div>

    <div v-else class="album-body" :class="{ 'album-body-location': viewMode === 'location' }">
      <template v-if="viewMode === 'timeline'">
        <div v-for="g in timelineGroups" :key="g.key" class="group">
          <div class="group-title">{{ g.title }}</div>
          <div class="grid">
            <div v-for="p in g.items" :key="p.path" class="tile" @click="openDetail(p)">
              <img class="thumb" :src="toSrc(p.path)" loading="lazy" />
            </div>
          </div>
        </div>

        <div v-if="photos.length > visibleLimit" class="load-more">
          <n-button size="small" tertiary @click="loadMore">加载更多</n-button>
          <div class="load-more-hint">已显示 {{ visibleLimit }} / {{ photos.length }}</div>
        </div>
      </template>

      <template v-else>
        <div class="location-view">
          <div class="map-card">
            <div ref="mapEl" class="map"></div>
            <div v-if="!geoPhotos.length" class="map-empty">
              <n-empty description="暂无带定位信息的照片" />
            </div>
          </div>

          <div class="location-list">
            <n-collapse accordion>
              <n-collapse-item v-for="g in locationGroups" :key="g.key" :title="g.title" :name="g.key">
                <div class="grid">
                  <div v-for="p in g.items" :key="p.path" class="tile" @click="openDetail(p)">
                    <img class="thumb" :src="toSrc(p.path)" loading="lazy" />
                  </div>
                </div>
              </n-collapse-item>
            </n-collapse>

            <div v-if="photos.length > visibleLimit" class="load-more">
              <n-button size="small" tertiary @click="loadMore">加载更多</n-button>
              <div class="load-more-hint">已显示 {{ visibleLimit }} / {{ photos.length }}</div>
            </div>
          </div>
        </div>
      </template>
    </div>

    <n-modal
      v-model:show="detailVisible"
      preset="card"
      class="viewer-modal"
      style="width: 100vw; height: 100vh; max-width: 100vw; max-height: 100vh; margin: 0;"
    >
      <div
        v-if="detail"
        ref="viewerStageEl"
        class="viewer-full"
        @click="detailVisible = false"
        @wheel.prevent="onViewerWheel"
      >
        <div class="viewer-toolbar" @click.stop>
          <div class="viewer-toolbar-left">
            <n-button size="tiny" tertiary @click="viewerZoomOut">-</n-button>
            <n-button size="tiny" tertiary @click="viewerZoomIn">+</n-button>
            <n-button size="tiny" tertiary @click="viewerRotateLeft">左转</n-button>
            <n-button size="tiny" tertiary @click="viewerRotateRight">右转</n-button>
            <n-button size="tiny" tertiary @click="viewerReset">重置</n-button>
          </div>
          <div class="viewer-toolbar-right">
            <n-button size="tiny" secondary type="error" @click="detailVisible = false">退出</n-button>
          </div>
        </div>

        <img
          class="viewer-full-img"
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
import { computed, ref, watch, nextTick, onMounted, onUnmounted } from 'vue'
import { createDiscreteApi } from 'naive-ui'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

import 'leaflet/dist/leaflet.css'
import L from 'leaflet'
import markerIcon2x from 'leaflet/dist/images/marker-icon-2x.png'
import markerIcon from 'leaflet/dist/images/marker-icon.png'
import markerShadow from 'leaflet/dist/images/marker-shadow.png'

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

const viewerStageEl = ref(null)
let viewerResizeObserver = null

const viewerZoom = ref(1)
const viewerRotation = ref(0)
const viewerStageSize = ref({ w: 0, h: 0 })
const viewerImgNatural = ref({ w: 0, h: 0 })

const onViewerImgLoad = (e) => {
  const el = e?.target
  const w = Number(el?.naturalWidth) || 0
  const h = Number(el?.naturalHeight) || 0
  viewerImgNatural.value = { w, h }
}

const fitScale = computed(() => {
  const stage = viewerStageSize.value
  const img = viewerImgNatural.value
  if (!stage.w || !stage.h || !img.w || !img.h) return 1

  const rad = (Math.abs(viewerRotation.value) % 360) * (Math.PI / 180)
  const c = Math.abs(Math.cos(rad))
  const s = Math.abs(Math.sin(rad))

  const bbW = img.w * c + img.h * s
  const bbH = img.w * s + img.h * c
  if (!bbW || !bbH) return 1

  const fit = Math.min(stage.w / bbW, stage.h / bbH)
  return Number.isFinite(fit) && fit > 0 ? fit : 1
})

const viewerReady = computed(() => {
  const stage = viewerStageSize.value
  const img = viewerImgNatural.value
  return Boolean(stage.w && stage.h && img.w && img.h)
})

const viewerImgStyle = computed(() => {
  const z = Math.min(8, Math.max(0.1, viewerZoom.value))
  const sc = z * fitScale.value
  return {
    transformOrigin: 'center center',
    transform: `scale(${sc}) rotate(${viewerRotation.value}deg)`,
    opacity: viewerReady.value ? 1 : 0
  }
})

const viewerReset = () => {
  viewerZoom.value = 1
  viewerRotation.value = 0
}

const viewerZoomIn = () => {
  viewerZoom.value = Math.min(8, viewerZoom.value * 1.12)
}

const viewerZoomOut = () => {
  viewerZoom.value = Math.max(0.1, viewerZoom.value / 1.12)
}

const viewerRotateLeft = () => {
  viewerRotation.value -= 90
}

const viewerRotateRight = () => {
  viewerRotation.value += 90
}

const onViewerWheel = (e) => {
  if (e?.deltaY < 0) viewerZoomIn()
  else viewerZoomOut()
}

const enriched = new Set()
const enrichingAll = ref(false)
let enrichAllToken = 0

const cityByCoordKey = ref({})
const visiblePhotos = computed(() => photos.value.slice(0, visibleLimit.value))

const toSrc = (p) => {
  const raw = String(p || '').trim()
  if (!raw) return ''

  if (/^(https?:|data:|blob:)/i.test(raw)) return raw

  try {
    const assetUrl = convertFileSrc(raw)
    return typeof assetUrl === 'string' && assetUrl ? assetUrl : raw
  } catch (e) {
    console.warn('[LifeAlbum] convertFileSrc failed:', e)
    return raw
  }
}

const mapEl = ref(null)
let map = null
let markerLayer = null

const mapViewBounds = ref(null)

const teardownMap = () => {
  if (!map) return
  map.remove()
  map = null
  markerLayer = null
  mapViewBounds.value = null
}

const ensureMap = async () => {
  await nextTick()
  if (!mapEl.value) return
  if (map) return

  map = L.map(mapEl.value, { zoomControl: true })
  map.on('moveend zoomend', () => {
    try {
      mapViewBounds.value = map.getBounds()
    } catch {
      mapViewBounds.value = null
    }
  })

  const osm = L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
    maxZoom: 19,
    attribution: '&copy; OpenStreetMap'
  })

  const amap = L.tileLayer('https://webrd0{s}.is.autonavi.com/appmaptile?style=7&x={x}&y={y}&z={z}', {
    subdomains: ['1', '2', '3', '4'],
    maxZoom: 19,
    attribution: '高德地图'
  })

  let fallbacked = false
  amap.on('tileerror', () => {
    if (fallbacked) return
    fallbacked = true
    try {
      map.removeLayer(amap)
    } catch {}
    osm.addTo(map)
  })

  amap.addTo(map)
  markerLayer = L.layerGroup().addTo(map)
}

const avgCoord = (items) => {
  const pts = items
    .map((p) => ({ lat: p?.latitude, lon: p?.longitude }))
    .filter((v) => typeof v.lat === 'number' && typeof v.lon === 'number')
  if (!pts.length) return null

  const sum = pts.reduce(
    (acc, v) => ({ lat: acc.lat + v.lat, lon: acc.lon + v.lon }),
    { lat: 0, lon: 0 }
  )
  return { lat: sum.lat / pts.length, lon: sum.lon / pts.length }
}

const parseTakenAt = (s) => {
  if (!s) return null
  const d = new Date(s)
  if (Number.isNaN(d.getTime())) return null
  return d
}

const dayKey = (p) => {
  const d = parseTakenAt(p?.taken_at)
  if (!d) return 'unknown'
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const dd = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${dd}`
}

const formatDay = (key) => {
  if (key === 'unknown') return '未知时间'
  const [y, m, d] = key.split('-')
  return `${y}年${m}月${d}日`
}

const formatTs = (s) => {
  const d = parseTakenAt(s)
  if (!d) return '未知'
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const dd = String(d.getDate()).padStart(2, '0')
  const hh = String(d.getHours()).padStart(2, '0')
  const mm = String(d.getMinutes()).padStart(2, '0')
  return `${y}-${m}-${dd} ${hh}:${mm}`
}

const coordKey = (p) => {
  const lat = p?.latitude
  const lon = p?.longitude
  if (typeof lat !== 'number' || typeof lon !== 'number') return 'unknown'
  return `${Number(lat).toFixed(4)},${Number(lon).toFixed(4)}`
}

const cityTitleFor = (key, count) => {
  if (key === 'unknown') return `未知位置 (${count})`
  const city = cityByCoordKey.value[key]
  return `${city || '定位中'} (${count})`
}

const sortByTimeDesc = (a, b) => {
  const ta = parseTakenAt(a?.taken_at)?.getTime?.() ?? 0
  const tb = parseTakenAt(b?.taken_at)?.getTime?.() ?? 0
  return tb - ta
}

const timelineGroups = computed(() => {
  const map = new Map()
  for (const p of visiblePhotos.value) {
    const k = dayKey(p)
    if (!map.has(k)) map.set(k, [])
    map.get(k).push(p)
  }
  const keys = Array.from(map.keys()).sort((a, b) => {
    if (a === 'unknown') return 1
    if (b === 'unknown') return -1
    return a < b ? 1 : -1
  })
  return keys.map((k) => ({
    key: k,
    title: formatDay(k),
    items: map.get(k).slice().sort(sortByTimeDesc)
  }))
})

const geoPhotosAll = computed(() =>
  photos.value.filter((p) => typeof p?.latitude === 'number' && typeof p?.longitude === 'number')
)

const geoPhotos = computed(() =>
  visiblePhotos.value.filter(
    (p) => typeof p?.latitude === 'number' && typeof p?.longitude === 'number'
  )
)

const buildLocationGroups = (list) => {
  const map = new Map()
  for (const p of list) {
    const k = coordKey(p)
    if (!map.has(k)) map.set(k, [])
    map.get(k).push(p)
  }
  const keys = Array.from(map.keys()).sort((a, b) => {
    if (a === 'unknown') return 1
    if (b === 'unknown') return -1
    return a < b ? -1 : 1
  })
  return keys.map((k) => ({
    key: k,
    title: cityTitleFor(k, map.get(k).length),
    items: map.get(k).slice().sort(sortByTimeDesc)
  }))
}

const locationGroupsAll = computed(() => buildLocationGroups(geoPhotosAll.value))

const parseCoordKeyValue = (key) => {
  if (typeof key !== 'string' || !key.includes(',')) return null
  const [a, b] = key.split(',')
  const lat = Number(a)
  const lon = Number(b)
  if (!Number.isFinite(lat) || !Number.isFinite(lon)) return null
  return { lat, lon }
}

const locationGroups = computed(() => {
  const bounds = mapViewBounds.value
  if (!bounds) return locationGroupsAll.value
  return locationGroupsAll.value.filter((g) => {
    if (g.key === 'unknown') return true
    const c = parseCoordKeyValue(g.key)
    if (!c) return true
    return bounds.contains(L.latLng(c.lat, c.lon))
  })
})

const refreshMapMarkers = async () => {
  if (viewMode.value !== 'location') return

  if (!geoPhotosAll.value.length) {
    teardownMap()
    return
  }

  await ensureMap()
  if (!map || !markerLayer) return

  markerLayer.clearLayers()

  const groups = locationGroupsAll.value.filter((g) => g.key !== 'unknown')

  const bounds = []
  for (const g of groups) {

    const c = avgCoord(g.items)
    if (!c) continue

    const lat = c.lat
    const lon = c.lon

    const first = g.items.slice().sort(sortByTimeDesc)[0]
    const title = cityTitleFor(g.key, g.items.length)

    const marker = L.marker([lat, lon])
    marker.bindPopup(title)
    marker.on('click', () => {
      if (first) openDetail(first)
    })
    marker.addTo(markerLayer)
    bounds.push([lat, lon])
  }

  await nextTick()
  map.invalidateSize()

  if (bounds.length === 1) {
    map.setView(bounds[0], 13)
  } else if (bounds.length > 1) {
    map.fitBounds(bounds, { padding: [24, 24], maxZoom: 15 })
  } else {
    map.setView([0, 0], 2)
  }

  try {
    mapViewBounds.value = map.getBounds()
  } catch {
    mapViewBounds.value = null
  }
}

watch(viewMode, async () => {
  if (viewMode.value === 'location') {
    await ensureCitiesForGeo()
    refreshMapMarkers()
  } else {
    teardownMap()
  }
})

watch(photos, () => {
  if (viewMode.value === 'location') {
    ensureCitiesForGeo()
    refreshMapMarkers()
  }
})

onUnmounted(() => {
  teardownMap()
})

const loadMore = () => {
  visibleLimit.value += 400
  enrichVisible()
  if (viewMode.value === 'location') ensureCitiesForGeo()
}

const openDetail = (p) => {
  detail.value = p
  viewerReset()
  detailVisible.value = true
}



const enrichVisible = async () => {
  const batch = visiblePhotos.value
    .map((p) => p?.path)
    .filter((p) => typeof p === 'string' && p && !enriched.has(p))
    .slice(0, 200)

  if (!batch.length) return

  for (const p of batch) enriched.add(p)

  try {
    const metas = await invoke('life_album_enrich_files', { paths: batch })
    const metaMap = new Map((Array.isArray(metas) ? metas : []).map((m) => [m.path, m]))
    photos.value = photos.value.map((p) => {
      const m = metaMap.get(p.path)
      if (!m) return p
      return {
        ...p,
        taken_at: m.taken_at ?? p.taken_at,
        latitude: m.latitude ?? p.latitude,
        longitude: m.longitude ?? p.longitude
      }
    })
  } catch (e) {
    console.error('EXIF 增强失败:', e)
  }
}

const enrichAll = async () => {
  if (enrichingAll.value) return

  const token = ++enrichAllToken
  enrichingAll.value = true

  try {
    const list = photos.value
    const indexByPath = new Map(list.map((p, i) => [p?.path, i]).filter(([k]) => k))

    const allPaths = list
      .map((p) => p?.path)
      .filter((p) => typeof p === 'string' && p && !enriched.has(p))

    const chunkSize = 400
    for (let i = 0; i < allPaths.length; i += chunkSize) {
      if (token !== enrichAllToken) return

      const batch = allPaths.slice(i, i + chunkSize)
      for (const p of batch) enriched.add(p)

      const metas = await invoke('life_album_enrich_files', { paths: batch }).catch((e) => {
        console.error('EXIF 全量增强失败:', e)
        return []
      })

      const arr = Array.isArray(metas) ? metas : []
      if (!arr.length) continue

      const next = photos.value.slice()
      for (const m of arr) {
        const idx = indexByPath.get(m?.path)
        if (typeof idx !== 'number') continue
        const p = next[idx]
        if (!p) continue
        next[idx] = {
          ...p,
          taken_at: m.taken_at ?? p.taken_at,
          latitude: m.latitude ?? p.latitude,
          longitude: m.longitude ?? p.longitude
        }
      }
      photos.value = next

      await new Promise((r) => setTimeout(r, 0))
    }
  } finally {
    if (token === enrichAllToken) enrichingAll.value = false
  }
}

const geocodingKeys = new Set()

const ensureCitiesForGeo = async () => {
  const keys = locationGroupsAll.value
    .map((g) => g.key)
    .filter((k) => k !== 'unknown' && !cityByCoordKey.value[k] && !geocodingKeys.has(k))
    .slice(0, 6)

  for (const k of keys) {
    const sample = geoPhotosAll.value.find((p) => coordKey(p) === k)
    if (!sample) continue

    geocodingKeys.add(k)
    try {
      const place = await invoke('life_album_reverse_geocode', {
        latitude: sample.latitude,
        longitude: sample.longitude
      })
      cityByCoordKey.value[k] = String(place || '').trim() || '未知位置'
    } catch (e) {
      console.error('城市反查失败:', e)
      cityByCoordKey.value[k] = '未知位置'
    } finally {
      geocodingKeys.delete(k)
    }
  }
}

const loadDir = async (dirPath, persist) => {
  enrichAllToken += 1
  enrichingAll.value = false

  albumDir.value = String(dirPath || '')
  visibleLimit.value = 400
  enriched.clear()
  cityByCoordKey.value = {}

  if (persist) {
    try {
      await invoke('set_life_album_album_dir', { path: albumDir.value })
    } catch (e) {
      console.error('保存相册目录失败:', e)
    }
  }

  loading.value = true
  try {
    const list = await invoke('life_album_scan_dir', { dirPath: albumDir.value })
    photos.value = Array.isArray(list) ? list : []
    message.success(`已加载 ${photos.value.length} 张照片`)
    enrichVisible()
    enrichAll()
  } catch (e) {
    console.error('加载目录失败:', e)
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
    cityByCoordKey.value = {}
    message.success(`已导入 ${photos.value.length} 张照片`)
    enrichVisible()
    enrichAll()
  } catch (e) {
    console.error('导入失败:', e)
    message.error('导入失败')
  } finally {
    loading.value = false
  }
}



watch(visibleLimit, () => {
  if (viewMode.value === 'location') ensureCitiesForGeo()
})

watch(detailVisible, async (v) => {
  if (!v) {
    if (viewerResizeObserver) {
      viewerResizeObserver.disconnect()
      viewerResizeObserver = null
    }
    viewerStageSize.value = { w: 0, h: 0 }
    viewerImgNatural.value = { w: 0, h: 0 }
    return
  }

  await nextTick()
  const el = viewerStageEl.value
  if (!el) return

  const update = () => {
    const r = el.getBoundingClientRect()
    viewerStageSize.value = { w: r.width, h: r.height }
  }

  update()
  viewerResizeObserver = new ResizeObserver(() => update())
  viewerResizeObserver.observe(el)
})

onMounted(async () => {
  try {
    const dir = await invoke('get_life_album_album_dir')
    if (dir) await loadDir(dir, false)
  } catch (e) {
    console.error('读取相册目录失败:', e)
  }
})
</script>

<style scoped>
.album-root {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 12px 16px;
  box-sizing: border-box;
  gap: 10px;
}

.album-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.album-title {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.album-title-main {
  font-size: 18px;
  font-weight: 650;
}

.album-title-sub {
  font-size: 12px;
  color: #6b7280;
  max-width: 46vw;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.album-actions {
  display: flex;
  gap: 8px;
}

.album-subbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.album-stats {
  font-size: 12px;
  color: #6b7280;
}

.album-loading,
.album-empty {
  flex: 1;
  min-height: 220px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
}

.album-loading-text {
  font-size: 12px;
  color: #6b7280;
}

.album-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.album-body-location {
  overflow: hidden;
}

.location-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  gap: 10px;
}

.location-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.map-card {
  position: relative;
  border-radius: 12px;
  overflow: hidden;
  background: #f3f4f6;
}

.map {
  height: clamp(420px, 58vh, 720px);
  width: 100%;
}

.map-empty {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(255, 255, 255, 0.88);
  backdrop-filter: blur(6px);
}

.group {
  margin-bottom: 14px;
}

.group-title {
  position: sticky;
  top: 0;
  z-index: 1;
  padding: 6px 0;
  background: rgba(255, 255, 255, 0.92);
  backdrop-filter: blur(8px);
  font-size: 13px;
  font-weight: 600;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: 10px;
}

.tile {
  border-radius: 12px;
  overflow: hidden;
  background: #f3f4f6;
  cursor: pointer;
}

.thumb {
  width: 100%;
  height: 140px;
  object-fit: cover;
  display: block;
  transition: transform 180ms ease;
}

.tile:hover .thumb {
  transform: scale(1.03);
}

.load-more {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 12px 0;
}

.load-more-hint {
  font-size: 12px;
  color: #6b7280;
}

.viewer-modal :deep(.n-card) {
  height: 100vh;
  display: flex;
  flex-direction: column;
}

.viewer-modal :deep(.n-card__content) {
  flex: 1;
  min-height: 0;
  padding: 0;
  position: relative;
}

.viewer-full {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #0b0f14;
  overflow: hidden;
}

.viewer-toolbar {
  position: absolute;
  top: 12px;
  left: 12px;
  right: 12px;
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px;
  border-radius: 12px;
  background: rgba(0, 0, 0, 0.45);
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.viewer-toolbar-left,
.viewer-toolbar-right {
  display: flex;
  gap: 8px;
}

:deep(.viewer-toolbar .n-button .n-button__content) {
  color: rgba(255, 255, 255, 0.92);
}

.viewer-full-img {
  width: auto;
  height: auto;
  max-width: none;
  max-height: none;
  flex: 0 0 auto;
  display: block;
  will-change: transform;
  user-select: none;
  cursor: default;
}
</style>