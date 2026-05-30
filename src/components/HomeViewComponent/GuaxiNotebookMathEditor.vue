<template>
  <div
    ref="hostRef"
    class="guaxi-notebook-math-editor"
    :class="{ 'is-pending': !editorReady && !loadFailed, 'is-fallback': loadFailed }"
    tabindex="-1"
  >
    <div v-if="!editorReady && !loadFailed" class="guaxi-notebook-math-editor__placeholder">
      <span>正在加载数学编辑器...</span>
    </div>
    <textarea
      v-if="loadFailed"
      class="guaxi-notebook-math-editor__fallback"
      spellcheck="false"
      :value="modelValue"
      :placeholder="placeholder"
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
  placeholder: {
    type: String,
    default: '输入 LaTeX / SymPy 公式，例如 sin(x) / x',
  },
})

const emit = defineEmits(['update:modelValue', 'focus', 'blur'])

const logger = createLogger('GuaxiNotebookMathEditor')
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

const MATH_EDITOR_LINE_HEIGHT = 22
const MATH_EDITOR_PADDING_VERTICAL = 24
const MATH_EDITOR_MIN_CONTENT_HEIGHT = MATH_EDITOR_LINE_HEIGHT * 3 + MATH_EDITOR_PADDING_VERTICAL
const MATH_EDITOR_MIN_HOST_HEIGHT = MATH_EDITOR_MIN_CONTENT_HEIGHT + 2

const getMonacoThemeName = () => {
  return String(props.theme || '').toLowerCase() === 'dark' ? 'guaxi-dark' : 'guaxi-light'
}

const ensureLatexLanguage = () => {
  if (!monacoRef?.languages) return
  const exists = monacoRef.languages.getLanguages().some((lang) => lang.id === 'latex')
  if (!exists) {
    monacoRef.languages.register({
      id: 'latex',
      extensions: ['.tex', '.ltx', '.latex'],
      aliases: ['LaTeX', 'TeX', 'latex', 'tex'],
      mimetypes: ['text/x-tex', 'text/x-latex'],
    })
  }

  // 设计说明：
  // 1. 数学块允许 LaTeX 与 SymPy 表达式混写，Monaco 没有内置 LaTeX contribution，因此这里提供轻量 Monarch 兜底。
  // 2. 只覆盖常见命令、括号、注释和数字，避免把数学语义写死在前端；真正的解析仍交给 Python/SymPy。
  // 3. 该注册是幂等的，多个数学块共享同一个 language id，不会为每个 cell 反复制造语言实例。
  monacoRef.languages.setMonarchTokensProvider('latex', {
    tokenizer: {
      root: [
        [/%.*$/, 'comment'],
        [/(\\(?:begin|end))(\s*)(\{)([A-Za-z*_-][\w*:-]*)(\})/, ['keyword', 'white', 'delimiter.bracket', 'type', 'delimiter.bracket']],
        [/\\(?:frac|sqrt|sum|prod|int|lim|sin|cos|tan|log|ln|exp|left|right)\b/, 'keyword'],
        [/\\[A-Za-z@]+\*?/, 'keyword'],
        [/\\./, 'operator'],
        [/\{|\}|\[|\]|\(|\)/, 'delimiter.bracket'],
        [/\$+\s*|\s*\$+/, 'string.delimiter'],
        [/\b\d+(?:\.\d+)?\b/, 'number'],
        [/[+\-*/^=<>]/, 'operator'],
      ],
    },
  })
  logger.info('[NotebookMathEditor] LaTeX language ready', {
    cellKey: String(props.cellKey || ''),
    existed: exists,
  })
}

const buildMathVirtualUri = () => {
  const rawCellKey = String(props.cellKey || 'math-cell').trim() || 'math-cell'
  const safeCellKey = rawCellKey.replace(/[^a-zA-Z0-9_-]/g, '_')
  const rawDocumentPath = String(props.documentPath || '').trim()
  if (!rawDocumentPath) return monacoRef.Uri.parse(`inmemory://guaxi-notebook-math/${safeCellKey}.tex`)

  const separator = rawDocumentPath.includes('\\') ? '\\' : '/'
  const normalizedDocumentPath = rawDocumentPath.replace(/[\\/]+$/, '')
  const lastSeparatorIndex = Math.max(
    normalizedDocumentPath.lastIndexOf('/'),
    normalizedDocumentPath.lastIndexOf('\\'),
  )
  if (lastSeparatorIndex < 0) return monacoRef.Uri.parse(`inmemory://guaxi-notebook-math/${safeCellKey}.tex`)
  const directory = normalizedDocumentPath.slice(0, lastSeparatorIndex)
  const baseName = normalizedDocumentPath
    .slice(lastSeparatorIndex + 1)
    .replace(/\.[^./\\]+$/, '') || 'notebook'
  return monacoRef.Uri.file(`${directory}${separator}.guaxi-notebook${separator}${baseName}__${safeCellKey}.tex`)
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
    logger.warn('[NotebookMathEditor] apply custom theme failed, fallback to Monaco built-in theme', {
      cellKey: String(props.cellKey || ''),
      error: String(error?.message || error),
      fallbackTheme,
    })
  }
}

const ensureHostHeight = () => {
  const host = hostRef.value
  if (!(host instanceof HTMLElement)) return
  if (!editor) {
    host.style.height = `${MATH_EDITOR_MIN_HOST_HEIGHT}px`
    return
  }
  const contentHeight = Math.max(Number(editor.getContentHeight?.() || 0), MATH_EDITOR_MIN_CONTENT_HEIGHT)
  host.style.height = `${contentHeight + 2}px`
  editor.layout()
  logger.debug('[NotebookMathEditor] synced editor height', {
    cellKey: String(props.cellKey || ''),
    contentHeight,
    hostHeight: contentHeight + 2,
  })
}

const blurEditorToPage = (reason = 'manual-blur') => {
  if (editor?.hasTextFocus?.()) {
    editor.setPosition(editor.getPosition())
    editor.getDomNode()?.blur?.()
  }
  hostRef.value?.focus?.()
  emit('blur')
  logger.info('[NotebookMathEditor] release editor focus back to notebook page', {
    cellKey: String(props.cellKey || ''),
    reason,
  })
}

const mountMonacoEditor = async () => {
  if (editor || loadFailed.value) return
  const host = hostRef.value
  if (!(host instanceof HTMLElement)) return

  try {
    monacoRef = await import('monaco-editor')
    ensureLatexLanguage()
    const virtualUri = buildMathVirtualUri()
    model = monacoRef.editor.createModel(String(props.modelValue || ''), 'latex', virtualUri)
    applyTheme()
    editor = monacoRef.editor.create(host, {
      model,
      language: 'latex',
      minimap: { enabled: false },
      automaticLayout: true,
      // 设计说明：数学块与 Python Cell 都承担“可执行源码”的角色，因此保留行号，
      // 方便用户在多行方程组、微分方程和积分方程报错时对齐后端 traceback。
      lineNumbers: 'on',
      lineNumbersMinChars: 3,
      glyphMargin: false,
      // 设计说明：多行方程组 / 分段函数 / 长微分方程常按缩进分层书写，开启折叠后
      // 可按缩进收起较长的子块，与 Python Cell 的折叠体验保持一致。
      folding: true,
      foldingStrategy: 'indentation',
      showFoldingControls: 'mouseover',
      roundedSelection: false,
      scrollBeyondLastLine: false,
      // 设计说明：
      // 1. 数学块输入既可能是可折行的说明文字，也可能是很长的 LaTeX/SymPy 单词串。
      // 2. `wordWrap:on` 优先让普通长行随容器折行；遇到 Monaco 不能安全断开的长 token 时，
      //    交给编辑器自己的水平滚动条处理，避免把探索模式中间区域整体撑宽。
      wordWrap: 'on',
      wrappingIndent: 'same',
      wrappingStrategy: 'advanced',
      quickSuggestions: false,
      wordBasedSuggestions: 'off',
      renderLineHighlight: 'line',
      tabSize: 4,
      insertSpaces: true,
      fontFamily: 'var(--editor-font-family, "Maple Mono NF CN", "JetBrainsMono Nerd Font Mono", Consolas, monospace)',
      fontSize: 18,
      lineHeight: MATH_EDITOR_LINE_HEIGHT,
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

    contentChangeDisposable = editor.onDidChangeModelContent(() => {
      emit('update:modelValue', editor?.getValue?.() || '')
    })
    contentSizeDisposable = editor.onDidContentSizeChange(() => {
      ensureHostHeight()
    })
    focusDisposable = editor.onDidFocusEditorText(() => {
      emit('focus')
      logger.debug('[NotebookMathEditor] editor focused', {
        cellKey: String(props.cellKey || ''),
      })
    })
    blurDisposable = editor.onDidBlurEditorText(() => {
      emit('blur')
      logger.debug('[NotebookMathEditor] editor blurred', {
        cellKey: String(props.cellKey || ''),
      })
    })
    editor.addCommand(monacoRef.KeyCode.Escape, () => {
      blurEditorToPage('escape')
    })

    editorReady.value = true
    await nextTick()
    ensureHostHeight()
    logger.info('[NotebookMathEditor] mounted Monaco math editor', {
      cellKey: String(props.cellKey || ''),
      theme: currentAppliedTheme,
      uri: String(virtualUri?.toString?.() || ''),
    })
  } catch (error) {
    loadFailed.value = true
    logger.error('[NotebookMathEditor] failed to mount Monaco math editor, fallback to textarea', {
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
  focusDisposable?.dispose?.()
  focusDisposable = null
  blurDisposable?.dispose?.()
  blurDisposable = null
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

const handleFallbackKeydown = (event) => {
  if (event?.key !== 'Escape') return
  event.preventDefault()
  event?.target?.blur?.()
  hostRef.value?.focus?.()
  emit('blur')
  logger.info('[NotebookMathEditor] release fallback editor focus back to notebook page', {
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
  if (model.getValue() === normalizedValue) return
  model.setValue(normalizedValue)
  ensureHostHeight()
})
</script>

<style scoped>
.guaxi-notebook-math-editor {
  width: 100%;
  max-width: 100%;
  min-width: 0;
  overflow: hidden;
  border: 1px solid var(--guaxi-notebook-strong-border);
  border-radius: 8px;
  background: var(--guaxi-notebook-panel-soft);
  box-sizing: border-box;
}

.guaxi-notebook-math-editor:focus-within {
  border-color: color-mix(in srgb, #0891b2 52%, var(--guaxi-notebook-primary));
  box-shadow: 0 0 0 3px color-mix(in srgb, #0891b2 12%, transparent);
}

.guaxi-notebook-math-editor__placeholder,
.guaxi-notebook-math-editor__fallback {
  width: 100%;
  min-height: 90px;
  box-sizing: border-box;
}

.guaxi-notebook-math-editor__placeholder {
  display: flex;
  align-items: center;
  padding: 10px 12px;
  color: var(--guaxi-notebook-muted);
  font-family: var(--guaxi-notebook-font);
  font-size: 13px;
}

.guaxi-notebook-math-editor__fallback {
  resize: vertical;
  padding: 10px 12px;
  border: 0;
  outline: none;
  background: transparent;
  color: var(--guaxi-notebook-text);
  font-family: var(--guaxi-notebook-mono-font);
  font-size: 13px;
  line-height: 22px;
}

:deep(.monaco-editor),
:deep(.monaco-editor-background),
:deep(.monaco-editor .margin) {
  background: var(--guaxi-notebook-panel-soft);
}
</style>
