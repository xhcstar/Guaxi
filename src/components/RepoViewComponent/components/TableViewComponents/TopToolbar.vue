<template>
  <div class="top-toolbar">
    <n-space justify="space-between" align="center">
      <!-- 左侧按钮组 -->
      <n-space class="toolbar-left">
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button text @click="$emit('toggle-task-list')" :type="progresses.length > 0 ? 'primary' : 'default'"
              class="toolbar-button">
              <template #icon>
                <n-icon :component="TasksApp20Regular" />
              </template>
              <n-badge v-if="progresses.length > 0" :value="progresses.length" />
            </n-button>
          </template>
          转换任务列表
        </n-tooltip>

        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button text @click="$emit('backup-database')" :type="progresses.length > 0 ? 'primary' : 'default'"
              class="toolbar-button">
              <template #icon>
                <n-icon :component="SaveSync20Regular" />
              </template>
              <n-badge v-if="progresses.length > 0" :value="progresses.length" />
            </n-button>
          </template>
          备份资料库
        </n-tooltip>

        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button text @click="$emit('recover-database')" :type="progresses.length > 0 ? 'primary' : 'default'"
              class="toolbar-button">
              <template #icon>
                <n-icon :component="BoxArrowLeft20Regular" />
              </template>
              <n-badge v-if="progresses.length > 0" :value="progresses.length" />
            </n-button>
          </template>
          从备份文件中恢复
        </n-tooltip>

        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button text @click="$emit('import-items')" :type="progresses.length > 0 ? 'primary' : 'default'"
              class="toolbar-button">
              <template #icon>
                <n-icon :component="TextBulletListAdd20Filled" />
              </template>
              <n-badge v-if="progresses.length > 0" :value="progresses.length" />
            </n-button>
          </template>
          导入指定条目
        </n-tooltip>

        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button text @click="$emit('export-items')" :type="progresses.length > 0 ? 'primary' : 'default'"
              class="toolbar-button">
              <template #icon>
                <n-icon :component="SignOut20Regular" />
              </template>
              <n-badge v-if="progresses.length > 0" :value="progresses.length" />
            </n-button>
          </template>
          导出指定条目
        </n-tooltip>
      </n-space>

      <!-- 右侧按钮组 -->
      <n-space>
        <!-- 可以添加其他右侧按钮 -->
      </n-space>
    </n-space>
  </div>
</template>

<script setup>
import { NIcon, NButton, NTooltip, NBadge, NSpace } from 'naive-ui'
import {
  TasksApp20Regular,
  SaveSync20Regular,
  BoxArrowLeft20Regular,
  TextBulletListAdd20Filled,
  SignOut20Regular
} from '@vicons/fluent'

defineProps({
  progresses: {
    type: Array,
    default: () => []
  }
})

defineEmits([
  'toggle-task-list',
  'backup-database',
  'recover-database',
  'import-items',
  'export-items'
])
</script>

<style scoped>
.top-toolbar {
  padding: 12px 16px;
  background-color: #fafafa;
  border-bottom: 1px solid #e8e8e8;
}

.toolbar-left {
  padding-left: 8px;
}

.toolbar-button {
  color: #5a5a5a;
  transition: all 0.2s ease;
  border-radius: 6px;
  padding: 4px 8px;
}

.toolbar-button:hover {
  background-color: #f0f4ff;
  color: #409EFF;
}

.toolbar-button[type="primary"] {
  color: #409EFF;
}
</style>