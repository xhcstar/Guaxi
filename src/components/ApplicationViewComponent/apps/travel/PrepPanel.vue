<template>
  <section class="tr-prep">
    <div class="tr-panel-head">
      <div>
        <div class="tr-eyebrow"><span></span> BEFORE DEPARTURE</div>
        <h2>行前准备</h2>
        <p>把该订的订掉，该带的带齐。出发前一天再看一次天气和路况。</p>
      </div>
      <div class="tr-prep-progress">
        <div class="tr-prep-ring" :style="{ '--progress': `${donePercent}%` }">
          <span>{{ doneCount }}/{{ checklist.length }}</span>
        </div>
        <small>{{ donePercent }}% 已完成</small>
      </div>
    </div>

    <div class="tr-prep-freshness">
      <TravelIcon name="clock" :size="14" />
      资料采集于 {{ timeAgo(plan.factsMeta?.collectedAt) }}。天气、路况和营业时间是生成时的快照，
      出发前 24-48 小时建议点「重新拉取资料并生成」再确认一次。
    </div>

    <div class="tr-prep-grid">
      <div class="tr-prep-card">
        <h3><TravelIcon name="check" :size="16" /> 出发前确认</h3>
        <ul class="tr-checklist">
          <li v-for="item in checklist" :key="item.id" :class="{ done: item.done }">
            <button type="button" class="tr-check" :class="{ done: item.done }" @click="$emit('toggle-check', item.id)">
              <TravelIcon v-if="item.done" name="check" :size="11" />
            </button>
            <div>
              <strong>{{ item.label }}</strong>
              <small v-if="item.note">{{ item.note }}</small>
            </div>
            <span v-if="item.due" class="tr-due">{{ shortDate(item.due) }} 前</span>
          </li>
        </ul>
      </div>

      <div class="tr-prep-card">
        <h3><TravelIcon name="ticket" :size="16" /> 需要预订</h3>
        <div v-if="bookings.length" class="tr-bookings">
          <div v-for="(item, index) in bookings" :key="index" class="tr-booking-item">
            <span class="tr-booking-day">D{{ item.dayIndex + 1 }}</span>
            <div>
              <strong>{{ item.title }}</strong>
              <small>{{ shortDate(item.date) }}<template v-if="item.channel"> · {{ item.channel }}</template><template v-if="item.deadline"> · 截止 {{ item.deadline }}</template></small>
              <p v-if="item.note">{{ item.note }}</p>
            </div>
          </div>
        </div>
        <div v-else class="tr-prep-empty">
          没有强制的预订项。热门博物馆、山顶餐厅和周末酒店仍建议提前 2-3 天确认。
        </div>
      </div>
    </div>

    <div class="tr-prep-card is-wide">
      <h3><TravelIcon name="backpack" :size="16" /> 行李清单</h3>
      <div class="tr-pack-grid">
        <div v-for="group in packing" :key="group.category" class="tr-pack-group">
          <strong>{{ group.category }}</strong>
          <ul>
            <li v-for="item in group.items" :key="item">{{ item }}</li>
          </ul>
        </div>
      </div>
    </div>

    <div class="tr-prep-grid">
      <div class="tr-prep-card">
        <h3><TravelIcon name="cloud" :size="16" /> 天气与穿衣</h3>
        <p v-if="plan.season?.label" class="tr-prep-season">季节：{{ plan.season.label }}</p>
        <p class="tr-prep-lead">{{ weatherAdvice }}</p>
        <div v-if="plan.weather?.length" class="tr-mini-weather">
          <div v-for="item in plan.weather" :key="item.date">
            <span>{{ shortDate(item.date) }}</span>
            <strong>{{ item.dayWeather }}</strong>
            <small>{{ item.nightTemp ?? '—' }}~{{ item.dayTemp ?? '—' }}℃</small>
          </div>
        </div>
      </div>
      <div class="tr-prep-card">
        <h3><TravelIcon name="lightbulb" :size="16" /> 旅行建议</h3>
        <ul class="tr-advice-list">
          <li v-for="(item, index) in advice" :key="index">{{ item }}</li>
        </ul>
      </div>
    </div>

    <div class="tr-prep-card is-wide">
      <h3><TravelIcon name="flag" :size="16" /> 备选方案</h3>
      <ul class="tr-advice-list is-alt">
        <li v-for="(item, index) in alternatives" :key="index">{{ item }}</li>
      </ul>
    </div>
  </section>
</template>

<script setup>
import { computed } from 'vue'
import TravelIcon from './TravelIcon.vue'
import { friendlyDate, shortDate, timeAgo } from './travelUtils.js'

const props = defineProps({
  plan: { type: Object, required: true },
})
defineEmits(['toggle-check'])

const checklist = computed(() =>
  [...(props.plan.checklist || [])].sort((a, b) => String(a.due || '').localeCompare(String(b.due || ''))),
)
const doneCount = computed(() => checklist.value.filter((item) => item.done).length)
const donePercent = computed(() =>
  checklist.value.length ? Math.round((doneCount.value / checklist.value.length) * 100) : 0,
)
const packing = computed(() => props.plan.packing || [])
const advice = computed(() => props.plan.advice || [])
const alternatives = computed(() => props.plan.alternatives || [])
const bookings = computed(() => {
  const items = []
  ;(props.plan.days || []).forEach((day, dayIndex) => {
    ;(day.blocks || []).forEach((block) => {
      if (block.booking?.needed) {
        items.push({
          dayIndex,
          date: day.date,
          title: block.title,
          channel: block.booking.channel,
          deadline: friendlyDate(block.booking.deadline),
          note: block.booking.note,
        })
      }
    })
  })
  return items
})
const weatherAdvice = computed(() => {
  const days = props.plan.weather || []
  const route = (props.plan.routeWeather || []).filter((point) => point.available !== false)
  if (!days.length && !route.length) return '暂无逐日预报，出发前 48 小时再确认一次。'
  const mins = days.map((item) => Number(item.nightTemp)).filter(Number.isFinite)
  const maxs = days.map((item) => Number(item.dayTemp)).filter(Number.isFinite)
  const routeTemps = route.map((point) => Number(point.temperature)).filter(Number.isFinite)
  const rain =
    days.some((item) => `${item.dayWeather || ''}${item.nightWeather || ''}`.match(/雨|雪|雷/)) ||
    route.some((point) => /雨|雪|雷/.test(point.weather || ''))
  const fog =
    route.some((point) => /雾|霾/.test(point.weather || '')) ||
    route.some((point) => Number(point.visibility) > 0 && Number(point.visibility) < 1000)
  const windy = route.some((point) => Number(point.windSpeed) >= 40)
  const min = mins.length ? Math.min(...mins) : null
  const max = maxs.length ? Math.max(...maxs) : null
  const routeMin = routeTemps.length ? Math.min(...routeTemps) : null
  const routeMax = routeTemps.length ? Math.max(...routeTemps) : null
  const parts = []
  if (min != null && max != null) parts.push(`目的地约 ${min}~${max}℃`)
  if (routeMin != null && routeMax != null && route.length) {
    parts.push(`沿途采样点约 ${Math.round(routeMin)}~${Math.round(routeMax)}℃`)
  }
  if (min != null && min < 10) parts.push('早晚偏冷，带一件保暖外套')
  else if (min != null && min < 18) parts.push('早晚温差大，薄外套比短袖更稳')
  if (max != null && max >= 28) parts.push('日间偏热，注意防晒补水')
  if (rain) parts.push('有降水概率，折叠伞和防水鞋套用得上')
  if (fog) parts.push('沿途有雾，自驾出发前确认能见度与雾灯')
  if (windy) parts.push('沿途风力较大，注意横风和保暖')
  return `${parts.join('；')}。`
})
</script>
