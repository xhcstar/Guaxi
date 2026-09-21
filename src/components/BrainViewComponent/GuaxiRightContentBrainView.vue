<template>
  <div ref="rightSiderContentRef" class="right-sider-content">
    <!-- 大脑（认知）模式：右栏不该显示图谱的节点/搜索/筛选面板。
         给一段说明占位，避免误显示图谱内容（issue：大脑标签下右栏出现图谱内容）。 -->
    <div v-if="cog.viewMode === 'cognition'" class="cog-right-placeholder">
      <n-empty size="small" description="大脑模式">
        <template #extra>
          <span class="cog-right-tip">这里在图谱模式下显示节点信息与筛选。大脑模式的操作在左侧边栏与中间收件箱。</span>
        </template>
      </n-empty>
    </div>

    <!-- 工业图谱模式:操作面板 + 概览/社区/历史等洞察面板(从左栏迁移至此) -->
    <div v-else-if="store.graphViewMode === 'kg3d'" class="kg3d-right">
      <GuaxiKg3dOpsPanel />
      <GuaxiKg3dLeftPanel />
    </div>

    <div v-else class="panel">
      <n-card title="节点信息" size="small" class="node-info-card">
        <n-space v-if="selectedGraphNode" vertical>
          <n-tag :type="getNodeTagType(selectedGraphNode.type)" size="small">
            {{ typeLabel(selectedGraphNode.type) }}
          </n-tag>
          <n-text v-if="selectedGraphNode.path">路径: {{ selectedGraphNode.path }}</n-text>
          <n-text v-if="selectedGraphNode.file_path">file_path: {{ selectedGraphNode.file_path }}</n-text>
          <n-text v-if="selectedGraphNode.stable_id">stable_id: {{ selectedGraphNode.stable_id }}</n-text>
          <n-text v-if="selectedGraphNode.start_line">行: {{ selectedGraphNode.start_line }}</n-text>
          <n-text v-if="Array.isArray(selectedGraphNode.tags) && selectedGraphNode.tags.length">
            标签: {{ selectedGraphNode.tags.join(', ') }}
          </n-text>
          <n-button-group size="small">
            <n-button @click="focusSelected">聚焦</n-button>
            <n-button @click="highlightSelected">高亮连接</n-button>
            <n-button v-if="canOpenSelected" @click="openSelected">打开</n-button>
            <n-button @click="clearSelected">清除</n-button>
          </n-button-group>
        </n-space>
        <n-text v-else depth="3">点击图谱中的节点查看详情</n-text>
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

      <n-card ref="resultsCardRef" title="结果" size="small" class="results-card">
        <n-list v-if="filteredResults.length > 0" class="search-results">
          <n-list-item
            v-for="n in filteredResults"
            :key="n.id ?? n.stable_id ?? n.path ?? n.name"
            @click="focusNode(n)"
          >
            <n-thing :title="n.name" :description="resultDesc(n)" />
          </n-list-item>
        </n-list>
        <n-text v-else depth="3" class="results-empty">暂无匹配结果</n-text>
      </n-card>
    </div>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useEditorStore } from '../../stores/editorStore'
import { useCognitionStore } from '../../stores/cognitionStore'

import {
  NButton,
  NButtonGroup,
  NCard,
  NCheckbox,
  NCheckboxGroup,
  NIcon,
  NInput,
  NList,
  NListItem,
  NSelect,
  NSpace,
  NTag,
  NText,
  NThing,
  NEmpty
} from 'naive-ui'

import { SearchOutline } from '@vicons/ionicons5'
import GuaxiKg3dOpsPanel from './GuaxiKg3dOpsPanel.vue'
import GuaxiKg3dLeftPanel from './GuaxiKg3dLeftPanel.vue'

const logger = createLogger('GuaxiRightContentBrainView')
const store = useKnowledgeBaseStore()
const fileTreeStore = useFileTreeStore()
const editorStore = useEditorStore()
const cog = useCognitionStore()
const rightSiderContentRef = ref(null)
const resultsCardRef = ref(null)

let rightSidebarResizeObserver = null
let layoutLogFrameId = 0

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
  set: (value) => store.setSearchQuery(value)
})

const visibleTypesModel = computed({
  get: () => visibleTypes.value,
  set: (value) => store.setVisibleTypes(value)
})

const selectedTagsModel = computed({
  get: () => selectedTags.value,
  set: (value) => store.setSelectedTags(value)
})

const normalize = (value) => String(value || '').toLowerCase()

const filteredResults = computed(() => {
  const all = Array.isArray(graphNodes.value) ? graphNodes.value : []
  const allowedTypes = new Set(Array.isArray(visibleTypes.value) ? visibleTypes.value : [])
  const selectedTagList = Array.isArray(selectedTags.value) ? selectedTags.value : []
  const query = normalize(searchQuery.value).trim()

  const results = all.filter((node) => {
    if (!node || !allowedTypes.has(node.type)) return false

    if (selectedTagList.length > 0) {
      const tags = Array.isArray(node.tags) ? node.tags : []
      const hasSelectedTag = selectedTagList.some((tag) => tags.includes(tag))
      if (!hasSelectedTag) return false
    }

    if (!query) return true

    const haystack = [
      node.name || '',
      node.path || '',
      node.stable_id || '',
      Array.isArray(node.tags) ? node.tags.join(',') : ''
    ].join(' ')

    return normalize(haystack).includes(query)
  })

  results.sort((left, right) => {
    if (left.type !== right.type) {
      return String(left.type).localeCompare(String(right.type), 'zh')
    }
    return String(left.name || '').localeCompare(String(right.name || ''), 'zh')
  })

  return results.slice(0, 200)
})

const typeLabel = (type) => {
  const matched = typeOptions.find((option) => option.value === type)
  return matched ? matched.label : String(type || '')
}

const getNodeTagType = (type) => {
  switch (type) {
    case 'knowledge-base':
      return 'warning'
    case 'directory':
      return 'info'
    case 'file':
      return 'success'
    case 'section':
      return 'primary'
    case 'paragraph':
      return 'default'
    default:
      return 'default'
  }
}

const canOpenSelected = computed(() => {
  const node = selectedGraphNode.value
  if (!node) return false

  const path = node.path || node.file_path
  if (!path) return false

  return ['file', 'section', 'paragraph'].includes(node.type)
})

const requestFocusForNode = (node, reason) => {
  if (!node) return

  logger.info('[right-sidebar] request focus for node', {
    reason,
    nodeId: node.id ?? null,
    stableId: node.stable_id ?? null,
    path: node.path || node.file_path || null,
    type: node.type || null
  })

  if (node.id != null) {
    store.requestFocus({ mode: 'id', value: node.id })
    return
  }

  if (node.stable_id) {
    store.requestFocus({ mode: 'stable_id', value: node.stable_id })
    return
  }

  if (node.path || node.file_path) {
    store.requestFocus({ mode: 'path', value: node.path || node.file_path })
  }
}

const focusSelected = () => {
  requestFocusForNode(selectedGraphNode.value, 'selected-node')
}

const highlightSelected = () => {
  const node = selectedGraphNode.value
  if (!node) return

  logger.info('[right-sidebar] highlight selected node', {
    nodeId: node.id ?? null,
    stableId: node.stable_id ?? null,
    type: node.type || null
  })

  store.setSelectedGraphNode({ ...node, __highlight_ts: Date.now() })
}

const openSelected = async () => {
  const node = selectedGraphNode.value
  if (!node) return

  const path = node.file_path || node.path
  if (!path) return

  const lineNumber = Number(node.start_line || 1)

  logger.info('[right-sidebar] open selected node', {
    path,
    lineNumber,
    type: node.type || null
  })

  try {
    await fileTreeStore.openFileInTab(path)
    const content = fileTreeStore.selectedFile?.content || ''

    editorStore.setFile({
      path,
      content,
      highlightInfo: {
        lineNumber,
        matches: [{ start: 0, end: 1000000 }],
        text: node.name || ''
      }
    })

    window.location.hash = '/'
  } catch (error) {
    logger.error('[right-sidebar] failed to open selected node', error)
  }
}

const clearSelected = () => {
  logger.info('[right-sidebar] clear selected node')
  store.setSelectedGraphNode(null)
}

const resultDesc = (node) => {
  const parts = []
  if (node?.type) parts.push(typeLabel(node.type))
  if (node?.path) parts.push(String(node.path))
  const tags = Array.isArray(node?.tags) ? node.tags : []
  if (tags.length) parts.push(tags.join(', '))
  return parts.join(' | ')
}

const focusNode = (node) => {
  requestFocusForNode(node, 'result-list')
}

const resolveElement = (target) => {
  if (!target) return null
  if (typeof HTMLElement !== 'undefined' && target instanceof HTMLElement) return target
  if (typeof target === 'object' && target.$el && typeof HTMLElement !== 'undefined' && target.$el instanceof HTMLElement) {
    return target.$el
  }
  return null
}

/* 设计说明：
 * 1. 这次右栏问题的根因是高度链路被中途截断，所以这里统一记录根容器、结果卡片和结果列表的可视高度与滚动高度。
 * 2. 当窗口缩放、筛选结果变化或容器自身尺寸变化时，日志能帮助我们快速判断是外层没有伸缩，还是内层列表没有拿到滚动空间。
 */
const logRightSidebarLayout = (reason) => {
  const rootEl = resolveElement(rightSiderContentRef.value)
  const resultsCardEl = resolveElement(resultsCardRef.value)
  const resultsListEl = resultsCardEl?.querySelector('.search-results')

  logger.info('[layout] right sidebar metrics', {
    reason,
    rootHeight: rootEl?.clientHeight ?? null,
    resultsCardHeight: resultsCardEl?.clientHeight ?? null,
    resultsListClientHeight: resultsListEl?.clientHeight ?? null,
    resultsListScrollHeight: resultsListEl?.scrollHeight ?? null,
    resultCount: filteredResults.value.length
  })
}

const scheduleRightSidebarLayoutLog = (reason) => {
  if (typeof window === 'undefined' || typeof window.requestAnimationFrame !== 'function') {
    logRightSidebarLayout(reason)
    return
  }

  if (layoutLogFrameId) {
    window.cancelAnimationFrame(layoutLogFrameId)
  }

  layoutLogFrameId = window.requestAnimationFrame(() => {
    layoutLogFrameId = 0
    logRightSidebarLayout(reason)
  })
}

const handleWindowResize = () => {
  scheduleRightSidebarLayoutLog('window-resize')
}

onMounted(async () => {
  await nextTick()

  const rootEl = resolveElement(rightSiderContentRef.value)
  if (rootEl && typeof ResizeObserver !== 'undefined') {
    rightSidebarResizeObserver = new ResizeObserver(() => {
      scheduleRightSidebarLayoutLog('resize-observer')
    })
    rightSidebarResizeObserver.observe(rootEl)
  }

  window.addEventListener('resize', handleWindowResize)
  scheduleRightSidebarLayoutLog('mounted')
})

watch(
  () => filteredResults.value.length,
  (nextCount, prevCount) => {
    logger.info('[search] filtered result count changed', {
      prevCount,
      nextCount
    })
    scheduleRightSidebarLayoutLog('filtered-results-change')
  },
  { flush: 'post' }
)

onBeforeUnmount(() => {
  window.removeEventListener('resize', handleWindowResize)
  rightSidebarResizeObserver?.disconnect()
  rightSidebarResizeObserver = null

  if (layoutLogFrameId && typeof window !== 'undefined' && typeof window.cancelAnimationFrame === 'function') {
    window.cancelAnimationFrame(layoutLogFrameId)
    layoutLogFrameId = 0
  }
})
</script>

<style scoped>
.right-sider-content {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', system-ui;
  font-size: 14px;
}

/* 工业图谱右栏:操作面板 + 洞察面板纵向堆叠,整体可滚动 */
.kg3d-right {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  gap: 8px;
  padding: 6px;
  box-sizing: border-box;
}

/* 设计说明：
 * 1. 右侧栏改成标准纵向 flex 布局，让前面的筛选卡片维持自然高度，最后的结果卡片吃掉剩余空间。
 * 2. 这样在窗口缩放时，结果区会自动重新分配高度，不再依赖固定的 max-height。
 */
.panel {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 8px;
  box-sizing: border-box;
  overflow: hidden;
}

.panel > * {
  flex: 0 0 auto;
  min-height: 0;
}

.node-info-card :deep(.n-card__content) {
  max-height: min(220px, 28vh);
  overflow: auto;
}

.node-info-card :deep(.n-text) {
  word-break: break-all;
}

.results-card {
  flex: 1 1 auto;
  min-height: 0;
}

.results-card :deep(.n-card__content) {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.search-results {
  flex: 1 1 auto;
  min-height: 0;
  max-height: none;
  overflow: auto;
  padding-bottom: 8px;
}

.results-empty {
  display: block;
}

/* 大脑模式右栏占位：居中说明，不显示图谱面板。 */
.cog-right-placeholder {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px 16px;
}

.cog-right-tip {
  display: block;
  max-width: 260px;
  font-size: 13px;
  line-height: 1.7;
  color: var(--guaxi-text-secondary, #64748b);
}
</style>
