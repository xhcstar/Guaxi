<template>
  <n-drawer :show="show" @update:show="$emit('update:show', $event)" width="400" placement="right">
    <n-drawer-content title="任务列表" closable @close="$emit('update:show', false)">
      <n-text v-if="categorizedProgresses.conversion.length" strong>转换任务</n-text>
      <n-list v-if="categorizedProgresses.conversion.length" hoverable>
        <n-list-item v-for="(progress, index) in categorizedProgresses.conversion" :key="'conv-' + index" class="task-item"
          :class="{ 'task-item-completed': progress.current >= 100 }">
          <div class="task-content">
            <div class="task-header">
              <n-text strong class="task-filename">{{ progress.fileName }}</n-text>
              <n-tag v-if="progress.current < 100" size="small" type="info" class="task-tag">
                {{ progress.type === 'backup' ? '备份中' : (progress.type === 'recovery' ? '恢复中' : (progress.type === 'import' ? '导入中' : (progress.type === 'export' ? '导出中' : '转换中'))) }}
              </n-tag>
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
    </n-drawer-content>
  </n-drawer>
</template>

<script setup>
import { NDrawer, NDrawerContent, NList, NListItem, NText, NTag, NProgress, NIcon } from 'naive-ui'
import { InfoCircleFilled } from '@vicons/antd'

defineProps({
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
      export: []
    })
  }
})

defineEmits(['update:show'])
</script>

<style scoped>
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