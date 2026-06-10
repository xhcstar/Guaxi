<template>
  <div class="tree-column">
    <!-- 知识库列表区域 -->
    <GuaxiBrainKnowledgeBaseList />
    <!-- 工业图谱模式:概览/社区/历史等洞察面板 -->
    <GuaxiKg3dLeftPanel v-if="store.graphViewMode === 'kg3d'" />
  </div>
</template>

<script setup>
import GuaxiBrainKnowledgeBaseList from "./GuaxiBrainKnowledgeBaseList.vue";
import GuaxiKg3dLeftPanel from "./GuaxiKg3dLeftPanel.vue";
import { useKnowledgeBaseStore } from "../../stores/knowledgeBaseStore";

const store = useKnowledgeBaseStore();
</script>

<style scope>
.tree-column {
  flex: 1;
  padding: 5px;
  overflow-y: auto;
  height: 100vh;
}
</style>