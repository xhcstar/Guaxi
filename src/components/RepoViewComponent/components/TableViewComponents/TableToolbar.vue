<template>
  <n-space justify="space-between" align="center" class="toolbar">
    <!-- 左侧按钮组 -->
    <n-space class="actions">
      <n-button @click="$emit('sort')" class="action-button" secondary>
        <template #icon>
          <n-icon :component="SortAscendingIcon" />
        </template>
        按标题排序
      </n-button>
      <n-button @click="$emit('clear-filters')" class="action-button" secondary>
        <template #icon>
          <n-icon :component="FilterOffIcon" />
        </template>
        清除筛选
      </n-button>
    </n-space>

    <!-- 右侧添加按钮 -->
    <n-tooltip trigger="hover">
      <template #trigger>
        <n-button @click="$emit('add-file')" class="add-path-button" secondary>
          <template #icon>
            <n-icon :component="FolderAdd20Regular" />
          </template>
          导入文件
        </n-button>
      </template>
      导入新的文件
    </n-tooltip>
  </n-space>
</template>

<script setup>
import { h } from 'vue'
import { NButton, NIcon, NTooltip, NSpace } from 'naive-ui'
import { FolderAdd20Regular } from '@vicons/fluent'

defineEmits(['sort', 'clear-filters', 'add-file'])

// 排序图标
const SortAscendingIcon = {
  render() {
    return h('svg', {
      xmlns: 'http://www.w3.org/2000/svg',
      viewBox: '0 0 24 24',
      fill: 'currentColor'
    }, [
      h('path', {
        d: 'M19 17h3l-4 4-4-4h3V3h2v14zM9 13H5v-2h4V9H5V7h4V5H5V3H3v12h2v-2h4v-2zm2-2h2v2h-2v-2zm0-4h2v2h-2V7zm0 8h2v2h-2v-2z'
      })
    ])
  }
}

// 清除筛选图标
const FilterOffIcon = {
  render() {
    return h('svg', {
      xmlns: 'http://www.w3.org/2000/svg',
      viewBox: '0 0 24 24',
      fill: 'currentColor'
    }, [
      h('path', {
        d: 'M19.79 5.61C20.3 4.95 19.83 4 19 4H6.83l7.97 7.97 4.99-6.36zM2.81 2.81L1.39 4.22 10 13v6c0 .55.45 1 1 1h2c.55 0 1-.45 1-1v-2.17l5.78 5.78 1.41-1.41L2.81 2.81z'
      })
    ])
  }
}
</script>

<style scoped>
.toolbar {
  width: 100%;
}

.actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.action-button {
  color: #5a5a5a;
  background-color: #f5f5f5;
  border: 1px solid #e0e0e0;
  border-radius: 6px;
  transition: all 0.2s ease;
}

.action-button:hover {
  background-color: #e8f0fe;
  border-color: #c6d8ff;
  color: #409EFF;
}

.add-path-button {
  background-color: #90bfee;
  border-color: #90bfee;
  color: white;
  border-radius: 6px;
  transition: all 0.2s ease;
}

.add-path-button:hover {
  background-color: #7aa8d6;
  border-color: #7aa8d6;
}

@media (max-width: 768px) {
  .actions {
    flex-direction: column;
  }

  .add-path-button {
    margin-left: 0;
  }
}
</style>