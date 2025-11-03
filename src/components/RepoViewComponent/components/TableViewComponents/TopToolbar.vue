<!-- TopToolbar.vue -->
<template>
  <div class="top-toolbar">
    <n-space justify="space-between" align="center">
      <n-space class="toolbar-left">
        <!-- 转换任务列表按钮 -->
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button text @click="$emit('toggle-task-list')"
              :type="categorizedProgresses.conversion.length > 0 ? 'primary' : 'default'" class="toolbar-button">
              <template #icon>
                <n-icon :component="TasksApp20Regular" />
              </template>
              <n-badge v-if="categorizedProgresses.conversion.length > 0"
                :value="categorizedProgresses.conversion.length" />
            </n-button>
          </template>
          转换任务列表 ({{ categorizedProgresses.conversion.length }})
        </n-tooltip>

        <!-- 备份任务按钮 -->
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button text @click="$emit('backup-database')"
              :type="categorizedProgresses.backup.length > 0 ? 'primary' : 'default'" class="toolbar-button">
              <template #icon>
                <n-icon :component="SaveSync20Regular" />
              </template>
              <n-badge v-if="categorizedProgresses.backup.length > 0" :value="categorizedProgresses.backup.length" />
            </n-button>
          </template>
          备份资料库 ({{ categorizedProgresses.backup.length }})
        </n-tooltip>

        <!-- 恢复任务按钮 -->
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button text @click="$emit('recover-database')"
              :type="categorizedProgresses.recovery.length > 0 ? 'primary' : 'default'" class="toolbar-button">
              <template #icon>
                <n-icon :component="BoxArrowLeft20Regular" />
              </template>
              <n-badge v-if="categorizedProgresses.recovery.length > 0"
                :value="categorizedProgresses.recovery.length" />
            </n-button>
          </template>
          从备份文件中恢复 ({{ categorizedProgresses.recovery.length }})
        </n-tooltip>

        <!-- 导入任务按钮 -->
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button text @click="$emit('import-items')"
              :type="categorizedProgresses.import.length > 0 ? 'primary' : 'default'" class="toolbar-button">
              <template #icon>
                <n-icon :component="TextBulletListAdd20Filled" />
              </template>
              <n-badge v-if="categorizedProgresses.import.length > 0" :value="categorizedProgresses.import.length" />
            </n-button>
          </template>
          导入指定条目 ({{ categorizedProgresses.import.length }})
        </n-tooltip>

        <!-- 导出任务按钮 -->
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button text @click="$emit('export-items')"
              :type="categorizedProgresses.export.length > 0 ? 'primary' : 'default'" class="toolbar-button">
              <template #icon>
                <n-icon :component="SignOut20Regular" />
              </template>
              <n-badge v-if="categorizedProgresses.export.length > 0" :value="categorizedProgresses.export.length" />
            </n-button>
          </template>
          导出指定条目 ({{ categorizedProgresses.export.length }})
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

// 定义属性
defineProps({
  progresses: {
    type: Array,
    default: () => []
  },
  categorizedProgresses: {
    type: Object,
    default: () => ({
      all: [],
      conversion: [],
      backup: [],
      recovery: [],
      import: [],
      export: []
    })
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
/* 样式保持不变 */
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
  position: relative;
}

.toolbar-button:hover {
  background-color: #f0f4ff;
  color: #409EFF;
}

.toolbar-button[type="primary"] {
  color: #409EFF;
}

/* 徽章样式优化 */
.toolbar-button :deep(.n-badge .n-badge-sup) {
  top: -2px;
  right: -2px;
  background-color: #ff4d4f;
  color: white;
  font-size: 12px;
  min-width: 18px;
  height: 18px;
  line-height: 18px;
}
</style>