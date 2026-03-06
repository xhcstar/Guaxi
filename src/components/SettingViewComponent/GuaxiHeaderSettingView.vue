<template>
  <div class="guaxi-header">
    <!-- 左侧：Logo 和 Icon -->
    <div class="header-left">
      <n-icon size="20" class="header-icon" :component="PanelLeftContract24Regular" @click="toggleLeftSider"></n-icon>
      <div class="logo-text">Guaxi</div>
    </div>

    <!-- 右侧：功能图标 -->
    <div class="header-right">
      <n-icon class="header-icon" size="20" :component="PanelRightContract24Regular" @click="toggleRightSider"></n-icon>
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";
import { useSiderStore } from '../../stores/useSiderStore'
import { useSiderHotkeys } from '../../composables/useSiderHotkeys'
import {
  PanelLeftContract24Regular,
  PanelRightContract24Regular,
} from '@vicons/fluent'

const viewKey = 'setting'
const siderStore = useSiderStore();

const toggleLeftSider = () => {
  siderStore.toggleLeftSider(viewKey)
};

const toggleRightSider = () => {
  siderStore.toggleRightSider(viewKey)
};

useSiderHotkeys({
  toggleLeft: toggleLeftSider,
  toggleRight: toggleRightSider,
})
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
</style>
