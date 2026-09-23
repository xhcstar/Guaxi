<template>
  <div class="rh-root">
    <div class="rh-header">
      <div class="rh-title">资源嗅探下载引擎</div>
      <div class="rh-sub">全网搜索 → 嗅探文件直链 → 多线程/断点续传/yt-dlp 下载 → 资源库管理</div>
      <n-button size="small" tertiary @click="showProxy = true">代理设置</n-button>
    </div>

    <div class="rh-body">
      <!-- 左：检索条件 -->
      <div class="rh-search" :style="{ width: searchWidth + 'px' }">
        <n-input v-model:value="keyword" placeholder="书名 / 论文关键词 / 方向…" @keyup.enter="doSearch" clearable />
        <div class="rh-block-title">资源类型</div>
        <n-checkbox-group v-model:value="kinds">
          <n-space vertical size="small">
            <n-checkbox value="paper" label="论文" />
            <n-checkbox value="book" label="书籍" />
            <n-checkbox value="video" label="视频" />
            <n-checkbox value="audio" label="音频" />
            <n-checkbox value="image" label="图片" />
          </n-space>
        </n-checkbox-group>
        <div class="rh-block-title">搜索引擎</div>
        <n-checkbox-group v-model:value="engines">
          <n-space vertical size="small">
            <n-checkbox value="duckduckgo" label="DuckDuckGo" />
            <n-checkbox value="bing" label="Bing" />
          </n-space>
        </n-checkbox-group>
        <div class="rh-block-title">来源</div>
        <n-space vertical size="small">
          <n-checkbox v-model:checked="webDiscovery" label="全网发现 + 嗅探" />
          <n-checkbox v-model:checked="seedSites" label="富集站(arXiv/OpenAlex)" />
        </n-space>
        <n-button type="primary" block :loading="searching" @click="doSearch" style="margin-top:14px">
          搜索
        </n-button>
        <div class="rh-resize" @mousedown="startResize"></div>
      </div>

      <!-- 中：结果 -->
      <div class="rh-results">
        <div class="rh-results-bar">
          <span>结果 {{ results.length }}</span>
          <n-button size="tiny" :disabled="!selected.length" @click="enqueueBatch">
            批量下载选中 ({{ selected.length }})
          </n-button>
        </div>
        <n-scrollbar class="rh-results-list">
          <div v-if="!results.length" class="rh-empty">输入关键词开始搜索</div>
          <div v-for="item in results" :key="item.id" class="rh-card">
            <n-checkbox :checked="selected.includes(item.id)" @update:checked="v => toggleSel(item.id, v)" />
            <div class="rh-card-main">
              <div class="rh-card-title">
                <span class="rh-badge" :class="'k-' + item.kind">{{ kindLabel(item.kind) }}</span>
                {{ item.title }}
              </div>
              <div class="rh-card-meta">
                {{ item.source }} · 相关度 {{ (item.relevance * 100).toFixed(0) }}%
                <span v-if="item.needs_ytdlp"> · yt-dlp 解析</span>
                <span v-if="item.magnet"> · 磁力</span>
              </div>
            </div>
            <div class="rh-card-actions">
              <n-button size="tiny" @click="openOnline(item)">在线看</n-button>
              <n-button size="tiny" type="primary" @click="enqueue(item)">下载</n-button>
            </div>
          </div>
        </n-scrollbar>
      </div>

      <!-- 右：下载队列 + 资源库 -->
      <div class="rh-side">
        <n-tabs type="segment" size="small" v-model:value="sideTab">
          <n-tab-pane name="tasks" tab="下载队列">
            <n-scrollbar class="rh-side-list">
              <div v-if="!taskList.length" class="rh-empty">暂无任务</div>
              <div v-for="t in taskList" :key="t.id" class="rh-task">
                <div class="rh-task-title">{{ t.title }}</div>
                <n-progress type="line" :percentage="pct(t)" :height="6" :show-indicator="false" />
                <div class="rh-task-meta">
                  <span>{{ statusLabel(t.status) }} · {{ fmtSpeed(t.speed) }}</span>
                  <span class="rh-task-btns">
                    <n-button v-if="t.status==='active'" size="tiny" @click="pause(t.id)">暂停</n-button>
                    <n-button v-else-if="t.status==='paused'||t.status==='error'" size="tiny" @click="resume(t.id)">继续</n-button>
                    <n-button size="tiny" @click="cancel(t.id)">取消</n-button>
                    <n-button v-if="t.status==='completed'" size="tiny" type="primary" @click="openPath(t.dest)">打开</n-button>
                  </span>
                </div>
              </div>
            </n-scrollbar>
          </n-tab-pane>
          <n-tab-pane name="library" tab="资源库">
            <n-scrollbar class="rh-side-list">
              <div v-if="!resources.length" class="rh-empty">还没有下载过资源</div>
              <div v-for="r in resources" :key="r.id" class="rh-task" @click="openPath(r.local_path)">
                <div class="rh-task-title">{{ r.title }}</div>
                <div class="rh-task-meta">{{ kindLabel(r.kind) }} · {{ r.source }}</div>
              </div>
            </n-scrollbar>
          </n-tab-pane>
        </n-tabs>
      </div>
    </div>

    <!-- 代理设置 -->
    <n-modal v-model:show="showProxy" preset="card" title="代理设置" style="width:460px">
      <n-space vertical>
        <n-radio-group v-model:value="proxy.mode">
          <n-radio value="none">不使用</n-radio>
          <n-radio value="system">跟随系统</n-radio>
          <n-radio value="custom">自定义</n-radio>
        </n-radio-group>
        <n-input v-if="proxy.mode==='custom'" v-model:value="proxy.url"
          placeholder="http://127.0.0.1:7890 或 socks5://127.0.0.1:1080" />
        <n-checkbox v-model:checked="proxy.proxy_foreign_only" label="仅境外域名走代理（境内直连）" />
        <div class="rh-block-title">直连白名单（每行一个域名后缀）</div>
        <n-input v-model:value="bypassText" type="textarea" :rows="4" placeholder=".cn&#10;bilibili.com" />
        <n-button type="primary" @click="saveProxy">保存</n-button>
      </n-space>
    </n-modal>
  </div>
</template>

<!-- PLACEHOLDER_SCRIPT -->

<script setup>
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { createDiscreteApi } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { createLogger } from '../../../utils/logger'

const logger = createLogger('ResourceHunterApp')
const { message } = createDiscreteApi(['message'])

const keyword = ref('')
const kinds = ref(['paper'])
const engines = ref(['duckduckgo', 'bing'])
const webDiscovery = ref(true)
const seedSites = ref(true)
const searching = ref(false)
const results = ref([])
const selected = ref([])
const sideTab = ref('tasks')
const tasks = ref({})
const resources = ref([])
const showProxy = ref(false)
const proxy = ref({ mode: 'system', url: '', bypass: [], proxy_foreign_only: false })
const bypassText = ref('')
const searchWidth = ref(240)

function startResize(e) {
  const startX = e.clientX
  const startW = searchWidth.value
  const onMove = (ev) => {
    searchWidth.value = Math.min(480, Math.max(180, startW + (ev.clientX - startX)))
  }
  const onUp = () => {
    document.removeEventListener('mousemove', onMove)
    document.removeEventListener('mouseup', onUp)
  }
  document.addEventListener('mousemove', onMove)
  document.addEventListener('mouseup', onUp)
}

let unlisten = null

const taskList = computed(() =>
  Object.values(tasks.value).sort((a, b) => (b.created_at || 0) - (a.created_at || 0)),
)

const KIND_LABEL = { paper: '论文', book: '书籍', video: '视频', audio: '音频', image: '图片', other: '其他' }
const kindLabel = (k) => KIND_LABEL[k] || k
const statusLabel = (s) =>
  ({ queued: '排队', active: '下载中', paused: '已暂停', completed: '完成', error: '出错' }[s] || s)

function pct(t) {
  if (!t.total_size) return t.status === 'completed' ? 100 : 0
  return Math.min(100, Math.round((t.downloaded / t.total_size) * 100))
}
function fmtSpeed(b) {
  if (!b) return '—'
  if (b > 1048576) return (b / 1048576).toFixed(1) + ' MB/s'
  if (b > 1024) return (b / 1024).toFixed(0) + ' KB/s'
  return b + ' B/s'
}
function toggleSel(id, v) {
  if (v) selected.value = [...selected.value, id]
  else selected.value = selected.value.filter((x) => x !== id)
}
// PLACEHOLDER_SCRIPT2

async function doSearch() {
  const kw = keyword.value.trim()
  if (!kw) return
  searching.value = true
  results.value = []
  selected.value = []
  try {
    const items = await invoke('rh_search', {
      query: {
        keyword: kw,
        kinds: kinds.value,
        engines: engines.value,
        web_discovery: webDiscovery.value,
        seed_sites: seedSites.value,
        limit: 40,
      },
    })
    results.value = items || []
    if (!results.value.length) message.info('没有找到结果，换个关键词或来源试试')
  } catch (e) {
    logger.error('搜索失败', e)
    message.error('搜索失败: ' + e)
  } finally {
    searching.value = false
  }
}

async function enqueue(item) {
  try {
    await invoke('rh_enqueue', { item })
    sideTab.value = 'tasks'
    await refreshTasks()
    message.success('已加入下载队列')
  } catch (e) {
    message.error('加入下载失败: ' + e)
  }
}

async function enqueueBatch() {
  const items = results.value.filter((r) => selected.value.includes(r.id))
  if (!items.length) return
  try {
    const ids = await invoke('rh_enqueue_batch', { items })
    sideTab.value = 'tasks'
    await refreshTasks()
    message.success(`已加入 ${ids.length} 个下载任务`)
    selected.value = []
  } catch (e) {
    message.error('批量下载失败: ' + e)
  }
}
// PLACEHOLDER_SCRIPT3

async function refreshTasks() {
  try {
    const list = await invoke('rh_list_tasks')
    const map = {}
    for (const t of list || []) map[t.id] = t
    tasks.value = map
  } catch (e) {
    logger.error('刷新任务失败', e)
  }
}

async function pause(id) {
  try { await invoke('rh_pause', { id }) } catch (e) { message.error('' + e) }
}
async function resume(id) {
  try { await invoke('rh_resume', { id }); await refreshTasks() } catch (e) { message.error('' + e) }
}
async function cancel(id) {
  try {
    await invoke('rh_cancel', { id })
    const m = { ...tasks.value }; delete m[id]; tasks.value = m
  } catch (e) { message.error('' + e) }
}

async function openOnline(item) {
  const url = item.source_url || item.download_url
  if (!url) return
  try { await invoke('rh_open_path', { path: url }) } catch (e) { message.error('打开失败: ' + e) }
}
async function openPath(path) {
  if (!path) return
  try { await invoke('rh_open_path', { path }) } catch (e) { message.error('打开失败: ' + e) }
}

async function loadResources() {
  try { resources.value = (await invoke('rh_list_resources', { kind: null })) || [] } catch (e) { logger.error(e) }
}

async function loadProxy() {
  try {
    const cfg = await invoke('rh_get_proxy')
    if (cfg) { proxy.value = cfg; bypassText.value = (cfg.bypass || []).join('\n') }
  } catch (e) { logger.error('读取代理失败', e) }
}
async function saveProxy() {
  proxy.value.bypass = bypassText.value.split('\n').map((s) => s.trim()).filter(Boolean)
  try {
    await invoke('rh_set_proxy', { config: proxy.value })
    showProxy.value = false
    message.success('代理已保存')
  } catch (e) { message.error('保存失败: ' + e) }
}
// PLACEHOLDER_SCRIPT4

onMounted(async () => {
  await loadProxy()
  await refreshTasks()
  await loadResources()
  unlisten = await listen('resource_hunter://download_update', (evt) => {
    const p = evt.payload
    if (!p) return
    if (p.removed) {
      const m = { ...tasks.value }; delete m[p.id]; tasks.value = m
      return
    }
    tasks.value = { ...tasks.value, [p.id]: p }
    if (p.status === 'completed') loadResources()
  })
})

onUnmounted(() => {
  if (unlisten) unlisten()
})
</script>

<!-- PLACEHOLDER_STYLE -->

<style scoped>
.rh-root {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--guaxi-content-bg, #ffffff);
  color: var(--guaxi-text-primary, #1f2328);
  font-size: 14px;
}
.rh-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 16px;
  border-bottom: 1px solid var(--guaxi-border, #d0d7de);
}
.rh-title { font-size: 16px; font-weight: 600; color: var(--guaxi-text-primary); }
.rh-sub { font-size: 13px; color: var(--guaxi-text-secondary); flex: 1; }
.rh-body { flex: 1; display: flex; min-height: 0; }
.rh-search {
  width: 240px;
  flex-shrink: 0;
  position: relative;
  padding: 12px;
  border-right: 1px solid var(--guaxi-border, #d0d7de);
  overflow-y: auto;
}
.rh-resize {
  position: absolute;
  top: 0; right: 0;
  width: 5px; height: 100%;
  cursor: ew-resize;
  background: transparent;
}
.rh-resize:hover { background: var(--guaxi-border, #d0d7de); }
.rh-block-title { font-size: 13px; color: var(--guaxi-text-secondary); margin: 14px 0 6px; }
.rh-results { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.rh-results-bar {
  display: flex; justify-content: space-between; align-items: center;
  padding: 10px 14px; border-bottom: 1px solid var(--guaxi-border, #d0d7de); font-size: 14px;
}
.rh-results-list { flex: 1; }
.rh-card {
  display: flex; align-items: center; gap: 10px;
  padding: 11px 14px; border-bottom: 1px solid var(--guaxi-border, #eee);
}
.rh-card-main { flex: 1; min-width: 0; }
.rh-card-title { font-size: 14px; line-height: 1.45; display: flex; align-items: center; gap: 6px; color: var(--guaxi-text-primary); }
.rh-card-meta { font-size: 12px; color: var(--guaxi-text-secondary); margin-top: 4px; }
.rh-card-actions { display: flex; gap: 6px; }
.rh-badge {
  font-size: 10px; padding: 1px 6px; border-radius: 4px; color: #fff; flex-shrink: 0;
}
.k-paper { background: #3b82f6; }
.k-book { background: #8b5cf6; }
.k-video { background: #ef4444; }
.k-audio { background: #f59e0b; }
.k-image { background: #10b981; }
.k-other { background: #6b7280; }
.rh-side { width: 320px; flex-shrink: 0; border-left: 1px solid var(--guaxi-border, #d0d7de); padding: 8px; display: flex; flex-direction: column; }
.rh-side-list { height: calc(100vh - 110px); }
.rh-task { padding: 9px 6px; border-bottom: 1px solid var(--guaxi-border, #eee); }
.rh-task-title { font-size: 13px; margin-bottom: 4px; color: var(--guaxi-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rh-task-meta { display: flex; justify-content: space-between; align-items: center; font-size: 12px; color: var(--guaxi-text-secondary); margin-top: 4px; }
.rh-task-btns { display: flex; gap: 4px; }
.rh-empty { text-align: center; color: var(--guaxi-text-secondary); font-size: 13px; padding: 30px 0; }
</style>
