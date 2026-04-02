<template>
  <div
    ref="hostRef"
    class="guaxi-notebook-python-editor"
    :class="{ 'is-pending': !editorReady && !loadFailed, 'is-fallback': loadFailed }"
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
    />
  </div>
</template>

<script setup>
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
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

const emit = defineEmits(['update:modelValue'])

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
let currentAppliedTheme = ''

const getMonacoThemeName = () => {
  return String(props.theme || '').toLowerCase() === 'dark' ? 'guaxi-dark' : 'guaxi-light'
}

const ensureHostHeight = () => {
  const host = hostRef.value
  if (!(host instanceof HTMLElement)) return
  if (!editor) {
    host.style.height = '168px'
    return
  }
  const contentHeight = Math.max(Number(editor.getContentHeight?.() || 0), 168)
  host.style.height = `${contentHeight + 2}px`
  editor.layout()
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
    model = monacoRef.editor.createModel(String(props.modelValue || ''), 'python')
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
      quickSuggestions: true,
      suggestOnTriggerCharacters: true,
      tabSize: 4,
      insertSpaces: true,
      fontFamily: 'var(--editor-font-family, "Maple Mono NF CN", "JetBrainsMono Nerd Font Mono", Consolas, monospace)',
      fontSize: 13,
      lineHeight: 22,
      padding: { top: 12, bottom: 12 },
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

    editorReady.value = true
    await nextTick()
    ensureHostHeight()
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
  contentChangeDisposable?.dispose?.()
  contentChangeDisposable = null
  contentSizeDisposable?.dispose?.()
  contentSizeDisposable = null
  editor?.dispose?.()
  editor = null
  model?.dispose?.()
  model = null
  monacoRef = null
  editorReady.value = false
}

const handleFallbackInput = (event) => {
  emit('update:modelValue', String(event?.target?.value || ''))
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
  const currentValue = model.getValue()
  if (currentValue === String(nextValue || '')) return
  model.pushEditOperations([], [{
    range: model.getFullModelRange(),
    text: String(nextValue || ''),
  }], () => null)
  ensureHostHeight()
})
</script>

<style scoped>
.guaxi-notebook-python-editor {
  position: relative;
  width: 100%;
  min-height: 168px;
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-border, rgba(15, 23, 42, 0.1)) 100%, transparent);
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft, #f8fafc) 92%, transparent);
  overflow: hidden;
}

.guaxi-notebook-python-editor__placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 168px;
  color: var(--guaxi-notebook-muted, rgba(23, 32, 51, 0.72));
  font-family: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace);
  font-size: 12px;
  background:
    linear-gradient(135deg, color-mix(in srgb, var(--guaxi-notebook-panel, #ffffff) 94%, transparent), color-mix(in srgb, var(--guaxi-notebook-panel-soft, #f8fafc) 90%, transparent));
}

.guaxi-notebook-python-editor__fallback {
  width: 100%;
  min-height: 168px;
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
