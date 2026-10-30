<template>
  <div
    ref="hostRef"
    class="guaxi-notebook-python-editor"
    :class="{ 'is-pending': !editorReady && !loadFailed, 'is-fallback': loadFailed }"
    tabindex="-1"
  >
    <div v-if="!editorReady && !loadFailed" class="guaxi-notebook-python-editor__placeholder">
      <span>正在按需加载 Python 编辑器...</span>
    </div>
    <textarea
      v-if="loadFailed"
      class="guaxi-notebook-python-editor__fallback"
      spellcheck="false"
      :value="modelValue"
      @input="handleFallbackInput"
      @focus="emit('focus')"
      @blur="emit('blur')"
      @keydown="handleFallbackKeydown"
    />
  </div>
</template>

<script setup>
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import 'monaco-editor/esm/vs/basic-languages/python/python.contribution'
import { createLogger } from '../../utils/logger'
import { attachSharedEditorFeatures, resolveSharedEditorDisplayOptions } from '../../monaco/editorFeatures'
import { getLspConnectionState, initLspClient, waitForLspConnection, setLspImeComposing } from '../../monaco/lspClient'
import { initTextmate } from '../../monaco/textmateLoader'
import {
  GUAXI_SYNTAX_TOKEN_TYPES,
  getGuaxiSyntaxClassForToken,
  mapTreeSitterCaptureToGuaxiToken,
} from '../../monaco/guaxiSyntaxTokens'
import {
  replaceModelContentPreserveUndo,
  retainVsCodeTextModel,
  releaseVsCodeTextModel,
  syncVsCodeTextModelOverlay,
  setBridgeImeComposing,
} from '../../monaco/vscodeModelBridge'

const props = defineProps({
  modelValue: {
    type: String,
    default: '',
  },
  theme: {
    type: String,
    default: 'light',
  },
  documentPath: {
    type: String,
    default: '',
  },
  cellKey: {
    type: String,
    default: '',
  },
})

const emit = defineEmits(['update:modelValue', 'focus', 'blur'])

const logger = createLogger('GuaxiNotebookPythonEditor')
const hostRef = ref(null)
const editorReady = ref(false)
const loadFailed = ref(false)

let observer = null
let monacoRef = null
let editor = null
let model = null
let contentChangeDisposable = null
let contentSizeDisposable = null
let focusDisposable = null
let blurDisposable = null
let typeDisposable = null
let compositionStartDisposable = null
let compositionEndDisposable = null
let imeDomCleanup = null
let imeComposing = false
let savedQuickSuggestionsDuringIme
// 记录最近一次向父组件 emit 的内容；当 props.modelValue 回流的值与之相等时，
// 说明这是「自己编辑的回声」，绝不能再回写 model（否则用旧值全量替换正在编辑的 model，回退输入、卡顿、打断中文）。
let lastEmittedModelValue = null
let currentAppliedTheme = ''
let featuresDisposable = null
let usesVscodeModelBridge = false
let hostResizeObserver = null
let serviceRefreshTimer = 0
let treeSitterDecorationIds = []
let variableHighlightDecorationIds = []
let treeSitterDecorationTimer = 0
let overlaySyncTimer = 0
let variableHighlightTimer = 0
let treeSitterDecorationSeq = 1

const NOTEBOOK_PYTHON_SUGGEST_TRIGGER_PATTERN = /[A-Za-z0-9_.:(\[,'"@]/

const NOTEBOOK_TREE_SITTER_TOKEN_CLASS_BY_TYPE = Object.fromEntries(
  GUAXI_SYNTAX_TOKEN_TYPES.map((type) => [
    type,
    getGuaxiSyntaxClassForToken(type, 'nb-python-token'),
  ])
)

const buildNotebookTreeSitterDecorations = (activeModel, spans) => {
  const text = activeModel?.getValue?.() || ''
  const decorations = []

  for (const span of spans || []) {
    if (!span || span.end_byte <= span.start_byte) continue
    // 行列与 UTF-16 偏移都由后端一次算好（syntax_highlight.rs 的 resolve_token_span_positions）。
    // 这里原本要为整个 cell 逐字符建一份 UTF-8 索引（每个字符一次 TextEncoder.encode），
    // 现在只用后端给的数字：后端给 1 基列，换回这个文件用的 0 基。
    const startLine = Number(span.line ?? 0)
    const endLine = Number(span.end_line ?? 0)
    const startColumn0 = Number(span.start_column ?? 0) - 1
    const endColumn0 = Number(span.end_column ?? 0) - 1
    if (startColumn0 < 0 || endColumn0 < 0) {
      // 后端没给解析结果时不按 0 列去猜，否则会把整行染错。
      continue
    }
    if (startLine !== endLine) continue

    const mappedType = mapTreeSitterCaptureToGuaxiToken(span.token_type, {
      sourceText: text,
      startOffset: Number(span.start_utf16 ?? 0),
    })
    const className = NOTEBOOK_TREE_SITTER_TOKEN_CLASS_BY_TYPE[mappedType]
    if (!className) continue

    const startColumn = startColumn0 + 1
    const endColumn = Math.max(startColumn, endColumn0 + 1)
    decorations.push({
      range: new monacoRef.Range(startLine + 1, startColumn, startLine + 1, endColumn),
      options: { inlineClassName: className },
    })
  }

  return decorations
}

const refreshNotebookTreeSitterDecorations = async (reason = 'manual') => {
  if (!editor || !model || !monacoRef) return
  if (String(props.theme || '').toLowerCase() === 'dark') {
    if (treeSitterDecorationIds.length) {
      treeSitterDecorationIds = editor.deltaDecorations(treeSitterDecorationIds, [])
    }
    logger.debug('[NotebookPythonEditor] skipped extra tree-sitter decorations in dark theme', {
      cellKey: String(props.cellKey || ''),
      reason,
      theme: String(props.theme || ''),
    })
    return
  }
  const requestId = treeSitterDecorationSeq++
  const versionId = model.getVersionId?.() || 0
  const code = model.getValue?.() || ''

  try {
    const response = await invoke('syntax_highlight_tokens', {
      request: {
        code,
        language: 'python',
      },
    })
    if (requestId !== treeSitterDecorationSeq - 1 || versionId !== (model?.getVersionId?.() || 0)) return
    if (!response?.supported) {
      logger.warn('[NotebookPythonEditor] tree-sitter decoration returned unsupported response', {
        cellKey: String(props.cellKey || ''),
        reason,
      })
      return
    }
    const decorations = buildNotebookTreeSitterDecorations(model, response.spans || [])
    treeSitterDecorationIds = editor.deltaDecorations(treeSitterDecorationIds, decorations)
    logger.debug('[NotebookPythonEditor] refreshed tree-sitter color decorations', {
      cellKey: String(props.cellKey || ''),
      reason,
      versionId,
      spanCount: response.spans?.length || 0,
      decorationCount: decorations.length,
    })
  } catch (error) {
    logger.warn('[NotebookPythonEditor] refresh tree-sitter decorations failed', {
      cellKey: String(props.cellKey || ''),
      reason,
      error: String(error?.message || error),
    })
  }
}

const clearVariableHighlight = () => {
  if (variableHighlightTimer) {
    window.clearTimeout(variableHighlightTimer)
    variableHighlightTimer = 0
  }
  if (!editor || !variableHighlightDecorationIds.length) {
    variableHighlightDecorationIds = []
    return
  }
  variableHighlightDecorationIds = editor.deltaDecorations(variableHighlightDecorationIds, [])
}

const escapeNotebookVariableSearchPattern = (value) => String(value || '').replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

const buildVariableHighlightDecorations = (activeModel, variableName) => {
  const text = activeModel?.getValue?.() || ''
  const name = String(variableName || '').trim()
  if (!text || !name || !monacoRef) return []

  const pattern = new RegExp(`(^|[^A-Za-z0-9_])(${escapeNotebookVariableSearchPattern(name)})(?![A-Za-z0-9_])`, 'g')
  const decorations = []
  let match = pattern.exec(text)
  while (match && decorations.length < 160) {
    const prefixLength = match[1] ? match[1].length : 0
    const startOffset = match.index + prefixLength
    const endOffset = startOffset + name.length
    const start = activeModel.getPositionAt(startOffset)
    const end = activeModel.getPositionAt(endOffset)
    decorations.push({
      range: new monacoRef.Range(start.lineNumber, start.column, end.lineNumber, end.column),
      options: {
        inlineClassName: 'nb-python-variable-focus',
        className: 'nb-python-variable-focus-line',
        overviewRuler: {
          color: '#f59e0b',
          position: monacoRef.editor.OverviewRulerLane.Center,
        },
      },
    })
    match = pattern.exec(text)
  }
  return decorations
}

const findScrollableNotebookContainer = (element) => {
  let current = element?.parentElement || null
  while (current) {
    const style = window.getComputedStyle(current)
    const overflowY = `${style.overflowY || ''} ${style.overflow || ''}`
    if (/(auto|scroll|overlay)/.test(overflowY) && current.scrollHeight > current.clientHeight + 1) {
      return current
    }
    current = current.parentElement
  }
  return document.scrollingElement || document.documentElement
}

const centerEditorLineInPageViewport = (lineNumber, reason = 'variable-highlight') => {
  if (!editor || !Number.isFinite(Number(lineNumber))) return false
  const editorElement = editor.getDomNode?.()
  if (!(editorElement instanceof HTMLElement)) return false

  const linePosition = editor.getScrolledVisiblePosition?.({
    lineNumber: Number(lineNumber),
    column: 1,
  })
  const editorRect = editorElement.getBoundingClientRect()
  const targetCenter = linePosition && Number.isFinite(Number(linePosition.top))
    ? editorRect.top + Number(linePosition.top) + Math.max(10, Number(linePosition.height || 20) / 2)
    : editorRect.top + editorRect.height / 2
  const scrollContainer = findScrollableNotebookContainer(editorElement)
  const containerRect = scrollContainer === document.scrollingElement || scrollContainer === document.documentElement
    ? { top: 0, height: window.innerHeight }
    : scrollContainer.getBoundingClientRect()
  const containerCenter = containerRect.top + containerRect.height / 2
  const delta = targetCenter - containerCenter
  if (Math.abs(delta) < 4) return true

  scrollContainer.scrollTo({
    top: scrollContainer.scrollTop + delta,
    behavior: 'smooth',
  })
  logger.info('[NotebookPythonEditor] centered variable line in notebook viewport', {
    cellKey: String(props.cellKey || ''),
    lineNumber: Number(lineNumber),
    delta: Math.round(delta),
    reason,
  })
  return true
}

// 设计说明：
// 1. 右侧会话变量列表点击后需要把用户带回“变量产生/使用的 Cell”，并在源码中给出短暂视觉锚点。
// 2. 这里在 Python 编辑器内部完成 decoration 和页面滚动聚焦，不改运行逻辑、不触碰预览模式，也不会向 Python 会话写入任何内容。
// 3. 高亮自动过期，避免用户继续编辑时旧高亮长期停留造成误导。
const highlightVariableOccurrences = (variableName, reason = 'session-variable-focus') => {
  if (!editor || !model || !monacoRef) return false
  clearVariableHighlight()

  const decorations = buildVariableHighlightDecorations(model, variableName)
  if (!decorations.length) {
    logger.info('[NotebookPythonEditor] variable highlight skipped because no match was found', {
      cellKey: String(props.cellKey || ''),
      variableName: String(variableName || ''),
      reason,
    })
    return false
  }

  variableHighlightDecorationIds = editor.deltaDecorations([], decorations)
  const firstRange = decorations[0]?.range
  if (firstRange) {
    editor.setPosition?.({
      lineNumber: firstRange.startLineNumber,
      column: firstRange.startColumn,
    })
    editor.revealLineInCenter?.(firstRange.startLineNumber)
    window.setTimeout(() => {
      centerEditorLineInPageViewport(firstRange.startLineNumber, reason)
    }, 60)
  }
  editor.focus?.()
  variableHighlightTimer = window.setTimeout(() => {
    clearVariableHighlight()
  }, 3600)
  logger.info('[NotebookPythonEditor] highlighted variable occurrences', {
    cellKey: String(props.cellKey || ''),
    variableName: String(variableName || ''),
    matchCount: decorations.length,
    reason,
  })
  return true
}

// 大 cell 阈值：超过即对每键 O(n) 的 overlay 同步/高亮加大防抖，保证长 cell 输入流畅。
// 除行数外也看字符数：几行长但很宽的 cell（比如粘贴进来的数据/长字符串）同样是 O(n) 的负担。
const LARGE_NOTEBOOK_CELL_LINE_THRESHOLD = 2000
const LARGE_NOTEBOOK_CELL_CHAR_THRESHOLD = 120000
const isLargeNotebookCell = () => {
  try {
    if (!model || typeof model.getLineCount !== 'function') return false
    if (model.getLineCount() > LARGE_NOTEBOOK_CELL_LINE_THRESHOLD) return true
    return typeof model.getValueLength === 'function'
      && model.getValueLength() > LARGE_NOTEBOOK_CELL_CHAR_THRESHOLD
  } catch {
    return false
  }
}

// 立刻把（可能正在防抖中的）overlay 内容同步给 LSP，补全前调用，保证候选基于最新代码。
const flushOverlaySyncNow = () => {
  if (overlaySyncTimer) {
    window.clearTimeout(overlaySyncTimer)
    overlaySyncTimer = 0
  }
  if (usesVscodeModelBridge && model?.uri && editor) {
    syncVsCodeTextModelOverlay(model.uri, editor.getValue?.() || '', {
      owner: getNotebookPythonModelOwner(),
      reason: 'notebook-python-overlay-flush',
    })
  }
}

const scheduleNotebookTreeSitterDecorations = (reason = 'change') => {
  if (treeSitterDecorationTimer) window.clearTimeout(treeSitterDecorationTimer)
  // 大 cell：把整篇高亮的后端往返从 120ms 拉长到 400ms，减少打字时的全量重算。
  const delay = isLargeNotebookCell() ? 400 : 120
  treeSitterDecorationTimer = window.setTimeout(() => {
    treeSitterDecorationTimer = 0
    void refreshNotebookTreeSitterDecorations(reason)
  }, delay)
}

const getNotebookSuggestControllerState = () => {
  try {
    const controller = editor?.getContribution?.('editor.contrib.suggestController')
    const suggestModel = controller?.model
    const completionModel = suggestModel?.completionModel || suggestModel?._completionModel
    const filteredItems = Array.isArray(completionModel?.items) ? completionModel.items : []
    const rawItems = Array.isArray(completionModel?._items) ? completionModel._items : filteredItems
    const doc = editor?.getDomNode?.()?.ownerDocument || (typeof document !== 'undefined' ? document : null)
    return {
      state: Number(suggestModel?.state ?? suggestModel?._state ?? -1),
      widgetVisible: !!doc?.querySelector?.('.suggest-widget.visible'),
      filteredCount: filteredItems.length,
      rawCount: rawItems.length,
      firstFilteredLabel: String(filteredItems[0]?.completion?.label || filteredItems[0]?.textLabel || ''),
      firstRawLabel: String(rawItems[0]?.completion?.label || rawItems[0]?.textLabel || ''),
    }
  } catch {
    return {
      state: -1,
      widgetVisible: false,
      filteredCount: 0,
      rawCount: 0,
      firstFilteredLabel: '',
      firstRawLabel: '',
    }
  }
}

const getNotebookCompletionProviderState = () => {
  try {
    const activeModel = editor?.getModel?.()
    const providers = monacoRef?.languages?.CompletionProviderRegistry?.all?.(activeModel)
    return {
      modelLanguageId: String(activeModel?.getLanguageId?.() || ''),
      modelUri: String(activeModel?.uri?.toString?.() || ''),
      providerCount: Array.isArray(providers) ? providers.length : -1,
    }
  } catch (error) {
    return {
      modelLanguageId: '',
      modelUri: '',
      providerCount: -1,
      providerStateError: String(error?.message || error),
    }
  }
}

const attachNotebookSuggestTrace = (reason = 'unknown') => {
  try {
    const controller = editor?.getContribution?.('editor.contrib.suggestController')
    const suggestModel = controller?.model
    if (!suggestModel?.onDidCancel && !suggestModel?.onDidSuggest && !suggestModel?.onDidTrigger) return () => {}

    const disposables = []
    const pushDisposable = (candidate) => {
      if (candidate?.dispose) disposables.push(candidate)
    }

    pushDisposable(suggestModel?.onDidTrigger?.((event) => {
      logger.info('[NotebookPythonEditor] suggest model triggered', {
        cellKey: String(props.cellKey || ''),
        reason,
        auto: !!event?.auto,
        shy: !!event?.shy,
        position: event?.position ? `${event.position.lineNumber}:${event.position.column}` : '',
      })
    }))
    pushDisposable(suggestModel?.onDidSuggest?.((event) => {
      logger.info('[NotebookPythonEditor] suggest model produced completion model', {
        cellKey: String(props.cellKey || ''),
        reason,
        completionCount: Number(event?.completionModel?.items?.length || 0),
        isFrozen: !!event?.isFrozen,
        auto: !!event?.triggerOptions?.auto,
        triggerKind: Number(event?.triggerOptions?.triggerKind || 0),
      })
    }))
    pushDisposable(suggestModel?.onDidCancel?.((event) => {
      logger.info('[NotebookPythonEditor] suggest model cancelled', {
        cellKey: String(props.cellKey || ''),
        reason,
        retrigger: !!event?.retrigger,
        stateAfterCancel: Number(suggestModel?.state ?? -1),
      })
    }))

    const timeout = window.setTimeout(() => {
      disposables.forEach((disposable) => {
        try {
          disposable.dispose()
        } catch {
        }
      })
    }, 1400)

    return () => {
      window.clearTimeout(timeout)
      disposables.forEach((disposable) => {
        try {
          disposable.dispose()
        } catch {
        }
      })
    }
  } catch (error) {
    logger.debug('[NotebookPythonEditor] attach suggest trace failed', {
      cellKey: String(props.cellKey || ''),
      reason,
      error: String(error?.message || error),
    })
    return () => {}
  }
}

const shouldNotebookPythonTriggerSuggestOnType = (text) => {
  const value = String(text || '')
  if (!value || value.length !== 1) return false
  if (/\s/.test(value)) return false
  return NOTEBOOK_PYTHON_SUGGEST_TRIGGER_PATTERN.test(value)
}

const triggerNotebookPythonSuggest = (reason = 'manual') => {
  if (!editor || !monacoRef) return
  // 触发补全前确保 overlay 已是最新（大 cell 时 overlay 同步是防抖的）。
  flushOverlaySyncNow()
  try {
    const disposeTrace = attachNotebookSuggestTrace(reason)
    editor.trigger('guaxi-notebook-python', 'editor.action.triggerSuggest', {})
    const providerState = getNotebookCompletionProviderState()
    logger.debug('[NotebookPythonEditor] trigger suggest', {
      cellKey: String(props.cellKey || ''),
      reason,
      position: editor.getPosition?.()
        ? `${editor.getPosition().lineNumber}:${editor.getPosition().column}`
        : '',
      beforeState: getNotebookSuggestControllerState(),
      ...providerState,
    })
    const logSnapshot = (delayMs) => {
      logger.info('[NotebookPythonEditor] suggest controller snapshot', {
        cellKey: String(props.cellKey || ''),
        reason: `after-${reason}-${delayMs}ms`,
        ...getNotebookSuggestControllerState(),
        ...getNotebookCompletionProviderState(),
      })
    }
    window.setTimeout(() => logSnapshot(260), 260)
    window.setTimeout(() => {
      logSnapshot(1100)
      disposeTrace()
    }, 1100)
  } catch (error) {
    logger.debug('[NotebookPythonEditor] trigger suggest failed', {
      cellKey: String(props.cellKey || ''),
      reason,
      error: String(error?.message || error),
    })
  }
}

const warmupNotebookPythonCompletion = async (reason = 'unknown') => {
  const languageId = 'python'
  const lspStateBefore = getLspConnectionState(languageId)
  // 设计说明：
  // 1. 探索者模式点击 Python cell 时会触发 editor focus；这里以前在 LSP ready 后继续执行 triggerSuggest。
  // 2. 这会把“鼠标点空行”变成一次显式补全请求，后端日志表现为 trigger_kind=1 且 trigger_character 为空。
  // 3. warmup 现在只负责等待 LSP 连接、同步状态和记录日志，真正弹补全只保留给用户输入触发字符或手动命令。
  logger.info('[NotebookPythonEditor] warmup completion start', {
    cellKey: String(props.cellKey || ''),
    reason,
    lspState: lspStateBefore?.state || 'unknown',
  })

  const ready = await waitForLspConnection(languageId, 2400)
  logger.info('[NotebookPythonEditor] warmup completion resolved', {
    cellKey: String(props.cellKey || ''),
    reason,
    ready,
    lspState: getLspConnectionState(languageId)?.state || 'unknown',
  })

  if (ready && editor?.hasTextFocus?.()) {
    logger.debug('[NotebookPythonEditor] warmup completed without opening suggest widget', {
      cellKey: String(props.cellKey || ''),
      reason,
      position: editor.getPosition?.()
        ? `${editor.getPosition().lineNumber}:${editor.getPosition().column}`
        : '',
    })
  }
}

const getNotebookPythonModelOwner = () => {
  const cellKey = String(props.cellKey || 'python-cell').trim() || 'python-cell'
  const documentPath = String(props.documentPath || '').trim() || 'anonymous-document'
  return `notebook-python:${documentPath}:${cellKey}`
}

const buildNotebookPythonVirtualFilePath = () => {
  const rawCellKey = String(props.cellKey || 'python-cell').trim() || 'python-cell'
  const safeCellKey = rawCellKey.replace(/[^a-zA-Z0-9_-]/g, '_')
  const rawDocumentPath = String(props.documentPath || '').trim()
  if (!rawDocumentPath) {
    return `file:///guaxi-notebook/${safeCellKey}.py`
  }

  // 设计说明：
  // 1. Python cell 也需要稳定 file URI，否则部分语言服务器只会把它当匿名内存文档，补全和诊断都不完整。
  // 2. 这里和 Markdown cell 保持同一套路由，统一挂到当前文档目录下的虚拟 .guaxi-notebook 子空间。
  const separator = rawDocumentPath.includes('\\') ? '\\' : '/'
  const normalizedDocumentPath = rawDocumentPath.replace(/[\\/]+$/, '')
  const lastSeparatorIndex = Math.max(
    normalizedDocumentPath.lastIndexOf('/'),
    normalizedDocumentPath.lastIndexOf('\\')
  )
  if (lastSeparatorIndex < 0) {
    return `file:///guaxi-notebook/${safeCellKey}.py`
  }
  const directory = normalizedDocumentPath.slice(0, lastSeparatorIndex)
  const baseName = normalizedDocumentPath
    .slice(lastSeparatorIndex + 1)
    .replace(/\.[^./\\]+$/, '') || 'notebook'
  return `${directory}${separator}.guaxi-notebook${separator}${baseName}__${safeCellKey}.py`
}

// 设计说明：
// 1. Python Cell 在未运行且代码很短时，不应该默认占出大块留白。
// 2. 这里将编辑器最小高度收敛到“单行代码 + 上下内边距 + 少量边框余量”，
//    让新插入的 Cell 默认更贴近内容高度，再随着代码增长自动扩展。
const PYTHON_EDITOR_LINE_HEIGHT = 22
const PYTHON_EDITOR_PADDING_VERTICAL = 24
const PYTHON_EDITOR_MIN_CONTENT_HEIGHT = PYTHON_EDITOR_LINE_HEIGHT + PYTHON_EDITOR_PADDING_VERTICAL
const PYTHON_EDITOR_MIN_HOST_HEIGHT = PYTHON_EDITOR_MIN_CONTENT_HEIGHT + 2

const getMonacoThemeName = () => {
  return String(props.theme || '').toLowerCase() === 'dark' ? 'guaxi-dark' : 'guaxi-light'
}

const ensureHostHeight = () => {
  const host = hostRef.value
  if (!(host instanceof HTMLElement)) return
  if (!editor) {
    host.style.height = `${PYTHON_EDITOR_MIN_HOST_HEIGHT}px`
    return
  }
  const contentHeight = Math.max(Number(editor.getContentHeight?.() || 0), PYTHON_EDITOR_MIN_CONTENT_HEIGHT)
  host.style.height = `${contentHeight + 2}px`
  editor.layout()
  logger.debug('[NotebookPythonEditor] synced editor height', {
    cellKey: String(props.cellKey || ''),
    contentHeight,
    hostHeight: contentHeight + 2,
  })
}

const refreshEditorServices = (reason = 'layout-refresh') => {
  if (!editor) return
  window.clearTimeout(serviceRefreshTimer)
  serviceRefreshTimer = window.setTimeout(() => {
    if (!editor || !model) return
    editor.layout()
    ensureHostHeight()
    if (usesVscodeModelBridge && model?.uri) {
      syncVsCodeTextModelOverlay(model.uri, editor.getValue?.() || '', {
        owner: getNotebookPythonModelOwner(),
        reason: `notebook-python-${reason}`,
      })
    }
    applyTheme()
    logger.info('[NotebookPythonEditor] refreshed editor services after layout change', {
      cellKey: String(props.cellKey || ''),
      reason,
      uri: String(model?.uri?.toString?.() || ''),
      language: String(model?.getLanguageId?.() || ''),
      lspState: getLspConnectionState('python')?.state || 'unknown',
    })
    if (editor.hasTextFocus?.()) {
      void warmupNotebookPythonCompletion(reason)
    }
  }, 80)
}

// 设计说明：
// 1. Monaco 默认会“强吃”滚轮事件，鼠标一停在编辑器上，整页滚动体验就会变得很差。
// 2. 探索模式里 cell 编辑器应该更像嵌入式输入区，因此把滚轮让渡给外层页面，只在编辑器确实需要时再处理。
// 3. 对键盘交互，保留正常编辑能力，但提供 Escape 快速失焦，让方向键能立即回到页面级滚动。
const blurEditorToPage = (reason = 'manual-blur') => {
  if (editor?.hasTextFocus?.()) {
    editor.setPosition(editor.getPosition())
    editor.getDomNode()?.blur?.()
  }
  hostRef.value?.focus?.()
  emit('blur')
  logger.info('[NotebookPythonEditor] release editor focus back to notebook page', {
    cellKey: String(props.cellKey || ''),
    reason,
  })
}

const applyTheme = () => {
  if (!monacoRef?.editor) return
  const nextTheme = getMonacoThemeName()
  if (currentAppliedTheme === nextTheme) return
  currentAppliedTheme = nextTheme
  try {
    monacoRef.editor.setTheme(nextTheme)
  } catch (error) {
    const fallbackTheme = String(props.theme || '').toLowerCase() === 'dark' ? 'vs-dark' : 'vs'
    monacoRef.editor.setTheme(fallbackTheme)
    currentAppliedTheme = fallbackTheme
    logger.warn('[NotebookPythonEditor] apply custom theme failed, fallback to Monaco built-in theme', {
      cellKey: String(props.cellKey || ''),
      error: String(error?.message || error),
      fallbackTheme,
    })
  }
}

const mountMonacoEditor = async () => {
  if (editor || loadFailed.value) return
  const host = hostRef.value
  if (!(host instanceof HTMLElement)) return

  try {
    monacoRef = await import('monaco-editor')
    const virtualFilePath = buildNotebookPythonVirtualFilePath()
    const virtualUri = String(virtualFilePath || '').startsWith('file://')
      ? monacoRef.Uri.parse(virtualFilePath)
      : monacoRef.Uri.file(virtualFilePath)
    model = await retainVsCodeTextModel({
      resource: virtualUri,
      content: String(props.modelValue || ''),
      languageId: 'python',
      owner: getNotebookPythonModelOwner(),
      reason: 'notebook-python-mount',
    })
    usesVscodeModelBridge = !!model
    if (!model) {
      // 设计说明：
      // 1. 这里优先走 VSCode model bridge，让 notebook cell 能被补全/hover/LSP 识别。
      // 2. 只有 bridge 初始化失败时才退回 standalone model，避免编辑器整体不可用。
      model = monacoRef.editor.createModel(String(props.modelValue || ''), 'python', virtualUri)
      logger.warn('[NotebookPythonEditor] VSCode model bridge unavailable, fallback to standalone Monaco model', {
        cellKey: String(props.cellKey || ''),
        virtualFilePath,
      })
    } else {
      logger.info('[NotebookPythonEditor] retained VSCode model bridge for notebook cell', {
        cellKey: String(props.cellKey || ''),
        virtualFilePath,
        owner: getNotebookPythonModelOwner(),
      })
    }
    applyTheme()
    const display = resolveSharedEditorDisplayOptions()
    editor = monacoRef.editor.create(host, {
      model,
      language: 'python',
      // 右键菜单由 editorFeatures 提供，关掉 Monaco 自带菜单避免双弹。
      contextmenu: false,
      minimap: { enabled: false },
      automaticLayout: true,
      // 大 cell 性能：万行 Python 也不卡 tokenize
      largeFileOptimizations: true,
      maxTokenizationLineLength: 2000,
      lineNumbers: 'on',
      roundedSelection: false,
      scrollBeyondLastLine: false,
      wordWrap: 'on',
      // 设计说明：括号配色来自主题的 editorBracketHighlight.foreground1..6（红/金/黄/绿/蓝/紫），
      // 属于编辑器额外装饰而非 token 高亮；不清掉的话同一段 Python 在 cell 里括号是彩色的，
      // 在 Markdown 代码块里却是基础色，代码高亮看起来仍然两套。语义分层保留，只关掉这层。
      bracketPairColorization: { enabled: false },
      // 设计说明：
      // 1. 探索者 Python Cell 默认应该随中间区域宽度折行，降低整页横向滚动和代码阅读来回拖动。
      // 2. Monaco / VSCode service 层可能通过 override 影响 wordWrap，这里显式把两级 override 都固定为 on。
      // 3. 对超长无空格 token，仍保留编辑器内部水平滚动条作为兜底，不再把外层 Notebook 撑宽。
      wordWrapOverride1: 'on',
      wordWrapOverride2: 'on',
      wrappingIndent: 'same',
      wrappingStrategy: 'advanced',
      wordBreak: 'normal',
      // 设计说明：
      // 1. Pyright 会把运行后的探索代码中大量临时变量、导入和中间对象标记为 unnecessary/unused。
      // 2. Monaco 默认会把这些标记渲染成半透明，右侧数据实验室展开触发布局/诊断刷新后就会出现大片暗色代码。
      // 3. 这里关闭“未使用代码淡化”，诊断仍保留，但不再牺牲可读性。
      showUnused: false,
      // 设计说明：
      // 1. 探索者模式的 Python Cell 不按源码编辑器的强自动补全策略走，避免鼠标点选单元格时建议框抢焦点。
      // 2. 仍保留 trigger character 和手动补全，输入 `.` 等符号时能像 VS Code 一样立即请求 LSP。
      // 3. 关闭 showWords 后，如果 LSP 尚未就绪，界面会宁可暂时无候选，也不显示误导性的伪语义补全。
      wordBasedSuggestions: 'off',
      quickSuggestions: {
        other: false,
        comments: false,
        strings: false,
      },
      suggestOnTriggerCharacters: true,
      suggest: {
        showWords: false,
        snippetsPreventQuickSuggestions: false,
      },
      // 设计说明：
      // 1. 探索者模式的 Python Cell 和编辑模式都使用同一套 TreeSitter semantic token 配色。
      // 2. 颜色稳定性由全局 `guaxi-*` 主题里的 semanticTokenColors 负责，而不是退回 Monaco 默认 tokenizer。
      // 3. 这样保留变量、函数、字符串、数字等语义分层，也避免右侧数据实验室展开后出现另一套高亮。
      'semanticHighlighting.enabled': true,
      tabSize: 4,
      insertSpaces: true,
      fontFamily: display.fontFamily,
      fontSize: display.fontSize,
      lineHeight: display.lineHeight,
      padding: { top: 12, bottom: 12 },
      fixedOverflowWidgets: true,
      alwaysConsumeMouseWheel: false,
      scrollbar: {
        alwaysConsumeMouseWheel: false,
        vertical: 'auto',
        horizontal: 'auto',
        handleMouseWheel: true,
        useShadows: false,
        verticalScrollbarSize: 8,
        horizontalScrollbarSize: 8,
      },
    })
    logger.debug('[NotebookPythonEditor] python lsp suggest options applied', {
      cellKey: String(props.cellKey || ''),
      wordBasedSuggestions: 'off',
      showWords: false,
    })

    try {
      await initTextmate(editor, 'python')
      scheduleNotebookTreeSitterDecorations('mount:init-textmate')
      logger.info('[NotebookPythonEditor] tree-sitter semantic highlight initialized with shared theme', {
        cellKey: String(props.cellKey || ''),
        language: model?.getLanguageId?.() || 'python',
        uri: String(model?.uri?.toString?.() || ''),
        theme: currentAppliedTheme,
      })
    } catch (error) {
      logger.warn('[NotebookPythonEditor] tree-sitter semantic highlight init failed, keep Monaco fallback tokenizer', {
        cellKey: String(props.cellKey || ''),
        error: String(error?.message || error),
      })
    }

    contentChangeDisposable = editor.onDidChangeModelContent(() => {
      const nextValue = editor?.getValue?.() || ''
      if (usesVscodeModelBridge && model?.uri) {
        // 设计说明：
        // 1. Notebook Python cell 绑定的是 `file://.../.guaxi-notebook/*.py` 虚拟文件。
        // 2. Python LSP 补全读取的是 VSCode 文件服务里的 overlay 内容，而不是只看 Monaco editor 当前缓冲区。
        // 3. 每次编辑都同步 overlay；但 overlay 写入是 O(n)，万行 cell 每键同步会卡，
        //    故大 cell 改为短防抖(150ms)，补全前会强制 flush（flushOverlaySyncNow）保证候选不丢。
        if (isLargeNotebookCell()) {
          if (overlaySyncTimer) window.clearTimeout(overlaySyncTimer)
          overlaySyncTimer = window.setTimeout(() => {
            overlaySyncTimer = 0
            if (usesVscodeModelBridge && model?.uri) {
              syncVsCodeTextModelOverlay(model.uri, editor?.getValue?.() || '', {
                owner: getNotebookPythonModelOwner(),
                reason: 'notebook-python-content-change-debounced',
              })
            }
          }, 150)
        } else {
          syncVsCodeTextModelOverlay(model.uri, nextValue, {
            owner: getNotebookPythonModelOwner(),
            reason: 'notebook-python-content-change',
          })
        }
      }
      lastEmittedModelValue = nextValue
      emit('update:modelValue', nextValue)
      scheduleNotebookTreeSitterDecorations('content-change')
    })

    contentSizeDisposable = editor.onDidContentSizeChange(() => {
      ensureHostHeight()
    })

    hostResizeObserver = new ResizeObserver(() => {
      refreshEditorServices('host-resize')
    })
    hostResizeObserver.observe(host)

    focusDisposable = editor.onDidFocusEditorText(() => {
      emit('focus')
      logger.debug('[NotebookPythonEditor] editor focused', {
        cellKey: String(props.cellKey || ''),
      })
      void warmupNotebookPythonCompletion('focus')
    })

    blurDisposable = editor.onDidBlurEditorText(() => {
      emit('blur')
      logger.debug('[NotebookPythonEditor] editor blurred', {
        cellKey: String(props.cellKey || ''),
      })
    })

    editor.addCommand(monacoRef.KeyCode.Escape, () => {
      blurEditorToPage('escape')
    })

    typeDisposable = editor.onDidType((text) => {
      // IME 组合期间不触发补全，避免打断中文输入
      if (imeComposing) return
      if (!shouldNotebookPythonTriggerSuggestOnType(text)) return
      window.setTimeout(() => {
        if (!editor?.hasTextFocus?.()) return
        triggerNotebookPythonSuggest(`type:${text}`)
      }, 0)
    })

    // IME 组合守卫：监听底层 textarea 原生 composition 事件（Monaco 的事件在本 webview 不一定触发），
    // 组合期间置位标志让补全/onDidType 跳过、并关闭已弹出的补全框，避免打断中文输入。
    const onImeStart = () => {
      if (imeComposing) return
      imeComposing = true
      setLspImeComposing(true)
      setBridgeImeComposing(true)
      try { editor?.trigger?.('ime', 'hideSuggestWidget', null) } catch { /* noop */ }
    }
    const onImeEnd = () => {
      if (!imeComposing) return
      imeComposing = false
      setLspImeComposing(false)
      setBridgeImeComposing(false)
      // 组合结束后把最终内容回写到 overlay，确保 LSP 看到最新文本
      try {
        if (usesVscodeModelBridge && model?.uri) {
          syncVsCodeTextModelOverlay(model.uri, editor?.getValue?.() || '', {
            owner: getNotebookPythonModelOwner(),
            reason: 'ime-composition-end-resync',
          })
        }
      } catch (error) {
        logger.debug('[NotebookPythonEditor] composition end resync failed', error)
      }
    }
    // 搜狗等输入法 composition 事件时序不稳，用 keydown 的 isComposing/229 兜底：组合中每次按键都置位并关闭补全框
    const onImeKeyDown = (event) => {
      if (event && (event.isComposing === true || event.keyCode === 229)) {
        onImeStart()
        try { editor?.trigger?.('ime', 'hideSuggestWidget', null) } catch { /* noop */ }
      }
    }
    const editorDom = editor.getDomNode?.()
    if (editorDom) {
      editorDom.addEventListener('compositionstart', onImeStart, true)
      editorDom.addEventListener('compositionend', onImeEnd, true)
      editorDom.addEventListener('keydown', onImeKeyDown, true)
      imeDomCleanup = () => {
        editorDom.removeEventListener('compositionstart', onImeStart, true)
        editorDom.removeEventListener('compositionend', onImeEnd, true)
        editorDom.removeEventListener('keydown', onImeKeyDown, true)
      }
    }
    compositionStartDisposable = editor.onDidCompositionStart?.(onImeStart)
    compositionEndDisposable = editor.onDidCompositionEnd?.(onImeEnd)

    // 与主编辑器一致的基础能力：右键菜单、主题同步（LSP 已由上方逻辑启动）。
    featuresDisposable = attachSharedEditorFeatures({
      monaco: monacoRef,
      editor,
      language: 'python',
      enableLsp: false,
    })

    editorReady.value = true
    await nextTick()
    ensureHostHeight()
    initLspClient(editor, 'python', virtualFilePath, {
      workspacePath: String(props.documentPath || '').trim() || virtualFilePath,
      documentUri: virtualUri.toString(),
    })
    logger.info('[NotebookPythonEditor] LSP init requested for notebook cell', {
      cellKey: String(props.cellKey || ''),
      language: 'python',
      virtualFilePath,
      documentPath: String(props.documentPath || ''),
    })
    void warmupNotebookPythonCompletion('mount')
    logger.info('[NotebookPythonEditor] mounted Monaco editor', {
      cellKey: String(props.cellKey || ''),
      theme: currentAppliedTheme,
    })
  } catch (error) {
    loadFailed.value = true
    logger.error('[NotebookPythonEditor] failed to mount Monaco editor, fallback to textarea', {
      cellKey: String(props.cellKey || ''),
      error: String(error?.message || error),
    })
  }
}

const disposeMonacoEditor = () => {
  featuresDisposable?.()
  featuresDisposable = null
  const currentModelUri = model?.uri || null
  window.clearTimeout(serviceRefreshTimer)
  serviceRefreshTimer = 0
  clearVariableHighlight()
  hostResizeObserver?.disconnect?.()
  hostResizeObserver = null
  contentChangeDisposable?.dispose?.()
  contentChangeDisposable = null
  contentSizeDisposable?.dispose?.()
  contentSizeDisposable = null
  if (treeSitterDecorationTimer) {
    window.clearTimeout(treeSitterDecorationTimer)
    treeSitterDecorationTimer = 0
  }
  // 销毁前把防抖中的 overlay 内容落地，避免丢失最后一段编辑对 LSP 的同步
  flushOverlaySyncNow()
  if (overlaySyncTimer) {
    window.clearTimeout(overlaySyncTimer)
    overlaySyncTimer = 0
  }
  if (editor && treeSitterDecorationIds.length) {
    treeSitterDecorationIds = editor.deltaDecorations(treeSitterDecorationIds, [])
  } else {
    treeSitterDecorationIds = []
  }
  focusDisposable?.dispose?.()
  focusDisposable = null
  blurDisposable?.dispose?.()
  blurDisposable = null
  typeDisposable?.dispose?.()
  typeDisposable = null
  compositionStartDisposable?.dispose?.()
  compositionStartDisposable = null
  compositionEndDisposable?.dispose?.()
  compositionEndDisposable = null
  if (imeDomCleanup) {
    try { imeDomCleanup() } catch { /* noop */ }
    imeDomCleanup = null
  }
  // 若在组合中途销毁，确保全局组合计数被正确释放
  if (imeComposing) {
    imeComposing = false
    setLspImeComposing(false)
    setBridgeImeComposing(false)
  }
  editor?.dispose?.()
  editor = null
  if (usesVscodeModelBridge && currentModelUri) {
    releaseVsCodeTextModel(currentModelUri, getNotebookPythonModelOwner(), 'notebook-python-dispose')
  } else {
    model?.dispose?.()
  }
  model = null
  monacoRef = null
  editorReady.value = false
  usesVscodeModelBridge = false
}

const handleFallbackInput = (event) => {
  emit('update:modelValue', String(event?.target?.value || ''))
}

const handleFallbackKeydown = (event) => {
  if (event?.key !== 'Escape') return
  event.preventDefault()
  event?.target?.blur?.()
  hostRef.value?.focus?.()
  emit('blur')
  logger.info('[NotebookPythonEditor] release fallback editor focus back to notebook page', {
    cellKey: String(props.cellKey || ''),
    reason: 'fallback-escape',
  })
}

onMounted(() => {
  const host = hostRef.value
  ensureHostHeight()
  if (!(host instanceof HTMLElement)) return
  observer = new IntersectionObserver((entries) => {
    if (!Array.isArray(entries)) return
    for (const entry of entries) {
      if (!entry.isIntersecting) continue
      observer?.disconnect?.()
      observer = null
      void mountMonacoEditor()
      break
    }
  }, {
    root: null,
    rootMargin: '240px 0px 240px 0px',
    threshold: 0.01,
  })
  observer.observe(host)
})

onBeforeUnmount(() => {
  observer?.disconnect?.()
  observer = null
  disposeMonacoEditor()
})

watch(() => props.theme, () => {
  applyTheme()
  scheduleNotebookTreeSitterDecorations('theme-change')
})

watch(() => props.modelValue, (nextValue) => {
  if (!editor || !model) return
  // 自己编辑的回声：父组件回流的就是我们刚 emit 的值，绝不回写（否则全量替换正在编辑的 model，回退输入、卡顿、打断中文）。
  if (nextValue === lastEmittedModelValue) return
  const normalizedValue = String(nextValue || '')
  const currentValue = model.getValue()
  if (currentValue === normalizedValue) return

  // 设计说明：
  // 1. 外部 notebook 状态回推到 Cell 编辑器时，统一走 bridge 提供的“保留 undo 的最小替换”能力，
  //    避免不同类型 text model（尤其是 monaco-vscode 的 bridge model）在直接 pushEditOperations 时出现边界异常。
  // 2. 这里仍保留最小范围替换，而不是 setValue，全量覆盖会破坏用户的撤销栈和光标上下文。
  replaceModelContentPreserveUndo(model, normalizedValue, {
    logPrefix: '[NotebookPythonEditor] synchronize external cell content',
    uri: String(model?.uri?.toString?.() || ''),
    owner: getNotebookPythonModelOwner(),
    reason: 'props-model-value-watch',
  })
  logger.debug('[NotebookPythonEditor] applied external model sync', {
    cellKey: String(props.cellKey || ''),
    nextLength: normalizedValue.length,
  })
  scheduleNotebookTreeSitterDecorations('external-model-sync')
  ensureHostHeight()
})

const insertContent = async (content) => {
  const text = String(content || '')
  if (!text) return false

  if (!editor && !loadFailed.value) {
    observer?.disconnect?.()
    observer = null
    await mountMonacoEditor()
    await nextTick()
  }

  if (editor && model && monacoRef?.Range) {
    const selection = editor.getSelection()
    const position = editor.getPosition() || { lineNumber: 1, column: 1 }
    const range = selection || new monacoRef.Range(position.lineNumber, position.column, position.lineNumber, position.column)
    editor.executeEdits('template-insert', [{ range, text, forceMoveMarkers: true }])
    const endPosition = editor.getPosition()
    editor.focus()
    if (endPosition) editor.revealPositionInCenterIfOutsideViewport(endPosition)
    logger.info('[NotebookPythonEditor] inserted template content at cursor', {
      cellKey: String(props.cellKey || ''),
      length: text.length,
    })
    return true
  }

  const fallback = hostRef.value?.querySelector?.('textarea')
  if (fallback instanceof HTMLTextAreaElement) {
    const start = fallback.selectionStart ?? fallback.value.length
    const end = fallback.selectionEnd ?? start
    const nextValue = `${fallback.value.slice(0, start)}${text}${fallback.value.slice(end)}`
    fallback.value = nextValue
    fallback.setSelectionRange(start + text.length, start + text.length)
    emit('update:modelValue', nextValue)
    fallback.focus()
    return true
  }

  return false
}

defineExpose({
  refreshEditorServices,
  highlightVariableOccurrences,
  insertContent,
})
</script>

<style scoped>
.guaxi-notebook-python-editor {
  position: relative;
  width: 100%;
  max-width: 100%;
  min-width: 0;
  min-height: 48px;
  outline: none;
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-border, rgba(15, 23, 42, 0.1)) 100%, transparent);
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft, #f8fafc) 92%, transparent);
  box-sizing: border-box;
  /* 设计说明：
     1. Python Cell 内部 Monaco 已保留自己的水平滚动条，用于查看极长代码行。
     2. 外层宿主必须保持可收缩，否则 Monaco 的 scrollWidth 会继续向上撑大探索模式中间区域。 */
  overflow: hidden;
}

.guaxi-notebook-python-editor__placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 48px;
  color: var(--guaxi-notebook-muted, rgba(23, 32, 51, 0.72));
  font-family: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace);
  font-size: 12px;
  background:
    linear-gradient(135deg, color-mix(in srgb, var(--guaxi-notebook-panel, #ffffff) 94%, transparent), color-mix(in srgb, var(--guaxi-notebook-panel-soft, #f8fafc) 90%, transparent));
}

.guaxi-notebook-python-editor__fallback {
  width: 100%;
  max-width: 100%;
  min-width: 0;
  min-height: 48px;
  resize: vertical;
  border: none;
  outline: none;
  padding: 12px 14px;
  background: transparent;
  color: var(--guaxi-notebook-text, #172033);
  font-family: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace);
  font-size: 13px;
  line-height: 1.7;
  box-sizing: border-box;
}

.guaxi-notebook-python-editor :deep(.squiggly-inline-unnecessary),
.guaxi-notebook-python-editor :deep(.squiggly-inline-deprecated),
.guaxi-notebook-python-editor :deep(.codicon-unused),
.guaxi-notebook-python-editor :deep(.unused) {
  opacity: 1 !important;
  filter: none !important;
}

/* 设计说明：
   1. Python cell 在亮色模式下补一层 Tree-sitter decoration，对齐编辑模式代码围栏的 md-fence-token-* 观感。
   2. 暗色模式不叠加这层 decoration，避免 Monaco semantic token 与 decoration 同时染色造成偏蓝。
   3. 两种模式仍然使用同一个 Tree-sitter 后端，只是暗色直接走 Monaco semantic token，亮色额外补齐可读性。 */
.guaxi-notebook-python-editor :deep(.nb-python-token-keyword) { color: var(--guaxi-syntax-keyword) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-string) { color: var(--guaxi-syntax-string) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-escape) { color: var(--guaxi-syntax-escape) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-number) { color: var(--guaxi-syntax-number) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-boolean) { color: var(--guaxi-syntax-boolean) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-comment) { color: var(--guaxi-syntax-comment) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-type) { color: var(--guaxi-syntax-type) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-class) { color: var(--guaxi-syntax-class) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-inherit) { color: var(--guaxi-syntax-inherit) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-function) { color: var(--guaxi-syntax-function) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-method) { color: var(--guaxi-syntax-method) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-parameter) { color: var(--guaxi-syntax-parameter) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-variable) { color: var(--guaxi-syntax-variable) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-variable-language) { color: var(--guaxi-syntax-variable-language) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-constant) { color: var(--guaxi-syntax-constant) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-regexp) { color: var(--guaxi-syntax-regexp) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-property) { color: var(--guaxi-syntax-property) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-namespace) { color: var(--guaxi-syntax-namespace) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-macro) { color: var(--guaxi-syntax-macro) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-attribute) { color: var(--guaxi-syntax-attribute) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-builtin) { color: var(--guaxi-syntax-builtin) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-decorator) { color: var(--guaxi-syntax-decorator) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-label) { color: var(--guaxi-syntax-label) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-tag) { color: var(--guaxi-syntax-tag) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-operator) { color: var(--guaxi-syntax-operator) !important; }
.guaxi-notebook-python-editor :deep(.nb-python-token-delimiter) { color: var(--guaxi-syntax-punctuation) !important; }

.guaxi-notebook-python-editor :deep(.nb-python-variable-focus) {
  border-radius: 4px;
  background: color-mix(in srgb, #f59e0b 32%, transparent);
  box-shadow: 0 0 0 1px color-mix(in srgb, #f59e0b 56%, transparent);
}

.guaxi-notebook-python-editor :deep(.nb-python-variable-focus-line) {
  background: color-mix(in srgb, #f59e0b 7%, transparent);
}

</style>
