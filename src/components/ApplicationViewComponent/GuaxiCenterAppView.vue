<template>
  <div class="app-center">
    <div class="app-grid">
      <n-card
        v-for="app in displayedApps"
        :key="app.id"
        class="app-card"
        hoverable
        @click="handleClick(app)"
      >
        <div class="app-card-header">
          <div class="app-icon" :style="{ background: app.color }">
            {{ app.icon }}
          </div>
          <div class="app-title">
            {{ app.name }}
          </div>
        </div>
        <div class="app-description">
          {{ app.description }}
        </div>
        <div class="app-tags">
          <span class="app-tag">{{ app.categoryLabel }}</span>
        </div>
      </n-card>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'

const props = defineProps({
  apps: {
    type: Array,
    default: () => []
  },
  selectedCategory: {
    type: String,
    default: ''
  }
})

const emit = defineEmits(['open-app'])

const displayedApps = computed(() => {
  if (!props.selectedCategory) {
    return props.apps
  }
  return props.apps.filter(app => app.category === props.selectedCategory)
})

const handleClick = (app) => {
  emit('open-app', app)
}
</script>

<style scoped>
.app-center {
  flex-grow: 1;
  padding: 16px 20px;
  width: 100%;
  box-sizing: border-box;
  /* 中间卡片区自己滚动，不能让溢出冒泡到 n-layout 的横向 flex 行，
     否则纵向滚动会把左侧栏一起带着动。 */
  height: 100%;
  min-height: 0;
  overflow-y: auto;
}

.app-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 18px;
}

.app-card {
  cursor: pointer;
  border-radius: 14px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.04);
  transition: transform 0.15s ease, box-shadow 0.15s ease, border-color 0.15s ease;
}

.app-card:hover {
  transform: translateY(-3px);
  box-shadow: 0 8px 20px rgba(0, 0, 0, 0.08);
  border-color: rgba(102, 126, 234, 0.35);
}

.app-card-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 8px;
}

.app-icon {
  width: 40px;
  height: 40px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 22px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.12);
}

.app-title {
  font-weight: 600;
  font-size: 15px;
}

.app-description {
  font-size: 13px;
  color: var(--guaxi-text-secondary);
  margin-top: 4px;
  min-height: 36px;
}

.app-tags {
  margin-top: 10px;
}

.app-tag {
  display: inline-flex;
  align-items: center;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
  background: var(--guaxi-header-bg);
  border: 1px solid var(--guaxi-border);
  color: var(--guaxi-text-secondary);
}
</style>