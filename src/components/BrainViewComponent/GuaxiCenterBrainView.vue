<template>
  <div class="text-aera">
    <GuaxiKnowledgeGraph3D :kb-name="kbName" />
  </div>
</template>

<script setup>
import { computed } from 'vue';
import GuaxiKnowledgeGraph3D from './GuaxiKnowledgeGraph3D.vue';
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore';

const store = useKnowledgeBaseStore();
const kbName = computed(() => store.selectedKnowledgeBase?.knowledge_base_name || '');
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
