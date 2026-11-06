<template>
  <div class="tm-heatmap">
    <div class="tm-hm-head">
      <span class="tm-hm-title">{{ year }} 年</span>
      <span class="tm-hm-sub">{{ activeDays }} 天有记录 · 共 {{ totalEvents }} 次</span>
      <span class="tm-hm-spacer" />
      <span class="tm-hm-legend">
        <i class="lv0" /><i class="lv1" /><i class="lv2" /><i class="lv3" /><i class="lv4" />
      </span>
    </div>

    <div class="tm-hm-body">
      <div class="tm-hm-weekdays">
        <span v-for="(w, i) in weekdayLabels" :key="i">{{ w }}</span>
      </div>
      <div class="tm-hm-scroll">
        <div class="tm-hm-months">
          <span
            v-for="m in monthLabels"
            :key="m.index"
            class="tm-hm-month"
            :style="{ gridColumn: m.col + 1 }"
          >{{ m.label }}</span>
        </div>
        <div class="tm-hm-grid">
          <div v-for="(week, wi) in weeks" :key="wi" class="tm-hm-week">
            <button
              v-for="(day, di) in week"
              :key="di"
              class="tm-hm-cell"
              :class="[day.inYear ? 'lv' + day.level : 'blank', { selected: day.iso && day.iso === selected }]"
              :title="day.iso ? `${day.iso} · 完成 ${day.done} · 打卡 ${day.checkins}` : ''"
              :disabled="!day.inYear"
              @click="pick(day)"
            />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, ref, watch } from 'vue'

const props = defineProps({
  items: { type: Array, default: () => [] },
  year: { type: Number, default: () => new Date().getFullYear() },
})
const emit = defineEmits(['select'])
const selected = ref('')

const pad = (n) => String(n).padStart(2, '0')
const isoOf = (y, m, d) => `${y}-${pad(m)}-${pad(d)}`
const cnToIso = (s) => {
  const m = /(\d{4})\s*[-/年]\s*(\d{1,2})\s*[-/月]\s*(\d{1,2})/.exec(String(s || ''))
  return m ? `${m[1]}-${pad(m[2])}-${pad(m[3])}` : ''
}

const activity = computed(() => {
  const map = new Map()
  const bump = (iso, key) => {
    if (!iso) return
    const cur = map.get(iso) || { done: 0, checkins: 0 }
    cur[key] += 1
    map.set(iso, cur)
  }
  for (const it of props.items) {
    if (it.done) bump(cnToIso(it.completedTime), 'done')
    for (const c of (it.checkins || [])) bump(cnToIso(c), 'checkins')
  }
  return map
})

const levelOf = (count) => {
  if (!count) return 0
  if (count <= 1) return 1
  if (count <= 3) return 2
  if (count <= 5) return 3
  return 4
}

const weeks = computed(() => {
  const y = props.year
  const first = new Date(y, 0, 1)
  // 周一为一周起点，与 FullCalendar 的 firstDay: 1 一致。
  const offset = (first.getDay() + 6) % 7
  const start = new Date(y, 0, 1 - offset)
  const out = []
  for (let w = 0; w < 53; w += 1) {
    const week = []
    for (let d = 0; d < 7; d += 1) {
      const dt = new Date(start)
      dt.setDate(start.getDate() + w * 7 + d)
      const inYear = dt.getFullYear() === y
      const iso = inYear ? isoOf(dt.getFullYear(), dt.getMonth() + 1, dt.getDate()) : ''
      const a = iso ? activity.value.get(iso) : null
      const count = a ? a.done + a.checkins : 0
      week.push({
        iso,
        inYear,
        done: a?.done || 0,
        checkins: a?.checkins || 0,
        level: levelOf(count),
      })
    }
    out.push(week)
  }
  return out
})

const activeDays = computed(() => {
  let n = 0
  for (const [iso, a] of activity.value) {
    if (Number(iso.slice(0, 4)) === props.year && (a.done || a.checkins)) n += 1
  }
  return n
})
const totalEvents = computed(() => {
  let n = 0
  for (const [iso, a] of activity.value) {
    if (Number(iso.slice(0, 4)) === props.year) n += a.done + a.checkins
  }
  return n
})

const weekdayLabels = ['一', '', '三', '', '五', '', '日']

const monthLabels = computed(() => {
  const out = []
  const seen = new Set()
  weeks.value.forEach((week, wi) => {
    const firstInYear = week.find((d) => d.inYear)
    if (!firstInYear) return
    const month = Number(firstInYear.iso.slice(5, 7))
    if (seen.has(month)) return
    seen.add(month)
    out.push({ index: month, label: `${month}月`, col: wi })
  })
  return out
})

watch(() => props.year, () => { selected.value = '' })
const pick = (day) => {
  if (!day.inYear || !day.iso) return
  selected.value = day.iso
  emit('select', day.iso)
}

defineExpose({ clear: () => { selected.value = '' } })
</script>

<style scoped>
.tm-heatmap { display: flex; flex-direction: column; gap: 10px; }
.tm-hm-head { display: flex; align-items: baseline; gap: 10px; }
.tm-hm-title { font-size: 16px; font-weight: 700; letter-spacing: 0.02em; }
.tm-hm-sub { font-size: 12px; color: var(--guaxi-text-secondary); }
.tm-hm-spacer { flex: 1; }
.tm-hm-legend { display: inline-flex; gap: 3px; }
.tm-hm-legend i { width: 10px; height: 10px; border-radius: 2px; display: block; }
.tm-hm-legend .lv0, .tm-hm-cell.lv0 { background: color-mix(in srgb, var(--guaxi-border) 55%, transparent); }
.tm-hm-legend .lv1, .tm-hm-cell.lv1 { background: color-mix(in srgb, #3f6ea8 26%, transparent); }
.tm-hm-legend .lv2, .tm-hm-cell.lv2 { background: color-mix(in srgb, #3f6ea8 46%, transparent); }
.tm-hm-legend .lv3, .tm-hm-cell.lv3 { background: color-mix(in srgb, #3f6ea8 68%, transparent); }
.tm-hm-legend .lv4, .tm-hm-cell.lv4 { background: #3f6ea8; }
.tm-hm-body { display: flex; gap: 6px; }
.tm-hm-weekdays { display: grid; grid-template-rows: repeat(7, 12px); gap: 2px; font-size: 10px; color: var(--guaxi-text-secondary); padding-top: 18px; }
.tm-hm-weekdays span { height: 12px; line-height: 12px; text-align: right; width: 12px; }
.tm-hm-scroll { overflow-x: auto; padding-bottom: 4px; }
.tm-hm-months { display: grid; grid-auto-flow: column; grid-auto-columns: 12px; gap: 2px; height: 16px; font-size: 10px; color: var(--guaxi-text-secondary); }
.tm-hm-month { white-space: nowrap; transform: translateX(-1px); }
.tm-hm-grid { display: flex; gap: 2px; }
.tm-hm-week { display: grid; grid-template-rows: repeat(7, 12px); gap: 2px; }
.tm-hm-cell { width: 12px; height: 12px; padding: 0; border: none; border-radius: 2px; cursor: pointer; }
.tm-hm-cell.blank { background: transparent; cursor: default; }
.tm-hm-cell:hover:not(.blank) { outline: 1px solid color-mix(in srgb, #3f6ea8 80%, transparent); outline-offset: 1px; }
.tm-hm-cell.selected { box-shadow: 0 0 0 1.5px #d03050; }
</style>
