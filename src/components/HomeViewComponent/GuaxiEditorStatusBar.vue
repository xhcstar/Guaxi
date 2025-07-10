<template>
  <div class="status-bar">
    <!-- 左侧状态项 -->
    <div class="status-bar-left">
      <div class="status-bar-item" v-for="item in leftItems" :key="item.id">
        <span class="status-bar-text">{{ item.text }}</span>
      </div>
    </div>

    <!-- 右侧状态项 -->
    <div class="status-bar-right">
      <div class="status-bar-item" v-for="item in rightItems" :key="item.id">
        <span class="status-bar-text">{{ item.text }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, watch } from 'vue'
import { useEditorStore } from '../../stores/editorStore'

const editorStore = useEditorStore()

// 状态项数据
const leftItems = ref([
  { id: 'language', text: 'JavaScript' },
  { id: 'problems', text: '0 Errors, 0 Warnings' }
])

const rightItems = ref([
  { id: 'encoding', text: 'UTF-8' },
  { id: 'line-ending', text: 'LF' },
  { id: 'cursor-position', text: 'Ln 1, Col 1' },
  { id: 'indentation', text: 'Spaces: 2' }
])

// 监听编辑器状态变化
watch(() => editorStore.cursorPosition, (pos) => {
  updateItem('cursor-position', `Ln ${pos.lineNumber}, Col ${pos.column}`)
})

watch(() => editorStore.language, (lang) => {
  updateItem('language', lang)
})

// 更新状态项
const updateItem = (id, text) => {
  const items = [...leftItems.value, ...rightItems.value]
  const item = items.find(i => i.id === id)
  if (item) item.text = text
}
</script>

<style scoped>
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

/* 主题适配 */
.vs .status-bar {
  --statusBar-background: #007acc;
  --statusBar-border: #ccc;
  --statusBar-foreground: #ffffff;
  --statusBarItem-hoverBackground: rgba(255, 255, 255, 0.1);
}

.vs-dark .status-bar,
.hc-black .status-bar {
  --statusBar-background: #1e1e1e;
  --statusBar-border: #333;
  --statusBar-foreground: #ffffff;
  --statusBarItem-hoverBackground: rgba(255, 255, 255, 0.1);
}
</style>