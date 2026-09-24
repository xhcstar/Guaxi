<template>
  <n-card size="small" title="知识库领域洞察" class="panel-card">
    <div class="kbi-head">
      <div class="kbi-sub">基于知识图谱(kgraph)的领域知识量、从属/关联结构、社区与核心概念</div>
      <div class="kbi-actions">
        <n-select size="small" v-model:value="selectedKb" :options="kbOptions" style="width: 220px"
          placeholder="选择知识库" @update:value="loadKb" />
        <n-button size="tiny" :loading="loading" @click="loadKb(selectedKb)">刷新</n-button>
        <n-button size="tiny" :loading="detecting" @click="detectCommunities">重新计算社区</n-button>
      </div>
    </div>

    <div v-if="!selectedKb" class="kbi-empty">请选择一个知识库</div>
    <div v-else-if="loading" class="kbi-empty">加载中…</div>
    <div v-else-if="notIndexed" class="kbi-empty">该知识库还没有图索引,请先在「知识库管理」里重建图索引后再看。</div>
    <div v-else class="kbi-grid">
      <!-- 概览指标 -->
      <div class="kbi-tiles">
        <div class="kbi-tile"><div class="kbi-tile__v">{{ stats.total_nodes || 0 }}</div><div class="kbi-tile__l">节点</div></div>
        <div class="kbi-tile"><div class="kbi-tile__v">{{ stats.total_edges || 0 }}</div><div class="kbi-tile__l">关系边</div></div>
        <div class="kbi-tile"><div class="kbi-tile__v">{{ conceptCount }}</div><div class="kbi-tile__l">概念</div></div>
        <div class="kbi-tile"><div class="kbi-tile__v">{{ assocDensity }}</div><div class="kbi-tile__l">关联密度</div></div>
      </div>

      <!-- 领域知识量(标签) + 内容构成(节点类型) -->
      <div class="kbi-row">
        <div class="kbi-chart">
          <div class="kbi-chart__t">领域知识量(按标签)</div>
          <div v-if="!tagSlices.length" class="kbi-empty-sm">无标签数据</div>
          <svg v-else class="kbi-donut" viewBox="0 0 240 150">
            <g v-for="s in tagSlices" :key="'t'+s.label"><title>{{ s.label }}:{{ s.value }}({{ s.pct }})</title>
              <path :d="s.path" :fill="s.color" /></g>
            <circle cx="70" cy="75" r="34" fill="var(--guaxi-panel-bg, #fff)" />
            <g v-for="(s,i) in tagSlices" :key="'tl'+s.label" :transform="`translate(150 ${18 + i*15})`">
              <rect x="0" y="-8" width="9" height="9" rx="2" :fill="s.color" />
              <text x="14" y="0" dominant-baseline="middle" font-size="11" :fill="labelFill">{{ s.label }} · {{ s.value }}</text>
            </g>
          </svg>
        </div>
        <div class="kbi-chart">
          <div class="kbi-chart__t">内容构成(按节点类型)</div>
          <svg class="kbi-donut" viewBox="0 0 240 150">
            <g v-for="s in typeSlices" :key="'ty'+s.label"><title>{{ s.label }}:{{ s.value }}({{ s.pct }})</title>
              <path :d="s.path" :fill="s.color" /></g>
            <circle cx="70" cy="75" r="34" fill="var(--guaxi-panel-bg, #fff)" />
            <g v-for="(s,i) in typeSlices" :key="'tyl'+s.label" :transform="`translate(150 ${18 + i*15})`">
              <rect x="0" y="-8" width="9" height="9" rx="2" :fill="s.color" />
              <text x="14" y="0" dominant-baseline="middle" font-size="11" :fill="labelFill">{{ s.label }} · {{ s.value }}</text>
            </g>
          </svg>
        </div>
      </div>

      <!-- PLACEHOLDER_TEMPLATE2 -->

      <!-- 关系构成 -->
      <div class="kbi-chart kbi-full">
        <div class="kbi-chart__t">关系构成(从属 vs 关联)</div>
        <div v-if="!edgeRows.length" class="kbi-empty-sm">无关系数据</div>
        <div v-for="r in edgeRows" :key="'e'+r.label" class="kbi-bar">
          <span class="kbi-bar__l">{{ r.label }}</span>
          <span class="kbi-bar__track"><span class="kbi-bar__fill" :style="{ width: r.pct + '%', background: r.color }" /></span>
          <span class="kbi-bar__v">{{ r.value }}</span>
        </div>
      </div>

      <!-- 核心概念/节点 Top -->
      <div class="kbi-chart kbi-full">
        <div class="kbi-chart__t">核心节点 Top(按 PageRank)</div>
        <div v-if="!topNodeRows.length" class="kbi-empty-sm">无核心节点</div>
        <div v-for="r in topNodeRows" :key="'n'+r.label" class="kbi-bar">
          <span class="kbi-bar__l" :title="r.label">{{ r.label }}</span>
          <span class="kbi-bar__track"><span class="kbi-bar__fill" :style="{ width: r.pct + '%', background: r.color }" /></span>
          <span class="kbi-bar__v">{{ r.type }}</span>
        </div>
      </div>

      <!-- 主题社区 -->
      <div class="kbi-chart kbi-full">
        <div class="kbi-chart__t">主题社区</div>
        <div v-if="!communitySlices.length" class="kbi-empty-sm">还没有社区,点上方「重新计算社区」生成。</div>
        <svg v-else class="kbi-donut kbi-donut--wide" viewBox="0 0 480 160">
          <g v-for="s in communitySlices" :key="'c'+s.label"><title>{{ s.label }}:{{ s.value }}({{ s.pct }})</title>
            <path :d="s.path" :fill="s.color" /></g>
          <circle cx="80" cy="80" r="38" fill="var(--guaxi-panel-bg, #fff)" />
          <g v-for="(s,i) in communitySlices" :key="'cl'+s.label" :transform="`translate(170 ${16 + i*15})`">
            <rect x="0" y="-8" width="9" height="9" rx="2" :fill="s.color" />
            <text x="14" y="0" dominant-baseline="middle" font-size="11" :fill="labelFill">{{ s.label }} · {{ s.value }}</text>
          </g>
        </svg>
      </div>

      <!-- 领域知识量指数 -->
      <div class="kbi-chart kbi-full">
        <div class="kbi-chart__t">领域知识量指数(综合:节点数 / 平均连接度 / 概念数)</div>
        <div v-if="graphTruncated" class="kbi-empty-sm">图较大已截断,指数为近似值。</div>
        <div v-if="!domainRows.length" class="kbi-empty-sm">无领域数据(需要节点带标签)</div>
        <div v-for="r in domainRows" :key="'d'+r.label" class="kbi-bar">
          <span class="kbi-bar__l" :title="r.label">{{ r.label }}</span>
          <span class="kbi-bar__track"><span class="kbi-bar__fill" :style="{ width: r.pct + '%', background: r.color }" /></span>
          <span class="kbi-bar__v">{{ r.score }}</span>
        </div>
      </div>
    </div>
  </n-card>
</template>

<!-- SCRIPT_PLACEHOLDER -->

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'

const store = useKnowledgeBaseStore()

const kbOptions = ref([])
const selectedKb = ref(null)
const loading = ref(false)
const detecting = ref(false)
const notIndexed = ref(false)
const stats = ref({})
const communities = ref([])
const graphNodes = ref([])
const graphTruncated = ref(false)

const labelFill = 'var(--guaxi-text-color, #333)'
const TYPE_LABELS = { 'knowledge-base': '知识库', directory: '目录', file: '文件', section: '章节', paragraph: '段落', concept: '概念' }
const PALETTE = ['#3b82f6', '#22c55e', '#a855f7', '#f97316', '#06b6d4', '#f43f5e', '#84cc16', '#eab308', '#14b8a6', '#6366f1', '#ec4899', '#f59e0b']
const colorFor = (label) => {
  const s = String(label || '')
  let h = 0
  for (let i = 0; i < s.length; i += 1) h = (h * 31 + s.charCodeAt(i)) >>> 0
  return PALETTE[h % PALETTE.length]
}
const piePoint = (cx, cy, r, a) => ({ x: cx + r * Math.cos(a), y: cy + r * Math.sin(a) })
const pieSlicePath = (cx, cy, r, a0, a1) => {
  const delta = a1 - a0
  if (delta >= Math.PI * 2 - 1e-6) {
    const p0 = piePoint(cx, cy, r, a0)
    const pm = piePoint(cx, cy, r, a0 + Math.PI)
    return `M ${cx} ${cy} L ${p0.x.toFixed(2)} ${p0.y.toFixed(2)} A ${r} ${r} 0 1 1 ${pm.x.toFixed(2)} ${pm.y.toFixed(2)} A ${r} ${r} 0 1 1 ${p0.x.toFixed(2)} ${p0.y.toFixed(2)} Z`
  }
  const p0 = piePoint(cx, cy, r, a0)
  const p1 = piePoint(cx, cy, r, a1)
  const largeArc = delta > Math.PI ? 1 : 0
  return `M ${cx} ${cy} L ${p0.x.toFixed(2)} ${p0.y.toFixed(2)} A ${r} ${r} 0 ${largeArc} 1 ${p1.x.toFixed(2)} ${p1.y.toFixed(2)} Z`
}
const buildSlices = (entries, { cx = 70, cy = 75, r = 68, max = 8 } = {}) => {
  let arr = entries.filter((e) => Number(e[1]) > 0).sort((a, b) => b[1] - a[1])
  const total = arr.reduce((s, e) => s + Number(e[1]), 0)
  if (!total) return []
  const tail = arr.slice(max)
  let head = arr.slice(0, max)
  if (tail.length) head = head.concat([['其他', tail.reduce((s, e) => s + Number(e[1]), 0)]])
  let a = -Math.PI / 2
  return head.map(([label, value]) => {
    const frac = Number(value) / total
    const a1 = a + frac * Math.PI * 2
    const path = pieSlicePath(cx, cy, r, a, a1)
    a = a1
    return { label: String(label), value: Number(value), pct: (frac * 100).toFixed(1) + '%', color: colorFor(label), path }
  })
}

const tagSlices = computed(() => buildSlices((stats.value?.tags || []).map((t) => [t.key, t.count])))
const typeSlices = computed(() => buildSlices((stats.value?.node_types || []).map((t) => [TYPE_LABELS[t.key] || t.key, t.count])))
const conceptCount = computed(() => {
  const c = (stats.value?.node_types || []).find((x) => x.key === 'concept')
  return c ? c.count : 0
})
const edgeInfo = computed(() => {
  let sub = 0
  let assoc = 0
  for (const e of stats.value?.edge_types || []) {
    if (e.key === 'contains' || e.key === 'next') sub += e.count
    else assoc += e.count
  }
  return { sub, assoc }
})
const assocDensity = computed(() => {
  const n = stats.value?.total_nodes || 0
  return n ? (edgeInfo.value.assoc / n).toFixed(2) : '0'
})
const edgeRows = computed(() => {
  const { sub, assoc } = edgeInfo.value
  const max = Math.max(sub, assoc, 1)
  return [
    { label: '从属 (contains/next)', value: sub, color: '#3b82f6' },
    { label: '关联 (related/link/…)', value: assoc, color: '#a855f7' },
  ].filter((r) => r.value > 0).map((r) => ({ ...r, pct: (r.value / max * 100).toFixed(0) }))
})
const topNodeRows = computed(() => {
  const tn = stats.value?.top_nodes || []
  if (!tn.length) return []
  const max = Math.max(...tn.map((n) => Number(n.pagerank) || 0), 1e-9)
  return tn.slice(0, 8).map((n) => ({
    label: String(n.name || n.id || ''),
    type: TYPE_LABELS[n.type] || n.type || '',
    pct: ((Number(n.pagerank) || 0) / max * 100).toFixed(0),
    color: colorFor(n.name || n.id),
  }))
})
const communitySlices = computed(() => {
  const cs = (communities.value || []).filter((c) => Number(c.size) > 0).sort((a, b) => b.size - a.size).slice(0, 10)
  const total = cs.reduce((s, c) => s + Number(c.size || 0), 0)
  if (!total) return []
  let a = -Math.PI / 2
  return cs.map((c) => {
    const frac = Number(c.size) / total
    const a1 = a + frac * Math.PI * 2
    const path = pieSlicePath(80, 80, 74, a, a1)
    a = a1
    return { label: String(c.label || ('社区 ' + c.community_id)), value: c.size, pct: (frac * 100).toFixed(1) + '%', color: c.color || colorFor(c.label || c.community_id), path }
  })
})
const domainRows = computed(() => {
  const nodes = graphNodes.value || []
  if (!nodes.length) return []
  const map = new Map()
  for (const n of nodes) {
    const tags = Array.isArray(n.tags) ? n.tags : []
    for (const t of tags) {
      const k = String(t).trim()
      if (!k) continue
      const e = map.get(k) || { count: 0, degSum: 0, concepts: 0 }
      e.count += 1
      e.degSum += Number(n.degree) || 0
      if (n.type === 'concept') e.concepts += 1
      map.set(k, e)
    }
  }
  let rows = Array.from(map.entries()).map(([label, e]) => ({ label, count: e.count, avgDeg: e.count ? e.degSum / e.count : 0, concepts: e.concepts }))
  if (!rows.length) return []
  const maxC = Math.max(...rows.map((r) => r.count), 1)
  const maxD = Math.max(...rows.map((r) => r.avgDeg), 1e-9)
  const maxK = Math.max(...rows.map((r) => r.concepts), 1)
  rows = rows.map((r) => ({ ...r, scoreNum: 100 * (0.5 * (r.count / maxC) + 0.3 * (r.avgDeg / maxD) + 0.2 * (r.concepts / maxK)) }))
  rows.sort((a, b) => b.scoreNum - a.scoreNum)
  rows = rows.slice(0, 10)
  const maxS = Math.max(...rows.map((r) => r.scoreNum), 1)
  return rows.map((r) => ({ label: r.label, score: r.scoreNum.toFixed(0), pct: (r.scoreNum / maxS * 100).toFixed(0), color: colorFor(r.label) }))
})

const loadKb = async (name) => {
  const kb = String(name || selectedKb.value || '').trim()
  if (!kb) return
  selectedKb.value = kb
  loading.value = true
  notIndexed.value = false
  try {
    const [st, comm, graph] = await Promise.all([
      store.kgStats(kb, 20).catch(() => null),
      store.kgCommunities(kb).catch(() => []),
      store.kgQueryGraph(kb, { limit: 3000 }).catch(() => null),
    ])
    if (!st || !st.total_nodes) {
      notIndexed.value = true
      stats.value = {}
    } else {
      stats.value = st
    }
    communities.value = Array.isArray(comm) ? comm : []
    graphNodes.value = graph?.nodes || []
    graphTruncated.value = !!graph?.truncated
  } catch (e) {
    notIndexed.value = true
  } finally {
    loading.value = false
  }
}

const detectCommunities = async () => {
  if (!selectedKb.value || detecting.value) return
  detecting.value = true
  try {
    await store.kgDetectCommunities(selectedKb.value, 1.0)
    communities.value = await store.kgCommunities(selectedKb.value).catch(() => [])
  } catch (e) {
    /* ignore */
  } finally {
    detecting.value = false
  }
}

onMounted(async () => {
  try {
    await store.fetchKnowledgeBases()
    const kbs = store.knowledgeBases || []
    kbOptions.value = kbs.map((k) => ({ label: k.knowledge_base_name, value: k.knowledge_base_name }))
    if (kbOptions.value.length) {
      selectedKb.value = kbOptions.value[0].value
      await loadKb(selectedKb.value)
    }
  } catch (e) {
    /* ignore */
  }
})
</script>

<!-- STYLE_PLACEHOLDER -->

<style scoped>
.kbi-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; margin-bottom: 10px; }
.kbi-sub { font-size: 12px; opacity: 0.65; }
.kbi-actions { display: flex; align-items: center; gap: 8px; }
.kbi-empty, .kbi-empty-sm { opacity: 0.6; font-size: 13px; padding: 16px 0; text-align: center; }
.kbi-empty-sm { padding: 8px 0; font-size: 12px; }
.kbi-grid { display: flex; flex-direction: column; gap: 14px; }
.kbi-tiles { display: flex; gap: 10px; flex-wrap: wrap; }
.kbi-tile { flex: 1 1 90px; min-width: 90px; text-align: center; padding: 10px; border: 1px solid var(--guaxi-border-color, #e0e0e6); border-radius: 8px; }
.kbi-tile__v { font-size: 20px; font-weight: 700; }
.kbi-tile__l { font-size: 12px; opacity: 0.6; margin-top: 2px; }
.kbi-row { display: flex; gap: 14px; flex-wrap: wrap; }
.kbi-chart { flex: 1 1 300px; min-width: 280px; border: 1px solid var(--guaxi-border-color, #e0e0e6); border-radius: 8px; padding: 10px; }
.kbi-full { flex-basis: 100%; }
.kbi-chart__t { font-size: 13px; font-weight: 600; margin-bottom: 8px; }
.kbi-donut { width: 100%; max-width: 260px; height: auto; }
.kbi-donut--wide { max-width: 480px; }
.kbi-bar { display: flex; align-items: center; gap: 8px; margin: 4px 0; font-size: 12px; }
.kbi-bar__l { flex: 0 0 120px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.kbi-bar__track { flex: 1 1 auto; height: 12px; background: var(--guaxi-hover-bg, #f0f0f3); border-radius: 6px; overflow: hidden; }
.kbi-bar__fill { display: block; height: 100%; border-radius: 6px; }
.kbi-bar__v { flex: 0 0 auto; opacity: 0.7; min-width: 32px; text-align: right; }
</style>


