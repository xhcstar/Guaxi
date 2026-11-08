<template>
  <div class="tr-home">
    <section class="tr-hero">
      <div class="tr-hero-copy">
        <div class="tr-eyebrow"><span></span> XINGJI · TRAVEL JOURNAL</div>
        <h1>把周末和远方，<br />排成一趟<span class="tr-underline">可以出发</span>的车。</h1>
        <p>
          告诉我从哪里出发、想花多少钱、想怎么玩。行迹会把真实地点、逐段路线、天气、吃饭和住宿
          排成一张能直接执行的行程单。
        </p>
        <div class="tr-hero-actions">
          <button type="button" class="tr-btn is-primary is-large" @click="$emit('new-trip')">
            <TravelIcon name="compass" :size="18" /> 规划一次出行
          </button>
          <button type="button" class="tr-btn is-outline is-large" @click="$emit('new-weekend')">
            <TravelIcon name="calendar" :size="18" /> 排未来 8 个周末
          </button>
        </div>
        <div class="tr-hero-meta">
          <span><TravelIcon name="map" :size="14" /> {{ mapProviderLabel(preferences.mapProvider || 'auto') }}</span>
          <span><TravelIcon name="sparkles" :size="14" /> {{ settings.providerLabel || '等待配置模型' }}</span>
          <span v-if="isMock" class="is-accent">浏览器预览模式</span>
        </div>
      </div>

      <div class="tr-hero-ticket" aria-hidden="true">
        <div class="tr-hero-ticket-top">
          <span>BOARDING PASS</span>
          <span>{{ nextPlan ? 'NEXT DEPARTURE' : 'BLANK TICKET' }}</span>
        </div>
        <div class="tr-hero-route">
          <div>
            <strong>{{ heroFrom }}</strong>
            <small>{{ heroFromCode }}</small>
          </div>
          <div class="tr-hero-route-line">
            <i></i>
            <TravelIcon name="trainFront" :size="18" />
            <i></i>
          </div>
          <div>
            <strong>{{ heroTo }}</strong>
            <small>{{ heroToCode }}</small>
          </div>
        </div>
        <div class="tr-hero-ticket-grid">
          <div><small>DATE</small><span>{{ nextPlan ? shortDate(nextPlan.startDate) : '待定' }}</span></div>
          <div><small>PARTY</small><span>{{ nextPlan ? `${people(nextPlan)} 人` : '—' }}</span></div>
          <div><small>CLASS</small><span>{{ nextPlan ? transportLabel(nextPlan.transport) : '—' }}</span></div>
        </div>
        <div class="tr-hero-ticket-bottom">
          <span class="tr-stamp" :class="{ 'is-empty': !nextPlan }">{{ nextPlan ? '已规划' : '待启程' }}</span>
          <span class="tr-hero-ticket-note">行迹 · 把时间花在路上，而不是做攻略上</span>
        </div>
        <svg class="tr-hero-contour" viewBox="0 0 420 260" xmlns="http://www.w3.org/2000/svg">
          <g fill="none" stroke="rgba(28,42,39,.16)" stroke-width="1">
            <path d="M-20 190 C70 120 120 230 210 160 S330 90 450 170" />
            <path d="M-20 210 C70 140 120 250 210 180 S330 110 450 190" />
            <path d="M-20 230 C70 160 120 270 210 200 S330 130 450 210" />
            <circle cx="300" cy="70" r="26" />
            <circle cx="300" cy="70" r="42" />
            <circle cx="300" cy="70" r="58" />
          </g>
        </svg>
      </div>
    </section>

    <section class="tr-mode-grid">
      <button type="button" class="tr-mode-card is-trip" @click="$emit('new-trip')">
        <div class="tr-mode-kicker">01 / LONG WEEKEND</div>
        <div class="tr-mode-icon"><TravelIcon name="compass" :size="26" /></div>
        <h2>长假出游</h2>
        <p>从出发地到目的地，逐天安排路线、天气、餐饮、住宿、预算和行前清单。适合 2-7 天的认真旅行。</p>
        <span class="tr-mode-link">开始规划 <TravelIcon name="arrowRight" :size="15" /></span>
      </button>
      <button type="button" class="tr-mode-card is-weekend" @click="$emit('new-weekend')">
        <div class="tr-mode-kicker">02 / WEEKEND SHUTTLE</div>
        <div class="tr-mode-icon"><TravelIcon name="calendar" :size="26" /></div>
        <h2>周末班次</h2>
        <p>以你所在的城市为圆心，检索周边 30-100 km，把未来 8 个周末排成一张班次表。周末不用再想“去哪”。</p>
        <span class="tr-mode-link">生成班次 <TravelIcon name="arrowRight" :size="15" /></span>
      </button>
    </section>

    <section v-if="nextPlan && upcoming" class="tr-reminder">
      <div class="tr-reminder-icon"><TravelIcon name="clock" :size="20" /></div>
      <div class="tr-reminder-copy">
        <strong>{{ nextPlan.title || '下一趟行程' }}</strong>
        <span>{{ reminderText }}</span>
      </div>
      <button type="button" class="tr-btn is-small is-outline" @click="$emit('open-plan', nextPlan.id)">查看行程</button>
    </section>

    <section class="tr-recent">
      <div class="tr-section-head">
        <div>
          <div class="tr-eyebrow"><span></span> YOUR PLANS</div>
          <h2>{{ plans.length ? '最近计划' : '从一张空白车票开始' }}</h2>
        </div>
        <div class="tr-section-actions">
          <label v-if="plans.length" class="tr-search">
            <TravelIcon name="search" :size="14" />
            <input v-model="query" type="search" placeholder="搜索计划或目的地" />
          </label>
          <button v-if="weekendCount" type="button" class="tr-link-btn" @click="$emit('open-weekend')">
            周末班次（{{ weekendCount }}）
          </button>
          <button v-if="!plans.length" type="button" class="tr-link-btn" @click="$emit('load-demo')">
            先看一份杭州示例
          </button>
        </div>
      </div>

      <div v-if="recentPlans.length" class="tr-plan-grid">
        <button
          v-for="(plan, index) in recentPlans"
          :key="plan.id"
          type="button"
          class="tr-plan-card"
          :style="{ '--i': index }"
          @click="$emit('open-plan', plan.id)"
        >
          <div class="tr-plan-card-top">
            <span class="tr-plan-kind" :class="plan.kind === 'weekend' ? 'is-weekend' : ''">
              {{ plan.kind === 'weekend' ? '周末' : '行程' }}
            </span>
            <span class="tr-plan-date">
              {{ plan.startDate ? shortDate(plan.startDate) : '未定' }}
              <em v-if="plan.adoptedAt" class="tr-plan-adopted">已采纳</em>
            </span>
          </div>
          <h3>{{ plan.title || plan.destinationName || '未命名行程' }}</h3>
          <p>{{ plan.destinationName || plan.originName || '还没有目的地' }}</p>
          <div class="tr-plan-card-foot">
            <span>{{ plan.dayCount || 1 }} 天 · {{ people(plan) }} 人</span>
            <span>{{ plan.budget?.total || plan.budgetTotal ? `¥${money(plan.budget?.total || plan.budgetTotal)}` : '预算待算' }}</span>
          </div>
        </button>
      </div>

      <div v-else-if="query.trim()" class="tr-search-empty">
        没有匹配「{{ query.trim() }}」的计划。
      </div>

      <div v-else class="tr-empty">
        <div class="tr-empty-illustration">
          <svg viewBox="0 0 260 120" xmlns="http://www.w3.org/2000/svg">
            <path d="M20 88 C80 30 120 110 180 52 S240 30 248 62" fill="none" stroke="rgba(196,85,45,.55)" stroke-width="2" stroke-dasharray="7 7"/>
            <circle cx="20" cy="88" r="6" fill="#1c2a27"/><circle cx="248" cy="62" r="6" fill="#4f6b57"/>
            <rect x="86" y="30" width="90" height="56" rx="6" fill="none" stroke="rgba(28,42,39,.35)" stroke-dasharray="4 5"/>
            <text x="100" y="63" font-size="13" fill="#1c2a27">空白车票</text>
          </svg>
        </div>
        <h3>把下一次出发，变成一张具体的票</h3>
        <p>先选一个目的地，行迹会帮你把路线、天气、吃饭、住宿和预算一次排好。</p>
        <div class="tr-empty-actions">
          <button type="button" class="tr-btn is-primary" @click="$emit('new-trip')">规划第一次出行</button>
          <button type="button" class="tr-btn is-ghost" @click="$emit('load-demo')">查看示例</button>
        </div>
      </div>
    </section>

    <section v-if="historyItems.length" class="tr-recent tr-history">
      <div class="tr-section-head">
        <div>
          <div class="tr-eyebrow"><span></span> HISTORY</div>
          <h2>采纳历史</h2>
        </div>
        <span class="tr-history-hint">每次采纳都会留一个版本，之后改动不会覆盖它</span>
      </div>
      <div class="tr-history-list">
        <div v-for="item in historyItems" :key="item.id" class="tr-history-row">
          <div class="tr-history-main">
            <strong>{{ item.title || '未命名行程' }}</strong>
            <small>
              {{ item.destinationName || item.originName || '未记录路线' }}
              · {{ item.startDate ? shortDate(item.startDate) : '未定日期' }}
              · 采纳于 {{ timeAgo(item.adoptedAt) }}
            </small>
          </div>
          <button type="button" class="tr-btn is-ghost is-small" @click="$emit('restore-snapshot', item.id)">
            <TravelIcon name="refresh" :size="13" /> 恢复为新计划
          </button>
        </div>
      </div>
    </section>

    <section class="tr-source-strip">
      <div class="tr-source-strip-icon"><TravelIcon name="info" :size="20" /></div>
      <div>
        <strong>地图数据：{{ mapProviderLabel(preferences.mapProvider || 'auto') }}</strong>
        <p>
          新计划会按所选服务商检索；没有可用 Key 时自动回退到 OpenStreetMap / OSRM / Open-Meteo。
          Key 只保存在本机配置里，不会进入前端页面或分享内容。
          <button type="button" class="tr-link-btn" @click="$emit('open-settings')">去设置</button>
        </p>
      </div>
      <span class="tr-source-tag">{{ (preferences.mapProvider || 'auto').toUpperCase() }}</span>
    </section>
  </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import TravelIcon from './TravelIcon.vue'
import { peopleCount, ticketCities, transportLabel } from './travelShare.js'
import { money, shortDate, timeAgo, todayString } from './travelUtils.js'
import { cityCode } from './travelShare.js'
import { mapProviderLabel } from './travelPreferences.js'

const props = defineProps({
  plans: { type: Array, default: () => [] },
  snapshots: { type: Array, default: () => [] },
  settings: { type: Object, default: () => ({}) },
  preferences: { type: Object, default: () => ({}) },
  isMock: Boolean,
})

defineEmits(['new-trip', 'new-weekend', 'open-plan', 'load-demo', 'open-weekend', 'open-settings', 'restore-snapshot'])

const query = ref('')
const tripPlans = computed(() => props.plans.filter((plan) => plan.kind !== 'weekend'))
const weekendCount = computed(() => props.plans.filter((plan) => plan.kind === 'weekend').length)
const recentPlans = computed(() => {
  const keyword = query.value.trim()
  return [...props.plans]
    .filter((plan) => {
      if (!keyword) return true
      const haystack = `${plan.title || ''} ${plan.destinationName || ''} ${plan.originName || ''}`
      return haystack.includes(keyword)
    })
    .sort((a, b) => String(b.updatedAt || b.startDate).localeCompare(String(a.updatedAt || a.startDate)))
    .slice(0, keyword ? 12 : 4)
})
const historyItems = computed(() =>
  [...props.snapshots]
    .sort((a, b) => String(b.adoptedAt || b.updatedAt).localeCompare(String(a.adoptedAt || a.updatedAt)))
    .slice(0, 12),
)
const upcomingPlans = computed(() => {
  const today = todayString()
  return props.plans
    .filter((plan) => plan.startDate && plan.startDate >= today)
    .sort((a, b) => String(a.startDate).localeCompare(String(b.startDate)))
})
const nextPlan = computed(() => upcomingPlans.value[0] || null)
const upcoming = computed(() => Boolean(nextPlan.value))
const reminderText = computed(() => {
  if (!nextPlan.value?.startDate) return ''
  const days = Math.ceil(
    (new Date(`${nextPlan.value.startDate}T12:00:00`) - new Date()) / (24 * 3600 * 1000),
  )
  if (days <= 0) return '今天出发，检查证件和车票。'
  if (days === 1) return '明天出发，今晚把行李收好。'
  return `${days} 天后出发，预订和门票可以开始确认了。`
})
const heroFrom = computed(() => (nextPlan.value ? ticketCities(nextPlan.value).from : '出发地'))
const heroTo = computed(() => (nextPlan.value ? ticketCities(nextPlan.value).to : '目的地'))
const heroFromCode = computed(() => (nextPlan.value ? cityCode(heroFrom.value) : '---'))
const heroToCode = computed(() => (nextPlan.value ? cityCode(heroTo.value) : '---'))

function people(plan) {
  return peopleCount(plan) || 1
}
</script>
