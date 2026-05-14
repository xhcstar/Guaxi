<template>
  <div class="status-bar">
    <!-- 左侧状态项 -->
    <div class="status-bar-left">
      <!-- 语言显示 -->
      <n-popover trigger="click" :show="showLanguageMenu" @update:show="val => showLanguageMenu = val">
        <template #trigger>
          <div class="status-bar-item">
            <span class="status-bar-text">{{ languageDisplay }}</span>
          </div>
        </template>
        <div class="language-menu">
          <div class="language-item" v-for="lang in supportedLanguages" :key="lang.id" @click="changeLanguage(lang.id)">
            {{ lang.name }}
          </div>
        </div>
      </n-popover>

      <!-- 问题计数 -->
      <div class="status-bar-item" :class="{ 'has-errors': problems.errors > 0, 'has-warnings': problems.warnings > 0 }"
        @click="handleProblemsClick">
        <span class="status-bar-text">{{ problemsDisplay }}</span>
      </div>

      <n-tooltip trigger="hover">
        <template #trigger>
          <div class="status-bar-item lsp-status-item" :class="lspStatusInfo.status === 'ok' ? 'lsp-status-ok' : 'lsp-status-bad'">
            <n-icon size="14" class="lsp-status-icon">
              <CheckmarkCircleOutline v-if="lspStatusInfo.status === 'ok'" />
              <AlertCircleOutline v-else />
            </n-icon>
            <span class="status-bar-text">{{ lspStatusInfo.label }}</span>
          </div>
        </template>
        <div class="lsp-status-tooltip">{{ lspStatusInfo.detail }}</div>
      </n-tooltip>
    </div>

    <!-- 右侧状态项 -->
    <div class="status-bar-right">
      <!-- 编码信息 -->
      <n-popover trigger="click" :show="showEncodingMenu" @update:show="val => showEncodingMenu = val">
        <template #trigger>
          <div class="status-bar-item">
            <span class="status-bar-text">{{ encoding }}</span>
          </div>
        </template>
        <div class="encoding-menu">
          <div class="encoding-item" v-for="enc in supportedEncodings" :key="enc" @click="changeEncoding(enc)">
            {{ enc }}
          </div>
        </div>
      </n-popover>

      <!-- 行尾符 -->
      <div class="status-bar-item" @click="toggleLineEnding">
        <span class="status-bar-text">{{ lineEnding }}</span>
      </div>

      <!-- 光标位置 -->
      <div class="status-bar-item">
        <span class="status-bar-text">{{ cursorPositionDisplay }}</span>
      </div>

      <!-- 缩进设置 -->
      <div class="status-bar-item" @click="toggleIndentation">
        <span class="status-bar-text">{{ indentation }}</span>
      </div>

      <!-- 选择状态 (仅在选中文本时显示) -->
      <div class="status-bar-item" v-if="selectionStatus">
        <span class="status-bar-text">{{ selectionStatus }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import * as monaco from 'monaco-editor'
import { useEditorStore } from '../../stores/editorStore'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { NPopover, NTooltip, NIcon } from 'naive-ui'
import { CheckmarkCircleOutline, AlertCircleOutline } from '@vicons/ionicons5'
import { getLspConnectionState, onLspConnectionStateChange } from '../../monaco/lspClient'
import { isLspKnownLanguage } from '../../monaco/lspCapabilities'

const logger = createLogger('GuaxiEditorStatusBar')
const editorStore = useEditorStore()
const fileTreeStore = useFileTreeStore()

// 状态控制
const showLanguageMenu = ref(false)
const showEncodingMenu = ref(false)

// 支持的配置项
const supportedLanguages = [
  { id: 'javascript', name: 'JavaScript' },
  { id: 'typescript', name: 'TypeScript' },
  { id: 'html', name: 'HTML' },
  { id: 'css', name: 'CSS' },
  { id: 'json', name: 'JSON' },
  { id: 'python', name: 'Python' },
  { id: 'c', name: 'C' },
  { id: 'cpp', name: 'C++' },
  { id: 'rust', name: 'Rust' },
  { id: 'lua', name: 'Lua' },
  { id: 'shell', name: 'Shell' },
  { id: 'latex', name: 'LaTeX' },
  { id: 'java', name: 'Java' },
  { id: 'mermaid', name: 'Mermaid' },
  { id: 'markdown', name: 'Markdown' },
  { id: 'plaintext', name: 'Plain Text' }
]

const supportedEncodings = [
  'UTF-8',
  'UTF-8 with BOM',
  'UTF-16 LE',
  'UTF-16 BE',
  'GBK',
  'GB18030',
  'Big5',
  'Shift-JIS'
]

const lspState = ref(getLspConnectionState(editorStore.language))

const refreshLspState = () => {
  lspState.value = getLspConnectionState(editorStore.language)
}

let disposeLspStateListener
onMounted(() => {
  disposeLspStateListener = onLspConnectionStateChange((payload) => {
    if (payload.languageId === editorStore.language) {
      lspState.value = payload
    }
  })
  refreshLspState()
})

onUnmounted(() => {
  if (disposeLspStateListener) {
    disposeLspStateListener()
  }
})

watch(
  () => editorStore.language,
  () => {
    refreshLspState()
  }
)

// 计算显示内容
const languageDisplay = computed(() => {
  const lang = supportedLanguages.find(l => l.id === editorStore.language)
  return lang ? lang.name : editorStore.language
})

const cursorPositionDisplay = computed(() => {
  const { lineNumber, column } = editorStore.cursorPosition;
  return `Ln ${lineNumber}, Col ${column}`;
});

const problemsDisplay = computed(() => {
  const { errors, warnings } = editorStore.problems;
  if (errors === 0 && warnings === 0) return 'No Problems';
  return `${errors} ${errors === 1 ? 'Error' : 'Errors'}, ${warnings} ${warnings === 1 ? 'Warning' : 'Warnings'}`;
});

const encoding = computed(() => editorStore.encoding)
const lineEnding = computed(() => editorStore.lineEnding)
const indentation = computed(() => editorStore.indentation)
const selectionStatus = computed(() => editorStore.selectionStatus)
const problems = computed(() => editorStore.problems)

const lspStatusInfo = computed(() => {
  const languageId = String(editorStore.language || '').trim().toLowerCase()
  if (!languageId) {
    return { status: 'bad', label: 'LSP 异常', detail: '未识别语言' }
  }
  if (!isLspKnownLanguage(languageId)) {
    return { status: 'bad', label: 'LSP 异常', detail: '当前语言未启用 LSP' }
  }
  const state = lspState.value?.state || 'idle'
  if (state === 'connected') {
    return { status: 'ok', label: 'LSP 正常', detail: '前后端通信正常' }
  }
  if (state === 'connecting') {
    return { status: 'bad', label: 'LSP 异常', detail: '正在连接 LSP' }
  }
  if (state === 'disabled') {
    const reason = lspState.value?.reason ? `: ${lspState.value.reason}` : ''
    return { status: 'bad', label: 'LSP 异常', detail: `LSP 已禁用${reason}` }
  }
  const reason = lspState.value?.reason ? `: ${lspState.value.reason}` : ''
  return { status: 'bad', label: 'LSP 异常', detail: `LSP 未连接${reason}` }
})

const lineEndingTooltip = computed(() => {
  return `Line endings: ${editorStore.lineEnding}. Click to toggle between LF and CRLF`
})

const indentationTooltip = computed(() => {
  return `Indentation: ${editorStore.indentation}. Click to toggle between spaces and tabs`
})

// 处理语言点击
const handleLanguageClick = (e) => {
  e.stopPropagation()
  showLanguageMenu.value = true
  showEncodingMenu.value = false
}

const handleLanguageRightClick = (e) => {
  e.preventDefault()
  // 可以在这里添加快捷切换语言的逻辑
}

const changeLanguage = (langId) => {
  if (editorStore.editorInstance) {
    const model = editorStore.editorInstance.getModel()
    if (model) {
      monaco.editor.setModelLanguage(model, langId)
      editorStore.updateLanguage(langId)
    }
  }
  showLanguageMenu.value = false
}

// 处理问题计数点击
const handleProblemsClick = () => {
  // 这里可以跳转到问题面板或显示问题详情
  logger.info('Show problems panel')
}

// 处理编码点击
const handleEncodingClick = (e) => {
  e.stopPropagation()
  showEncodingMenu.value = true
  showLanguageMenu.value = false
}

const handleEncodingRightClick = (e) => {
  e.preventDefault()
  // 可以在这里添加快捷切换编码的逻辑
}

const changeEncoding = (enc) => {
  editorStore.updateEncoding(enc)
  showEncodingMenu.value = false
  // 这里需要实际更改文件编码的实现
  logger.info('Change encoding to:', enc)
}

// 切换行尾符
const toggleLineEnding = () => {
  const newLineEnding = editorStore.lineEnding === 'LF' ? 'CRLF' : 'LF'
  editorStore.updateLineEnding(newLineEnding)

  if (editorStore.editorInstance) {
    const model = editorStore.editorInstance.getModel()
    if (model) {
      const content = model.getValue()
      const newContent = newLineEnding === 'LF'
        ? content.replace(/\r\n/g, '\n')
        : content.replace(/\n/g, '\r\n')

      model.setValue(newContent)
    }
  }
}

// 切换缩进方式
const toggleIndentation = () => {
  const current = editorStore.indentation
  let newIndentation = 'Spaces: 2'

  if (current.includes('Spaces')) {
    newIndentation = 'Tabs'
  } else if (current === 'Tabs') {
    newIndentation = 'Spaces: 4'
  } else {
    const spaceCount = parseInt(current.split(':')[1].trim())
    newIndentation = spaceCount === 2 ? 'Spaces: 4' : 'Tabs'
  }

  editorStore.updateIndentation(newIndentation)

  if (editorStore.editorInstance) {
    const model = editorStore.editorInstance.getModel()
    if (model) {
      const options = model.getOptions()
      if (newIndentation === 'Tabs') {
        options.insertSpaces = false
        options.tabSize = 4
      } else {
        options.insertSpaces = true
        options.tabSize = parseInt(newIndentation.split(':')[1].trim())
      }
      model.updateOptions(options)
    }
  }
}


</script>

<style scoped>
/* 保持原有样式不变 */
.status-bar {
  --guaxi-status-bar-background: var(--guaxi-panel-bg, #ffffff);
  --guaxi-status-bar-border: var(--guaxi-border, #d0d7de);
  --guaxi-status-bar-foreground: var(--guaxi-text-primary, #1f2328);
  --guaxi-status-bar-hover-background: rgba(64, 120, 242, 0.08);
  --guaxi-status-bar-error-color: #e45649;
  --guaxi-status-bar-warning-color: #c18401;
  --guaxi-status-bar-lsp-ok-color: #2f855a;
  --guaxi-status-bar-lsp-bad-color: #c53030;
  display: flex;
  justify-content: space-between;
  height: 22px;
  background-color: var(--guaxi-status-bar-background);
  border-top: 1px solid var(--guaxi-status-bar-border);
  font-size: 12px;
  font-family: var(--editor-font-family);
  color: var(--guaxi-status-bar-foreground);
  padding: 0 8px;
  user-select: none;
}

.status-bar-left,
.status-bar-right {
  display: flex;
  align-items: center;
  height: 100%;
}

.status-bar-item {
  display: flex;
  align-items: center;
  height: 100%;
  padding: 0 6px;
  cursor: pointer;
}

.status-bar-item:hover {
  background-color: var(--guaxi-status-bar-hover-background);
}

.status-bar-text {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.lsp-status-item {
  gap: 4px;
}

.lsp-status-icon {
  display: flex;
  align-items: center;
}

.lsp-status-ok {
  color: var(--guaxi-status-bar-lsp-ok-color);
}

.lsp-status-bad {
  color: var(--guaxi-status-bar-lsp-bad-color);
}

.lsp-status-tooltip {
  max-width: 220px;
}

/* 问题计数特殊样式 */
.has-errors {
  color: var(--guaxi-status-bar-error-color);
}

.has-warnings {
  color: var(--guaxi-status-bar-warning-color);
}

/* 菜单样式 */
.language-menu,
.encoding-menu {
  padding: 4px 0;
  min-width: 120px;
}

.language-item,
.encoding-item {
  padding: 4px 12px;
  cursor: pointer;
}

.language-item:hover,
.encoding-item:hover {
  background-color: var(--guaxi-status-bar-hover-background);
}

/* 主题适配 */
/* 设计说明：
   1. monaco-vscode-api 会注入 VSCode 兼容变量，例如 --statusBar-background，默认亮色主题是蓝色。
   2. Guaxi 的状态栏不是 VSCode workbench status bar，不能再直接读取这些全局变量，否则切换主题时会被覆盖。
   3. 这里使用 Guaxi 自己的变量，并通过根节点 data-guaxi-theme 同步亮/暗色，避免 scoped CSS 依赖 Monaco 的 .vs/.vs-dark 类。 */
:global(:root[data-guaxi-theme='dark']) .status-bar {
  --guaxi-status-bar-background: var(--guaxi-panel-bg, #21252b);
  --guaxi-status-bar-border: var(--guaxi-border, #3e4451);
  --guaxi-status-bar-foreground: var(--guaxi-text-primary, #abb2bf);
  --guaxi-status-bar-hover-background: rgba(255, 255, 255, 0.08);
  --guaxi-status-bar-error-color: #e06c75;
  --guaxi-status-bar-warning-color: #e5c07b;
  --guaxi-status-bar-lsp-ok-color: #98c379;
  --guaxi-status-bar-lsp-bad-color: #e06c75;
}
</style>
