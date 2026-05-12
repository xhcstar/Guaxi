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
import { createLogger } from '../../utils/logger'
import { getLspConnectionState, initLspClient, waitForLspConnection } from '../../monaco/lspClient'
import { registerPythonCompletionBridge } from '../../monaco/pythonCompletionBridge'
import {
  replaceModelContentPreserveUndo,
  retainVsCodeTextModel,
  releaseVsCodeTextModel,
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
let currentAppliedTheme = ''
let usesVscodeModelBridge = false

const NOTEBOOK_PYTHON_SUGGEST_TRIGGER_PATTERN = /[A-Za-z0-9_.:(\[,'"@]/

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

  if (!ready || !editor?.hasTextFocus?.()) return
  window.setTimeout(() => {
    triggerNotebookPythonSuggest(`warmup-${reason}`)
  }, 30)
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
    registerPythonCompletionBridge(monacoRef, ['python'])
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
    editor = monacoRef.editor.create(host, {
      model,
      language: 'python',
      minimap: { enabled: false },
      automaticLayout: true,
      lineNumbers: 'on',
      roundedSelection: false,
      scrollBeyondLastLine: false,
      wordWrap: 'on',
      // 设计说明：
      // 1. Notebook Python 单元格的补全应来自 Python LSP/bridge，而不是 Monaco 默认的文档单词扫描。
      // 2. 默认 word suggestions 会把 `sys.007aff`、`sys..2f` 这类同文档碎片展示成候选，看起来像 LSP 返回了错误结果。
      // 3. 关闭 showWords 后，如果 LSP 尚未就绪，界面会宁可暂时无候选，也不显示误导性的伪语义补全。
      wordBasedSuggestions: 'off',
      quickSuggestions: {
        other: true,
        comments: false,
        strings: true,
      },
      suggestOnTriggerCharacters: true,
      suggest: {
        showWords: false,
        snippetsPreventQuickSuggestions: false,
      },
      tabSize: 4,
      insertSpaces: true,
      fontFamily: 'var(--editor-font-family, "Maple Mono NF CN", "JetBrainsMono Nerd Font Mono", Consolas, monospace)',
      fontSize: 17,
      lineHeight: PYTHON_EDITOR_LINE_HEIGHT,
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

    contentChangeDisposable = editor.onDidChangeModelContent(() => {
      const nextValue = editor?.getValue?.() || ''
      emit('update:modelValue', nextValue)
    })

    contentSizeDisposable = editor.onDidContentSizeChange(() => {
      ensureHostHeight()
    })

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
      if (!shouldNotebookPythonTriggerSuggestOnType(text)) return
      window.setTimeout(() => {
        if (!editor?.hasTextFocus?.()) return
        triggerNotebookPythonSuggest(`type:${text}`)
      }, 0)
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
  const currentModelUri = model?.uri || null
  contentChangeDisposable?.dispose?.()
  contentChangeDisposable = null
  contentSizeDisposable?.dispose?.()
  contentSizeDisposable = null
  focusDisposable?.dispose?.()
  focusDisposable = null
  blurDisposable?.dispose?.()
  blurDisposable = null
  typeDisposable?.dispose?.()
  typeDisposable = null
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
})

watch(() => props.modelValue, (nextValue) => {
  if (!editor || !model) return
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
  ensureHostHeight()
})
</script>

<style scoped>
.guaxi-notebook-python-editor {
  position: relative;
  width: 100%;
  min-height: 48px;
  outline: none;
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-border, rgba(15, 23, 42, 0.1)) 100%, transparent);
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft, #f8fafc) 92%, transparent);
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
}
</style>
