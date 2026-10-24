<template>
  <div class="sider-container">
    <GuaxiNavRail />
    <div class="app-category-list">
      <div v-for="category in props.categories" :key="category.key"
        :class="['app-category-item', { active: category.key === selectedCategory }]"
        @click="handleSelect(category.key)">
        <div class="app-category-dot"></div>
        <span class="app-category-name">{{ category.label }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import GuaxiNavRail from '../GuaxiNavRail.vue'
const props = defineProps({
  // 分类由 GuaxiContentAppView 统一维护。此前这里存在第二份硬编码列表，
  // 会导致父组件新增应用分类后侧栏不可见，且分类与卡片过滤状态不一致。
  categories: {
    type: Array,
    default: () => []
  },
  selectedCategory: {
    type: String,
    default: ''
  }
})

const emit = defineEmits(['update:selectedCategory'])

const handleSelect = (key) => {
  emit('update:selectedCategory', key)
}
</script>

<style scoped>
.sider-container {
  display: flex;
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', monospace;
}

.app-category-list {
  flex: 1;
  padding: 16px 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  font-size: 13px;
  background: var(--guaxi-panel-bg);
}

.app-category-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 10px;
  cursor: pointer;
  color: var(--guaxi-text-secondary);
  transition: background 0.2s ease, color 0.2s ease, transform 0.1s ease, box-shadow 0.15s ease;
}

.app-category-item:hover {
  background: var(--guaxi-header-bg);
  transform: translateX(2px);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.06);
}

.app-category-item.active {
  background: linear-gradient(135deg, #667eea, #764ba2);
  color: #fff;
  box-shadow: 0 4px 12px rgba(102, 126, 234, 0.3);
}

.app-category-dot {
  width: 6px;
  height: 6px;
  border-radius: 999px;
  background: currentColor;
}

.app-category-name {
  white-space: nowrap;
}
</style>
