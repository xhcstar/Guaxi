<template>
  <n-drawer :show="show" @update:show="$emit('update:show', $event)" width="400" placement="right">
    <n-drawer-content title="任务列表" closable @close="$emit('update:show', false)">
      <div class="task-actions" v-if="hasFailedTasks">
        <n-space justify="end">
          <n-button size="small" type="error" secondary @click="handleClearFailed">清除失败 ({{ failedTaskCount }})</n-button>
        </n-space>
      </div>
      <n-text v-if="categorizedProgresses.conversion.length" strong>转换任务</n-text>
      <n-list v-if="categorizedProgresses.conversion.length" hoverable>
        <n-list-item v-for="(progress, index) in categorizedProgresses.conversion" :key="'conv-' + index" class="task-item"
          :class="{ 'task-item-completed': progress.current >= 100, 'task-item-failed': isFailed(progress) }">
          <div class="task-content">
            <div class="task-header">
              <n-text strong class="task-filename">{{ progress.fileName }}</n-text>
              <n-tag v-if="isFailed(progress)" size="small" type="error" class="task-tag">失败</n-tag>
              <n-tag v-else-if="progress.current < 100" size="small" type="info" class="task-tag">
                {{ progress.type === 'backup' ? '备份中' : (progress.type === 'recovery' ? '恢复中' : (progress.type === 'import' ? '导入中' : (progress.type === 'export' ? '导出中' : '转换中'))) }}
              </n-tag>
              <n-tag v-else size="small" type="success" class="task-tag">已完成</n-tag>
            </div>
            <n-progress type="line" :percentage="progress.current" :indicator-placement="'inside'"
              :status="isFailed(progress) ? 'error' : (progress.current >= 100 ? 'success' : 'info')" :height="15" class="task-progress" />
            <n-text depth="3" class="task-message">
              <n-icon size="14" class="task-message-icon"><InfoCircleFilled /></n-icon>
              {{ progress.message }}
            </n-text>
          </div>
        </n-list-item>
      </n-list>

      <n-text v-if="categorizedProgresses.backup.length" strong>备份任务</n-text>
      <n-list v-if="categorizedProgresses.backup.length" hoverable>
        <n-list-item v-for="(progress, index) in categorizedProgresses.backup" :key="'bak-' + index" class="task-item"
          :class="{ 'task-item-completed': progress.current >= 100 }">
          <div class="task-content">
            <div class="task-header">
              <n-text strong class="task-filename">{{ progress.fileName }}</n-text>
              <n-tag v-if="progress.current < 100" size="small" type="info" class="task-tag">备份中</n-tag>
              <n-tag v-else size="small" type="success" class="task-tag">已完成</n-tag>
            </div>
            <n-progress type="line" :percentage="progress.current" :indicator-placement="'inside'"
              :status="progress.current >= 100 ? 'success' : 'info'" :height="15" class="task-progress" />
            <n-text depth="3" class="task-message">
              <n-icon size="14" class="task-message-icon"><InfoCircleFilled /></n-icon>
              {{ progress.message }}
            </n-text>
          </div>
        </n-list-item>
      </n-list>

      <n-text v-if="categorizedProgresses.recovery.length" strong>恢复任务</n-text>
      <n-list v-if="categorizedProgresses.recovery.length" hoverable>
        <n-list-item v-for="(progress, index) in categorizedProgresses.recovery" :key="'rec-' + index" class="task-item"
          :class="{ 'task-item-completed': progress.current >= 100 }">
          <div class="task-content">
            <div class="task-header">
              <n-text strong class="task-filename">{{ progress.fileName }}</n-text>
              <n-tag v-if="progress.current < 100" size="small" type="info" class="task-tag">恢复中</n-tag>
              <n-tag v-else size="small" type="success" class="task-tag">已完成</n-tag>
            </div>
            <n-progress type="line" :percentage="progress.current" :indicator-placement="'inside'"
              :status="progress.current >= 100 ? 'success' : 'info'" :height="15" class="task-progress" />
            <n-text depth="3" class="task-message">
              <n-icon size="14" class="task-message-icon"><InfoCircleFilled /></n-icon>
              {{ progress.message }}
            </n-text>
          </div>
        </n-list-item>
      </n-list>

      <n-text v-if="categorizedProgresses.import.length" strong>导入任务</n-text>
      <n-list v-if="categorizedProgresses.import.length" hoverable>
        <n-list-item v-for="(progress, index) in categorizedProgresses.import" :key="'imp-' + index" class="task-item"
          :class="{ 'task-item-completed': progress.current >= 100 }">
          <div class="task-content">
            <div class="task-header">
              <n-text strong class="task-filename">{{ progress.fileName }}</n-text>
              <n-tag v-if="progress.current < 100" size="small" type="info" class="task-tag">导入中</n-tag>
              <n-tag v-else size="small" type="success" class="task-tag">已完成</n-tag>
            </div>
            <n-progress type="line" :percentage="progress.current" :indicator-placement="'inside'"
              :status="progress.current >= 100 ? 'success' : 'info'" :height="15" class="task-progress" />
            <n-text depth="3" class="task-message">
              <n-icon size="14" class="task-message-icon"><InfoCircleFilled /></n-icon>
              {{ progress.message }}
            </n-text>
          </div>
        </n-list-item>
      </n-list>

      <n-text v-if="categorizedProgresses.export.length" strong>导出任务</n-text>
      <n-list v-if="categorizedProgresses.export.length" hoverable>
        <n-list-item v-for="(progress, index) in categorizedProgresses.export" :key="'exp-' + index" class="task-item"
          :class="{ 'task-item-completed': progress.current >= 100 }">
          <div class="task-content">
            <div class="task-header">
              <n-text strong class="task-filename">{{ progress.fileName }}</n-text>
              <n-tag v-if="progress.current < 100" size="small" type="info" class="task-tag">导出中</n-tag>
              <n-tag v-else size="small" type="success" class="task-tag">已完成</n-tag>
            </div>
            <n-progress type="line" :percentage="progress.current" :indicator-placement="'inside'"
              :status="progress.current >= 100 ? 'success' : 'info'" :height="15" class="task-progress" />
            <n-text depth="3" class="task-message">
              <n-icon size="14" class="task-message-icon"><InfoCircleFilled /></n-icon>
              {{ progress.message }}
            </n-text>
          </div>
        </n-list-item>
      </n-list>

      <n-text v-if="categorizedProgresses.sync.length" strong>同步任务</n-text>
      <n-list v-if="categorizedProgresses.sync.length" hoverable>
        <n-list-item v-for="(progress, index) in categorizedProgresses.sync" :key="'sync-' + index" class="task-item"
          :class="{ 'task-item-completed': progress.current >= 100, 'task-item-failed': isFailed(progress) }">
          <div class="task-content">
            <div class="task-header">
              <n-text strong class="task-filename">{{ progress.fileName }}</n-text>
              <n-tag v-if="progress.cancelled" size="small" type="warning" class="task-tag">已终止</n-tag>
              <n-tag v-else-if="isFailed(progress)" size="small" type="error" class="task-tag">失败</n-tag>
              <n-tag v-else-if="progress.current < 100" size="small" type="info" class="task-tag">同步中</n-tag>
              <n-tag v-else size="small" type="success" class="task-tag">已完成</n-tag>
              <!-- 终止按钮：仅进行中可终止 -->
              <n-tooltip v-if="canCancel(progress)" trigger="hover">
                <template #trigger>
                  <n-button text size="tiny" type="error" class="task-cancel-btn"
                    :disabled="progress.cancelling" @click="handleCancelSync(progress)">
                    <template #icon>
                      <n-icon :size="18"><DismissCircle20Regular /></n-icon>
                    </template>
                  </n-button>
                </template>
                {{ progress.cancelling ? '终止中…' : '终止同步' }}
              </n-tooltip>
            </div>
            <n-progress type="line" :percentage="progress.current" :indicator-placement="'inside'"
              :status="progress.cancelled ? 'warning' : (isFailed(progress) ? 'error' : (progress.current >= 100 ? 'success' : 'info'))" :height="15" class="task-progress" />
            <n-text depth="3" class="task-message">
              <n-icon size="14" class="task-message-icon"><InfoCircleFilled /></n-icon>
              {{ progress.message }}
            </n-text>
          </div>
        </n-list-item>
      </n-list>
    </n-drawer-content>
  </n-drawer>
</template>

<script setup>
import { computed } from 'vue'
import { NDrawer, NDrawerContent, NList, NListItem, NText, NTag, NProgress, NIcon, NButton, NSpace, NTooltip } from 'naive-ui'
import { InfoCircleFilled } from '@vicons/antd'
import { DismissCircle20Regular } from '@vicons/fluent'

const props = defineProps({
  show: {
    type: Boolean,
    default: false
  },
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
      export: [],
      sync: []
    })
  },
  clearFailedTasks: {
    type: Function,
    default: null
  },
  cancelSync: {
    type: Function,
    default: null
  }
})

defineEmits(['update:show'])

const isFailed = (progress) => {
  return Boolean(progress?.failed || progress?.message?.includes('失败'))
}

// 仅进行中（未完成、未失败、未终止）的同步任务可终止。
const canCancel = (progress) => {
  return Boolean(
    progress &&
    progress.current < 100 &&
    !isFailed(progress) &&
    !progress.cancelled
  )
}

const handleCancelSync = (progress) => {
  if (typeof props.cancelSync === 'function') {
    props.cancelSync(progress?.id)
  }
}

const failedTaskCount = computed(() => {
  return (props.progresses || []).filter((p) => p?.type === 'conversion' && isFailed(p)).length
})

const hasFailedTasks = computed(() => failedTaskCount.value > 0)

const handleClearFailed = () => {
  if (typeof props.clearFailedTasks === 'function') {
    props.clearFailedTasks()
  }
}
</script>

<style scoped>
.task-actions {
  padding-bottom: 12px;
}

.task-item {
  padding: 12px;
  margin-bottom: 8px;
  border-radius: 8px;
  background-color: var(--n-color);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
  transition: all 0.3s ease;
}

.task-item:hover {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.task-item-completed {
  opacity: 0.8;
  background-color: var(--n-color-hover);
}

.task-item-failed {
  background-color: var(--n-color-hover);
}

.task-content {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.task-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 8px;
}

.task-filename {
  flex: 1;
  min-width: 0;
  word-break: break-word;
  line-height: 1.5;
}

.task-tag {
  flex-shrink: 0;
}

.task-progress {
  margin: 0;
  padding-top: 8px;
}

.task-message {
  display: flex;
  align-items: center;
  font-size: 13px;
  color: var(--n-text-color);
  gap: 6px;
}

.task-message-icon {
  flex-shrink: 0;
}
</style>