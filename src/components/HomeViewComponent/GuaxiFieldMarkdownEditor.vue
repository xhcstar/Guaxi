<template>
  <div
    ref="hostRef"
    class="guaxi-field-md-editor"
    :class="{ 'is-fill': fill, 'is-ready': editorReady, 'is-fallback': loadFailed }"
    :style="hostStyle"
    tabindex="-1"
  >
    <div v-if="!editorReady && !loadFailed" class="guaxi-field-md-editor__placeholder">
      正在加载编辑器…
    </div>
    <textarea
      v-if="loadFailed"
      class="guaxi-field-md-editor__fallback"
      spellcheck="false"
      :value="modelValue"
      @input="handleFallbackInput"
    />
  </div>
</template>

<script setup>
/**
 * 任务字段的 Monaco 编辑器。
 *
 * 与主编辑器同源：Markdown 语法、围栏代码块高亮、共享右键菜单、主题色板，
 * 以及主编辑器注册的「文件路径补全」provider —— 字段模型同样是 markdown 语言，
 * 所以输入 `./`、`../`、`/` 时会跟正文一样弹出目录候选。
 *
 * 刻意不启用：行号（默认关闭）、LSP（字段是片段而不是文档）、Git 装饰。
 */
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import 'monaco-editor/esm/vs/basic-languages/markdown/markdown.contribution'
import 'monaco-editor/esm/vs/editor/contrib/clipboard/browser/clipboard.js'
import { createLogger } from '../../utils/logger'
import { attachSharedEditorFeatures, resolveSharedEditorDisplayOptions } from '../../monaco/editorFeatures'
import { createMarkdownFenceHighlighter } from '../../monaco/markdownFenceHighlight'
import { defineGuaxiMonacoThemes, getGuaxiMonacoTheme } from '../../monaco/guaxiMonacoThemes'

const props = defineProps({
  modelValue: { type: String, default: '' },
  theme: { type: String, default: 'light' },
  documentPath: { type: String, default: '' },
  fieldKey: { type: String, default: 'field' },
  minHeight: { type: Number, default: 96 },
  maxHeight: { type: Number, default: 360 },
  /** 占满父容器（放大编辑浮层），不按内容自动伸缩。 */
  fill: { type: Boolean, default: false },
  /** 立即挂载 Monaco；默认等组件进入视口附近再挂载。 */
  immediate: { type: Boolean, default: false },
})

const emit = defineEmits(['update:modelValue'])

const logger = createLogger('GuaxiFieldMarkdownEditor')
const hostRef = ref(null)
const editorReady = ref(false)
const loadFailed = ref(false)
const hostHeight = ref(Math.max(64, props.minHeight))

let observer = null
let monacoRef = null
let editor = null
let model = null
let featuresDisposable = null
let fenceHighlighter = null
let contentChangeDisposable = null
let contentSizeDisposable = null
let mountPromise = null
let lastEmittedValue = null
// 每个组件实例一个后缀，只用于模型 URI 去重。
const instanceId = Math.random().toString(36).slice(2, 8)

const hostStyle = computed(() => {
  if (props.fill) return {}
  return { height: `${hostHeight.value}px` }
})

const virtualUriOf = () => {
  const documentPath = String(props.documentPath || '').trim().replace(/\\/g, '/')
  const fieldKey = String(props.fieldKey || 'field').replace(/[^a-zA-Z0-9_-]/g, '_')
  // 每次挂载都换一个后缀：Monaco 的模型 URI 是全局注册的，卸载与重挂在同一帧里交错时
  // 同名 URI 会撞车（createModel 直接抛错），编辑器就再也打不开了。
  const instanceSuffix = `_${instanceId}`
  if (!documentPath) return monacoRef.Uri.parse(`inmemory://guaxi-org-fields/${fieldKey}${instanceSuffix}.md`)
  // 放在文档同目录下，便于任何按 URI 推断工作区的能力（如路径补全兜底）拿到正确基准。
  const dir = documentPath.slice(0, documentPath.lastIndexOf('/'))
  const base = documentPath.slice(documentPath.lastIndexOf('/') + 1).replace(/\.[^.\/]+$/, '') || 'org'
  return monacoRef.Uri.file(`${dir}/.guaxi-org-fields/${base}__${fieldKey}${instanceSuffix}.md`)
}

const syncHeight = () => {
  if (!editor) return
  if (props.fill) {
    editor.layout()
    return
  }
  const contentHeight = Math.max(Number(editor.getContentHeight?.() || 0), props.minHeight)
  hostHeight.value = Math.min(Math.max(contentHeight + 2, props.minHeight), props.maxHeight)
  editor.layout()
}

const applyTheme = () => {
  if (!monacoRef?.editor) return
  try {
    defineGuaxiMonacoThemes(monacoRef)
    monacoRef.editor.setTheme(getGuaxiMonacoTheme(props.theme))
  } catch (error) {
    logger.warn('[FieldEditor] apply theme failed', { error: String(error?.message || error) })
  }
}

const mountEditor = async () => {
  if (editor || loadFailed.value) return editor
  if (mountPromise) return mountPromise
  const host = hostRef.value
  if (!(host instanceof HTMLElement)) return null

  mountPromise = (async () => {
    try {
      monacoRef = await import('monaco-editor')
      defineGuaxiMonacoThemes(monacoRef)
      applyTheme()

      model = monacoRef.editor.createModel(String(props.modelValue || ''), 'markdown', virtualUriOf())
      const display = resolveSharedEditorDisplayOptions()

      editor = monacoRef.editor.create(host, {
        model,
        language: 'markdown',
        contextmenu: false,
        lineNumbers: 'off',
        lineDecorationsWidth: 8,
        lineNumbersMinChars: 0,
        glyphMargin: false,
        folding: false,
        renderLineHighlight: 'none',
        occurrencesHighlight: 'off',
        selectionHighlight: false,
        minimap: { enabled: false },
        automaticLayout: true,
        scrollBeyondLastLine: false,
        wordWrap: 'on',
        fixedOverflowWidgets: true,
        alwaysConsumeMouseWheel: false,
        quickSuggestions: { other: false, comments: false, strings: false },
        suggestOnTriggerCharacters: true,
        wordBasedSuggestions: 'off',
        suggest: { showWords: false, snippetsPreventQuickSuggestions: false },
        tabSize: 2,
        insertSpaces: true,
        fontFamily: display.fontFamily,
        fontSize: display.fontSize,
        lineHeight: display.lineHeight,
        padding: { top: 8, bottom: 8 },
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
        lastEmittedValue = nextValue
        emit('update:modelValue', nextValue)
      })
      contentSizeDisposable = editor.onDidContentSizeChange(syncHeight)

      featuresDisposable = attachSharedEditorFeatures({
        monaco: monacoRef,
        editor,
        language: 'markdown',
        enableSnippets: false,
        enableContextMenu: true,
        enableLsp: false,
        ensureThemes: () => defineGuaxiMonacoThemes(monacoRef),
      })
      fenceHighlighter = createMarkdownFenceHighlighter(editor)

      editorReady.value = true
      await nextTick()
      syncHeight()
      logger.info('[FieldEditor] mounted Monaco field editor', {
        fieldKey: props.fieldKey,
        documentPath: props.documentPath,
        fill: props.fill,
      })
      return editor
    } catch (error) {
      loadFailed.value = true
      logger.error('[FieldEditor] failed to mount Monaco, fallback to textarea', {
        fieldKey: props.fieldKey,
        error: String(error?.message || error),
      })
      return null
    } finally {
      mountPromise = null
    }
  })()

  return mountPromise
}

const disposeEditor = () => {
  featuresDisposable?.()
  featuresDisposable = null
  fenceHighlighter?.dispose?.()
  fenceHighlighter = null
  contentChangeDisposable?.dispose?.()
  contentChangeDisposable = null
  contentSizeDisposable?.dispose?.()
  contentSizeDisposable = null
  editor?.dispose?.()
  editor = null
  model?.dispose?.()
  model = null
  monacoRef = null
  mountPromise = null
  editorReady.value = false
}

const handleFallbackInput = (event) => {
  emit('update:modelValue', String(event?.target?.value || ''))
}

const focusEditor = async (options = {}) => {
  const placeCursorAtEnd = options?.cursor === 'end'
  if (!editor && !loadFailed.value) {
    await mountEditor()
    await nextTick()
  }
  if (editor && model) {
    editor.focus()
    if (placeCursorAtEnd) {
      const lineCount = Math.max(1, model.getLineCount())
      const lastColumn = model.getLineMaxColumn(lineCount)
      editor.setPosition({ lineNumber: lineCount, column: lastColumn })
      editor.revealPositionInCenterIfOutsideViewport({ lineNumber: lineCount, column: lastColumn })
    }
    return true
  }
  const fallback = hostRef.value?.querySelector?.('textarea')
  if (fallback instanceof HTMLTextAreaElement) {
    fallback.focus()
    if (placeCursorAtEnd) {
      const end = fallback.value.length
      fallback.setSelectionRange(end, end)
    }
    return true
  }
  return false
}

onMounted(() => {
  if (props.immediate || props.fill) {
    void mountEditor()
    return
  }
  const host = hostRef.value
  if (!(host instanceof HTMLElement) || typeof IntersectionObserver !== 'function') {
    void mountEditor()
    return
  }
  observer = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue
      observer?.disconnect()
      observer = null
      void mountEditor()
      break
    }
  }, { root: null, rootMargin: '200px 0px 200px 0px', threshold: 0.01 })
  observer.observe(host)
})

onBeforeUnmount(() => {
  observer?.disconnect()
  observer = null
  disposeEditor()
})

watch(() => props.theme, () => applyTheme())

watch(
  () => props.modelValue,
  (nextValue) => {
    if (!editor || !model) return
    if (nextValue === lastEmittedValue) return
    const normalized = String(nextValue || '')
    if (model.getValue() === normalized) return
    model.setValue(normalized)
    syncHeight()
  },
)

watch(
  () => props.fill,
  async () => {
    await nextTick()
    syncHeight()
  },
)

defineExpose({ focusEditor })
</script>

<style scoped>
.guaxi-field-md-editor {
  position: relative;
  width: 100%;
  min-height: 64px;
  border: 1px solid var(--otp-border, #d0d7de);
  border-radius: 8px;
  overflow: hidden;
  background: var(--otp-bg, #f7f8fa);
  box-sizing: border-box;
}

.guaxi-field-md-editor.is-fill {
  height: 100%;
  min-height: 0;
}

.guaxi-field-md-editor__placeholder {
  display: flex;
  align-items: center;
  padding: 10px 12px;
  min-height: 64px;
  font-size: 13px;
  color: var(--otp-sub, #6a737d);
}

.guaxi-field-md-editor__fallback {
  width: 100%;
  min-height: 96px;
  resize: vertical;
  border: none;
  outline: none;
  padding: 10px 12px;
  background: transparent;
  color: var(--otp-text, #1f2328);
  font: inherit;
  font-size: 14px;
  line-height: 1.7;
  box-sizing: border-box;
}
</style>
