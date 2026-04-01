<template>
  <div ref="notebookRootRef" class="guaxi-notebook" :class="themeClass">
    <div class="guaxi-notebook__toolbar">
      <div class="guaxi-notebook__toolbar-copy">
        <div class="guaxi-notebook__toolbar-title">探索者模式</div>
        <div class="guaxi-notebook__toolbar-subtitle">按文档结构拆分 Markdown Cell，保持单栏 Notebook 编辑体验</div>
      </div>
      <div class="guaxi-notebook__toolbar-main">
        <button type="button" class="guaxi-notebook__toolbar-btn primary" @click="appendCell('python')">新增 Python Cell</button>
        <button type="button" class="guaxi-notebook__toolbar-btn" @click="appendCell('markdown')">新增 Markdown Cell</button>
      </div>
    </div>

    <div v-for="(cell, index) in notebookCells" :key="cell.key" class="guaxi-notebook__cell" :class="`is-${cell.type}`">
      <div class="guaxi-notebook__cell-header">
        <div class="guaxi-notebook__cell-label">
          <span class="guaxi-notebook__cell-kind">{{ cell.type === 'python' ? `In [${index + 1}]` : `Markdown ${index + 1}` }}</span>
          <span class="guaxi-notebook__cell-meta">{{ cell.type === 'python' ? (cell.meta?.title || 'Python Cell') : getMarkdownCellTitle(cell) }}</span>
        </div>
        <div class="guaxi-notebook__cell-actions">
          <button type="button" class="guaxi-notebook__cell-btn" @click="insertCell(index, 'above', 'markdown')">上方 Markdown</button>
          <button type="button" class="guaxi-notebook__cell-btn" @click="insertCell(index, 'above', 'python')">上方 Python</button>
          <button type="button" class="guaxi-notebook__cell-btn" @click="insertCell(index, 'below', 'markdown')">下方 Markdown</button>
          <button type="button" class="guaxi-notebook__cell-btn" @click="insertCell(index, 'below', 'python')">下方 Python</button>
          <button type="button" class="guaxi-notebook__cell-btn danger" @click="removeCell(cell.key)">删除</button>
        </div>
      </div>

      <template v-if="cell.type === 'markdown'">
        <div class="guaxi-notebook__markdown-layout">
          <div v-if="isMarkdownCellEditing(cell)" class="guaxi-notebook__markdown-edit-shell">
            <div class="guaxi-notebook__markdown-edit-bar">
              <span class="guaxi-notebook__markdown-edit-meta">Markdown 原文编辑中</span>
              <button type="button" class="guaxi-notebook__cell-btn" @click="exitMarkdownEditMode(cell.key, 'toolbar-finish')">完成</button>
            </div>
            <textarea
              :data-cell-key="cell.key"
              class="guaxi-notebook__markdown-editor"
              spellcheck="false"
              :value="cell.content"
              @input="updateMarkdownCell(cell.key, $event)"
              @keydown.esc.prevent="exitMarkdownEditMode(cell.key, 'escape')"
              @keydown.ctrl.enter.prevent="exitMarkdownEditMode(cell.key, 'ctrl-enter')"
            />
          </div>
          <div
            v-else
            class="guaxi-notebook__markdown-preview-shell is-clickable"
            role="button"
            tabindex="0"
            @click="enterMarkdownEditMode(cell.key, 'preview-click')"
            @keydown.enter.prevent="enterMarkdownEditMode(cell.key, 'preview-enter')"
            @keydown.space.prevent="enterMarkdownEditMode(cell.key, 'preview-space')"
          >
            <div class="guaxi-notebook__markdown-preview-bar">
              <span class="guaxi-notebook__panel-title">预览</span>
              <span class="guaxi-notebook__markdown-preview-hint">点击进入编辑</span>
            </div>
            <div
              v-if="String(cell.content || '').trim()"
              class="guaxi-notebook__markdown-preview markdown-body"
              v-html="getRenderedMarkdownCellHtml(cell)"
            ></div>
            <div v-else class="guaxi-notebook__empty guaxi-notebook__empty--dashed">点击这里开始编写 Markdown 内容</div>
          </div>
        </div>
      </template>

      <template v-else>
        <div class="guaxi-notebook__python-actions">
          <button type="button" class="guaxi-notebook__cell-btn primary" :disabled="isCellBusy(cell)" @click="runPythonCell(cell, { writeBack: false })">运行</button>
          <button type="button" class="guaxi-notebook__cell-btn" :disabled="isCellBusy(cell)" @click="runPythonCell(cell, { writeBack: true })">运行并写回</button>
          <button v-if="hasVisibleOutput(cell)" type="button" class="guaxi-notebook__cell-btn" @click="toggleCellOutput(cell)">{{ getCellSnapshot(cell).meta.outputCollapsed ? '显示输出' : '隐藏输出' }}</button>
          <button type="button" class="guaxi-notebook__cell-btn" @click="toggleCellSession(cell)">{{ getCellSnapshot(cell).meta.sessionCollapsed ? '显示变量' : '隐藏变量' }}</button>
          <button type="button" class="guaxi-notebook__cell-btn" @click="$emit('open-console')">控制台</button>
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

        <div v-if="hasVisibleOutput(cell) && !getCellSnapshot(cell).meta.outputCollapsed" class="guaxi-notebook__panel">
          <div class="guaxi-notebook__panel-title">输出</div>
          <div v-if="getCellSnapshot(cell).display?.type === 'html'" class="guaxi-notebook__rich" v-html="String(getCellSnapshot(cell).display.html || '')"></div>
          <div v-else-if="getCellSnapshot(cell).display?.type === 'svg'" class="guaxi-notebook__rich" v-html="String(getCellSnapshot(cell).display.svg || '')"></div>
          <div
            v-else-if="getCellSnapshot(cell).display?.type === 'markdown'"
            class="guaxi-notebook__rich markdown-body"
            v-html="getRenderedOutputMarkdownHtml(cell)"
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
import { computed, nextTick, ref, watch } from 'vue'
import { useMessage } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../../utils/logger'
import { renderMarkdown } from '../GuaxiMarkdownRender/GuaxiMarker'
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

const emit = defineEmits(['update:markdown', 'state-change', 'open-console'])

const message = useMessage()
const logger = createLogger('GuaxiNotebookView')

const refreshTick = ref(0)
const pendingFocusCellKey = ref('')
const busyMap = ref({})
const notebookRootRef = ref(null)
const editingMarkdownCellKey = ref('')
const renderedMarkdownHtmlMap = ref({})
const renderedOutputMarkdownHtmlMap = ref({})
const markdownRenderVersion = ref(0)

const notebookCells = computed(() => createNotebookCellsFromMarkdown(props.markdown, {
  filePath: props.filePath,
}))

const themeClass = computed(() => (String(props.theme || '').toLowerCase() === 'dark' ? 'theme-dark' : 'theme-light'))

const bumpRefresh = () => {
  refreshTick.value += 1
  emit('state-change')
}

const getNotebookBasePath = () => {
  const filePath = String(props.filePath || '').trim()
  if (!filePath) return ''
  const normalized = filePath.replace(/\\/g, '/')
  const lastSlashIndex = normalized.lastIndexOf('/')
  return lastSlashIndex >= 0 ? normalized.slice(0, lastSlashIndex) : ''
}

const buildMarkdownRenderOptions = () => ({
  preserveOriginal: false,
  mermaid: true,
  math: true,
  codeHighlight: true,
  buttons: false,
  pythonCells: false,
  codeCells: false,
  theme: props.theme,
  basePath: getNotebookBasePath(),
})

const getMarkdownCellTitle = (cell) => {
  const lines = String(cell?.content || '').split('\n')
  const firstMeaningfulLine = lines.find((line) => String(line || '').trim())
  if (!firstMeaningfulLine) return '空白 Markdown Cell'

  const normalized = String(firstMeaningfulLine)
    .replace(/^\s{0,3}#{1,6}\s*/, '')
    .replace(/^\s*[-*+]\s+/, '')
    .replace(/^\s*\d+\.\s+/, '')
    .trim()

  if (!normalized) return 'Markdown Cell'
  return normalized.length > 42 ? `${normalized.slice(0, 42)}...` : normalized
}

const resizeTextareaToContent = (target) => {
  if (!(target instanceof HTMLTextAreaElement)) return
  target.style.height = 'auto'
  target.style.overflowY = 'hidden'
  const nextHeight = Math.max(target.scrollHeight, 88)
  target.style.height = `${nextHeight}px`
}

const syncNotebookTextareaHeights = (reason = 'unknown') => {
  const root = notebookRootRef.value
  if (!(root instanceof HTMLElement)) return
  root.querySelectorAll('textarea').forEach((node) => {
    resizeTextareaToContent(node)
  })
}

const focusMarkdownEditor = (key, reason = 'focus-editor') => {
  nextTick(() => {
    const target = notebookRootRef.value?.querySelector?.(`[data-cell-key="${CSS.escape(String(key || ''))}"]`)
    if (target instanceof HTMLTextAreaElement) {
      resizeTextareaToContent(target)
      target.focus()
      target.setSelectionRange(target.value.length, target.value.length)
      logger.info('[Notebook] focus markdown editor', {
        key: String(key || ''),
        reason,
      })
    }
  })
}

const isMarkdownCellEditing = (cell) => {
  return cell?.type === 'markdown' && String(editingMarkdownCellKey.value || '') === String(cell?.key || '')
}

const enterMarkdownEditMode = (key, reason = 'manual-enter') => {
  const nextKey = String(key || '')
  if (!nextKey) return
  editingMarkdownCellKey.value = nextKey
  logger.info('[Notebook] enter markdown edit mode', {
    key: nextKey,
    reason,
  })
  focusMarkdownEditor(nextKey, reason)
}

const exitMarkdownEditMode = (key, reason = 'manual-exit') => {
  const targetKey = String(key || '')
  if (String(editingMarkdownCellKey.value || '') !== targetKey) return
  editingMarkdownCellKey.value = ''
  logger.info('[Notebook] exit markdown edit mode', {
    key: targetKey,
    reason,
  })
}

const emitCells = (cells, focusKey = '') => {
  pendingFocusCellKey.value = String(focusKey || '')
  const normalizedCells = normalizeNotebookCells(cells)
  logger.info('[Notebook] emit cells', {
    focusKey: pendingFocusCellKey.value,
    cellCount: normalizedCells.length,
    markdownCellCount: normalizedCells.filter((cell) => cell.type === 'markdown').length,
    pythonCellCount: normalizedCells.filter((cell) => cell.type === 'python').length,
  })
  emit('update:markdown', serializeNotebookCells(normalizedCells))
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
  resizeTextareaToContent(event?.target)
  const value = String(event?.target?.value || '')
  emitCells(
    notebookCells.value.map((cell) => {
      if (cell.type !== 'markdown' || String(cell.key) !== String(key)) return cell
      return createNotebookMarkdownCell(value, { key: cell.key })
    }),
  )
}

const updatePythonCell = (key, event) => {
  resizeTextareaToContent(event?.target)
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
  logger.info('[Notebook] insert cell', {
    index,
    position,
    type,
  })
  if (type === 'markdown') {
    editingMarkdownCellKey.value = newCell.key
  }
  next.splice(Math.max(0, index + offset), 0, newCell)
  emitCells(next, newCell.key)
  if (type === 'markdown') {
    focusMarkdownEditor(newCell.key, 'insert-markdown-cell')
  }
}

const appendCell = (type = 'markdown') => {
  insertCell(notebookCells.value.length - 1, 'below', type)
}

const removeCell = (key) => {
  logger.info('[Notebook] remove cell', { key: String(key || '') })
  if (String(editingMarkdownCellKey.value || '') === String(key || '')) {
    editingMarkdownCellKey.value = ''
  }
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

const renderMarkdownHtml = async (content, reason = 'unknown') => {
  const markdownText = String(content || '')
  if (!markdownText.trim()) return ''
  try {
    return await renderMarkdown(markdownText, buildMarkdownRenderOptions())
  } catch (error) {
    logger.error('[Notebook] render markdown html failed', {
      reason,
      error: String(error?.message || error),
    })
    return markdownText
  }
}

// Explorer 模式中的 Markdown 预览需要复用主编辑器渲染链，
// 这样代码高亮、数学公式与其他增强能力才能和主界面保持一致。
const syncRenderedMarkdownHtml = async (cells, reason = 'unknown') => {
  const renderVersion = markdownRenderVersion.value + 1
  markdownRenderVersion.value = renderVersion

  logger.info('[Notebook] sync rendered markdown start', {
    reason,
    renderVersion,
    cellCount: Array.isArray(cells) ? cells.length : 0,
  })

  const nextMarkdownMap = {}
  const nextOutputMap = {}
  const renderTasks = []

  for (const cell of Array.isArray(cells) ? cells : []) {
    if (cell?.type === 'markdown') {
      renderTasks.push((async () => {
        nextMarkdownMap[String(cell.key || '')] = await renderMarkdownHtml(
          cell.content,
          `markdown-cell:${String(cell.key || '')}`,
        )
      })())
      continue
    }

    if (cell?.type === 'python') {
      const snapshot = getCellSnapshot(cell)
      const outputMarkdown = snapshot?.display?.type === 'markdown'
        ? String(snapshot.display.markdown || '')
        : ''
      if (!outputMarkdown.trim()) continue

      renderTasks.push((async () => {
        nextOutputMap[String(cell.key || '')] = await renderMarkdownHtml(
          outputMarkdown,
          `python-output:${String(cell.key || '')}`,
        )
      })())
    }
  }

  await Promise.all(renderTasks)

  if (renderVersion !== markdownRenderVersion.value) {
    logger.info('[Notebook] skip stale rendered markdown result', {
      reason,
      renderVersion,
      latestRenderVersion: markdownRenderVersion.value,
    })
    return
  }

  renderedMarkdownHtmlMap.value = nextMarkdownMap
  renderedOutputMarkdownHtmlMap.value = nextOutputMap

  logger.info('[Notebook] sync rendered markdown finished', {
    reason,
    renderVersion,
    markdownCellCount: Object.keys(nextMarkdownMap).length,
    outputMarkdownCount: Object.keys(nextOutputMap).length,
  })
}

const getRenderedMarkdownCellHtml = (cell) => String(renderedMarkdownHtmlMap.value[String(cell?.key || '')] || '')

const getRenderedOutputMarkdownHtml = (cell) => String(renderedOutputMarkdownHtmlMap.value[String(cell?.key || '')] || '')

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
  logger.info('[Notebook] toggle output panel', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    nextCollapsed: !snapshot.meta.outputCollapsed,
  })
  patchCellMeta(cell, {
    outputCollapsed: !snapshot.meta.outputCollapsed,
  })
}

const toggleCellSession = (cell) => {
  const snapshot = getCellSnapshot(cell)
  logger.info('[Notebook] toggle session panel', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    nextCollapsed: !snapshot.meta.sessionCollapsed,
  })
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
    const display = getGuaxiDisplayFromResult(res)
    const dataframe = getGuaxiDataFrameFromResult(res)
    const hasRenderableOutput = !!(String(outputText || '').trim() || display || dataframe)
    const hideOutput = !hasRenderableOutput
    logger.info('[Notebook] python cell executed', {
      key: String(cell?.key || ''),
      cellId: String(cell?.cellId || ''),
      writeBack,
      hasTextOutput: !!String(outputText || '').trim(),
      hasDisplay: !!display,
      hasDataframe: !!dataframe,
      hideOutput,
    })

    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      outputText: hideOutput ? '' : outputText,
      display,
      dataframe,
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

watch(
  notebookCells,
  (cells) => {
    const keys = new Set(cells.map((cell) => String(cell.key || '')))
    if (editingMarkdownCellKey.value && !keys.has(String(editingMarkdownCellKey.value))) {
      editingMarkdownCellKey.value = ''
    }
    syncRenderedMarkdownHtml(cells, 'notebook-cells-watch')
    nextTick(() => {
      syncNotebookTextareaHeights('notebook-cells-changed')
    })
  },
  { deep: true, immediate: true },
)

watch(
  [() => props.theme, () => props.filePath, refreshTick],
  () => {
    syncRenderedMarkdownHtml(notebookCells.value, 'theme-file-refresh-watch')
  },
)

watch(editingMarkdownCellKey, (nextKey) => {
  if (!nextKey) return
  focusMarkdownEditor(nextKey, 'editing-markdown-cell-key')
})

defineExpose({
  insertCell(type = 'python') {
    appendCell(type)
  },
})
</script>

<style scoped>
.guaxi-notebook {
  --guaxi-notebook-bg: var(--guaxi-content-bg, #f7f9fc);
  --guaxi-notebook-panel: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 94%, var(--guaxi-content-bg, #f7f9fc) 6%);
  --guaxi-notebook-panel-muted: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 86%, var(--guaxi-content-bg, #f7f9fc) 14%);
  --guaxi-notebook-panel-soft: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 78%, var(--guaxi-content-bg, #f7f9fc) 22%);
  --guaxi-notebook-border: color-mix(in srgb, var(--guaxi-border, rgba(15, 23, 42, 0.1)) 86%, transparent);
  --guaxi-notebook-strong-border: color-mix(in srgb, var(--guaxi-border, rgba(15, 23, 42, 0.1)) 100%, rgba(15, 23, 42, 0.04));
  --guaxi-notebook-text: var(--guaxi-text-primary, #172033);
  --guaxi-notebook-muted: var(--guaxi-text-secondary, rgba(23, 32, 51, 0.72));
  --guaxi-notebook-btn: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 82%, var(--guaxi-content-bg, #f7f9fc) 18%);
  --guaxi-notebook-btn-hover: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 70%, var(--guaxi-content-bg, #f7f9fc) 30%);
  --guaxi-notebook-primary: #3b82f6;
  --guaxi-notebook-primary-hover: #2563eb;
  --guaxi-notebook-danger: color-mix(in srgb, #ef4444 14%, var(--guaxi-panel-bg, #ffffff) 86%);
  --guaxi-notebook-focus-ring: rgba(59, 130, 246, 0.2);
  --guaxi-notebook-shadow: 0 10px 28px rgba(15, 23, 42, 0.06);
  --guaxi-notebook-font: var(--guaxi-font-family, 'Microsoft YaHei UI', 'PingFang SC', sans-serif);
  --guaxi-notebook-mono: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-height: 100%;
  padding: 18px;
  background: var(--guaxi-notebook-bg, #f6f8fc);
  color: var(--guaxi-notebook-text, #172033);
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook.theme-dark {
  --guaxi-notebook-panel: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 96%, white 4%);
  --guaxi-notebook-panel-muted: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 90%, white 10%);
  --guaxi-notebook-panel-soft: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 84%, white 16%);
  --guaxi-notebook-border: color-mix(in srgb, var(--guaxi-border, rgba(100, 116, 139, 0.35)) 90%, transparent);
  --guaxi-notebook-strong-border: color-mix(in srgb, var(--guaxi-border, rgba(100, 116, 139, 0.35)) 100%, rgba(255, 255, 255, 0.05));
  --guaxi-notebook-muted: var(--guaxi-text-secondary, rgba(230, 236, 243, 0.72));
  --guaxi-notebook-btn: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 82%, white 18%);
  --guaxi-notebook-btn-hover: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 72%, white 28%);
  --guaxi-notebook-primary: #60a5fa;
  --guaxi-notebook-primary-hover: #3b82f6;
  --guaxi-notebook-danger: color-mix(in srgb, #ef4444 20%, var(--guaxi-panel-bg, #21252b) 80%);
  --guaxi-notebook-focus-ring: rgba(96, 165, 250, 0.24);
  --guaxi-notebook-shadow: 0 14px 34px rgba(2, 6, 23, 0.24);
}

.guaxi-notebook.theme-light {
  --guaxi-notebook-primary: #2563eb;
  --guaxi-notebook-primary-hover: #1d4ed8;
  --guaxi-notebook-danger: color-mix(in srgb, #ef4444 14%, var(--guaxi-panel-bg, #ffffff) 86%);
  --guaxi-notebook-focus-ring: rgba(37, 99, 235, 0.18);
}

.guaxi-notebook__toolbar,
.guaxi-notebook__cell,
.guaxi-notebook__panel {
  border: 1px solid var(--guaxi-notebook-strong-border);
  border-radius: 18px;
  background: var(--guaxi-notebook-panel);
  box-shadow: var(--guaxi-notebook-shadow);
}

.guaxi-notebook__toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  flex-wrap: wrap;
  padding: 14px 16px;
}

.guaxi-notebook__toolbar-copy {
  display: grid;
  gap: 4px;
}

.guaxi-notebook__toolbar-title {
  font-family: var(--guaxi-notebook-font);
  font-size: 15px;
  line-height: 1.25;
  font-weight: 700;
}

.guaxi-notebook__toolbar-subtitle {
  color: var(--guaxi-notebook-muted);
  font-size: 12px;
  font-family: var(--guaxi-notebook-font);
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
  background: linear-gradient(180deg, var(--guaxi-notebook-panel-muted), var(--guaxi-notebook-panel));
}

.guaxi-notebook__cell-label {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
}

.guaxi-notebook__cell-kind {
  font-family: var(--guaxi-notebook-font);
  font-size: 12px;
  line-height: 1;
  font-weight: 700;
  color: var(--guaxi-notebook-primary);
  background: color-mix(in srgb, var(--guaxi-notebook-primary) 12%, transparent);
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-primary) 24%, transparent);
  padding: 6px 10px;
  border-radius: 999px;
}

.guaxi-notebook__cell-meta,
.guaxi-notebook__status-line,
.guaxi-notebook__empty,
.guaxi-notebook__var-preview {
  color: var(--guaxi-notebook-muted);
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__markdown-layout {
  display: grid;
  gap: 12px;
  padding: 14px 16px 16px;
}

.guaxi-notebook__markdown-edit-shell {
  display: grid;
  gap: 10px;
}

.guaxi-notebook__markdown-edit-bar,
.guaxi-notebook__markdown-preview-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.guaxi-notebook__markdown-edit-meta,
.guaxi-notebook__markdown-preview-hint {
  color: var(--guaxi-notebook-muted);
  font-size: 12px;
}

.guaxi-notebook__markdown-editor,
.guaxi-notebook__python-editor {
  width: 100%;
  min-height: 88px;
  border: 1px solid var(--guaxi-notebook-strong-border);
  border-radius: 14px;
  outline: none;
  resize: none;
  padding: 16px 18px;
  color: var(--guaxi-notebook-text);
  background: var(--guaxi-notebook-panel-soft);
  font-size: 14px;
  line-height: 1.7;
  box-sizing: border-box;
  overflow-y: hidden;
}

.guaxi-notebook__markdown-editor {
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__python-editor,
.guaxi-notebook__output-text {
  font-family: var(--guaxi-notebook-mono);
}

.guaxi-notebook__markdown-editor:focus,
.guaxi-notebook__python-editor:focus,
.guaxi-notebook__markdown-preview-shell.is-clickable:focus-visible {
  border-color: color-mix(in srgb, var(--guaxi-notebook-primary) 62%, var(--guaxi-notebook-border) 38%);
  box-shadow: 0 0 0 3px var(--guaxi-notebook-focus-ring);
}

.guaxi-notebook__markdown-preview-shell {
  border: 1px solid var(--guaxi-notebook-strong-border);
  border-radius: 14px;
  background: var(--guaxi-notebook-panel);
  padding: 12px 14px 14px;
}

.guaxi-notebook__markdown-preview-shell.is-clickable {
  cursor: text;
}

.guaxi-notebook__markdown-preview {
  min-height: 0;
  padding: 0;
  overflow: visible;
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
  border: 1px solid var(--guaxi-notebook-strong-border);
  border-radius: 10px;
  background: var(--guaxi-notebook-btn);
  color: var(--guaxi-notebook-text);
  padding: 7px 11px;
  cursor: pointer;
  transition: background 0.18s ease, border-color 0.18s ease, transform 0.18s ease;
  font-family: var(--guaxi-notebook-font);
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
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__markdown-preview-bar .guaxi-notebook__panel-title {
  margin-bottom: 0;
}

.guaxi-notebook__output-text,
.guaxi-notebook__rich,
.guaxi-notebook__dataframe {
  margin: 0;
  border-radius: 14px;
  border: 1px solid var(--guaxi-notebook-strong-border);
  background: var(--guaxi-notebook-panel-soft);
  padding: 12px 14px;
  overflow-x: auto;
  overflow-y: visible;
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

.guaxi-notebook__empty--dashed {
  min-height: 56px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed var(--guaxi-notebook-border);
  border-radius: 12px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 90%, var(--guaxi-notebook-panel-soft) 10%);
}

.guaxi-notebook__vars {
  display: grid;
  gap: 10px;
}

.guaxi-notebook__var {
  padding: 10px 12px;
  border-radius: 12px;
  background: var(--guaxi-notebook-panel-soft);
  border: 1px solid var(--guaxi-notebook-strong-border);
}

.guaxi-notebook__var-main {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.guaxi-notebook__var-name {
  font-weight: 700;
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__var-type {
  font-size: 12px;
  color: var(--guaxi-notebook-muted);
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__markdown-preview :deep(.md-paragraph),
.guaxi-notebook__rich :deep(.md-paragraph) {
  color: var(--guaxi-notebook-text);
}

.guaxi-notebook__markdown-preview :deep(pre),
.guaxi-notebook__rich :deep(pre) {
  max-width: 100%;
}

.guaxi-notebook__markdown-preview :deep(.hljs),
.guaxi-notebook__rich :deep(.hljs) {
  border-radius: 10px;
}

.guaxi-notebook__markdown-preview :deep(mjx-container),
.guaxi-notebook__rich :deep(mjx-container) {
  overflow-x: auto;
  overflow-y: hidden;
}

.guaxi-notebook__markdown-preview :deep(.copy-button),
.guaxi-notebook__rich :deep(.copy-button) {
  opacity: 0.92;
}

@media (max-width: 1100px) {
  .guaxi-notebook__toolbar,
  .guaxi-notebook__cell-header,
  .guaxi-notebook__status-line {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
