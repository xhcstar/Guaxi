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
          <n-button size="small" :loading="loading" @click="onManualRefresh">刷新</n-button>
        </div>
      </div>
    </div>

    <div class="main-content">
      <n-scrollbar style="height: 100%">
        <div class="page" :class="{ 'theme-dark': isDark }">
          <n-alert v-if="loadError" type="error" :show-icon="true" style="margin-bottom: 12px">
            {{ loadError }}
          </n-alert>

          <n-grid :x-gap="12" :y-gap="12" :cols="12">
            <n-grid-item :span="12">
              <n-card ref="trendRootEl" size="small" title="知识量趋势" class="panel-card">
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

                  <svg
                    class="trend-chart"
                    :viewBox="`0 0 ${trendW} ${trendH}`"
                    preserveAspectRatio="xMidYMid meet"
                    @mousemove="onTrendMouseMove"
                    @mouseleave="onTrendMouseLeave"
                  >
                    <g v-for="s in volumeSeries" :key="`line-${s.name}`">
                      <path
                        :d="s.linePath"
                        fill="none"
                        :stroke="s.color"
                        :stroke-width="isKbSeriesActive(s.name) ? 2.5 : 1.5"
                        :opacity="isKbSeriesActive(s.name) ? 1 : 0.15"
                      />
                    </g>

                    <g v-if="trendHoverIndex >= 0">
                      <line
                        :x1="trendHoverX"
                        :x2="trendHoverX"
                        :y1="trendPlot.plotTop"
                        :y2="trendPlot.plotBottom"
                        stroke="rgba(148, 163, 184, 0.85)"
                        stroke-width="1"
                      />

                      <g v-for="s in volumeSeries" :key="`hp-${s.name}`">
                        <circle
                          v-if="isKbSeriesActive(s.name) && s.points && s.points[trendHoverIndex]"
                          :cx="s.points[trendHoverIndex].x"
                          :cy="s.points[trendHoverIndex].y"
                          r="3.5"
                          :fill="s.color"
                          stroke="rgba(255, 255, 255, 0.95)"
                          stroke-width="1"
                        />
                      </g>

                      <foreignObject
                        :x="trendTooltipX"
                        :y="trendPlot.plotTop"
                        :width="TREND_TOOLTIP_W"
                        :height="trendTooltipH"
                      >
                        <div xmlns="http://www.w3.org/1999/xhtml" class="trend-tooltip">
                          <div class="tt-title">{{ trendHoverTitle }}</div>
                          <div v-for="r in trendHoverRows" :key="`tt-${r.name}`" class="tt-row">
                            <span class="tt-dot" :style="{ background: r.color }"></span>
                            <span class="tt-name">{{ r.name }}</span>
                            <span class="tt-val">{{ r.value }}</span>
                          </div>
                        </div>
                      </foreignObject>
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
              <n-card size="small" title="活跃热力图" class="panel-card">
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
              <n-card size="small" title="阅读进度" class="panel-card">
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

                <div v-else ref="readListEl" class="read-list">
                  <div
                    v-for="r in readerDocRows"
                    :key="r.key"
                    class="read-row"
                    :data-read-key="r.key"
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
              <n-card size="small" title="资料库文档" class="panel-card">
                <div class="trend-head">
                  <div class="trend-sub">按导入文件 tags 统计（可切换知识库）</div>
                  <div class="trend-actions">
                    <n-select size="small" v-model:value="repoFilter" :options="repoOptions" style="width: 240px" />
                  </div>
                </div>

                <div v-if="repoTagPieSlices.length === 0" class="empty">暂无资料库数据</div>

                <div v-else class="tag-pie-wrap">
                  <svg class="chart" :viewBox="`0 0 ${repoTagPieW} ${repoTagPieH}`" preserveAspectRatio="xMinYMin meet">
                    <g>
                      <g
                        v-for="s in repoTagPieSlices"
                        :key="`pie-${s.label}`"
                        style="cursor: pointer"
                        @mouseenter="setHoveredRepoTag(s.label)"
                        @mouseleave="clearHoveredRepoTag"
                        @click="toggleRepoTag(s.label)"
                      >
                        <title>{{ s.label }}：{{ s.value }}（{{ s.percentText }}）</title>
                        <path
                          :d="s.path"
                          :fill="s.color"
                          :opacity="repoTagSliceOpacity(s)"
                          :stroke="repoTagSliceStroke(s)"
                          :stroke-width="repoTagSliceStrokeWidth(s)"
                          :transform="repoTagSliceTransform(s)"
                        />
                      </g>

                      <circle :cx="repoTagPieCx" :cy="repoTagPieCy" :r="repoTagPieInnerR" fill="var(--guaxi-panel-bg)" />

                      <text :x="repoTagPieCx" :y="repoTagPieCy - 6" text-anchor="middle" font-size="12" :fill="axisLabelFill" style="font-weight: 700">
                        {{ repoTagCenterTitle }}
                      </text>
                      <text :x="repoTagPieCx" :y="repoTagPieCy + 14" text-anchor="middle" font-size="12" :fill="axisLabelFill" style="opacity: 0.92">
                        {{ repoTagCenterSub }}
                      </text>
                    </g>

                    <g :transform="`translate(${repoTagLegendX} ${repoTagLegendY})`">
                      <g
                        v-for="(s, idx) in repoTagPieSlices"
                        :key="`leg-${s.label}`"
                        style="cursor: pointer"
                        :transform="`translate(0 ${idx * repoTagLegendRowH})`"
                        @mouseenter="setHoveredRepoTag(s.label)"
                        @mouseleave="clearHoveredRepoTag"
                        @click="toggleRepoTag(s.label)"
                      >
                        <rect x="0" y="-9" width="10" height="10" rx="2" :fill="s.color" :opacity="repoTagSliceOpacity(s)" />
                        <text x="16" y="0" dominant-baseline="middle" font-size="12" :fill="axisLabelFill" :opacity="repoTagSliceOpacity(s)">
                          {{ s.label }} · {{ s.value }} · {{ s.percentText }}
                        </text>
                      </g>
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
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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

const knowledgeBasesRoot = ref("");

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
const hoveredRepoTag = ref("");
const activeReaderKey = ref("");

const readListEl = ref(null);

function cssEscapeForAttr(v) {
  const s = String(v || "");
  try {
    if (typeof CSS !== "undefined" && typeof CSS.escape === "function") return CSS.escape(s);
  } catch (e) {
    logger.warn("[statistics] CSS.escape failed", e);
  }
  return s.replace(/["\\]/g, "\\$&");
}

async function scrollReaderRowIntoView(key, reason) {
  const wrap = readListEl.value;
  const k = String(key || "");
  if (!wrap || !k) return;

  await nextTick();

  const selector = `[data-read-key="${cssEscapeForAttr(k)}"]`;
  const row = wrap.querySelector?.(selector);
  if (!row || typeof row.scrollIntoView !== "function") return;

  row.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
  logger.info("[statistics] scroll reader row into view", { reason: String(reason || "auto"), key: k });
}

function setHoveredRepoTag(tag) {
  hoveredRepoTag.value = String(tag || "");
}

function clearHoveredRepoTag() {
  hoveredRepoTag.value = "";
}

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
  logger.info("[statistics] toggleRepoTag", { tag: t, selected: selectedRepoTag.value || "" });
}

function toggleActiveReader(key) {
  const k = String(key || "");
  if (!k) return;
  activeReaderKey.value = activeReaderKey.value === k ? "" : k;
}

watch(activeReaderKey, (k, prev) => {
  const cur = String(k || "");
  if (!cur) return;
  if (cur === String(prev || "")) return;
  scrollReaderRowIntoView(cur, "active_changed");
});

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

const trendHoverIndex = ref(-1);
const trendHoverX = ref(0);
const TREND_TOOLTIP_W = 240;

const trendHoverTitle = computed(() => {
  const idx = Number(trendHoverIndex.value);
  if (!Number.isFinite(idx) || idx < 0) return "";
  const p = trendPoints.value[idx];
  if (!p) return "";
  const unit = effectiveTrendUnit.value;
  const label = formatTickLabel(p?.tsMs, unit);
  const metricLabel = volumeMetric.value === "dir" ? "目录数" : "文件数";
  return `${label} · ${metricLabel}`;
});

const trendHoverRows = computed(() => {
  const idx = Number(trendHoverIndex.value);
  if (!Number.isFinite(idx) || idx < 0) return [];

  const rows = [];
  for (const s of volumeSeries.value) {
    if (!isKbSeriesActive(s.name)) continue;
    const v = Array.isArray(s.values) ? s.values[idx] : undefined;
    rows.push({ name: s.name, color: s.color, value: Number(v) || 0 });
  }

  rows.sort((a, b) => b.value - a.value);
  return rows.slice(0, 10);
});

const trendTooltipH = computed(() => {
  const n = trendHoverRows.value.length;
  return 28 + n * 18;
});

const trendTooltipX = computed(() => {
  const w = Number(trendW.value) || 0;
  const x = Number(trendHoverX.value) || 0;
  const next = x + 10;
  const maxX = Math.max(4, w - TREND_TOOLTIP_W - 4);
  return Math.max(4, Math.min(next, maxX));
});

function onTrendMouseMove(e) {
  const el = e?.currentTarget;
  const rect = el?.getBoundingClientRect?.();
  if (!rect) return;

  const n = trendPointCount.value;
  if (!n) return;

  const xPx = Number(e?.clientX) - rect.left;
  const ratio = rect.width > 0 ? xPx / rect.width : 0;
  const x = Math.max(0, Math.min(1, ratio)) * trendW.value;

  const p = trendPlot.value;
  const idx = n <= 1 ? 0 : Math.round(((x - p.plotLeft) / p.plotW) * (n - 1));
  const clamped = Math.max(0, Math.min(n - 1, idx));

  trendHoverIndex.value = clamped;
  trendHoverX.value = trendX(clamped, n);
}

function onTrendMouseLeave() {
  trendHoverIndex.value = -1;
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
    const points = [];
    const values = [];

    for (let i = 0; i < n; i += 1) {
      const v = Number(snaps[i]?.items?.[nm]?.[metric]) || 0;
      const x = p.plotLeft + (n <= 1 ? 0 : (i / (n - 1)) * p.plotW);
      const y = p.plotBottom - (v / maxV) * p.plotH;

      const sx = Math.round(x * 2) / 2;
      const sy = Math.round(y * 2) / 2;

      values.push(v);
      points.push({ x: sx, y: sy, v, tsMs: snaps[i]?.tsMs });
    }

    const linePath = points
      .map((pt, idx) => `${idx === 0 ? "M" : "L"}${pt.x.toFixed(2)} ${pt.y.toFixed(2)}`)
      .join(" ");
    const lastValue = Number(snaps[n - 1]?.items?.[nm]?.[metric]) || 0;
    series.push({ name: nm, color, lastValue, linePath, points, values });
  }

  return series;
});

const barChartWidth = 520;
const barRowHeight = 16;
const barRowGap = 8;

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

const repoTagPieW = 520;
const repoTagPieH = 220;
const repoTagPieCx = 110;
const repoTagPieCy = 110;
const repoTagPieR = 88;
const repoTagPieInnerR = 52;
const repoTagLegendX = 230;
const repoTagLegendY = 26;
const repoTagLegendRowH = 18;
const REPO_TAG_PIE_MAX_SLICES = 10;

function piePoint(cx, cy, r, aRad) {
  return { x: cx + r * Math.cos(aRad), y: cy + r * Math.sin(aRad) };
}

function pieSlicePath(cx, cy, r, a0, a1) {
  const delta = a1 - a0;
  if (delta >= Math.PI * 2 - 1e-6) {
    const p0 = piePoint(cx, cy, r, a0);
    const pm = piePoint(cx, cy, r, a0 + Math.PI);
    return `M ${cx} ${cy} L ${p0.x.toFixed(2)} ${p0.y.toFixed(2)} A ${r} ${r} 0 1 1 ${pm.x.toFixed(2)} ${pm.y.toFixed(2)} A ${r} ${r} 0 1 1 ${p0.x.toFixed(2)} ${p0.y.toFixed(2)} Z`;
  }

  const p0 = piePoint(cx, cy, r, a0);
  const p1 = piePoint(cx, cy, r, a1);
  const largeArc = delta > Math.PI ? 1 : 0;
  return `M ${cx} ${cy} L ${p0.x.toFixed(2)} ${p0.y.toFixed(2)} A ${r} ${r} 0 ${largeArc} 1 ${p1.x.toFixed(2)} ${p1.y.toFixed(2)} Z`;
}

const repoTagPieSlices = computed(() => {
  const entries = Array.from((repoTagMap.value || new Map()).entries()).sort((a, b) => b[1] - a[1]);
  const total = entries.reduce((sum, it) => sum + (Number(it[1]) || 0), 0);
  if (!total) return [];

  const head = entries.slice(0, REPO_TAG_PIE_MAX_SLICES);
  const tail = entries.slice(REPO_TAG_PIE_MAX_SLICES);
  const tailSum = tail.reduce((sum, it) => sum + (Number(it[1]) || 0), 0);

  const items = head.map(([label, value]) => ({ label: String(label), value: Number(value) || 0 }));
  if (tailSum > 0) items.push({ label: "其他", value: tailSum });

  let a = -Math.PI / 2;
  const out = [];
  for (const it of items) {
    const ratio = total > 0 ? it.value / total : 0;
    const next = a + ratio * Math.PI * 2;
    const mid = (a + next) / 2;

    const color = colorForTag(it.label);
    const path = pieSlicePath(repoTagPieCx, repoTagPieCy, repoTagPieR, a, next);

    const dx = Math.round(Math.cos(mid) * 6 * 2) / 2;
    const dy = Math.round(Math.sin(mid) * 6 * 2) / 2;

    const percent = ratio * 100;
    const percentText = `${percent.toFixed(1)}%`;

    out.push({
      label: it.label,
      value: it.value,
      color,
      ratio,
      percentText,
      path,
      dx,
      dy,
    });

    a = next;
  }

  return out;
});

function isRepoTagEmphasized(slice) {
  const label = String(slice?.label || "");
  if (!label) return false;
  if (hoveredRepoTag.value && hoveredRepoTag.value === label) return true;
  if (selectedRepoTag.value && selectedRepoTag.value === label) return true;
  return false;
}

function repoTagSliceOpacity(slice) {
  const label = String(slice?.label || "");
  if (!label) return 1;

  if (selectedRepoTag.value && selectedRepoTag.value !== label) return 0.22;
  if (hoveredRepoTag.value && hoveredRepoTag.value !== label) return 0.45;
  return 1;
}

function repoTagSliceStroke(slice) {
  return isRepoTagEmphasized(slice) ? axisLabelFill.value : "transparent";
}

function repoTagSliceStrokeWidth(slice) {
  return isRepoTagEmphasized(slice) ? 1.25 : 0;
}

function repoTagSliceTransform(slice) {
  if (!slice) return "";
  if (!isRepoTagEmphasized(slice)) return "";
  const dx = Number(slice?.dx) || 0;
  const dy = Number(slice?.dy) || 0;
  if (!dx && !dy) return "";
  return `translate(${dx.toFixed(2)} ${dy.toFixed(2)})`;
}

const repoTagCenterTitle = computed(() => {
  const key = String(hoveredRepoTag.value || selectedRepoTag.value || "");
  if (!key) return "资料分类";
  return key;
});

const repoTagCenterSub = computed(() => {
  const key = String(hoveredRepoTag.value || selectedRepoTag.value || "");
  const slices = Array.isArray(repoTagPieSlices.value) ? repoTagPieSlices.value : [];
  const total = slices.reduce((sum, s) => sum + (Number(s?.value) || 0), 0);

  if (!key) return `总计 ${total}`;

  const hit = slices.find((s) => String(s?.label) === key);
  if (!hit) return "";

  return `${hit.value} · ${hit.percentText}`;
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

function recordVolumeSnapshotIfNeeded(opts) {
  const force = !!opts?.force;

  const list = Array.isArray(kbVolumeSnapshots.value) ? kbVolumeSnapshots.value.slice() : [];
  const lastTs = toSnapshotTsMs(list[list.length - 1]?.ts);
  const now = Date.now();
  const intervalMs = getVolumeSnapshotIntervalMs();

  if (!force && Number.isFinite(lastTs) && lastTs > 0 && now - lastTs < intervalMs) return;

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
  const forceSnapshot = !!opts?.forceSnapshot;

  const list = await invoke("kbmeta_sync_all");
  const arr = Array.isArray(list) ? list : [];
  kbSummaries.value = arr;
  logger.info("[statistics] kbmeta_sync_all", { kbCount: arr.length, reason, forceSnapshot });
  recordVolumeSnapshotIfNeeded({ force: forceSnapshot });
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

async function reloadAll(opts) {
  if (loading.value) return;

  const reason = String(opts?.reason || "reload");
  const silent = !!opts?.silent;
  const includeGit = opts?.includeGit !== false;
  const forceSnapshot = !!opts?.forceSnapshot;

  loading.value = true;
  loadError.value = "";

  logger.info("[statistics] reloadAll start", {
    timeRange: timeRange.value,
    reason,
    silent,
    includeGit,
    forceSnapshot,
    ts: new Date().toISOString(),
  });

  try {
    const tasks = [loadKbSummaries({ reason, forceSnapshot }), loadRepoImportedFiles()];
    if (includeGit) tasks.push(loadKnowledgeBasesGitLog());

    await Promise.all(tasks);

    scanReaderState();

    if (!silent) message.success("统计数据已刷新");
  } catch (e) {
    logger.error("[statistics] reloadAll failed", e);
    loadError.value = String(e?.message || e || "加载失败");
    if (!silent) message.error("统计数据加载失败");
  } finally {
    loading.value = false;
    logger.info("[statistics] reloadAll end", { ts: new Date().toISOString() });
  }
}

async function onManualRefresh() {
  logger.info("[statistics] manual refresh clicked", { ts: new Date().toISOString() });
  await reloadAll({ reason: "manual", silent: false, includeGit: true, forceSnapshot: true });
}

let autoReloadTimer = null;
let lastAutoReloadAt = 0;
let cleanupAutoRefreshHooks = null;

let unlistenFileChange = null;
let unlistenConversionProgress = null;
let unlistenAddWebClipper = null;
let unlistenAddBrowserSavedHtml = null;
let unlistenRepoImportedFilesChanged = null;

const AUTO_RELOAD_DEBOUNCE_MS = 1200;
const AUTO_RELOAD_MIN_INTERVAL_MS = 4500;

function scheduleAutoReload(reason, opts) {
  const r = String(reason || "auto");
  const forceSnapshot = !!opts?.forceSnapshot;

  if (autoReloadTimer) clearTimeout(autoReloadTimer);
  autoReloadTimer = setTimeout(() => {
    autoReloadTimer = null;

    const now = Date.now();
    if (now - lastAutoReloadAt < AUTO_RELOAD_MIN_INTERVAL_MS) {
      logger.info("[statistics] auto reload skipped (min-interval)", {
        reason: r,
        lastAutoReloadAt,
        now,
        minIntervalMs: AUTO_RELOAD_MIN_INTERVAL_MS,
      });
      return;
    }

    lastAutoReloadAt = now;
    reloadAll({ reason: r, silent: true, includeGit: false, forceSnapshot });
  }, AUTO_RELOAD_DEBOUNCE_MS);
}

watch(timeRange, () => {
  logger.info("[statistics] timeRange changed", timeRange.value);

  // 说明：切换时间范围只影响顶部时间分布图；数据源无需重新拉取
});

onMounted(async () => {
  logger.info("[statistics] dashboard mounted");

  try {
    const root = await invoke("get_knowledge_bases_root");
    const kbRoot = typeof root === "string" ? root : "";
    knowledgeBasesRoot.value = kbRoot;

    if (kbRoot) {
      await invoke("start_file_watching", { path: kbRoot });
      logger.info("[statistics] file watcher started (KnowledgeBases root)", { path: kbRoot });
    } else {
      logger.warn("[statistics] get_knowledge_bases_root returned empty path");
    }
  } catch (e) {
    logger.warn("[statistics] start_file_watching failed (KnowledgeBases root)", e);
  }

  kbVolumeSnapshots.value = readVolumeSnapshotsFromLocal();

  syncTheme();
  mo = new MutationObserver(() => syncTheme());
  mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-guaxi-theme", "class"] });

  updateTrendWidth();
  ro = new ResizeObserver(() => updateTrendWidth());
  const el = trendRootEl.value?.$el || trendRootEl.value;
  if (el) ro.observe(el);

  scanReaderState();
  await reloadAll({ reason: "mounted", silent: true, includeGit: true, forceSnapshot: false });

  cleanupAutoRefreshHooks = (() => {
    const onFocus = () => scheduleAutoReload("window_focus", { forceSnapshot: false });
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  })();

  try {
    unlistenFileChange = await listen("file_change", (event) => {
      const payload = event?.payload || {};
      const kind = String(payload?.kind || "");
      if (kind === "Create" || kind === "Remove" || kind === "Rename") {
        scheduleAutoReload(`file_change:${kind}`, { forceSnapshot: true });
      }
    });

    unlistenConversionProgress = await listen("conversion_progress", (event) => {
      const p = event?.payload || {};
      const current = Number(p?.current) || 0;
      const msg = String(p?.message || "");
      const failed = !!p?.failed || msg.includes("失败");
      if (current >= 100 && !failed) {
        scheduleAutoReload("conversion_progress:done", { forceSnapshot: true });
      }
    });

    unlistenAddWebClipper = await listen("add_web_clipper_file_event", () => {
      scheduleAutoReload("add_web_clipper_file_event", { forceSnapshot: true });
    });

    unlistenAddBrowserSavedHtml = await listen("add_browser_saved_html_file_event", () => {
      scheduleAutoReload("add_browser_saved_html_file_event", { forceSnapshot: true });
    });

    unlistenRepoImportedFilesChanged = await listen("repo_imported_files_changed", (event) => {
      const payload = event?.payload || {};
      scheduleAutoReload("repo_imported_files_changed", { forceSnapshot: true });
      logger.info("[statistics] repo_imported_files_changed", payload);
    });

    logger.info("[statistics] auto refresh listeners ready");
  } catch (e) {
    logger.warn("[statistics] init auto refresh listeners failed", e);
  }

  scheduleNextVolumeSnapshot();
});

onBeforeUnmount(() => {
  try {
    clearVolumeSnapshotTimer();

    if (autoReloadTimer) {
      clearTimeout(autoReloadTimer);
      autoReloadTimer = null;
    }

    Promise.resolve(invoke("stop_file_watching")).catch((e) => {
      logger.warn("[statistics] stop_file_watching failed", e);
    });

    if (typeof cleanupAutoRefreshHooks === "function") {
      try {
        cleanupAutoRefreshHooks();
      } catch (e) {
        logger.warn("[statistics] cleanup auto refresh hooks failed", e);
      }
      cleanupAutoRefreshHooks = null;
    }

    const unlistens = [
      unlistenFileChange,
      unlistenConversionProgress,
      unlistenAddWebClipper,
      unlistenAddBrowserSavedHtml,
      unlistenRepoImportedFilesChanged,
    ];
    for (const fn of unlistens) {
      if (typeof fn === "function") {
        Promise.resolve(fn()).catch((e) => logger.warn("[statistics] unlisten failed", e));
      }
    }
    unlistenFileChange = null;
    unlistenConversionProgress = null;
    unlistenAddWebClipper = null;
    unlistenAddBrowserSavedHtml = null;
    unlistenRepoImportedFilesChanged = null;

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

.tag-pie-wrap {
  width: 100%;
  overflow: hidden;
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

.trend-tooltip {
  font-size: 12px;
  line-height: 1.2;
  padding: 8px 10px;
  border-radius: 10px;
  color: rgba(15, 23, 42, 0.92);
  background: rgba(255, 255, 255, 0.82);
  border: 1px solid rgba(15, 23, 42, 0.12);
  box-shadow: 0 10px 28px rgba(15, 23, 42, 0.12);
  backdrop-filter: blur(8px);
  pointer-events: none;
}

.theme-dark .trend-tooltip {
  color: rgba(255, 255, 255, 0.92);
  background: rgba(15, 23, 42, 0.64);
  border: 1px solid rgba(148, 163, 184, 0.28);
  box-shadow: 0 10px 28px rgba(0, 0, 0, 0.42);
}

.trend-tooltip .tt-title {
  font-weight: 700;
  margin-bottom: 6px;
  opacity: 0.95;
}

.trend-tooltip .tt-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 2px 0;
}

.trend-tooltip .tt-dot {
  width: 8px;
  height: 8px;
  border-radius: 999px;
  flex: 0 0 auto;
}

.trend-tooltip .tt-name {
  flex: 1 1 auto;
  opacity: 0.92;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.trend-tooltip .tt-val {
  flex: 0 0 auto;
  font-variant-numeric: tabular-nums;
  opacity: 0.95;
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
  color: var(--guaxi-text-primary);
  transition: background-color 0.15s ease, opacity 0.15s ease;
}

.theme-dark .kb-legend-item {
  color: rgba(255, 255, 255, 0.92);
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
  opacity: 0.95;
  font-variant-numeric: tabular-nums;
  padding: 0 8px;
  border-radius: 999px;
  line-height: 18px;
  height: 18px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: rgba(255, 255, 255, 0.92);
  background: rgba(15, 23, 42, 0.66);
  border: 1px solid rgba(15, 23, 42, 0.2);
}

.theme-dark .kb-val {
  color: rgba(0, 0, 0, 0.78);
  background: rgba(255, 255, 255, 0.72);
  border: 1px solid rgba(255, 255, 255, 0.22);
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
  transition: background-color 0.15s ease, opacity 0.15s ease, transform 0.15s ease, box-shadow 0.15s ease;
}

.read-row:hover {
  background: rgba(59, 130, 246, 0.08);
  box-shadow: 0 6px 18px rgba(16, 24, 40, 0.08);
  transform: translateY(-1px);
  opacity: 1;
}

.theme-dark .read-row:hover {
  background: rgba(255, 255, 255, 0.06);
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.35);
}

.read-row.active {
  background: rgba(34, 197, 94, 0.12);
}

.read-row.active:hover {
  background: rgba(34, 197, 94, 0.16);
}

.read-row.dim {
  opacity: 0.6;
}

.read-row.dim:hover {
  opacity: 0.92;
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

.theme-dark .read-progress {
  background: rgba(148, 163, 184, 0.14);
  border: 1px solid rgba(148, 163, 184, 0.18);
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
  opacity: 0.9;
}

.theme-dark .read-progress-text {
  color: rgba(255, 255, 255, 0.92);
  background: rgba(0, 0, 0, 0.28);
  padding: 0 6px;
  border-radius: 999px;
  text-shadow: 0 1px 1px rgba(0, 0, 0, 0.6);
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