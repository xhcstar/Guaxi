<template>
  <div class="tree-column">
    <!-- 知识库列表区域 -->
    <GuaxiBrainKnowledgeBaseList />
  </div>
</template>

<script setup>
import GuaxiBrainKnowledgeBaseList from "./GuaxiBrainKnowledgeBaseList.vue";
</script>

<style scope>
.tree-column {
  flex: 1;
  padding: 5px;
  overflow-y: auto;
  height: 100vh;
}
</style>
