<!-- SearchPanel.vue - 修改知识库选择器部分 -->
<template>
  <div class="search-container">
    <n-card class="search-card" :bordered="false">
      <!-- 紧凑型搜索栏头部 -->
      <div class="search-header">
        <div class="search-left">
          <h3 class="search-title">搜索</h3>
        </div>
        <div class="search-controls">
          <n-button-group>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button @click="$emit('search')" class="search-btn" type="primary" size="small">
                  <template #icon>
                    <n-icon :component="SearchIcon" :size="16" />
                  </template>
                  搜索
                </n-button>
              </template>
              快速搜索
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button @click="handleToggleCollapse" class="toggle-btn" size="small" secondary>
                  <n-icon :size="16">
                    <svg v-if="collapsed" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
                      <path d="M7 10l5 5 5-5z" />
                    </svg>
                    <svg v-else xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
                      <path d="M7 14l5-5 5 5z" />
                    </svg>
                  </n-icon>
                </n-button>
              </template>
              {{ collapsed ? '展开高级搜索' : '折叠高级搜索' }}
            </n-tooltip>
          </n-button-group>
        </div>
      </div>

      <!-- 关键字搜索输入框 - 始终显示 -->
      <n-input-group class="keyword-search">
        <n-input v-model:value="searchParams.keyword" placeholder="输入关键字搜索..." clearable size="small"
          @keyup.enter="$emit('search')">
          <template #prefix>
            <n-icon :component="SearchIcon" :size="16" />
          </template>
        </n-input>
      </n-input-group>

      <!-- 高级搜索条件 -->
      <transition name="search-expand" @enter="onEnter" @leave="onLeave">
        <div v-if="!collapsed" class="advanced-search" :key="advancedSearchKey">
          <n-space :size="12" wrap>
            <!-- 文件库选择器 -->
            <n-select :key="`library-${libraryOptions.length}-${Date.now()}`" v-model:value="searchParams.library"
              :options="libraryOptions" placeholder="文件库" clearable filterable size="small" style="min-width: 140px"
              @search="$emit('library-search', $event)" @focus="handleLibraryFocus" />

            <!-- 标签选择器 -->
            <n-select :key="`tags-${importedFileTagOptions.length}-${Date.now()}`" v-model:value="searchParams.tags"
              :options="importedFileTagOptions" placeholder="标签" clearable filterable multiple size="small"
              style="min-width: 140px" @search="$emit('tag-search', $event)" @focus="handleTagFocus" />

            <!-- 日期选择器 -->
            <n-date-picker v-model:value="searchParams.dateRange" type="daterange" clearable size="small"
              placeholder="日期范围" style="min-width: 200px" />

            <!-- 知识库选择器 - 添加搜索功能 -->
            <n-select :key="`repo-${knowledgeRepoOptions.length}-${Date.now()}`"
              v-model:value="searchParams.knowledgeRepo" :options="knowledgeRepoOptions" placeholder="知识库" clearable
              filterable size="small" style="min-width: 140px" @search="$emit('knowledge-repo-search', $event)"
              @focus="handleKnowledgeRepoFocus" />

            <!-- 文件类型选择器 -->
            <n-select :key="`filetype-${fileTypeOptions.length}-${Date.now()}`" v-model:value="searchParams.fileType"
              :options="fileTypeOptions" placeholder="文件类型" clearable size="small" style="min-width: 120px" />
          </n-space>
        </div>
      </transition>
    </n-card>
  </div>
</template>

<script setup>
import { ref, h, watch, nextTick } from 'vue'
import { NCard, NSpace, NInputGroup, NInput, NButton, NIcon, NTooltip, NSelect, NDatePicker, NButtonGroup } from 'naive-ui'

const props = defineProps({
  searchParams: {
    type: Object,
    required: true
  },
  collapsed: {
    type: Boolean,
    default: true
  },
  fileTypeOptions: {
    type: Array,
    default: () => []
  },
  knowledgeRepoOptions: {
    type: Array,
    default: () => []
  },
  libraryOptions: {
    type: Array,
    default: () => []
  },
  importedFileTagOptions: {
    type: Array,
    default: () => []
  }
})

const emit = defineEmits([
  'update:search-params',
  'update:collapsed',
  'search',
  'library-search',
  'tag-search',
  'knowledge-repo-search' // 新增：知识库搜索事件
])

// 添加高级搜索的 key 用于强制重新渲染
const advancedSearchKey = ref(0)

// 处理折叠/展开切换
const handleToggleCollapse = () => {
  const newCollapsed = !props.collapsed
  emit('update:collapsed', newCollapsed)

  if (!newCollapsed) {
    nextTick(() => {
      advancedSearchKey.value += 1
    })
  }
}

// 焦点事件处理
const handleLibraryFocus = () => {
  if (props.libraryOptions.length === 0) {
    emit('library-search', '')
  }
}

const handleTagFocus = () => {
  if (props.importedFileTagOptions.length === 0) {
    emit('tag-search', '')
  }
}

const handleKnowledgeRepoFocus = () => {
  if (props.knowledgeRepoOptions.length === 0) {
    emit('knowledge-repo-search', '')
  }
}

// 监听选项数据变化
watch([
  () => props.libraryOptions,
  () => props.knowledgeRepoOptions,
  () => props.fileTypeOptions,
  () => props.importedFileTagOptions
], () => {
  if (!props.collapsed) {
    nextTick(() => {
      advancedSearchKey.value += 1
    })
  }
}, { deep: true })

// 搜索图标
const SearchIcon = {
  render() {
    return h('svg', {
      xmlns: 'http://www.w3.org/2000/svg',
      viewBox: '0 0 24 24',
      fill: 'currentColor'
    }, [
      h('path', {
        d: 'M15.5 14h-.79l-.28-.27a6.5 6.5 0 0 0 1.48-5.34c-.47-2.78-2.79-5-5.59-5.34a6.505 6.505 0 0 0-7.27 7.27c.34 2.8 2.56 5.12 5.34 5.59a6.5 6.5 0 0 0 5.34-1.48l.27.28v.79l4.25 4.25c.41.41 1.08.41 1.49 0 .41-.41.41-1.08 0-1.49L15.5 14zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z'
      })
    ])
  }
}

// 动画钩子
const onEnter = (el) => {
  el.style.maxHeight = '0px'
  el.style.opacity = '0'
  el.offsetHeight
  el.style.maxHeight = el.scrollHeight + 'px'
  el.style.opacity = '1'
}

const onLeave = (el) => {
  el.style.maxHeight = el.scrollHeight + 'px'
  el.style.opacity = '1'
  el.offsetHeight
  el.style.maxHeight = '0px'
  el.style.opacity = '0'
}
</script>

<style scoped>
.search-container {
  padding: 8px 16px 12px;
}

.search-card {
  border-radius: 12px;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.04);
  transition: all 0.3s ease;
}

.search-card:hover {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
}

.search-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
  gap: 12px;
}

.search-left {
  flex: 1;
}

.search-title {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: #333;
  line-height: 1;
}

.search-controls {
  display: flex;
  gap: 6px;
}

.search-btn {
  background-color: #409EFF;
  border-color: #409EFF;
  color: white;
  border-radius: 6px;
  transition: all 0.2s ease;
  padding: 4px 12px;
}

.search-btn:hover {
  background-color: #3385d9;
  border-color: #3385d9;
}

.toggle-btn {
  border-radius: 6px;
  color: #5a5a5a;
  transition: all 0.2s ease;
  padding: 4px 8px;
}

.toggle-btn:hover {
  background-color: #f0f4ff;
  color: #409EFF;
}

.keyword-search {
  margin-bottom: 8px;
  border-radius: 8px;
  overflow: hidden;
}

.advanced-search {
  overflow: hidden;
  transition: max-height 0.3s ease, opacity 0.3s ease;
  padding-top: 8px;
  border-top: 1px solid #f0f0f0;
}

/* 平滑展开动画 */
.search-expand-enter-active,
.search-expand-leave-active {
  transition: max-height 0.3s ease, opacity 0.3s ease;
}

.search-expand-enter-from,
.search-expand-leave-to {
  max-height: 0;
  opacity: 0;
}

:deep(.n-input-group) {
  border-radius: 8px;
  overflow: hidden;
}

:deep(.n-input.n-input--small) {
  height: 32px;
}

:deep(.n-select) {
  border-radius: 6px;
}

:deep(.n-date-picker) {
  border-radius: 6px;
}
</style>