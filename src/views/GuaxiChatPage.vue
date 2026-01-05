<template>
  <n-message-provider placement="bottom">
    <div class="app-layout">
      <div class="main-header">
        <GuaxiHeaderChatView />
      </div>
      <div class="main-content">
        <GuaxiContentChatView />
      </div>
    </div>
  </n-message-provider>
</template>

<script setup>
import GuaxiHeaderChatView from "../components/ChatViewComponent/GuaxiHeaderChatView.vue";
import GuaxiContentChatView from "../components/ChatViewComponent/GuaxiContentChatView.vue";
import { NMessageProvider } from 'naive-ui'
</script>

<style scoped>
/* 确保根布局占据整个窗口 */
.app-layout {
  display: flex;
  flex-direction: column;
  height: 100vh; /* 占满整个视口高度 */
  margin: 0; /* 确保没有外部间距 */
  padding: 0; /* 确保没有外部内边距 */
  box-sizing: border-box; /* 包括 padding 和 border 在内计算高度 */
}

/* 头部样式 */
.main-header {
  flex: 0 0 auto; /* 固定高度的 header */
  height: 30px; /* 设置你需要的高度 */
  background-color: var(--guaxi-header-bg);
  box-shadow: var(--guaxi-header-shadow);
}

/* 内容区域样式 */
.main-content {
  flex: 1 1 auto; /* 填充剩余空间 */
  display: flex;
  flex-direction: column;
  background-color: var(--guaxi-content-bg);
  overflow: hidden;
  margin: 0;
  padding: 0;
}
</style>
