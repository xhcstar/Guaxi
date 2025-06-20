<template>
  <div class="app-layout">
    <div class="main-header">
      <GuaxiHeaderBrainView />
    </div>
    <div class="main-content">
      <GuaxiContentBrainView />
    </div>
  </div>
</template>


<script setup>
import GuaxiHeaderBrainView from "../components/BrainViewComponent/GuaxiHeaderBrainView.vue";
import GuaxiContentBrainView from "../components/BrainViewComponent/GuaxiContentBrainView.vue";

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
  background-color: #f5f5f5; /* 示例背景色 */
  box-shadow: 0px 2px 4px rgba(0, 0, 0, 0.1); /* 示例阴影 */
}

/* 内容区域样式 */
.main-content {
  flex: 1 1 auto; /* 填充剩余空间 */
  display: flex;
  flex-direction: column;
  background-color: #ffffff; /* 示例背景色 */
  overflow-y: auto; /* 如果内容超出，允许垂直滚动 */
  margin: 0;
  padding: 0;
}
</style>