<template>
  <div class="text-aera">
    <GuaxiKnowledgeBaseGraph />
  </div>
</template>

<script setup>
import GuaxiKnowledgeBaseGraph from './GuaxiKnowledgeBaseGraph.vue';
</script>

<style scoped>
.text-aera {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 2px;
  margin-top: 2px;
}

:deep(.knowledge-graph-container) {
  flex: 1 1 auto;
  min-height: 0;
}
</style>