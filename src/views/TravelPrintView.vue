<template>
  <div class="tr-print" :data-theme="theme">
    <div class="tr-print-toolbar">
      <div>
        <strong>行迹 · 打印行程</strong>
        <span>{{ plan?.title || '正在加载…' }}</span>
      </div>
      <div class="tr-print-actions">
        <button type="button" class="tr-btn is-primary is-small" @click="print">
          <TravelIcon name="print" :size="14" /> 打印 / 保存为 PDF
        </button>
        <button type="button" class="tr-btn is-ghost is-small" @click="closeWindow">关闭</button>
      </div>
    </div>

    <div v-if="error" class="tr-print-error">{{ error }}</div>
    <article v-else-if="plan" class="tr-print-sheet">
      <header class="tr-print-head">
        <div class="tr-print-brand">
          <span class="tr-print-seal">行迹</span>
          <div><strong>行迹 · 旅行计划</strong><small>XINGJI TRAVEL JOURNAL</small></div>
        </div>
        <div class="tr-print-route">
          <strong>{{ cities.from }}</strong>
          <span>→</span>
          <strong>{{ cities.to }}</strong>
        </div>
        <h1>{{ plan.title }}</h1>
        <p v-if="plan.subtitle">{{ plan.subtitle }}</p>
        <div class="tr-print-meta">
          <span>{{ dateText }}</span>
          <span>{{ people }} 人</span>
          <span>{{ transportLabel(plan.transport) }}</span>
          <span>预算 ¥{{ money(plan.budget?.total) }} · 人均 ¥{{ money(plan.budget?.perPerson) }}</span>
          <span>{{ plan.source === 'amap' ? '高德数据' : 'OSM 数据' }}</span>
          <span v-if="plan.season?.label">{{ plan.season.label }}</span>
        </div>
      </header>

      <section v-for="(day, index) in plan.days" :key="day.date" class="tr-print-day">
        <div class="tr-print-day-head">
          <span>DAY {{ String(index + 1).padStart(2, '0') }}</span>
          <strong>{{ shortDate(day.date) }} · {{ day.weekday }} · {{ day.theme }}</strong>
          <em v-if="day.weather">{{ day.weather.dayWeather }} {{ day.weather.nightTemp ?? '—' }}~{{ day.weather.dayTemp ?? '—' }}℃</em>
        </div>
        <p v-if="day.routeWeather?.length" class="tr-print-route-weather">
          沿途天气：{{ routeWeatherText(day) }}
        </p>
        <table class="tr-print-table">
          <tbody>
            <tr v-for="block in day.blocks" :key="block.id">
              <td class="tr-print-time">{{ block.start }}<br />{{ block.end }}</td>
              <td>
                <strong>{{ block.title }}</strong>
                <span class="tr-print-kind">{{ kindLabel(block.kind) }}</span>
                <p v-if="block.detail">{{ block.detail }}</p>
                <p v-if="block.route" class="tr-print-route-line">
                  {{ block.route.summary }}<template v-if="block.route.traffic"> · {{ block.route.traffic }}</template>
                </p>
                <p v-if="block.poi?.address" class="tr-print-address">{{ block.poi.address }}<template v-if="block.poi.tel"> · {{ block.poi.tel }}</template></p>
                <p v-if="block.tips?.length" class="tr-print-tips">{{ block.tips.join('；') }}</p>
              </td>
            </tr>
          </tbody>
        </table>
      </section>

      <section class="tr-print-section">
        <h2>花费清单</h2>
        <table class="tr-print-budget">
          <tbody>
            <tr v-for="item in plan.budget?.items || []" :key="item.category">
              <td>{{ item.category }}<small>{{ item.label }}</small></td>
              <td>¥{{ money(item.amount) }}</td>
            </tr>
          </tbody>
          <tfoot>
            <tr><th>合计</th><th>¥{{ money(plan.budget?.total) }}</th></tr>
            <tr><td colspan="2">人均 ¥{{ money(plan.budget?.perPerson) }}</td></tr>
          </tfoot>
        </table>
      </section>

      <section class="tr-print-section tr-print-columns">
        <div>
          <h2>行前准备</h2>
          <div v-for="group in plan.packing || []" :key="group.category" class="tr-print-pack">
            <strong>{{ group.category }}</strong>
            <span>{{ group.items.join(' · ') }}</span>
          </div>
        </div>
        <div>
          <h2>出发前确认</h2>
          <ul class="tr-print-checklist">
            <li v-for="item in plan.checklist || []" :key="item.id">
              <span>{{ item.done ? '☑' : '☐' }}</span>{{ item.label }}<small v-if="item.due">（{{ shortDate(item.due) }} 前）</small>
            </li>
          </ul>
        </div>
      </section>

      <section class="tr-print-section">
        <h2>建议与备选</h2>
        <ul class="tr-print-advice">
          <li v-for="(item, index) in [...(plan.advice || []), ...(plan.alternatives || [])]" :key="index">{{ item }}</li>
        </ul>
      </section>

      <footer class="tr-print-foot">
        <span>由行迹生成 · {{ plan.factsMeta?.provider || 'AI 编排' }}</span>
        <span>{{ issued }}</span>
      </footer>
    </article>
  </div>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import TravelIcon from '../components/ApplicationViewComponent/apps/travel/TravelIcon.vue'
import { api } from '../components/ApplicationViewComponent/apps/travel/travelApi.js'
import { ticketCities, transportLabel, peopleCount } from '../components/ApplicationViewComponent/apps/travel/travelShare.js'
import { money, shortDate, weekdayCn } from '../components/ApplicationViewComponent/apps/travel/travelUtils.js'
import '../components/ApplicationViewComponent/apps/travel/travel.css'

const plan = ref(null)
const error = ref('')
const theme = ref('light')
const issued = new Date().toLocaleDateString('zh-CN')

const cities = computed(() => (plan.value ? ticketCities(plan.value) : { from: '', to: '' }))
const people = computed(() => peopleCount(plan.value))
const dateText = computed(() => {
  if (!plan.value) return ''
  if (!plan.value.endDate || plan.value.endDate === plan.value.startDate) {
    return `${shortDate(plan.value.startDate)} · ${weekdayCn(plan.value.startDate)}`
  }
  return `${shortDate(plan.value.startDate)} — ${shortDate(plan.value.endDate)} · ${plan.value.days?.length || 1} 天`
})

function kindLabel(kind) {
  return { transit: '交通', sight: '游览', food: '餐饮', hotel: '住宿', rest: '休息', activity: '体验' }[kind] || ''
}

function routeWeatherText(day) {
  return (day.routeWeather || [])
    .map((point) => {
      if (point.available === false) return `${point.name || '途经'} 临近更新`
      const temperature =
        point.temperature != null ? ` ${Math.round(Number(point.temperature))}℃` : ''
      return `${point.name || '途经'} ${point.time || ''} ${point.weather || ''}${temperature}`.trim()
    })
    .join(' → ')
}

function print() {
  window.print()
}

async function closeWindow() {
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    await getCurrentWindow().close()
  } catch {
    window.close()
  }
}

onMounted(async () => {
  const params = new URLSearchParams(window.location.search)
  theme.value = params.get('theme') === 'dark' ? 'dark' : 'light'
  const planId = params.get('planId') || ''
  if (!planId) {
    error.value = '缺少计划 id'
    return
  }
  try {
    plan.value = await api.getPlan(planId)
    document.title = `${plan.value.title || '行程'} · 行迹打印`
  } catch (err) {
    error.value = `读取计划失败：${err?.message || err}`
  }
})
</script>
