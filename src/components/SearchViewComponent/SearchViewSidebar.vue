<template>
  <div class="sider-container">
    <GuaxiNavRail />
    <div class="flipboard-sidebar">
      <div class="sidebar-header">
        <h3 class="sidebar-title">知识领域</h3>
      </div>

      <n-scrollbar class="sidebar-scroll">
        <div class="category-list">
          <div v-for="category in categories" :key="category.id"
            :class="['category-item', { active: selectedCategory === category.id }]"
            @click="selectCategory(category.id)">
            <div class="category-icon">
              <n-icon size="20" :component="getCategoryIcon(category.id)" />
            </div>
            <span class="category-name">{{ category.name }}</span>
            <div class="category-count" v-if="category.count">
              {{ category.count }}
            </div>
          </div>
        </div>
      </n-scrollbar>

      <div class="sidebar-footer">
        <n-button block type="primary" @click="openFeedManagerWindow" class="add-feed-btn">
          <template #icon>
            <n-icon>
              <Add />
            </n-icon>
          </template>
          管理知识源
        </n-button>
      </div>
    </div>
  </div>
</template>

<script setup>
import GuaxiNavRail from '../GuaxiNavRail.vue'
import { createLogger } from '../../utils/logger'
import { computed } from "vue"
import { invoke } from '@tauri-apps/api/core'
import { useFlipboardStore } from '../../stores/flipboardStore.js'
import { useMessage } from 'naive-ui'
import {
  GlobeOutline as Globe,
  BusinessOutline as Business,
  HeartOutline as Heart,
  CameraOutline as Camera,
  TrendingUpOutline as Technology,
  CashOutline as Cash,
  MedkitOutline as Medkit,
  ScaleOutline as Scale,
  SchoolOutline as School,
  SparklesOutline as Sparkles,
  AddOutline as Add
} from '@vicons/ionicons5'

const logger = createLogger('SearchViewSidebar')
const store = useFlipboardStore()
const message = useMessage()
const categories = computed(() => store.categories.value)
const selectedCategory = computed(() => store.selectedCategory.value)

const getCategoryIcon = (categoryId) => {
  const iconMap = {
    'business': Business,
    'technology': Technology,
    'computer': Technology,
    'math': Business,
    'physics': Globe,
    'lifestyle': Heart,
    'entertainment': Camera,
    'news': Globe,
    'ai': Sparkles,
    'finance': Cash,
    'medicine': Medkit,
    'law': Scale,
    'education': School
  }
  return iconMap[categoryId] || Globe
}

const selectCategory = (categoryId) => {
  const topic = String(store.searchQuery?.value || '').trim()
  if (topic) {
    store.fetchTopicKnowledge(topic, categoryId)
    return
  }
  store.fetchArticles(categoryId)
}

const openFeedManagerWindow = async () => {
  try {
    logger.info('SearchViewSidebar: open feed manager window')
    await invoke('create_flipboard_feed_window', {
      label: 'flipboard_feeds',
      title: '知识源管理'
    })
  } catch (error) {
    logger.error('SearchViewSidebar: open feed manager window failed:', error)
    message.error('打开知识源管理窗口失败')
  }
}
</script>

<style scoped>
.sider-container {
  display: flex;
  width: 100%;
  height: 100%;
  overflow: hidden;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', monospace;
}

.flipboard-sidebar {
  height: 100%;
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  background: var(--guaxi-panel-bg);
  backdrop-filter: blur(10px);
  border-right: 1px solid var(--guaxi-border);
}

.sidebar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 24px 16px;
  border-bottom: 1px solid var(--guaxi-border);
}

.sidebar-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
  margin: 0;
}

.sidebar-scroll {
  flex: 1;
  padding: 16px 0;
}

.category-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 0 16px;
}

.category-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  border-radius: 12px;
  cursor: pointer;
  transition: all 0.2s ease;
  color: var(--guaxi-text-secondary);
  position: relative;
}

.category-item:hover {
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
}

.category-item.active {
  background: linear-gradient(135deg, #667eea, #764ba2);
  color: white;
  box-shadow: 0 4px 12px rgba(102, 126, 234, 0.3);
}

.category-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
}

.category-name {
  flex: 1;
  font-weight: 500;
  font-size: 14px;
}

.category-count {
  background: rgba(255, 255, 255, 0.2);
  padding: 2px 8px;
  border-radius: 10px;
  font-size: 12px;
  font-weight: 600;
}

.category-item:not(.active) .category-count {
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-secondary);
}

.sidebar-footer {
  padding: 20px 24px;
  border-top: 1px solid var(--guaxi-border);
}

.add-feed-btn {
  background: linear-gradient(135deg, #667eea, #764ba2);
  border: none;
  border-radius: 12px;
  height: 40px;
  font-weight: 500;
  transition: all 0.2s ease;
}

.add-feed-btn:hover {
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(102, 126, 234, 0.3);
}
</style>
