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
        <div
          class="data-workbench__edit-count"
          :class="{ 'has-edits': pendingEdits.length }"
          title="当前变量尚未保存的编辑数量"
        >
          待保存 {{ pendingEdits.length }} 处
        </div>
      </div>

      <div class="data-workbench__actions">
        <button type="button" class="data-workbench__icon-btn" title="刷新变量和快照" data-shortcut="Ctrl+R" @click="refreshWorkbench">
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
          data-shortcut="Ctrl+S"
          :disabled="!pendingEdits.length || saving"
          @click="saveEdits(false)"
        >
          <Icon :icon="saveIcon" width="17" height="17" />
        </button>
        <button
          type="button"
          class="data-workbench__icon-btn"
          title="另存为新变量"
          data-shortcut="Ctrl+Shift+S"
          :disabled="!pendingEdits.length || saving"
          @click="saveEdits(true)"
        >
          <Icon :icon="saveAllIcon" width="17" height="17" />
        </button>
        <button
          type="button"
          class="data-workbench__icon-btn"
          title="撤销最后一次编辑"
          data-shortcut="Ctrl+Z"
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
      <aside class="data-workbench__sidebar" :class="{ 'is-collapsed': leftSidebarCollapsed }">
        <div class="data-workbench__tab-list">
          <button
            v-for="(tab, index) in tabs"
            :key="tab.value"
            type="button"
            class="data-workbench__tab"
            :class="{ 'is-active': activeTab === tab.value }"
            :title="`${tab.label}（Ctrl+${index + 1}）`"
            @click="selectTab(tab.value)"
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
      >
        <button
          type="button"
          class="data-workbench__pane-toggle is-left"
          :title="leftSidebarCollapsed ? '展开左侧导航' : '收起左侧导航'"
          @mousedown.stop
          @click.stop="togglePane('left')"
        >
          <Icon :icon="leftSidebarCollapsed ? arrowRightToLineIcon : arrowLeftToLineIcon" width="16" height="16" />
        </button>
      </div>

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
              <div class="data-workbench__table-footer">
                <div class="data-workbench__pager">
                  <span>{{ pageOffset + 1 }} - {{ Math.min(pageOffset + pageLimit, tableState.rowCount || 0) }} / {{ tableState.rowCount || 0 }}</span>
                  <button type="button" class="data-workbench__mini-btn" :disabled="pageOffset <= 0" @click="goPage(-1)">上一页</button>
                  <button
                    type="button"
                    class="data-workbench__mini-btn"
                    :disabled="pageOffset + pageLimit >= (tableState.rowCount || 0)"
                    @click="goPage(1)"
                  >
                    下一页
                  </button>
                </div>
              </div>
            </div>

            <div v-else class="data-workbench__repr">
              <pre>{{ prettyPreview || '当前变量无法表格化展示。' }}</pre>
            </div>
          </section>

          <section v-else-if="activeTab === 'structure'" class="data-workbench__panel">
            <div class="data-workbench__structure">
              <div class="data-workbench__structure-toolbar">
                <div>
                  <strong>{{ structureFocusLabel }}</strong>
                  <span>{{ structureGraph.nodes.length }} 个节点 · {{ structureGraph.edges.length }} 条关系</span>
                </div>
                <div class="data-workbench__structure-actions">
                  <button type="button" class="data-workbench__icon-btn" title="放大结构图" @click="zoomStructure(0.14)">
                    <Icon :icon="zoomInIcon" width="16" height="16" />
                  </button>
                  <button type="button" class="data-workbench__icon-btn" title="缩小结构图" @click="zoomStructure(-0.14)">
                    <Icon :icon="zoomOutIcon" width="16" height="16" />
                  </button>
                  <button type="button" class="data-workbench__icon-btn" title="重置视图" @click="resetStructureView">
                    <Icon :icon="rotateCcwIcon" width="16" height="16" />
                  </button>
                  <button type="button" class="data-workbench__icon-btn" title="返回上一级节点" :disabled="!canFocusStructureParent" @click="focusStructureParent">
                    <Icon :icon="cornerUpLeftIcon" width="16" height="16" />
                  </button>
                  <button type="button" class="data-workbench__icon-btn" title="回到根节点" :disabled="structureFocusPath === 'value'" @click="focusStructureNode('value')">
                    <Icon :icon="homeIcon" width="16" height="16" />
                  </button>
                  <button
                    type="button"
                    class="data-workbench__icon-btn"
                    title="展开或收起选中节点"
                    :disabled="!selectedStructureNode?.childCount"
                    @click="toggleSelectedStructureNode"
                  >
                    <Icon :icon="selectedStructureNode?.expanded ? foldIcon : unfoldIcon" width="16" height="16" />
                  </button>
                  <button
                    type="button"
                    class="data-workbench__icon-btn"
                    title="聚焦选中节点"
                    :disabled="!selectedStructureNode?.childCount"
                    @click="focusStructureNode(selectedStructureNode.path)"
                  >
                    <Icon :icon="focusIcon" width="16" height="16" />
                  </button>
                </div>
              </div>

              <div
                class="data-workbench__structure-canvas"
                @mousedown="startStructurePan"
                @wheel.prevent="handleStructureWheel"
              >
                <div
                  class="data-workbench__structure-stage"
                  :style="structureStageStyle"
                >
                  <svg
                    class="data-workbench__structure-links"
                    :width="structureGraph.width"
                    :height="structureGraph.height"
                    aria-hidden="true"
                  >
                    <path
                      v-for="edge in structureGraph.edges"
                      :key="edge.key"
                      class="data-workbench__structure-link"
                      :d="edge.path"
                    />
                  </svg>
                  <button
                    v-for="node in structureGraph.nodes"
                    :key="node.path"
                    type="button"
                    class="data-workbench__structure-node"
                    :class="{ 'is-selected': node.path === selectedStructurePath, 'has-children': node.childCount }"
                    :style="{ left: `${node.x}px`, top: `${node.y}px` }"
                    @mousedown.stop
                    @click="selectStructureNode(node)"
                    @dblclick.stop="focusStructureNode(node.path)"
                  >
                    <span class="data-workbench__structure-node-title">{{ node.label }}</span>
                    <code>{{ node.type }}</code>
                    <small>{{ node.summary }}</small>
                    <span v-if="node.childCount" class="data-workbench__structure-node-badge">
                      {{ node.expanded ? '展开' : `${node.childCount} 子项` }}
                    </span>
                    <span
                      v-if="node.childCount"
                      class="data-workbench__structure-node-toggle"
                      :title="node.expanded ? '收起子节点' : '展开子节点'"
                      @click.stop="toggleStructureNode(node.path)"
                    >
                      <Icon :icon="node.expanded ? foldIcon : unfoldIcon" width="14" height="14" />
                    </span>
                  </button>
                </div>
                <aside v-if="selectedStructureNode" class="data-workbench__structure-detail">
                  <div>
                    <strong>{{ selectedStructureNode.label }}</strong>
                    <code>{{ selectedStructureNode.type }}</code>
                  </div>
                  <pre>{{ selectedStructureNode.preview }}</pre>
                </aside>
                <div v-if="!structureGraph.nodes.length" class="data-workbench__structure-empty">
                  {{ prettyPreview || '当前变量没有可展开的结构。' }}
                </div>
              </div>
            </div>
          </section>

          <section v-else-if="activeTab === 'chart'" class="data-workbench__panel data-workbench__chart-panel">
            <div class="data-workbench__chart-builder">
              <section class="data-workbench__chart-config">
                <label>
                  <span>图表类型</span>
                  <GuaxiWorkbenchCombobox v-model="chartDraft.chartType" :options="chartTypes" select-only placeholder="选择图表类型" />
                </label>
                <label>
                  <span>X 轴数据</span>
                  <GuaxiWorkbenchCombobox v-model="chartDraft.x" :options="chartSourceOptions" placeholder="例如 index、price_df.date、other_var" />
                </label>
                <label>
                  <span>Z / 大小</span>
                  <GuaxiWorkbenchCombobox v-model="chartDraft.z" :options="chartSourceOptions" placeholder="可选，例如 volume" />
                </label>
                <label>
                  <span>颜色分组</span>
                  <GuaxiWorkbenchCombobox v-model="chartDraft.color" :options="chartSourceOptions" placeholder="可选，例如 category" />
                </label>
                <label class="wide">
                  <span>图表标题</span>
                  <input v-model="chartDraft.title" class="data-workbench__field" />
                </label>
              </section>

              <section class="data-workbench__chart-series">
                <div class="data-workbench__chart-section-head">
                  <h2>Y 轴序列</h2>
                  <button type="button" class="data-workbench__mini-btn" @click="addChartSeries">
                    <Icon :icon="plusIcon" width="14" height="14" />
                    添加序列
                  </button>
                </div>
                <div class="data-workbench__series-list">
                  <div v-for="(series, index) in chartDraft.ySeries" :key="`series-${index}`" class="data-workbench__series-row">
                    <GuaxiWorkbenchCombobox v-model="series.source" :options="chartSourceOptions" placeholder="变量或字段，例如 value、df.score" />
                    <input v-model="series.label" class="data-workbench__field" placeholder="图例名称，可选" />
                    <button
                      type="button"
                      class="data-workbench__icon-btn"
                      title="删除序列"
                      :disabled="chartDraft.ySeries.length <= 1"
                      @click="removeChartSeries(index)"
                    >
                      <Icon :icon="xIcon" width="15" height="15" />
                    </button>
                  </div>
                </div>
              </section>

              <section class="data-workbench__chart-help">
                <strong>数据源写法</strong>
                <span>字段可直接写字段名；跨变量可写变量名或变量.字段。多个 Y 序列会自动按行索引对齐。</span>
              </section>

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
      >
        <button
          type="button"
          class="data-workbench__pane-toggle is-right"
          :title="rightInspectorCollapsed ? '展开右侧信息栏' : '收起右侧信息栏'"
          @mousedown.stop
          @click.stop="togglePane('right')"
        >
          <Icon :icon="rightInspectorCollapsed ? arrowLeftToLineIcon : arrowRightToLineIcon" width="16" height="16" />
        </button>
      </div>

      <aside class="data-workbench__inspector" :class="{ 'is-collapsed': rightInspectorCollapsed }">
        <section class="data-workbench__inspector-section">
          <div class="data-workbench__inspector-header">
            <h2>变量概览</h2>
            <span>{{ activeVariableKindLabel }}</span>
          </div>
          <div class="data-workbench__metric-grid">
            <div v-for="item in activeVariableDetails" :key="item.label" class="data-workbench__metric">
              <span>{{ item.label }}</span>
              <strong>{{ item.value }}</strong>
            </div>
          </div>
        </section>

        <section v-if="snapshot?.preview" class="data-workbench__inspector-section">
          <div class="data-workbench__inspector-header">
            <h2>预览</h2>
            <div class="data-workbench__inspector-tools">
              <span>{{ previewLineCount }} 行</span>
              <button type="button" class="data-workbench__copy-btn" title="复制预览内容" @click="copyPreviewText">
                <Icon :icon="copyIcon" width="14" height="14" />
              </button>
            </div>
          </div>
          <pre class="data-workbench__inspector-preview">{{ prettyPreview }}</pre>
        </section>

        <section class="data-workbench__inspector-section">
          <div class="data-workbench__inspector-header">
            <h2>字段</h2>
            <span v-if="tableState.columns.length">{{ tableState.columns.length }} 列</span>
          </div>
          <div v-if="!tableState.columns.length" class="data-workbench__muted">没有可表格化字段</div>
          <div v-else class="data-workbench__schema-table">
            <div v-for="column in tableState.columns" :key="`schema-${column}`" class="data-workbench__schema-row">
              <span>{{ column }}</span>
              <code>{{ tableState.dtypes[column] || 'object' }}</code>
            </div>
          </div>
        </section>

        <section class="data-workbench__inspector-section">
          <div class="data-workbench__inspector-header">
            <h2>最近编辑</h2>
            <span>{{ pendingEdits.length }} 处</span>
          </div>
          <div v-if="!pendingEdits.length" class="data-workbench__muted">暂无未保存编辑</div>
          <div v-else class="data-workbench__edit-list">
            <div v-for="(edit, index) in pendingEdits.slice(-8).reverse()" :key="`${edit.rowIndex}-${edit.column}-${index}`" class="data-workbench__edit-row">
              <span>{{ edit.column }}</span>
              <code>第 {{ edit.rowIndex }} 行</code>
            </div>
          </div>
        </section>
      </aside>
    </section>
  </main>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { Icon } from '@iconify/vue'
import { listen } from '@tauri-apps/api/event'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import saveIcon from '@iconify-icons/lucide/save'
import saveAllIcon from '@iconify-icons/lucide/save-all'
import undoIcon from '@iconify-icons/lucide/rotate-ccw'
import tableIcon from '@iconify-icons/lucide/table-properties'
import treeIcon from '@iconify-icons/lucide/git-fork'
import chartIcon from '@iconify-icons/lucide/bar-chart-3'
import downloadIcon from '@iconify-icons/lucide/file-down'
import sendIcon from '@iconify-icons/lucide/send'
import copyIcon from '@iconify-icons/lucide/copy'
import plusIcon from '@iconify-icons/lucide/plus'
import xIcon from '@iconify-icons/lucide/x'
import arrowLeftToLineIcon from '@iconify-icons/lucide/arrow-left-to-line'
import arrowRightToLineIcon from '@iconify-icons/lucide/arrow-right-to-line'
import clipboardIcon from '@iconify-icons/lucide/clipboard'
import chevronDownIcon from '@iconify-icons/lucide/chevron-down'
import cornerUpLeftIcon from '@iconify-icons/lucide/corner-up-left'
import fileTextIcon from '@iconify-icons/lucide/file-text'
import focusIcon from '@iconify-icons/lucide/focus'
import foldIcon from '@iconify-icons/lucide/fold-horizontal'
import homeIcon from '@iconify-icons/lucide/home'
import linkIcon from '@iconify-icons/lucide/link'
import rotateCcwIcon from '@iconify-icons/lucide/rotate-ccw'
import searchIcon from '@iconify-icons/lucide/search'
import unfoldIcon from '@iconify-icons/lucide/unfold-horizontal'
import uploadIcon from '@iconify-icons/lucide/upload'
import zoomInIcon from '@iconify-icons/lucide/zoom-in'
import zoomOutIcon from '@iconify-icons/lucide/zoom-out'
import { useMessage } from 'naive-ui'
import { useRoute } from 'vue-router'
import { createLogger } from '../../utils/logger'
import GuaxiWorkbenchCombobox from './GuaxiWorkbenchCombobox.vue'
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
const leftPaneWidth = ref(176)
const rightPaneWidth = ref(300)
const lastLeftPaneWidth = ref(176)
const lastRightPaneWidth = ref(300)
const leftSidebarCollapsed = ref(false)
const rightInspectorCollapsed = ref(false)
const resizingPane = ref('')
const structureFocusPath = ref('value')
const selectedStructurePath = ref('value')
const expandedStructurePaths = ref(new Set(['value']))
const structurePan = reactive({ x: 36, y: 34, scale: 1 })
const structureDrag = reactive({ active: false, startX: 0, startY: 0, originX: 0, originY: 0 })
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
// 设计说明：
// 1. 工作台主体使用 5 列网格：左栏 / 左分隔条 / 主编辑区 / 右分隔条 / 右栏，折叠时只把侧栏列压到 0，保留分隔条按钮。
// 2. 左右栏宽度都由响应式变量驱动，并在拖拽中按当前窗口宽度计算上限，避免固定三列把表格、预览或导航挤死。
// 3. 侧栏宽度仍保留最后一次拖拽值，避免展开后丢失用户调整过的工作区比例。
const workbenchGridStyle = computed(() => ({
  gridTemplateColumns: `${leftSidebarCollapsed.value ? 0 : leftPaneWidth.value}px 6px minmax(0, 1fr) 6px ${rightInspectorCollapsed.value ? 0 : rightPaneWidth.value}px`,
}))
const chartDraft = reactive({
  chartType: 'line',
  x: '',
  y: '',
  ySeries: [
    { source: '', label: '' },
  ],
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
const activeColumnSources = computed(() => {
  const name = String(activeVariableName.value || '').trim()
  return tableState.value.columns.map((column) => ({
    value: column,
    label: name ? `${name}.${column}` : column,
  }))
})
const chartSourceOptions = computed(() => {
  const options = []
  const seen = new Set()
  const addOption = (value, label = value) => {
    const nextValue = String(value || '').trim()
    if (!nextValue || seen.has(nextValue)) return
    seen.add(nextValue)
    options.push({ value: nextValue, label: String(label || nextValue) })
  }

  activeColumnSources.value.forEach((item) => addOption(item.value, item.label))
  if (activeVariableName.value) addOption(activeVariableName.value, '当前变量')
  sessionVariables.value.forEach((item) => {
    const name = String(item?.name || '').trim()
    if (!name) return
    addOption(name, `${name} · ${item?.value_type || item?.type || 'value'}`)
  })
  return options
})
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
// 设计说明：
// 1. 右侧栏不再为每一段简单信息创建大卡片，而是尽量用密集的键值网格和等宽预览提升扫读效率。
// 2. preview 如果本身是 JSON 字符串，先格式化成缩进文本；普通 repr 则原样保留，避免破坏 Python 的可读输出。
// 3. 行数用于快速判断预览内容体量，减少用户在窄侧栏里反复滚动试探。
const prettyPreview = computed(() => formatPreviewText(snapshot.value?.preview || ''))
const previewLineCount = computed(() => {
  const text = prettyPreview.value
  if (!text) return 0
  return text.split(/\r?\n/).length
})
const structureRootValue = computed(() => snapshot.value?.structure ?? snapshot.value?.table ?? snapshot.value?.preview ?? null)
const structureStageStyle = computed(() => ({
  width: `${structureGraph.value.width}px`,
  height: `${structureGraph.value.height}px`,
  transform: `translate(${structurePan.x}px, ${structurePan.y}px) scale(${structurePan.scale})`,
}))
const structureFocusLabel = computed(() => {
  const node = getStructureNodeByPath(structureFocusPath.value)
  return node?.label || activeVariableName.value || 'value'
})
const selectedStructureNode = computed(() => getStructureNodeByPath(selectedStructurePath.value))
const canFocusStructureParent = computed(() => getStructureParentPath(structureFocusPath.value) !== String(structureFocusPath.value || 'value'))
const structureGraph = computed(() => {
  const root = getStructureNodeByPath(structureFocusPath.value) || createStructureNode(structureRootValue.value, activeVariableName.value || 'value', 'value', 0)
  if (!root) return { nodes: [], edges: [], width: 960, height: 560 }

  const nodes = []
  const edges = []
  const levelCounts = new Map()
  const maxNodes = 140
  const collect = (node, depth = 0, parent = null) => {
    if (!node || nodes.length >= maxNodes || depth > 6) return
    const levelIndex = levelCounts.get(depth) || 0
    levelCounts.set(depth, levelIndex + 1)
    const nextNode = {
      ...node,
      graphDepth: depth,
      levelIndex,
      expanded: expandedStructurePaths.value.has(node.path),
    }
    nodes.push(nextNode)
    if (parent) edges.push({ from: parent.path, to: node.path, key: `${parent.path}->${node.path}` })
    if (!nextNode.expanded) return
    nextNode.children.slice(0, 18).forEach((child) => collect(child, depth + 1, nextNode))
  }
  collect(root)

  const levelGapX = 260
  const nodeGapY = 126
  const width = Math.max(920, (Math.max(...nodes.map((node) => node.graphDepth), 0) + 1) * levelGapX + 220)
  const height = Math.max(560, Math.max(...Array.from(levelCounts.values(), (count) => count * nodeGapY), [0]) + 140)
  const levelOffsets = new Map()
  levelCounts.forEach((count, depth) => {
    levelOffsets.set(depth, Math.max(40, (height - count * nodeGapY) / 2))
  })

  const positionByPath = new Map()
  nodes.forEach((node) => {
    node.x = 40 + node.graphDepth * levelGapX
    node.y = (levelOffsets.get(node.graphDepth) || 40) + node.levelIndex * nodeGapY
    positionByPath.set(node.path, node)
  })

  const shapedEdges = edges.map((edge) => {
    const from = positionByPath.get(edge.from)
    const to = positionByPath.get(edge.to)
    const startX = Number(from?.x || 0) + 210
    const startY = Number(from?.y || 0) + 42
    const endX = Number(to?.x || 0)
    const endY = Number(to?.y || 0) + 42
    const midX = startX + Math.max(44, (endX - startX) / 2)
    return {
      ...edge,
      path: `M ${startX} ${startY} C ${midX} ${startY}, ${midX} ${endY}, ${endX} ${endY}`,
    }
  })

  return { nodes, edges: shapedEdges, width, height }
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
  const defaultY = columns[1] || columns[0] || ''
  if (!chartDraft.y) chartDraft.y = defaultY
  if (!chartDraft.ySeries?.[0]?.source) chartDraft.ySeries[0].source = defaultY
  if (!chartDraft.title) chartDraft.title = `${activeVariableName.value || context.variableName || '变量'} 图表`
}, { immediate: true })

watch(activeVariableName, (name) => {
  context.variableName = String(name || '')
  structureFocusPath.value = 'value'
  selectedStructurePath.value = 'value'
  expandedStructurePaths.value = new Set(['value'])
  resetStructureView()
})

const formatShape = (shape) => {
  if (!Array.isArray(shape) || !shape.length) return ''
  return `(${shape.map((item) => String(item ?? '')).join(', ')})`
}

const formatPreviewText = (value) => {
  const text = String(value || '').trim()
  if (!text) return ''
  try {
    return JSON.stringify(JSON.parse(text), null, 2)
  } catch {
    return text
  }
}

const getValueType = (value) => {
  if (Array.isArray(value)) return `array[${value.length}]`
  if (value === null) return 'null'
  return typeof value
}

const getStructureEntries = (value) => {
  if (!value || typeof value !== 'object') return []
  return Array.isArray(value)
    ? value.map((item, index) => [String(index), item])
    : Object.entries(value)
}

const createStructureNode = (value, label = 'value', path = 'value', depth = 0) => {
  const entries = getStructureEntries(value)
  return {
    label: String(label || 'value'),
    path: String(path || 'value'),
    depth,
    type: getValueType(value),
    summary: entries.length ? `${entries.length} 子项` : compactDisplayValue(value).slice(0, 44),
    preview: compactDisplayValue(value),
    childCount: entries.length,
    children: entries.slice(0, 120).map(([key, child]) => createStructureNode(child, key, `${path}.${key}`, depth + 1)),
  }
}

const getStructureNodeByPath = (path) => {
  const targetPath = String(path || 'value')
  const root = createStructureNode(structureRootValue.value, activeVariableName.value || 'value', 'value', 0)
  const stack = [root]
  while (stack.length) {
    const node = stack.shift()
    if (!node) continue
    if (node.path === targetPath) return node
    stack.push(...(node.children || []))
  }
  return root
}

const getStructureParentPath = (path) => {
  const normalizedPath = String(path || 'value')
  if (!normalizedPath || normalizedPath === 'value') return 'value'
  const dotIndex = normalizedPath.lastIndexOf('.')
  return dotIndex > 0 ? normalizedPath.slice(0, dotIndex) : 'value'
}

const compactDisplayValue = (value) => {
  if (value == null) return String(value)
  if (typeof value === 'string') return value
  if (typeof value === 'number' || typeof value === 'boolean') return String(value)
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

const clampStructureScale = (value) => Math.min(2.2, Math.max(0.45, Number(value) || 1))

const zoomStructure = (delta) => {
  structurePan.scale = clampStructureScale(structurePan.scale + Number(delta || 0))
  logger.info('[DataWorkbench] zoom structure canvas', {
    variableName: activeVariableName.value,
    scale: structurePan.scale,
  })
}

const resetStructureView = () => {
  structurePan.x = 36
  structurePan.y = 34
  structurePan.scale = 1
  logger.info('[DataWorkbench] reset structure canvas', {
    variableName: activeVariableName.value,
  })
}

const selectStructureNode = (node) => {
  selectedStructurePath.value = String(node?.path || 'value')
  logger.info('[DataWorkbench] selected structure node', {
    variableName: activeVariableName.value,
    path: selectedStructurePath.value,
  })
}

const toggleStructureNode = (path) => {
  const normalizedPath = String(path || 'value')
  const next = new Set(expandedStructurePaths.value)
  if (next.has(normalizedPath)) {
    next.delete(normalizedPath)
  } else {
    next.add(normalizedPath)
  }
  expandedStructurePaths.value = next
  selectedStructurePath.value = normalizedPath
  logger.info('[DataWorkbench] toggled structure node', {
    variableName: activeVariableName.value,
    path: normalizedPath,
    expanded: next.has(normalizedPath),
  })
}

const toggleSelectedStructureNode = () => {
  if (!selectedStructureNode.value?.childCount) return
  toggleStructureNode(selectedStructureNode.value.path)
}

const focusStructureNode = (path) => {
  const normalizedPath = String(path || 'value')
  structureFocusPath.value = normalizedPath
  selectedStructurePath.value = normalizedPath
  expandedStructurePaths.value = new Set([normalizedPath])
  resetStructureView()
  logger.info('[DataWorkbench] focused structure subgraph', {
    variableName: activeVariableName.value,
    path: normalizedPath,
  })
}

const focusStructureParent = () => {
  const parentPath = getStructureParentPath(structureFocusPath.value)
  if (parentPath === String(structureFocusPath.value || 'value')) return
  focusStructureNode(parentPath)
  logger.info('[DataWorkbench] focused structure parent', {
    variableName: activeVariableName.value,
    parentPath,
  })
}

const startStructurePan = (event) => {
  structureDrag.active = true
  structureDrag.startX = Number(event.clientX || 0)
  structureDrag.startY = Number(event.clientY || 0)
  structureDrag.originX = structurePan.x
  structureDrag.originY = structurePan.y
  window.addEventListener('mousemove', handleStructurePanMove)
  window.addEventListener('mouseup', stopStructurePan, { once: true })
}

const handleStructurePanMove = (event) => {
  if (!structureDrag.active) return
  structurePan.x = structureDrag.originX + Number(event.clientX || 0) - structureDrag.startX
  structurePan.y = structureDrag.originY + Number(event.clientY || 0) - structureDrag.startY
}

const stopStructurePan = () => {
  if (!structureDrag.active) return
  structureDrag.active = false
  window.removeEventListener('mousemove', handleStructurePanMove)
  logger.info('[DataWorkbench] panned structure canvas', {
    variableName: activeVariableName.value,
    x: structurePan.x,
    y: structurePan.y,
    scale: structurePan.scale,
  })
}

const handleStructureWheel = (event) => {
  const delta = Number(event.deltaY || 0) > 0 ? -0.08 : 0.08
  structurePan.scale = clampStructureScale(structurePan.scale + delta)
}

const cellDisplayValue = (value) => {
  if (value == null) return ''
  if (typeof value === 'object') {
    try {
      return JSON.stringify(value, null, 2)
    } catch {
      return String(value)
    }
  }
  return String(value)
}

const writeClipboardText = async (text) => {
  const value = String(text || '')
  if (!value) return false
  if (navigator?.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(value)
      return true
    } catch (error) {
      logger.warn('[DataWorkbench] navigator clipboard write failed', {
        error: String(error?.message || error),
      })
    }
  }

  try {
    const mod = await import('@tauri-apps/plugin-clipboard-manager')
    await mod.writeText(value)
    return true
  } catch (error) {
    logger.error('[DataWorkbench] clipboard write failed', {
      error: String(error?.message || error),
    })
    return false
  }
}

const copyPreviewText = async () => {
  const copied = await writeClipboardText(prettyPreview.value)
  if (!copied) {
    message.error('复制预览内容失败')
    return
  }
  logger.info('[DataWorkbench] copied inspector preview', {
    variableName: activeVariableName.value,
    textLength: prettyPreview.value.length,
  })
  message.success('已复制预览内容')
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

const selectTab = (value) => {
  const nextTab = String(value || '')
  if (!nextTab || nextTab === activeTab.value) return
  logger.info('[DataWorkbench] switched tab', {
    from: activeTab.value,
    to: nextTab,
    variableName: activeVariableName.value,
  })
  activeTab.value = nextTab
}

const addChartSeries = () => {
  chartDraft.ySeries.push({ source: '', label: '' })
  logger.info('[DataWorkbench] added chart series', {
    variableName: activeVariableName.value,
    seriesCount: chartDraft.ySeries.length,
  })
}

const removeChartSeries = (index) => {
  if (chartDraft.ySeries.length <= 1) return
  chartDraft.ySeries.splice(index, 1)
  logger.info('[DataWorkbench] removed chart series', {
    variableName: activeVariableName.value,
    index,
    seriesCount: chartDraft.ySeries.length,
  })
}

const focusMainWindow = async (reason = 'data-workbench') => {
  try {
    const mainWindow = await WebviewWindow.getByLabel('main')
    if (!mainWindow) return
    await mainWindow.show()
    await mainWindow.setFocus()
    logger.info('[DataWorkbench] focused main window', { reason })
  } catch (error) {
    logger.warn('[DataWorkbench] focus main window failed', {
      reason,
      error: String(error?.message || error),
    })
  }
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
  logger.info('[DataWorkbench] page changed', {
    variableName: activeVariableName.value,
    direction,
    pageOffset: pageOffset.value,
    pageLimit: pageLimit.value,
  })
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

const isWorkbenchEditableShortcutTarget = (target) => (
  target instanceof HTMLElement
  && !!target.closest('input, textarea, select, option, [contenteditable="true"]')
)

const runWorkbenchShortcutAction = async (action, event) => {
  logger.info('[DataWorkbenchShortcut] trigger workbench shortcut', {
    action,
    key: String(event?.key || ''),
    ctrlKey: !!event?.ctrlKey,
    metaKey: !!event?.metaKey,
    shiftKey: !!event?.shiftKey,
    activeTab: activeTab.value,
    variableName: activeVariableName.value,
    pendingCount: pendingEdits.value.length,
  })
  if (action === 'refresh') {
    await refreshWorkbench()
    return
  }
  if (action === 'save') {
    await saveEdits(false)
    return
  }
  if (action === 'save-as') {
    await saveEdits(true)
    return
  }
  if (action === 'undo') {
    undoLastEdit()
    return
  }
  if (action.startsWith('tab:')) {
    selectTab(action.slice(4))
  }
}

const handleWorkbenchShortcutKeydown = (event) => {
  const key = String(event?.key || '').toLowerCase()
  const primary = !!(event?.ctrlKey || event?.metaKey)
  if (!primary || event?.altKey) return

  const editableTarget = isWorkbenchEditableShortcutTarget(event?.target)
  let action = ''
  if (key === 'r' && !event.shiftKey) action = 'refresh'
  else if (key === 's' && event.shiftKey) action = 'save-as'
  else if (key === 's') action = 'save'
  else if (!editableTarget && key === 'z' && !event.shiftKey) action = 'undo'
  else if (!editableTarget && /^[1-5]$/.test(key)) {
    const tab = tabs.value[Number(key) - 1]
    if (tab?.value) action = `tab:${tab.value}`
  }

  if (!action) return
  event.preventDefault()
  event.stopPropagation()
  void runWorkbenchShortcutAction(action, event)
}

const sendChart = async (writeBack = false) => {
  try {
    await writeWorkbenchChartToNotebook({
      context,
      chart: chartDraft,
      writeBack,
    })
    await focusMainWindow(writeBack ? 'chart-writeback' : 'chart-output')
    message.success(writeBack ? '绘图代码已写回当前 Cell' : '绘图请求已发送到当前 Cell')
  } catch (error) {
    logger.error('[DataWorkbench] send chart failed', { error: String(error?.message || error) })
    message.error(`绘图失败：${String(error?.message || error)}`)
  }
}

const startResize = (pane) => {
  if ((pane === 'left' && leftSidebarCollapsed.value) || (pane === 'right' && rightInspectorCollapsed.value)) {
    logger.info('[DataWorkbench] ignore resize while pane is collapsed', { pane })
    return
  }
  resizingPane.value = String(pane || '')
  document.body?.classList?.add('data-workbench-resizing')
  logger.info('[DataWorkbench] start pane resize', { pane: resizingPane.value })
}

// 设计说明：
// 1. 折叠按钮放在分隔条中央，比放在侧栏内容区更稳定，侧栏宽度为 0 时仍然可见。
// 2. 折叠只影响布局宽度和内容显隐，不重置 activeTab、字段列表或编辑队列，避免用户展开后上下文丢失。
// 3. 每次切换都写入日志，后续如果出现侧栏状态和网格宽度不一致，可以从用户操作序列回溯。
const togglePane = (pane) => {
  if (pane === 'left') {
    if (!leftSidebarCollapsed.value) {
      lastLeftPaneWidth.value = leftPaneWidth.value
    } else {
      leftPaneWidth.value = Math.max(132, lastLeftPaneWidth.value || 176)
    }
    leftSidebarCollapsed.value = !leftSidebarCollapsed.value
    logger.info('[DataWorkbench] toggled left sidebar', {
      collapsed: leftSidebarCollapsed.value,
      leftPaneWidth: leftPaneWidth.value,
    })
    return
  }

  if (!rightInspectorCollapsed.value) {
    lastRightPaneWidth.value = rightPaneWidth.value
  } else {
    rightPaneWidth.value = Math.max(240, lastRightPaneWidth.value || 300)
  }
  rightInspectorCollapsed.value = !rightInspectorCollapsed.value
  logger.info('[DataWorkbench] toggled right inspector', {
    collapsed: rightInspectorCollapsed.value,
    rightPaneWidth: rightPaneWidth.value,
  })
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
  const bodyWidth = Math.max(0, Number(rect.width || 0))
  const minMainWidth = Math.min(680, Math.max(360, bodyWidth * 0.36))
  if (resizingPane.value === 'left') {
    const maxLeftWidth = Math.max(176, bodyWidth - rightPaneWidth.value - minMainWidth - 12)
    leftPaneWidth.value = Math.min(maxLeftWidth, Math.max(132, Number(event.clientX) - rect.left))
    lastLeftPaneWidth.value = leftPaneWidth.value
    logger.debug('[DataWorkbench] resizing left pane', {
      leftPaneWidth: leftPaneWidth.value,
      maxLeftWidth,
      minMainWidth,
    })
    return
  }
  const maxRightWidth = Math.max(300, bodyWidth - leftPaneWidth.value - minMainWidth - 12)
  rightPaneWidth.value = Math.min(maxRightWidth, Math.max(240, rect.right - Number(event.clientX)))
  lastRightPaneWidth.value = rightPaneWidth.value
  logger.debug('[DataWorkbench] resizing right pane', {
    rightPaneWidth: rightPaneWidth.value,
    maxRightWidth,
    minMainWidth,
  })
}

onMounted(() => {
  applyWorkbenchTheme()
  document.addEventListener('keydown', handleWorkbenchShortcutKeydown, true)
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
  document.removeEventListener('keydown', handleWorkbenchShortcutKeydown, true)
  if (pendingRemoteReloadTimer) {
    window.clearTimeout(pendingRemoteReloadTimer)
    pendingRemoteReloadTimer = 0
  }
  if (typeof unlistenVariableUpdated === 'function') {
    unlistenVariableUpdated()
    unlistenVariableUpdated = null
  }
  window.removeEventListener('mousemove', handleStructurePanMove)
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
  color-scheme: light;
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
  color-scheme: dark;
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
  color-scheme: light;
}

.data-workbench__header {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--dl-border);
  background: var(--dl-panel);
}

.data-workbench__title-block {
  min-width: 0;
  flex: 1 1 auto;
  display: flex;
  align-items: center;
  gap: 10px;
}

.data-workbench__eyebrow {
  font-size: 11px;
  text-transform: uppercase;
  color: var(--dl-muted);
}

.data-workbench__title {
  flex: 0 0 auto;
  margin: 0;
  font-size: 18px;
  line-height: 1.2;
  white-space: nowrap;
}

.data-workbench__edit-count {
  flex: 0 0 auto;
  border: 1px solid var(--dl-border);
  border-radius: 999px;
  padding: 4px 9px;
  background: var(--dl-panel-2);
  color: var(--dl-muted);
  font-size: 12px;
  line-height: 1;
  white-space: nowrap;
}

.data-workbench__edit-count.has-edits {
  border-color: color-mix(in srgb, var(--dl-accent) 45%, var(--dl-border));
  background: var(--dl-accent-soft);
  color: var(--dl-text);
}

.data-workbench__variable-picker {
  flex: 0 1 420px;
  position: relative;
  width: min(420px, 42vw);
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
  flex: 0 0 auto;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.data-workbench__icon-btn,
.data-workbench__action-btn,
.data-workbench__mini-btn,
.data-workbench__field,
.data-workbench__copy-btn {
  border: 1px solid var(--dl-border);
  background: var(--dl-panel);
  color: var(--dl-text);
  outline: none;
}

.data-workbench__icon-btn,
.data-workbench__action-btn,
.data-workbench__mini-btn,
.data-workbench__copy-btn {
  cursor: pointer;
}

.data-workbench__icon-btn {
  position: relative;
  width: 34px;
  height: 34px;
  border-radius: 8px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.data-workbench__copy-btn {
  position: relative;
  width: 26px;
  height: 26px;
  border-radius: 7px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: var(--dl-panel-2);
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

.data-workbench__icon-btn::after,
.data-workbench__copy-btn::after,
.data-workbench__pane-toggle::after,
.data-workbench__structure-node-toggle::after {
  content: attr(title) "  " attr(data-shortcut);
  position: absolute;
  left: 50%;
  bottom: calc(100% + 8px);
  z-index: 60;
  width: max-content;
  max-width: min(300px, calc(100vw - 48px));
  padding: 6px 8px;
  border-radius: 7px;
  background: color-mix(in srgb, #111827 92%, transparent);
  color: #ffffff;
  font-size: 12px;
  line-height: 1.35;
  text-align: center;
  white-space: normal;
  overflow-wrap: anywhere;
  opacity: 0;
  pointer-events: none;
  transform: translate(-50%, 5px);
  transition: opacity 0.14s ease, transform 0.14s ease;
  box-shadow: 0 10px 26px rgba(15, 23, 42, 0.22);
}

/* 设计说明：
 * 1. 工作台头部操作区贴近独立窗口顶部，tooltip 向上弹出会被窗口边界裁切。
 * 2. 仅让头部常用 icon 向下弹出，结构图、检查器等内部 icon 保持原来的就近提示方向。
 */
.data-workbench__actions .data-workbench__icon-btn::after {
  top: calc(100% + 8px);
  bottom: auto;
  transform: translate(-50%, -5px);
}

.data-workbench__icon-btn:hover::after,
.data-workbench__icon-btn:focus-visible::after,
.data-workbench__copy-btn:hover::after,
.data-workbench__copy-btn:focus-visible::after,
.data-workbench__pane-toggle:hover::after,
.data-workbench__pane-toggle:focus-visible::after,
.data-workbench__structure-node-toggle:hover::after,
.data-workbench__structure-node-toggle:focus-visible::after {
  opacity: 1;
  transform: translate(-50%, 0);
}

.data-workbench__body {
  flex: 1 1 auto;
  min-height: 0;
  display: grid;
  grid-template-columns: 176px 6px minmax(0, 1fr) 6px 300px;
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
  /*
   * 设计说明：
   * 1. 左侧 tab 的 tooltip 通过 ::after 从按钮右侧伸出，展开态不能被侧栏裁切。
   * 2. 折叠态仍由 .is-collapsed 规则接管 overflow:hidden，避免收起后内容漏出。
   */
  position: relative;
  z-index: 8;
  overflow: visible;
}

.data-workbench__sidebar.is-collapsed,
.data-workbench__inspector.is-collapsed {
  padding: 0;
  border-color: transparent;
  overflow: hidden;
}

.data-workbench__sidebar.is-collapsed > *,
.data-workbench__inspector.is-collapsed > * {
  display: none;
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
  z-index: 4;
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

.data-workbench__pane-toggle {
  position: absolute;
  top: 50%;
  z-index: 5;
  width: 24px;
  height: 34px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--dl-border);
  border-radius: 7px;
  background: var(--dl-panel);
  color: var(--dl-muted);
  box-shadow: 0 8px 22px rgba(15, 23, 42, 0.12);
  transform: translateY(-50%);
  cursor: pointer;
}

.data-workbench__pane-toggle:hover {
  color: var(--dl-text);
  border-color: color-mix(in srgb, var(--dl-accent) 48%, var(--dl-border));
  background: var(--dl-accent-soft);
}

.data-workbench__pane-toggle.is-left {
  left: -10px;
}

.data-workbench__pane-toggle.is-right {
  right: -10px;
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
  position: relative;
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
  padding: 0;
}

.data-workbench__panel,
.data-workbench__table-shell {
  height: 100%;
  min-height: 0;
}

.data-workbench__table-shell {
  display: flex;
  flex-direction: column;
  border: 0;
  border-radius: 0;
  background: var(--dl-panel);
  overflow: hidden;
}

.data-workbench__table-footer {
  flex: 0 0 auto;
  min-height: 38px;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding: 5px 10px;
  border-top: 1px solid var(--dl-border);
  background: var(--dl-panel);
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
  border: 0;
  border-radius: 0;
  background: var(--dl-panel);
  padding: 16px;
}

.data-workbench__structure {
  background:
    linear-gradient(90deg, color-mix(in srgb, var(--dl-border) 42%, transparent) 1px, transparent 1px),
    linear-gradient(color-mix(in srgb, var(--dl-border) 42%, transparent) 1px, transparent 1px),
    var(--dl-bg);
  background-size: 44px 44px;
}

.data-workbench__structure-toolbar {
  position: sticky;
  top: 0;
  z-index: 3;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin: -16px -16px 0;
  padding: 10px 12px;
  border-bottom: 1px solid var(--dl-border);
  background: color-mix(in srgb, var(--dl-panel) 88%, transparent);
  backdrop-filter: blur(10px);
}

.data-workbench__structure-toolbar > div:first-child {
  min-width: 0;
  display: grid;
  gap: 3px;
}

.data-workbench__structure-toolbar strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.data-workbench__structure-toolbar span {
  color: var(--dl-muted);
  font-size: 12px;
}

.data-workbench__structure-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.data-workbench__structure-canvas {
  position: relative;
  min-height: calc(100% - 44px);
  overflow: hidden;
  cursor: grab;
}

.data-workbench__structure-canvas:active {
  cursor: grabbing;
}

.data-workbench__structure-stage {
  position: absolute;
  left: 0;
  top: 0;
  transform-origin: 0 0;
}

.data-workbench__structure-links {
  position: absolute;
  inset: 0;
  overflow: visible;
}

.data-workbench__structure-link {
  fill: none;
  stroke: color-mix(in srgb, var(--dl-accent) 42%, var(--dl-border));
  stroke-width: 2;
}

.data-workbench__structure-node {
  position: absolute;
  width: 210px;
  min-height: 84px;
  display: grid;
  align-content: start;
  gap: 5px;
  border: 1px solid var(--dl-border);
  border-radius: 8px;
  padding: 10px;
  background: color-mix(in srgb, var(--dl-panel) 92%, var(--dl-accent-soft) 8%);
  color: var(--dl-text);
  text-align: left;
  box-shadow: 0 12px 28px rgba(15, 23, 42, 0.14);
  cursor: pointer;
}

.data-workbench__structure-node.has-children {
  padding-right: 28px;
}

.data-workbench__structure-node:hover,
.data-workbench__structure-node.is-selected {
  border-color: color-mix(in srgb, var(--dl-accent) 60%, var(--dl-border));
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--dl-accent) 18%, transparent), 0 16px 34px rgba(15, 23, 42, 0.18);
}

.data-workbench__structure-node-title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 800;
}

.data-workbench__structure-node code,
.data-workbench__structure-detail code {
  color: var(--dl-muted);
  font-family: var(--editor-font-family, Consolas, monospace);
  font-size: 12px;
}

.data-workbench__structure-node small {
  color: var(--dl-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.data-workbench__structure-node-badge {
  justify-self: start;
  border-radius: 999px;
  padding: 2px 7px;
  background: var(--dl-accent-soft);
  color: var(--dl-text);
  font-size: 11px;
}

.data-workbench__structure-node-toggle {
  position: absolute;
  top: 50%;
  right: -13px;
  width: 26px;
  height: 26px;
  transform: translateY(-50%);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--dl-border);
  border-radius: 999px;
  background: var(--dl-panel);
  color: var(--dl-text);
  box-shadow: 0 8px 18px rgba(15, 23, 42, 0.16);
}

.data-workbench__structure-node-toggle:hover {
  border-color: color-mix(in srgb, var(--dl-accent) 58%, var(--dl-border));
  background: var(--dl-accent-soft);
}

.data-workbench__tab::after {
  content: attr(title);
  position: absolute;
  left: calc(100% + 8px);
  top: 50%;
  z-index: 60;
  /*
   * 设计说明：
   * 1. 伪元素挂在 tab 内部，自动宽度会受按钮/侧栏剩余空间影响，导致中文标题被压成竖排。
   * 2. tab 标题都很短，使用内容宽度展示；max-width 只作为极端长文案的窗口边界兜底。
   */
  width: max-content;
  max-width: min(260px, calc(100vw - 48px));
  padding: 6px 8px;
  border-radius: 7px;
  background: color-mix(in srgb, #111827 92%, transparent);
  color: #ffffff;
  font-size: 12px;
  line-height: 1.35;
  text-align: center;
  white-space: nowrap;
  overflow-wrap: anywhere;
  opacity: 0;
  pointer-events: none;
  transform: translate(0, calc(-50% + 5px));
  transition: opacity 0.14s ease, transform 0.14s ease;
  box-shadow: 0 10px 26px rgba(15, 23, 42, 0.22);
}

.data-workbench__tab:hover::after,
.data-workbench__tab:focus-visible::after {
  opacity: 1;
  transform: translate(0, -50%);
}

.data-workbench__structure-detail {
  position: absolute;
  right: 14px;
  bottom: 14px;
  z-index: 4;
  width: min(360px, calc(100% - 28px));
  display: grid;
  gap: 10px;
  border: 1px solid var(--dl-border);
  border-radius: 8px;
  padding: 12px;
  background: var(--dl-panel);
  color: var(--dl-text);
  box-shadow: 0 18px 42px rgba(15, 23, 42, 0.2);
}

.data-workbench__structure-detail > div:first-child {
  display: flex;
  justify-content: space-between;
  gap: 8px;
}

.data-workbench__structure-detail pre,
.data-workbench__structure-empty {
  margin: 0;
  max-height: 220px;
  overflow: auto;
  border-radius: 7px;
  padding: 9px;
  background: var(--dl-panel-2);
  color: var(--dl-text);
  white-space: pre-wrap;
  word-break: break-word;
  font-family: var(--editor-font-family, Consolas, monospace);
  font-size: 12px;
  line-height: 1.45;
}

.data-workbench__structure-empty {
  position: absolute;
  left: 16px;
  top: 58px;
}

.data-workbench__state.is-error {
  color: #d92d20;
}

.data-workbench__repr pre {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
}

.data-workbench__form-grid,
.data-workbench__chart-config {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(220px, 100%), 1fr));
  gap: 14px;
  min-width: 0;
}

.data-workbench__form-grid label,
.data-workbench__chart-config label {
  display: grid;
  gap: 6px;
  font-size: 12px;
  color: var(--dl-muted);
}

.data-workbench__form-grid label.wide,
.data-workbench__chart-config label.wide {
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
  overflow: auto;
  background: var(--dl-panel);
}

.data-workbench__chart-builder {
  display: grid;
  align-content: start;
  gap: 16px;
  padding: 16px;
  min-width: 0;
}

.data-workbench__chart-config,
.data-workbench__chart-series,
.data-workbench__chart-help {
  width: 100%;
  box-sizing: border-box;
  border: 1px solid var(--dl-border);
  border-radius: 8px;
  padding: 14px;
  background: var(--dl-panel);
}

.data-workbench__chart-section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 10px;
}

.data-workbench__chart-section-head h2 {
  margin: 0;
  font-size: 14px;
}

.data-workbench__series-list {
  display: grid;
  gap: 8px;
}

.data-workbench__series-row {
  display: grid;
  grid-template-columns: minmax(0, 1.1fr) minmax(0, 0.8fr) auto;
  gap: 8px;
  align-items: center;
}

.data-workbench__chart-help {
  display: grid;
  gap: 4px;
  color: var(--dl-muted);
  font-size: 12px;
}

.data-workbench__chart-help strong {
  color: var(--dl-text);
}

.data-workbench__operation-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 12px;
  padding: 16px;
}

.data-workbench__operation-card {
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

.data-workbench__inspector-section {
  padding: 10px 0;
  border-bottom: 1px solid var(--dl-border);
}

.data-workbench__inspector-section:first-child {
  padding-top: 0;
}

.data-workbench__inspector-section:last-child {
  border-bottom: 0;
}

.data-workbench__inspector-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}

.data-workbench__inspector-header h2 {
  margin: 0;
  font-size: 13px;
  line-height: 1.2;
}

.data-workbench__inspector-header span {
  min-width: 0;
  color: var(--dl-muted);
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.data-workbench__inspector-tools {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.data-workbench__metric-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
}

.data-workbench__metric {
  min-width: 0;
  display: grid;
  gap: 3px;
  border-radius: 7px;
  padding: 7px 8px;
  background: var(--dl-panel-2);
}

.data-workbench__metric span {
  color: var(--dl-muted);
  font-size: 11px;
}

.data-workbench__metric strong {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
}

.data-workbench__inspector-preview {
  margin: 0;
  max-height: 260px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
  border: 1px solid var(--dl-border);
  border-radius: 7px;
  padding: 10px;
  background: var(--dl-panel-2);
  font-family: var(--editor-font-family, Consolas, monospace);
  font-size: 12px;
  line-height: 1.45;
}

.data-workbench__schema-table,
.data-workbench__edit-list {
  display: grid;
  gap: 4px;
}

.data-workbench__schema-row,
.data-workbench__edit-row {
  min-width: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  padding: 6px 7px;
  border-radius: 6px;
  background: var(--dl-panel-2);
  font-size: 12px;
}

.data-workbench__schema-row span,
.data-workbench__edit-row span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.data-workbench__schema-row code,
.data-workbench__edit-row code {
  color: var(--dl-muted);
  font-family: var(--editor-font-family, Consolas, monospace);
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
