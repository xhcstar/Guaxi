<template>
  <div class="flipboard-app">
    <!-- 顶部导航栏 -->
    <FlipboardHeader />

    <!-- 主内容区域 -->
    <div class="flipboard-main">
      <!-- 左侧分类导航 -->
      <div class="sidebar-container" :class="{ collapsed: isLeftSiderCollapsed }"
        :style="{ width: isLeftSiderCollapsed ? '0' : leftSiderbarWidth + 'px' }">

        <SearchViewSidebar />
        <div class="sider-resize-handle" @mousedown="onMouseDownLeftSidebar"></div>
      </div>

      <!-- 中间内容区域 + 右侧阅读区域交给 ReadingCoordinator 统一协调 -->
      <ReadingCoordinator view-key="search" />
    </div>

    <!-- 我的收集：统一笔记库（可搜索 / 插入主页编辑器） -->
    <CollectionsPanel
      :visible="collectionsPanelVisible"
      @close="collectionsPanelVisible = false"
    />
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount, watch } from "vue"
import { useFlipboardStore } from '../stores/flipboardStore.js'
import { useSiderStore } from '../stores/useSiderStore'
import SearchViewSidebar from '../components/SearchViewComponent/SearchViewSidebar.vue'
import FlipboardHeader from "../components/SearchViewComponent/FlipboardHeader.vue"
import ReadingCoordinator from "../components/SearchViewComponent/ReadingCoordinator.vue"
import CollectionsPanel from "../components/SearchViewComponent/CollectionsPanel.vue"

// 使用 store
const store = useFlipboardStore()
const siderStore = useSiderStore()
const viewKey = 'search'

// 布局状态
const leftSiderbarWidth = ref(280)
const rightSiderbarWidth = ref(500)
const isLeftSiderCollapsed = computed({
  get: () => siderStore.isLeftSiderCollapsed[viewKey] ?? false,
  set: (val) => siderStore.setLeftSiderCollapsed(viewKey, val)
})
const isRightSiderCollapsed = computed({
  get: () => siderStore.isRightSiderCollapsed[viewKey] ?? true,
  set: (val) => siderStore.setRightSiderCollapsed(viewKey, val)
})
const isReadingMode = ref(false)
const readerPanelStyle = computed(() => {
  if (isRightSiderCollapsed.value) {
    return { width: '0' }
  }
  if (isReadingMode.value) {
    return { flex: '1 1 auto', width: 'auto' }
  }
  return { width: rightSiderbarWidth.value + 'px' }
})

// 拖拽状态
const leftSidebarresizing = ref(false)
const leftSidebarStartX = ref(0)
const leftSidebarStartWidth = ref(0)
const rightSidebarresizing = ref(false)
const rightSidebarStartX = ref(0)
const rightSidebarStartWidth = ref(0)

// 计算属性
const filteredArticles = computed(() => store.filteredArticles.value)
const currentArticle = computed(() => store.currentArticle.value)
const currentView = computed(() => store.currentView.value)
const isLoading = computed(() => store.isLoading.value)
const collectionsPanelVisible = computed({
  get: () => store.collectionsPanelVisible.value,
  set: (val) => { store.collectionsPanelVisible.value = val }
})


const handleArticleSelect = async (article) => {
  try {
    await store.fetchArticleDetail(article)
    if (isRightSiderCollapsed.value) {
      isRightSiderCollapsed.value = false
    }
    isReadingMode.value = true
  } catch (error) {
    console.error('Failed to load article detail:', error)
  }
}

const handleReaderClose = () => {
  isRightSiderCollapsed.value = true
  isReadingMode.value = false
  store.currentArticle.value = null
}

const handleRelatedArticleSelect = async (article) => {
  try {
    await store.fetchArticleDetail(article)
  } catch (error) {
    console.error('Failed to load related article detail:', error)
  }
}

watch(() => store.currentView.value, () => {
  isReadingMode.value = false
})

// 拖拽调整宽度逻辑
const onMouseDownLeftSidebar = (event) => {
  leftSidebarresizing.value = true
  leftSidebarStartX.value = event.clientX
  leftSidebarStartWidth.value = leftSiderbarWidth.value
  document.addEventListener("mousemove", onMouseMoveLeftSidebar)
  document.addEventListener("mouseup", onMouseUpLeftSidebar)
}

const onMouseMoveLeftSidebar = (event) => {
  if (!leftSidebarresizing.value) return
  const delta = event.clientX - leftSidebarStartX.value
  leftSiderbarWidth.value = Math.max(200, Math.min(400, leftSidebarStartWidth.value + delta))
}

const onMouseUpLeftSidebar = () => {
  leftSidebarresizing.value = false
  document.removeEventListener("mousemove", onMouseMoveLeftSidebar)
  document.removeEventListener("mouseup", onMouseUpLeftSidebar)
}

const onMouseDownRightSidebar = (event) => {
  rightSidebarresizing.value = true
  rightSidebarStartX.value = event.clientX
  rightSidebarStartWidth.value = rightSiderbarWidth.value
  document.addEventListener("mousemove", onMouseMoveRightSidebar)
  document.addEventListener("mouseup", onMouseUpRightSidebar)
}

const onMouseMoveRightSidebar = (event) => {
  if (!rightSidebarresizing.value) return
  const delta = rightSidebarStartX.value - event.clientX
  rightSiderbarWidth.value = Math.max(300, Math.min(800, rightSidebarStartWidth.value + delta))
}

const onMouseUpRightSidebar = () => {
  rightSidebarresizing.value = false
  document.removeEventListener("mousemove", onMouseMoveRightSidebar)
  document.removeEventListener("mouseup", onMouseUpRightSidebar)
}

onMounted(() => {
  store.init()
})

onBeforeUnmount(() => {
  document.removeEventListener("mousemove", onMouseMoveLeftSidebar)
  document.removeEventListener("mouseup", onMouseUpLeftSidebar)
  document.removeEventListener("mousemove", onMouseMoveRightSidebar)
  document.removeEventListener("mouseup", onMouseUpRightSidebar)
})
</script>

<style scoped>
.flipboard-app {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--guaxi-app-bg);
}

.flipboard-main {
  flex: 1;
  display: flex;
  overflow: hidden;
  position: relative;
}

.sidebar-container {
  position: relative;
  background: var(--guaxi-panel-bg);
  transition: width 0.3s ease;
  overflow: hidden;
}

.sidebar-container.collapsed {
  width: 0 !important;
}

.content-area {
  flex: 1;
  min-width: 0;
  background: var(--guaxi-content-bg);
}

.reader-panel {
  position: relative;
  background: var(--guaxi-panel-bg);
  transition: width 0.3s ease;
  overflow: hidden;
  box-shadow: -2px 0 20px rgba(0, 0, 0, 0.1);
}

.reader-panel.collapsed {
  width: 0 !important;
}

.sider-resize-handle {
  position: absolute;
  top: 0;
  right: 0;
  width: 3px;
  height: 100%;
  cursor: ew-resize;
  background: rgba(0, 0, 0, 0.1);
  z-index: 10;
  transition: background 0.2s ease;
}

.sider-resize-handle:hover {
  background: rgba(0, 0, 0, 0.3);
}

.sider-resize-handle-right {
  position: absolute;
  top: 0;
  left: 0;
  width: 3px;
  height: 100%;
  cursor: ew-resize;
  background: rgba(0, 0, 0, 0.1);
  z-index: 10;
  transition: background 0.2s ease;
}

.sider-resize-handle-right:hover {
  background: rgba(0, 0, 0, 0.3);
}
</style>
