<template>
  <div class="guaxi-header">
    <!-- 左侧：Logo 和 Icon -->
    <div class="header-left">
      <GuaxiSiderToggle view-key="chat" side="left" />
      <div class="logo-text">Guaxi</div>
    </div>

    <!-- 右侧：功能图标 -->
    <div class="header-right">
      <GuaxiSiderToggle view-key="chat" side="right" />
    </div>
  </div>
</template>

<script setup>
/**
 * 智能助手（Agent 工作台）顶栏。
 *
 * 工作台改成三段式布局后一度没有顶栏，于是左右侧栏的展开 / 收起按钮也跟着消失了——
 * 会话栏和仪表只能一直占着宽度。这里补回与其他页面同一条顶栏：
 * 同一个高度、同一种底色与分隔线，两侧只放统一的 GuaxiSiderToggle。
 */
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
</style>
