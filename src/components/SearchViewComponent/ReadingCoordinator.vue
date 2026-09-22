<template>
  <!-- 阅读模式协调器：负责中间卡片区域与右侧阅读面板的联动、占位与显隐 -->
  <div class="reading-coordinator">
    <!-- 中间内容区域：非阅读模式下显示 -->
    <div class="content-area" v-show="!isReadingMode">
      <FlipboardMagazineView :articles="filteredArticles" :current-view="currentView" :is-loading="isLoading"
        @article-select="handleArticleSelect" />
    </div>

    <!-- 右侧阅读面板：仅在选中文章后出现。没有文章时整块不渲染，不再冒出空白面板。 -->
    <div v-if="currentArticle" class="reader-panel" :class="{ collapsed: isRightSiderCollapsed }" :style="readerPanelStyle">
      <FlipboardDetailView :current-article="currentArticle" :is-loading="isLoading" @close="handleReaderClose"
        @article-select="handleRelatedArticleSelect" />
      <div class="sider-resize-handle-right" @mousedown="onMouseDownRightSidebar"></div>
    </div>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, computed, onBeforeUnmount, watch } from 'vue'
import { useFlipboardStore } from '../../stores/flipboardStore.js'
import { useSiderStore } from '../../stores/useSiderStore'
import FlipboardMagazineView from './FlipboardMagazineView.vue'
import FlipboardDetailView from './FlipboardDetailView.vue'

const logger = createLogger('ReadingCoordinator')

/**
 * 允许外部指定 viewKey（与 useSiderStore 的右侧栏折叠状态绑定）
 * 默认与搜索视图一致为 'search'
 */
const props = defineProps({
  viewKey: {
    type: String,
    default: 'search'
  }
})

/** 全局数据与侧栏状态存储 */
const store = useFlipboardStore()
const siderStore = useSiderStore()

/** 阅读模式：true 时隐藏中间内容区域，让右侧阅读面板占满剩余空间 */
const isReadingMode = ref(false)

/** 右侧阅读面板宽度（非阅读模式下的固定宽度）与拖拽状态 */
const rightSiderbarWidth = ref(500)
const rightSidebarresizing = ref(false)
const rightSidebarStartX = ref(0)
const rightSidebarStartWidth = ref(0)

/** 右侧阅读面板折叠状态（与 useSiderStore 同步） */
const isRightSiderCollapsed = computed({
  get: () => siderStore.isRightSiderCollapsed[props.viewKey] ?? true,
  set: (val) => siderStore.setRightSiderCollapsed(props.viewKey, val)
})

/** 右侧面板在不同模式下的宽度/占位策略 */
const readerPanelStyle = computed(() => {
  // 没有选中文章时一律不占位：避免出现一块"选择一篇文章"的空白阅读面板。
  // 之前折叠状态持久化在 store 里，若上次留成展开、而当前无文章，就会冒出这块空白且没有明显控制入口。
  if (!currentArticle.value) {
    return { width: '0' }
  }
  if (isRightSiderCollapsed.value) {
    return { width: '0' }
  }
  if (isReadingMode.value) {
    // 阅读模式：右侧面板占满左侧栏以外的全部空间
    return { flex: '1 1 auto', width: 'auto' }
  }
  // 普通模式：固定宽度的右侧侧栏
  return { width: rightSiderbarWidth.value + 'px' }
})

/** 计算属性映射到全局 store */
const filteredArticles = computed(() => store.filteredArticles.value)
const currentArticle = computed(() => store.currentArticle.value)
const currentView = computed(() => store.currentView.value)
const isLoading = computed(() => store.isLoading.value)

/** 交互：点击卡片进入阅读模式，并展开右侧面板 */
const handleArticleSelect = async (article) => {
  logger.info('ReadingCoordinator: enter reading mode', { articleId: article?.id })
  // 立即展开面板并进入阅读模式（哪怕正文还在后台加载），避免“点击无反应”。
  if (isRightSiderCollapsed.value) {
    isRightSiderCollapsed.value = false
  }
  isReadingMode.value = true
  // currentArticle 由 fetchArticleDetail 内部立即用列表数据回填，正文随后异步替换。
  try {
    await store.fetchArticleDetail(article)
  } catch (error) {
    logger.error('ReadingCoordinator: failed to load article detail', error)
  }
}

/** 交互：阅读面板关闭，折叠右侧面板并退出阅读模式 */
const handleReaderClose = () => {
  logger.info('ReadingCoordinator: close reader')
  isRightSiderCollapsed.value = true
  isReadingMode.value = false
  store.currentArticle.value = null
}

/** 交互：阅读模式内切换相关文章 */
const handleRelatedArticleSelect = async (article) => {
  try {
    logger.info('ReadingCoordinator: select related article', { articleId: article?.id })
    await store.fetchArticleDetail(article)
  } catch (error) {
    logger.error('ReadingCoordinator: failed to load related article detail', error)
  }
}

/** 收集面板请求打开某篇文章：进入阅读模式并加载（类似浏览器收藏夹点击） */
watch(() => store.pendingReaderArticle.value, (article) => {
  if (article) {
    handleArticleSelect(article)
    store.pendingReaderArticle.value = null
  }
})

/** 切换知识领域分类时：退出阅读模式，回到卡片列表页（即使当前正在看正文） */
watch(() => store.selectedCategory.value, () => {
  if (isReadingMode.value) {
    isReadingMode.value = false
    isRightSiderCollapsed.value = true
    store.currentArticle.value = null
  }
})

/** 视图模式切换（网格/杂志/列表）时退出阅读模式，恢复中间内容区域显示 */
watch(() => store.currentView.value, () => {
  logger.info('ReadingCoordinator: view mode changed, exit reading mode')
  isReadingMode.value = false
})

/** 右侧阅读面板宽度拖拽逻辑（仅在普通模式下有效） */
const onMouseDownRightSidebar = (event) => {
  rightSidebarresizing.value = true
  rightSidebarStartX.value = event.clientX
  rightSidebarStartWidth.value = rightSiderbarWidth.value
  document.addEventListener('mousemove', onMouseMoveRightSidebar)
  document.addEventListener('mouseup', onMouseUpRightSidebar)
}

const onMouseMoveRightSidebar = (event) => {
  if (!rightSidebarresizing.value) return
  const delta = rightSidebarStartX.value - event.clientX
  rightSiderbarWidth.value = Math.max(300, Math.min(800, rightSidebarStartWidth.value + delta))
}

const onMouseUpRightSidebar = () => {
  rightSidebarresizing.value = false
  document.removeEventListener('mousemove', onMouseMoveRightSidebar)
  document.removeEventListener('mouseup', onMouseUpRightSidebar)
}

/** 清理事件监听 */
onBeforeUnmount(() => {
  document.removeEventListener('mousemove', onMouseMoveRightSidebar)
  document.removeEventListener('mouseup', onMouseUpRightSidebar)
})
</script>

<style scoped>
/* 根容器：作为左侧栏以右的主区域（中间卡片 + 右侧阅读面板） */
.reading-coordinator {
  flex: 1;
  min-width: 0;
  display: flex;
  overflow: hidden;
}

/* 中间卡片区域（与现有页面样式保持一致） */
.content-area {
  flex: 1;
  min-width: 0;
  background: var(--guaxi-content-bg);
}

/* 右侧阅读面板（与现有页面样式保持一致） */
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

/* 右侧宽度拖拽把手（与现有页面样式保持一致） */
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
