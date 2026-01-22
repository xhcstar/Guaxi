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
              <GuaxiStatisticsTimeCharts :time-range="timeRange" :kb-file-metas="kbFileMetas"
                :repo-imported-files="repoImportedFiles" />
            </n-grid-item>

            <n-grid-item :span="12">
              <n-card size="small" title="活跃热力图（近一年）" class="panel-card">
                <div class="heatmap-head">
                  <div class="heatmap-sub">
                    说明：统计“知识库新增/修改 + 资料库导入”的每日活跃度（近一年）。
                  </div>
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

            <n-grid-item :span="8">
              <n-card size="small" title="知识库分类（按标签）" class="panel-card">
                <div v-if="kbCategoryBars.length === 0" class="empty">暂无知识库数据</div>

                <svg v-else class="chart" :viewBox="`0 0 ${barChartWidth} ${barChartHeight}`"
                  preserveAspectRatio="none">
                  <defs>
                    <linearGradient id="kbBarGrad" x1="0" y1="0" x2="1" y2="0">
                      <stop offset="0%" stop-color="rgba(45, 212, 191, 0.85)" />
                      <stop offset="100%" stop-color="rgba(59, 130, 246, 0.85)" />
                    </linearGradient>
                  </defs>

                  <g v-for="(b, idx) in kbCategoryBars" :key="b.label">
                    <title>{{ b.label }}：{{ b.value }}</title>
                    <rect :x="0" :y="idx * (barRowHeight + barRowGap)" :width="b.w" :height="barRowHeight" rx="8"
                      fill="url(#kbBarGrad)" />
                    <text x="8" :y="idx * (barRowHeight + barRowGap) + 14" fill="rgba(255,255,255,0.92)" font-size="12">
                      {{ b.label }} · {{ b.value }}
                    </text>
                  </g>
                </svg>
              </n-card>
            </n-grid-item>

            <n-grid-item :span="4">
              <n-card size="small" title="阅读完成情况（阅读器本地记录）" class="panel-card">
                <div class="progress-wrap">
                  <div class="progress-ring">
                    <svg viewBox="0 0 120 120">
                      <circle cx="60" cy="60" r="46" fill="none" stroke="rgba(0,0,0,0.08)" stroke-width="12" />
                      <circle cx="60" cy="60" r="46" fill="none" stroke="rgba(34, 197, 94, 0.95)" stroke-width="12"
                        stroke-linecap="round" :stroke-dasharray="progressDashArray"
                        :stroke-dashoffset="progressDashOffset" transform="rotate(-90 60 60)" />
                    </svg>
                    <div class="progress-text">
                      <div class="pct">{{ readerCompletionPct }}%</div>
                      <div class="sub">{{ readerCompleted }} / {{ readerTotal }}</div>
                    </div>
                  </div>

                  <n-button size="small" secondary @click="scanReaderState">
                    重新扫描
                  </n-button>
                </div>

                <div class="stat-sub" style="margin-top: 10px">
                  说明：仅统计“曾打开过”的文档（localStorage：guaxi_reader_state_v1:*）。
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
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { createDiscreteApi } from "naive-ui";
import { useKnowledgeBaseStore } from "../../stores/knowledgeBaseStore";
import GuaxiStatisticsHeatmap from "./GuaxiStatisticsHeatmap.vue";
import GuaxiStatisticsTimeCharts from "./GuaxiStatisticsTimeCharts.vue";

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
];

const timeRange = ref("week");
const loading = ref(false);
const loadError = ref("");

const knowledgeBaseStore = useKnowledgeBaseStore();
const kbList = ref([]);
const kbFileMetas = ref([]);
const repoImportedFiles = ref([]);

/**
 * 阅读器本地状态扫描结果：
 * - total: 匹配到的 reader state 条目数（代表“曾打开过的文档”）
 * - completed: completedAt 不为空的条目数
 */
const readerState = ref({ total: 0, completed: 0 });

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

const range = computed(() => {
  if (timeRange.value === "week") return { start: daysAgo(7), end: new Date() };
  if (timeRange.value === "month") return { start: daysAgo(30), end: new Date() };
  return { start: daysAgo(365), end: new Date() };
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

function toBarRows(items) {
  const max = Math.max(...items.map((x) => x.value), 1);
  return items.map((x) => ({
    ...x,
    w: Math.round((x.value / max) * barChartWidth),
  }));
}

const kbCategoryMap = computed(() => {
  const out = new Map();
  const list = Array.isArray(kbList.value) ? kbList.value : [];
  for (const kb of list) {
    const tags = String(kb?.tags || "")
      .split(",")
      .map((x) => x.trim())
      .filter(Boolean);

    // 约定：取第一个标签作为“分类”
    const cat = tags[0] || "未分类";
    out.set(cat, (out.get(cat) || 0) + 1);
  }
  return out;
});

const kbCategoryBars = computed(() => toBarRows(topN(kbCategoryMap.value, 10)));
const barChartHeight = computed(() => {
  const rows = Math.max(kbCategoryBars.value.length, 1);
  return rows * (barRowHeight + barRowGap) - barRowGap;
});

function walkTreeFiles(node, acc) {
  if (!node) return;
  const kids = Array.isArray(node.children) ? node.children : [];
  for (const ch of kids) {
    if (ch?.is_dir) walkTreeFiles(ch, acc);
    else if (ch?.rel_path) acc.push(String(ch.rel_path));
  }
}

async function mapWithConcurrency(items, limit, fn) {
  const res = new Array(items.length);
  let idx = 0;

  const workers = new Array(Math.min(limit, items.length)).fill(0).map(async () => {
    while (idx < items.length) {
      const i = idx++;
      try {
        res[i] = await fn(items[i], i);
      } catch (e) {
        res[i] = null;
      }
    }
  });

  await Promise.all(workers);
  return res;
}

async function loadKbMetaAndFiles() {
  const list = await invoke("kbmeta_sync_all");
  const arr = Array.isArray(list) ? list : [];
  const fileTasks = [];

  for (const kb of arr) {
    const kbName = String(kb?.knowledge_base_name || kb?.name || "");
    const tree = kb?.tree;
    if (!kbName || !tree) continue;

    const rels = [];
    walkTreeFiles(tree, rels);
    for (const relPath of rels) fileTasks.push({ kbName, relPath });
  }

  console.info("[statistics] kbmeta_sync_all", {
    kbCount: arr.length,
    fileTaskCount: fileTasks.length,
  });

  const metas = await mapWithConcurrency(fileTasks, 6, async (t) => {
    const meta = await invoke("kbmeta_read_file_meta", {
      knowledgeBaseName: t.kbName,
      relPath: t.relPath,
    });
    const createdAt = parseDateSafe(meta?.created_at);
    const modifiedAt = parseDateSafe(meta?.modified_at);
    return { kbName: t.kbName, relPath: t.relPath, createdAt, modifiedAt };
  });

  kbList.value = arr;
  kbFileMetas.value = metas.filter(Boolean);
}

async function loadKnowledgeBaseListForCategories() {
  try {
    await knowledgeBaseStore.fetchKnowledgeBases();
    const list = Array.isArray(knowledgeBaseStore.knowledgeBases)
      ? knowledgeBaseStore.knowledgeBases
      : [];

    // 说明：kbmeta_sync_all 返回的 kbList 顺序与 store 的列表通常一致，这里按 index 进行 tags 合并
    const mapped = list.map((kb) => ({ tags: kb?.tags || "" }));
    for (let i = 0; i < mapped.length; i += 1) {
      if (kbList.value[i]) kbList.value[i].tags = mapped[i].tags;
    }
  } catch (e) {
    console.warn("[statistics] fetchKnowledgeBases failed", e);
  }
}

async function loadRepoImportedFiles() {
  const repos = await invoke("get_knowledge_repos");
  const repoNames = Array.isArray(repos) ? repos : [];

  const out = [];
  for (const r of repoNames) {
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
      console.warn("[statistics] get_imported_files failed", { repo: r, err: e });
    }
  }

  repoImportedFiles.value = out;
}

/**
 * 扫描阅读器本地状态（localStorage）：
 * - key 规则：guaxi_reader_state_v1:${library}:${fileType}:${safeId}
 * - value：JSON，含 completedAt 字段
 */
function scanReaderState() {
  try {
    let total = 0;
    let completed = 0;

    for (let i = 0; i < localStorage.length; i += 1) {
      const k = localStorage.key(i);
      if (!k) continue;
      if (!k.startsWith("guaxi_reader_state_v1:")) continue;

      total += 1;
      try {
        const raw = localStorage.getItem(k);
        if (!raw) continue;
        const parsed = JSON.parse(raw);
        if (parsed?.completedAt) completed += 1;
      } catch (e) {
        console.warn("[statistics] parse reader state failed", { key: k, err: e });
      }
    }

    readerState.value = { total, completed };

    console.info("[statistics] scanReaderState done", readerState.value);
  } catch (e) {
    console.warn("[statistics] scanReaderState failed", e);
    readerState.value = { total: 0, completed: 0 };
  }
}

const readerTotal = computed(() => readerState.value?.total || 0);
const readerCompleted = computed(() => readerState.value?.completed || 0);
const readerCompletionPct = computed(() => {
  if (readerTotal.value <= 0) return 0;
  return Math.min(100, Math.max(0, Math.round((readerCompleted.value / readerTotal.value) * 100)));
});

const progressDashArray = computed(() => {
  const r = 46;
  const c = 2 * Math.PI * r;
  return `${c} ${c}`;
});

const progressDashOffset = computed(() => {
  const r = 46;
  const c = 2 * Math.PI * r;
  return c - (readerCompletionPct.value / 100) * c;
});

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

  for (const it of kbFileMetas.value || []) {
    bump(it?.createdAt, 1);
    bump(it?.modifiedAt, 1);
  }

  for (const f of repoImportedFiles.value || []) {
    const d = parseDateSafe(f?.import_file_create_time || f?.importFileCreateTime);
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

  console.info("[statistics] reloadAll start", {
    timeRange: timeRange.value,
    ts: new Date().toISOString(),
  });

  try {
    await Promise.all([loadKbMetaAndFiles(), loadRepoImportedFiles()]);
    await loadKnowledgeBaseListForCategories();

    // 扫描阅读器状态是同步的，不要放到并发里避免与刷新 UI 抢占节奏
    scanReaderState();

    message.success("统计数据已刷新");
  } catch (e) {
    console.error("[statistics] reloadAll failed", e);
    loadError.value = String(e?.message || e || "加载失败");
    message.error("统计数据加载失败");
  } finally {
    loading.value = false;
    console.info("[statistics] reloadAll end", { ts: new Date().toISOString() });
  }
}

watch(timeRange, () => {
  console.info("[statistics] timeRange changed", timeRange.value);

  // 说明：切换时间范围只影响顶部时间分布图；数据源无需重新拉取
});

onMounted(async () => {
  console.info("[statistics] dashboard mounted");
  scanReaderState();
  await reloadAll();
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
  height: 220px;
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