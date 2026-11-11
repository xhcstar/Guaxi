<template>
  <div class="tr-map-wrap">
    <div class="tr-map-toolbar">
      <div v-hscroll class="tr-map-days" aria-label="按天查看路线">
        <button type="button" :class="{ active: activeDay === 'all' }" @click="setDay('all')">全部</button>
        <button
          v-for="(day, index) in plan.days || []"
          :key="day.date"
          type="button"
          :class="{ active: activeDay === index }"
          @click="setDay(index)"
        >
          D{{ index + 1 }}
        </button>
      </div>
      <div class="tr-map-tools">
        <span class="tr-map-source">{{ mapSourceLabel(plan.source) }}</span>
        <button type="button" class="tr-btn is-small is-outline" @click="openInAmap">在高德打开</button>
      </div>
    </div>

    <div class="tr-map-frame">
      <div ref="mapEl" class="tr-map"></div>
      <div v-if="offline" class="tr-map-offline">
        <TravelIcon name="info" :size="18" />
        地图瓦片加载失败（可能离线或被网络拦截）。路线、时间与地点信息仍可查看。
      </div>
      <div v-if="!hasPoints" class="tr-map-empty">
        <TravelIcon name="map" :size="30" />
        <strong>还没有可落图的坐标</strong>
        <span>重新生成行程，或在高德/OSM 数据可用时再试。</span>
      </div>
      <div class="tr-map-legend">
        <span v-for="(color, index) in usedDayColors" :key="index">
          <i :style="{ background: color }"></i> DAY {{ index + 1 }}
        </span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import L from 'leaflet'
import 'leaflet/dist/leaflet.css'
import TravelIcon from './TravelIcon.vue'
import { escapeHtml, bd09ToGcj02, bd09ToWgs84, wgs84ToGcj02 } from './travelUtils.js'
import { mapSourceLabel } from './travelShare.js'
import { vHscroll } from './travelScrollAssist.js'

const props = defineProps({
  plan: { type: Object, required: true },
  dayIndex: { type: [Number, String], default: null },
})

// 路线统一用绿色系，和地图上的公园/道路区分开；多天用深浅与冷绿区分。
const DAY_COLORS = ['#0f7a3d', '#0b8f6b', '#4f9a2f', '#1f8f8f', '#2f6b4f', '#6aa84f']
const mapEl = ref(null)
const activeDay = ref(props.dayIndex == null ? 'all' : Number(props.dayIndex))
const offline = ref(false)
let map = null
let layerGroup = null
let resizeObserver = null

const dayCount = computed(() => (props.plan.days || []).length)
const usedDayColors = computed(() => DAY_COLORS.slice(0, Math.max(1, dayCount.value)))
const hasPoints = computed(() =>
  (props.plan.days || []).some((day) =>
    (day.blocks || []).some((block) => block.poi && block.poi.lng && block.poi.lat),
  ),
)

function dayColor(index) {
  return DAY_COLORS[index % DAY_COLORS.length]
}

function coordSystem() {
  const plan = props.plan || {}
  if (plan.coordSystem) return plan.coordSystem
  if (plan.source === 'amap' || plan.source === 'tencent') return 'gcj02'
  if (plan.source === 'baidu') return 'bd09'
  return 'wgs84'
}

function mapCoords(lng, lat) {
  if (coordSystem() === 'bd09') return bd09ToWgs84(lng, lat)
  return [lng, lat]
}

function parsePolyline(value) {
  return String(value || '')
    .split(';')
    .map((point) => point.split(',').map(Number))
    .filter((pair) => pair.length === 2 && pair.every(Number.isFinite))
    .map(([lng, lat]) => [lat, lng])
}

function markerIcon(label, color, kind) {
  return L.divIcon({
    className: 'tr-map-pin-wrap',
    html: `<div class="tr-map-pin ${kind || ''}" style="--pin-color:${color}"><span>${escapeHtml(label)}</span></div>`,
    iconSize: [28, 28],
    iconAnchor: [14, 14],
  })
}

function popupHtml(poi) {
  return `<div class="tr-map-popup"><strong>${escapeHtml(poi.name || '')}</strong>${
    poi.category ? `<span>${escapeHtml(poi.category)}</span>` : ''
  }${poi.address ? `<p>${escapeHtml(poi.address)}</p>` : ''}${
    poi.rating ? `<small>★ ${escapeHtml(poi.rating)}</small>` : ''
  }</div>`
}

function initMap() {
  if (!mapEl.value) return
  map = L.map(mapEl.value, {
    zoomControl: false,
    attributionControl: true,
    preferCanvas: true,
  }).setView([30.24, 120.13], 11)
  L.control.zoom({ position: 'bottomright' }).addTo(map)
  const amap = coordSystem() === 'gcj02'
  const url = amap
    ? 'https://webrd0{s}.is.autonavi.com/appmaptile?lang=zh_cn&size=1&scale=1&style=8&x={x}&y={y}&z={z}'
    : 'https://tile.openstreetmap.org/{z}/{x}/{y}.png'
  const layer = amap
    ? L.tileLayer(url, {
        subdomains: '1234',
        maxZoom: 18,
        attribution: '© 高德地图',
      })
    : L.tileLayer(url, {
        maxZoom: 19,
        attribution: '© OpenStreetMap contributors',
      })
  let errors = 0
  layer.on('tileerror', () => {
    errors += 1
    if (errors >= 3) offline.value = true
  })
  layer.on('tileload', () => {
    errors = 0
    offline.value = false
  })
  layer.addTo(map)
  layerGroup = L.layerGroup().addTo(map)
  render()
  resizeObserver = new ResizeObserver(() => map?.invalidateSize())
  resizeObserver.observe(mapEl.value)
}

function render() {
  if (!map || !layerGroup) return
  layerGroup.clearLayers()
  const bounds = []
  ;(props.plan.days || []).forEach((day, dayIndexValue) => {
    if (activeDay.value !== 'all' && Number(activeDay.value) !== dayIndexValue) return
    const color = dayColor(dayIndexValue)
    const seen = new Set()
    let order = 0
    ;(day.blocks || []).forEach((block) => {
      if (block.route?.polyline) {
        const points = parsePolyline(block.route.polyline)
          .map(([lat, lng]) => {
            const [mappedLng, mappedLat] = mapCoords(lng, lat)
            return [mappedLat, mappedLng]
          })
        if (points.length > 1) {
          L.polyline(points, {
            color: '#fffaf0',
            weight: 9,
            opacity: 0.85,
            lineCap: 'round',
            lineJoin: 'round',
          }).addTo(layerGroup)
          L.polyline(points, {
            color,
            weight: 4.5,
            opacity: 0.95,
            lineCap: 'round',
            lineJoin: 'round',
            dashArray: block.route.mode === 'walking' ? '7 8' : null,
          }).addTo(layerGroup)
          bounds.push(...points)
        }
      }
      const poi = block.poi
      if (!poi || !Number(poi.lng) || !Number(poi.lat)) return
      const key = `${dayIndexValue}|${poi.id || poi.name}`
      if (seen.has(key)) return
      seen.add(key)
      order += 1
      const label =
        block.kind === 'hotel' ? 'H' : block.kind === 'food' ? 'F' : String(order)
      const [poiLng, poiLat] = mapCoords(Number(poi.lng), Number(poi.lat))
      const marker = L.marker([poiLat, poiLng], {
        icon: markerIcon(label, color, block.kind),
        keyboard: false,
      })
      marker.bindPopup(popupHtml(poi), { closeButton: false })
      marker.addTo(layerGroup)
      bounds.push([poiLat, poiLng])
    })
  })
  if (bounds.length) {
    map.fitBounds(bounds, { padding: [44, 44], maxZoom: 14 })
  }
}

function setDay(value) {
  activeDay.value = value
  nextTick(render)
}

async function openInAmap() {
  const destination = props.plan.destination
  if (!destination?.lng || !destination?.lat) return
  const system = coordSystem()
  let lng = Number(destination.lng)
  let lat = Number(destination.lat)
  if (system === 'bd09') {
    [lng, lat] = bd09ToGcj02(lng, lat)
  } else if (system === 'wgs84') {
    [lng, lat] = wgs84ToGcj02(lng, lat)
  }
  const url = `https://uri.amap.com/marker?position=${lng},${lat}&name=${encodeURIComponent(destination.name || '目的地')}&src=guaxi&coordinate=gaode&callnative=0`
  try {
    const { openUrl } = await import('@tauri-apps/plugin-opener')
    await openUrl(url)
  } catch {
    window.open(url, '_blank', 'noopener')
  }
}

watch(
  () => props.dayIndex,
  (value) => {
    activeDay.value = value == null ? 'all' : Number(value)
    nextTick(render)
  },
)

watch(
  () => props.plan.id,
  () => nextTick(render),
)

onMounted(() => {
  nextTick(initMap)
})

onBeforeUnmount(() => {
  resizeObserver?.disconnect()
  map?.remove()
  map = null
  layerGroup = null
})
</script>
