<template>
  <div class="search-container">
    <n-card class="search-card" :bordered="false">
      <n-space vertical :size="16">
        <n-space align="center" justify="space-between">
          <h3 class="search-title">搜索</h3>
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button @click="$emit('update:collapsed', !collapsed)" class="search-button" secondary>
                <n-icon :size="20">
                  <svg v-if="collapsed" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z" />
                  </svg>
                  <svg v-else xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M19 13H5v-2h14v2z" />
                  </svg>
                </n-icon>
              </n-button>
            </template>
            {{ collapsed ? '展开高级搜索' : '折叠高级搜索' }}
          </n-tooltip>
        </n-space>

        <!-- 默认显示关键字搜索栏 -->
        <n-input-group>
          <n-input v-model:value="searchParams.keyword" placeholder="输入关键字搜索" clearable>
            <template #prefix>
              <n-icon :component="SearchIcon" />
            </template>
          </n-input>
          <n-button secondary @click="$emit('search')" class="search-button">
            搜索
          </n-button>
        </n-input-group>

        <!-- 高级搜索条件 - 点击展开后显示 -->
        <n-space v-show="!collapsed" vertical :size="12">
          <n-space :size="16" wrap>
            <n-select v-model:value="searchParams.library" :options="libraryOptions" placeholder="选择文件库" clearable
              filterable style="min-width: 180px" @search="$emit('library-search', $event)" />

            <n-select v-model:value="searchParams.tags" :options="importedFileTagOptions" placeholder="选择标签" clearable
              filterable multiple style="min-width: 180px" @search="$emit('tag-search', $event)" />

            <n-date-picker v-model:value="searchParams.dateRange" type="daterange" clearable placeholder="选择日期范围"
              style="min-width: 220px" />

            <n-select v-model:value="searchParams.knowledgeRepo" :options="knowledgeRepoOptions" placeholder="选择知识库"
              clearable filterable style="min-width: 180px" />

            <n-select v-model:value="searchParams.fileType" :options="fileTypeOptions" placeholder="选择文件类型" clearable
              style="min-width: 150px" />
          </n-space>
        </n-space>
      </n-space>
    </n-card>
  </div>
</template>

<script setup>
import { ref, h } from 'vue'
import { NCard, NSpace, NInputGroup, NInput, NButton, NIcon, NTooltip, NSelect, NDatePicker } from 'naive-ui'

defineProps({
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

defineEmits(['update:search-params', 'update:collapsed', 'search', 'library-search', 'tag-search'])

// 搜索图标组件
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
</script>

<style scoped>
.search-container {
  padding: 0 16px 16px;
}

.search-card {
  border-radius: 12px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.05);
  transition: all 0.3s ease;
}

.search-card:hover {
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

.search-title {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: #333;
}

.search-button {
  background-color: #409EFF;
  border-color: #409EFF;
  color: white;
  border-radius: 6px;
  transition: all 0.2s ease;
}

.search-button:hover {
  background-color: #3385d9;
  border-color: #3385d9;
}

:deep(.n-input-group) {
  border-radius: 20px;
  overflow: hidden;
}

:deep(.n-input),
:deep(.n-button) {
  border-radius: 16px !important;
}
</style>