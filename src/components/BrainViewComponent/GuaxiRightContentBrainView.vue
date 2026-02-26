<template>
  <div class="right-sider-content">
    <n-space vertical :size="12" class="panel">
      <n-card title="节点信息" size="small" class="node-info-card">
        <n-space v-if="selectedGraphNode" vertical>
          <n-tag :type="getNodeTagType(selectedGraphNode.type)" size="small">
            {{ typeLabel(selectedGraphNode.type) }}
          </n-tag>
          <n-text v-if="selectedGraphNode.path">路径: {{ selectedGraphNode.path }}</n-text>
          <n-text v-if="selectedGraphNode.file_path">file_path: {{ selectedGraphNode.file_path }}</n-text>
          <n-text v-if="selectedGraphNode.stable_id">stable_id: {{ selectedGraphNode.stable_id }}</n-text>
          <n-text v-if="selectedGraphNode.start_line">行: {{ selectedGraphNode.start_line }}</n-text>
          <n-text v-if="Array.isArray(selectedGraphNode.tags) && selectedGraphNode.tags.length">标签: {{ selectedGraphNode.tags.join(', ') }}</n-text>
          <n-button-group size="small">
            <n-button @click="focusSelected">聚焦</n-button>
            <n-button @click="highlightSelected">高亮连接</n-button>
            <n-button v-if="canOpenSelected" @click="openSelected">打开</n-button>
            <n-button @click="clearSelected">清除</n-button>
          </n-button-group>
        </n-space>
        <n-text v-else depth="3">点击图谱中的节点查看属性</n-text>
      </n-card>

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
import { createLogger } from '../../utils/logger'
import { computed } from 'vue'
import { storeToRefs } from 'pinia'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useEditorStore } from '../../stores/editorStore'

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
  NText,
  NTag,
  NButton,
  NButtonGroup
} from 'naive-ui'

import { SearchOutline } from '@vicons/ionicons5'

const logger = createLogger('GuaxiRightContentBrainView')
const store = useKnowledgeBaseStore()
const fileTreeStore = useFileTreeStore()
const editorStore = useEditorStore()

const { searchQuery, visibleTypes, selectedTags, graphNodes, tagOptions, selectedGraphNode } = storeToRefs(store)

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

const getNodeTagType = (type) => {
  switch (type) {
    case 'knowledge-base': return 'warning'
    case 'directory': return 'info'
    case 'file': return 'success'
    case 'section': return 'primary'
    case 'paragraph': return 'default'
    default: return 'default'
  }
}

const canOpenSelected = computed(() => {
  const n = selectedGraphNode.value
  if (!n) return false
  const path = n.path || n.file_path
  if (!path) return false
  return ['file', 'section', 'paragraph'].includes(n.type)
})

const focusSelected = () => {
  const n = selectedGraphNode.value
  if (!n) return
  if (n.id != null) store.requestFocus({ mode: 'id', value: n.id })
  else if (n.stable_id) store.requestFocus({ mode: 'stable_id', value: n.stable_id })
  else if (n.path || n.file_path) store.requestFocus({ mode: 'path', value: n.path || n.file_path })
}

const highlightSelected = () => {
  const n = selectedGraphNode.value
  if (!n) return
  store.setSelectedGraphNode({ ...n, __highlight_ts: Date.now() })
}

const openSelected = async () => {
  const n = selectedGraphNode.value
  if (!n) return

  const path = n?.file_path || n?.path
  if (!path) return

  const lineNumber = Number(n?.start_line || 1)

  try {
    await fileTreeStore.openFileInTab(path)
    const content = fileTreeStore.selectedFile?.content || ''

    editorStore.setFile({
      path,
      content,
      highlightInfo: {
        lineNumber,
        matches: [{ start: 0, end: 1000000 }],
        text: n?.name || ''
      }
    })

    window.location.hash = '/'
  } catch (e) {
    logger.error('打开节点失败:', e)
  }
}

const clearSelected = () => {
  store.setSelectedGraphNode(null)
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
  if (!n) return
  if (n.id != null) store.requestFocus({ mode: 'id', value: n.id })
  else if (n.stable_id) store.requestFocus({ mode: 'stable_id', value: n.stable_id })
  else if (n.path) store.requestFocus({ mode: 'path', value: n.path })
}
</script>

<style scoped>
.right-sider-content {
  flex: 1 1 auto;
  height: 100%;
  width: 100%;
  overflow: hidden;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', system-ui;
  font-size: 14px;
}

.panel {
  padding: 8px;
  height: 100%;
  overflow: hidden;
}

.node-info-card :deep(.n-card__content) {
  max-height: 220px;
  overflow: auto;
}

.node-info-card :deep(.n-text) {
  word-break: break-all;
}

.search-results {
  max-height: 380px;
  overflow: auto;
}
</style>