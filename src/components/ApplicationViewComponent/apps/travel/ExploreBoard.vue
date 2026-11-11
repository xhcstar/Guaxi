<template>
  <div class="tr-explore">
    <section class="tr-explore-main">
      <header class="tr-explore-head">
        <div class="tr-eyebrow"><span></span> DESTINATION POOL</div>
        <h1>{{ heading }}</h1>
        <p>{{ subtitle }}</p>
        <div class="tr-explore-meta">
          <span><TravelIcon name="pin" :size="13" />{{ destinationName }}</span>
          <span v-if="bases.length"><TravelIcon name="route" :size="13" />{{ bases.length }} 个基地</span>
          <span><TravelIcon name="map" :size="13" />{{ sourceLabel }}</span>
          <span v-if="data?.truncated">候选较多，已保留前 {{ candidates.length }} 个</span>
        </div>
      </header>

      <div v-if="loading" class="tr-explore-loading">
        <div v-for="index in 6" :key="index" class="tr-explore-skeleton">
          <i></i><i></i><i></i>
        </div>
        <p>正在按类别检索目的地，同城检索会缓存 7 天。</p>
      </div>

      <template v-else>
        <div v-if="scope === 'province'" class="tr-explore-bases">
          <div class="tr-explore-bases-head">
            <div>
              <h2>{{ bases.length ? '还可以再加一个落脚城市' : '先定基地城市' }}</h2>
              <p v-if="bases.length">按基地分别扫描景点、餐饮、博物馆、街区与夜市，加得越多扫得越慢。</p>
              <p v-else>这个省太大，一次扫全省既慢又排不出行程。先点一个城市当落脚点（省会已排在最前），也可以直接搜索。</p>
            </div>
            <span class="tr-explore-bases-count">{{ bases.length }}/4</span>
          </div>

          <div v-if="!bases.length && baseCandidates.length" class="tr-explore-base-suggested">
            <p class="tr-explore-base-suggested-head">
              <TravelIcon name="sparkles" :size="13" />
              按离省会由近到远挑了这些，点一个就能开始：
            </p>
            <div class="tr-explore-base-suggested-grid">
              <button
                v-for="city in baseCandidates"
                :key="city.id || city.name"
                type="button"
                class="tr-explore-base-city"
                @click="chooseBase(city)"
              >
                <strong>{{ city.name }}</strong>
                <small>{{ cityCaption(city) }}</small>
              </button>
            </div>
          </div>

          <div class="tr-explore-base-chips">
            <button
              v-if="bases.length > 1"
              type="button"
              class="tr-chip is-plain"
              :class="{ active: activeBase === 'all' }"
              @click="$emit('switch-base', 'all')"
            >
              全部基地
            </button>
            <span
              v-for="(base, index) in bases"
              :key="base.id || base.name"
              class="tr-explore-base-tab"
              :class="{ active: isActiveBase(base) }"
            >
              <button
                type="button"
                class="tr-explore-base-tab-name"
                :title="`查看 ${base.name} 的候选（不重新检索）`"
                @click="$emit('switch-base', base.id || base.name)"
              >
                {{ base.name }}
              </button>
              <button
                type="button"
                class="tr-explore-base-tab-x"
                :title="`移除 ${base.name}`"
                @click="$emit('remove-base', index)"
              >
                <TravelIcon name="x" :size="11" />
              </button>
            </span>
            <span v-if="!bases.length" class="tr-explore-base-empty">还没有基地城市</span>
          </div>
          <div class="tr-explore-base-search">
            <TravelIcon name="search" :size="14" />
            <input
              v-model="baseKeyword"
              type="text"
              placeholder="搜索基地城市，例如：乌鲁木齐、伊宁、喀什"
              autocomplete="off"
              @input="onBaseInput(baseKeyword)"
              @keydown.esc="clearBaseSearch"
            />
            <div v-if="baseSuggestions.length" class="tr-explore-base-suggest">
              <button
                v-for="place in baseSuggestions"
                :key="place.id || place.name"
                type="button"
                @click="chooseBase(place)"
              >
                <TravelIcon name="pin" :size="13" />
                <span><strong>{{ place.name }}</strong><small>{{ place.city || place.address }}</small></span>
              </button>
            </div>
          </div>
        </div>

        <div class="tr-explore-command">
          <TravelIcon name="sparkles" :size="14" />
          <input
            v-model="commandText"
            type="text"
            placeholder="说一句话：重新检索 / 扩大到 150 公里 / 切换到伊犁 / 只要博物馆 / 帮我挑 / 清空"
            autocomplete="off"
            :disabled="commandBusy"
            @keydown.enter="runCommand"
          />
          <button
            type="button"
            class="tr-btn is-ghost is-small"
            :disabled="commandBusy || !commandText.trim()"
            @click="runCommand"
          >
            {{ commandBusy ? '解析中…' : '执行' }}
          </button>
        </div>
        <p v-if="commandFeedback" class="tr-explore-command-feedback">{{ commandFeedback }}</p>

        <template v-if="candidates.length">
          <div v-hscroll class="tr-explore-toolbar" aria-label="候选分类">
            <div class="tr-explore-cats">
              <button
                type="button"
                class="tr-chip"
                :class="{ active: activeCategory === 'all' }"
                @click="activeCategory = 'all'"
              >
                全部 <em>{{ candidates.length }}</em>
              </button>
              <button
                v-for="group in groups"
                :key="group.key"
                type="button"
                class="tr-chip"
                :class="{ active: activeCategory === group.key }"
                @click="activeCategory = group.key"
              >
                <TravelIcon :name="group.icon" :size="12" />{{ group.label }} <em>{{ group.count }}</em>
              </button>
            </div>
          </div>

          <div class="tr-explore-subbar">
            <div class="tr-chip-row">
              <button
                type="button"
                class="tr-chip is-plain"
                :class="{ active: filters.high }"
                @click="filters.high = !filters.high"
              >
                高分优先
              </button>
              <button
                type="button"
                class="tr-chip is-plain"
                :class="{ active: filters.indoor }"
                @click="filters.indoor = !filters.indoor"
              >
                室内
              </button>
              <button
                type="button"
                class="tr-chip is-plain"
                :class="{ active: filters.people }"
                @click="filters.people = !filters.people"
              >
                适合老幼
              </button>
              <button
                type="button"
                class="tr-chip is-plain"
                :class="{ active: filters.must }"
                @click="filters.must = !filters.must"
              >
                只看必去
              </button>
            </div>
            <div class="tr-explore-controls">
              <div class="tr-explore-search">
                <TravelIcon name="search" :size="13" />
                <input v-model="keyword" type="text" placeholder="搜索名称或区域" />
              </div>
              <label class="tr-explore-sort">
                <span>排序</span>
                <select v-model="sortKey">
                  <option v-for="option in SORTS" :key="option.value" :value="option.value">{{ option.label }}</option>
                </select>
              </label>
            </div>
          </div>

          <div class="tr-explore-batch">
            <span class="tr-explore-batch-label">批量（当前 {{ visible.length }} 张）</span>
            <button type="button" class="tr-chip is-plain" @click="batch('add')">全选</button>
            <button type="button" class="tr-chip is-plain" @click="batch('toggle')">反选</button>
            <button type="button" class="tr-chip is-plain" @click="batchHigh">加选高分</button>
            <button type="button" class="tr-chip is-plain" @click="batch('remove')">取消</button>
            <span class="tr-explore-batch-count">已勾 {{ selected.length }}</span>
          </div>

          <div class="tr-explore-grid">
            <article
              v-for="item in visible"
              :key="item.id || item.name"
              class="tr-explore-card"
              :class="{ 'is-selected': selectedSet.has(item.id) }"
              @click="$emit('toggle', item.id)"
            >
              <div class="tr-explore-card-top">
                <button type="button" class="tr-explore-check" @click.stop="$emit('toggle', item.id)">
                  <TravelIcon v-if="selectedSet.has(item.id)" name="check" :size="12" />
                </button>
                <span class="tr-explore-cat">
                  <TravelIcon :name="categoryMeta(item.categoryKey).icon" :size="12" />
                  {{ item.categoryLabel || categoryMeta(item.categoryKey).label }}
                </span>
                <span class="tr-explore-score" :class="scoreLevel(item.score)" :style="{ '--score': item.score }">
                  {{ item.score }}
                </span>
              </div>
              <strong class="tr-explore-name">{{ item.name }}</strong>
              <small class="tr-explore-addr">{{ item.address || item.city || item.baseName }}</small>
              <div class="tr-explore-facts">
                <span :class="{ 'is-unknown': !Number(item.rating) }">
                  {{ Number(item.rating) ? `★ ${Number(item.rating).toFixed(1)}` : '评分未核实' }}
                </span>
                <span>{{ Number(item.distanceBaseKm) || 0 }} km</span>
                <span>{{ durationText(item.suggestedDuration) }}</span>
                <span v-if="Number(item.cost) > 0">¥{{ Math.round(Number(item.cost)) }}</span>
              </div>
              <p class="tr-explore-reason">{{ item.reason }}</p>
              <p v-if="item.caution" class="tr-explore-caution">{{ item.caution }}</p>
              <div class="tr-explore-bars">
                <div v-for="metric in cardMetrics(item)" :key="metric.label">
                  <small>{{ metric.label }}</small>
                  <i><b :style="{ width: `${Math.round(metric.value * 100)}%` }"></b></i>
                </div>
              </div>
            </article>
          </div>
          <div v-if="!visible.length" class="tr-explore-empty">
            <TravelIcon name="search" :size="22" />
            <p>没有符合当前筛选的地点，换个类别或清掉筛选试试。</p>
          </div>
        </template>

        <div v-else-if="scope === 'province' && !bases.length" class="tr-explore-empty">
          <TravelIcon name="route" :size="24" />
          <template v-if="baseCandidates.length">
            <p>点上面任意一个城市当落脚点，我立刻按它检索景点、餐饮与住宿；也可以直接开始编排。</p>
            <button type="button" class="tr-btn is-ghost is-small" :disabled="busy" @click="$emit('fallback')">
              不等候选，直接按 {{ destinationName }} 编排
            </button>
          </template>
          <p v-else>添加基地城市后，会按基地分别扫描景点、餐饮、博物馆、街区与夜市。</p>
        </div>
        <div v-else class="tr-explore-empty">
          <TravelIcon name="warning" :size="24" />
          <p>{{ error || (bases.length ? `「${bases.map((item) => item.name).join('、')}」附近这次没扫到候选。可以直接编排，我会按目的地城市补齐地点；排不顺时会再和你商量怎么放宽条件。` : '这个范围内没有扫描到候选地点。可以直接编排，我会按目的地城市补齐地点；排不顺时会再和你商量怎么放宽条件。') }}</p>
          <div class="tr-explore-empty-actions">
            <button type="button" class="tr-btn is-ghost is-small" @click="$emit('refresh')">重新扫描</button>
            <button type="button" class="tr-btn is-primary is-small" :disabled="busy" @click="$emit('fallback')">
              直接编排
            </button>
          </div>
        </div>
      </template>
    </section>

    <aside class="tr-explore-aside">
      <div class="tr-explore-ticket">
        <div class="tr-live-ticket-top">
          <span>GUAXI · SHORTLIST</span>
          <span>{{ dayCount }} DAYS</span>
        </div>
        <h3>{{ selected.length }} 个地点</h3>
        <p class="tr-explore-ticket-meta">
          约 {{ statsValue.hours }} 小时 · {{ statsValue.categories }} 类<template v-if="statsValue.indoor"> · {{ statsValue.indoor }} 个室内</template>
        </p>
        <div class="tr-explore-selected">
          <button
            v-for="item in selectedPreview"
            :key="item.id"
            type="button"
            @click="$emit('toggle', item.id)"
          >
            <span>{{ item.name }}</span>
            <TravelIcon name="x" :size="11" />
          </button>
          <span v-if="selected.length > selectedPreview.length" class="tr-explore-more">
            还有 {{ selected.length - selectedPreview.length }} 个
          </span>
          <p v-if="!selected.length" class="tr-explore-empty-selection">
            还没选。可以逐个勾选，或让我按 {{ dayCount }} 天节奏挑一批。
          </p>
        </div>
        <div class="tr-explore-actions">
          <button
            v-if="candidates.length"
            type="button"
            class="tr-btn is-ghost is-small"
            @click="$emit('autopick')"
          >
            <TravelIcon name="sparkles" :size="14" />帮我挑
          </button>
          <button
            v-if="candidates.length"
            type="button"
            class="tr-btn is-ghost is-small"
            :disabled="!selected.length"
            @click="$emit('clear')"
          >
            清空
          </button>
          <button
            type="button"
            class="tr-btn is-primary is-large"
            :disabled="busy"
            @click="$emit('submit')"
          >
            {{ busy ? '正在编排…' : submitLabel }}
          </button>
          <button
            v-if="candidates.length"
            type="button"
            class="tr-btn is-ghost is-small"
            @click="$emit('skip')"
          >
            不挑，直接编排
          </button>
        </div>
        <p v-if="candidates.length" class="tr-explore-hint">
          {{ hotelCount }} 家住宿候选会自动带上；勾选的才会进入行程，没勾餐饮时会补 2 个备选。
        </p>
        <p v-if="isMock" class="tr-explore-hint">
          浏览器预览：候选为内置示例，桌面端会替换为高德/OSM 的真实检索结果。
        </p>
        <div class="tr-live-stamp">SHORTLIST</div>
      </div>

      <div v-if="notes.length" class="tr-explore-notes">
        <strong>数据说明</strong>
        <p v-for="note in notes" :key="note">{{ note }}</p>
      </div>

      <button type="button" class="tr-explore-back" @click="$emit('back')">
        <TravelIcon name="chevronLeft" :size="13" />返回修改条件
      </button>
    </aside>
  </div>
</template>

<script setup>
import { computed, reactive, ref } from 'vue'
import TravelIcon from './TravelIcon.vue'
import { categoryMeta, exploreStats, EXPLORE_CATEGORIES } from './travelExplore.js'
import { vHscroll } from './travelScrollAssist.js'
import './explore.css'

const props = defineProps({
  data: { type: Object, default: null },
  loading: Boolean,
  error: { type: String, default: '' },
  selectedIds: { type: Array, default: () => [] },
  form: { type: Object, default: () => ({}) },
  search: { type: Function, default: null },
  interpret: { type: Function, default: null },
  activeBase: { type: String, default: 'all' },
  busy: Boolean,
  isMock: Boolean,
  dayCount: { type: Number, default: 1 },
})

const emit = defineEmits([
  'toggle',
  'autopick',
  'clear',
  'submit',
  'skip',
  'back',
  'add-base',
  'remove-base',
  'refresh',
  'fallback',
  'batch',
  'switch-base',
  'expand',
])

const SORTS = [
  { value: 'score', label: '综合推荐' },
  { value: 'rating', label: '口碑优先' },
  { value: 'distance', label: '离基地近' },
  { value: 'duration', label: '耗时从短到长' },
  { value: 'cost', label: '花费从低到高' },
]

const activeCategory = ref('all')
const sortKey = ref('score')
const keyword = ref('')
const filters = reactive({ high: false, indoor: false, people: false, must: false })
const baseKeyword = ref('')
const baseSuggestions = ref([])
const commandText = ref('')
const commandBusy = ref(false)
const commandFeedback = ref('')
let baseTimer = null

const allCandidates = computed(() => props.data?.candidates || [])
// 基地视角：切换只过滤已检索到的卡片，不触发新的检索。
const candidates = computed(() => {
  const active = props.activeBase
  if (!active || active === 'all') return allCandidates.value
  return allCandidates.value.filter((item) => item.baseId === active || item.baseName === active)
})
const groups = computed(() => {
  const counts = new Map()
  for (const item of candidates.value) {
    const meta = categoryMeta(item.categoryKey)
    counts.set(meta.key, (counts.get(meta.key) || 0) + 1)
  }
  return EXPLORE_CATEGORIES.filter((meta) => counts.has(meta.key)).map((meta) => ({
    key: meta.key,
    label: meta.label,
    icon: meta.icon,
    count: counts.get(meta.key),
  }))
})
const bases = computed(() => props.data?.bases || [])
const baseCandidates = computed(() => (bases.value.length ? [] : props.data?.baseCandidates || []))
const scope = computed(() => props.data?.scope || 'poi')
const notes = computed(() => props.data?.notes || [])
const sourceLabel = computed(() => (props.data?.source === 'amap' ? '高德数据' : 'OpenStreetMap'))
const hotelCount = computed(() => (props.data?.hotels || []).length)
const destinationName = computed(
  () => props.data?.destination?.city || props.data?.destination?.name || props.form?.destination?.name || '',
)
const heading = computed(() => {
  if (scope.value === 'province' && !bases.value.length) return `${destinationName.value || '这个省'} · 先定基地`
  return `去 ${destinationName.value || '目的地'}，想停留的地方`
})
const subtitle = computed(() => {
  if (scope.value === 'province') {
    return '按基地城市分类扫描：自然景观、人文古迹、博物馆、街区、夜市、市场、徒步与演出都在里面。'
  }
  return '这些是从真实检索里筛出来的地点，附上口碑、到达、花费、时长和老幼适配的综合判断。勾选后才会进入编排。'
})
const statsValue = computed(() => exploreStats(selected.value))

const selectedSet = computed(() => new Set(props.selectedIds || []))
// 短名单统计跨基地的全部勾选，不随当前基地视角缩水。
const selected = computed(() => allCandidates.value.filter((item) => selectedSet.value.has(item.id)))
const selectedPreview = computed(() => selected.value.slice(0, 8))
const submitLabel = computed(() => {
  if (!candidates.value.length) return '按当前条件直接编排'
  return selected.value.length ? `用这 ${selected.value.length} 个地点编排` : '帮我挑一批并编排'
})

function isActiveBase(base) {
  return (base.id || base.name) === props.activeBase
}

function batch(mode) {
  emit('batch', visible.value.map((item) => item.id), mode)
}

function batchHigh() {
  emit('batch', visible.value.filter((item) => item.score >= 80).map((item) => item.id), 'add')
}

async function runCommand() {
  const text = commandText.value.trim()
  if (!text || commandBusy.value) return
  commandBusy.value = true
  commandFeedback.value = ''
  try {
    const fn = props.interpret
    const action = typeof fn === 'function' ? await fn(text) : { action: 'unknown', message: '指令解析不可用。' }
    applyCommand(action || { action: 'unknown' })
  } catch (error) {
    commandFeedback.value = `解析失败：${error?.message || error}`
  } finally {
    commandBusy.value = false
  }
}

function applyCommand(action) {
  switch (action.action) {
    case 'refresh':
      emit('refresh')
      commandFeedback.value = '重新扫描全部基地（保留已勾）。'
      break
    case 'expand':
      emit('expand', action.km > 0 ? action.km : 120)
      commandFeedback.value = `扩大检索范围到 ${action.km > 0 ? action.km : 120} km 并重扫。`
      break
    case 'switch':
      emit('switch-base', action.base || 'all')
      commandFeedback.value = action.base === 'all' ? '已切到全部基地。' : `已切到 ${action.base} 的候选（未重搜）。`
      break
    case 'category':
      activeCategory.value = action.key
      commandFeedback.value = `只看「${categoryMeta(action.key).label}」。`
      break
    case 'filter':
      if (action.key in filters) filters[action.key] = !filters[action.key]
      commandFeedback.value = `已切换筛选：${action.key}。`
      break
    case 'pick':
      emit('autopick')
      commandFeedback.value = '已按天数挑一批。'
      break
    case 'clear':
      emit('clear')
      commandFeedback.value = '已清空勾选。'
      break
    case 'batch':
      batch(action.mode || 'add')
      commandFeedback.value = '已批量勾选当前卡片。'
      break
    case 'noop':
      break
    default:
      commandFeedback.value = action.message || '没听懂这条指令。'
      return
  }
  commandText.value = ''
}

const visible = computed(() => {
  let list = candidates.value
  if (activeCategory.value !== 'all') {
    list = list.filter((item) => item.categoryKey === activeCategory.value)
  }
  if (filters.high) list = list.filter((item) => item.score >= 75)
  if (filters.indoor) list = list.filter((item) => item.indoor)
  if (filters.people) list = list.filter((item) => (item.metrics?.people ?? 0) >= 0.7)
  if (filters.must) list = list.filter((item) => item.must)
  const text = keyword.value.trim().toLowerCase()
  if (text) {
    list = list.filter((item) =>
      `${item.name || ''} ${item.address || ''} ${item.category || ''}`.toLowerCase().includes(text),
    )
  }
  const sorted = [...list]
  if (sortKey.value === 'rating') {
    sorted.sort((a, b) => (Number(b.rating) || 0) - (Number(a.rating) || 0))
  } else if (sortKey.value === 'distance') {
    sorted.sort((a, b) => (Number(a.distanceBaseKm) || 0) - (Number(b.distanceBaseKm) || 0))
  } else if (sortKey.value === 'duration') {
    sorted.sort((a, b) => (Number(a.suggestedDuration) || 0) - (Number(b.suggestedDuration) || 0))
  } else if (sortKey.value === 'cost') {
    sorted.sort((a, b) => (Number(a.cost) || 0) - (Number(b.cost) || 0))
  } else {
    sorted.sort((a, b) => {
      if (a.must !== b.must) return a.must ? -1 : 1
      return b.score - a.score
    })
  }
  return sorted
})

function scoreLevel(score) {
  if (score >= 82) return 'is-high'
  if (score >= 68) return 'is-mid'
  return 'is-low'
}

/** 保底城市卡片的副标题：优先说“离省会多远”，否则退回地址。 */
function cityCaption(city) {
  const km = Number(city?.distanceKm) || 0
  if (km > 0) return `离省会 ${Math.round(km)} km`
  const address = String(city?.address || '').trim()
  const name = String(city?.name || '').trim()
  if (address && address !== name) return address
  return '可作为落脚城市'
}

function durationText(minutes) {
  const value = Number(minutes) || 0
  if (value <= 0) return '时长未知'
  if (value < 60) return `${value} 分钟`
  const hours = value / 60
  return `${Number.isInteger(hours) ? hours : hours.toFixed(1)} 小时`
}

function cardMetrics(item) {
  const metrics = item.metrics || {}
  return [
    { label: '口碑', value: metrics.rating ?? 0.5 },
    { label: '到达', value: metrics.access ?? 0.5 },
    { label: '体验', value: metrics.experience ?? 0.5 },
  ]
}

function onBaseInput(value) {
  if (baseTimer) clearTimeout(baseTimer)
  const text = String(value || '').trim()
  if (!text) {
    baseSuggestions.value = []
    return
  }
  baseTimer = setTimeout(async () => {
    const fn = props.search
    if (typeof fn !== 'function') return
    const items = await fn(text, '')
    baseSuggestions.value = (items || []).slice(0, 6)
  }, 280)
}

function chooseBase(place) {
  if (baseTimer) clearTimeout(baseTimer)
  baseKeyword.value = ''
  baseSuggestions.value = []
  emit('add-base', place)
}

function clearBaseSearch() {
  baseKeyword.value = ''
  baseSuggestions.value = []
}
</script>
