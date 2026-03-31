<template>
  <div class="guaxi-notebook" :class="themeClass">
    <div class="guaxi-notebook__toolbar">
      <div class="guaxi-notebook__toolbar-main">
        <button type="button" class="guaxi-notebook__toolbar-btn primary" @click="appendCell('python')">新增 Python Cell</button>
        <button type="button" class="guaxi-notebook__toolbar-btn" @click="appendCell('markdown')">新增 Markdown Cell</button>
      </div>
      <button type="button" class="guaxi-notebook__toolbar-btn ghost" @click="$emit('open-source')">打开源码视图</button>
    </div>

    <div v-for="(cell, index) in notebookCells" :key="cell.key" class="guaxi-notebook__cell" :class="`is-${cell.type}`">
      <div class="guaxi-notebook__cell-header">
        <div class="guaxi-notebook__cell-label">
          <span class="guaxi-notebook__cell-kind">{{ cell.type === 'python' ? `In [${index + 1}]` : `Markdown ${index + 1}` }}</span>
          <span class="guaxi-notebook__cell-meta">{{ cell.type === 'python' ? (cell.meta?.title || 'Python Cell') : 'Markdown Cell' }}</span>
        </div>
        <div class="guaxi-notebook__cell-actions">
          <button type="button" class="guaxi-notebook__cell-btn" @click="insertCell(index, 'above', 'markdown')">上方 Markdown</button>
          <button type="button" class="guaxi-notebook__cell-btn" @click="insertCell(index, 'above', 'python')">上方 Python</button>
          <button type="button" class="guaxi-notebook__cell-btn danger" @click="removeCell(cell.key)">删除</button>
        </div>
      </div>

      <template v-if="cell.type === 'markdown'">
        <div class="guaxi-notebook__markdown-layout">
          <textarea
            :data-cell-key="cell.key"
            class="guaxi-notebook__markdown-editor"
            spellcheck="false"
            :value="cell.content"
            @input="updateMarkdownCell(cell.key, $event)"
          />
          <div class="guaxi-notebook__markdown-preview markdown-body" v-html="renderMarkdownCell(cell.content)"></div>
        </div>
      </template>

      <template v-else>
        <div class="guaxi-notebook__python-actions">
          <button type="button" class="guaxi-notebook__cell-btn primary" :disabled="isCellBusy(cell)" @click="runPythonCell(cell, { writeBack: false })">运行</button>
          <button type="button" class="guaxi-notebook__cell-btn" :disabled="isCellBusy(cell)" @click="runPythonCell(cell, { writeBack: true })">运行并写回</button>
          <button type="button" class="guaxi-notebook__cell-btn" @click="toggleCellOutput(cell)">{{ getCellSnapshot(cell).meta.outputCollapsed ? '显示输出' : '隐藏输出' }}</button>
          <button type="button" class="guaxi-notebook__cell-btn" @click="toggleCellSession(cell)">{{ getCellSnapshot(cell).meta.sessionCollapsed ? '显示变量' : '隐藏变量' }}</button>
          <button type="button" class="guaxi-notebook__cell-btn" @click="$emit('open-console')">控制台</button>
          <button type="button" class="guaxi-notebook__cell-btn" @click="insertCell(index, 'below', 'python')">下方插入 Python</button>
          <button type="button" class="guaxi-notebook__cell-btn" @click="insertCell(index, 'below', 'markdown')">下方插入 Markdown</button>
        </div>

        <textarea
          :data-cell-key="cell.key"
          class="guaxi-notebook__python-editor"
          spellcheck="false"
          :value="cell.code"
          @input="updatePythonCell(cell.key, $event)"
        />

        <div class="guaxi-notebook__status-line">
          <span>{{ isCellBusy(cell) ? '运行中...' : '共享当前文档的 Python 会话' }}</span>
          <span>{{ getCellSnapshot(cell).outputText ? '已有输出' : '尚未运行' }}</span>
        </div>

        <div v-if="!getCellSnapshot(cell).meta.outputCollapsed" class="guaxi-notebook__panel">
          <div class="guaxi-notebook__panel-title">输出</div>
          <div v-if="getCellSnapshot(cell).display?.type === 'html'" class="guaxi-notebook__rich" v-html="String(getCellSnapshot(cell).display.html || '')"></div>
          <div v-else-if="getCellSnapshot(cell).display?.type === 'svg'" class="guaxi-notebook__rich" v-html="String(getCellSnapshot(cell).display.svg || '')"></div>
          <div
            v-else-if="getCellSnapshot(cell).display?.type === 'markdown'"
            class="guaxi-notebook__rich markdown-body"
            v-html="renderMarkdownCell(String(getCellSnapshot(cell).display.markdown || ''))"
          ></div>
          <img
            v-else-if="getCellImageUrl(cell)"
            class="guaxi-notebook__image"
            :src="getCellImageUrl(cell)"
            alt="Python Cell Output"
          />
          <div v-else-if="getCellSnapshot(cell).dataframe" class="guaxi-notebook__dataframe">
            <table>
              <thead>
                <tr>
                  <th v-for="column in getCellSnapshot(cell).dataframe.columns || []" :key="column">{{ column }}</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="(row, rowIndex) in getCellSnapshot(cell).dataframe.rows || []" :key="`${cell.key}-row-${rowIndex}`">
                  <td v-for="(value, colIndex) in row" :key="`${cell.key}-${rowIndex}-${colIndex}`">{{ value }}</td>
                </tr>
              </tbody>
            </table>
          </div>
          <pre v-if="getCellSnapshot(cell).outputText" class="guaxi-notebook__output-text">{{ getCellSnapshot(cell).outputText }}</pre>
          <div v-if="!hasVisibleOutput(cell)" class="guaxi-notebook__empty">当前单元还没有输出</div>
        </div>

        <div v-if="!getCellSnapshot(cell).meta.sessionCollapsed" class="guaxi-notebook__panel">
          <div class="guaxi-notebook__panel-title">会话变量</div>
          <div v-if="getCellSessionVars(cell).length" class="guaxi-notebook__vars">
            <div v-for="item in getCellSessionVars(cell)" :key="`${cell.key}-${item.name}`" class="guaxi-notebook__var">
              <div class="guaxi-notebook__var-main">
                <span class="guaxi-notebook__var-name">{{ item.name }}</span>
                <span class="guaxi-notebook__var-type">{{ item.value_type || item.type || 'value' }}</span>
              </div>
              <div class="guaxi-notebook__var-preview">{{ item.preview || '' }}</div>
            </div>
          </div>
          <div v-else class="guaxi-notebook__empty">当前会话里还没有可见变量</div>
        </div>
      </template>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, ref } from 'vue'
import { useMessage } from 'naive-ui'
import MarkdownIt from 'markdown-it'
import { invoke } from '@tauri-apps/api/core'
import {
  createPythonCellId,
  createPythonSessionId,
  parsePythonCellBlocks,
} from './utils/guaxiPythonCellSyntax'
import {
  createNotebookCellsFromMarkdown,
  createNotebookMarkdownCell,
  createNotebookPythonCell,
  normalizeNotebookCells,
  serializeNotebookCells,
} from './utils/guaxiNotebookCells'
import { getPythonCellSnapshot, patchPythonCellSnapshot } from './utils/guaxiPythonCellState'

const props = defineProps({
  markdown: {
    type: String,
    default: '',
  },
  filePath: {
    type: String,
    default: '',
  },
  theme: {
    type: String,
    default: 'light',
  },
})

const emit = defineEmits(['update:markdown', 'state-change', 'open-source', 'open-console'])

const message = useMessage()
const markdownRenderer = new MarkdownIt({
  html: true,
  breaks: true,
  linkify: true,
})

const refreshTick = ref(0)
const pendingFocusCellKey = ref('')
const busyMap = ref({})

const notebookCells = computed(() => createNotebookCellsFromMarkdown(props.markdown, {
  filePath: props.filePath,
}))

const themeClass = computed(() => (String(props.theme || '').toLowerCase() === 'dark' ? 'theme-dark' : 'theme-light'))

const bumpRefresh = () => {
  refreshTick.value += 1
  emit('state-change')
}

const emitCells = (cells, focusKey = '') => {
  pendingFocusCellKey.value = String(focusKey || '')
  emit('update:markdown', serializeNotebookCells(normalizeNotebookCells(cells)))
  nextTick(() => {
    if (!pendingFocusCellKey.value) return
    const target = document.querySelector(`[data-cell-key="${CSS.escape(pendingFocusCellKey.value)}"]`)
    if (target instanceof HTMLElement) {
      target.focus()
    }
    pendingFocusCellKey.value = ''
  })
}

const updateMarkdownCell = (key, event) => {
  const value = String(event?.target?.value || '')
  emitCells(
    notebookCells.value.map((cell) => {
      if (cell.type !== 'markdown' || String(cell.key) !== String(key)) return cell
      return createNotebookMarkdownCell(value, { key: cell.key })
    }),
  )
}

const updatePythonCell = (key, event) => {
  const value = String(event?.target?.value || '')
  emitCells(
    notebookCells.value.map((cell) => {
      if (cell.type !== 'python' || String(cell.key) !== String(key)) return cell
      return createNotebookPythonCell({
        ...cell,
        code: value,
      })
    }),
  )
}

const insertCell = (index, position = 'below', type = 'markdown') => {
  const next = notebookCells.value.slice()
  const newCell = type === 'python'
    ? createNotebookPythonCell({
      code: '# Python Cell\nprint("Hello from Guaxi Python Cell")',
    })
    : createNotebookMarkdownCell('')
  const offset = position === 'below' ? 1 : 0
  next.splice(Math.max(0, index + offset), 0, newCell)
  emitCells(next, newCell.key)
}

const appendCell = (type = 'markdown') => {
  insertCell(notebookCells.value.length - 1, 'below', type)
}

const removeCell = (key) => {
  const filtered = notebookCells.value.filter((cell) => String(cell.key) !== String(key))
  emitCells(filtered)
}

const formatPythonExecOutput = (res) => {
  const chunks = []
  const stdout = String(res?.stdout || '').trim()
  const stderr = String(res?.stderr || '').trim()
  const resultRepr = String(res?.result_repr || '').trim()
  if (stdout) chunks.push(stdout)
  if (stderr) chunks.push(stderr)
  if (resultRepr && !stdout.includes(resultRepr)) chunks.push(resultRepr)
  return chunks.join('\n\n').trim()
}

const getGuaxiDisplayFromResult = (res) => {
  const value = res?.result_json?.__guaxi_display__
  return value && typeof value === 'object' ? value : null
}

const getGuaxiDataFrameFromResult = (res) => {
  const value = res?.result_json?.__guaxi_dataframe__
  if (!value || typeof value !== 'object') return null
  return {
    columns: Array.isArray(value.columns) ? value.columns : [],
    rows: Array.isArray(value.rows) ? value.rows : [],
    rowCount: Number(value.row_count || 0),
    truncated: !!value.truncated,
  }
}

const getCellSnapshot = (cell) => {
  refreshTick.value
  const snapshot = getPythonCellSnapshot(props.filePath, cell.cellId)
  return {
    outputText: String(snapshot?.outputText || ''),
    display: snapshot?.display || null,
    dataframe: snapshot?.dataframe || null,
    result: snapshot?.result || null,
    sessionVars: Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
    meta: {
      ...(cell.meta || {}),
      ...(snapshot?.meta || {}),
      outputCollapsed: snapshot?.meta?.outputCollapsed ?? cell.meta?.outputCollapsed ?? false,
      sessionCollapsed: snapshot?.meta?.sessionCollapsed ?? cell.meta?.sessionCollapsed ?? true,
    },
  }
}

const getCellSessionVars = (cell) => Array.isArray(getCellSnapshot(cell).sessionVars) ? getCellSnapshot(cell).sessionVars : []

const getCellImageUrl = (cell) => {
  const display = getCellSnapshot(cell).display
  if (!display || display.type !== 'image') return ''
  if (display.dataUrl && String(display.dataUrl).startsWith('data:')) return String(display.dataUrl)
  if (display.data_base64) {
    return `data:${String(display.mime || 'image/png')};base64,${String(display.data_base64)}`
  }
  return ''
}

const hasVisibleOutput = (cell) => {
  const snapshot = getCellSnapshot(cell)
  return !!(
    snapshot.outputText ||
    snapshot.display ||
    snapshot.dataframe
  )
}

const patchCellMeta = (cell, patch) => {
  const snapshot = getCellSnapshot(cell)
  patchPythonCellSnapshot(props.filePath, cell.cellId, {
    meta: {
      ...(snapshot.meta || {}),
      ...(patch || {}),
    },
  })
  bumpRefresh()
}

const toggleCellOutput = (cell) => {
  const snapshot = getCellSnapshot(cell)
  patchCellMeta(cell, {
    outputCollapsed: !snapshot.meta.outputCollapsed,
  })
}

const toggleCellSession = (cell) => {
  const snapshot = getCellSnapshot(cell)
  patchCellMeta(cell, {
    sessionCollapsed: !snapshot.meta.sessionCollapsed,
  })
}

const isCellBusy = (cell) => !!busyMap.value[String(cell.key)]

const upsertPythonOutputBlock = (markdown, targetCellId, outputText) => {
  const current = String(markdown || '').replace(/\r\n?/g, '\n')
  const blocks = parsePythonCellBlocks(current)
  const targetBlock = blocks.find((block) => createPythonCellId(props.filePath, block) === targetCellId)
  if (!targetBlock) return current

  const marker = `<!-- guaxi:pyout:${targetCellId} -->`
  const lines = current.split('\n')
  const existingMarkerIndex = lines.findIndex((line) => String(line || '').trim() === marker)
  const replacement = [marker, '```text', String(outputText || ''), '```']

  if (existingMarkerIndex >= 0) {
    let fenceStart = -1
    let fenceEnd = -1
    for (let i = existingMarkerIndex + 1; i < lines.length; i += 1) {
      if (String(lines[i] || '').trim().startsWith('```')) {
        fenceStart = i
        break
      }
    }
    if (fenceStart >= 0) {
      for (let i = fenceStart + 1; i < lines.length; i += 1) {
        if (String(lines[i] || '').trim() === '```') {
          fenceEnd = i
          break
        }
      }
    }
    if (fenceStart >= 0 && fenceEnd >= 0) {
      lines.splice(existingMarkerIndex, fenceEnd - existingMarkerIndex + 1, ...replacement)
      return `${lines.join('\n')}\n`
    }
  }

  const insertIndex = Math.max(0, Number(targetBlock.endLine || 1))
  lines.splice(insertIndex, 0, '', ...replacement)
  return `${lines.join('\n')}\n`
}

const runPythonCell = async (cell, { writeBack = false } = {}) => {
  if (!cell?.code) {
    message.warning('Python 代码为空')
    return
  }

  busyMap.value = {
    ...busyMap.value,
    [cell.key]: true,
  }

  try {
    const sessionId = createPythonSessionId(props.filePath)
    const res = await invoke('python_exec', {
      request: {
        code: String(cell.code || ''),
        persist: true,
        expect_json: true,
        vars: {
          _guaxi_session: sessionId,
          _doc_path: String(props.filePath || ''),
          _py_cell_id: String(cell.cellId || ''),
        },
      },
    })

    const outputText = formatPythonExecOutput(res)
    const hideOutput = !String(outputText || '').trim()

    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      outputText: hideOutput ? '' : outputText,
      display: getGuaxiDisplayFromResult(res),
      dataframe: getGuaxiDataFrameFromResult(res),
      result: res,
      sessionVars: Array.isArray(res?.session_vars) ? res.session_vars : [],
      meta: {
        ...(getCellSnapshot(cell).meta || {}),
        outputCollapsed: hideOutput,
      },
    })
    bumpRefresh()

    if (writeBack && !hideOutput) {
      emit('update:markdown', upsertPythonOutputBlock(props.markdown, cell.cellId, outputText))
    }

    if (res?.ok === false) {
      message.error('Python 执行失败')
      return
    }

    message.success(writeBack ? '运行完成，输出已写回文档' : '运行完成')
  } catch (error) {
    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      outputText: `运行失败：${String(error?.message || error)}`,
      display: null,
      dataframe: null,
      result: null,
      sessionVars: [],
      meta: {
        ...(getCellSnapshot(cell).meta || {}),
        outputCollapsed: false,
      },
    })
    bumpRefresh()
    message.error(`运行失败：${String(error?.message || error)}`)
  } finally {
    busyMap.value = {
      ...busyMap.value,
      [cell.key]: false,
    }
  }
}

const renderMarkdownCell = (content) => markdownRenderer.render(String(content || ''))

defineExpose({
  insertCell(type = 'python') {
    appendCell(type)
  },
})
</script>

<style scoped>
.guaxi-notebook {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-height: 100%;
  padding: 18px;
  background: var(--guaxi-notebook-bg, #f6f8fc);
  color: var(--guaxi-notebook-text, #172033);
}

.guaxi-notebook.theme-dark {
  --guaxi-notebook-bg: #0f172a;
  --guaxi-notebook-panel: #111c34;
  --guaxi-notebook-panel-2: #0b1427;
  --guaxi-notebook-border: rgba(148, 163, 184, 0.18);
  --guaxi-notebook-text: #e5edf8;
  --guaxi-notebook-muted: #97a8c5;
  --guaxi-notebook-btn: #1b2945;
  --guaxi-notebook-btn-hover: #223454;
  --guaxi-notebook-primary: #315efb;
  --guaxi-notebook-primary-hover: #4b74ff;
  --guaxi-notebook-danger: #7f1d1d;
}

.guaxi-notebook.theme-light {
  --guaxi-notebook-bg: #f7f9fc;
  --guaxi-notebook-panel: #ffffff;
  --guaxi-notebook-panel-2: #f8fbff;
  --guaxi-notebook-border: rgba(15, 23, 42, 0.1);
  --guaxi-notebook-text: #172033;
  --guaxi-notebook-muted: #61718f;
  --guaxi-notebook-btn: #eef3fb;
  --guaxi-notebook-btn-hover: #e3ebf8;
  --guaxi-notebook-primary: #2563eb;
  --guaxi-notebook-primary-hover: #1d4ed8;
  --guaxi-notebook-danger: #fee2e2;
}

.guaxi-notebook__toolbar,
.guaxi-notebook__cell,
.guaxi-notebook__panel {
  border: 1px solid var(--guaxi-notebook-border);
  border-radius: 16px;
  background: var(--guaxi-notebook-panel);
  box-shadow: 0 12px 28px rgba(15, 23, 42, 0.08);
}

.guaxi-notebook__toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
}

.guaxi-notebook__toolbar-main,
.guaxi-notebook__cell-actions,
.guaxi-notebook__python-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.guaxi-notebook__cell {
  overflow: hidden;
}

.guaxi-notebook__cell-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 16px;
  border-bottom: 1px solid var(--guaxi-notebook-border);
  background: var(--guaxi-notebook-panel-2);
}

.guaxi-notebook__cell-label {
  display: flex;
  align-items: center;
  gap: 10px;
}

.guaxi-notebook__cell-kind {
  font: 700 12px/1 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  color: #f8fafc;
  background: linear-gradient(135deg, var(--guaxi-notebook-primary), var(--guaxi-notebook-primary-hover));
  padding: 6px 9px;
  border-radius: 999px;
}

.guaxi-notebook__cell-meta,
.guaxi-notebook__status-line,
.guaxi-notebook__empty,
.guaxi-notebook__var-preview {
  color: var(--guaxi-notebook-muted);
}

.guaxi-notebook__markdown-layout {
  display: grid;
  grid-template-columns: minmax(0, 1.05fr) minmax(0, 0.95fr);
  gap: 0;
}

.guaxi-notebook__markdown-editor,
.guaxi-notebook__python-editor {
  width: 100%;
  min-height: 160px;
  border: 0;
  outline: none;
  resize: vertical;
  padding: 16px 18px;
  color: var(--guaxi-notebook-text);
  background: transparent;
  font: 14px/1.7 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  box-sizing: border-box;
}

.guaxi-notebook__markdown-preview {
  min-height: 160px;
  padding: 16px 18px;
  border-left: 1px solid var(--guaxi-notebook-border);
  background: var(--guaxi-notebook-panel-2);
  overflow: auto;
}

.guaxi-notebook__python-actions,
.guaxi-notebook__status-line {
  padding: 12px 16px 0;
}

.guaxi-notebook__status-line {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding-bottom: 12px;
}

.guaxi-notebook__cell-btn,
.guaxi-notebook__toolbar-btn {
  border: 1px solid var(--guaxi-notebook-border);
  border-radius: 10px;
  background: var(--guaxi-notebook-btn);
  color: var(--guaxi-notebook-text);
  padding: 7px 11px;
  cursor: pointer;
  transition: background 0.18s ease, border-color 0.18s ease, transform 0.18s ease;
}

.guaxi-notebook__cell-btn:hover,
.guaxi-notebook__toolbar-btn:hover {
  background: var(--guaxi-notebook-btn-hover);
  transform: translateY(-1px);
}

.guaxi-notebook__cell-btn.primary,
.guaxi-notebook__toolbar-btn.primary {
  color: #ffffff;
  border-color: transparent;
  background: linear-gradient(135deg, var(--guaxi-notebook-primary), var(--guaxi-notebook-primary-hover));
}

.guaxi-notebook__cell-btn.ghost,
.guaxi-notebook__toolbar-btn.ghost {
  background: transparent;
}

.guaxi-notebook__cell-btn.danger {
  background: var(--guaxi-notebook-danger);
}

.guaxi-notebook__panel {
  margin: 0 16px 16px;
  padding: 14px 16px;
}

.guaxi-notebook__panel-title {
  margin-bottom: 10px;
  font-weight: 700;
}

.guaxi-notebook__output-text,
.guaxi-notebook__rich,
.guaxi-notebook__dataframe {
  margin: 0;
  border-radius: 12px;
  border: 1px solid var(--guaxi-notebook-border);
  background: var(--guaxi-notebook-panel-2);
  padding: 12px 14px;
  overflow: auto;
}

.guaxi-notebook__output-text {
  white-space: pre-wrap;
}

.guaxi-notebook__dataframe table {
  width: 100%;
  border-collapse: collapse;
}

.guaxi-notebook__dataframe th,
.guaxi-notebook__dataframe td {
  padding: 8px 10px;
  border-bottom: 1px solid var(--guaxi-notebook-border);
  text-align: left;
}

.guaxi-notebook__image {
  display: block;
  max-width: 100%;
  border-radius: 12px;
}

.guaxi-notebook__vars {
  display: grid;
  gap: 10px;
}

.guaxi-notebook__var {
  padding: 10px 12px;
  border-radius: 12px;
  background: var(--guaxi-notebook-panel-2);
  border: 1px solid var(--guaxi-notebook-border);
}

.guaxi-notebook__var-main {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.guaxi-notebook__var-name {
  font-weight: 700;
}

.guaxi-notebook__var-type {
  font-size: 12px;
  color: var(--guaxi-notebook-muted);
}

@media (max-width: 1100px) {
  .guaxi-notebook__markdown-layout {
    grid-template-columns: 1fr;
  }

  .guaxi-notebook__markdown-preview {
    border-left: 0;
    border-top: 1px solid var(--guaxi-notebook-border);
  }

  .guaxi-notebook__toolbar,
  .guaxi-notebook__cell-header,
  .guaxi-notebook__status-line {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
