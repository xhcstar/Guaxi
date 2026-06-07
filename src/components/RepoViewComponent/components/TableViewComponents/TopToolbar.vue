<template>
  <div class="top-toolbar">
    <n-space justify="space-between" align="center" :size="8">
      <n-space class="toolbar-left" :size="4">
        <!-- 转换任务列表按钮 -->
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button text @click="$emit('toggle-task-list')" size="small"
              :type="categorizedProgresses.all.length > 0 ? 'primary' : 'default'" class="toolbar-button">
              <template #icon>
                <n-icon :component="TasksApp20Regular" :size="18" />
              </template>
              <n-badge v-if="categorizedProgresses.all.length > 0"
                :value="categorizedProgresses.all.length" class="badge" />
            </n-button>
          </template>
          任务列表 ({{ categorizedProgresses.all.length }})
        </n-tooltip>

        <!-- 备份任务按钮 -->
        <n-dropdown trigger="click" :options="backupOptions" @select="(key) => $emit('backup-database', key)">
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button text size="small"
                :type="categorizedProgresses.backup.length > 0 ? 'primary' : 'default'" class="toolbar-button">
                <template #icon>
                  <n-icon :component="SaveSync20Regular" :size="18" />
                </template>

              </n-button>
            </template>
            备份资料库 ({{ categorizedProgresses.backup.length }})
          </n-tooltip>
        </n-dropdown>

        <!-- 恢复任务按钮 -->
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button text @click="$emit('recover-database')" size="small"
              :type="categorizedProgresses.recovery.length > 0 ? 'primary' : 'default'" class="toolbar-button">
              <template #icon>
                <n-icon :component="BoxArrowLeft20Regular" :size="18" />
              </template>

            </n-button>
          </template>
          从备份恢复 ({{ categorizedProgresses.recovery.length }})
        </n-tooltip>

        <!-- P2P 同步按钮 -->
        <n-dropdown trigger="click" :options="syncOptions" @select="(key) => $emit('sync-repo', key)">
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button text size="small" class="toolbar-button">
                <template #icon>
                  <n-icon :component="ArrowSync20Regular" :size="18" />
                </template>
              </n-button>
            </template>
            在线同步
          </n-tooltip>
        </n-dropdown>

        <!-- 导入任务按钮 -->
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button text @click="$emit('import-items')" size="small"
              :type="categorizedProgresses.import.length > 0 ? 'primary' : 'default'" class="toolbar-button">
              <template #icon>
                <n-icon :component="TextBulletListAdd20Filled" :size="18" />
              </template>

            </n-button>
          </template>
          导入条目 ({{ categorizedProgresses.import.length }})
        </n-tooltip>

        <!-- 导出任务按钮 -->
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button text @click="$emit('export-items')" size="small"
              :type="categorizedProgresses.export.length > 0 ? 'primary' : 'default'" class="toolbar-button">
              <template #icon>
                <n-icon :component="SignOut20Regular" :size="18" />
              </template>

            </n-button>
          </template>
          导出条目 ({{ categorizedProgresses.export.length }})
        </n-tooltip>
      </n-space>

      <!-- 右侧空间保留 -->
      <n-space></n-space>
    </n-space>
  </div>
</template>

<script setup>
import { NIcon, NButton, NTooltip, NBadge, NSpace, NDropdown } from 'naive-ui'
import {
  TasksApp20Regular,
  SaveSync20Regular,
  BoxArrowLeft20Regular,
  TextBulletListAdd20Filled,
  SignOut20Regular,
  ArrowSync20Regular
} from '@vicons/fluent'

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
  'export-items',
  'sync-repo'
])

const backupOptions = [
  { label: '备份当前库', key: 'current' },
  { label: '备份所有库', key: 'all' }
]

const syncOptions = [
  { label: '同步文件库根目录（仅文件）', key: 'libraries' },
  { label: '同步当前资料库（文件 + 记录）', key: 'current_repo' }
]
</script>

<style scoped>
.top-toolbar {
  padding: 8px 16px;
  background-color: var(--guaxi-header-bg);
  border-bottom: 1px solid var(--guaxi-border);
  height: auto;
  min-height: 40px;
}

.toolbar-left {
  display: flex;
  align-items: center;
}

.toolbar-button {
  color: var(--guaxi-text-secondary);
  transition: all 0.2s ease;
  border-radius: 6px;
  padding: 4px 6px;
  position: relative;
  height: 32px;
  width: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.toolbar-button:hover {
  background-color: var(--guaxi-panel-bg);
  color: var(--n-color-primary, #409EFF);
}

.toolbar-button[type="primary"] {
  color: #409EFF;
}

.badge {
  position: absolute;
  top: -2px;
  right: -2px;
}

:deep(.n-badge-sup) {
  background-color: #ff4d4f;
  color: white;
  font-size: 11px;
  min-width: 18px;
  height: 18px;
  line-height: 18px;
  display: flex;
  align-items: center;
  justify-content: center;
}
</style>