<template>
  <div class="tr-wizard">
    <section class="tr-wizard-form">
      <div class="tr-wizard-head">
        <div>
          <div class="tr-eyebrow"><span></span> {{ form.mode === 'weekend' ? 'WEEKEND SHUTTLE' : 'NEW JOURNEY' }}</div>
          <h1>{{ form.mode === 'weekend' ? '先定一个出发点，剩下的周末交给行迹。' : '把这次出发，说清楚一点。' }}</h1>
          <p>
            {{ form.mode === 'weekend'
              ? '以你的城市为圆心检索周边，一次排出未来 8 个周末。最近一个周末会自动细化到时间表。'
              : '不需要先做攻略。填完这些，路线、天气、吃饭、住宿和预算会一起排好。' }}
          </p>
        </div>
        <button type="button" class="tr-icon-btn tr-back-btn" title="返回" @click="$emit('cancel')">
          <TravelIcon name="x" :size="17" />
        </button>
      </div>

      <div v-if="error" class="tr-form-error">
        <TravelIcon name="warning" :size="15" /> {{ error }}
      </div>

      <form class="tr-form" @submit.prevent="submit">
        <section class="tr-form-section">
          <div class="tr-form-label"><span>01</span> 从哪里出发</div>
          <div class="tr-place-row">
            <div class="tr-place-field" :class="{ 'is-open': activeField === 'origin' }">
              <TravelIcon name="pin" :size="17" />
              <input
                v-model="form.origin.name"
                type="text"
                placeholder="输入出发地，或点右侧定位"
                autocomplete="off"
                @input="onPlaceInput('origin', $event.target.value)"
                @focus="openSuggestions('origin')"
                @blur="closeSuggestionsLater"
                @keydown.esc="closeAllSuggestions"
              />
              <button type="button" class="tr-locate-btn" :disabled="locating" @click.stop="locate">
                <TravelIcon name="navigation" :size="15" />
                {{ locating ? '定位中…' : '定位' }}
              </button>
              <div v-if="suggestions.origin.length" class="tr-suggest">
                <button
                  v-for="place in suggestions.origin"
                  :key="place.id || place.name"
                  type="button"
                  @mousedown.prevent="choosePlace('origin', place)"
                >
                  <TravelIcon name="pin" :size="14" />
                  <span><strong>{{ place.name }}</strong><small>{{ place.address || place.city }}</small></span>
                </button>
              </div>
            </div>
          </div>
          <p v-if="form.origin.lng" class="tr-field-note">
            已确认坐标 {{ Number(form.origin.lng).toFixed(4) }}, {{ Number(form.origin.lat).toFixed(4) }} · 出发前可再次修改
          </p>
          <div class="tr-origin-quick">
            <span>常用：</span>
            <button v-if="prefs.home" type="button" @click="useOrigin(prefs.home)">
              <TravelIcon name="pin" :size="12" /> 家 · {{ shortPlace(prefs.home) }}
            </button>
            <button v-if="prefs.lastOrigin" type="button" @click="useOrigin(prefs.lastOrigin)">
              <TravelIcon name="clock" :size="12" /> 上次 · {{ shortPlace(prefs.lastOrigin) }}
            </button>
            <button type="button" @click="locate">
              <TravelIcon name="navigation" :size="12" /> 当前定位
            </button>
            <span v-if="prefs.home || prefs.lastOrigin" class="tr-origin-pref-note">已按个人偏好预填，可随时切换</span>
          </div>
        </section>

        <section v-if="form.mode !== 'weekend'" class="tr-form-section">
          <div class="tr-form-label"><span>02</span> 想去哪里</div>
          <div class="tr-place-field" :class="{ 'is-open': activeField === 'destination' }">
            <TravelIcon name="compass" :size="17" />
            <input
              v-model="form.destination.name"
              type="text"
              placeholder="输入城市、景点或区域，例如：杭州西湖"
              autocomplete="off"
              @input="onPlaceInput('destination', $event.target.value)"
              @focus="openSuggestions('destination')"
              @blur="closeSuggestionsLater"
              @keydown.esc="closeAllSuggestions"
            />
            <div v-if="suggestions.destination.length" class="tr-suggest">
              <button
                v-for="place in suggestions.destination"
                :key="place.id || place.name"
                type="button"
                @mousedown.prevent="choosePlace('destination', place)"
              >
                <TravelIcon name="pin" :size="14" />
                <span><strong>{{ place.name }}</strong><small>{{ place.address || place.city }}</small></span>
              </button>
            </div>
          </div>
          <div class="tr-quick-places">
            <span>常去：</span>
            <button
              v-for="place in quickPlaces"
              :key="place.name"
              type="button"
              class="tr-chip is-plain"
              @click="choosePlace('destination', place)"
            >
              {{ place.name }}
            </button>
          </div>
        </section>

        <section v-else class="tr-form-section">
          <div class="tr-form-label"><span>02</span> 周边范围</div>
          <div class="tr-segmented is-four">
            <button
              v-for="option in radiusOptions"
              :key="option.value"
              type="button"
              :class="{ active: form.radiusKm === option.value }"
              @click="form.radiusKm = option.value"
            >
              <strong>{{ option.label }}</strong>
              <small>{{ option.hint }}</small>
            </button>
          </div>
          <p class="tr-field-note">会以出发地所在城市为圆心，检索这个范围内的景点、餐饮与住宿。</p>
        </section>

        <section class="tr-form-section">
          <div class="tr-form-label"><span>{{ form.mode === 'weekend' ? '03' : '03' }}</span> 什么时候</div>
          <div class="tr-grid-2">
            <label class="tr-field">
              <span>出发日期</span>
              <input v-model="form.startDate" type="date" :min="today" @change="onStartDateChange" />
            </label>
            <label class="tr-field">
              <span>返回日期</span>
              <input v-model="form.endDate" type="date" :min="form.startDate || today" />
            </label>
            <label class="tr-field">
              <span>每天出发时间</span>
              <input v-model="form.startTime" type="time" />
            </label>
            <label class="tr-field">
              <span>每天结束时间</span>
              <input v-model="form.endTime" type="time" />
            </label>
          </div>
          <p v-if="form.mode === 'weekend'" class="tr-field-note">
            默认周六当天往返（09:30-17:00）；把返回日期改到周日即可生成两天一夜。
          </p>
        </section>

        <section class="tr-form-section">
          <div class="tr-form-label"><span>04</span> 怎么去</div>
          <div class="tr-segmented">
            <button
              v-for="option in transportOptions"
              :key="option.value"
              type="button"
              :class="{ active: form.transport === option.value }"
              @click="form.transport = option.value"
            >
              <TravelIcon :name="transportIcon(option.value)" :size="17" />
              <strong>{{ option.label }}</strong>
              <small>{{ option.hint }}</small>
            </button>
          </div>
        </section>

        <section class="tr-form-section">
          <div class="tr-form-label"><span>05</span> 和谁、怎么花</div>
          <div class="tr-travelers">
            <div v-for="item in travelerFields" :key="item.key" class="tr-stepper">
              <div><strong>{{ item.label }}</strong><small>{{ item.hint }}</small></div>
              <div class="tr-stepper-controls">
                <button type="button" :disabled="form[item.key] <= item.min" @click="stepTraveler(item.key, -1, item.min)">−</button>
                <span>{{ form[item.key] }}</span>
                <button type="button" @click="stepTraveler(item.key, 1, item.min)">＋</button>
              </div>
            </div>
          </div>
          <div class="tr-budget-row">
            <div class="tr-segmented is-three">
              <button
                v-for="option in budgetLevels"
                :key="option.value"
                type="button"
                :class="{ active: form.budgetLevel === option.value }"
                @click="form.budgetLevel = option.value"
              >
                <strong>{{ option.label }}</strong>
                <small>{{ option.hint }}</small>
              </button>
            </div>
            <label class="tr-field tr-budget-total">
              <span>总预算上限（可选）</span>
              <div class="tr-money-input">
                <span>¥</span>
                <input v-model.number="form.budgetTotal" type="number" min="0" step="100" placeholder="不填则不设上限" />
              </div>
            </label>
          </div>
        </section>

        <section class="tr-form-section">
          <div class="tr-form-label"><span>06</span> 想要什么</div>
          <div class="tr-chips">
            <button
              v-for="interest in interestOptions"
              :key="interest"
              type="button"
              class="tr-chip"
              :class="{ active: form.interests.includes(interest) }"
              @click="toggleInterest(interest)"
            >
              {{ interest }}
            </button>
          </div>
          <div class="tr-pace-row">
            <span class="tr-inline-label">节奏</span>
            <div class="tr-segmented is-three is-compact">
              <button
                v-for="option in paceOptions"
                :key="option.value"
                type="button"
                :class="{ active: form.pace === option.value }"
                @click="form.pace = option.value"
              >
                <strong>{{ option.label }}</strong>
                <small>{{ option.hint }}</small>
              </button>
            </div>
          </div>
        </section>

        <details class="tr-advanced">
          <summary>补充要求 · 必去 · 避开 · 数据源</summary>
          <div class="tr-grid-2">
            <label class="tr-field is-wide">
              <span>这次最想实现的画面</span>
              <textarea v-model="form.keywords" rows="2" placeholder="例如：想看西湖日落，想吃一顿正经杭帮菜，不想早起"></textarea>
            </label>
            <label class="tr-field">
              <span>必去（逗号分隔）</span>
              <input v-model="form.mustVisit" type="text" placeholder="灵隐寺、龙井村" />
            </label>
            <label class="tr-field">
              <span>避开（逗号分隔）</span>
              <input v-model="form.avoid" type="text" placeholder="人太多的网红店、爬山" />
            </label>
            <label class="tr-field">
              <span>地图服务商</span>
              <select v-model="form.mapProvider">
                <option value="auto">自动（有高德 Key 优先高德）</option>
                <option value="amap">高德地图</option>
                <option value="baidu">百度地图</option>
                <option value="tencent">腾讯地图</option>
                <option value="google">Google Maps</option>
                <option value="mapbox">Mapbox</option>
                <option value="osm">OpenStreetMap（免费）</option>
              </select>
            </label>
          </div>
        </details>

        <div class="tr-form-actions">
          <button type="button" class="tr-btn is-ghost" @click="$emit('cancel')">先不规划</button>
          <button type="submit" class="tr-btn is-primary is-large" :disabled="busy">
            <TravelIcon name="sparkles" :size="18" />
            {{ busy ? '正在编排…' : form.mode === 'weekend' ? '生成未来 8 个周末' : '开始编排行程' }}
          </button>
        </div>
      </form>
    </section>

    <aside class="tr-wizard-aside">
      <div class="tr-live-ticket">
        <div class="tr-live-ticket-top">
          <span>XINGJI · LIVE TICKET</span>
          <span>{{ form.mode === 'weekend' ? 'WEEKEND' : 'JOURNEY' }}</span>
        </div>
        <div class="tr-live-route">
          <div>
            <strong>{{ fromCity }}</strong>
            <small>{{ fromCode }}</small>
          </div>
          <div class="tr-live-route-line">
            <span></span>
            <TravelIcon :name="transportIcon(form.transport)" :size="18" />
            <span></span>
          </div>
          <div>
            <strong>{{ toCity }}</strong>
            <small>{{ toCode }}</small>
          </div>
        </div>
        <div class="tr-live-title">
          {{ form.mode === 'weekend' ? `以 ${fromCity || '出发地'} 为圆心的周末班次` : toCity ? `去 ${toCity}` : '目的地还没定' }}
        </div>
        <div class="tr-live-grid">
          <div><small>日期</small><span>{{ dateText }}</span></div>
          <div><small>人数</small><span>{{ totalPeople }} 人</span></div>
          <div><small>方式</small><span>{{ transportLabel }}</span></div>
          <div><small>预算</small><span>{{ form.budgetTotal ? `¥${form.budgetTotal}` : budgetLabel }}</span></div>
        </div>
        <div class="tr-live-chips">
          <span v-for="interest in form.interests.slice(0, 5)" :key="interest">{{ interest }}</span>
          <span v-if="!form.interests.length">兴趣待选</span>
        </div>
        <div class="tr-live-note">
          <TravelIcon name="info" :size="14" />
          生成后会包含逐段导航、天气、餐饮住宿、预算和行前清单
        </div>
        <div class="tr-live-stamp">PLANNING</div>
        <div class="tr-live-perforation" aria-hidden="true"></div>
      </div>

      <div v-if="isMock" class="tr-aside-note">
        <strong>浏览器预览</strong>
        <p>不会调用高德或模型；生成时会使用内置的杭州示例数据，界面、预算、地图和分享都可完整走通。</p>
      </div>
      <div v-else-if="!settings.amapKeySet" class="tr-aside-note">
        <strong>还没有高德 Key</strong>
        <p>会自动使用 OpenStreetMap 免费数据。想要逐段公交、实时路况和更完整的 POI，请到设置里填一个高德 Web 服务 Key。</p>
      </div>
    </aside>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import TravelIcon from './TravelIcon.vue'
import { api } from './travelApi.js'
import {
  BUDGET_LEVELS,
  INTEREST_OPTIONS,
  PACE_OPTIONS,
  TRANSPORT_OPTIONS,
} from './travelPlan.js'
import { cityCode } from './travelShare.js'
import { addDays, nextSaturday, shortDate, todayString, weekdayCn } from './travelUtils.js'

const props = defineProps({
  draft: { type: Object, required: true },
  settings: { type: Object, default: () => ({}) },
  isMock: Boolean,
  busy: Boolean,
  search: { type: Function, default: null },
  preferences: { type: Object, default: () => ({}) },
})

const emit = defineEmits(['update', 'locate', 'search', 'submit', 'cancel'])

const form = reactive(JSON.parse(JSON.stringify(props.draft)))
const error = ref('')
const locating = ref(false)
const activeField = ref('')
const suggestions = reactive({ origin: [], destination: [] })
let blurTimer = null
let searchTimers = { origin: null, destination: null }

const transportOptions = TRANSPORT_OPTIONS
const budgetLevels = BUDGET_LEVELS
const interestOptions = INTEREST_OPTIONS
const paceOptions = PACE_OPTIONS
const travelerFields = [
  { key: 'adults', label: '成人', hint: '按 1 人计', min: 1 },
  { key: 'children', label: '儿童', hint: '门票与餐费按 0.6 计', min: 0 },
  { key: 'elders', label: '老人', hint: '行程会减少爬坡', min: 0 },
]
const radiusOptions = [
  { value: 30, label: '30 km', hint: '近郊公园 / 古镇' },
  { value: 60, label: '60 km', hint: '周边区县' },
  { value: 100, label: '100 km', hint: '邻市 / 山野' },
]
const quickPlaces = [
  { name: '杭州西湖', city: '杭州', lng: 120.1302, lat: 30.2405, address: '浙江省杭州市西湖区龙井路 1 号' },
  { name: '苏州平江路', city: '苏州', lng: 120.6298, lat: 31.3195, address: '江苏省苏州市姑苏区平江路' },
  { name: '莫干山', city: '湖州', lng: 119.8688, lat: 30.5995, address: '浙江省湖州市德清县莫干山' },
  { name: '乌镇西栅', city: '嘉兴', lng: 120.4878, lat: 30.7444, address: '浙江省嘉兴市桐乡市乌镇' },
]
const today = todayString()

const fromCity = computed(() => form.origin?.city || form.origin?.name || '出发地')
const prefs = computed(() => props.preferences || {})
const toCity = computed(() => {
  if (form.mode === 'weekend') return '周边'
  return form.destination?.city || form.destination?.name || ''
})
const fromCode = computed(() => cityCode(fromCity.value))
const toCode = computed(() => (form.mode === 'weekend' ? 'AROUND' : toCity.value ? cityCode(toCity.value) : '---'))
const totalPeople = computed(
  () => (Number(form.adults) || 0) + (Number(form.children) || 0) + (Number(form.elders) || 0),
)
const transportLabel = computed(
  () => TRANSPORT_OPTIONS.find((item) => item.value === form.transport)?.label || '自驾',
)
const budgetLabel = computed(
  () => BUDGET_LEVELS.find((item) => item.value === form.budgetLevel)?.label || '舒适',
)
const dateText = computed(() => {
  if (!form.startDate) return '待定'
  const start = shortDate(form.startDate)
  const end = form.endDate && form.endDate !== form.startDate ? shortDate(form.endDate) : ''
  return `${start}${end ? ` — ${end}` : ''}`
})

watch(
  () => props.draft,
  (draft) => {
    Object.assign(form, JSON.parse(JSON.stringify(draft)))
  },
  { deep: true },
)

watch(
  () => form.mode,
  (mode) => {
    if (mode === 'weekend') {
      const saturday = nextSaturday()
      form.startDate = saturday
      form.endDate = saturday
      form.startTime = '09:30'
      form.endTime = '17:00'
    }
  },
)

function onStartDateChange() {
  if (form.mode === 'weekend') {
    const date = new Date(`${form.startDate}T12:00:00`)
    if (Number.isNaN(date.getTime())) return
    if (date.getDay() !== 0 && date.getDay() !== 6) {
      form.startDate = nextSaturday(date)
    }
    if (!form.endDate || form.endDate < form.startDate) {
      form.endDate = form.startDate
    }
    if (form.endDate > addDays(form.startDate, 1)) {
      form.endDate = addDays(form.startDate, 1)
    }
    return
  }
  if (form.endDate && form.endDate < form.startDate) {
    form.endDate = form.startDate
  }
}

function transportIcon(value) {
  return value === 'transit' ? 'train' : value === 'walking' ? 'walk' : value === 'tour' ? 'users' : 'car'
}

function stepTraveler(key, delta, min) {
  const next = Math.max(min, (Number(form[key]) || 0) + delta)
  form[key] = next
}

function shortPlace(place) {
  const text = String(place?.city || place?.name || '')
  return text.length > 16 ? `${text.slice(0, 16)}…` : text
}

function useOrigin(place) {
  if (!place) return
  form.origin = { ...place }
  suggestions.origin = []
  activeField.value = ''
}

function toggleInterest(interest) {
  const list = form.interests || (form.interests = [])
  const index = list.indexOf(interest)
  if (index >= 0) list.splice(index, 1)
  else list.push(interest)
}

function openSuggestions(field) {
  activeField.value = field
  if (!suggestions[field].length) {
    runSearch(field, form[field]?.name || '')
  }
}

function closeSuggestionsLater() {
  blurTimer = setTimeout(() => {
    activeField.value = ''
  }, 160)
}

function closeAllSuggestions() {
  if (blurTimer) clearTimeout(blurTimer)
  activeField.value = ''
  suggestions.origin = []
  suggestions.destination = []
}

function onGlobalPointerDown(event) {
  if (!event.target?.closest?.('.tr-place-field')) {
    closeAllSuggestions()
  }
}

onMounted(() => {
  document.addEventListener('pointerdown', onGlobalPointerDown, true)
  window.addEventListener('scroll', closeAllSuggestions, true)
})

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', onGlobalPointerDown, true)
  window.removeEventListener('scroll', closeAllSuggestions, true)
  if (blurTimer) clearTimeout(blurTimer)
  if (searchTimers.origin) clearTimeout(searchTimers.origin)
  if (searchTimers.destination) clearTimeout(searchTimers.destination)
})

function onPlaceInput(field, value) {
  form[field].name = value
  form[field].lng = 0
  form[field].lat = 0
  form[field].city = ''
  activeField.value = field
  if (searchTimers[field]) clearTimeout(searchTimers[field])
  searchTimers[field] = setTimeout(() => runSearch(field, value), 280)
}

async function runSearch(field, keyword) {
  const text = String(keyword || '').trim()
  if (text.length < 1) {
    suggestions[field] = []
    return
  }
  const items = await emitSearch(text, form.origin?.city || '')
  suggestions[field] = Array.isArray(items) ? items.slice(0, 6) : []
}

function emitSearch(keyword, city) {
  // props 里的 search 是父级注入的函数，直接调用可以拿到 Promise 结果。
  const fn = props.search
  if (typeof fn === 'function') return fn(keyword, city)
  return api.searchPlaces(keyword, city)
}

function choosePlace(field, place) {
  if (blurTimer) clearTimeout(blurTimer)
  form[field] = {
    ...place,
    name: place.name || form[field]?.name || '',
  }
  suggestions[field] = []
  activeField.value = ''
}

async function locate() {
  if (locating.value) return
  locating.value = true
  try {
    const place = await api.ipLocation()
    form.origin = { ...place }
    suggestions.origin = []
    activeField.value = ''
  } catch (err) {
    error.value = `定位失败：${err?.message || err}。可以直接输入城市或详细地址。`
  } finally {
    locating.value = false
  }
}

function validate() {
  if (!form.origin?.name) return '请先确认出发地'
  if (form.mode !== 'weekend' && !form.destination?.name) return '请选择目的地'
  if (!form.startDate || !form.endDate) return '请选择出发和返回日期'
  if (form.endDate < form.startDate) return '返回日期不能早于出发日期'
  if (!form.startTime || !form.endTime) return '请选择每天的出发和结束时间'
  if (!totalPeople.value) return '至少需要一位出行人'
  if (form.mode !== 'weekend' && !form.destination?.lng && !form.destination?.lat) {
    // 允许只填名称，后端会做地理编码；这里只做提示，不阻断。
    return ''
  }
  return ''
}

function submit() {
  error.value = validate()
  if (error.value) return
  if (form.mode === 'weekend') {
    form.destination = { ...form.origin }
  }
  emit('submit', JSON.parse(JSON.stringify(form)))
}

defineExpose({ form })
</script>
