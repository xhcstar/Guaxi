<template>
  <div class="right-sider-content">
    <n-space vertical :size="12" class="panel">
      <n-card title="搜索" size="small">
        <n-input v-model:value="searchQueryModel" placeholder="搜索节点..." clearable>
          <template #prefix>
            <n-icon>
              <SearchOutline />
            </n-icon>
          </template>
        </n-input>
      </n-card>

      <n-card title="类型筛选" size="small">
        <n-checkbox-group v-model:value="visibleTypesModel">
          <n-space vertical>
            <n-checkbox v-for="opt in typeOptions" :key="opt.value" :value="opt.value">
              {{ opt.label }}
            </n-checkbox>
          </n-space>
        </n-checkbox-group>
      </n-card>

      <n-card title="标签筛选" size="small">
        <n-select
          v-model:value="selectedTagsModel"
          :options="tagOptions"
          multiple
          clearable
          placeholder="选择标签..."
        />
      </n-card>

      <n-card title="结果" size="small">
        <n-list v-if="filteredResults.length > 0" class="search-results">
          <n-list-item v-for="n in filteredResults" :key="n.id" @click="focusNode(n)">
            <n-thing :title="n.name" :description="resultDesc(n)" />
          </n-list-item>
        </n-list>
        <n-text v-else depth="3">无匹配结果</n-text>
      </n-card>
    </n-space>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { storeToRefs } from 'pinia'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'

import {
  NCard,
  NInput,
  NIcon,
  NSpace,
  NCheckbox,
  NCheckboxGroup,
  NSelect,
  NList,
  NListItem,
  NThing,
  NText
} from 'naive-ui'

import { SearchOutline } from '@vicons/ionicons5'

const store = useKnowledgeBaseStore()
const { searchQuery, visibleTypes, selectedTags, graphNodes, tagOptions } = storeToRefs(store)

const typeOptions = [
  { label: '知识库', value: 'knowledge-base' },
  { label: '目录', value: 'directory' },
  { label: '文件', value: 'file' },
  { label: '标题', value: 'section' },
  { label: '段落', value: 'paragraph' }
]

const searchQueryModel = computed({
  get: () => searchQuery.value,
  set: (v) => store.setSearchQuery(v)
})

const visibleTypesModel = computed({
  get: () => visibleTypes.value,
  set: (v) => store.setVisibleTypes(v)
})

const selectedTagsModel = computed({
  get: () => selectedTags.value,
  set: (v) => store.setSelectedTags(v)
})

const normalize = (v) => String(v || '').toLowerCase()

const filteredResults = computed(() => {
  const all = Array.isArray(graphNodes.value) ? graphNodes.value : []
  const allowedTypes = new Set(Array.isArray(visibleTypes.value) ? visibleTypes.value : [])
  const selTags = Array.isArray(selectedTags.value) ? selectedTags.value : []
  const q = normalize(searchQuery.value).trim()

  const out = all.filter((n) => {
    if (!n || !allowedTypes.has(n.type)) return false

    if (selTags.length > 0) {
      const ts = Array.isArray(n.tags) ? n.tags : []
      const ok = selTags.some(t => ts.includes(t))
      if (!ok) return false
    }

    if (!q) return true
    const hay = `${n.name || ''} ${n.path || ''} ${n.stable_id || ''} ${(Array.isArray(n.tags) ? n.tags.join(',') : '')}`
    return normalize(hay).includes(q)
  })

  out.sort((a, b) => {
    if (a.type !== b.type) return String(a.type).localeCompare(String(b.type), 'zh')
    return String(a.name || '').localeCompare(String(b.name || ''), 'zh')
  })

  return out.slice(0, 200)
})

const typeLabel = (t) => {
  const found = typeOptions.find(x => x.value === t)
  return found ? found.label : String(t || '')
}

const resultDesc = (n) => {
  const parts = []
  if (n?.type) parts.push(typeLabel(n.type))
  if (n?.path) parts.push(String(n.path))
  const ts = Array.isArray(n?.tags) ? n.tags : []
  if (ts.length) parts.push(ts.join(', '))
  return parts.join(' | ')
}

const focusNode = (n) => {
  if (!n?.id) return
  store.requestFocus({ mode: 'id', value: n.id })
}
</script>

<style scoped>
.right-sider-content {
  flex: 1 1 auto;
  height: 100%;
  width: 100%;
  overflow: auto;
}

.panel {
  padding: 8px;
}

.search-results {
  max-height: 380px;
  overflow: auto;
}
</style>