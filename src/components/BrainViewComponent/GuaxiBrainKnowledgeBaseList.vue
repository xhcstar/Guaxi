<template>
  <div class="knowledge-base-list">
    <div class="list-title">第二大脑</div>
    <n-list bordered>
      <n-list-item v-for="kb in knowledgeBases" :key="kb.id" @click="selectKnowledgeBase(kb)"
        :class="{ active: kb.id === selectedId }">
        <div class="kb-title">
          {{ kb.knowledge_base_name }}
        </div>
        <n-tree v-if="kb.id === selectedId" :data="treeData" :default-expand-all="true" block-line
          style="margin-top: 8px" />
      </n-list-item>
    </n-list>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import { invoke } from '@tauri-apps/api/core'

const store = useKnowledgeBaseStore()
const knowledgeBases = store.knowledgeBases
const selectedId = ref(null)
const treeData = ref([])

const selectKnowledgeBase = async (kb) => {
  selectedId.value = kb.id
  const root = await invoke('get_knowledge_base_tree', { root_path: kb.location })
  treeData.value = [convertToTreeNode(root)]
}

function convertToTreeNode(node) {
  return {
    label: node.name,
    key: node.path,
    isLeaf: !node.is_dir,
    children: node.children ? node.children.map(convertToTreeNode) : undefined
  }
}

onMounted(() => {
  store.fetchKnowledgeBases()
})
</script>

<style scoped>
.knowledge-base-list {
  padding: 8px;
}
.list-title {
  font-weight: bold;
  font-size: 16px;
  margin-bottom: 8px;
}
.n-list-item.active {
  background: #e6f7ff;
}
.kb-title {
  font-weight: 500;
  font-size: 15px;
}
</style>