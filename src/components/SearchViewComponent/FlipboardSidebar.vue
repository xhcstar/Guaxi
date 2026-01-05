<template>
  <div class="flipboard-sidebar">
    <div class="sidebar-header">
      <h3 class="sidebar-title">分类</h3>
      <n-button text @click="toggleCollapse" class="collapse-btn">
        <n-icon>
          <ChevronLeft v-if="!collapsed" />
          <ChevronRight v-else />
        </n-icon>
      </n-button>
    </div>

    <n-scrollbar class="sidebar-scroll">
      <div class="category-list">
        <div 
          v-for="category in categories" 
          :key="category.id"
          :class="['category-item', { active: activeCategory === category.id }]"
          @click="selectCategory(category.id)"
        >
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
      <n-button block type="primary" @click="addFeed" class="add-feed-btn">
        <template #icon>
          <n-icon><Add /></n-icon>
        </template>
        添加订阅源
      </n-button>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import { 
  GlobeOutline as Globe,
  BusinessOutline as Business,
  Football as Sports,
  HeartOutline as Heart,
  CameraOutline as Camera,
  TrendingUpOutline as Technology,
  ChevronBackOutline as ChevronLeft,
  ChevronForwardOutline as ChevronRight,
  AddOutline as Add
} from '@vicons/ionicons5'

const props = defineProps({
  categories: {
    type: Array,
    default: () => []
  },
  activeCategory: {
    type: String,
    default: null
  }
})

const emit = defineEmits(['category-change'])

const collapsed = ref(false)

const getCategoryIcon = (categoryId) => {
  const iconMap = {
    'news': Globe,
    'business': Business,
    'sports': Sports,
    'lifestyle': Heart,
    'entertainment': Camera,
    'technology': Technology
  }
  return iconMap[categoryId] || Globe
}

const selectCategory = (categoryId) => {
  emit('category-change', categoryId)
}

const toggleCollapse = () => {
  collapsed.value = !collapsed.value
  // 这里可以通知父组件调整布局
}

const addFeed = () => {
  console.log('添加订阅源')
}
</script>

<style scoped>
.flipboard-sidebar {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--guaxi-panel-bg);
  border-right: 1px solid var(--guaxi-border);
}

.sidebar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 24px 16px;
  border-bottom: 1px solid rgba(0, 0, 0, 0.06);
}

.sidebar-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
  margin: 0;
}

.collapse-btn {
  color: var(--guaxi-text-secondary);
  transition: color 0.2s ease;
}

.collapse-btn:hover {
  color: var(--guaxi-text-primary);
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
  border-top: 1px solid rgba(0, 0, 0, 0.06);
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