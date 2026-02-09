<template>
  <div class="app-layout">
    <div class="main-header">
      <div class="header-inner">
        <div class="header-title">
          <span class="title-text">统计看板</span>
          <n-tag size="small" round type="info">Beta</n-tag>
        </div>

        <div class="header-actions">
          <n-select size="small" v-model:value="timeRange" :options="timeRangeOptions" style="width: 140px" />
          <n-button size="small" :loading="loading" @click="reloadAll">刷新</n-button>
        </div>
      </div>
    </div>

    <div class="main-content">
      <n-scrollbar style="height: 100%">
        <div class="page">
          <n-alert v-if="loadError" type="error" :show-icon="true" style="margin-bottom: 12px">
            {{ loadError }}
          </n-alert>

          <n-grid :x-gap="12" :y-gap="12" :cols="12">
            <n-grid-item :span="12">
              <n-card ref="trendRootEl" size="small" title="知识量趋势（按知识库）" class="panel-card">
                <div class="trend-head">
                  <div class="trend-sub">{{ rangeLabel }}</div>
                  <div class="trend-actions">
                    <n-select
                      size="small"
                      multiple
                      clearable
                      placeholder="全部知识库"
                      v-model:value="selectedKbNames"
                      :options="kbNameOptions"
                      style="min-width: 240px"
                    />
                    <n-select size="small" v-model:value="volumeMetric" :options="volumeMetricOptions" style="width: 140px" />
                    <n-select size="small" v-model:value="trendUnit" :options="trendUnitOptions" style="width: 110px" />
                  </div>
                </div>

                <div v-if="volumeSeries.length === 0" class="empty">暂无趋势数据（会在后台按时间间隔自动记录一次）</div>

                <div v-else class="trend-body">
                  <div class="kb-legend">
                    <span
                      v-for="s in volumeSeries"
                      :key="s.name"
                      class="kb-legend-item"
                      :class="{ active: activeKbName === s.name, dim: !!activeKbName && activeKbName !== s.name }"
                      :title="s.name"
                      @click="toggleActiveKb(s.name)"
                    >
                      <span class="kb-dot" :style="{ background: s.color }"></span>
                      <span class="kb-name">{{ s.name }}</span>
                      <span class="kb-val">{{ s.lastValue }}</span>
                    </span>
                  </div>

                  <svg class="trend-chart" :viewBox="`0 0 ${trendW} ${trendH}`" preserveAspectRatio="xMidYMid meet">
                    <g v-for="s in volumeSeries" :key="`line-${s.name}`">
                      <path
                        :d="s.linePath"
                        fill="none"
                        :stroke="s.color"
                        :stroke-width="isKbSeriesActive(s.name) ? 2.5 : 1.5"
                        :opacity="isKbSeriesActive(s.name) ? 1 : 0.15"
                      />
                    </g>

                    <g v-for="t in trendTicks" :key="`v-${t.i}-${t.label}`">
                      <text
                        :x="trendX(t.i, trendPointCount)"
                        :y="trendH - 6"
                        text-anchor="middle"
                        class="axis-text"
                        :style="`fill: ${axisLabelFill} !important`"
                        font-size="11"
                      >
                        {{ t.label }}
                      </text>
                    </g>
                  </svg>
                </div>

                <div class="stat-sub" style="margin-top: 10px">
                  说明：统计 KnowledgeBases 下每个顶级目录的文件数/目录数（自动排除 .meta 与隐藏项），并按时间间隔写入本地趋势。
                </div>
              </n-card>
            </n-grid-item>

            <n-grid-item :span="12">
              <n-card size="small" title="活跃热力图（近一年）" class="panel-card">
                <div class="heatmap-head">
                  <div class="heatmap-sub">说明：统计 KnowledgeBases 所在 Git 仓库的每日 commit 数（近一年）。</div>
                  <div class="heatmap-meta">
                    <span class="muted">总活跃：{{ heatmapTotal }}</span>
                    <span class="muted">·</span>
                    <span class="muted">有活跃天数：{{ heatmapActiveDays }}</span>
                  </div>
                </div>

                <GuaxiStatisticsHeatmap :activity="heatmapActivity" />

                <div class="stat-sub" style="margin-top: 10px">
                  提示：鼠标悬停格子可查看当天活跃次数与日期。
                </div>
              </n-card>
            </n-grid-item>

            <n-grid-item :span="12">
              <n-card size="small" title="阅读进度与耗时（阅读器本地记录）" class="panel-card">
                <div class="read-head">
                  <div class="read-meta">
                    <span class="muted">文档：{{ readerDocsTotal }}</span>
                    <span class="muted">·</span>
                    <span class="muted">已完成：{{ readerDocsCompleted }}</span>
                    <span class="muted">·</span>
                    <span class="muted">总耗时：{{ readerTotalTimeText }}</span>
                  </div>
                  <n-button size="small" secondary @click="scanReaderState">重新扫描</n-button>
                </div>

                <div v-if="readerDocRows.length === 0" class="empty">暂无阅读器记录（localStorage：guaxi_reader_state_v1:*）</div>

                <div v-else class="read-list">
                  <div
                    v-for="r in readerDocRows"
                    :key="r.key"
                    class="read-row"
                    :class="{ active: activeReaderKey === r.key, dim: !!activeReaderKey && activeReaderKey !== r.key }"
                    @click="toggleActiveReader(r.key)"
                  >
                    <div class="read-name" :title="r.titleFull">{{ r.title }}</div>
                    <div class="read-progress">
                      <div class="read-progress-bar" :style="{ width: `${r.progress}%` }"></div>
                      <div class="read-progress-text">{{ r.progress }}%</div>
                    </div>
                    <div class="read-time" :title="r.timeFull">{{ r.timeText }}</div>
                  </div>
                </div>

                <div class="stat-sub" style="margin-top: 10px">
                  说明：进度来自阅读器本地状态（progressPercent / completedAt）；耗时来自 totalReadingMs。
                </div>
              </n-card>
            </n-grid-item>

            <n-grid-item :span="12">
              <n-card size="small" title="资料库文档分类（按标签）" class="panel-card">
                <div class="trend-head">
                  <div class="trend-sub">按导入文件 tags 统计（可切换知识库）</div>
                  <div class="trend-actions">
                    <n-select size="small" v-model:value="repoFilter" :options="repoOptions" style="width: 240px" />
                  </div>
                </div>

                <div v-if="repoTagBars.length === 0" class="empty">暂无资料库数据</div>

                <div v-else class="tag-chart-wrap">
                  <svg
                    class="chart"
                    :viewBox="`0 0 ${barChartWidth} ${repoTagChartHeight}`"
                    preserveAspectRatio="xMinYMin meet"
                  >
                    <g
                      v-for="(b, idx) in repoTagBars"
                      :key="b.label"
                      style="cursor: pointer"
                      @click="toggleRepoTag(b.label)"
                    >
                      <title>{{ b.label }}：{{ b.value }}</title>
                      <rect
                        :x="0"
                        :y="idx * (barRowHeight + barRowGap)"
                        :width="b.w"
                        :height="barRowHeight"
                        rx="8"
                        :fill="b.color"
                        :opacity="selectedRepoTag && selectedRepoTag !== b.label ? 0.25 : 1"
                        :stroke="selectedRepoTag === b.label ? axisLabelFill : 'transparent'"
                        :stroke-width="selectedRepoTag === b.label ? 2 : 0"
                      />
                      <text
                        x="10"
                        :y="idx * (barRowHeight + barRowGap) + barRowHeight / 2"
                        :fill="b.textFill"
                        font-size="12"
                        font-weight="700"
                        dominant-baseline="middle"
                        :opacity="selectedRepoTag && selectedRepoTag !== b.label ? 0.35 : 1"
                      >
                        {{ b.label }} · {{ b.value }}
                      </text>
                    </g>
                  </svg>
                </div>

                <div class="stat-sub" style="margin-top: 10px">
                  说明：每个文件可含多个标签；未打标签的文件计入“未标注”。
                </div>
              </n-card>
            </n-grid-item>
          </n-grid>
        </div>
      </n-scrollbar>
    </div>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { createDiscreteApi } from "naive-ui";
import GuaxiStatisticsHeatmap from "./GuaxiStatisticsHeatmap.vue";

const logger = createLogger('GuaxiStatisticsDashboard')

/**
 * 统计看板主实现：
 * - 顶部三个“卡片”改成按时间分布的图表（新增、修改、导入）
 * - 移除“资料库文件类型分布 / 最近新增修改 / 文档新增修改趋势”
 * - 增加 GitHub 风格热力图（近一年）
 * - 保留“知识库分类（按标签）”
 * - 阅读完成情况改为扫描阅读器本地状态（更贴近真实阅读完成）
 */
const { message } = createDiscreteApi(["message"]);

const timeRangeOptions = [
  { label: "最近 7 天", value: "week" },
  { label: "最近 30 天", value: "month" },
  { label: "最近 1 年", value: "year" },
  { label: "最近 5 年", value: "5y" },
  { label: "最近 10 年", value: "10y" },
  { label: "最近 20 年", value: "20y" },
];

const timeRange = ref("week");
const loading = ref(false);
const loadError = ref("");

const KB_VOLUME_SNAPSHOTS_KEY = "guaxi_kb_volume_snapshots_v1";

const kbSummaries = ref([]);
const kbVolumeSnapshots = ref([]);

const selectedKbNames = ref([]);
const volumeMetric = ref("file");
const volumeMetricOptions = [
  { label: "知识点（文件数）", value: "file" },
  { label: "分类（目录数）", value: "dir" },
];

const trendUnit = ref("auto");
const trendUnitOptions = [
  { label: "自动", value: "auto" },
  { label: "天", value: "day" },
  { label: "月", value: "month" },
  { label: "年", value: "year" },
];

const effectiveTrendUnit = computed(() => {
  const v = String(trendUnit.value || "auto");
  if (v !== "auto") return v;

  if (timeRange.value === "week" || timeRange.value === "month") return "day";
  if (timeRange.value === "year") return "month";
  if (/^\d+y$/.test(String(timeRange.value || ""))) return "year";
  return "day";
});

const repoImportedFiles = ref([]);
const repoNames = ref([]);
const repoFilter = ref("__all__");

const gitCommits = ref([]);

const readerDocs = ref([]);

const activeKbName = ref("");
const selectedRepoTag = ref("");
const activeReaderKey = ref("");

function toggleActiveKb(name) {
  const nm = String(name || "");
  if (!nm) return;
  activeKbName.value = activeKbName.value === nm ? "" : nm;
}

function isKbSeriesActive(name) {
  const nm = String(name || "");
  return !activeKbName.value || activeKbName.value === nm;
}

function toggleRepoTag(tag) {
  const t = String(tag || "");
  if (!t) return;
  selectedRepoTag.value = selectedRepoTag.value === t ? "" : t;
}

function toggleActiveReader(key) {
  const k = String(key || "");
  if (!k) return;
  activeReaderKey.value = activeReaderKey.value === k ? "" : k;
}

const trendRootEl = ref(null);
const trendW = ref(760);
const trendH = 220;

const isDark = ref(false);
const axisLabelFill = computed(() => (isDark.value ? "rgba(255, 255, 255, 0.92)" : "rgba(0, 0, 0, 0.65)"));

function syncTheme() {
  const el = document?.documentElement;
  const byDataset = String(el?.dataset?.guaxiTheme || "") === "dark";
  const byClass = !!el?.classList?.contains?.("dark-mode") || !!el?.classList?.contains?.("dark");
  isDark.value = byDataset || byClass;
}

let ro;
let mo;
function updateTrendWidth() {
  const el = trendRootEl.value?.$el || trendRootEl.value;
  const cw = el?.clientWidth || 0;
  if (!cw) return;
  trendW.value = Math.max(360, Math.floor(cw));
}

const kbNameOptions = computed(() => {
  const list = Array.isArray(kbSummaries.value) ? kbSummaries.value : [];
  return list
    .map((x) => String(x?.knowledge_base_name || "").trim())
    .filter(Boolean)
    .sort((a, b) => a.localeCompare(b, "zh"))
    .map((name) => ({ label: name, value: name }));
});

const repoOptions = computed(() => {
  const base = [{ label: "全部知识库", value: "__all__" }];
  const list = Array.isArray(repoNames.value) ? repoNames.value : [];
  return base.concat(list.map((r) => ({ label: String(r), value: String(r) })));
});

const importedFilesById = computed(() => {
  const map = new Map();
  const list = Array.isArray(repoImportedFiles.value) ? repoImportedFiles.value : [];
  for (const f of list) {
    const id = f?.id ?? f?.import_file_id ?? f?.importFileId;
    if (id == null) continue;
    const key = String(id);
    if (!key) continue;
    map.set(key, f);
  }
  return map;
});

const warnedUnresolvedReaderIds = new Set();

function formatDuration(ms) {
  const v = Number(ms);
  if (!Number.isFinite(v) || v <= 0) return "0s";
  const totalSeconds = Math.floor(v / 1000);
  const h = Math.floor(totalSeconds / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  const s = totalSeconds % 60;
  if (h > 0) return `${h}h ${m}m ${s}s`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

function parseReaderKey(k) {
  const raw = String(k || "");
  if (!raw.startsWith("guaxi_reader_state_v1:")) return null;
  const parts = raw.split(":");
  if (parts.length < 5) return null;

  const library = parts[1] || "";
  const fileType = parts[2] || "";
  const safeId = parts.slice(3).join(":");

  let kind = "other";
  let id = "";
  let display = safeId;

  if (safeId.startsWith("path:")) {
    kind = "path";
    const p = safeId.slice("path:".length).replace(/\\/g, "/");
    const name = p.split("/").filter(Boolean).pop() || p;
    display = name;
  } else if (safeId.startsWith("id:")) {
    kind = "id";
    id = safeId.slice("id:".length);
    display = "导入文件";
  } else {
    const p = safeId.replace(/\\/g, "/");
    const name = p.split("/").filter(Boolean).pop();
    if (name) display = name;
  }

  return { library, fileType, safeId, kind, id, display };
}

function scanReaderState() {
  try {
    const out = [];
    const imported = importedFilesById.value;

    for (let i = 0; i < localStorage.length; i += 1) {
      const k = localStorage.key(i);
      if (!k) continue;
      const parsedKey = parseReaderKey(k);
      if (!parsedKey) continue;

      try {
        const raw = localStorage.getItem(k);
        if (!raw) continue;
        const parsed = JSON.parse(raw);

        const totalReadingMs = Math.max(0, Number(parsed?.totalReadingMs) || 0);
        const completedAt = parsed?.completedAt || null;
        const progressPercent = parsed?.progressPercent;

        const progress = Number.isFinite(Number(progressPercent))
          ? Math.max(0, Math.min(100, Math.round(Number(progressPercent))))
          : completedAt
            ? 100
            : 0;

        let title = parsedKey.display;
        if (parsedKey.kind === "id" && parsedKey.id) {
          const f = imported.get(String(parsedKey.id));
          const nm = String(f?.import_file_name || f?.importFileName || "").trim();
          if (nm) title = nm;
          else if (!warnedUnresolvedReaderIds.has(parsedKey.id)) {
            warnedUnresolvedReaderIds.add(parsedKey.id);
            logger.warn("[statistics] resolve import file name failed", {
              importFileId: parsedKey.id,
              library: parsedKey.library,
              fileType: parsedKey.fileType,
            });
          }
        }

        const titleFull = `${title}（${parsedKey.library}/${parsedKey.fileType}）`;

        out.push({
          key: k,
          title,
          titleFull,
          progress,
          totalReadingMs,
          completedAt,
        });
      } catch (e) {
        logger.warn("[statistics] parse reader state failed", { key: k, err: e });
      }
    }

    out.sort((a, b) => (b.totalReadingMs || 0) - (a.totalReadingMs || 0));
    readerDocs.value = out;

    logger.info("[statistics] scanReaderState done", {
      total: out.length,
      completed: out.filter((x) => !!x.completedAt).length,
    });
  } catch (e) {
    logger.warn("[statistics] scanReaderState failed", e);
    readerDocs.value = [];
  }
}

const readerDocsTotal = computed(() => (Array.isArray(readerDocs.value) ? readerDocs.value.length : 0));
const readerDocsCompleted = computed(() => (Array.isArray(readerDocs.value) ? readerDocs.value.filter((x) => !!x?.completedAt).length : 0));
const readerTotalTimeText = computed(() => {
  const list = Array.isArray(readerDocs.value) ? readerDocs.value : [];
  const sum = list.reduce((a, b) => a + (Number(b?.totalReadingMs) || 0), 0);
  return formatDuration(sum);
});

const readerDocRows = computed(() => {
  const list = Array.isArray(readerDocs.value) ? readerDocs.value : [];
  return list.slice(0, 200).map((x) => ({
    key: x.key,
    title: x.title,
    titleFull: x.titleFull || x.title,
    progress: Number(x.progress) || 0,
    timeFull: `${Number(x.totalReadingMs) || 0}ms`,
    timeText: formatDuration(x.totalReadingMs),
  }));
});

function parseDateSafe(v) {
  if (v == null) return null;
  if (v instanceof Date && !Number.isNaN(v.getTime())) return v;

  // 兼容 git log 返回的 epoch seconds（字符串）
  if (typeof v === "number" && Number.isFinite(v)) {
    const ms = v < 1e12 ? v * 1000 : v;
    const d = new Date(ms);
    return Number.isNaN(d.getTime()) ? null : d;
  }

  const s = String(v).trim();
  if (!s) return null;

  if (/^\d{9,13}$/.test(s)) {
    const n = Number(s);
    const ms = n < 1e12 ? n * 1000 : n;
    const d = new Date(ms);
    return Number.isNaN(d.getTime()) ? null : d;
  }

  const iso = s.includes("T") ? s : s.replace(" ", "T");
  const d1 = new Date(iso);
  if (!Number.isNaN(d1.getTime())) return d1;

  const d2 = new Date(s);
  if (!Number.isNaN(d2.getTime())) return d2;

  return null;
}

function pad2(n) {
  return String(n).padStart(2, "0");
}

/**
 * 注意：热力图/日维度统计用“本地日期”作为 key，避免 UTC 导致跨天错位。
 */
function toLocalDayKey(d) {
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

function daysAgo(n) {
  const d = new Date();
  d.setDate(d.getDate() - n);
  return d;
}

function yearsAgo(n) {
  const d = new Date();
  d.setFullYear(d.getFullYear() - n);
  return d;
}

const range = computed(() => {
  if (timeRange.value === "week") return { start: daysAgo(7), end: new Date() };
  if (timeRange.value === "month") return { start: daysAgo(30), end: new Date() };
  if (timeRange.value === "year") return { start: yearsAgo(1), end: new Date() };
  if (timeRange.value === "5y") return { start: yearsAgo(5), end: new Date() };
  if (timeRange.value === "10y") return { start: yearsAgo(10), end: new Date() };
  if (timeRange.value === "20y") return { start: yearsAgo(20), end: new Date() };
  return { start: yearsAgo(1), end: new Date() };
});

const rangeLabel = computed(() => {
  const { start, end } = range.value;
  return `${toLocalDayKey(start)} ~ ${toLocalDayKey(end)}`;
});

function toSnapshotTsMs(ts) {
  const d = parseDateSafe(ts);
  return d ? d.getTime() : NaN;
}

const snapshotsInRange = computed(() => {
  const list = Array.isArray(kbVolumeSnapshots.value) ? kbVolumeSnapshots.value : [];
  const { start, end } = range.value;
  const s = start.getTime();
  const e = end.getTime();

  return list
    .map((x) => ({
      ...x,
      tsMs: toSnapshotTsMs(x?.ts),
    }))
    .filter((x) => Number.isFinite(x?.tsMs) && x.tsMs >= s && x.tsMs <= e)
    .sort((a, b) => (Number(a?.tsMs) || 0) - (Number(b?.tsMs) || 0));
});

function pickColor(name) {
  const palette = ["#22c55e", "#3b82f6", "#a855f7", "#f97316", "#06b6d4", "#f43f5e", "#84cc16", "#eab308"]; 
  let h = 0;
  for (let i = 0; i < name.length; i += 1) h = (h * 31 + name.charCodeAt(i)) >>> 0;
  return palette[h % palette.length];
}

function toLocalMonthKey(d) {
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}`;
}

function toLocalYearKey(d) {
  return `${d.getFullYear()}`;
}

const trendPlot = computed(() => {
  const plotTop = 12;
  const plotBottom = trendH - 26;
  const plotLeft = 18;
  const plotRight = trendW.value - 18;
  return {
    plotTop,
    plotBottom,
    plotLeft,
    plotRight,
    plotW: Math.max(1, plotRight - plotLeft),
    plotH: Math.max(1, plotBottom - plotTop),
  };
});

const trendPoints = computed(() => {
  const snaps = snapshotsInRange.value;
  if (snaps.length === 0) return [];

  const unit = effectiveTrendUnit.value;
  const buckets = new Map();

  for (const s of snaps) {
    const t = Number(s?.tsMs);
    if (!Number.isFinite(t) || t <= 0) continue;
    const d = new Date(t);

    let k = "";
    if (unit === "year") k = toLocalYearKey(d);
    else if (unit === "month") k = toLocalMonthKey(d);
    else k = toLocalDayKey(d);

    const prev = buckets.get(k);
    if (!prev || Number(prev?.tsMs) < t) buckets.set(k, { ...s, bucketKey: k, tsMs: t });
  }

  return Array.from(buckets.values()).sort((a, b) => (Number(a?.tsMs) || 0) - (Number(b?.tsMs) || 0));
});

const trendPointCount = computed(() => trendPoints.value.length);

function formatTickLabel(tsMs, unit) {
  const t = Number(tsMs);
  if (!Number.isFinite(t) || t <= 0) return "";
  const d = new Date(t);
  if (unit === "year") return `${d.getFullYear()}`;
  if (unit === "month") return `${String(d.getFullYear()).slice(2)}-${pad2(d.getMonth() + 1)}`;
  return `${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

const trendTicks = computed(() => {
  const pts = trendPoints.value;
  const n = pts.length;
  if (n === 0) return [];

  const unit = effectiveTrendUnit.value;
  const maxTicks = Math.min(8, n);
  const idxs =
    maxTicks === n
      ? Array.from({ length: n }, (_, i) => i)
      : Array.from({ length: maxTicks }, (_, k) => Math.round((k / (maxTicks - 1)) * (n - 1)));

  const uniq = [];
  for (const i of idxs) {
    if (uniq.length === 0 || uniq[uniq.length - 1] !== i) uniq.push(i);
  }

  return uniq.map((i) => ({
    i,
    label: formatTickLabel(pts[i]?.tsMs, unit),
  }));
});

function trendX(pointIndex, totalPoints) {
  const p = trendPlot.value;
  const n = Math.max(1, Number(totalPoints) || 0);
  if (n <= 1) return p.plotLeft;
  return p.plotLeft + (Number(pointIndex) / (n - 1)) * p.plotW;
}

const volumeSeries = computed(() => {
  const snaps = trendPoints.value;
  if (snaps.length === 0) return [];

  const selected = Array.isArray(selectedKbNames.value) ? selectedKbNames.value.map((x) => String(x)).filter(Boolean) : [];
  const lastItems = snaps[snaps.length - 1]?.items || {};
  const allNames = Object.keys(lastItems);
  const names =
    selected.length > 0
      ? selected
      : allNames.slice().sort((a, b) => String(a).localeCompare(String(b), "zh"));

  const metric = volumeMetric.value === "dir" ? "dir" : "file";
  const p = trendPlot.value;

  let maxV = 1;
  for (const nm of names) {
    for (const s of snaps) {
      const v = Number(s?.items?.[nm]?.[metric]) || 0;
      if (v > maxV) maxV = v;
    }
  }

  const n = snaps.length;
  const series = [];
  for (const nm of names) {
    const color = pickColor(nm);
    const pts = [];
    for (let i = 0; i < n; i += 1) {
      const v = Number(snaps[i]?.items?.[nm]?.[metric]) || 0;
      const x = p.plotLeft + (n <= 1 ? 0 : (i / (n - 1)) * p.plotW);
      const y = p.plotBottom - (v / maxV) * p.plotH;
      pts.push([x, y]);
    }

    const linePath = pts.map((p2, idx) => `${idx === 0 ? "M" : "L"}${p2[0].toFixed(2)} ${p2[1].toFixed(2)}`).join(" ");
    const lastValue = Number(snaps[n - 1]?.items?.[nm]?.[metric]) || 0;
    series.push({ name: nm, color, lastValue, linePath });
  }

  return series;
});

const barChartWidth = 520;
const barRowHeight = 20;
const barRowGap = 10;

function topN(map, n = 8) {
  return Array.from(map.entries())
    .sort((a, b) => b[1] - a[1])
    .slice(0, n)
    .map(([label, value]) => ({ label, value }));
}

function colorForTag(label) {
  const palette = ["#3b82f6", "#22c55e", "#a855f7", "#f97316", "#06b6d4", "#f43f5e", "#84cc16", "#eab308", "#14b8a6", "#6366f1", "#ec4899", "#f59e0b"]; 
  const s = String(label || "");
  let h = 0;
  for (let i = 0; i < s.length; i += 1) h = (h * 31 + s.charCodeAt(i)) >>> 0;
  return palette[h % palette.length];
}

function textColorForBgHex(hex) {
  const s = String(hex || "").trim();
  const m = /^#?([0-9a-fA-F]{6})$/.exec(s);
  if (!m) return "rgba(255,255,255,0.92)";
  const v = m[1];
  const r = parseInt(v.slice(0, 2), 16);
  const g = parseInt(v.slice(2, 4), 16);
  const b = parseInt(v.slice(4, 6), 16);
  const yiq = (r * 299 + g * 587 + b * 114) / 1000;
  return yiq >= 150 ? "rgba(0,0,0,0.78)" : "rgba(255,255,255,0.92)";
}

function toBarRows(items) {
  const max = Math.max(...items.map((x) => x.value), 1);
  return items.map((x) => {
    const color = colorForTag(x.label);
    return {
      ...x,
      w: Math.round((x.value / max) * barChartWidth),
      color,
      textFill: textColorForBgHex(color),
    };
  });
}

const repoTagMap = computed(() => {
  const out = new Map();
  const list = Array.isArray(repoImportedFiles.value) ? repoImportedFiles.value : [];
  const filtered = repoFilter.value === "__all__" ? list : list.filter((x) => String(x?.knowledge_repo) === String(repoFilter.value));

  for (const f of filtered) {
    const tags = Array.isArray(f?.tags) ? f.tags.map((x) => String(x).trim()).filter(Boolean) : [];
    if (tags.length === 0) {
      out.set("未标注", (out.get("未标注") || 0) + 1);
      continue;
    }
    for (const t of tags) out.set(t, (out.get(t) || 0) + 1);
  }
  return out;
});

const repoTagBars = computed(() => toBarRows(topN(repoTagMap.value, 30)));
const repoTagChartHeight = computed(() => {
  const rows = Math.max(repoTagBars.value.length, 1);
  return rows * (barRowHeight + barRowGap) - barRowGap;
});

// 说明：默认 6 小时采样一次，避免频繁扫盘影响性能。
// 测试时可在 DevTools 里设置 localStorage 覆盖间隔（毫秒）：
// localStorage.setItem('guaxi_kb_volume_snapshot_interval_ms_v1', '5000')
const KB_VOLUME_SNAPSHOT_INTERVAL_MS_DEFAULT = 6 * 60 * 60 * 1000;
const KB_VOLUME_SNAPSHOT_INTERVAL_MS_KEY = "guaxi_kb_volume_snapshot_interval_ms_v1";
const KB_VOLUME_SNAPSHOT_MAX = 500;

function getVolumeSnapshotIntervalMs() {
  try {
    const raw = localStorage.getItem(KB_VOLUME_SNAPSHOT_INTERVAL_MS_KEY);
    const n = Number(raw);
    if (Number.isFinite(n) && n >= 1000) return Math.floor(n);
  } catch (e) {
    logger.warn("[statistics] read snapshot interval failed", e);
  }
  return KB_VOLUME_SNAPSHOT_INTERVAL_MS_DEFAULT;
}

function readVolumeSnapshotsFromLocal() {
  try {
    const raw = localStorage.getItem(KB_VOLUME_SNAPSHOTS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch (e) {
    logger.warn("[statistics] read volume snapshots failed", e);
    return [];
  }
}

function writeVolumeSnapshotsToLocal(list) {
  try {
    localStorage.setItem(KB_VOLUME_SNAPSHOTS_KEY, JSON.stringify(list));
  } catch (e) {
    logger.warn("[statistics] write volume snapshots failed", e);
  }
}

function recordVolumeSnapshotIfNeeded() {
  const list = Array.isArray(kbVolumeSnapshots.value) ? kbVolumeSnapshots.value.slice() : [];
  const lastTs = toSnapshotTsMs(list[list.length - 1]?.ts);
  const now = Date.now();
  const intervalMs = getVolumeSnapshotIntervalMs();

  if (Number.isFinite(lastTs) && lastTs > 0 && now - lastTs < intervalMs) return;

  const summaries = Array.isArray(kbSummaries.value) ? kbSummaries.value : [];
  if (summaries.length === 0) return;

  const items = {};
  for (const s of summaries) {
    const name = String(s?.knowledge_base_name || "").trim();
    if (!name) continue;
    items[name] = {
      file: Math.max(0, Number(s?.file_count) || 0),
      dir: Math.max(0, Number(s?.dir_count) || 0),
    };
  }

  list.push({ ts: now, items });
  const trimmed = list.slice(-KB_VOLUME_SNAPSHOT_MAX);
  kbVolumeSnapshots.value = trimmed;
  writeVolumeSnapshotsToLocal(trimmed);

  logger.info("[statistics] recordVolumeSnapshot", { ts: now, kbCount: Object.keys(items).length });
}

async function loadKbSummaries(opts) {
  const reason = String(opts?.reason || "manual");
  const list = await invoke("kbmeta_sync_all");
  const arr = Array.isArray(list) ? list : [];
  kbSummaries.value = arr;
  logger.info("[statistics] kbmeta_sync_all", { kbCount: arr.length, reason });
  recordVolumeSnapshotIfNeeded();
}

let volumeSnapshotTimer;
function clearVolumeSnapshotTimer() {
  if (volumeSnapshotTimer) {
    clearTimeout(volumeSnapshotTimer);
    volumeSnapshotTimer = null;
  }
}

function scheduleNextVolumeSnapshot() {
  clearVolumeSnapshotTimer();

  const intervalMs = getVolumeSnapshotIntervalMs();
  const list = Array.isArray(kbVolumeSnapshots.value) ? kbVolumeSnapshots.value : [];
  const lastTs = toSnapshotTsMs(list[list.length - 1]?.ts);
  const now = Date.now();

  const dueAt = Number.isFinite(lastTs) && lastTs > 0 ? lastTs + intervalMs : now;
  const delay = Math.max(1000, dueAt - now);

  logger.info("[statistics] scheduleNextVolumeSnapshot", {
    intervalMs,
    lastTs,
    dueAt,
    delay,
  });

  volumeSnapshotTimer = setTimeout(async () => {
    try {
      logger.info("[statistics] auto volume snapshot tick", { ts: new Date().toISOString() });
      await loadKbSummaries({ reason: "auto" });
    } catch (e) {
      logger.warn("[statistics] auto volume snapshot failed", e);
    } finally {
      scheduleNextVolumeSnapshot();
    }
  }, delay);
}

async function loadKnowledgeBasesGitLog() {
  try {
    const root = await invoke("get_knowledge_bases_root");
    const repoPath = typeof root === "string" ? root : "";
    if (!repoPath) {
      gitCommits.value = [];
      return;
    }

    let branch = "main";
    try {
      const b = await invoke("get_current_branch", { path: repoPath });
      branch = typeof b === "string" && b ? b : branch;
    } catch (e) {
      logger.warn("[statistics] get_current_branch failed, fallback to list_branches", e);
      try {
        const brs = await invoke("list_branches", { path: repoPath });
        const arr = Array.isArray(brs) ? brs : [];
        const current = arr.find((x) => !!x?.is_current)?.name;
        const first = arr[0]?.name;
        branch = String(current || first || branch);
      } catch (e2) {
        logger.warn("[statistics] list_branches failed", e2);
      }
    }

    try {
      const commits = await invoke("get_git_log", { path: repoPath, branchName: branch, limit: 2000 });
      gitCommits.value = Array.isArray(commits) ? commits : [];
      logger.info("[statistics] get_git_log", { branch, count: gitCommits.value.length });
      return;
    } catch (e) {
      logger.warn("[statistics] get_git_log failed, retry with fallback branch", { branch, err: e });
    }

    let fallback = branch === "main" ? "master" : "main";
    try {
      const brs = await invoke("list_branches", { path: repoPath });
      const arr = Array.isArray(brs) ? brs : [];
      const alt = arr.find((x) => x?.name && String(x.name) !== String(branch))?.name;
      fallback = String(alt || fallback);
    } catch (e3) {
      logger.warn("[statistics] list_branches failed (fallback)", e3);
    }

    const commits2 = await invoke("get_git_log", { path: repoPath, branchName: fallback, limit: 2000 });
    gitCommits.value = Array.isArray(commits2) ? commits2 : [];
    logger.info("[statistics] get_git_log", { branch: fallback, count: gitCommits.value.length, fallback: true });
  } catch (e) {
    logger.warn("[statistics] loadKnowledgeBasesGitLog failed", e);
    gitCommits.value = [];
  }
}

async function loadRepoImportedFiles() {
  const repos = await invoke("get_knowledge_repos");
  const list = Array.isArray(repos) ? repos.map((x) => String(x)).filter(Boolean) : [];
  repoNames.value = list;

  if (repoFilter.value !== "__all__" && !list.includes(String(repoFilter.value))) {
    repoFilter.value = "__all__";
  }

  const out = [];
  for (const r of list) {
    try {
      const files = await invoke("get_imported_files", { knowledgeRepo: r });
      const arr = Array.isArray(files) ? files : [];
      for (const f of arr) {
        out.push({
          ...f,
          knowledge_repo: f?.knowledge_repo || r,
        });
      }
    } catch (e) {
      logger.warn("[statistics] get_imported_files failed", { repo: r, err: e });
    }
  }

  repoImportedFiles.value = out;
}


/**
 * 构建热力图 activity：
 * - 统计近一年（365 天）每天的活跃次数
 * - 活跃来源：kb createdAt、kb modifiedAt、repo import create time
 */
function inLastYear(d) {
  if (!d) return false;
  const end = new Date();
  end.setHours(23, 59, 59, 999);
  const start = new Date(end);
  start.setDate(start.getDate() - 365);
  start.setHours(0, 0, 0, 0);
  return d >= start && d <= end;
}

const heatmapActivity = computed(() => {
  const out = {};
  const bump = (d, w = 1) => {
    if (!d) return;
    if (!inLastYear(d)) return;
    const key = toLocalDayKey(d);
    out[key] = (out[key] || 0) + w;
  };

  const list = Array.isArray(gitCommits.value) ? gitCommits.value : [];
  for (const c of list) {
    const d = parseDateSafe(c?.date);
    bump(d, 1);
  }

  return out;
});

const heatmapTotal = computed(() => {
  const obj = heatmapActivity.value || {};
  return Object.values(obj).reduce((a, b) => a + (Number(b) || 0), 0);
});

const heatmapActiveDays = computed(() => {
  const obj = heatmapActivity.value || {};
  return Object.values(obj).filter((v) => (Number(v) || 0) > 0).length;
});

async function reloadAll() {
  if (loading.value) return;

  loading.value = true;
  loadError.value = "";

  logger.info("[statistics] reloadAll start", {
    timeRange: timeRange.value,
    ts: new Date().toISOString(),
  });

  try {
    await Promise.all([
      loadKbSummaries({ reason: "reload" }),
      loadRepoImportedFiles(),
      loadKnowledgeBasesGitLog(),
    ]);

    // 扫描阅读器状态是同步的，不要放到并发里避免与刷新 UI 抢占节奏
    scanReaderState();

    message.success("统计数据已刷新");
  } catch (e) {
    logger.error("[statistics] reloadAll failed", e);
    loadError.value = String(e?.message || e || "加载失败");
    message.error("统计数据加载失败");
  } finally {
    loading.value = false;
    logger.info("[statistics] reloadAll end", { ts: new Date().toISOString() });
  }
}

watch(timeRange, () => {
  logger.info("[statistics] timeRange changed", timeRange.value);

  // 说明：切换时间范围只影响顶部时间分布图；数据源无需重新拉取
});

onMounted(async () => {
  logger.info("[statistics] dashboard mounted");

  kbVolumeSnapshots.value = readVolumeSnapshotsFromLocal();

  syncTheme();
  mo = new MutationObserver(() => syncTheme());
  mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-guaxi-theme", "class"] });

  updateTrendWidth();
  ro = new ResizeObserver(() => updateTrendWidth());
  const el = trendRootEl.value?.$el || trendRootEl.value;
  if (el) ro.observe(el);

  scanReaderState();
  await reloadAll();

  // 后台自动采样：按间隔到点后才触发 kbmeta_sync_all + 写快照
  scheduleNextVolumeSnapshot();
});

onBeforeUnmount(() => {
  try {
    clearVolumeSnapshotTimer();
    if (ro) ro.disconnect();
    if (mo) mo.disconnect();
  } catch (e) {
    logger.warn("[statistics] cleanup observers failed", e);
  }
});
</script>

<style scoped>
.app-layout {
  display: flex;
  flex-direction: column;
  height: 100%;
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

.main-header {
  flex: 0 0 auto;
  height: 35px;
  background-color: var(--guaxi-header-bg);
  box-shadow: var(--guaxi-header-shadow);
  display: flex;
  align-items: center;
}

.header-inner {
  width: 100%;
  padding: 0 10px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 10px;
}

.header-title {
  display: flex;
  align-items: center;
  gap: 10px;
}

.title-text {
  font-weight: 700;
  letter-spacing: 0.3px;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.main-content {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background-color: var(--guaxi-content-bg);
  overflow: hidden;
}

.page {
  padding: 12px;
  box-sizing: border-box;
}

.panel-card {
  border-radius: 12px;
  box-shadow: 0 8px 24px rgba(16, 24, 40, 0.06);
}

.empty {
  padding: 14px 0;
  opacity: 0.7;
  font-size: 13px;
}

.chart {
  width: 100%;
  height: auto;
  display: block;
}

.tag-chart-wrap {
  max-height: 320px;
  overflow: auto;
}

.trend-head {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: center;
  margin-bottom: 10px;
}

.trend-sub {
  font-size: 12px;
  opacity: 0.75;
  white-space: nowrap;
}

.trend-actions {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.trend-body {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.trend-chart {
  width: 100%;
  height: 220px;
}

.kb-legend {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  align-items: center;
  max-height: 84px;
  overflow: auto;
  padding-right: 6px;
}

.kb-legend-item {
  display: inline-flex;
  gap: 6px;
  align-items: center;
  font-size: 12px;
  padding: 2px 8px;
  border-radius: 10px;
  cursor: pointer;
  user-select: none;
  transition: background-color 0.15s ease, opacity 0.15s ease;
}

.kb-legend-item.active {
  background: rgba(59, 130, 246, 0.12);
}

.kb-legend-item.dim {
  opacity: 0.55;
}

.kb-dot {
  width: 8px;
  height: 8px;
  border-radius: 999px;
}

.kb-name {
  opacity: 0.9;
}

.kb-val {
  opacity: 0.75;
}

.read-head {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: center;
  margin-bottom: 10px;
}

.read-meta {
  font-size: 12px;
  white-space: nowrap;
  opacity: 0.75;
}

.read-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 360px;
  overflow: auto;
  padding-right: 6px;
}

.read-row {
  display: grid;
  grid-template-columns: minmax(120px, 1fr) 220px 80px;
  gap: 10px;
  align-items: center;
  padding: 6px 8px;
  border-radius: 10px;
  cursor: pointer;
  transition: background-color 0.15s ease, opacity 0.15s ease;
}

.read-row.active {
  background: rgba(34, 197, 94, 0.12);
}

.read-row.dim {
  opacity: 0.6;
}

.read-name {
  font-size: 12px;
  opacity: 0.92;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.read-progress {
  position: relative;
  height: 12px;
  border-radius: 999px;
  background: rgba(0, 0, 0, 0.06);
  overflow: hidden;
}

:global(:root[data-guaxi-theme="dark"]) .read-progress {
  background: rgba(255, 255, 255, 0.08);
}

.read-progress-bar {
  height: 100%;
  border-radius: 999px;
  background: linear-gradient(90deg, rgba(34, 197, 94, 0.85), rgba(59, 130, 246, 0.85));
}

.read-progress-text {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  font-size: 11px;
  opacity: 0.85;
}

.read-time {
  font-size: 12px;
  opacity: 0.8;
  text-align: right;
  white-space: nowrap;
}

@media (max-width: 700px) {
  .read-row {
    grid-template-columns: 1fr;
    gap: 6px;
  }
  .read-time {
    text-align: left;
  }
}

.stat-sub {
  margin-top: 8px;
  font-size: 12px;
  opacity: 0.75;
}

.muted {
  opacity: 0.7;
}

.progress-wrap {
  display: flex;
  flex-direction: column;
  gap: 10px;
  align-items: flex-start;
}

.progress-ring {
  position: relative;
  width: 140px;
  height: 140px;
}

.progress-ring svg {
  width: 140px;
  height: 140px;
}

.progress-text {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
}

.pct {
  font-size: 20px;
  font-weight: 750;
}

.sub {
  font-size: 12px;
  opacity: 0.75;
}

.heatmap-head {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: flex-start;
  margin-bottom: 10px;
}

.heatmap-sub {
  font-size: 12px;
  opacity: 0.75;
}

.heatmap-meta {
  font-size: 12px;
  white-space: nowrap;
  opacity: 0.75;
}
</style>