<template>
  <div ref="root" class="tr-poster">
    <div class="tr-poster-top">
      <div class="tr-poster-brand">
        <span class="tr-poster-seal">行迹</span>
        <div>
          <strong>行迹 · 旅行计划</strong>
          <small>XINGJI TRAVEL JOURNAL · ISSUED {{ issued }}</small>
        </div>
      </div>
      <div class="tr-poster-code">
        <span>{{ cities.fromCode }}</span>
        <i>→</i>
        <span>{{ cities.toCode }}</span>
      </div>
    </div>

    <div class="tr-poster-route">
      <div><strong>{{ cities.from }}</strong><small>出发</small></div>
      <div class="tr-poster-route-line"><span></span><TravelIcon :name="transportIcon" :size="22" /><span></span></div>
      <div><strong>{{ cities.to }}</strong><small>目的地</small></div>
    </div>

    <h1>{{ plan.title }}</h1>
    <p v-if="plan.subtitle" class="tr-poster-sub">{{ plan.subtitle }}</p>
    <div class="tr-poster-meta">
      <span>{{ dateRangeText(plan) }}</span>
      <span>{{ people }} 人</span>
      <span>{{ transportLabel(plan.transport) }}</span>
      <span>人均 ¥{{ money(plan.budget?.perPerson) }}</span>
    </div>

    <div class="tr-poster-map">
      <img v-if="isMapImage" :src="plan.posterMap" alt="路线示意图" />
      <svg v-else viewBox="0 0 980 300" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
        <rect width="980" height="300" fill="#efe7d7" />
        <g fill="none" stroke="rgba(28,42,39,.12)" stroke-width="1">
          <path d="M-20 240 C140 120 220 300 400 190 S700 100 1000 220" />
          <path d="M-20 265 C140 145 220 325 400 215 S700 125 1000 245" />
          <path d="M-20 290 C140 170 220 350 400 240 S700 150 1000 270" />
        </g>
        <path d="M60 230 C220 60, 360 280, 540 150 S800 80, 920 190" fill="none" stroke="#c4552d" stroke-width="4" stroke-dasharray="10 9" />
        <circle cx="60" cy="230" r="10" fill="#1c2a27" />
        <circle cx="540" cy="150" r="8" fill="#c4552d" />
        <circle cx="920" cy="190" r="10" fill="#4f6b57" />
        <text x="80" y="258" font-size="18" fill="#1c2a27">{{ cities.from }}</text>
        <text x="820" y="230" font-size="18" fill="#1c2a27">{{ cities.to }}</text>
      </svg>
      <span class="tr-poster-map-note">{{ plan.source === 'amap' ? 'ROUTE · AMAP' : 'ROUTE · OSM' }}</span>
    </div>

    <div class="tr-poster-days">
      <section v-for="(day, index) in plan.days" :key="day.date" class="tr-poster-day">
        <div class="tr-poster-day-head">
          <span>DAY {{ String(index + 1).padStart(2, '0') }}</span>
          <strong>{{ shortDate(day.date) }} · {{ day.weekday }}</strong>
          <em>{{ day.theme }}</em>
          <small v-if="day.weather">{{ day.weather.dayWeather }} {{ day.weather.nightTemp ?? '—' }}~{{ day.weather.dayTemp ?? '—' }}℃</small>
        </div>
        <ul>
          <li v-for="block in day.blocks" :key="block.id">
            <span class="tr-poster-time">{{ block.start }}–{{ block.end }}</span>
            <span class="tr-poster-block-title">{{ block.title }}</span>
            <small v-if="block.route">{{ block.route.summary }}</small>
          </li>
        </ul>
      </section>
    </div>

    <div class="tr-poster-bottom">
      <div class="tr-poster-budget">
        <h3>花费清单</h3>
        <div v-for="item in plan.budget?.items || []" :key="item.category" class="tr-poster-budget-row">
          <span>{{ item.category }}<small>{{ item.label }}</small></span>
          <strong>¥{{ money(item.amount) }}</strong>
        </div>
        <div class="tr-poster-budget-total">
          <span>合计</span>
          <strong>¥{{ money(plan.budget?.total) }}</strong>
        </div>
      </div>
      <div class="tr-poster-prep">
        <h3>行前重点</h3>
        <ul>
          <li v-for="item in topChecklist" :key="item.id">{{ item.label }}</li>
        </ul>
        <div class="tr-poster-pack">
          <strong>{{ plan.packing?.[0]?.category || '证件' }}</strong>
          <span>{{ (plan.packing?.[0]?.items || []).slice(0, 5).join(' · ') }}</span>
        </div>
      </div>
      <div v-if="qr" class="tr-poster-qr">
        <img :src="qr" alt="手机查看二维码" />
        <small>扫码在手机查看</small>
      </div>
      <div v-else class="tr-poster-stamp">
        <span>行迹</span>
        <small>XINGJI</small>
      </div>
    </div>

    <div class="tr-poster-foot">
      <span>由行迹生成 · {{ plan.factsMeta?.provider || 'AI 编排' }} · {{ plan.source === 'amap' ? '高德地图数据' : 'OpenStreetMap 数据' }}</span>
      <span>{{ issued }}</span>
    </div>
    <div class="tr-poster-perforation" aria-hidden="true"></div>
  </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import TravelIcon from './TravelIcon.vue'
import { cityCode, dateRangeText, peopleCount, ticketCities, transportLabel } from './travelShare.js'
import { money, shortDate, weekdayCn } from './travelUtils.js'

const props = defineProps({
  plan: { type: Object, required: true },
  qr: { type: String, default: '' },
})

const root = ref(null)
defineExpose({ getNode: () => root.value })

const cities = computed(() => ticketCities(props.plan))
const people = computed(() => peopleCount(props.plan))
const issued = new Date().toLocaleDateString('zh-CN')
const isMapImage = computed(() => String(props.plan.posterMap || '').startsWith('data:image/'))
const transportIcon = computed(() => {
  const mode = props.plan.transport
  return mode === 'transit' ? 'train' : mode === 'walking' ? 'walk' : mode === 'tour' ? 'users' : 'car'
})
const topChecklist = computed(() => (props.plan.checklist || []).slice(0, 6))
</script>
