<template>
  <div class="tr-app" :class="{ 'is-busy': state.busy, 'is-preview': api.isMock, 'is-rail-collapsed': railCollapsed }">
    <aside class="tr-rail" :style="railStyle">
      <div class="tr-rail-texture" aria-hidden="true"></div>
      <header class="tr-brand" @click="store.showHome()">
        <span class="tr-seal">G</span>
        <div class="tr-brand-text">
          <strong>Guaxi</strong>
          <small>GUAXI · TRAVEL JOURNAL</small>
        </div>
      </header>
      <div v-if="!railCollapsed" class="tr-rail-resize" title="拖动调整侧栏宽度" @mousedown="startRailResize"></div>

      <div class="tr-rail-actions">
        <button type="button" class="tr-btn is-primary" @click="store.newTrip('trip')">
          <TravelIcon name="plus" :size="16" />
          <span>规划一次出行</span>
        </button>
        <button type="button" class="tr-btn is-ghost" @click="openWeekend">
          <TravelIcon name="calendar" :size="16" />
          <span>周末班次表</span>
        </button>
      </div>

      <nav class="tr-rail-nav" aria-label="Guaxi 导航">
        <button type="button" :class="{ active: state.view === 'home' }" @click="store.showHome()">
          <TravelIcon name="book" :size="15" /><span>计划库</span>
        </button>
        <button
          type="button"
          :class="{ active: state.view === 'weekend' || state.view === 'wizard' && state.wizard?.mode === 'weekend' }"
          @click="openWeekend"
        >
          <TravelIcon name="ticket" :size="15" /><span>周末班次</span>
        </button>
        <button
          type="button"
          :class="{ active: state.view === 'settings' }"
          @click="state.view = 'settings'"
        >
          <TravelIcon name="settings" :size="15" /><span>设置</span>
        </button>
      </nav>

      <div class="tr-rail-list">
        <div class="tr-rail-list-head">
          <span>最近计划</span>
          <small>{{ adoptedPlans.length }}</small>
        </div>
        <div v-if="!libraryPlans.length" class="tr-rail-empty">
          还没有已采纳的计划。<br />生成方案后可在计划页确认采纳。
        </div>
        <button
          v-for="item in libraryPlans"
          :key="item.id"
          type="button"
          class="tr-rail-plan"
          :class="{ active: state.plan?.id === item.id }"
          @click="store.openPlan(item.id)"
        >
          <span class="tr-rail-plan-mark" :class="item.kind === 'weekend' ? 'is-weekend' : ''"></span>
          <span class="tr-rail-plan-main">
            <strong>{{ item.title || item.destinationName || '未命名行程' }}</strong>
            <small>{{ railMeta(item) }}</small>
          </span>
          <TravelIcon name="chevronRight" :size="14" />
        </button>
      </div>

      <footer class="tr-rail-foot">
        <button type="button" class="tr-source-pill" @click="state.view = 'settings'">
          <span class="tr-source-dot" :class="state.preferences.mapProvider === 'osm' ? 'is-osm' : 'is-amap'"></span>
          <span>{{ mapProviderLabel(state.preferences.mapProvider) }}</span>
        </button>
        <small>{{ state.settings.providerLabel || '模型未配置' }}</small>
      </footer>
    </aside>

    <main class="tr-main">
      <header class="tr-topbar">
        <div class="tr-topbar-left">
          <button
            type="button"
            class="tr-icon-btn tr-rail-toggle-btn"
            :title="railCollapsed ? '展开侧栏' : '折叠侧栏'"
            @click="toggleRail"
          >
            <TravelIcon :name="railCollapsed ? 'chevronRight' : 'chevronLeft'" :size="16" />
          </button>
          <div class="tr-crumb">
            <span class="tr-crumb-brand">Guaxi</span>
            <TravelIcon name="chevronRight" :size="12" />
            <span>{{ viewTitle }}</span>
            <span v-if="state.plan && state.view === 'plan'" class="tr-crumb-plan">{{ state.plan.title }}</span>
          </div>
        </div>
        <div class="tr-topbar-actions">
          <span v-if="api.isMock" class="tr-preview-pill">浏览器预览 · 内置数据</span>
          <button type="button" class="tr-icon-btn" title="设置" @click="state.view = 'settings'">
            <TravelIcon name="settings" :size="16" />
          </button>
        </div>
      </header>

      <div ref="scrollEl" class="tr-scroll">
        <TravelHome
          v-if="state.view === 'home'"
          :plans="state.plans"
          :snapshots="state.snapshots"
          :settings="state.settings"
          :preferences="state.preferences"
          :is-mock="api.isMock"
          @new-trip="store.newTrip('trip')"
          @new-weekend="store.newTrip('weekend')"
          @open-plan="store.openPlan"
          @load-demo="store.loadDemo"
          @open-weekend="openWeekend"
          @open-settings="state.view = 'settings'"
          @restore-snapshot="store.restoreSnapshot"
        />

        <TravelWizard
          v-else-if="state.view === 'wizard' && state.wizard"
          :draft="state.wizard"
          :settings="state.settings"
          :is-mock="api.isMock"
          :busy="state.busy"
          :search="store.searchPlaces"
          :preferences="state.preferences"
          @update="store.editWizard"
          @locate="store.locateOrigin"
          @submit="handleSubmit"
          @cancel="handleCancelWizard"
        />

        <PlanWorkspace
          v-else-if="state.view === 'plan' && state.plan"
          :plan="state.plan"
          :is-mock="api.isMock"
          :refining="state.weekend.refiningId === state.plan.id"
          @back="store.showHome()"
          @update-budget="store.updateBudget"
          @update-plan="store.updatePlan"
          @toast="store.showToast"
          @toggle-check="store.toggleChecklist"
          @delete="handleDelete"
          @regenerate="handleRegenerate"
          @print="handlePrint"
          @refine-weekend="store.refineWeekend"
          @replace-weekend="store.replaceWeekend"
          @toggle-lock="store.toggleWeekendLock"
          @rename-plan="handleRenamePlan"
          @duplicate-plan="store.duplicatePlan"
          @adopt-plan="store.adoptPlan"
        />

        <WeekendBoard
          v-else-if="state.view === 'weekend'"
          :plans="state.weekend.plans"
          :loading="state.weekend.loading"
          :refining-id="state.weekend.refiningId"
          :origin="state.defaultOrigin"
          :is-mock="api.isMock"
          @generate="handleWeekendGenerate"
          @open-plan="store.openPlan"
          @refine="store.refineWeekend"
          @replace="store.replaceWeekend"
          @toggle-lock="store.toggleWeekendLock"
          @home="store.showHome()"
        />

        <TravelSettings
          v-else-if="state.view === 'settings'"
          :settings="state.settings"
          :prefer-osm="state.preferOsm"
          :is-mock="api.isMock"
          :preferences="state.preferences"
          @set-prefer-osm="store.setPreferOsm"
          @save-preferences="store.savePreferences"
          @reset-preferences="store.resetPreferences"
          @home="store.showHome()"
        />
      </div>
    </main>

    <Transition name="tr-fade">
      <div v-if="state.error" class="tr-banner is-error">
        <TravelIcon name="warning" :size="17" />
        <span>{{ state.error }}</span>
        <button type="button" class="tr-link-btn" @click="store.loadDemo()">载入示例</button>
        <button type="button" class="tr-icon-btn" @click="state.error = ''"><TravelIcon name="x" :size="14" /></button>
      </div>
    </Transition>
    <Transition name="tr-fade">
      <div v-if="state.warning" class="tr-banner is-warning">
        <TravelIcon name="info" :size="17" />
        <span>{{ state.warning }}</span>
        <button type="button" class="tr-icon-btn" @click="state.warning = ''"><TravelIcon name="x" :size="14" /></button>
      </div>
    </Transition>

    <Transition name="tr-fade">
      <div v-if="state.busy" class="tr-progress-mask">
        <div class="tr-progress-card">
          <div class="tr-progress-illustration" aria-hidden="true">
            <svg viewBox="0 0 320 84" xmlns="http://www.w3.org/2000/svg">
              <path d="M8 64 C70 10, 120 78, 176 40 S270 20, 312 48" fill="none" stroke="rgba(196,85,45,.5)" stroke-width="2" stroke-dasharray="7 7" />
              <circle cx="8" cy="64" r="5" fill="#1c2a27" />
              <circle cx="176" cy="40" r="4" fill="#c4552d" />
              <circle cx="312" cy="48" r="5" fill="#4f6b57" />
              <text x="8" y="80" font-size="9" fill="#7b8a82">出发</text>
              <text x="284" y="70" font-size="9" fill="#7b8a82">抵达</text>
            </svg>
          </div>
          <div class="tr-progress-eyebrow">GUAXI IS PLANNING</div>
          <h3>{{ state.stage.label || '正在编排行程' }}</h3>
          <p v-if="state.stage.detail">{{ state.stage.detail }}</p>
          <div class="tr-progress-bar" :class="{ 'is-indeterminate': !state.stage.percent }">
            <i :style="{ width: `${Math.max(3, state.stage.percent || 0)}%` }"></i>
          </div>
          <div class="tr-progress-foot">
            <span>{{ Math.round(state.stage.percent || 0) }}% · 已等待 {{ elapsedSeconds }}s</span>
            <div class="tr-progress-foot-right">
              <span class="tr-progress-hint">可以先去泡杯茶，路线和预算会一起算好</span>
              <button type="button" class="tr-progress-cancel" @click="store.cancelGeneration()">取消</button>
            </div>
          </div>
        </div>
      </div>
    </Transition>

    <Transition name="tr-toast">
      <div v-if="state.toast.show" class="tr-toast" :class="`is-${state.toast.type}`">
        <span class="tr-toast-dot"></span>
        {{ state.toast.text }}
      </div>
    </Transition>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { api } from './travel/travelApi.js'
import { useTravelStore } from './travel/travelStore.js'
import { mapProviderLabel } from './travel/travelPreferences.js'
import { shortDate, todayString } from './travel/travelUtils.js'
import { buildPlanPosterMap } from './travel/travelShare.js'
import TravelIcon from './travel/TravelIcon.vue'
import TravelHome from './travel/TravelHome.vue'
import TravelWizard from './travel/TravelWizard.vue'
import PlanWorkspace from './travel/PlanWorkspace.vue'
import WeekendBoard from './travel/WeekendBoard.vue'
import TravelSettings from './travel/TravelSettings.vue'
import './travel/travel.css'

const store = useTravelStore()
const { state } = store
const scrollEl = ref(null)

// 进度卡按秒刷新，让长等待看起来是“还在跑”，而不是卡死。
const nowTs = ref(Date.now())
const elapsedSeconds = computed(() => {
  const startedAt = state.stage.startedAt
  if (!startedAt) return 0
  return Math.max(0, Math.floor((nowTs.value - startedAt) / 1000))
})
let stageTimer = null
watch(
  () => state.busy,
  (busy) => {
    if (stageTimer) {
      clearInterval(stageTimer)
      stageTimer = null
    }
    if (!busy) return
    nowTs.value = Date.now()
    stageTimer = setInterval(() => {
      nowTs.value = Date.now()
    }, 1000)
  },
)

const RAIL_WIDTH_KEY = 'guaxi.travel.railWidth.v1'
const RAIL_COLLAPSED_KEY = 'guaxi.travel.railCollapsed.v1'
const RAIL_MIN = 220
const RAIL_MAX = 420
const railWidth = ref(Number(localStorage.getItem(RAIL_WIDTH_KEY)) || 274)
const railCollapsed = ref(localStorage.getItem(RAIL_COLLAPSED_KEY) === '1')
const railStyle = computed(() =>
  railCollapsed.value
    ? { flex: '0 0 64px', width: '64px' }
    : { flex: `0 0 ${railWidth.value}px`, width: `${railWidth.value}px` },
)
let railResizing = false
let railStartX = 0
let railStartWidth = 0

function toggleRail() {
  railCollapsed.value = !railCollapsed.value
  localStorage.setItem(RAIL_COLLAPSED_KEY, railCollapsed.value ? '1' : '0')
}

function startRailResize(event) {
  if (railCollapsed.value) return
  railResizing = true
  railStartX = event.clientX
  railStartWidth = railWidth.value
  document.addEventListener('mousemove', onRailResize)
  document.addEventListener('mouseup', stopRailResize)
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
  event.preventDefault()
}

function onRailResize(event) {
  if (!railResizing) return
  railWidth.value = Math.min(RAIL_MAX, Math.max(RAIL_MIN, railStartWidth + event.clientX - railStartX))
}

function stopRailResize() {
  if (!railResizing) return
  railResizing = false
  document.removeEventListener('mousemove', onRailResize)
  document.removeEventListener('mouseup', stopRailResize)
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
  localStorage.setItem(RAIL_WIDTH_KEY, String(railWidth.value))
}

const viewTitle = computed(() => {
  if (state.view === 'wizard') return state.wizard?.mode === 'weekend' ? '生成周末班次' : '规划新行程'
  if (state.view === 'plan') return state.plan?.kind === 'weekend' ? '周末计划' : '行程详情'
  if (state.view === 'weekend') return '周末班次表'
  if (state.view === 'settings') return '设置'
  return '计划库'
})

// 只有用户确认采纳的计划才进计划库；生成出来的方案先留在草稿里。
const adoptedPlans = computed(() => state.plans.filter((plan) => plan.adoptedAt))
const libraryPlans = computed(() => {
  return [...adoptedPlans.value]
    .sort((a, b) => {
      const aDate = a.kind === 'weekend' ? a.weekendStart || a.startDate : a.startDate
      const bDate = b.kind === 'weekend' ? b.weekendStart || b.startDate : b.startDate
      return String(bDate).localeCompare(String(aDate))
    })
    .slice(0, 14)
})

function railMeta(item) {
  const date = item.kind === 'weekend' ? item.weekendStart : item.startDate
  const when = date ? shortDate(date) : '未定日期'
  const where = item.destinationName || item.originName || ''
  return `${when}${where ? ` · ${where}` : ''}`
}

async function openWeekend() {
  await store.openWeekendBoard()
  await nextTick()
  scrollEl.value?.scrollTo({ top: 0 })
}

async function handleSubmit(form) {
  if (form.mode === 'weekend') {
    await store.generateWeekend(form)
  } else {
    await store.generateTrip(form)
  }
}

function handleCancelWizard() {
  store.showHome()
}

function handleDelete(planId) {
  if (!window.confirm('删除这份计划？删除后无法恢复。')) return
  store.deletePlan(planId)
}

function handleRegenerate(plan) {
  const isWeekend = plan.kind === 'weekend'
  store.newTrip(isWeekend ? 'weekend' : 'trip')
  Object.assign(store.state.wizard, {
    mode: isWeekend ? 'weekend' : 'trip',
    origin: plan.origin,
    destination: isWeekend ? plan.origin : plan.destination,
    startDate: plan.startDate,
    endDate: plan.endDate,
    startTime: plan.startTime,
    endTime: plan.endTime,
    transport: plan.transport,
    budgetLevel: plan.budgetLevel,
    adults: plan.travelers?.adults || 1,
    children: plan.travelers?.children || 0,
    elders: plan.travelers?.elders || 0,
    interests: plan.interests || [],
    pace: plan.pace || 'balanced',
    source: plan.source === 'osm' ? 'osm' : 'auto',
  })
}

function handleRenamePlan({ id, title } = {}) {
  if (id && title) store.renamePlan(id, title)
}

async function handlePrint(plan) {
  const theme = document.documentElement.dataset.guaxiTheme === 'dark' ? 'dark' : 'light'
  try {
    // 打印用同一张长图地图：路线样式改版后先补一张新的（绿色路线）。
    if (!api.isMock && plan?.source === 'amap' && (!plan.posterMap || plan.posterMapVersion !== 2)) {
      const dataUrl = await buildPlanPosterMap(plan, api)
      if (dataUrl) {
        await store.updatePlan(plan, { posterMap: dataUrl, posterMapVersion: 2 })
      }
    }
    await api.openPrintWindow(plan.id, theme)
  } catch (error) {
    store.showToast(`打开打印窗口失败：${error?.message || error}`, 'error')
  }
}

function handleWeekendGenerate() {
  store.newTrip('weekend')
}

onMounted(async () => {
  await store.init()
  const params = new URLSearchParams(window.location.search)
  if (params.get('demo') === '1') {
    await store.loadDemo()
  }
  const previewView = params.get('view')
  const previewPlanId = params.get('planId')
  if (previewPlanId) {
    await store.openPlan(previewPlanId)
  } else if (previewView === 'wizard') {
    store.newTrip(params.get('mode') === 'weekend' ? 'weekend' : 'trip')
  } else if (previewView === 'plan' && state.plans[0]) {
    await store.openPlan(state.plans[0].id)
  } else if (previewView === 'weekend') {
    if (params.get('generate') === '1') {
      store.newTrip('weekend')
      await store.generateWeekend(store.state.wizard)
    } else {
      await openWeekend()
    }
  } else if (previewView === 'settings') {
    state.view = 'settings'
  }
  // 近期出行提醒：只在首页出现一次，不打断用户。
  const upcoming = state.plans
    .filter((plan) => plan.startDate && plan.startDate >= todayString())
    .sort((a, b) => a.startDate.localeCompare(b.startDate))
  if (upcoming.length) {
    const days = Math.ceil(
      (new Date(`${upcoming[0].startDate}T12:00:00`) - new Date()) / (24 * 3600 * 1000),
    )
    if (days >= 0 && days <= 3) {
      store.showToast(`${upcoming[0].title || '下一趟行程'}还有 ${Math.max(0, days)} 天出发`, 'info', 4200)
    }
  }
})

onBeforeUnmount(() => {
  if (stageTimer) clearInterval(stageTimer)
  document.removeEventListener('mousemove', onRailResize)
  document.removeEventListener('mouseup', stopRailResize)
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
})
</script>
