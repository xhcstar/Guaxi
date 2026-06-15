<template>
  <div class="tree-column">
    <!-- 知识库列表区域 -->
    <GuaxiBrainKnowledgeBaseList />
  </div>
</template>

<script setup>
import GuaxiBrainKnowledgeBaseList from "./GuaxiBrainKnowledgeBaseList.vue";
</script>

<style scoped>
.tree-column {
  flex: 1;
  padding: 0;
  overflow: hidden;
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background-color: var(--guaxi-content-bg);
}
</style>
