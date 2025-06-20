import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export const useKnowledgeBaseStore = defineStore('knowledgeBase', () => {
  const knowledgeBases = ref([])
  const loading = ref(false)

  // 获取知识库列表
  async function fetchKnowledgeBases() {
    loading.value = true
    try {
      knowledgeBases.value = await invoke('get_knowledge_base_list')
    } finally {
      loading.value = false
    }
  }

  // 新建知识库后刷新
  async function refresh() {
    await fetchKnowledgeBases()
  }

  return {
    knowledgeBases,
    loading,
    fetchKnowledgeBases,
    refresh,
  }
})