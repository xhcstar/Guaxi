<template>
  <div
    ref="hostRef"
    class="guaxi-notebook-markdown-editor"
    :class="{ 'is-pending': !editorReady && !loadFailed, 'is-fallback': loadFailed }"
    :data-cell-key="cellKey"
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

const props = defineProps({
  modelValue: {
    type: String,
    default: '',
  },
  theme: {
    type: String,
    default: 'light',
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
        editorReady.value = true
        ensureHostHeight()
        logger.warn('[NotebookMarkdownEditor] reused existing Monaco editor bound to current host', {
          cellKey: String(props.cellKey || ''),
        })
        return editor
      }

      model = monacoRef.editor.createModel(String(props.modelValue || ''), 'markdown')
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
        quickSuggestions: true,
        suggestOnTriggerCharacters: true,
        tabSize: 2,
        insertSpaces: true,
        renderLineHighlight: 'none',
        occurrencesHighlight: 'off',
        selectionHighlight: false,
        fontFamily: 'var(--editor-font-family, "Maple Mono NF CN", "JetBrainsMono Nerd Font Mono", Consolas, monospace)',
        fontSize: 14,
        lineHeight: MARKDOWN_EDITOR_LINE_HEIGHT,
        padding: { top: 14, bottom: 14 },
        scrollbar: {
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
        emit('request-exit', 'escape')
      })
      editor.addCommand(monacoRef.KeyMod.CtrlCmd | monacoRef.KeyCode.Enter, () => {
        emit('request-exit', 'ctrl-enter')
      })

      editorReady.value = true
      await nextTick()
      ensureHostHeight()
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
  model?.dispose?.()
  model = null
  monacoRef = null
  mountMonacoEditorPromise = null
  editorReady.value = false
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
  model.pushEditOperations([], [{
    range: model.getFullModelRange(),
    text: normalizedValue,
  }], () => null)
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
