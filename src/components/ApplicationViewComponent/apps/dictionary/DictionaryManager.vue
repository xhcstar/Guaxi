<template>
  <n-modal :show="show" :mask-closable="false" @update:show="onShowChange">
    <div class="dm-shell">
      <header class="dm-head">
        <div class="dm-head-main">
          <div class="dm-eyebrow">GUAXI · LEXICON</div>
          <h2 class="dm-title">词典库</h2>
          <p class="dm-sub">
            已装 <b>{{ packs.length }}</b> 本 · 启用 <b>{{ enabledCount }}</b> 本 ·
            词条 <b>{{ totalEntries.toLocaleString() }}</b> 条
          </p>
        </div>
        <div class="dm-head-side">
          <nav class="dm-tabs">
            <button
              v-for="tab in tabs"
              :key="tab.key"
              type="button"
              class="dm-tab"
              :class="{ active: activeTab === tab.key }"
              @click="activeTab = tab.key"
            >
              {{ tab.label }}
            </button>
          </nav>
          <button type="button" class="dm-close" title="关闭" @click="onShowChange(false)">
            <Icon :icon="xIcon" width="16" height="16" />
          </button>
        </div>
      </header>

      <div class="dm-body">
        <!-- 已安装 -->
        <section v-if="activeTab === 'installed'" class="dm-pane">
          <div v-if="!packs.length" class="dm-empty">
            <Icon :icon="libraryIcon" width="34" height="34" />
            <p>还没有安装任何离线词库</p>
            <span>去「在线词库」下载，或到「导入」加载你已有的 MDX / StarDict / DSL 词典。</span>
          </div>

          <div v-else class="dm-pack-list">
            <div v-for="(pack, index) in orderedPacks" :key="pack.id" class="dm-pack">
              <div class="dm-pack-order">
                <button type="button" class="dm-mini" title="提高优先级" :disabled="index === 0" @click="movePack(index, -1)">
                  <Icon :icon="arrowUpIcon" width="14" height="14" />
                </button>
                <span class="dm-rank">{{ index + 1 }}</span>
                <button
                  type="button"
                  class="dm-mini"
                  title="降低优先级"
                  :disabled="index === orderedPacks.length - 1"
                  @click="movePack(index, 1)"
                >
                  <Icon :icon="arrowDownIcon" width="14" height="14" />
                </button>
              </div>

              <div class="dm-pack-main">
                <div class="dm-pack-title">
                  <span class="dm-pack-name">{{ pack.name }}</span>
                  <span class="dm-badge">{{ formatLabel(pack.format) }}</span>
                  <span v-if="pack.builtin" class="dm-badge is-quiet">内置目录</span>
                  <span v-if="pack.has_resources" class="dm-badge is-quiet">含图片</span>
                </div>
                <div class="dm-pack-meta">
                  <span>{{ langLabel(pack.source_lang) }} → {{ langLabel(pack.target_lang) }}</span>
                  <span class="dm-dot">·</span>
                  <span>{{ Number(pack.entry_count || 0).toLocaleString() }} 词条</span>
                  <span v-if="pack.resource_bytes" class="dm-dot">·</span>
                  <span v-if="pack.resource_bytes">资源 {{ formatBytes(pack.resource_bytes) }}</span>
                  <span class="dm-dot">·</span>
                  <span>装于 {{ shortDate(pack.installed_at) }}</span>
                </div>
              </div>

              <div class="dm-pack-actions">
                <n-switch
                  size="small"
                  :value="pack.enabled !== false"
                  :loading="enablingId === pack.id"
                  @update:value="(value) => togglePack(pack, value)"
                />
                <button type="button" class="dm-btn" @click="exportPack(pack)">
                  <Icon :icon="downloadIcon" width="14" height="14" />
                  <span>导出</span>
                </button>
                <button type="button" class="dm-btn is-danger" @click="removePack(pack)">
                  <Icon :icon="trashIcon" width="14" height="14" />
                  <span>删除</span>
                </button>
              </div>
            </div>
          </div>

          <p class="dm-hint">
            查词时按这里的顺序逐本命中；关掉开关的词库不参与查词，但文件仍保留在本机。
          </p>
        </section>

        <!-- 在线词库 -->
        <section v-else-if="activeTab === 'catalog'" class="dm-pane">
          <div class="dm-catalog-grid">
            <article v-for="item in catalog" :key="item.id" class="dm-catalog">
              <div class="dm-catalog-head">
                <h3>{{ item.name }}</h3>
                <span class="dm-badge">{{ langLabel(item.source_lang) }} → {{ langLabel(item.target_lang) }}</span>
              </div>
              <p class="dm-catalog-desc">{{ item.description }}</p>
              <div class="dm-catalog-meta">
                <span>{{ formatBytes(item.size_bytes) }}</span>
                <span class="dm-dot">·</span>
                <span>{{ item.license }}</span>
                <span v-if="item.installed" class="dm-dot">·</span>
                <span v-if="item.installed" class="dm-installed-mark">
                  <Icon :icon="checkIcon" width="13" height="13" /> 已安装
                </span>
              </div>
              <div v-if="jobFor(item.id)" class="dm-job">
                <n-progress
                  type="line"
                  :percentage="Math.round(jobFor(item.id).progress || 0)"
                  :height="4"
                  :show-indicator="false"
                  :status="jobFor(item.id).stage === 'error' ? 'error' : 'default'"
                />
                <span class="dm-job-text">{{ jobFor(item.id).message || jobFor(item.id).stage }}</span>
              </div>
              <div class="dm-catalog-actions">
                <button
                  type="button"
                  class="dm-btn is-primary"
                  :disabled="busyCatalogId === item.id || isJobRunning(item.id)"
                  @click="installPack(item)"
                >
                  <Icon :icon="downloadIcon" width="14" height="14" />
                  <span>{{ item.installed ? '重新下载' : '下载安装' }}</span>
                </button>
              </div>
            </article>
          </div>
          <p class="dm-hint">
            目录里的词库来自 ECDICT、CC-CEDICT 与 Wiktionary 的开放数据，下载后完全离线可用。
          </p>
        </section>

        <!-- 导入 -->
        <section v-else class="dm-pane">
          <div class="dm-import">
            <button type="button" class="dm-drop" @click="pickFiles">
              <Icon :icon="folderIcon" width="30" height="30" />
              <strong>{{ importPaths.length ? `${importPaths.length} 个文件已选择` : '选择或拖入词典文件' }}</strong>
              <span>MDX/MDD · StarDict(.ifo+.idx+.dict) · DSL · JSONL · ZIP · CSV · JSON</span>
            </button>
            <ul v-if="importPaths.length" class="dm-file-list">
              <li v-for="path in importPaths" :key="path">{{ baseName(path) }}</li>
            </ul>

            <div class="dm-form">
              <label class="dm-field">
                <span>词典名称（留空则用文件名）</span>
                <input v-model="importName" type="text" placeholder="例如：牛津高阶英汉双解" />
              </label>
              <div class="dm-field-row">
                <label class="dm-field">
                  <span>源语言</span>
                  <n-select v-model:value="importFrom" size="small" :options="langOptions" filterable />
                </label>
                <label class="dm-field">
                  <span>目标语言</span>
                  <n-select v-model:value="importTo" size="small" :options="langOptions" filterable />
                </label>
              </div>
            </div>

            <div v-if="importJob" class="dm-job">
              <n-progress
                type="line"
                :percentage="Math.round(importJob.progress || 0)"
                :height="4"
                :show-indicator="false"
                :status="importJob.stage === 'error' ? 'error' : 'default'"
              />
              <span class="dm-job-text">{{ importJob.message || importJob.stage }}</span>
            </div>

            <div class="dm-import-actions">
              <button type="button" class="dm-btn is-primary" :disabled="importing || !importPaths.length" @click="doImport">
                <Icon :icon="importing ? loaderIcon : uploadIcon" :class="{ spin: importing }" width="15" height="15" />
                <span>{{ importing ? '正在导入…' : '开始导入' }}</span>
              </button>
              <button v-if="importPaths.length" type="button" class="dm-btn" :disabled="importing" @click="resetImport">
                清空
              </button>
            </div>

            <div class="dm-note">
              <h4>导入说明</h4>
              <p>· <b>MDX/MDD</b>：一次选中同名 .mdx 与 .mdd，图片与样式会一起解包；MDD 也可单独后补。</p>
              <p>· <b>StarDict</b>：选中 .ifo，同目录的 .idx / .dict(.dz) 会自动配对。</p>
              <p>· <b>DSL</b>：ABBYY Lingvo / 灵格斯导出的 .dsl，UTF-16 与 UTF-8 都能识别。</p>
              <p>· 大词库导入需要几十秒到几分钟，进度会实时刷新。</p>
            </div>
          </div>
        </section>
      </div>
    </div>
  </n-modal>

  <!-- 自带的确认框：应用没有 <n-dialog-provider>，useDialog() 会直接抛错，
       这里用 preset=dialog 的 modal 顶替，主题仍然走外层 config-provider。 -->
  <n-modal
    v-model:show="confirmState.show"
    preset="dialog"
    type="warning"
    :title="confirmState.title"
    :content="confirmState.content"
    :positive-text="confirmState.confirmLabel"
    negative-text="取消"
    @positive-click="settleConfirm(true)"
    @negative-click="settleConfirm(false)"
  />
</template>

<script setup>
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import { useMessage } from 'naive-ui'
import { Icon } from '@iconify/vue'
import xIcon from '@iconify-icons/lucide/x'
import downloadIcon from '@iconify-icons/lucide/download'
import trashIcon from '@iconify-icons/lucide/trash-2'
import folderIcon from '@iconify-icons/lucide/folder-open'
import arrowUpIcon from '@iconify-icons/lucide/arrow-up'
import arrowDownIcon from '@iconify-icons/lucide/arrow-down'
import checkIcon from '@iconify-icons/lucide/check'
import libraryIcon from '@iconify-icons/lucide/library'
import uploadIcon from '@iconify-icons/lucide/upload'
import loaderIcon from '@iconify-icons/lucide/loader-2'

const props = defineProps({
  show: { type: Boolean, default: false },
})
const emit = defineEmits(['update:show', 'changed'])

const message = useMessage()

// 应用根节点只有 n-message-provider，没有 n-dialog-provider，
// 所以用自带的 n-modal 做确认框，而不是 useDialog()。
const confirmState = ref({
  show: false,
  title: '',
  content: '',
  confirmLabel: '确定',
})
let confirmResolve = null

const askConfirm = ({ title, content, confirmLabel = '确定' }) =>
  new Promise((resolve) => {
    // 上一个确认尚未返回时先当作取消，避免 Promise 悬挂。
    if (confirmResolve) settleConfirm(false)
    confirmState.value = { show: true, title, content, confirmLabel }
    confirmResolve = resolve
  })

function settleConfirm(result) {
  const resolve = confirmResolve
  confirmResolve = null
  confirmState.value.show = false
  if (typeof resolve === 'function') resolve(result)
}

const tabs = [
  { key: 'installed', label: '已安装' },
  { key: 'catalog', label: '在线词库' },
  { key: 'import', label: '导入' },
]
const activeTab = ref('installed')

const packs = ref([])
const catalog = ref([])
const jobs = ref({})
const enablingId = ref('')
const busyCatalogId = ref('')

const importPaths = ref([])
const importName = ref('')
const importFrom = ref('en')
const importTo = ref('zh')
const importing = ref(false)
const importJobId = ref('')

let pollTimer = 0

const langOptions = [
  { label: '中文', value: 'zh' },
  { label: '英文', value: 'en' },
  { label: '日文', value: 'ja' },
  { label: '韩文', value: 'ko' },
  { label: '法文', value: 'fr' },
  { label: '德文', value: 'de' },
  { label: '西班牙语', value: 'es' },
  { label: '俄语', value: 'ru' },
  { label: '葡萄牙语', value: 'pt' },
  { label: '意大利语', value: 'it' },
  { label: '荷兰语', value: 'nl' },
  { label: '越南语', value: 'vi' },
  { label: '泰语', value: 'th' },
]

const langNames = Object.fromEntries(langOptions.map((item) => [item.value, item.label]))

const orderedPacks = computed(() => {
  return [...packs.value].sort((a, b) => {
    const diff = Number(b.priority || 0) - Number(a.priority || 0)
    if (diff !== 0) return diff
    return String(a.name || '').localeCompare(String(b.name || ''))
  })
})
const enabledCount = computed(() => packs.value.filter((pack) => pack.enabled !== false).length)
const totalEntries = computed(() => packs.value.reduce((sum, pack) => sum + Number(pack.entry_count || 0), 0))
const importJob = computed(() => (importJobId.value ? jobs.value[importJobId.value] : null))

const formatLabel = (format) => {
  const map = {
    mdx: 'MDict',
    stardict: 'StarDict',
    dsl: 'DSL',
    wiktionary: 'Wiktionary',
    ecdict_csv: 'ECDICT',
    cedict_zip: 'CC-CEDICT',
    zip: 'ZIP 包',
    csv: 'CSV',
    json: 'JSON',
  }
  return map[String(format || '')] || '词库'
}
const langLabel = (code) => langNames[String(code || '').toLowerCase()] || String(code || '—')
const formatBytes = (bytes) => {
  const value = Number(bytes || 0)
  if (!value) return '0 B'
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`
  if (value < 1024 * 1024 * 1024) return `${(value / 1024 / 1024).toFixed(1)} MB`
  return `${(value / 1024 / 1024 / 1024).toFixed(2)} GB`
}
const shortDate = (iso) => {
  const text = String(iso || '')
  return text ? text.slice(0, 10) : '—'
}
const baseName = (path) => String(path || '').split(/[\\/]/).pop()
const jobFor = (id) => jobs.value[String(id || '')] || null
const isJobRunning = (id) => {
  const job = jobFor(id)
  return Boolean(job && job.stage && !['done', 'error'].includes(job.stage))
}

const onShowChange = (value) => emit('update:show', value)

const refresh = async () => {
  try {
    const [packList, catalogList, jobList] = await Promise.all([
      invoke('language_learning_dict_list'),
      invoke('language_learning_dict_catalog'),
      invoke('language_learning_dict_jobs'),
    ])
    packs.value = Array.isArray(packList) ? packList : []
    catalog.value = Array.isArray(catalogList) ? catalogList : []
    const map = {}
    ;(Array.isArray(jobList) ? jobList : []).forEach((job) => {
      if (job?.id) map[job.id] = job
    })
    jobs.value = map
  } catch (err) {
    message.error(`读取词典库失败：${String(err?.message || err || '')}`)
  }
}

const refreshJobs = async () => {
  try {
    const jobList = await invoke('language_learning_dict_jobs')
    const map = {}
    ;(Array.isArray(jobList) ? jobList : []).forEach((job) => {
      if (job?.id) map[job.id] = job
    })
    jobs.value = map
    const running = Object.values(map).some((job) => job?.stage && !['done', 'error'].includes(job.stage))
    if (!running) {
      stopPolling()
      await refresh()
      emit('changed')
    }
  } catch {}
}

const ensurePolling = () => {
  if (pollTimer) return
  pollTimer = window.setInterval(refreshJobs, 900)
}
const stopPolling = () => {
  if (!pollTimer) return
  window.clearInterval(pollTimer)
  pollTimer = 0
}

watch(
  () => props.show,
  (visible) => {
    if (visible) {
      refresh()
      ensurePolling()
    } else {
      stopPolling()
      // 管理器被关掉时，别让确认框的 Promise 悬着。
      settleConfirm(false)
    }
  },
  { immediate: true },
)

// ESC / 点遮罩关闭确认框时，modal 只把 show 置 false，这里补上 Promise 的落定。
watch(
  () => confirmState.value.show,
  (visible) => {
    if (!visible) settleConfirm(false)
  },
)

onBeforeUnmount(stopPolling)

const installPack = async (item) => {
  busyCatalogId.value = item.id
  try {
    await invoke('language_learning_dict_install', { id: item.id })
    message.info(`已开始下载「${item.name}」`)
    ensurePolling()
  } catch (err) {
    message.error(`启动下载失败：${String(err?.message || err || '')}`)
  } finally {
    busyCatalogId.value = ''
  }
}

const togglePack = async (pack, value) => {
  enablingId.value = pack.id
  try {
    await invoke('dict_set_enabled', { id: pack.id, enabled: Boolean(value) })
    await refresh()
    emit('changed')
  } catch (err) {
    message.error(`切换词典状态失败：${String(err?.message || err || '')}`)
  } finally {
    enablingId.value = ''
  }
}

const movePack = async (index, delta) => {
  const list = [...orderedPacks.value]
  const target = index + delta
  if (target < 0 || target >= list.length) return
  const [item] = list.splice(index, 1)
  list.splice(target, 0, item)
  const snapshot = packs.value
  packs.value = list
  try {
    await invoke('dict_reorder', { ids: list.map((pack) => pack.id) })
    await refresh()
    emit('changed')
  } catch (err) {
    packs.value = snapshot
    message.error(`调整顺序失败：${String(err?.message || err || '')}`)
  }
}

const removePack = async (pack) => {
  const ok = await askConfirm({
    title: '删除离线词典',
    content: `将删除「${pack.name}」及其本地索引${pack.has_resources ? '与已解包的图片资源' : ''}，之后查词会回退到其它词库。`,
    confirmLabel: '删除',
  })
  if (!ok) return
  try {
    await invoke('language_learning_dict_remove', { id: pack.id })
    message.success('词典已删除')
    await refresh()
    emit('changed')
  } catch (err) {
    message.error(`删除失败：${String(err?.message || err || '')}`)
  }
}

const exportPack = async (pack) => {
  try {
    const target = await save({
      defaultPath: `${pack.id}-dict.zip`,
      filters: [{ name: 'ZIP', extensions: ['zip'] }],
    })
    if (!target) return
    await invoke('language_learning_dict_export', { id: pack.id, outputPath: String(target) })
    message.success(`已导出「${pack.name}」`)
  } catch (err) {
    message.error(`导出失败：${String(err?.message || err || '')}`)
  }
}

const pickFiles = async () => {
  try {
    const picked = await open({
      multiple: true,
      filters: [
        { name: '词典文件', extensions: ['mdx', 'mdd', 'ifo', 'idx', 'dict', 'dz', 'dsl', 'jsonl', 'zip', 'csv', 'json'] },
        { name: '全部文件', extensions: ['*'] },
      ],
    })
    if (!picked) return
    const list = Array.isArray(picked) ? picked : [picked]
    importPaths.value = list.map(String)
    const first = baseName(list[0] || '')
    if (!importName.value && first) {
      importName.value = first.replace(/\.[^.]+$/, '')
    }
  } catch (err) {
    message.error(`选择文件失败：${String(err?.message || err || '')}`)
  }
}

const resetImport = () => {
  importPaths.value = []
  importName.value = ''
  importJobId.value = ''
}

const doImport = async () => {
  if (importing.value || !importPaths.value.length) return
  importing.value = true
  const jobId = `import-${Date.now()}`
  importJobId.value = jobId
  ensurePolling()
  try {
    const manifest = await invoke('language_learning_dict_import_files', {
      paths: importPaths.value,
      name: String(importName.value || '').trim(),
      sourceLang: importFrom.value,
      targetLang: importTo.value,
      jobId,
    })
    message.success(`已导入「${manifest?.name || '词典'}」（${Number(manifest?.entry_count || 0).toLocaleString()} 条）`)
    resetImport()
    await refresh()
    emit('changed')
  } catch (err) {
    message.error(`导入失败：${String(err?.message || err || '')}`)
  } finally {
    importing.value = false
  }
}
</script>

<style scoped>
.dm-shell {
  --dm-paper: var(--guaxi-panel-bg, #ffffff);
  --dm-ink: var(--guaxi-text-primary, #1f2328);
  --dm-muted: var(--guaxi-text-secondary, #57606a);
  --dm-rule: var(--guaxi-border, #d0d7de);
  --dm-accent: color-mix(in srgb, #2f7d6d 82%, var(--dm-ink));
  --dm-accent-soft: color-mix(in srgb, var(--dm-accent) 12%, transparent);
  width: min(980px, 94vw);
  height: min(78vh, 760px);
  display: flex;
  flex-direction: column;
  background: var(--dm-paper);
  color: var(--dm-ink);
  border: 1px solid var(--dm-rule);
  border-radius: 18px;
  overflow: hidden;
  box-shadow: 0 26px 70px rgba(15, 23, 42, 0.28);
  font-family: var(--guaxi-editor-font-family, 'Maple Mono NF CN', Consolas, monospace);
}

.dm-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
  padding: 18px 20px 12px;
  border-bottom: 1px solid var(--dm-rule);
  background: linear-gradient(180deg, var(--dm-accent-soft), transparent 92%);
}

.dm-eyebrow {
  font-size: 10px;
  letter-spacing: 0.22em;
  color: var(--dm-muted);
}

.dm-title {
  margin: 2px 0 0;
  font-size: 24px;
  font-weight: 700;
  letter-spacing: 0.06em;
  font-family: 'Iowan Old Style', 'Palatino Linotype', Palatino, Georgia, 'Songti SC', 'Noto Serif SC', serif;
}

.dm-sub {
  margin: 4px 0 0;
  font-size: 12px;
  color: var(--dm-muted);
}
.dm-sub b {
  color: var(--dm-ink);
  font-weight: 600;
}

.dm-head-side {
  display: flex;
  align-items: center;
  gap: 10px;
}

.dm-tabs {
  display: flex;
  gap: 2px;
  padding: 3px;
  border: 1px solid var(--dm-rule);
  border-radius: 999px;
  background: color-mix(in srgb, var(--dm-paper) 70%, transparent);
}

.dm-tab {
  border: 0;
  background: transparent;
  color: var(--dm-muted);
  font-size: 12px;
  padding: 5px 13px;
  border-radius: 999px;
  cursor: pointer;
  transition: color 0.15s ease, background 0.15s ease;
}
.dm-tab:hover {
  color: var(--dm-ink);
}
.dm-tab.active {
  background: var(--dm-accent);
  color: #fff;
}

.dm-close {
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  border: 1px solid var(--dm-rule);
  border-radius: 50%;
  background: transparent;
  color: var(--dm-muted);
  cursor: pointer;
}
.dm-close:hover {
  color: var(--dm-ink);
  background: var(--dm-accent-soft);
}

.dm-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 16px 20px 22px;
}

.dm-pane {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.dm-empty {
  padding: 48px 20px;
  text-align: center;
  color: var(--dm-muted);
  border: 1px dashed var(--dm-rule);
  border-radius: 16px;
}
.dm-empty p {
  margin: 10px 0 4px;
  color: var(--dm-ink);
  font-size: 15px;
}
.dm-empty span {
  font-size: 12px;
}

.dm-pack-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.dm-pack {
  display: grid;
  grid-template-columns: 42px minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  border: 1px solid var(--dm-rule);
  border-radius: 12px;
  background: color-mix(in srgb, var(--dm-paper) 88%, var(--dm-accent) 4%);
}

.dm-pack-order {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1px;
  color: var(--dm-muted);
}

.dm-rank {
  font-size: 11px;
  font-weight: 700;
  color: var(--dm-accent);
}

.dm-mini {
  border: 0;
  background: transparent;
  color: var(--dm-muted);
  cursor: pointer;
  padding: 0;
  line-height: 0;
}
.dm-mini:disabled {
  opacity: 0.25;
  cursor: default;
}
.dm-mini:not(:disabled):hover {
  color: var(--dm-accent);
}

.dm-pack-main {
  min-width: 0;
}

.dm-pack-title {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.dm-pack-name {
  font-size: 15px;
  font-weight: 600;
  font-family: 'Iowan Old Style', 'Palatino Linotype', Palatino, Georgia, 'Songti SC', serif;
}

.dm-badge {
  font-size: 10px;
  padding: 2px 7px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--dm-accent) 45%, transparent);
  color: var(--dm-accent);
  white-space: nowrap;
}
.dm-badge.is-quiet {
  border-color: var(--dm-rule);
  color: var(--dm-muted);
}

.dm-pack-meta {
  margin-top: 4px;
  font-size: 11px;
  color: var(--dm-muted);
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.dm-dot {
  opacity: 0.5;
}

.dm-pack-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.dm-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 6px 11px;
  font-size: 12px;
  border-radius: 8px;
  border: 1px solid var(--dm-rule);
  background: transparent;
  color: var(--dm-ink);
  cursor: pointer;
  transition: background 0.15s ease, border-color 0.15s ease, color 0.15s ease;
}
.dm-btn:hover:not(:disabled) {
  background: var(--dm-accent-soft);
  border-color: color-mix(in srgb, var(--dm-accent) 40%, var(--dm-rule));
}
.dm-btn:disabled {
  opacity: 0.5;
  cursor: default;
}
.dm-btn.is-primary {
  background: var(--dm-accent);
  border-color: var(--dm-accent);
  color: #fff;
}
.dm-btn.is-primary:hover:not(:disabled) {
  background: color-mix(in srgb, var(--dm-accent) 88%, #000);
}
.dm-btn.is-danger {
  color: #c2534b;
  border-color: color-mix(in srgb, #c2534b 35%, var(--dm-rule));
}
.dm-btn.is-danger:hover {
  background: color-mix(in srgb, #c2534b 10%, transparent);
}

.dm-hint {
  margin: 2px 0 0;
  font-size: 11px;
  color: var(--dm-muted);
  line-height: 1.7;
}

.dm-catalog-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 10px;
}

.dm-catalog {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 14px;
  border: 1px solid var(--dm-rule);
  border-radius: 12px;
  background: color-mix(in srgb, var(--dm-paper) 92%, var(--dm-accent) 3%);
}

.dm-catalog-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.dm-catalog-head h3 {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
}

.dm-catalog-desc {
  margin: 0;
  font-size: 11.5px;
  line-height: 1.7;
  color: var(--dm-muted);
}

.dm-catalog-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  color: var(--dm-muted);
  flex-wrap: wrap;
}

.dm-installed-mark {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  color: var(--dm-accent);
}

.dm-catalog-actions {
  margin-top: auto;
  display: flex;
  justify-content: flex-end;
}

.dm-job {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.dm-job-text {
  font-size: 10.5px;
  color: var(--dm-muted);
}

.dm-import {
  display: flex;
  flex-direction: column;
  gap: 14px;
  max-width: 620px;
}

.dm-drop {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 30px 18px;
  border: 1.5px dashed color-mix(in srgb, var(--dm-accent) 40%, var(--dm-rule));
  border-radius: 16px;
  background: color-mix(in srgb, var(--dm-accent) 5%, transparent);
  color: var(--dm-muted);
  cursor: pointer;
  transition: background 0.15s ease;
}
.dm-drop:hover {
  background: color-mix(in srgb, var(--dm-accent) 10%, transparent);
}
.dm-drop strong {
  color: var(--dm-ink);
  font-size: 14px;
}
.dm-drop span {
  font-size: 11px;
}

.dm-file-list {
  margin: 0;
  padding: 0 0 0 18px;
  font-size: 11.5px;
  color: var(--dm-muted);
  max-height: 120px;
  overflow-y: auto;
}

.dm-form {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.dm-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 11.5px;
  color: var(--dm-muted);
}
.dm-field input {
  height: 32px;
  padding: 0 10px;
  border-radius: 8px;
  border: 1px solid var(--dm-rule);
  background: var(--dm-paper);
  color: var(--dm-ink);
  font-size: 13px;
  outline: none;
}
.dm-field input:focus {
  border-color: var(--dm-accent);
}
.dm-field-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

.dm-import-actions {
  display: flex;
  gap: 8px;
}

.dm-note {
  border: 1px solid var(--dm-rule);
  border-radius: 12px;
  padding: 12px 14px;
  background: color-mix(in srgb, var(--dm-paper) 92%, var(--dm-accent) 3%);
}
.dm-note h4 {
  margin: 0 0 6px;
  font-size: 12px;
  color: var(--dm-ink);
}
.dm-note p {
  margin: 3px 0;
  font-size: 11.5px;
  line-height: 1.7;
  color: var(--dm-muted);
}

.spin {
  animation: dm-spin 0.9s linear infinite;
}
@keyframes dm-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
