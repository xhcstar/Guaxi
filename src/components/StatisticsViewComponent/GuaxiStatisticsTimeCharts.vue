<template>
  <div class="time-charts">
    <n-grid :x-gap="12" :y-gap="12" :cols="12">
      <n-grid-item :span="4">
        <n-card size="small" class="panel-card" title="新增文档（时间分布）">
          <div class="sub">{{ rangeLabel }}</div>
          <svg class="chart" :viewBox="`0 0 ${w} ${h}`" preserveAspectRatio="none">
            <defs>
              <linearGradient id="createdArea" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stop-color="rgba(45, 212, 191, 0.35)" />
                <stop offset="100%" stop-color="rgba(45, 212, 191, 0)" />
              </linearGradient>
            </defs>
            <path :d="created.areaPath" fill="url(#createdArea)" />
            <path :d="created.linePath" fill="none" stroke="rgba(45, 212, 191, 0.95)" stroke-width="2" />
            <g v-for="(l, i) in labels" :key="`c-${l}`">
              <text :x="x(i)" :y="h - 6" text-anchor="middle" fill="rgba(0,0,0,0.55)" font-size="11">
                {{ l }}
              </text>
            </g>
          </svg>
        </n-card>
      </n-grid-item>

      <n-grid-item :span="4">
        <n-card size="small" class="panel-card" title="修改文档（时间分布）">
          <div class="sub">{{ rangeLabel }}</div>
          <svg class="chart" :viewBox="`0 0 ${w} ${h}`" preserveAspectRatio="none">
            <defs>
              <linearGradient id="modifiedArea" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stop-color="rgba(59, 130, 246, 0.35)" />
                <stop offset="100%" stop-color="rgba(59, 130, 246, 0)" />
              </linearGradient>
            </defs>
            <path :d="modified.areaPath" fill="url(#modifiedArea)" />
            <path :d="modified.linePath" fill="none" stroke="rgba(59, 130, 246, 0.95)" stroke-width="2" />
            <g v-for="(l, i) in labels" :key="`m-${l}`">
              <text :x="x(i)" :y="h - 6" text-anchor="middle" fill="rgba(0,0,0,0.55)" font-size="11">
                {{ l }}
              </text>
            </g>
          </svg>
        </n-card>
      </n-grid-item>

      <n-grid-item :span="4">
        <n-card size="small" class="panel-card" title="资料导入（时间分布）">
          <div class="sub">{{ rangeLabel }}</div>
          <svg class="chart" :viewBox="`0 0 ${w} ${h}`" preserveAspectRatio="none">
            <defs>
              <linearGradient id="importArea" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stop-color="rgba(249, 115, 22, 0.35)" />
                <stop offset="100%" stop-color="rgba(249, 115, 22, 0)" />
              </linearGradient>
            </defs>
            <path :d="imported.areaPath" fill="url(#importArea)" />
            <path :d="imported.linePath" fill="none" stroke="rgba(249, 115, 22, 0.95)" stroke-width="2" />
            <g v-for="(l, i) in labels" :key="`i-${l}`">
              <text :x="x(i)" :y="h - 6" text-anchor="middle" fill="rgba(0,0,0,0.55)" font-size="11">
                {{ l }}
              </text>
            </g>
          </svg>
        </n-card>
      </n-grid-item>
    </n-grid>
  </div>
</template>

<script setup>
import { computed, watch } from "vue";

/**
 * 顶部时间分布图组件：
 * - 输出 3 张图：新增、修改、导入
 * - 只做可视化与分桶统计，不负责拉取数据
 */
const props = defineProps({
  timeRange: { type: String, default: "week" },
  kbFileMetas: { type: Array, default: () => [] },
  repoImportedFiles: { type: Array, default: () => [] },
});

const w = 760;
const h = 200;

function parseDateSafe(v) {
  if (!v) return null;
  if (v instanceof Date && !Number.isNaN(v.getTime())) return v;

  const s = String(v).trim();
  if (!s) return null;

  const iso = s.includes("T") ? s : s.replace(" ", "T");
  const d1 = new Date(iso);
  if (!Number.isNaN(d1.getTime())) return d1;

  const d2 = new Date(s);
  if (!Number.isNaN(d2.getTime())) return d2;

  return null;
}

function daysAgo(n) {
  const d = new Date();
  d.setDate(d.getDate() - n);
  return d;
}

const range = computed(() => {
  if (props.timeRange === "week") return { start: daysAgo(7), end: new Date() };
  if (props.timeRange === "month") return { start: daysAgo(30), end: new Date() };
  return { start: daysAgo(365), end: new Date() };
});

const rangeLabel = computed(() => {
  const s = range.value.start.toISOString().slice(0, 10);
  const e = range.value.end.toISOString().slice(0, 10);
  return `${s} ~ ${e}`;
});

function inRange(d) {
  if (!d) return false;
  return d >= range.value.start && d <= range.value.end;
}

function makeBuckets() {
  const labels = [];
  const starts = [];
  const start = range.value.start;
  const end = range.value.end;

  if (props.timeRange === "week") {
    for (let i = 6; i >= 0; i -= 1) {
      const d = daysAgo(i);
      d.setHours(0, 0, 0, 0);
      labels.push(d.toISOString().slice(5, 10));
      starts.push(d);
    }
    return { labels, starts };
  }

  if (props.timeRange === "month") {
    const base = new Date(start);
    base.setHours(0, 0, 0, 0);
    for (let i = 0; i < 5; i += 1) {
      const d = new Date(base.getTime() + i * 7 * 24 * 3600 * 1000);
      labels.push(`W${i + 1}`);
      starts.push(d);
    }
    return { labels, starts };
  }

  // year：按月 12 桶
  const s = new Date(end);
  s.setDate(1);
  s.setHours(0, 0, 0, 0);
  for (let i = 11; i >= 0; i -= 1) {
    const d = new Date(s);
    d.setMonth(d.getMonth() - i);
    labels.push(`${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`);
    starts.push(d);
  }
  return { labels, starts };
}

function bucketIndex(d, bucket) {
  if (!d) return -1;
  const t = d.getTime();
  const arr = bucket.starts;
  for (let i = arr.length - 1; i >= 0; i -= 1) {
    if (t >= arr[i].getTime()) return i;
  }
  return -1;
}

const bucket = computed(() => makeBuckets());
const labels = computed(() => bucket.value.labels);

const padX = 20;
const padY = 16;
const innerW = w - padX * 2;
const innerH = h - padY * 2 - 18;

const x = (i) => padX + (labels.value.length === 1 ? innerW / 2 : (i / (labels.value.length - 1)) * innerW);
const y = (v, maxY) => padY + (1 - v / maxY) * innerH;

function buildPaths(arr) {
  const maxY = Math.max(...arr, 1);

  let d = "";
  for (let i = 0; i < arr.length; i += 1) {
    d += `${i === 0 ? "M" : "L"} ${x(i).toFixed(2)} ${y(arr[i], maxY).toFixed(2)} `;
  }
  const linePath = d.trim();

  const lastX = x(arr.length - 1);
  const baseY = padY + innerH;
  const areaPath = `${linePath} L ${lastX.toFixed(2)} ${baseY.toFixed(2)} L ${x(0).toFixed(2)} ${baseY.toFixed(2)} Z`;

  return { linePath, areaPath };
}

const created = computed(() => {
  const b = bucket.value;
  const arr = new Array(b.labels.length).fill(0);

  for (const it of props.kbFileMetas || []) {
    const d = it?.createdAt;
    if (!inRange(d)) continue;
    const idx = bucketIndex(d, b);
    if (idx >= 0) arr[idx] += 1;
  }

  return buildPaths(arr);
});

const modified = computed(() => {
  const b = bucket.value;
  const arr = new Array(b.labels.length).fill(0);

  for (const it of props.kbFileMetas || []) {
    const d = it?.modifiedAt;
    if (!inRange(d)) continue;
    const idx = bucketIndex(d, b);
    if (idx >= 0) arr[idx] += 1;
  }

  return buildPaths(arr);
});

const imported = computed(() => {
  const b = bucket.value;
  const arr = new Array(b.labels.length).fill(0);

  for (const f of props.repoImportedFiles || []) {
    const d = parseDateSafe(f?.import_file_create_time || f?.importFileCreateTime);
    if (!inRange(d)) continue;
    const idx = bucketIndex(d, b);
    if (idx >= 0) arr[idx] += 1;
  }

  return buildPaths(arr);
});

watch(
  () => props.timeRange,
  () => {
    console.info("[statistics:timecharts] timeRange changed", props.timeRange);
  }
);
</script>

<style scoped>
.time-charts {
  width: 100%;
}

.panel-card {
  border-radius: 12px;
  box-shadow: 0 8px 24px rgba(16, 24, 40, 0.06);
}

.chart {
  width: 100%;
  height: 190px;
}

.sub {
  margin-top: 6px;
  margin-bottom: 6px;
  font-size: 12px;
  opacity: 0.75;
}
</style>