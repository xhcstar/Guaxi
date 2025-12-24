<template>
  <div class="flipboard-app">
    <!-- 顶部导航栏 -->
    <FlipboardHeader />
    
    <!-- 主内容区域 -->
    <div class="flipboard-main">
      <!-- 左侧分类导航 -->
      <div 
        class="sidebar-container"
        :class="{ collapsed: isLeftSiderCollapsed }"
        :style="{ width: isLeftSiderCollapsed ? '0' : leftSiderbarWidth + 'px' }"
      >
        <FlipboardSidebar 
          :categories="categories"
          :active-category="selectedCategory"
          @category-change="handleCategoryChange"
        />
        <div class="sider-resize-handle" @mousedown="onMouseDownLeftSidebar"></div>
      </div>

      <!-- 中间内容区域 -->
      <div class="content-area">
        <FlipboardMagazineView 
          :articles="filteredArticles"
          :current-view="currentView"
          :is-loading="isLoading"
          @article-select="handleArticleSelect"
        />
      </div>

      <!-- 右侧阅读面板 -->
      <div 
        class="reader-panel"
        :class="{ collapsed: isRightSiderCollapsed }"
        :style="{ width: isRightSiderCollapsed ? '0' : rightSiderbarWidth + 'px' }"
      >
        <FlipboardDetailView 
          :current-article="currentArticle"
          :is-loading="isLoading"
        />
        <div class="sider-resize-handle-right" @mousedown="onMouseDownRightSidebar"></div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from "vue"
import { useFlipboardStore } from '../stores/flipboardStore.js'
import FlipboardHeader from "../components/SearchViewComponent/FlipboardHeader.vue"
import FlipboardSidebar from "../components/SearchViewComponent/FlipboardSidebar.vue"
import FlipboardMagazineView from "../components/SearchViewComponent/FlipboardMagazineView.vue"
import FlipboardDetailView from "../components/SearchViewComponent/FlipboardDetailView.vue"

// 使用 store
const store = useFlipboardStore()

// 布局状态
const leftSiderbarWidth = ref(280)
const rightSiderbarWidth = ref(500)
const isLeftSiderCollapsed = ref(false)
const isRightSiderCollapsed = ref(false)

// 拖拽状态
const leftSidebarresizing = ref(false)
const leftSidebarStartX = ref(0)
const leftSidebarStartWidth = ref(0)
const rightSidebarresizing = ref(false)
const rightSidebarStartX = ref(0)
const rightSidebarStartWidth = ref(0)

// 计算属性
const categories = computed(() => store.categories.value)
const selectedCategory = computed(() => store.selectedCategory.value)
const filteredArticles = computed(() => store.filteredArticles.value)
const currentArticle = computed(() => store.currentArticle.value)
const currentView = computed(() => store.currentView.value)
const isLoading = computed(() => store.isLoading.value)

// 事件处理
const handleCategoryChange = (categoryId) => {
  store.fetchArticles(categoryId)
}

const handleArticleSelect = async (article) => {
  try {
    await store.fetchArticleDetail(article.id)
    // 自动展开右侧面板
    if (isRightSiderCollapsed.value) {
      isRightSiderCollapsed.value = false
    }
  } catch (error) {
    console.error('Failed to load article detail:', error)
  }
}

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
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
}

.flipboard-main {
  flex: 1;
  display: flex;
  overflow: hidden;
  position: relative;
}

.sidebar-container {
  position: relative;
  background: rgba(255, 255, 255, 0.95);
  backdrop-filter: blur(10px);
  transition: width 0.3s ease;
  overflow: hidden;
}

.sidebar-container.collapsed {
  width: 0 !important;
}

.content-area {
  flex: 1;
  min-width: 0;
  background: #f8f9fa;
}

.reader-panel {
  position: relative;
  background: rgba(255, 255, 255, 0.98);
  backdrop-filter: blur(10px);
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