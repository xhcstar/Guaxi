<template>
  <main class="data-workbench" :data-workbench-theme="workbenchTheme">
    <header class="data-workbench__header">
      <div class="data-workbench__title-block">
        <h1 class="data-workbench__title">变量工作台</h1>
        <div class="data-workbench__variable-picker">
          <button
            type="button"
            class="data-workbench__variable-trigger"
            :class="{ 'is-open': variableMenuOpen }"
            @click="toggleVariableMenu"
          >
            <span>{{ activeVariableName || '选择变量' }}</span>
            <small>{{ activeVariableKindLabel }}</small>
            <Icon :icon="chevronDownIcon" width="16" height="16" />
          </button>
          <div v-if="variableMenuOpen" class="data-workbench__variable-menu" @mousedown.stop>
            <label class="data-workbench__variable-search">
              <Icon :icon="searchIcon" width="15" height="15" />
              <input
                v-model="variableSearch"
                type="text"
                placeholder="搜索变量"
                @keydown.esc.prevent="closeVariableMenu"
              />
            </label>
            <div class="data-workbench__variable-options">
              <button
                v-for="item in filteredSessionVariables"
                :key="item.name"
                type="button"
                class="data-workbench__variable-option"
                :class="{ 'is-active': item.name === activeVariableName }"
                @click="selectVariableFromMenu(item)"
              >
                <span>{{ item.name }}</span>
                <small>{{ item.value_type || item.type || 'value' }}</small>
              </button>
              <div v-if="!filteredSessionVariables.length" class="data-workbench__muted data-workbench__variable-empty">
                无匹配变量
              </div>
            </div>
          </div>
        </div>
      </div>

      <div class="data-workbench__actions">
        <button type="button" class="data-workbench__icon-btn" title="刷新变量和快照" @click="refreshWorkbench">
          <Icon :icon="refreshIcon" width="17" height="17" />
        </button>
        <button type="button" class="data-workbench__icon-btn" title="生成会话报告" @click="emitNotebookAction('generate-report')">
          <Icon :icon="fileTextIcon" width="17" height="17" />
        </button>
        <button type="button" class="data-workbench__icon-btn" title="生成报告并写回" @click="emitNotebookAction('writeback-report')">
          <Icon :icon="sendIcon" width="17" height="17" />
        </button>
        <button
          type="button"
          class="data-workbench__icon-btn"
          title="保存当前变量编辑"
          :disabled="!pendingEdits.length || saving"
          @click="saveEdits(false)"
        >
          <Icon :icon="saveIcon" width="17" height="17" />
        </button>
        <button
          type="button"
          class="data-workbench__icon-btn"
          title="另存为新变量"
          :disabled="!pendingEdits.length || saving"
          @click="saveEdits(true)"
        >
          <Icon :icon="saveAllIcon" width="17" height="17" />
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

    <section
      class="data-workbench__body"
      :style="workbenchGridStyle"
      @mousemove="handleResizeMove"
      @mouseup="stopResize"
      @mouseleave="stopResize"
    >
      <aside class="data-workbench__sidebar">
        <div class="data-workbench__tab-list">
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
        </div>
      </aside>

      <div
        class="data-workbench__resize-handle"
        role="separator"
        aria-label="调整导航宽度"
        @mousedown.prevent="startResize('left', $event)"
      ></div>

      <section class="data-workbench__main">
        <div v-if="loading" class="data-workbench__state">正在加载变量快照...</div>
        <div v-else-if="errorText" class="data-workbench__state is-error">{{ errorText }}</div>
        <div v-else-if="!activeVariableName && activeTab !== 'import'" class="data-workbench__state">请选择一个变量。</div>

        <template v-else>
          <section v-if="activeTab === 'import'" class="data-workbench__panel">
            <div class="data-workbench__operation-grid">
              <button
                v-for="item in importOptions"
                :key="item.value"
                type="button"
                class="data-workbench__operation-card"
                @click="requestImportOption(item)"
              >
                <Icon :icon="item.icon" width="20" height="20" />
                <strong>{{ item.label }}</strong>
                <span>{{ item.summary }}</span>
              </button>
            </div>
          </section>

          <section v-else-if="activeTab === 'data'" class="data-workbench__panel">
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
                          v-if="isEditingCell(rowIndex, colIndex)"
                          class="data-workbench__cell-input"
                          :value="editingCellDraft"
                          :rows="getCellEditorRows(editingCellDraft)"
                          autofocus
                          spellcheck="false"
                          @focus="resizeCellEditor"
                          @input="handleCellEditorInput"
                          @blur="commitEditingCell(rowIndex, colIndex)"
                          @keydown.ctrl.enter.prevent="commitEditingCell(rowIndex, colIndex)"
                          @keydown.esc.prevent="cancelEditingCell"
                        />
                        <button
                          v-else
                          type="button"
                          class="data-workbench__cell-view"
                          title="双击编辑"
                          @dblclick="beginCellEdit(rowIndex, colIndex, row[colIndex])"
                        >
                          <span>{{ cellDisplayValue(row[colIndex]) }}</span>
                        </button>
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
                :name="activeVariableName || 'value'"
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
            <div class="data-workbench__operation-grid">
              <button
                v-for="item in exportFormats"
                :key="item.value"
                type="button"
                class="data-workbench__operation-card"
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

      <div
        class="data-workbench__resize-handle"
        role="separator"
        aria-label="调整检查器宽度"
        @mousedown.prevent="startResize('right', $event)"
      ></div>

      <aside class="data-workbench__inspector">
        <section class="data-workbench__info-card">
          <h2>变量详情</h2>
          <div class="data-workbench__detail-list">
            <div v-for="item in activeVariableDetails" :key="item.label">
              <span>{{ item.label }}</span>
              <strong>{{ item.value }}</strong>
            </div>
          </div>
          <pre v-if="snapshot?.preview" class="data-workbench__inspector-preview">{{ snapshot.preview }}</pre>
        </section>

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
import saveAllIcon from '@iconify-icons/lucide/save-all'
import undoIcon from '@iconify-icons/lucide/rotate-ccw'
import tableIcon from '@iconify-icons/lucide/table-properties'
import treeIcon from '@iconify-icons/lucide/git-fork'
import chartIcon from '@iconify-icons/lucide/bar-chart-3'
import downloadIcon from '@iconify-icons/lucide/file-down'
import sendIcon from '@iconify-icons/lucide/send'
import clipboardIcon from '@iconify-icons/lucide/clipboard'
import chevronDownIcon from '@iconify-icons/lucide/chevron-down'
import fileTextIcon from '@iconify-icons/lucide/file-text'
import linkIcon from '@iconify-icons/lucide/link'
import searchIcon from '@iconify-icons/lucide/search'
import uploadIcon from '@iconify-icons/lucide/upload'
import { useMessage } from 'naive-ui'
import { useRoute } from 'vue-router'
import { createLogger } from '../../utils/logger'
import {
  WORKBENCH_CHART_TYPES,
  WORKBENCH_SUPPORTED_EXPORTS,
  DATA_LAB_SESSION_VARS_UPDATED_EVENT,
  applyWorkbenchEdits,
  emitWorkbenchNotebookAction,
  emitWorkbenchVariableUpdated,
  fetchWorkbenchSessionVariables,
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
const sessionVariables = ref([])
const activeVariableName = ref(context.variableName)
const variableMenuOpen = ref(false)
const variableSearch = ref('')
const editingCellKey = ref('')
const editingCellDraft = ref('')
const leftPaneWidth = ref(112)
const rightPaneWidth = ref(300)
const resizingPane = ref('')
const chartTypes = WORKBENCH_CHART_TYPES
const exportFormats = WORKBENCH_SUPPORTED_EXPORTS
const importOptions = [
  { label: '文件', value: 'file', summary: 'CSV / JSON / Excel / 文本', icon: uploadIcon },
  { label: '剪贴板', value: 'clipboard', summary: '表格文本或结构化文本', icon: clipboardIcon },
  { label: 'URL', value: 'url', summary: '远程 CSV / JSON / Excel / 文本', icon: linkIcon },
]
let unlistenVariableUpdated = null
let pendingRemoteReloadTimer = 0
const workbenchTheme = computed(() => (context.theme === 'dark' ? 'dark' : 'light'))
const workbenchGridStyle = computed(() => ({
  gridTemplateColumns: `${leftPaneWidth.value}px 6px minmax(0, 1fr) 6px ${rightPaneWidth.value}px`,
}))
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
const activeVariableRecord = computed(() => {
  const name = String(activeVariableName.value || '').trim()
  return sessionVariables.value.find((item) => String(item?.name || '').trim() === name) || null
})
const activeVariableKindLabel = computed(() => {
  const record = activeVariableRecord.value
  return String(snapshot.value?.kind || record?.value_type || record?.type || context.dataKind || 'value')
})
const filteredSessionVariables = computed(() => {
  const keyword = String(variableSearch.value || '').trim().toLowerCase()
  const items = Array.isArray(sessionVariables.value) ? sessionVariables.value : []
  if (!keyword) return items
  return items.filter((item) => {
    const name = String(item?.name || '').toLowerCase()
    const type = String(item?.value_type || item?.type || '').toLowerCase()
    return name.includes(keyword) || type.includes(keyword)
  })
})
const activeVariableDetails = computed(() => {
  const details = [
    { label: '名称', value: activeVariableName.value || '-' },
    { label: '类型', value: activeVariableKindLabel.value || '-' },
  ]
  const shape = formatShape(snapshot.value?.shape || activeVariableRecord.value?.shape)
  if (shape) details.push({ label: 'shape', value: shape })
  const length = snapshot.value?.length ?? activeVariableRecord.value?.length
  if (length != null) details.push({ label: 'len', value: String(length) })
  if (tableState.value.columns.length) {
    details.push({ label: '行数', value: String(tableState.value.rowCount) })
    details.push({ label: '列数', value: String(tableState.value.columns.length) })
  }
  return details
})

const tabs = computed(() => [
  { label: '导入', value: 'import', icon: uploadIcon },
  { label: '数据', value: 'data', icon: tableIcon },
  { label: '结构', value: 'structure', icon: treeIcon },
  { label: '绘图', value: 'chart', icon: chartIcon },
  { label: '导出', value: 'export', icon: downloadIcon },
])

watch(chartColumns, (columns) => {
  if (!Array.isArray(columns) || !columns.length) return
  if (!chartDraft.x) chartDraft.x = columns[0] || ''
  if (!chartDraft.y) chartDraft.y = columns[1] || columns[0] || ''
  if (!chartDraft.title) chartDraft.title = `${activeVariableName.value || context.variableName || '变量'} 图表`
}, { immediate: true })

watch(activeVariableName, (name) => {
  context.variableName = String(name || '')
})

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

const toggleVariableMenu = () => {
  variableMenuOpen.value = !variableMenuOpen.value
  if (variableMenuOpen.value) {
    variableSearch.value = ''
    logger.info('[DataWorkbench] opened variable selector', {
      variableCount: sessionVariables.value.length,
      activeVariableName: activeVariableName.value,
    })
  }
}

const closeVariableMenu = () => {
  variableMenuOpen.value = false
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
  if (!activeVariableName.value) return
  if (!preservePending && pendingEdits.value.length) {
    logger.info('[DataWorkbench] skip snapshot reload because local edits are pending', {
      reason,
      variableName: activeVariableName.value,
      pendingCount: pendingEdits.value.length,
    })
    return
  }
  loading.value = true
  errorText.value = ''
  try {
    const result = await fetchWorkbenchVariableSnapshot({
      sessionId: context.sessionId,
      variableName: activeVariableName.value,
      offset: pageOffset.value,
      limit: pageLimit.value,
    })
    snapshot.value = result.snapshot
    logger.info('[DataWorkbench] snapshot loaded', {
      reason,
      sessionId: context.sessionId,
      variableName: activeVariableName.value,
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

const loadSessionVariables = async (reason = 'manual') => {
  try {
    sessionVariables.value = await fetchWorkbenchSessionVariables({
      sessionId: context.sessionId,
    })
    logger.info('[DataWorkbench] session variables loaded', {
      reason,
      variableCount: sessionVariables.value.length,
      activeVariableName: activeVariableName.value,
    })
    if (!activeVariableName.value && sessionVariables.value.length) {
      activeVariableName.value = String(sessionVariables.value[0]?.name || '')
    }
  } catch (error) {
    logger.warn('[DataWorkbench] load session variables failed', {
      reason,
      error: String(error?.message || error),
    })
  }
}

const refreshWorkbench = async () => {
  await loadSessionVariables('manual-refresh')
  await loadSnapshot({ preservePending: true, reason: 'manual-refresh' })
}

const selectVariable = async (item) => {
  const nextName = String(item?.name || '').trim()
  if (!nextName || nextName === activeVariableName.value) return
  if (pendingEdits.value.length && !window.confirm?.('当前变量有未保存编辑，切换变量会丢弃这些编辑。是否继续？')) return
  pendingEdits.value = []
  editingCellKey.value = ''
  editingCellDraft.value = ''
  pageOffset.value = 0
  activeVariableName.value = nextName
  context.dataKind = String(item?.value_type || item?.type || 'value')
  logger.info('[DataWorkbench] selected variable', {
    variableName: nextName,
    dataKind: context.dataKind,
  })
  await loadSnapshot({ preservePending: true, reason: 'select-variable' })
}

const selectVariableFromMenu = async (item) => {
  closeVariableMenu()
  await selectVariable(item)
}

const emitNotebookAction = async (action, extra = {}) => {
  try {
    await emitWorkbenchNotebookAction({
      context,
      action,
      variableName: activeVariableName.value,
      dataKind: context.dataKind,
      extra,
    })
    message.success('已发送到当前 Notebook')
  } catch (error) {
    logger.error('[DataWorkbench] emit notebook action failed', {
      action,
      error: String(error?.message || error),
    })
    message.error(`发送失败：${String(error?.message || error)}`)
  }
}

const requestImportOption = async (item) => {
  const value = String(item?.value || '')
  // 设计说明：
  // 1. 导入和导出都采用“配置数组 + 卡片区域”的结构，后续扩展新的数据源时只需要追加配置和 action 映射。
  // 2. 顶部工具栏只保留跨工作区的轻量命令，避免导入方式增多后挤占窗口标题区域。
  // 3. URL 这类需要额外参数的导入方式在这里收集参数，再复用统一的 Notebook RPC 通道。
  logger.info('[DataWorkbench] import option selected', {
    value,
    label: String(item?.label || ''),
  })
  if (value === 'file') {
    await emitNotebookAction('import-file')
    return
  }
  if (value === 'clipboard') {
    await emitNotebookAction('import-clipboard')
    return
  }
  if (value === 'url') {
    const sourceUrl = String(window.prompt?.('输入要导入的 CSV / JSON / Excel / 文本 URL', '') || '').trim()
    if (!sourceUrl) return
    await emitNotebookAction('import-url', {
      sourceUrl,
      sourceLabel: sourceUrl,
    })
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
  if (cellDisplayValue(oldValue) === String(value ?? '')) {
    logger.debug('[DataWorkbench] skipped unchanged cell edit', {
      variableName: activeVariableName.value,
      rowIndex: absoluteRowIndex,
      column,
    })
    return
  }
  row[colIndex] = value
  pendingEdits.value.push({
    rowIndex: absoluteRowIndex,
    column,
    key: columns[0] === 'key' ? row[0] : null,
    oldValue,
    value,
  })
  logger.info('[DataWorkbench] cell edit recorded', {
    variableName: activeVariableName.value,
    rowIndex: absoluteRowIndex,
    column,
    pendingCount: pendingEdits.value.length,
  })
}

const getCellEditKey = (rowIndex, colIndex) => `${pageOffset.value + Number(rowIndex || 0)}:${Number(colIndex || 0)}`
const isEditingCell = (rowIndex, colIndex) => editingCellKey.value === getCellEditKey(rowIndex, colIndex)

const beginCellEdit = (rowIndex, colIndex, value) => {
  editingCellKey.value = getCellEditKey(rowIndex, colIndex)
  editingCellDraft.value = cellDisplayValue(value)
  logger.info('[DataWorkbench] begin cell edit', {
    variableName: activeVariableName.value,
    rowIndex: pageOffset.value + Number(rowIndex || 0),
    column: tableState.value.columns[colIndex] || '',
  })
}

// 设计说明：
// 1. 表格默认进入只读展示态，避免用户滚动或点选时把数据误改。
// 2. 只有双击单元格后才创建 textarea，并在 blur / Ctrl+Enter 时提交编辑。
// 3. 输入框按内容高度自适应，长文本不再被固定 38px 的单行高度截断。
const commitEditingCell = (rowIndex, colIndex) => {
  if (!isEditingCell(rowIndex, colIndex)) return
  recordCellEdit(rowIndex, colIndex, editingCellDraft.value)
  editingCellKey.value = ''
  editingCellDraft.value = ''
}

const cancelEditingCell = () => {
  logger.info('[DataWorkbench] cancelled cell edit', {
    variableName: activeVariableName.value,
    editingCellKey: editingCellKey.value,
  })
  editingCellKey.value = ''
  editingCellDraft.value = ''
}

const resizeCellEditor = (event) => {
  const target = event?.target
  if (!(target instanceof HTMLTextAreaElement)) return
  target.style.height = 'auto'
  target.style.height = `${Math.min(260, Math.max(42, target.scrollHeight + 2))}px`
}

const handleCellEditorInput = (event) => {
  editingCellDraft.value = String(event?.target?.value ?? '')
  resizeCellEditor(event)
}

const getCellEditorRows = (value) => {
  const text = String(value ?? '')
  const lineCount = text.split(/\r?\n/).length
  const wrapCount = Math.ceil(text.length / 56)
  return Math.min(12, Math.max(2, lineCount, wrapCount))
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
    variableName: activeVariableName.value,
    pendingCount: pendingEdits.value.length,
  })
  reloadAfterPendingCleared()
}

const saveEdits = async (saveAsNew = false) => {
  if (!pendingEdits.value.length) return
  const targetName = saveAsNew
    ? String(window.prompt?.('保存为新的变量名', `${activeVariableName.value || context.variableName}_edited`) || '').trim()
    : ''
  if (saveAsNew && !targetName) return

  saving.value = true
  try {
    await applyWorkbenchEdits({
      sessionId: context.sessionId,
      variableName: activeVariableName.value,
      edits: pendingEdits.value,
      saveAs: targetName,
    })
    pendingEdits.value = []
    if (targetName) {
      context.variableName = targetName
      activeVariableName.value = targetName
    }
    await loadSnapshot({ preservePending: true, reason: 'after-save-edits' })
    await requestNotebookInspectorRefresh(context)
    await emitWorkbenchVariableUpdated({
      sessionId: context.sessionId,
      variableName: activeVariableName.value,
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
      variableName: activeVariableName.value,
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

const startResize = (pane) => {
  resizingPane.value = String(pane || '')
  document.body?.classList?.add('data-workbench-resizing')
  logger.info('[DataWorkbench] start pane resize', { pane: resizingPane.value })
}

const stopResize = () => {
  if (!resizingPane.value) return
  logger.info('[DataWorkbench] stop pane resize', {
    pane: resizingPane.value,
    leftPaneWidth: leftPaneWidth.value,
    rightPaneWidth: rightPaneWidth.value,
  })
  resizingPane.value = ''
  document.body?.classList?.remove('data-workbench-resizing')
}

const handleResizeMove = (event) => {
  if (!resizingPane.value) return
  const root = event.currentTarget
  const rect = root?.getBoundingClientRect?.()
  if (!rect) return
  if (resizingPane.value === 'left') {
    leftPaneWidth.value = Math.min(180, Math.max(84, Number(event.clientX) - rect.left))
    return
  }
  rightPaneWidth.value = Math.min(460, Math.max(240, rect.right - Number(event.clientX)))
}

onMounted(() => {
  applyWorkbenchTheme()
  logger.info('[DataWorkbench] mounted', {
    sessionId: context.sessionId,
    variableName: context.variableName,
    dataKind: context.dataKind,
  })
  loadSnapshot({ preservePending: true, reason: 'mounted' })
  loadSessionVariables('mounted')
  listen(DATA_LAB_SESSION_VARS_UPDATED_EVENT, (event) => {
    const payload = event?.payload && typeof event.payload === 'object' ? event.payload : {}
    const sameSession = String(payload?.sessionId || '') === String(context.sessionId || '')
    const variableName = String(payload?.variableName || '')
    const sameVariable = !variableName || variableName === String(activeVariableName.value || context.variableName || '')
    if (!sameSession) return
    loadSessionVariables(`remote-vars:${String(payload?.source || 'session')}`)
    if (!sameVariable) return

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
      variableName: activeVariableName.value,
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
  document.body?.classList?.remove('data-workbench-resizing')
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
  padding: 14px 18px;
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
  margin: 0 0 8px;
  font-size: 22px;
  line-height: 1.2;
}

.data-workbench__edit-count {
  border: 1px solid var(--dl-border);
  border-radius: 999px;
  padding: 3px 8px;
  background: var(--dl-panel-2);
}

.data-workbench__variable-picker {
  position: relative;
  width: min(420px, 48vw);
}

.data-workbench__variable-trigger {
  width: 100%;
  min-height: 34px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  align-items: center;
  gap: 8px;
  border: 1px solid var(--dl-border);
  border-radius: 8px;
  padding: 0 10px;
  background: var(--dl-panel-2);
  color: var(--dl-text);
  cursor: pointer;
}

.data-workbench__variable-trigger span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
  font-weight: 700;
}

.data-workbench__variable-trigger small {
  color: var(--dl-muted);
}

.data-workbench__variable-trigger.is-open {
  border-color: color-mix(in srgb, var(--dl-accent) 55%, var(--dl-border));
}

.data-workbench__variable-menu {
  position: absolute;
  z-index: 20;
  top: calc(100% + 8px);
  left: 0;
  width: min(520px, 72vw);
  max-height: min(520px, 70vh);
  display: flex;
  flex-direction: column;
  gap: 8px;
  border: 1px solid var(--dl-border);
  border-radius: 8px;
  padding: 10px;
  background: var(--dl-panel);
  box-shadow: 0 18px 44px rgba(15, 23, 42, 0.22);
}

.data-workbench__variable-search {
  flex: 0 0 auto;
  height: 34px;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  border: 1px solid var(--dl-border);
  border-radius: 8px;
  padding: 0 10px;
  color: var(--dl-muted);
  background: var(--dl-panel-2);
}

.data-workbench__variable-search input {
  min-width: 0;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--dl-text);
  font: inherit;
}

.data-workbench__variable-options {
  min-height: 0;
  overflow: auto;
  display: grid;
  align-content: start;
  gap: 6px;
}

.data-workbench__variable-option {
  min-height: 38px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  border: 1px solid transparent;
  border-radius: 8px;
  padding: 7px 9px;
  background: transparent;
  color: var(--dl-text);
  text-align: left;
  cursor: pointer;
}

.data-workbench__variable-option:hover,
.data-workbench__variable-option.is-active {
  border-color: var(--dl-border);
  background: var(--dl-accent-soft);
}

.data-workbench__variable-option span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 700;
}

.data-workbench__variable-option small,
.data-workbench__variable-empty {
  color: var(--dl-muted);
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
  grid-template-columns: 112px 6px minmax(0, 1fr) 6px 300px;
}

.data-workbench__sidebar,
.data-workbench__inspector {
  min-height: 0;
  border-right: 1px solid var(--dl-border);
  background: var(--dl-panel);
  padding: 14px;
}

.data-workbench__sidebar {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.data-workbench__inspector {
  border-right: 0;
  border-left: 1px solid var(--dl-border);
  overflow: auto;
}

.data-workbench__resize-handle {
  cursor: col-resize;
  background: transparent;
  position: relative;
}

.data-workbench__resize-handle::before {
  content: "";
  position: absolute;
  inset: 0 2px;
  background: transparent;
  border-left: 1px solid var(--dl-border);
  border-right: 1px solid transparent;
}

.data-workbench__resize-handle:hover::before {
  border-left-color: var(--dl-accent);
}

:global(body.data-workbench-resizing) {
  cursor: col-resize;
  user-select: none;
}

.data-workbench__tab-list {
  flex: 1 1 auto;
  display: grid;
  align-content: start;
  gap: 6px;
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
  min-width: 180px;
  max-width: 460px;
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
  min-height: 42px;
  max-height: 260px;
  resize: vertical;
  border: 1px solid var(--dl-accent);
  border-radius: 6px;
  padding: 8px 9px;
  box-sizing: border-box;
  background: var(--dl-panel-2);
  color: var(--dl-text);
  font: inherit;
  font-family: var(--editor-font-family, Consolas, monospace);
  line-height: 1.45;
  outline: none;
  appearance: none;
  -webkit-appearance: none;
}

.data-workbench__cell-input:focus {
  box-shadow: inset 0 0 0 1px var(--dl-accent);
}

.data-workbench__cell-input::selection {
  background: color-mix(in srgb, var(--dl-accent) 34%, transparent);
  color: var(--dl-text);
}

.data-workbench__cell-view {
  width: 100%;
  min-height: 42px;
  max-height: 220px;
  overflow: auto;
  display: block;
  border: 0;
  padding: 8px 10px;
  background: transparent;
  color: var(--dl-text);
  text-align: left;
  cursor: default;
  font: inherit;
  font-family: var(--editor-font-family, Consolas, monospace);
  line-height: 1.45;
}

.data-workbench__cell-view:hover {
  background: color-mix(in srgb, var(--dl-accent-soft) 72%, transparent);
}

.data-workbench__cell-view span {
  white-space: pre-wrap;
  word-break: break-word;
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

.data-workbench__operation-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 12px;
}

.data-workbench__operation-card,
.data-workbench__info-card {
  border: 1px solid var(--dl-border);
  border-radius: 8px;
  background: var(--dl-panel);
}

.data-workbench__operation-card {
  min-height: 110px;
  display: grid;
  place-items: center;
  gap: 4px;
  color: var(--dl-text);
  cursor: pointer;
}

.data-workbench__operation-card:hover {
  border-color: color-mix(in srgb, var(--dl-accent) 42%, var(--dl-border));
  background: var(--dl-accent-soft);
}

.data-workbench__operation-card span,
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

.data-workbench__detail-list {
  display: grid;
  gap: 8px;
}

.data-workbench__detail-list div {
  display: grid;
  gap: 3px;
  padding: 8px;
  border-radius: 7px;
  background: var(--dl-panel-2);
  font-size: 12px;
}

.data-workbench__detail-list span {
  color: var(--dl-muted);
}

.data-workbench__detail-list strong {
  min-width: 0;
  overflow-wrap: anywhere;
}

.data-workbench__inspector-preview {
  margin: 10px 0 0;
  max-height: 220px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
  border: 1px solid var(--dl-border);
  border-radius: 8px;
  padding: 10px;
  background: var(--dl-panel-2);
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
    grid-template-columns: minmax(92px, 132px) 6px minmax(0, 1fr);
  }

  .data-workbench__body .data-workbench__resize-handle:last-of-type,
  .data-workbench__inspector {
    display: none;
  }
}
</style>
