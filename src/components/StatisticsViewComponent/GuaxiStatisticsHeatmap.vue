<template>
  <div ref="wrapEl" class="heatmap-wrap">
    <div class="legend">
      <span class="legend-text">少</span>
      <span class="legend-cell lv-0"></span>
      <span class="legend-cell lv-1"></span>
      <span class="legend-cell lv-2"></span>
      <span class="legend-cell lv-3"></span>
      <span class="legend-cell lv-4"></span>
      <span class="legend-text">多</span>
    </div>

    <div class="heatmap-body" :style="{ '--hm-cell': `${cell}px`, '--hm-gap': '4px', '--hm-month-h': `${cell}px` }">
      <div class="y-labels" aria-hidden="true">
        <div v-for="(t, i) in yLabels" :key="`y-${i}`" class="y-label">{{ t }}</div>
      </div>

      <div ref="scrollEl" class="heatmap-scroll">
        <div
          class="months-row"
          :style="{ gridTemplateColumns: `repeat(${weeks.length}, ${cell}px)`, paddingLeft: `${sidePadPx}px`, paddingRight: `${sidePadPx}px` }"
          aria-hidden="true"
        >
          <div v-for="(m, i) in monthCells" :key="`m-${i}`" class="month-cell">{{ m }}</div>
        </div>

        <div
          class="heatmap-grid"
          :style="{ gridTemplateColumns: `repeat(${weeks.length}, ${cell}px)`, paddingLeft: `${sidePadPx}px`, paddingRight: `${sidePadPx}px` }"
        >
          <div v-for="(wk, wi) in weeks" :key="`w-${wi}`" class="week-col">
            <div
              v-for="(d, di) in wk"
              :key="`d-${wi}-${di}`"
              class="day-cell"
              :class="`lv-${d.level}`"
              :title="d.title"
              :style="{ width: `${cell}px`, height: `${cell}px` }"
            ></div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";

const logger = createLogger('GuaxiStatisticsHeatmap')

/**
 * GitHub 风格热力图：
 * - 输入 activity：{ 'YYYY-MM-DD': count }
 * - 固定展示近一年（365 天），并按“周列 + 7 天行”排列
 * - 颜色等级：0~4（根据 max 动态分级）
 */
const props = defineProps({
  activity: { type: Object, default: () => ({}) },
  cell: { type: Number, default: 12 },
});

const wrapEl = ref(null);
const scrollEl = ref(null);
const sidePadPx = ref(0);

const yLabels = ["星期一", "", "星期三", "", "星期五", "", ""];

let ro;

function updateSidePad() {
  const el = scrollEl.value;
  const w = el?.clientWidth || 0;
  if (!w) return;

  const next = Math.max(0, Math.floor(w / 2));
  if (sidePadPx.value !== next) {
    sidePadPx.value = next;
    logger.info("[statistics:heatmap] update side pad", { sidePadPx: next, clientWidth: w });
  }
}

function scrollToLatestCentered(reason) {
  const el = scrollEl.value;
  if (!el) return;

  const maxScroll = Math.max(0, (el.scrollWidth || 0) - (el.clientWidth || 0));
  el.scrollLeft = maxScroll;

  logger.info("[statistics:heatmap] scroll to latest", {
    reason: String(reason || "auto"),
    scrollLeft: el.scrollLeft,
    maxScroll,
    clientWidth: el.clientWidth,
    scrollWidth: el.scrollWidth,
  });
}

onMounted(async () => {
  updateSidePad();

  if (typeof ResizeObserver !== "undefined") {
    ro = new ResizeObserver(() => {
      updateSidePad();
      nextTick(() => scrollToLatestCentered("resize"));
    });
    if (wrapEl.value) ro.observe(wrapEl.value);
  } else {
    window.addEventListener(
      "resize",
      () => {
        updateSidePad();
        nextTick(() => scrollToLatestCentered("resize"));
      },
      { passive: true }
    );
  }

  await nextTick();
  scrollToLatestCentered("mounted");
});

onBeforeUnmount(() => {
  if (ro) ro.disconnect();
});

function pad2(n) {
  return String(n).padStart(2, "0");
}

function toLocalDayKey(d) {
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

function startOfDay(d) {
  const x = new Date(d);
  x.setHours(0, 0, 0, 0);
  return x;
}

function levelForCount(c, max) {
  if (!c || c <= 0) return 0;
  if (max <= 4) return Math.min(4, Math.max(1, c)); // max 小时，尽量拉开差异

  const r = c / max; // (0,1]
  if (r <= 0.25) return 1;
  if (r <= 0.5) return 2;
  if (r <= 0.75) return 3;
  return 4;
}

const weeks = computed(() => {
  const end = startOfDay(new Date());

  // 近一年：从 end 往前推 365 天，并对齐到周一（周列从周一开始）
  const start = startOfDay(new Date(end));
  start.setDate(start.getDate() - 365);

  // 将 start 对齐到周一：getDay() 返回 0=周日..6=周六
  // 这里把“周一”视为一周起点，计算距离周一的偏移量并回退
  const offsetToMon = (start.getDay() + 6) % 7;
  start.setDate(start.getDate() - offsetToMon);

  const act = props.activity || {};
  const max = Math.max(0, ...Object.values(act).map((v) => Number(v) || 0));

  const days = [];
  for (let d = new Date(start); d <= end; d.setDate(d.getDate() + 1)) {
    const dd = new Date(d);
    const key = toLocalDayKey(dd);
    const cnt = Number(act[key]) || 0;

    days.push({
      key,
      date: dd,
      count: cnt,
      level: levelForCount(cnt, max),
      title: `${key}：${cnt}`,
    });
  }

  const out = [];
  for (let i = 0; i < days.length; i += 7) {
    out.push(days.slice(i, i + 7));
  }

  logger.info("[statistics:heatmap] build weeks", {
    start: toLocalDayKey(start),
    end: toLocalDayKey(end),
    weekCount: out.length,
    max,
  });

  return out;
});

const monthCells = computed(() => {
  const out = new Array(weeks.value.length).fill("");
  for (let wi = 0; wi < weeks.value.length; wi += 1) {
    const wk = weeks.value[wi] || [];
    const firstDay = wk.find((d) => (d?.date instanceof Date ? d.date.getDate() === 1 : false));
    if (!firstDay) continue;
    const m = firstDay.date.getMonth() + 1;
    out[wi] = `${m}月`;
  }
  logger.info("[statistics:heatmap] build month cells", { labeled: out.filter(Boolean).length, weekCount: out.length });
  return out;
});

watch(
  () => props.activity,
  async () => {
    await nextTick();
    scrollToLatestCentered("activity_changed");
  },
  { deep: true }
);
</script>

<style scoped>
.heatmap-wrap {
  width: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.heatmap-body {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.y-labels {
  display: grid;
  grid-template-rows: repeat(7, var(--hm-cell));
  gap: var(--hm-gap);
  padding-top: calc(var(--hm-month-h) + var(--hm-gap));
  font-size: 12px;
  color: rgba(0, 0, 0, 0.55);
  user-select: none;
  min-width: 52px;
  text-align: right;
  white-space: nowrap;
}

.theme-dark .y-labels {
  color: rgba(255, 255, 255, 0.72);
}

.y-label {
  height: var(--hm-cell);
  line-height: var(--hm-cell);
  padding-right: 4px;
}

.heatmap-scroll {
  width: 100%;
  overflow: auto;
  padding-bottom: 6px;
}

.months-row {
  display: grid;
  gap: var(--hm-gap);
  width: max-content;
  margin-bottom: var(--hm-gap);
  height: var(--hm-month-h);
  align-items: flex-end;
}

.month-cell {
  width: var(--hm-cell);
  height: var(--hm-month-h);
  line-height: var(--hm-month-h);
  font-size: 12px;
  color: rgba(0, 0, 0, 0.55);
  white-space: nowrap;
  overflow: visible;
  user-select: none;
}

.theme-dark .month-cell {
  color: rgba(255, 255, 255, 0.72);
}

.heatmap-grid {
  display: grid;
  gap: var(--hm-gap);
  width: max-content;
}

.week-col {
  display: grid;
  grid-template-rows: repeat(7, auto);
  gap: var(--hm-gap);
}

.day-cell {
  border-radius: 3px;
  background: rgba(0, 0, 0, 0.06);
  border: 1px solid rgba(0, 0, 0, 0.06);
  cursor: pointer;
  transition: transform 0.12s ease, box-shadow 0.12s ease, border-color 0.12s ease, filter 0.12s ease;
}

.day-cell:hover {
  border-color: rgba(59, 130, 246, 0.65);
  box-shadow: 0 6px 16px rgba(16, 24, 40, 0.14);
  transform: translateY(-1px) scale(1.06);
  filter: brightness(1.04);
}

:global(:root[data-guaxi-theme="dark"]) .day-cell:hover,
:global(.dark-mode) .day-cell:hover,
:global(.dark) .day-cell:hover {
  border-color: rgba(96, 165, 250, 0.75);
  box-shadow: 0 6px 16px rgba(0, 0, 0, 0.45);
  filter: brightness(1.08);
}

.legend {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 8px;
  font-size: 12px;
  color: rgba(0, 0, 0, 0.65);
}

.theme-dark .legend {
  color: rgba(255, 255, 255, 0.82);
}

.legend-cell {
  width: 12px;
  height: 12px;
  border-radius: 3px;
  border: 1px solid rgba(0, 0, 0, 0.06);
}

.legend-text {
  padding: 0 2px;
}

.lv-0 {
  background: rgba(0, 0, 0, 0.06);
}

.lv-1 {
  background: rgba(34, 197, 94, 0.18);
}

.lv-2 {
  background: rgba(34, 197, 94, 0.32);
}

.lv-3 {
  background: rgba(34, 197, 94, 0.55);
}

.lv-4 {
  background: rgba(34, 197, 94, 0.9);
}

/* 深色主题下提高对比度（仅作用于本组件，避免影响全局） */
.theme-dark .day-cell,
.theme-dark .legend-cell {
  border-color: rgba(148, 163, 184, 0.18);
}

.theme-dark .lv-0 {
  background: rgba(148, 163, 184, 0.2);
}

.theme-dark .day-cell.lv-0,
.theme-dark .legend-cell.lv-0 {
  border-color: rgba(148, 163, 184, 0.26);
  box-shadow: inset 0 0 0 1px rgba(148, 163, 184, 0.12);
}

.theme-dark .lv-1 {
  background: rgba(34, 197, 94, 0.26);
}

.theme-dark .lv-2 {
  background: rgba(34, 197, 94, 0.4);
}

.theme-dark .lv-3 {
  background: rgba(34, 197, 94, 0.62);
}

.theme-dark .lv-4 {
  background: rgba(34, 197, 94, 0.92);
}
</style>