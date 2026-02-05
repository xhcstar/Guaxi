<template>
  <div class="heatmap-wrap">
    <div class="heatmap-grid" :style="{ gridTemplateColumns: `repeat(${weeks.length}, ${cell}px)` }">
      <div v-for="(wk, wi) in weeks" :key="`w-${wi}`" class="week-col">
        <div v-for="(d, di) in wk" :key="`d-${wi}-${di}`" class="day-cell" :class="`lv-${d.level}`" :title="d.title"
          :style="{ width: `${cell}px`, height: `${cell}px` }"></div>
      </div>
    </div>

    <div class="legend">
      <span class="legend-text">少</span>
      <span class="legend-cell lv-0"></span>
      <span class="legend-cell lv-1"></span>
      <span class="legend-cell lv-2"></span>
      <span class="legend-cell lv-3"></span>
      <span class="legend-cell lv-4"></span>
      <span class="legend-text">多</span>
    </div>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { computed } from "vue";

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

  // 近一年：从 end 往前推 365 天，并对齐到周日（GitHub 风格：周列）
  const start = startOfDay(new Date(end));
  start.setDate(start.getDate() - 365);
  start.setDate(start.getDate() - start.getDay()); // 回退到周日（0=周日）

  const act = props.activity || {};
  const max = Math.max(0, ...Object.values(act).map((v) => Number(v) || 0));

  const days = [];
  for (let d = new Date(start); d <= end; d.setDate(d.getDate() + 1)) {
    const dd = new Date(d);
    const key = toLocalDayKey(dd);
    const cnt = Number(act[key]) || 0;

    days.push({
      key,
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
</script>

<style scoped>
.heatmap-wrap {
  width: 100%;
  overflow: auto;
  padding-bottom: 6px;
}

.heatmap-grid {
  display: grid;
  gap: 4px;
  width: max-content;
}

.week-col {
  display: grid;
  grid-template-rows: repeat(7, auto);
  gap: 4px;
}

.day-cell {
  border-radius: 3px;
  background: rgba(0, 0, 0, 0.06);
  border: 1px solid rgba(0, 0, 0, 0.06);
}

.legend {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 8px;
  font-size: 12px;
  color: rgba(0, 0, 0, 0.65);
}

:global(:root[data-guaxi-theme="dark"]) .legend {
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

/* 深色主题下提高对比度 */
:global(:root[data-guaxi-theme="dark"]) .day-cell,
:global(:root[data-guaxi-theme="dark"]) .legend-cell {
  border-color: rgba(255, 255, 255, 0.08);
}

:global(:root[data-guaxi-theme="dark"]) .lv-0 {
  background: rgba(255, 255, 255, 0.08);
}

:global(:root[data-guaxi-theme="dark"]) .lv-1 {
  background: rgba(34, 197, 94, 0.26);
}

:global(:root[data-guaxi-theme="dark"]) .lv-2 {
  background: rgba(34, 197, 94, 0.4);
}

:global(:root[data-guaxi-theme="dark"]) .lv-3 {
  background: rgba(34, 197, 94, 0.62);
}

:global(:root[data-guaxi-theme="dark"]) .lv-4 {
  background: rgba(34, 197, 94, 0.92);
}
</style>