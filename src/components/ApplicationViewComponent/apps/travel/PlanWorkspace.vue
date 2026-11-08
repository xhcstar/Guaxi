<template>
  <div class="tr-workspace">
    <header class="tr-plan-head">
      <div class="tr-plan-head-top">
        <button type="button" class="tr-back-link" @click="$emit('back')">
          <TravelIcon name="chevronLeft" :size="15" /> 计划库
        </button>
        <div class="tr-plan-badges">
          <span class="tr-badge">{{ plan.kind === 'weekend' ? '周末班次' : '长假出游' }}</span>
          <span class="tr-badge is-quiet">{{ mapSourceLabel(plan.source) }}</span>
          <span v-if="plan.detailLevel === 'draft'" class="tr-badge is-warn">待细化</span>
          <span v-if="plan.locked" class="tr-badge is-lock"><TravelIcon name="lock" :size="12" /> 已锁定</span>
        </div>
      </div>

      <div class="tr-ticket-row">
        <div class="tr-ticket-route">
          <div>
            <strong>{{ cities.from }}</strong>
            <small>{{ cities.fromCode }}</small>
          </div>
          <div class="tr-ticket-arrow">
            <span></span>
            <TravelIcon :name="transportIcon" :size="20" />
            <span></span>
          </div>
          <div>
            <strong>{{ cities.to }}</strong>
            <small>{{ cities.toCode }}</small>
          </div>
        </div>
        <div class="tr-ticket-heading">
          <div class="tr-eyebrow"><span></span> {{ dateText }}</div>
          <h1>{{ plan.title }}</h1>
          <p v-if="plan.subtitle">{{ plan.subtitle }}</p>
        </div>
        <div class="tr-ticket-actions">
          <button type="button" class="tr-btn is-primary is-small" @click="activeTab = 'share'">
            <TravelIcon name="share" :size="15" /> 分享
          </button>
          <button type="button" class="tr-btn is-outline is-small" @click="$emit('print', plan)">
            <TravelIcon name="print" :size="15" /> 打印
          </button>
          <details class="tr-more">
            <summary class="tr-icon-btn" title="更多操作"><TravelIcon name="settings" :size="16" /></summary>
            <div class="tr-more-menu">
              <button type="button" @click="$emit('regenerate', plan)">
                <TravelIcon name="refresh" :size="14" /> 重新拉取资料并生成
              </button>
              <button v-if="isWeekend" type="button" @click="$emit('replace-weekend', plan.id)">
                <TravelIcon name="refresh" :size="14" /> 换一个目的地
              </button>
              <button v-if="isWeekend" type="button" @click="$emit('refine-weekend', plan.id)">
                <TravelIcon name="sparkles" :size="14" /> AI 细化这个周末
              </button>
              <button v-if="isWeekend" type="button" @click="$emit('toggle-lock', plan.id)">
                <TravelIcon :name="plan.locked ? 'unlock' : 'lock'" :size="14" />
                {{ plan.locked ? '解锁' : '锁定这个周末' }}
              </button>
              <button type="button" class="is-danger" @click="$emit('delete', plan.id)">
                <TravelIcon name="trash" :size="14" /> 删除计划
              </button>
            </div>
          </details>
        </div>
      </div>

      <div class="tr-ticket-meta">
        <div><small>日期</small><span>{{ dateText }}</span></div>
        <div><small>人数</small><span>{{ people }} 人</span></div>
        <div><small>方式</small><span>{{ transportLabel(plan.transport) }}</span></div>
        <div><small>预算</small><span>¥{{ money(plan.budget?.total) }}</span></div>
        <div><small>人均</small><span>¥{{ money(plan.budget?.perPerson) }}</span></div>
      </div>
      <div class="tr-data-freshness">
        <span class="tr-freshness-dot"></span>
        <span>资料更新于 {{ timeAgo(plan.factsMeta?.collectedAt) }}</span>
        <span>· {{ mapSourceLabel(plan.source) }}</span>
        <span>· 天气/路况为生成时快照（路线缓存 2 小时）</span>
        <span>· POI 缓存 7 天</span>
        <span v-if="plan.factsMeta?.notes?.length">· {{ plan.factsMeta.notes[0] }}</span>
      </div>
      <div class="tr-perforation" aria-hidden="true"></div>
    </header>

    <div v-if="isWeekend && plan.detailLevel === 'draft'" class="tr-draft-banner">
      <TravelIcon name="sparkles" :size="18" />
      <div>
        <strong>这份周末计划还是班次表草稿</strong>
        <p>已经可以选择执行；点“AI 细化”会重新核对天气、路线并把每个时间段排细。</p>
      </div>
      <button type="button" class="tr-btn is-primary is-small" @click="$emit('refine-weekend', plan.id)">AI 细化</button>
    </div>

    <nav class="tr-tabs" aria-label="行程视图">
      <button
        v-for="tab in tabs"
        :key="tab.key"
        type="button"
        :class="{ active: activeTab === tab.key }"
        @click="activeTab = tab.key"
      >
        <TravelIcon :name="tab.icon" :size="15" /> {{ tab.label }}
        <small v-if="tab.count != null">{{ tab.count }}</small>
      </button>
    </nav>

    <div v-if="activeTab === 'itinerary'" class="tr-tab-panel">
      <div v-if="plan.weather?.length" class="tr-weather-strip">
        <div v-for="item in plan.weather" :key="item.date" class="tr-weather-day">
          <span class="tr-weather-date">{{ shortDate(item.date) }}</span>
          <TravelIcon :name="weatherIcon(item)" :size="22" />
          <strong>{{ item.dayWeather }}</strong>
          <span>{{ item.nightTemp ?? '—' }}~{{ item.dayTemp ?? '—' }}℃</span>
          <small>{{ item.wind }}</small>
        </div>
      </div>

      <div class="tr-day-switcher">
        <button
          v-for="(day, index) in days"
          :key="day.date"
          type="button"
          :class="{ active: activeDay === index }"
          @click="activeDay = index"
        >
          <small>DAY {{ String(index + 1).padStart(2, '0') }}</small>
          <strong>{{ shortDate(day.date) }}</strong>
          <span>{{ day.theme }}</span>
        </button>
        <button type="button" class="tr-all-days" :class="{ active: showAllDays }" @click="showAllDays = !showAllDays">
          <TravelIcon name="book" :size="14" />
          {{ showAllDays ? '只看一天' : '展开全部' }}
        </button>
      </div>

      <div v-if="!showAllDays && currentDay" class="tr-day-wrap">
        <div class="tr-day-summary">
          <span><TravelIcon name="route" :size="14" /> {{ dayDistance }} km</span>
          <span><TravelIcon name="clock" :size="14" /> {{ dayDuration }}</span>
          <span><TravelIcon name="pin" :size="14" /> {{ daySpots }} 个停留点</span>
          <span v-if="currentDay.weather"><TravelIcon :name="weatherIcon(currentDay.weather)" :size="14" /> {{ currentDay.weather.dayWeather }}</span>
        </div>
        <DayTimeline :day="currentDay" :plan="plan" />
      </div>

      <div v-else class="tr-all-day-list">
        <section v-for="(day, index) in days" :key="day.date" class="tr-day-wrap">
          <div class="tr-all-day-head">
            <span>DAY {{ String(index + 1).padStart(2, '0') }}</span>
            <strong>{{ shortDate(day.date) }} · {{ day.theme }}</strong>
          </div>
          <DayTimeline :day="day" :plan="plan" />
        </section>
      </div>
    </div>

    <div v-else-if="activeTab === 'map'" class="tr-tab-panel">
      <PlanMap :plan="plan" :day-index="showAllDays ? null : activeDay" />
    </div>

    <div v-else-if="activeTab === 'budget'" class="tr-tab-panel">
      <BudgetPanel :plan="plan" @update="$emit('update-budget', $event)" />
    </div>

    <div v-else-if="activeTab === 'prep'" class="tr-tab-panel">
      <PrepPanel :plan="plan" @toggle-check="$emit('toggle-check', $event)" />
    </div>

    <div v-else-if="activeTab === 'share'" class="tr-tab-panel">
      <SharePanel
        :plan="plan"
        :is-mock="isMock"
        @update-plan="$emit('update-plan', $event)"
        @toast="(text, type) => $emit('toast', text, type)"
      />
    </div>
  </div>
</template>

<script setup>
import { computed, ref, watch } from 'vue'
import TravelIcon from './TravelIcon.vue'
import DayTimeline from './DayTimeline.vue'
import PlanMap from './PlanMap.vue'
import BudgetPanel from './BudgetPanel.vue'
import PrepPanel from './PrepPanel.vue'
import SharePanel from './SharePanel.vue'
import { cityCode, mapSourceLabel, peopleCount, ticketCities, transportLabel } from './travelShare.js'
import { durationText, money, shortDate, timeAgo, weekdayCn } from './travelUtils.js'

const props = defineProps({
  plan: { type: Object, required: true },
  isMock: Boolean,
})

defineEmits([
  'back',
  'update-budget',
  'update-plan',
  'toggle-check',
  'delete',
  'regenerate',
  'print',
  'refine-weekend',
  'replace-weekend',
  'toggle-lock',
  'toast',
])

const activeTab = ref('itinerary')
const activeDay = ref(0)
const showAllDays = ref(false)
const days = computed(() => props.plan.days || [])

const tabs = computed(() => [
  { key: 'itinerary', label: '行程', icon: 'route', count: days.value.length },
  { key: 'map', label: '地图', icon: 'map' },
  { key: 'budget', label: '预算', icon: 'wallet' },
  { key: 'prep', label: '行前', icon: 'backpack' },
  { key: 'share', label: '分享', icon: 'share' },
])

const cities = computed(() => ticketCities(props.plan))
const isWeekend = computed(() => props.plan.kind === 'weekend')
const people = computed(() => peopleCount(props.plan))
const currentDay = computed(() => days.value[activeDay.value] || days.value[0] || null)
const dateText = computed(() => {
  if (!props.plan.startDate) return '日期待定'
  const start = shortDate(props.plan.startDate)
  if (!props.plan.endDate || props.plan.endDate === props.plan.startDate) {
    return `${start} · ${weekdayCn(props.plan.startDate)}`
  }
  return `${start} — ${shortDate(props.plan.endDate)} · ${(props.plan.days?.length || 1)} 天`
})
const transportIcon = computed(() => {
  const mode = props.plan.transport
  return mode === 'transit' ? 'train' : mode === 'walking' ? 'walk' : mode === 'tour' ? 'users' : 'car'
})
const dayDistance = computed(() => {
  const total = (currentDay.value?.legs || []).reduce((sum, leg) => sum + (Number(leg.distance) || 0), 0)
  return total ? total.toFixed(1) : '0'
})
const dayDuration = computed(() => {
  const total = (currentDay.value?.legs || []).reduce((sum, leg) => sum + (Number(leg.duration) || 0), 0)
  return total ? durationText(total) : '无路段'
})
const daySpots = computed(
  () => (currentDay.value?.blocks || []).filter((block) => block.poi && block.kind !== 'hotel').length,
)

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

watch(
  () => props.plan.id,
  () => {
    activeTab.value = 'itinerary'
    activeDay.value = 0
    showAllDays.value = false
  },
)
</script>
