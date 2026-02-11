<template>
  <div class="guaxi-content">
    <n-layout has-sider>
      <n-layout-sider :width="SIDER_WIDTH" bordered content-style="padding: 0px;">
        <GuaxiExcalidrawSidebar />
      </n-layout-sider>

      <n-layout-content class="main-content-area">
        <GuaxiExcalidrawBoard />
      </n-layout-content>
    </n-layout>
  </div>
</template>

<script setup>
import { onMounted } from 'vue'
import { createLogger } from '../../utils/logger'
import GuaxiExcalidrawSidebar from './GuaxiExcalidrawSidebar.vue'
import GuaxiExcalidrawBoard from './GuaxiExcalidrawBoard.vue'

const logger = createLogger('GuaxiExcalidrawView')

/**
 * Excalidraw 白板布局：
 * - 左侧仅保留图标栏（44px）
 * - 与其它页面的 Sidebar 解耦，避免互相影响
 */
const SIDER_WIDTH = 44

onMounted(() => {
  logger.info('[excalidraw:view] mounted', { siderWidth: SIDER_WIDTH })
})
</script>

<style scoped>
.guaxi-content {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100%;
  overflow: hidden;
}

.n-layout {
  height: 100%;
  overflow: visible;
}

.main-content-area {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.n-layout-sider {
  background: var(--guaxi-panel-bg);
  height: 100%;
  overflow: visible;
  position: relative;
  z-index: 2;
}
</style>