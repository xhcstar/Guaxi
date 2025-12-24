<template>
  <div class="flipboard-header">
    <div class="header-content">
      <!-- 左侧品牌和菜单 -->
      <div class="header-left">
        <n-icon
          size="20"
          class="header-icon"
          :component="PanelLeftContract24Regular"
          @click="toggleLeftSidebar"
        />
        <div class="logo">
          <span class="logo-text">FlipGuaxi</span>
        </div>
      </div>

      <!-- 中间搜索框 -->
      <div class="header-center">
        <n-input round placeholder="搜索文章、主题..." clearable class="search-input" :value="searchQuery"
          @update:value="handleSearch">
          <template #prefix>
            <n-icon :component="SearchIcon" />
          </template>
        </n-input>
      </div>

      <!-- 右侧操作按钮 -->
      <div class="header-right">
        <n-button-group>
          <n-button circle size="small" @click="setViewMode('magazine')"
            :type="currentView === 'magazine' ? 'primary' : 'default'">
            <n-icon>
              <MagazineIcon />
            </n-icon>
          </n-button>
          <n-button circle size="small" @click="setViewMode('grid')"
            :type="currentView === 'grid' ? 'primary' : 'default'">
            <n-icon>
              <GridIcon />
            </n-icon>
          </n-button>
          <n-button circle size="small" @click="setViewMode('list')"
            :type="currentView === 'list' ? 'primary' : 'default'">
            <n-icon>
              <ListIcon />
            </n-icon>
          </n-button>
        </n-button-group>

        <n-button circle size="small" @click="refreshArticles" :loading="isLoading">
          <n-icon>
            <RefreshIcon />
          </n-icon>
        </n-button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { useFlipboardStore } from '../../stores/flipboardStore.js'
import { useSiderStore } from '../../stores/useSiderStore'
import { PanelLeftContract24Regular } from '@vicons/fluent'
import {
  SearchOutline,
  GridOutline,
  ListOutline,
  NewspaperOutline,
  RefreshOutline
} from '@vicons/ionicons5'

const SearchIcon = SearchOutline
const GridIcon = GridOutline
const ListIcon = ListOutline
const MagazineIcon = NewspaperOutline
const RefreshIcon = RefreshOutline

const store = useFlipboardStore()
const siderStore = useSiderStore()
const viewKey = 'search'

const searchQuery = computed(() => store.searchQuery.value)
const currentView = computed(() => store.currentView.value)
const isLoading = computed(() => store.isLoading.value)

const handleSearch = (query) => {
  store.searchQuery.value = query
  if (query.trim()) {
    store.searchArticles(query)
  } else {
    store.fetchArticles(store.selectedCategory.value)
  }
}

const setViewMode = (mode) => {
  store.currentView.value = mode
  localStorage.setItem('flipboard_view_mode', mode)
}

const refreshArticles = () => {
  store.refreshArticles()
}

const toggleLeftSidebar = () => {
  siderStore.toggleLeftSider(viewKey)
}
</script>

<style scoped>
.flipboard-header {
  background: rgba(255, 255, 255, 0.98);
  backdrop-filter: blur(20px);
  border-bottom: 1px solid rgba(0, 0, 0, 0.08);
  height: 64px;
  position: sticky;
  top: 0;
  z-index: 1000;
}

.header-content {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 100%;
  padding: 0 24px;
  max-width: 100%;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.header-icon {
  cursor: pointer;
  transition: all 0.2s ease;
  padding: 2px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.header-icon:hover {
  opacity: 0.8;
  transform: scale(1.05);
  background-color: rgba(0, 0, 0, 0.05);
}

.logo {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 700;
  font-size: 20px;
}

.logo-icon {
  font-size: 24px;
}

.logo-text {
  background: linear-gradient(135deg, #667eea, #764ba2);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}

.header-center {
  flex: 1;
  max-width: 500px;
  margin: 0 40px;
}

.search-input {
  width: 100%;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-shrink: 0;
}

.header-right .n-button {
  border: 1px solid rgba(0, 0, 0, 0.1);
  background: rgba(255, 255, 255, 0.8);
  transition: all 0.2s ease;
}

.header-right .n-button:hover {
  background: rgba(255, 255, 255, 1);
  border-color: rgba(0, 0, 0, 0.2);
}
</style>