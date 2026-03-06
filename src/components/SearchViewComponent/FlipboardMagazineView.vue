<template>
  <div class="magazine-container">
    <!-- 视图控制栏 -->
    <div class="view-controls-bar">
      <div class="controls-left">
        <n-button-group>
          <n-button 
            size="small"
            :type="currentView === 'grid' ? 'primary' : 'default'"
            @click="setViewMode('grid')"
          >
            <template #icon>
              <n-icon><GridIcon /></n-icon>
            </template>
            网格
          </n-button>
          <n-button 
            size="small"
            :type="currentView === 'magazine' ? 'primary' : 'default'"
            @click="setViewMode('magazine')"
          >
            <template #icon>
              <n-icon><MagazineIcon /></n-icon>
            </template>
            杂志
          </n-button>
          <n-button 
            size="small"
            :type="currentView === 'list' ? 'primary' : 'default'"
            @click="setViewMode('list')"
          >
            <template #icon>
              <n-icon><ListIcon /></n-icon>
            </template>
            列表
          </n-button>
        </n-button-group>
        
        <div class="results-count">
          共 {{ articles.length }} 篇文章
        </div>
      </div>
    </div>

    <!-- 内容区域 -->
    <div class="magazine-content">
      <n-scrollbar>
        <!-- 网格视图 -->
        <div 
          v-if="currentView === 'grid'" 
          class="grid-view"
        >
          <div 
            v-for="article in articles" 
            :key="articleKey(article)"
            class="grid-card"
            @click="selectArticle(article)"
          >
            <ArticleCard 
              :article="article"
              :view-mode="currentView"
            />
          </div>
        </div>

        <!-- 杂志视图 -->
        <div 
          v-else-if="currentView === 'magazine'" 
          class="magazine-view"
        >
          <div 
            v-for="article in articles" 
            :key="articleKey(article)"
            :class="['magazine-card', getCardSize(article)]"
            @click="selectArticle(article)"
          >
            <ArticleCard 
              :article="article"
              :view-mode="currentView"
            />
          </div>
        </div>

        <!-- 列表视图 -->
        <div 
          v-else 
          class="list-view"
        >
          <div 
            v-for="article in articles" 
            :key="articleKey(article)"
            class="list-card"
            @click="selectArticle(article)"
          >
            <ArticleCard 
              :article="article"
              :view-mode="currentView"
            />
          </div>
        </div>

        <!-- 空状态 -->
        <div v-if="articles.length === 0 && !isLoading" class="empty-state">
          <n-empty size="large" description="暂无文章">
            <template #extra>
              <n-button type="primary" @click="refreshArticles">
                刷新获取文章
              </n-button>
            </template>
          </n-empty>
        </div>

        <!-- 加载状态 -->
        <div v-if="isLoading" class="loading-state">
          <n-spin size="large">
            <template #description>
              加载中...
            </template>
          </n-spin>
        </div>
      </n-scrollbar>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { useFlipboardStore } from '../../stores/flipboardStore.js'
import ArticleCard from './ArticleCard.vue'
import { 
  GridOutline,
  ListOutline,
  NewspaperOutline,
  RefreshOutline
} from '@vicons/ionicons5'

const GridIcon = GridOutline
const ListIcon = ListOutline
const MagazineIcon = NewspaperOutline
const RefreshIcon = RefreshOutline

const props = defineProps({
  articles: {
    type: Array,
    default: () => []
  },
  currentView: {
    type: String,
    default: 'magazine'
  },
  isLoading: {
    type: Boolean,
    default: false
  }
})

const emit = defineEmits(['article-select'])

const store = useFlipboardStore()

const selectArticle = (article) => {
  emit('article-select', article)
}

const articleKey = (article) => {
  const url = String(article?.url || '').trim()
  if (url) return url
  return `${String(article?.id || '')}|${String(article?.title || '')}|${String(article?.published_at || '')}`
}

const setViewMode = (mode) => {
  store.currentView.value = mode
  localStorage.setItem('flipboard_view_mode', mode)
}

const getCardSize = (article) => {
  // 根据文章特征决定卡片大小
  if (article.is_featured) return 'large'
  if (article.image_url && article.description?.length > 100) return 'medium'
  return 'small'
}

const refreshArticles = () => {
  store.refreshArticles()
}
</script>

<style scoped>
.magazine-container {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--guaxi-content-bg);
}

.view-controls-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 24px;
  background: var(--guaxi-panel-bg);
  border-bottom: 1px solid var(--guaxi-border);
}

.controls-left {
  display: flex;
  align-items: center;
  gap: 20px;
}

.results-count {
  color: var(--guaxi-text-secondary);
  font-size: 14px;
}


.magazine-content {
  flex: 1;
  overflow: hidden;
}

/* 网格视图 */
.grid-view {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 24px;
  padding: 24px;
  max-width: 1400px;
  margin: 0 auto;
}

.grid-card {
  transition: transform 0.2s ease;
}

.grid-card:hover {
  transform: translateY(-4px);
}

/* 杂志视图 */
.magazine-view {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: 24px;
  padding: 24px;
  max-width: 1600px;
  margin: 0 auto;
}

.magazine-card.large {
  grid-column: span 2;
  grid-row: span 2;
}

.magazine-card.medium {
  grid-column: span 1;
  grid-row: span 1;
}

.magazine-card.small {
  grid-column: span 1;
  grid-row: span 1;
}

/* 列表视图 */
.list-view {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 24px;
  max-width: 1000px;
  margin: 0 auto;
}

.list-card {
  transition: transform 0.2s ease;
}

.list-card:hover {
  transform: translateX(4px);
}

.empty-state, .loading-state {
  display: flex;
  justify-content: center;
  align-items: center;
  height: 400px;
}
</style>
