<template>
  <div
    ref="hostRef"
    class="guaxi-notebook-markdown-editor"
    :class="{ 'is-pending': !editorReady && !loadFailed, 'is-fallback': loadFailed }"
    :data-cell-key="cellKey"
    tabindex="-1"
  >
    <div v-if="!editorReady && !loadFailed" class="guaxi-notebook-markdown-editor__placeholder">
      <span>正在按需加载 Markdown 编辑器...</span>
    </div>
    <textarea
      v-if="loadFailed"
      class="guaxi-notebook-markdown-editor__fallback"
      spellcheck="false"
      :value="modelValue"
      @input="handleFallbackInput"
      @focus="handleFallbackFocus"
      @blur="handleFallbackBlur"
      @keydown="handleFallbackKeydown"
    />
  </div>
</template>

<script setup>
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import 'monaco-editor/esm/vs/basic-languages/markdown/markdown.contribution'
import 'monaco-editor/esm/vs/editor/contrib/clipboard/browser/clipboard.js'
import 'monaco-editor/esm/vs/editor/standalone/browser/quickAccess/standaloneCommandsQuickAccess.js'
import { createLogger } from '../../utils/logger'
import { initLspClient } from '../../monaco/lspClient'
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

const emit = defineEmits(['update:modelValue', 'focus', 'request-exit'])

const logger = createLogger('GuaxiNotebookMarkdownEditor')
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
let currentAppliedTheme = ''
let mountMonacoEditorPromise = null
let usesVscodeModelBridge = false

const getNotebookMarkdownModelOwner = () => {
  const cellKey = String(props.cellKey || 'markdown-cell').trim() || 'markdown-cell'
  const documentPath = String(props.documentPath || '').trim() || 'anonymous-document'
  return `notebook-markdown:${documentPath}:${cellKey}`
}

const buildNotebookMarkdownVirtualFilePath = () => {
  const rawCellKey = String(props.cellKey || 'markdown-cell').trim() || 'markdown-cell'
  const safeCellKey = rawCellKey.replace(/[^a-zA-Z0-9_-]/g, '_')
  const rawDocumentPath = String(props.documentPath || '').trim()
  if (!rawDocumentPath) {
    return `file:///guaxi-notebook/${safeCellKey}.md`
  }

  // 设计说明：
  // 1. Explorer/Notebook 模式里的 cell 不是磁盘上的真实文件，但 LSP 对稳定的 file:// URI 更友好。
  // 2. 这里把 cell 挂到当前文档同目录下的虚拟 .guaxi-notebook 命名空间里，让补全、hover、workspace 推断都更稳定。
  const separator = rawDocumentPath.includes('\\') ? '\\' : '/'
  const normalizedDocumentPath = rawDocumentPath.replace(/[\\/]+$/, '')
  const lastSeparatorIndex = Math.max(
    normalizedDocumentPath.lastIndexOf('/'),
    normalizedDocumentPath.lastIndexOf('\\')
  )
  if (lastSeparatorIndex < 0) {
    return `file:///guaxi-notebook/${safeCellKey}.md`
  }
  const directory = normalizedDocumentPath.slice(0, lastSeparatorIndex)
  const baseName = normalizedDocumentPath
    .slice(lastSeparatorIndex + 1)
    .replace(/\.[^./\\]+$/, '') || 'notebook'
  return `${directory}${separator}.guaxi-notebook${separator}${baseName}__${safeCellKey}.md`
}

// 设计说明：
// 1. Markdown Cell 进入编辑态后，也应该像正文编辑一样尽量贴合当前内容。
// 2. 默认最小高度只保证一行内容可见，避免新建空 Markdown Cell 时出现大面积空白。
const MARKDOWN_EDITOR_LINE_HEIGHT = 24
const MARKDOWN_EDITOR_PADDING_VERTICAL = 28
const MARKDOWN_EDITOR_MIN_CONTENT_HEIGHT = MARKDOWN_EDITOR_LINE_HEIGHT + MARKDOWN_EDITOR_PADDING_VERTICAL
const MARKDOWN_EDITOR_MIN_HOST_HEIGHT = MARKDOWN_EDITOR_MIN_CONTENT_HEIGHT + 2

const getMonacoThemeName = () => {
  return String(props.theme || '').toLowerCase() === 'dark' ? 'guaxi-dark' : 'guaxi-light'
}

const ensureHostHeight = () => {
  const host = hostRef.value
  if (!(host instanceof HTMLElement)) return
  if (!editor) {
    host.style.height = `${MARKDOWN_EDITOR_MIN_HOST_HEIGHT}px`
    return
  }
  const contentHeight = Math.max(Number(editor.getContentHeight?.() || 0), MARKDOWN_EDITOR_MIN_CONTENT_HEIGHT)
  host.style.height = `${contentHeight + 2}px`
  editor.layout()
  logger.debug('[NotebookMarkdownEditor] synced editor height', {
    cellKey: String(props.cellKey || ''),
    contentHeight,
    hostHeight: contentHeight + 2,
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
    logger.warn('[NotebookMarkdownEditor] apply custom theme failed, fallback to Monaco built-in theme', {
      cellKey: String(props.cellKey || ''),
      error: String(error?.message || error),
      fallbackTheme,
    })
  }
}

const focusEditor = async (options = {}) => {
  const placeCursorAtEnd = options?.cursor === 'end'
  await nextTick()

  if (!editor && !loadFailed.value) {
    const ensureHostReady = async (attempt = 0) => {
      if (hostRef.value instanceof HTMLElement) return true
      if (attempt >= 3) return false
      await new Promise((resolve) => window.requestAnimationFrame(resolve))
      return ensureHostReady(attempt + 1)
    }

    const hostReady = await ensureHostReady()
    if (hostReady) {
      observer?.disconnect?.()
      observer = null
      await mountMonacoEditor()
      await nextTick()
    }
  }

  if (editor && model) {
    editor.focus()
    if (placeCursorAtEnd) {
      const lineCount = Math.max(1, model.getLineCount())
      const lastColumn = model.getLineMaxColumn(lineCount)
      editor.setPosition({ lineNumber: lineCount, column: lastColumn })
      editor.revealPositionInCenterIfOutsideViewport({ lineNumber: lineCount, column: lastColumn })
    }
    logger.info('[NotebookMarkdownEditor] focus Monaco editor', {
      cellKey: String(props.cellKey || ''),
      cursor: placeCursorAtEnd ? 'end' : 'keep',
    })
    return true
  }

  const fallback = hostRef.value?.querySelector?.('textarea')
  if (fallback instanceof HTMLTextAreaElement) {
    fallback.focus()
    if (placeCursorAtEnd) {
      const end = fallback.value.length
      fallback.setSelectionRange(end, end)
    }
    logger.info('[NotebookMarkdownEditor] focus fallback editor', {
      cellKey: String(props.cellKey || ''),
      cursor: placeCursorAtEnd ? 'end' : 'keep',
    })
    return true
  }

  logger.warn('[NotebookMarkdownEditor] focus skipped because editor host is unavailable', {
    cellKey: String(props.cellKey || ''),
    hasHost: hostRef.value instanceof HTMLElement,
    loadFailed: loadFailed.value,
  })
  return false
}

const getEditorValue = () => {
  if (editor && model) {
    return String(model.getValue() || '')
  }
  const fallback = hostRef.value?.querySelector?.('textarea')
  if (fallback instanceof HTMLTextAreaElement) {
    return String(fallback.value || '')
  }
  return String(props.modelValue || '')
}

const shouldKeepEditingForActiveElement = (element) => {
  if (!(element instanceof Element)) return false
  if (hostRef.value instanceof HTMLElement && hostRef.value.contains(element)) return true
  return !!element.closest?.('.suggest-widget, .monaco-editor-hover, .monaco-hover, .context-view, .monaco-menu-container')
}

const requestBlurExit = (reason = 'blur') => {
  window.setTimeout(() => {
    const activeElement = document.activeElement
    if (shouldKeepEditingForActiveElement(activeElement)) {
      logger.debug('[NotebookMarkdownEditor] keep edit mode after blur because focus moved inside editor ecosystem', {
        cellKey: String(props.cellKey || ''),
        reason,
        activeTag: String(activeElement?.tagName || '').toLowerCase(),
      })
      return
    }
    logger.info('[NotebookMarkdownEditor] request exit because editor lost focus', {
      cellKey: String(props.cellKey || ''),
      reason,
      activeTag: String(activeElement?.tagName || '').toLowerCase(),
    })
    emit('request-exit', reason)
  }, 0)
}

const releaseEditorFocusToPage = (reason = 'manual-release') => {
  if (editor?.hasTextFocus?.()) {
    editor.setPosition(editor.getPosition())
    editor.getDomNode()?.blur?.()
  }
  hostRef.value?.focus?.()
  logger.info('[NotebookMarkdownEditor] release editor focus back to notebook page', {
    cellKey: String(props.cellKey || ''),
    reason,
  })
}

const mountMonacoEditor = async () => {
  if (editor || loadFailed.value) return editor
  if (mountMonacoEditorPromise) {
    logger.debug('[NotebookMarkdownEditor] reuse pending Monaco mount promise', {
      cellKey: String(props.cellKey || ''),
    })
    return mountMonacoEditorPromise
  }

  const host = hostRef.value
  if (!(host instanceof HTMLElement)) return null

  mountMonacoEditorPromise = (async () => {
    try {
      monacoRef = await import('monaco-editor')

      const existingEditor = monacoRef?.editor?.getEditors?.()
        ?.find?.((item) => item?.getContainerDomNode?.() === host) || null
      if (existingEditor) {
        editor = existingEditor
        model = editor.getModel?.() || null
        if (model?.uri) {
          const retainedModel = await retainVsCodeTextModel({
            resource: model.uri,
            content: model.getValue?.() || String(props.modelValue || ''),
            languageId: model.getLanguageId?.() || 'markdown',
            owner: getNotebookMarkdownModelOwner(),
            reason: 'notebook-markdown-reuse-existing-editor',
          })
          if (retainedModel) {
            model = retainedModel
            editor.setModel(model)
            usesVscodeModelBridge = true
          }
        }
        editorReady.value = true
        ensureHostHeight()
        logger.warn('[NotebookMarkdownEditor] reused existing Monaco editor bound to current host', {
          cellKey: String(props.cellKey || ''),
        })
        return editor
      }

      const virtualFilePath = buildNotebookMarkdownVirtualFilePath()
      const virtualUri = String(virtualFilePath || '').startsWith('file://')
        ? monacoRef.Uri.parse(virtualFilePath)
        : monacoRef.Uri.file(virtualFilePath)
      model = await retainVsCodeTextModel({
        resource: virtualUri,
        content: String(props.modelValue || ''),
        languageId: 'markdown',
        owner: getNotebookMarkdownModelOwner(),
        reason: 'notebook-markdown-mount',
      })
      usesVscodeModelBridge = !!model
      if (!model) {
        // 设计说明：
        // 1. Markdown notebook cell 需要被 VSCode 文档服务识别，代码块内嵌补全才能沿着同一个 URI 工作。
        // 2. 这里保留 standalone model 兜底，只在 bridge 失败时使用，避免编辑器挂掉。
        model = monacoRef.editor.createModel(String(props.modelValue || ''), 'markdown', virtualUri)
        logger.warn('[NotebookMarkdownEditor] VSCode model bridge unavailable, fallback to standalone Monaco model', {
          cellKey: String(props.cellKey || ''),
          virtualFilePath,
        })
      } else {
        logger.info('[NotebookMarkdownEditor] retained VSCode model bridge for notebook cell', {
          cellKey: String(props.cellKey || ''),
          virtualFilePath,
          owner: getNotebookMarkdownModelOwner(),
        })
      }
      applyTheme()
      editor = monacoRef.editor.create(host, {
        model,
        language: 'markdown',
        minimap: { enabled: false },
        automaticLayout: true,
        lineNumbers: 'on',
        glyphMargin: false,
        folding: false,
        roundedSelection: false,
        scrollBeyondLastLine: false,
        wordWrap: 'on',
        // 设计说明：
        // 1. Markdown Cell 的候选应来自 Marksman/VSCode completion provider，也就是 monaco-languageclient 注册的标准链路。
        // 2. 关闭文档词兜底，避免 LSP 连接失败时 Monaco 用当前文档碎片伪装成“语义补全”。
        // 3. 这样如果仍然没有候选，日志里的 provider/LSP 状态就是排查根因的单一入口。
        wordBasedSuggestions: 'off',
        quickSuggestions: true,
        suggestOnTriggerCharacters: true,
        suggest: {
          showWords: false,
          snippetsPreventQuickSuggestions: false,
        },
        tabSize: 2,
        insertSpaces: true,
        renderLineHighlight: 'none',
        occurrencesHighlight: 'off',
        selectionHighlight: false,
        fontFamily: 'var(--editor-font-family, "Maple Mono NF CN", "JetBrainsMono Nerd Font Mono", Consolas, monospace)',
        fontSize: 17,
        lineHeight: MARKDOWN_EDITOR_LINE_HEIGHT,
        padding: { top: 14, bottom: 14 },
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

      contentChangeDisposable = editor.onDidChangeModelContent(() => {
        const nextValue = editor?.getValue?.() || ''
        emit('update:modelValue', nextValue)
      })

      contentSizeDisposable = editor.onDidContentSizeChange(() => {
        ensureHostHeight()
      })

      focusDisposable = editor.onDidFocusEditorText(() => {
        emit('focus')
      })
      blurDisposable = editor.onDidBlurEditorText(() => {
        requestBlurExit('blur')
      })

      editor.addCommand(monacoRef.KeyCode.Escape, () => {
        releaseEditorFocusToPage('escape')
        emit('request-exit', 'escape')
      })
      editor.addCommand(monacoRef.KeyMod.CtrlCmd | monacoRef.KeyCode.Enter, () => {
        emit('request-exit', 'ctrl-enter')
      })

      editorReady.value = true
      await nextTick()
      ensureHostHeight()
      initLspClient(editor, 'markdown', virtualFilePath, {
        workspacePath: String(props.documentPath || '').trim() || virtualFilePath,
        documentUri: virtualUri.toString(),
      })
      logger.info('[NotebookMarkdownEditor] LSP init requested for notebook cell', {
        cellKey: String(props.cellKey || ''),
        language: 'markdown',
        virtualFilePath,
        documentPath: String(props.documentPath || ''),
      })
      logger.info('[NotebookMarkdownEditor] mounted Monaco editor', {
        cellKey: String(props.cellKey || ''),
        theme: currentAppliedTheme,
      })
      return editor
    } catch (error) {
      loadFailed.value = true
      logger.error('[NotebookMarkdownEditor] failed to mount Monaco editor, fallback to textarea', {
        cellKey: String(props.cellKey || ''),
        error: String(error?.message || error),
      })
      return null
    } finally {
      mountMonacoEditorPromise = null
    }
  })()

  return mountMonacoEditorPromise
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
  editor?.dispose?.()
  editor = null
  if (usesVscodeModelBridge && currentModelUri) {
    releaseVsCodeTextModel(currentModelUri, getNotebookMarkdownModelOwner(), 'notebook-markdown-dispose')
  } else {
    model?.dispose?.()
  }
  model = null
  monacoRef = null
  mountMonacoEditorPromise = null
  editorReady.value = false
  usesVscodeModelBridge = false
}

const handleFallbackInput = (event) => {
  emit('update:modelValue', String(event?.target?.value || ''))
}

const handleFallbackFocus = () => {
  emit('focus')
}

const handleFallbackBlur = () => {
  requestBlurExit('blur')
}

const handleFallbackKeydown = (event) => {
  if (event?.key === 'Escape') {
    event.preventDefault()
    emit('request-exit', 'escape')
    return
  }
  if ((event?.ctrlKey || event?.metaKey) && event?.key === 'Enter') {
    event.preventDefault()
    emit('request-exit', 'ctrl-enter')
  }
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
  // 1. Markdown / Python notebook cell 共享同一套外部回推策略，避免两个编辑器在 bridge model 上行为分叉。
  // 2. 统一改用带日志的安全同步封装，后续若再出现内容回推异常，可以直接从日志定位到具体 cell 和 URI。
  replaceModelContentPreserveUndo(model, normalizedValue, {
    logPrefix: '[NotebookMarkdownEditor] synchronize external cell content',
    uri: String(model?.uri?.toString?.() || ''),
    owner: getNotebookMarkdownModelOwner(),
    reason: 'props-model-value-watch',
  })
  logger.debug('[NotebookMarkdownEditor] applied external model sync', {
    cellKey: String(props.cellKey || ''),
    nextLength: normalizedValue.length,
  })
  ensureHostHeight()
})

defineExpose({
  focusEditor,
  getEditorValue,
})
</script>

<style scoped>
.guaxi-notebook-markdown-editor {
  position: relative;
  width: 100%;
  min-height: 54px;
  outline: none;
  border-radius: 14px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border, rgba(15, 23, 42, 0.1)) 100%, transparent);
  background: var(--guaxi-notebook-panel-soft, #f8fafc);
  overflow: hidden;
}

.guaxi-notebook-markdown-editor__placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 54px;
  color: var(--guaxi-notebook-muted, rgba(23, 32, 51, 0.72));
  font-family: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace);
  font-size: 12px;
  background:
    linear-gradient(135deg, color-mix(in srgb, var(--guaxi-notebook-panel, #ffffff) 94%, transparent), color-mix(in srgb, var(--guaxi-notebook-panel-soft, #f8fafc) 90%, transparent));
}

.guaxi-notebook-markdown-editor__fallback {
  width: 100%;
  min-height: 54px;
  resize: vertical;
  border: none;
  outline: none;
  padding: 14px 16px;
  background: transparent;
  color: var(--guaxi-notebook-text, #172033);
  font-family: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace);
  font-size: 14px;
  line-height: 1.7;
  box-sizing: border-box;
}
</style>
