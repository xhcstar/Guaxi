<template>
  <div class="aux-status-bar">
    <div class="aux-status-left">
      <slot name="leading" />
      <div class="aux-status-item" :title="languageTitle">{{ languageDisplay }}</div>
      <div
        class="aux-status-item"
        :class="{ 'has-errors': problems.errors > 0, 'has-warnings': problems.warnings > 0 }"
        :title="problemsTitle"
      >
        {{ problemsDisplay }}
      </div>
      <n-tooltip trigger="hover">
        <template #trigger>
          <div class="aux-status-item aux-lsp-item" :class="lspStatus.status === 'ok' ? 'is-ok' : 'is-bad'">
            <n-icon size="13">
              <CheckmarkCircleOutline v-if="lspStatus.status === 'ok'" />
              <AlertCircleOutline v-else />
            </n-icon>
            <span>{{ lspStatus.label }}</span>
          </div>
        </template>
        {{ lspStatus.detail }}
      </n-tooltip>
    </div>

    <div class="aux-status-right">
      <div class="aux-status-item" :title="lineEndingTitle" @click="toggleLineEnding">{{ lineEnding }}</div>
      <div class="aux-status-item">Ln {{ cursor.lineNumber }}, Col {{ cursor.column }}</div>
      <div class="aux-status-item" :title="indentationTitle" @click="toggleIndentation">{{ indentation }}</div>
      <div v-if="selectionText" class="aux-status-item">{{ selectionText }}</div>
    </div>
  </div>
</template>

<script setup>
// 辅助编辑器（模板管理器 / Slidev / 探索者 cell）用的状态栏。
//
// 与主编辑器的 GuaxiEditorStatusBar 职责相同，但完全由传入的 Monaco 实例驱动，
// 不依赖全局 editorStore——独立窗口里没有主编辑器，读 store 只会拿到空状态。
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import * as monaco from 'monaco-editor'
import { NIcon, NTooltip } from 'naive-ui'
import { AlertCircleOutline, CheckmarkCircleOutline } from '@vicons/ionicons5'
import { getLspConnectionState, onLspConnectionStateChange } from '../../monaco/lspClient'
import { isLspKnownLanguage } from '../../monaco/lspCapabilities'

const props = defineProps({
  editorInstance: {
    type: Object,
    default: null,
  },
  language: {
    type: String,
    default: '',
  },
})

const LANGUAGE_NAMES = {
  markdown: 'Markdown',
  javascript: 'JavaScript',
  typescript: 'TypeScript',
  html: 'HTML',
  css: 'CSS',
  json: 'JSON',
  python: 'Python',
  c: 'C',
  cpp: 'C++',
  rust: 'Rust',
  lua: 'Lua',
  shell: 'Shell',
  latex: 'LaTeX',
  java: 'Java',
  mermaid: 'Mermaid',
  plaintext: 'Plain Text',
}

const currentLanguage = ref(String(props.language || 'plaintext'))
const cursor = ref({ lineNumber: 1, column: 1 })
const selectionText = ref('')
const lineEnding = ref('LF')
const indentation = ref('Spaces: 2')
const problems = ref({ errors: 0, warnings: 0 })
const lspState = ref(getLspConnectionState(currentLanguage.value))

let disposers = []

const languageDisplay = computed(() => LANGUAGE_NAMES[currentLanguage.value] || currentLanguage.value)
const languageTitle = computed(() => `当前语言：${currentLanguage.value}`)
const problemsDisplay = computed(() => {
  const { errors, warnings } = problems.value
  if (!errors && !warnings) return 'No Problems'
  return `${errors} Error${errors === 1 ? '' : 's'}, ${warnings} Warning${warnings === 1 ? '' : 's'}`
})
const problemsTitle = computed(() => `${problems.value.errors} 个错误，${problems.value.warnings} 个警告`)
const lineEndingTitle = computed(() => `行尾符：${lineEnding.value}（点击切换）`)
const indentationTitle = computed(() => `缩进：${indentation.value}（点击切换空格/制表符）`)

const lspStatus = computed(() => {
  const languageId = String(currentLanguage.value || '').trim().toLowerCase()
  if (!languageId) return { status: 'bad', label: 'LSP 异常', detail: '未识别语言' }
  if (!isLspKnownLanguage(languageId)) return { status: 'bad', label: 'LSP 异常', detail: '当前语言未启用 LSP' }
  const state = lspState.value?.state || 'idle'
  if (state === 'connected') return { status: 'ok', label: 'LSP 正常', detail: '前后端通信正常' }
  if (state === 'connecting') return { status: 'bad', label: 'LSP 连接中', detail: '正在连接 LSP' }
  const reason = lspState.value?.reason ? `：${lspState.value.reason}` : ''
  return { status: 'bad', label: 'LSP 未连接', detail: `LSP 未连接${reason}` }
})

const refreshLspState = () => {
  lspState.value = getLspConnectionState(currentLanguage.value)
}

const refreshModelInfo = () => {
  const editor = props.editorInstance
  const model = editor?.getModel?.()
  if (!model) return
  currentLanguage.value = String(model.getLanguageId?.() || props.language || 'plaintext')
  const eol = model.getEOL?.()
  lineEnding.value = eol === '\r\n' ? 'CRLF' : 'LF'
  const options = model.getOptions?.() || {}
  indentation.value = options.insertSpaces
    ? `Spaces: ${options.tabSize || 2}`
    : 'Tabs'
  refreshMarkers()
  refreshLspState()
}

const refreshMarkers = () => {
  const model = props.editorInstance?.getModel?.()
  if (!model) {
    problems.value = { errors: 0, warnings: 0 }
    return
  }
  const markers = monaco.editor.getModelMarkers({ resource: model.uri }) || []
  let errors = 0
  let warnings = 0
  for (const marker of markers) {
    if (marker.severity === monaco.MarkerSeverity.Error) errors += 1
    else if (marker.severity === monaco.MarkerSeverity.Warning) warnings += 1
  }
  problems.value = { errors, warnings }
}

const refreshCursor = () => {
  const editor = props.editorInstance
  const position = editor?.getPosition?.()
  if (position) cursor.value = { lineNumber: position.lineNumber, column: position.column }
  updateSelection(editor)
}

const updateSelection = (editor) => {
  const selection = editor?.getSelection?.()
  const model = editor?.getModel?.()
  if (!selection || !model || selection.isEmpty()) {
    selectionText.value = ''
    return
  }
  const selected = model.getValueInRange(selection) || ''
  const chars = Array.from(selected).length
  const lines = selection.endLineNumber - selection.startLineNumber + 1
  selectionText.value = lines > 1 ? `${lines} 行, ${chars} 字符` : `${chars} 字符`
}

const bindEditor = () => {
  unbindEditor()
  const editor = props.editorInstance
  if (!editor) return

  currentLanguage.value = String(editor.getModel?.()?.getLanguageId?.() || props.language || 'plaintext')
  refreshModelInfo()
  refreshCursor()

  disposers = [
    editor.onDidChangeCursorPosition((event) => {
      cursor.value = event.position
    }),
    editor.onDidChangeCursorSelection(() => updateSelection(editor)),
    editor.onDidChangeModel(() => {
      refreshModelInfo()
      refreshCursor()
    }),
    editor.onDidChangeModelLanguage?.((event) => {
      currentLanguage.value = String(event?.newLanguage || '')
      refreshLspState()
    }),
    monaco.editor.onDidChangeMarkers(() => refreshMarkers()),
  ].filter(Boolean)
}

const unbindEditor = () => {
  for (const disposable of disposers) {
    try {
      disposable?.dispose?.()
    } catch {
      // ignore
    }
  }
  disposers = []
}

const toggleLineEnding = () => {
  const model = props.editorInstance?.getModel?.()
  if (!model) return
  const next = model.getEOL?.() === '\r\n' ? monaco.editor.EndOfLineSequence.LF : monaco.editor.EndOfLineSequence.CRLF
  model.pushEOL(next)
  refreshModelInfo()
}

const toggleIndentation = () => {
  const model = props.editorInstance?.getModel?.()
  if (!model) return
  const options = model.getOptions?.() || {}
  const tabSize = Number(options.tabSize || 2)
  const insertSpaces = !!options.insertSpaces

  // 与主编辑器一致的循环：2 空格 → 4 空格 → Tab → 2 空格
  let next
  if (!insertSpaces) next = { insertSpaces: true, tabSize: 2 }
  else if (tabSize === 2) next = { insertSpaces: true, tabSize: 4 }
  else next = { insertSpaces: false, tabSize: 4 }

  model.updateOptions(next)
  refreshModelInfo()
}

let disposeLspListener = null

onMounted(() => {
  disposeLspListener = onLspConnectionStateChange((payload) => {
    if (payload.languageId === currentLanguage.value) {
      lspState.value = payload
    }
  })
  bindEditor()
})

watch(() => props.editorInstance, () => bindEditor())
watch(() => props.language, () => {
  if (!props.editorInstance?.getModel?.()) {
    currentLanguage.value = String(props.language || 'plaintext')
    refreshLspState()
  }
})

onBeforeUnmount(() => {
  unbindEditor()
  try {
    disposeLspListener?.()
  } catch {
    // ignore
  }
  disposeLspListener = null
})
</script>

<style scoped>
.aux-status-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 22px;
  padding: 0 8px;
  flex: 0 0 auto;
  background: var(--guaxi-panel-bg);
  border-top: 1px solid var(--guaxi-border);
  color: var(--guaxi-text-primary);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
  font-size: 12px;
  user-select: none;
}

.aux-status-left,
.aux-status-right {
  display: flex;
  align-items: center;
  height: 100%;
}

.aux-status-item {
  display: flex;
  align-items: center;
  gap: 4px;
  height: 100%;
  padding: 0 6px;
  cursor: default;
  white-space: nowrap;
}

.aux-status-item:hover {
  background: rgba(64, 120, 242, 0.08);
}

.aux-lsp-item {
  gap: 4px;
}

.aux-lsp-item.is-ok {
  color: #2f855a;
}

.aux-lsp-item.is-bad {
  color: #c53030;
}

.has-errors {
  color: #e45649;
}

.has-warnings {
  color: #c18401;
}

:global(:root[data-guaxi-theme='dark']) .aux-status-bar {
  background: var(--guaxi-panel-bg);
  border-top-color: var(--guaxi-border);
}

:global(:root[data-guaxi-theme='dark']) .aux-lsp-item.is-ok {
  color: #98c379;
}

:global(:root[data-guaxi-theme='dark']) .aux-lsp-item.is-bad {
  color: #e06c75;
}

:global(:root[data-guaxi-theme='dark']) .has-errors {
  color: #e06c75;
}

:global(:root[data-guaxi-theme='dark']) .has-warnings {
  color: #e5c07b;
}
</style>
