<template>
  <div class="app-layout">
    <n-config-provider>
      <n-message-provider>
        <div class="main-header">
          <GuaxiHeaderBrainView />
        </div>
        <div class="main-content">
          <GuaxiContentBrainView />
          <CreateKnowledgeBaseForm />
        </div>
      </n-message-provider>
    </n-config-provider>
  </div>
</template>


<script setup>
import GuaxiHeaderBrainView from "../components/BrainViewComponent/GuaxiHeaderBrainView.vue";
import GuaxiContentBrainView from "../components/BrainViewComponent/GuaxiContentBrainView.vue";
import CreateKnowledgeBaseForm from "../components/BrainViewComponent/CreateKnowledgeBaseForm.vue";
</script>

<style scoped>
/* 确保根布局占据整个窗口 */
.app-layout {
  display: flex;
  flex-direction: column;
  height: 100vh;
  /* 占满整个视口高度 */
  margin: 0;
  /* 确保没有外部间距 */
  padding: 0;
  /* 确保没有外部内边距 */
  box-sizing: border-box;
  /* 包括 padding 和 border 在内计算高度 */
  overflow: hidden; /* 禁止外层滚动 */
}

/* 头部样式 */
.main-header {
  flex: 0 0 auto;
  height: 35px;
  background-color: #f5f5f5;
  box-shadow: 0px 2px 4px rgba(0, 0, 0, 0.1);
}

/* 内容区域样式 */
.main-content {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background-color: #ffffff;
  overflow: hidden;
  margin: 0;
  padding: 0;
}
</style>