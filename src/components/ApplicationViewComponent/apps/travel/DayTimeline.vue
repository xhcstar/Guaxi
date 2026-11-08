<template>
  <section class="tr-timeline">
    <div class="tr-timeline-head">
      <div>
        <div class="tr-eyebrow"><span></span> {{ weekday }}</div>
        <h2>{{ day.theme || shortDate(day.date) }}</h2>
        <p v-if="day.area">{{ day.area }}</p>
      </div>
      <div v-if="day.weather" class="tr-timeline-weather">
        <TravelIcon :name="weatherIcon(day.weather)" :size="26" />
        <div>
          <strong>{{ day.weather.dayWeather }}</strong>
          <span>{{ day.weather.nightTemp ?? '—' }}~{{ day.weather.dayTemp ?? '—' }}℃ · {{ day.weather.wind }}</span>
        </div>
      </div>
    </div>

    <div v-if="!blocks.length" class="tr-timeline-empty">
      这一天还没有安排，点“重新生成”或直接在计划里补充一个地点。
    </div>

    <ol v-else class="tr-timeline-list">
      <li
        v-for="(block, index) in blocks"
        :key="block.id || index"
        class="tr-timeline-item"
        :class="[`is-${block.kind}`, { 'is-last': index === blocks.length - 1 }]"
      >
        <div class="tr-timeline-time">
          <strong>{{ block.start }}</strong>
          <span>{{ block.end }}</span>
        </div>
        <div class="tr-timeline-rail">
          <i class="tr-timeline-dot">
            <TravelIcon :name="kindIcon(block.kind)" :size="12" />
          </i>
        </div>
        <article class="tr-block">
          <div class="tr-block-head">
            <div class="tr-block-title">
              <span class="tr-block-kind">{{ kindLabel(block.kind) }}</span>
              <h3>{{ block.title }}</h3>
            </div>
            <span v-if="block.booking?.needed" class="tr-block-booking">
              <TravelIcon name="ticket" :size="12" /> 需预订
            </span>
          </div>

          <p v-if="block.detail" class="tr-block-detail">{{ block.detail }}</p>

          <div v-if="block.poi" class="tr-block-meta">
            <span v-if="block.poi.category">{{ block.poi.category }}</span>
            <span v-if="block.poi.rating"><TravelIcon name="heart" :size="11" /> {{ block.poi.rating }}</span>
            <span v-if="block.poi.cost">参考 ¥{{ block.poi.cost }}</span>
            <span v-if="block.poi.address">{{ block.poi.address }}</span>
            <span v-if="block.poi.tel">{{ block.poi.tel }}</span>
          </div>

          <div v-if="block.route" class="tr-route-line">
            <div class="tr-route-summary">
              <span class="tr-route-mode"><TravelIcon :name="routeIcon(block.route.mode)" :size="14" /> {{ routeModeText(block.route.mode) }}</span>
              <span>{{ distanceText(block.route.distance) }}</span>
              <span v-if="block.route.duration">{{ durationText(block.route.duration) }}</span>
              <span v-if="block.route.tolls">过路费 ¥{{ money(block.route.tolls) }}</span>
              <span v-if="block.route.fare">票价 ¥{{ money(block.route.fare) }}</span>
              <span v-if="block.route.traffic" class="tr-traffic" :class="{ 'is-bad': /拥堵|严重/.test(block.route.traffic) }">{{ block.route.traffic }}</span>
            </div>
            <div v-if="block.route.warning" class="tr-route-warning">
              <TravelIcon name="warning" :size="13" /> {{ block.route.warning }}
            </div>
            <div class="tr-route-options">
              <div
                v-for="(option, optionIndex) in routeOptions(block)"
                :key="optionIndex"
                class="tr-route-option"
                :class="{ 'is-primary': optionIndex === 0 }"
              >
                <div class="tr-route-option-head">
                  <span>{{ option.label || (optionIndex === 0 ? '推荐' : `备选 ${optionIndex}`) }}</span>
                  <strong>{{ durationText(option.duration) || '—' }}</strong>
                </div>
                <div class="tr-route-option-meta">
                  <span>{{ distanceText(option.distance) }}</span>
                  <span v-if="option.traffic">{{ option.traffic }}</span>
                  <span v-if="option.tolls">过路费 ¥{{ money(option.tolls) }}</span>
                  <span v-if="option.fare">票价 ¥{{ money(option.fare) }}</span>
                </div>
              </div>
            </div>
            <button
              v-if="canNavigate(block)"
              type="button"
              class="tr-route-nav"
              @click="openNavigation(block)"
            >
              <TravelIcon name="navigation" :size="12" />
              用手机导航这段
            </button>
          </div>

          <ul v-if="block.tips?.length" class="tr-block-tips">
            <li v-for="(tip, tipIndex) in block.tips" :key="tipIndex">
              <TravelIcon name="lightbulb" :size="12" /> {{ tip }}
            </li>
          </ul>

          <div v-if="block.booking?.needed" class="tr-block-booking-note">
            建议提前预订
            <span v-if="block.booking.channel"> · {{ block.booking.channel }}</span>
            <span v-if="block.booking.deadline"> · 截止 {{ friendlyDate(block.booking.deadline) }}</span>
            <span v-if="block.booking.note"> · {{ block.booking.note }}</span>
          </div>
        </article>
      </li>
    </ol>

    <div v-if="day.tips?.length" class="tr-day-tips">
      <strong><TravelIcon name="info" :size="14" /> 当天提醒</strong>
      <ul><li v-for="(tip, index) in day.tips" :key="index">{{ tip }}</li></ul>
    </div>
  </section>
</template>

<script setup>
import { computed } from 'vue'
import TravelIcon from './TravelIcon.vue'
import {
  bd09ToGcj02,
  bd09ToWgs84,
  distanceText,
  durationText,
  friendlyDate,
  gcj02ToWgs84,
  money,
  shortDate,
  wgs84ToGcj02,
  weekdayCn,
} from './travelUtils.js'

const props = defineProps({
  day: { type: Object, required: true },
  plan: { type: Object, default: () => ({}) },
})

const blocks = computed(() => props.day.blocks || [])
const weekday = computed(() => props.day.weekday || weekdayCn(props.day.date))

function kindIcon(kind) {
  return kind === 'transit'
    ? 'navigation'
    : kind === 'food'
      ? 'food'
      : kind === 'hotel'
        ? 'hotel'
        : kind === 'rest'
          ? 'clock'
          : 'pin'
}

function kindLabel(kind) {
  return {
    transit: '在路上',
    sight: '游览',
    food: '吃饭',
    hotel: '住宿',
    rest: '休息',
    activity: '体验',
  }[kind] || '安排'
}

function routeIcon(mode) {
  return mode === 'walking' ? 'walk' : mode === 'transit' ? 'train' : 'car'
}

function routeModeText(mode) {
  return mode === 'walking' ? '步行' : mode === 'transit' ? '公共交通' : '驾车'
}

function weatherIcon(item) {
  return item?.icon === 'sun'
    ? 'sun'
    : item?.icon === 'rain'
      ? 'rain'
      : item?.icon === 'snow'
        ? 'snow'
        : item?.icon === 'storm'
          ? 'storm'
          : item?.icon === 'fog'
            ? 'fog'
            : 'cloud'
}

function routeOptions(block) {
  const list = block?.route?.alternatives
  if (Array.isArray(list) && list.length) return list.slice(0, 3)
  return block?.route ? [block.route] : []
}

function canNavigate(block) {
  const route = block?.route
  return Boolean(
    route &&
      Number.isFinite(Number(route.fromLng)) &&
      Number.isFinite(Number(route.fromLat)) &&
      Number.isFinite(Number(route.toLng)) &&
      Number.isFinite(Number(route.toLat)),
  )
}

async function openUrl(url) {
  try {
    const { openUrl: openExternal } = await import('@tauri-apps/plugin-opener')
    await openExternal(url)
  } catch {
    window.open(url, '_blank', 'noopener')
  }
}

async function openNavigation(block) {
  const route = block.route
  if (!route) return
  const source = props.plan?.source || 'osm'
  const toWgs = (lng, lat) => {
    if (source === 'amap' || source === 'tencent') return gcj02ToWgs84(lng, lat)
    if (source === 'baidu') return bd09ToWgs84(lng, lat)
    return [lng, lat]
  }
  const toGcj = (lng, lat) => {
    if (source === 'amap' || source === 'tencent') return [lng, lat]
    if (source === 'baidu') return bd09ToGcj02(lng, lat)
    return wgs84ToGcj02(lng, lat)
  }
  const fromLng = Number(route.fromLng)
  const fromLat = Number(route.fromLat)
  const toLng = Number(route.toLng)
  const toLat = Number(route.toLat)
  const inChina = fromLng > 72 && fromLng < 138 && fromLat > 0.8 && fromLat < 56
  const [wFromLng, wFromLat] = toWgs(fromLng, fromLat)
  const [wToLng, wToLat] = toWgs(toLng, toLat)
  if (inChina) {
    const [gFromLng, gFromLat] = toGcj(fromLng, fromLat)
    const [gToLng, gToLat] = toGcj(toLng, toLat)
    const mode = route.mode === 'walking' ? 'walk' : route.mode === 'transit' ? 'bus' : 'car'
    const url =
      `https://uri.amap.com/navigation?from=${gFromLng},${gFromLat},${encodeURIComponent(route.fromName || '起点')}` +
      `&to=${gToLng},${gToLat},${encodeURIComponent(route.toName || '终点')}` +
      `&mode=${mode}&policy=1&src=guaxi&coordinate=gaode&callnative=1`
    await openUrl(url)
    return
  }
  const travelMode = route.mode === 'walking' ? 'walking' : route.mode === 'transit' ? 'transit' : 'driving'
  await openUrl(
    `https://www.google.com/maps/dir/?api=1&origin=${wFromLat},${wFromLng}&destination=${wToLat},${wToLng}&travelmode=${travelMode}`,
  )
}

</script>
