<template>
  <main class="data-workbench" :data-workbench-theme="workbenchTheme">
    <header class="data-workbench__header">
      <div class="data-workbench__title-block">
        <div class="data-workbench__eyebrow">Data Lab Workbench</div>
        <h1 class="data-workbench__title">{{ context.variableName || '变量工作台' }}</h1>
        <div class="data-workbench__meta">
          <span>{{ snapshot?.kind || context.dataKind || 'value' }}</span>
          <span v-if="formatShape(snapshot?.shape)">shape {{ formatShape(snapshot.shape) }}</span>
          <span v-if="snapshot?.length != null">len {{ snapshot.length }}</span>
          <span v-if="tableState.rowCount != null">{{ tableState.rowCount }} 行</span>
          <span v-if="tableState.columns.length">{{ tableState.columns.length }} 列</span>
        </div>
      </div>

      <div class="data-workbench__actions">
        <button type="button" class="data-workbench__icon-btn" title="刷新变量快照" @click="loadSnapshot">
          <Icon :icon="refreshIcon" width="17" height="17" />
        </button>
        <button
          type="button"
          class="data-workbench__action-btn"
          :disabled="!pendingEdits.length || saving"
          @click="saveEdits(false)"
        >
          <Icon :icon="saveIcon" width="16" height="16" />
          保存
        </button>
        <button
          type="button"
          class="data-workbench__action-btn"
          :disabled="!pendingEdits.length || saving"
          @click="saveEdits(true)"
        >
          另存变量
        </button>
        <button
          type="button"
          class="data-workbench__icon-btn"
          title="撤销最后一次编辑"
          :disabled="!pendingEdits.length"
          @click="undoLastEdit"
        >
          <Icon :icon="undoIcon" width="17" height="17" />
        </button>
      </div>
    </header>

    <section class="data-workbench__body">
      <aside class="data-workbench__sidebar">
        <button
          v-for="tab in tabs"
          :key="tab.value"
          type="button"
          class="data-workbench__tab"
          :class="{ 'is-active': activeTab === tab.value }"
          @click="activeTab = tab.value"
        >
          <Icon :icon="tab.icon" width="17" height="17" />
          <span>{{ tab.label }}</span>
        </button>
      </aside>

      <section class="data-workbench__main">
        <div v-if="loading" class="data-workbench__state">正在加载变量快照...</div>
        <div v-else-if="errorText" class="data-workbench__state is-error">{{ errorText }}</div>

        <template v-else>
          <section v-if="activeTab === 'data'" class="data-workbench__panel">
            <div v-if="tableState.columns.length" class="data-workbench__table-shell">
              <div class="data-workbench__table-toolbar">
                <div class="data-workbench__pager">
                  <button type="button" class="data-workbench__mini-btn" :disabled="pageOffset <= 0" @click="goPage(-1)">上一页</button>
                  <span>{{ pageOffset + 1 }} - {{ Math.min(pageOffset + pageLimit, tableState.rowCount || 0) }}</span>
                  <button
                    type="button"
                    class="data-workbench__mini-btn"
                    :disabled="pageOffset + pageLimit >= (tableState.rowCount || 0)"
                    @click="goPage(1)"
                  >
                    下一页
                  </button>
                </div>
                <div class="data-workbench__edit-count">待保存 {{ pendingEdits.length }} 处</div>
              </div>

              <div class="data-workbench__table-wrap">
                <table class="data-workbench__table">
                  <thead>
                    <tr>
                      <th class="data-workbench__row-head">#</th>
                      <th v-for="column in tableState.columns" :key="column">
                        <div>{{ column }}</div>
                        <small>{{ tableState.dtypes[column] || 'object' }}</small>
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="(row, rowIndex) in tableState.rows" :key="`${pageOffset}-${rowIndex}`">
                      <th class="data-workbench__row-head">{{ pageOffset + rowIndex }}</th>
                      <td v-for="(column, colIndex) in tableState.columns" :key="`${rowIndex}-${column}`">
                        <textarea
                          class="data-workbench__cell-input"
                          :value="cellDisplayValue(row[colIndex])"
                          spellcheck="false"
                          @change="recordCellEdit(rowIndex, colIndex, $event.target.value)"
                        />
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>

            <div v-else class="data-workbench__repr">
              <pre>{{ snapshot?.preview || '当前变量无法表格化展示。' }}</pre>
            </div>
          </section>

          <section v-else-if="activeTab === 'structure'" class="data-workbench__panel">
            <div class="data-workbench__structure">
              <GuaxiDataLabTreeNode
                :name="context.variableName || 'value'"
                :value="snapshot?.structure"
                :depth="0"
              />
            </div>
          </section>

          <section v-else-if="activeTab === 'chart'" class="data-workbench__panel data-workbench__chart-panel">
            <div class="data-workbench__form-grid">
              <label>
                <span>图表类型</span>
                <select v-model="chartDraft.chartType" class="data-workbench__field">
                  <option v-for="chart in chartTypes" :key="chart.value" :value="chart.value">{{ chart.label }}</option>
                </select>
              </label>
              <label>
                <span>X 轴</span>
                <select v-model="chartDraft.x" class="data-workbench__field">
                  <option value="">自动</option>
                  <option v-for="column in chartColumns" :key="`x-${column}`" :value="column">{{ column }}</option>
                </select>
              </label>
              <label>
                <span>Y 轴</span>
                <select v-model="chartDraft.y" class="data-workbench__field">
                  <option value="">自动</option>
                  <option v-for="column in chartColumns" :key="`y-${column}`" :value="column">{{ column }}</option>
                </select>
              </label>
              <label>
                <span>Z / 大小</span>
                <select v-model="chartDraft.z" class="data-workbench__field">
                  <option value="">无</option>
                  <option v-for="column in chartColumns" :key="`z-${column}`" :value="column">{{ column }}</option>
                </select>
              </label>
              <label>
                <span>颜色分组</span>
                <select v-model="chartDraft.color" class="data-workbench__field">
                  <option value="">无</option>
                  <option v-for="column in chartColumns" :key="`c-${column}`" :value="column">{{ column }}</option>
                </select>
              </label>
              <label class="wide">
                <span>图表标题</span>
                <input v-model="chartDraft.title" class="data-workbench__field" />
              </label>
            </div>
            <div class="data-workbench__chart-actions">
              <button type="button" class="data-workbench__action-btn" @click="sendChart(false)">
                <Icon :icon="chartIcon" width="16" height="16" />
                绘图到 Cell
              </button>
              <button type="button" class="data-workbench__action-btn" @click="sendChart(true)">
                <Icon :icon="sendIcon" width="16" height="16" />
                绘图写回
              </button>
            </div>
          </section>

          <section v-else-if="activeTab === 'export'" class="data-workbench__panel">
            <div class="data-workbench__export-grid">
              <button
                v-for="item in exportFormats"
                :key="item.value"
                type="button"
                class="data-workbench__export-card"
                @click="exportVariable(item.value)"
              >
                <Icon :icon="downloadIcon" width="20" height="20" />
                <strong>{{ item.label }}</strong>
                <span>.{{ item.extensions[0] }}</span>
              </button>
            </div>
          </section>
        </template>
      </section>

      <aside class="data-workbench__inspector">
        <section class="data-workbench__info-card">
          <h2>字段</h2>
          <div v-if="!tableState.columns.length" class="data-workbench__muted">没有可表格化字段</div>
          <div v-else class="data-workbench__schema-list">
            <div v-for="column in tableState.columns" :key="`schema-${column}`">
              <strong>{{ column }}</strong>
              <span>{{ tableState.dtypes[column] || 'object' }}</span>
            </div>
          </div>
        </section>

        <section class="data-workbench__info-card">
          <h2>编辑队列</h2>
          <div v-if="!pendingEdits.length" class="data-workbench__muted">暂无未保存编辑</div>
          <div v-else class="data-workbench__edit-list">
            <div v-for="(edit, index) in pendingEdits.slice(-8).reverse()" :key="`${edit.rowIndex}-${edit.column}-${index}`">
              <strong>{{ edit.column }}</strong>
              <span>第 {{ edit.rowIndex }} 行</span>
            </div>
          </div>
        </section>
      </aside>
    </section>
  </main>
</template>

<script setup>
import { computed, defineComponent, h, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { Icon } from '@iconify/vue'
import { listen } from '@tauri-apps/api/event'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import saveIcon from '@iconify-icons/lucide/save'
import undoIcon from '@iconify-icons/lucide/rotate-ccw'
import tableIcon from '@iconify-icons/lucide/table-properties'
import treeIcon from '@iconify-icons/lucide/git-fork'
import chartIcon from '@iconify-icons/lucide/bar-chart-3'
import downloadIcon from '@iconify-icons/lucide/file-down'
import sendIcon from '@iconify-icons/lucide/send'
import { useMessage } from 'naive-ui'
import { useRoute } from 'vue-router'
import { createLogger } from '../../utils/logger'
import {
  WORKBENCH_CHART_TYPES,
  WORKBENCH_SUPPORTED_EXPORTS,
  DATA_LAB_SESSION_VARS_UPDATED_EVENT,
  applyWorkbenchEdits,
  emitWorkbenchVariableUpdated,
  exportWorkbenchVariable,
  fetchWorkbenchVariableSnapshot,
  parseWorkbenchRouteQuery,
  requestNotebookInspectorRefresh,
  writeWorkbenchChartToNotebook,
} from './utils/guaxiDataLabWorkbenchClient'

const logger = createLogger('GuaxiDataLabWorkbench')
const message = useMessage()
const route = useRoute()

const context = reactive(parseWorkbenchRouteQuery(route.query || {}))
const snapshot = ref(null)
const loading = ref(false)
const saving = ref(false)
const errorText = ref('')
const activeTab = ref('data')
const pageOffset = ref(0)
const pageLimit = ref(200)
const pendingEdits = ref([])
const chartTypes = WORKBENCH_CHART_TYPES
const exportFormats = WORKBENCH_SUPPORTED_EXPORTS
let unlistenVariableUpdated = null
let pendingRemoteReloadTimer = 0
const workbenchTheme = computed(() => (context.theme === 'dark' ? 'dark' : 'light'))
const chartDraft = reactive({
  chartType: 'line',
  x: '',
  y: '',
  z: '',
  color: '',
  title: '',
})

const tableState = computed(() => {
  const table = snapshot.value?.table && typeof snapshot.value.table === 'object' ? snapshot.value.table : {}
  return {
    columns: Array.isArray(table.columns) ? table.columns.map((item) => String(item ?? '')) : [],
    rows: Array.isArray(table.rows) ? table.rows : [],
    rowCount: Number.isFinite(Number(table.rowCount)) ? Number(table.rowCount) : 0,
    dtypes: table.dtypes && typeof table.dtypes === 'object' ? table.dtypes : {},
  }
})

const chartColumns = computed(() => tableState.value.columns)

const tabs = computed(() => [
  { label: '数据', value: 'data', icon: tableIcon },
  { label: '结构', value: 'structure', icon: treeIcon },
  { label: '绘图', value: 'chart', icon: chartIcon },
  { label: '导出', value: 'export', icon: downloadIcon },
])

watch(chartColumns, (columns) => {
  if (!Array.isArray(columns) || !columns.length) return
  if (!chartDraft.x) chartDraft.x = columns[0] || ''
  if (!chartDraft.y) chartDraft.y = columns[1] || columns[0] || ''
  if (!chartDraft.title) chartDraft.title = `${context.variableName || '变量'} 图表`
}, { immediate: true })

const formatShape = (shape) => {
  if (!Array.isArray(shape) || !shape.length) return ''
  return `(${shape.map((item) => String(item ?? '')).join(', ')})`
}

const cellDisplayValue = (value) => {
  if (value == null) return ''
  if (typeof value === 'object') {
    try {
      return JSON.stringify(value, null, 0)
    } catch {
      return String(value)
    }
  }
  return String(value)
}

const applyWorkbenchTheme = () => {
  const theme = workbenchTheme.value
  if (!document?.documentElement) return
  // 设计说明：
  // 1. 数据工作台是独立 WebView，不能假设主窗口的 DOM 主题会自动继承过来。
  // 2. 这里在工作台挂载时显式写入 Guaxi 全局主题标记，让 naive-ui、全局 CSS 与本组件局部变量保持一致。
  // 3. URL 中的 theme 由主窗口打开工作台时带入，因此新窗口首次绘制就能选中正确的亮/暗色。
  document.documentElement.dataset.guaxiTheme = theme
  document.documentElement.classList.toggle('dark-mode', theme === 'dark')
  document.body?.setAttribute?.('data-guaxi-theme', theme)
}

const loadSnapshot = async ({ preservePending = false, reason = 'manual' } = {}) => {
  if (!preservePending && pendingEdits.value.length) {
    logger.info('[DataWorkbench] skip snapshot reload because local edits are pending', {
      reason,
      variableName: context.variableName,
      pendingCount: pendingEdits.value.length,
    })
    return
  }
  loading.value = true
  errorText.value = ''
  try {
    const result = await fetchWorkbenchVariableSnapshot({
      sessionId: context.sessionId,
      variableName: context.variableName,
      offset: pageOffset.value,
      limit: pageLimit.value,
    })
    snapshot.value = result.snapshot
    logger.info('[DataWorkbench] snapshot loaded', {
      reason,
      sessionId: context.sessionId,
      variableName: context.variableName,
      kind: String(result.snapshot?.kind || ''),
      rowCount: Number(result.snapshot?.table?.rowCount || 0),
    })
  } catch (error) {
    errorText.value = String(error?.message || error)
    logger.error('[DataWorkbench] load snapshot failed', { error: errorText.value })
  } finally {
    loading.value = false
  }
}

const goPage = async (direction) => {
  const nextOffset = Math.max(0, pageOffset.value + Number(direction || 0) * pageLimit.value)
  if (nextOffset === pageOffset.value) return
  pageOffset.value = nextOffset
  await loadSnapshot({ preservePending: true, reason: 'page-change' })
}

const recordCellEdit = (rowIndex, colIndex, value) => {
  const columns = tableState.value.columns
  const column = columns[colIndex]
  if (!column) return
  const absoluteRowIndex = pageOffset.value + Number(rowIndex || 0)
  const row = tableState.value.rows[rowIndex] || []
  const oldValue = row[colIndex]
  row[colIndex] = value
  pendingEdits.value.push({
    rowIndex: absoluteRowIndex,
    column,
    key: columns[0] === 'key' ? row[0] : null,
    oldValue,
    value,
  })
  logger.info('[DataWorkbench] cell edit recorded', {
    variableName: context.variableName,
    rowIndex: absoluteRowIndex,
    column,
    pendingCount: pendingEdits.value.length,
  })
}

const reloadAfterPendingCleared = () => {
  if (pendingEdits.value.length || pendingRemoteReloadTimer) return
  pendingRemoteReloadTimer = window.setTimeout(() => {
    pendingRemoteReloadTimer = 0
    loadSnapshot({ preservePending: true, reason: 'pending-edits-cleared' })
  }, 120)
}

const undoLastEdit = () => {
  const edit = pendingEdits.value.pop()
  if (!edit) return
  const localRowIndex = Number(edit.rowIndex) - pageOffset.value
  const colIndex = tableState.value.columns.indexOf(edit.column)
  if (localRowIndex >= 0 && colIndex >= 0 && tableState.value.rows[localRowIndex]) {
    tableState.value.rows[localRowIndex][colIndex] = edit.oldValue
  }
  logger.info('[DataWorkbench] edit undone', {
    variableName: context.variableName,
    pendingCount: pendingEdits.value.length,
  })
  reloadAfterPendingCleared()
}

const saveEdits = async (saveAsNew = false) => {
  if (!pendingEdits.value.length) return
  const targetName = saveAsNew
    ? String(window.prompt?.('保存为新的变量名', `${context.variableName}_edited`) || '').trim()
    : ''
  if (saveAsNew && !targetName) return

  saving.value = true
  try {
    await applyWorkbenchEdits({
      sessionId: context.sessionId,
      variableName: context.variableName,
      edits: pendingEdits.value,
      saveAs: targetName,
    })
    pendingEdits.value = []
    if (targetName) {
      context.variableName = targetName
    }
    await loadSnapshot({ preservePending: true, reason: 'after-save-edits' })
    await requestNotebookInspectorRefresh(context)
    await emitWorkbenchVariableUpdated({
      sessionId: context.sessionId,
      variableName: context.variableName,
      filePath: context.filePath,
      cellId: context.cellId,
      source: 'data-workbench-save',
    })
    message.success('变量编辑已保存')
  } catch (error) {
    logger.error('[DataWorkbench] save edits failed', { error: String(error?.message || error) })
    message.error(`保存失败：${String(error?.message || error)}`)
  } finally {
    saving.value = false
  }
}

const exportVariable = async (format) => {
  try {
    const outputPath = await exportWorkbenchVariable({
      sessionId: context.sessionId,
      variableName: context.variableName,
      format,
    })
    if (outputPath) {
      message.success('变量已导出')
    }
  } catch (error) {
    logger.error('[DataWorkbench] export failed', { error: String(error?.message || error), format })
    message.error(`导出失败：${String(error?.message || error)}`)
  }
}

const sendChart = async (writeBack = false) => {
  try {
    await writeWorkbenchChartToNotebook({
      context,
      chart: chartDraft,
      writeBack,
    })
    message.success(writeBack ? '绘图代码已写回当前 Cell' : '绘图请求已发送到当前 Cell')
  } catch (error) {
    logger.error('[DataWorkbench] send chart failed', { error: String(error?.message || error) })
    message.error(`绘图失败：${String(error?.message || error)}`)
  }
}

const GuaxiDataLabTreeNode = defineComponent({
  name: 'GuaxiDataLabTreeNode',
  props: {
    name: { type: String, default: 'value' },
    value: { type: null, default: null },
    depth: { type: Number, default: 0 },
  },
  setup(props) {
    const open = ref(props.depth < 2)
    const isObject = computed(() => props.value && typeof props.value === 'object')
    const entries = computed(() => {
      if (!isObject.value) return []
      if (Array.isArray(props.value)) {
        return props.value.slice(0, 200).map((item, index) => [String(index), item])
      }
      return Object.entries(props.value).slice(0, 200)
    })
    return () => h('div', { class: 'data-tree-node' }, [
      h('button', {
        class: ['data-tree-node__head', isObject.value ? 'is-expandable' : ''],
        type: 'button',
        onClick: () => { if (isObject.value) open.value = !open.value },
      }, [
        h('span', { class: 'data-tree-node__name' }, props.name),
        h('span', { class: 'data-tree-node__type' }, Array.isArray(props.value) ? `array[${props.value.length}]` : typeof props.value),
        !isObject.value ? h('code', { class: 'data-tree-node__value' }, String(props.value ?? 'null')) : null,
      ]),
      isObject.value && open.value ? h('div', { class: 'data-tree-node__children' }, entries.value.map(([key, value]) =>
        h(GuaxiDataLabTreeNode, {
          key,
          name: key,
          value,
          depth: props.depth + 1,
        }),
      )) : null,
    ])
  },
})

onMounted(() => {
  applyWorkbenchTheme()
  logger.info('[DataWorkbench] mounted', {
    sessionId: context.sessionId,
    variableName: context.variableName,
    dataKind: context.dataKind,
  })
  loadSnapshot({ preservePending: true, reason: 'mounted' })
  listen(DATA_LAB_SESSION_VARS_UPDATED_EVENT, (event) => {
    const payload = event?.payload && typeof event.payload === 'object' ? event.payload : {}
    const sameSession = String(payload?.sessionId || '') === String(context.sessionId || '')
    const variableName = String(payload?.variableName || '')
    const sameVariable = !variableName || variableName === String(context.variableName || '')
    if (!sameSession || !sameVariable) return

    if (pendingEdits.value.length) {
      logger.info('[DataWorkbench] defer remote snapshot reload because local edits are pending', {
        source: String(payload?.source || ''),
        variableName,
        pendingCount: pendingEdits.value.length,
      })
      message.info('Python 会话中的变量已更新；保存或撤销当前编辑后可刷新查看最新数据')
      return
    }

    if (pendingRemoteReloadTimer) {
      window.clearTimeout(pendingRemoteReloadTimer)
    }
    pendingRemoteReloadTimer = window.setTimeout(() => {
      pendingRemoteReloadTimer = 0
      loadSnapshot({ preservePending: true, reason: `remote:${String(payload?.source || 'session')}` })
    }, 180)
  }).then((unlisten) => {
    unlistenVariableUpdated = unlisten
    logger.info('[DataWorkbench] variable update listener registered', {
      eventName: DATA_LAB_SESSION_VARS_UPDATED_EVENT,
      sessionId: context.sessionId,
      variableName: context.variableName,
    })
  }).catch((error) => {
    logger.warn('[DataWorkbench] variable update listener failed', {
      error: String(error?.message || error),
    })
  })
})

onBeforeUnmount(() => {
  if (pendingRemoteReloadTimer) {
    window.clearTimeout(pendingRemoteReloadTimer)
    pendingRemoteReloadTimer = 0
  }
  if (typeof unlistenVariableUpdated === 'function') {
    unlistenVariableUpdated()
    unlistenVariableUpdated = null
  }
})
</script>

<style scoped>
.data-workbench {
  --dl-bg: #f6f8fb;
  --dl-panel: #ffffff;
  --dl-panel-2: #eef3f7;
  --dl-text: #1d2430;
  --dl-muted: #667085;
  --dl-border: rgba(31, 41, 55, 0.13);
  --dl-accent: #2f6f55;
  --dl-accent-soft: rgba(47, 111, 85, 0.13);
  height: 100vh;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--dl-bg);
  color: var(--dl-text);
  font-family: var(--guaxi-ui-font-family, "Microsoft YaHei", system-ui, sans-serif);
}

.data-workbench[data-workbench-theme='dark'],
:global(:root[data-guaxi-theme='dark']) .data-workbench {
  --dl-bg: #20252d;
  --dl-panel: #2a303a;
  --dl-panel-2: #343b47;
  --dl-text: #e8edf6;
  --dl-muted: #aeb8ca;
  --dl-border: rgba(151, 164, 186, 0.2);
  --dl-accent: #77c69f;
  --dl-accent-soft: rgba(119, 198, 159, 0.14);
}

.data-workbench[data-workbench-theme='light'] {
  --dl-bg: #f6f8fb;
  --dl-panel: #ffffff;
  --dl-panel-2: #eef3f7;
  --dl-text: #1d2430;
  --dl-muted: #667085;
  --dl-border: rgba(31, 41, 55, 0.13);
  --dl-accent: #2f6f55;
  --dl-accent-soft: rgba(47, 111, 85, 0.13);
}

.data-workbench__header {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  padding: 18px 22px;
  border-bottom: 1px solid var(--dl-border);
  background: var(--dl-panel);
}

.data-workbench__title-block {
  min-width: 0;
}

.data-workbench__eyebrow {
  font-size: 11px;
  text-transform: uppercase;
  color: var(--dl-muted);
}

.data-workbench__title {
  margin: 4px 0 8px;
  font-size: 22px;
  line-height: 1.2;
}

.data-workbench__meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  color: var(--dl-muted);
  font-size: 12px;
}

.data-workbench__meta span,
.data-workbench__edit-count {
  border: 1px solid var(--dl-border);
  border-radius: 999px;
  padding: 3px 8px;
  background: var(--dl-panel-2);
}

.data-workbench__actions,
.data-workbench__chart-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.data-workbench__icon-btn,
.data-workbench__action-btn,
.data-workbench__mini-btn,
.data-workbench__field {
  border: 1px solid var(--dl-border);
  background: var(--dl-panel);
  color: var(--dl-text);
  outline: none;
}

.data-workbench__icon-btn,
.data-workbench__action-btn,
.data-workbench__mini-btn {
  cursor: pointer;
}

.data-workbench__icon-btn {
  width: 34px;
  height: 34px;
  border-radius: 8px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.data-workbench__action-btn {
  min-height: 34px;
  border-radius: 8px;
  padding: 0 12px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: var(--dl-accent-soft);
}

.data-workbench__mini-btn {
  min-height: 28px;
  border-radius: 7px;
  padding: 0 10px;
}

.data-workbench__icon-btn:disabled,
.data-workbench__action-btn:disabled,
.data-workbench__mini-btn:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.data-workbench__body {
  flex: 1 1 auto;
  min-height: 0;
  display: grid;
  grid-template-columns: 160px minmax(0, 1fr) 280px;
}

.data-workbench__sidebar,
.data-workbench__inspector {
  min-height: 0;
  overflow: auto;
  border-right: 1px solid var(--dl-border);
  background: var(--dl-panel);
  padding: 14px;
}

.data-workbench__inspector {
  border-right: 0;
  border-left: 1px solid var(--dl-border);
}

.data-workbench__tab {
  width: 100%;
  height: 38px;
  border: 1px solid transparent;
  border-radius: 8px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 10px;
  background: transparent;
  color: var(--dl-muted);
  cursor: pointer;
}

.data-workbench__tab.is-active {
  color: var(--dl-text);
  border-color: var(--dl-border);
  background: var(--dl-accent-soft);
}

.data-workbench__main {
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  padding: 16px;
}

.data-workbench__panel,
.data-workbench__table-shell {
  height: 100%;
  min-height: 0;
}

.data-workbench__table-shell {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--dl-border);
  border-radius: 8px;
  background: var(--dl-panel);
  overflow: hidden;
}

.data-workbench__table-toolbar {
  flex: 0 0 auto;
  min-height: 46px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--dl-border);
}

.data-workbench__pager {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--dl-muted);
}

.data-workbench__table-wrap {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
}

.data-workbench__table {
  border-collapse: separate;
  border-spacing: 0;
  width: max-content;
  min-width: 100%;
  font-size: 13px;
}

.data-workbench__table th,
.data-workbench__table td {
  border-right: 1px solid var(--dl-border);
  border-bottom: 1px solid var(--dl-border);
  min-width: 150px;
  max-width: 360px;
  vertical-align: top;
}

.data-workbench__table th {
  position: sticky;
  top: 0;
  z-index: 2;
  background: var(--dl-panel-2);
  padding: 8px;
  text-align: left;
}

.data-workbench__table th small {
  display: block;
  margin-top: 3px;
  color: var(--dl-muted);
  font-weight: 400;
}

.data-workbench__row-head {
  position: sticky;
  left: 0;
  z-index: 3;
  min-width: 72px !important;
  width: 72px;
  max-width: 72px !important;
  color: var(--dl-muted);
  background: var(--dl-panel-2);
}

.data-workbench__cell-input {
  width: 100%;
  min-height: 38px;
  height: 38px;
  resize: vertical;
  border: 0;
  padding: 8px;
  box-sizing: border-box;
  background: var(--dl-panel);
  color: var(--dl-text);
  font: inherit;
  outline: none;
  appearance: none;
  -webkit-appearance: none;
}

.data-workbench__cell-input:focus {
  background: var(--dl-accent-soft);
  box-shadow: inset 0 0 0 1px var(--dl-accent);
}

.data-workbench__cell-input::selection {
  background: color-mix(in srgb, var(--dl-accent) 34%, transparent);
  color: var(--dl-text);
}

.data-workbench__repr,
.data-workbench__structure,
.data-workbench__state {
  height: 100%;
  overflow: auto;
  border: 1px solid var(--dl-border);
  border-radius: 8px;
  background: var(--dl-panel);
  padding: 16px;
}

.data-workbench__state.is-error {
  color: #d92d20;
}

.data-workbench__repr pre {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
}

.data-workbench__form-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
  max-width: 760px;
}

.data-workbench__form-grid label {
  display: grid;
  gap: 6px;
  font-size: 12px;
  color: var(--dl-muted);
}

.data-workbench__form-grid label.wide {
  grid-column: 1 / -1;
}

.data-workbench__field {
  min-height: 36px;
  border-radius: 8px;
  padding: 0 10px;
  box-sizing: border-box;
  background: var(--dl-panel);
}

.data-workbench__chart-panel {
  display: grid;
  align-content: start;
  gap: 18px;
}

.data-workbench__export-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 12px;
}

.data-workbench__export-card,
.data-workbench__info-card {
  border: 1px solid var(--dl-border);
  border-radius: 8px;
  background: var(--dl-panel);
}

.data-workbench__export-card {
  min-height: 110px;
  display: grid;
  place-items: center;
  gap: 4px;
  color: var(--dl-text);
  cursor: pointer;
}

.data-workbench__export-card span,
.data-workbench__muted {
  color: var(--dl-muted);
}

.data-workbench__info-card {
  padding: 12px;
  margin-bottom: 12px;
}

.data-workbench__info-card h2 {
  margin: 0 0 10px;
  font-size: 14px;
}

.data-workbench__schema-list,
.data-workbench__edit-list {
  display: grid;
  gap: 8px;
}

.data-workbench__schema-list div,
.data-workbench__edit-list div {
  display: grid;
  gap: 3px;
  padding: 8px;
  border-radius: 7px;
  background: var(--dl-panel-2);
  font-size: 12px;
}

.data-workbench__schema-list span,
.data-workbench__edit-list span {
  color: var(--dl-muted);
}

.data-tree-node {
  font-size: 13px;
}

.data-tree-node__head {
  width: 100%;
  min-height: 30px;
  display: grid;
  grid-template-columns: minmax(90px, 180px) 90px minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--dl-text);
  text-align: left;
}

.data-tree-node__head.is-expandable {
  cursor: pointer;
}

.data-tree-node__head:hover {
  background: var(--dl-panel-2);
}

.data-tree-node__name {
  font-weight: 700;
}

.data-tree-node__type {
  color: var(--dl-muted);
}

.data-tree-node__value {
  color: var(--dl-text);
  white-space: pre-wrap;
  word-break: break-word;
}

.data-tree-node__children {
  margin-left: 18px;
  padding-left: 10px;
  border-left: 1px solid var(--dl-border);
}

@media (max-width: 1100px) {
  .data-workbench__body {
    grid-template-columns: 132px minmax(0, 1fr);
  }

  .data-workbench__inspector {
    display: none;
  }
}
</style>
