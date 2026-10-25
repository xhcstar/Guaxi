<template>
  <div class="guaxi-header">
    <!-- 左侧：Logo 和 Icon -->
    <div class="header-left">
      <GuaxiSiderToggle view-key="app" side="left" />
      <div class="logo-text">Guaxi</div>
    </div>

    <!-- 右侧：功能图标 -->
    <div class="header-right">
      <n-icon class="header-icon" size="20">
        <Icon :icon="listTreeIcon" width="18" height="18" />
      </n-icon>
      <n-icon class="header-icon" size="20">
        <Icon :icon="cameraIcon" width="18" height="18" />
      </n-icon>
      <GuaxiSiderToggle view-key="app" side="right" />
    </div>
  </div>
</template>

<script setup>
import { Icon } from '@iconify/vue'
import listTreeIcon from '@iconify-icons/lucide/list-tree'
import cameraIcon from '@iconify-icons/lucide/camera'
import GuaxiSiderToggle from '../GuaxiSiderToggle.vue'
</script>

<style scoped>
.guaxi-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 35px;
  background-color: var(--guaxi-header-bg);
  box-shadow: var(--guaxi-header-shadow);
  border-bottom: 2px solid var(--guaxi-border);
  padding: 0 8px;
  box-sizing: border-box;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 200px;
}

.logo-text {
  font-weight: bold;
  font-size: 16px;
  white-space: nowrap;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 200px;
  justify-content: flex-end;
}

.header-icon {
  flex: 0 0 auto;
  cursor: pointer;
  padding: 3px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--guaxi-text-secondary);
  transition: color 0.16s ease, background-color 0.16s ease;
}

.header-icon:hover {
  color: var(--guaxi-text-primary);
  background-color: rgba(127, 127, 127, 0.16);
}
</style>
