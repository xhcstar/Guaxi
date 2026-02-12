<template>
  <div class="guaxi-content">
    <n-layout has-sider>
      <n-layout-sider :width="SIDER_WIDTH" bordered content-style="padding: 0px;">
        <GuaxiStatisticsSidebar />
      </n-layout-sider>

      <n-layout-content class="main-content-area">
        <GuaxiStatisticsDashboard />
      </n-layout-content>
    </n-layout>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { onMounted } from "vue";
import GuaxiStatisticsSidebar from "./GuaxiStatisticsSidebar.vue";
import GuaxiStatisticsDashboard from "./GuaxiStatisticsDashboard.vue";

const logger = createLogger('GuaxiStatisticsView')

/**
 * 统计页视图容器：
 * - 左侧仅保留图标栏（44px），不再占用额外空白
 * - 不提供拖拽调宽，避免出现“图标栏 + 大片空白”的布局
 */
const SIDER_WIDTH = 44;

onMounted(() => {
  logger.info("[statistics:view] mounted", { siderWidth: SIDER_WIDTH });
});
</script>

<style scoped>
.guaxi-content {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100%;
  overflow: hidden;
  /* 防止外层滚动 */
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