<template>
  <div class="guaxi-header">
    <!-- 左侧：Logo 和 Icon -->
    <div class="header-left">
      <GuaxiSiderToggle view-key="error" side="left" />
      <div class="logo-text">Guaxi</div>
    </div>

    <!-- 右侧：功能图标 -->
    <div class="header-right">
      <GuaxiSiderToggle view-key="error" side="right" />
    </div>
  </div>
</template>

<script setup>
import GuaxiSiderToggle from '../GuaxiSiderToggle.vue'
</script>

<style scoped>
.guaxi-header {
  flex: 0 0 auto;
  height: 35px;
  display: flex;
  align-items: center;
  justify-content: space-between;
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
