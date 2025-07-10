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
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useEditorStore } from '../../stores/editorStore'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { NPopover } from 'naive-ui'

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
  { id: 'python', name: 'Python' },
  { id: 'cpp', name: 'C++' },
  { id: 'rust', name: 'Rust' },
  { id: 'shell', name: 'Shell' },
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
  console.log('Show problems panel')
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
  console.log('Change encoding to:', enc)
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

// 点击外部关闭菜单
const handleClickOutside = () => {
  showLanguageMenu.value = false
  showEncodingMenu.value = false
}

// 添加全局点击监听
onMounted(() => {
  document.addEventListener('click', handleClickOutside)
})

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside)
})
</script>

<style scoped>
/* 保持原有样式不变 */
.status-bar {
  display: flex;
  justify-content: space-between;
  height: 22px;
  background-color: var(--statusBar-background);
  border-top: 1px solid var(--statusBar-border);
  font-size: 12px;
  font-family: var(--editor-font-family);
  color: var(--statusBar-foreground);
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
  background-color: var(--statusBarItem-hoverBackground);
}

.status-bar-text {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 问题计数特殊样式 */
.has-errors {
  color: var(--error-color);
}

.has-warnings {
  color: var(--warning-color);
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
  background-color: var(--menuItem-hoverBackground);
}

/* 主题适配 */
.vs .status-bar {
  --statusBar-background: #007acc;
  --statusBar-border: #ccc;
  --statusBar-foreground: #ffffff;
  --statusBarItem-hoverBackground: rgba(255, 255, 255, 0.1);
  --menuItem-hoverBackground: rgba(255, 255, 255, 0.1);
  --error-color: #ff5555;
  --warning-color: #ffb86c;
}

.vs-dark .status-bar,
.hc-black .status-bar {
  --statusBar-background: #1e1e1e;
  --statusBar-border: #333;
  --statusBar-foreground: #ffffff;
  --statusBarItem-hoverBackground: rgba(255, 255, 255, 0.1);
  --menuItem-hoverBackground: rgba(255, 255, 255, 0.1);
  --error-color: #ff5555;
  --warning-color: #ffb86c;
}
</style>