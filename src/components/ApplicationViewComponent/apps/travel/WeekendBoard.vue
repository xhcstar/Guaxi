<template>
  <div class="tr-board">
    <header class="tr-board-head">
      <div class="tr-board-head-top">
        <button type="button" class="tr-board-back" @click="$emit('home')">
          <TravelIcon name="chevronLeft" :size="15" /> 计划库
        </button>
        <span class="tr-board-live"><i></i> WEEKEND SHUTTLE</span>
      </div>
      <div class="tr-board-head-main">
        <div>
          <div class="tr-eyebrow is-light"><span></span> 未来 8 个周末</div>
          <h1>周末班次表</h1>
          <p>不用再想“这周去哪”。以 {{ origin?.city || origin?.name || '你的城市' }} 为圆心，Guaxi 已经排好接下来两个月的周末。</p>
        </div>
        <div class="tr-board-head-actions">
          <button type="button" class="tr-btn is-light is-small" :disabled="loading" @click="$emit('generate')">
            <TravelIcon name="refresh" :size="15" /> {{ loading ? '生成中…' : plans.length ? '重新生成班次' : '生成未来 8 个周末' }}
          </button>
        </div>
      </div>
      <div class="tr-board-meta">
        <span><TravelIcon name="calendar" :size="14" /> {{ plans.length || 8 }} 个周末</span>
        <span><TravelIcon name="map" :size="14" /> 周边 30-100 km</span>
        <span><TravelIcon name="sparkles" :size="14" /> 最近一个周末已细化</span>
      </div>
    </header>

    <div v-if="loading" class="tr-board-loading">
      <div v-for="index in 4" :key="index" class="tr-board-skeleton">
        <span></span><span></span><span></span>
      </div>
    </div>

    <div v-else-if="!plans.length" class="tr-board-empty">
      <div class="tr-board-empty-art" aria-hidden="true">
        <svg viewBox="0 0 420 180" xmlns="http://www.w3.org/2000/svg">
          <path d="M30 130 C130 40, 190 170, 280 90 S380 40, 400 100" fill="none" stroke="rgba(196,85,45,.6)" stroke-width="3" stroke-dasharray="10 9"/>
          <circle cx="30" cy="130" r="8" fill="#1c2a27"/><circle cx="280" cy="90" r="7" fill="#c4552d"/><circle cx="400" cy="100" r="8" fill="#4f6b57"/>
          <rect x="130" y="45" width="110" height="52" rx="6" fill="none" stroke="rgba(28,42,39,.3)" stroke-dasharray="5 5"/>
          <text x="146" y="76" font-size="14" fill="#1c2a27">周末班次</text>
        </svg>
      </div>
      <h2>还没有班次表</h2>
      <p>先确认出发城市，Guaxi 会检索周边适合两天一夜或当天往返的地点，排成未来 8 个周末。</p>
      <button type="button" class="tr-btn is-primary" @click="$emit('generate')">
        <TravelIcon name="calendar" :size="17" /> 生成本季周末班次
      </button>
      <small>最近一个周末会自动细化到时间表；后面的周末先给方向和预订提醒。</small>
    </div>

    <div v-else class="tr-board-list">
      <div class="tr-board-list-head">
        <span>周末</span>
        <span>目的地 / 主题</span>
        <span>两天怎么玩</span>
        <span>天气</span>
        <span>预算</span>
        <span>状态</span>
        <span>操作</span>
      </div>
      <article
        v-for="(plan, index) in plans"
        :key="plan.id"
        class="tr-board-row"
        :class="{ 'is-locked': plan.locked }"
        :style="{ '--i': index }"
        @click="$emit('open-plan', plan.id)"
      >
        <div class="tr-board-date">
          <strong>{{ dayOfMonth(plan.weekendStart || plan.startDate) }}</strong>
          <span>{{ monthText(plan.weekendStart || plan.startDate) }}</span>
          <small>{{ weekday(plan.weekendStart || plan.startDate) }}</small>
        </div>
        <div class="tr-board-dest">
          <strong>{{ plan.destination?.name || plan.destinationName || '周边目的地' }}</strong>
          <span>{{ plan.weekend?.theme || plan.theme || plan.title }}</span>
          <small>{{ plan.subtitle || '两天一晚，按地理位置顺路安排' }}</small>
        </div>
        <div class="tr-board-highlights">
          <p v-for="(day, dayIndex) in (plan.days || []).slice(0, 2)" :key="day.date">
            <span>D{{ dayIndex + 1 }}</span>{{ daySummary(day) || day.theme }}
          </p>
        </div>
        <div class="tr-board-weather">
          <template v-if="weatherOf(plan)">
            <TravelIcon :name="weatherIcon(weatherOf(plan))" :size="20" />
            <strong>{{ weatherOf(plan).dayWeather }}</strong>
            <small v-if="weatherOf(plan).single">{{ weatherOf(plan).dayTemp }}℃</small>
            <small v-else>{{ weatherOf(plan).nightTemp ?? '—' }}~{{ weatherOf(plan).dayTemp ?? '—' }}℃</small>
          </template>
          <span v-else class="is-muted">临近更新</span>
        </div>
        <div class="tr-board-budget">
          <strong>¥{{ money(plan.budget?.total) }}</strong>
          <small>人均 ¥{{ money(plan.budget?.perPerson) }}</small>
        </div>
        <div class="tr-board-status">
          <span v-if="plan.locked" class="tr-status is-locked"><TravelIcon name="lock" :size="11" /> 已锁定</span>
          <span v-else-if="plan.detailLevel === 'draft'" class="tr-status is-draft">待细化</span>
          <span v-else class="tr-status is-ready">已细化</span>
        </div>
        <div class="tr-board-actions" @click.stop>
          <button type="button" class="tr-mini-btn" @click="$emit('open-plan', plan.id)">
            <TravelIcon name="arrowRight" :size="13" /> 查看
          </button>
          <button
            v-if="plan.detailLevel === 'draft'"
            type="button"
            class="tr-mini-btn is-accent"
            :disabled="refiningId === plan.id"
            @click="$emit('refine', plan.id)"
          >
            <TravelIcon name="sparkles" :size="13" /> {{ refiningId === plan.id ? '细化中…' : '细化' }}
          </button>
          <button type="button" class="tr-icon-btn" title="换一个目的地" @click="$emit('replace', plan.id)">
            <TravelIcon name="refresh" :size="14" />
          </button>
          <button type="button" class="tr-icon-btn" :title="plan.locked ? '解锁' : '锁定'" @click="$emit('toggle-lock', plan.id)">
            <TravelIcon :name="plan.locked ? 'unlock' : 'lock'" :size="14" />
          </button>
        </div>
      </article>
    </div>

    <div class="tr-board-foot">
      <div><TravelIcon name="info" :size="15" /> 周末目的地按评分和距离混合排序，避免连续几周都去同一类地方。</div>
      <div><TravelIcon name="cloud" :size="15" /> 16 天以外的天气还不可靠，班次表会在临近时提示更新。</div>
    </div>
  </div>
</template>

<script setup>
import TravelIcon from './TravelIcon.vue'
import { money, parseDate, shortDate, weekdayCn } from './travelUtils.js'

const props = defineProps({
  plans: { type: Array, default: () => [] },
  loading: Boolean,
  refiningId: { type: String, default: '' },
  origin: { type: Object, default: null },
  isMock: Boolean,
})

defineEmits(['generate', 'open-plan', 'refine', 'replace', 'toggle-lock', 'home'])

function dateOf(value) {
  return parseDate(value)
}

function dayOfMonth(value) {
  const date = dateOf(value)
  return date ? String(date.getDate()).padStart(2, '0') : '--'
}

function monthText(value) {
  const date = dateOf(value)
  return date ? `${date.getMonth() + 1} 月` : '待定'
}

function weekday(value) {
  return weekdayCn(value) || ''
}

function weatherOf(plan) {
  const days = plan.days || []
  const first = days[0]
  if (first?.weather) return first.weather
  if (plan.weather?.[0]) return plan.weather[0]
  const points = (plan.routeWeather || []).filter((point) => point.available !== false)
  const destination = plan.destination || {}
  const distance2 = (item) =>
    (Number(item.lng) - Number(destination.lng)) ** 2 +
    (Number(item.lat) - Number(destination.lat)) ** 2
  const point = points.reduce(
    (best, item) => (!best || distance2(item) < distance2(best) ? item : best),
    null,
  )
  if (!point) return null
  return {
    dayWeather: point.weather,
    icon: point.icon,
    dayTemp: point.temperature != null ? Math.round(Number(point.temperature)) : null,
    nightTemp: null,
    single: true,
    source: point.source,
  }
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

function daySummary(day) {
  return (day.blocks || [])
    .filter((block) => block.poi && block.kind !== 'hotel')
    .map((block) => block.poi.name)
    .slice(0, 2)
    .join(' · ')
}
</script>
