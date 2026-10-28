<template>
  <n-drawer :show="show" @update:show="$emit('update:show', $event)" width="400" placement="right">
    <n-drawer-content title="任务列表" closable @close="$emit('update:show', false)">
      <!-- 转换任务：读取后端持久化队列（conversion_tasks），重启后仍可见/重试 -->
      <div v-if="activeConversionTasks.length" class="section">
        <div class="section-head">
          <div class="section-title">
            <n-icon :size="16" class="section-title-icon"><DocumentSync20Regular /></n-icon>
            <span>转换任务</span>
            <span class="section-count">{{ activeConversionTasks.length }}</span>
          </div>
          <n-tooltip v-if="hasFinishedTasks" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" class="section-clear" @click="handleClearFinished">
                <template #icon><n-icon :size="16"><Broom20Regular /></n-icon></template>
              </n-button>
            </template>
            清除已结束（成功/失败/取消）
          </n-tooltip>
        </div>

        <div class="ctask" v-for="task in activeConversionTasks" :key="'ctask-' + task.id"
          :class="{ 'is-failed': isTaskFailed(task), 'is-canceled': task.status === 'canceled' }">
          <div class="ctask-top">
            <span class="ctask-engine-dot" :class="'eng-' + (task.resolvedEngine || task.requestedEngine || 'auto')"></span>
            <span class="ctask-name" :title="task.fileName">{{ task.fileName }}</span>
            <n-tag size="small" :type="taskTagType(task)" round class="ctask-status">{{ taskStatusLabel(task) }}</n-tag>
            <div class="ctask-ops">
              <n-tooltip v-if="canRetryTask(task)" trigger="hover">
                <template #trigger>
                  <n-button text size="tiny" type="primary" class="ctask-op" @click="handleRetry(task)">
                    <template #icon><n-icon :size="16"><ArrowClockwise20Regular /></n-icon></template>
                  </n-button>
                </template>
                重试转换
              </n-tooltip>
              <n-tooltip v-if="canCancelTask(task)" trigger="hover">
                <template #trigger>
                  <n-button text size="tiny" type="warning" class="ctask-op" @click="handleCancel(task)">
                    <template #icon><n-icon :size="16"><Stop20Regular /></n-icon></template>
                  </n-button>
                </template>
                停止转换
              </n-tooltip>
              <n-tooltip v-if="canDeleteTask(task)" trigger="hover">
                <template #trigger>
                  <n-button text size="tiny" type="error" class="ctask-op" @click="handleDelete(task)">
                    <template #icon><n-icon :size="16"><Dismiss20Regular /></n-icon></template>
                  </n-button>
                </template>
                删除记录
              </n-tooltip>
            </div>
          </div>
          <n-progress type="line" :percentage="taskPercent(task)" :indicator-placement="'inside'" :processing="isTaskActive(task)"
            :status="isTaskFailed(task) ? 'error' : (task.status === 'canceled' ? 'warning' : 'info')"
            :height="14" class="ctask-progress" />
          <div class="ctask-msg" :class="{ 'is-error': isTaskFailed(task) }">{{ taskMessage(task) }}</div>
        </div>
      </div>

      <!-- 备份 / 恢复 / 导入 / 导出：统一卡片式布局（无控制按钮，仅展示进度） -->
      <div v-for="grp in simpleGroups" :key="grp.key" v-show="categorizedProgresses[grp.key].length" class="section">
        <div class="section-head">
          <div class="section-title">
            <n-icon :size="16" class="section-title-icon"><component :is="grp.icon" /></n-icon>
            <span>{{ grp.title }}</span>
            <span class="section-count">{{ categorizedProgresses[grp.key].length }}</span>
          </div>
        </div>

        <div class="ctask" v-for="(progress, index) in categorizedProgresses[grp.key]" :key="grp.key + '-' + index"
          :class="{ 'is-failed': isFailed(progress) }">
          <div class="ctask-top">
            <span class="ctask-engine-dot" :class="simpleDotClass(progress)"></span>
            <span class="ctask-name" :title="progress.fileName">{{ progress.fileName }}</span>
            <n-tag size="small" round :type="simpleTagType(progress)" class="ctask-status">
              {{ simpleStatusLabel(progress, grp.activeLabel) }}
            </n-tag>
          </div>
          <n-progress type="line" :percentage="progress.current" :indicator-placement="'inside'"
            :processing="progress.current < 100 && !isFailed(progress)"
            :status="isFailed(progress) ? 'error' : (progress.current >= 100 ? 'success' : 'info')"
            :height="14" class="ctask-progress" />
          <div class="ctask-msg" :class="{ 'is-error': isFailed(progress) }">{{ progress.message }}</div>
        </div>
      </div>

      <!-- 同步任务：与转换任务同款卡片式布局 + 图标控制 -->
      <div v-if="categorizedProgresses.sync.length" class="section">
        <div class="section-head">
          <div class="section-title">
            <n-icon :size="16" class="section-title-icon"><ArrowSync20Regular /></n-icon>
            <span>同步任务</span>
            <span class="section-count">{{ categorizedProgresses.sync.length }}</span>
          </div>
        </div>

        <div class="ctask" v-for="(progress, index) in categorizedProgresses.sync" :key="'sync-' + index"
          :class="{ 'is-failed': isFailed(progress), 'is-canceled': progress.cancelled }">
          <div class="ctask-top">
            <span class="ctask-engine-dot" :class="syncDotClass(progress)"></span>
            <span class="ctask-name" :title="progress.fileName">{{ progress.fileName }}</span>
            <n-tag size="small" round :type="syncTagType(progress)" class="ctask-status">{{ syncStatusLabel(progress) }}</n-tag>
            <div class="ctask-ops">
              <n-tooltip v-if="canCancel(progress)" trigger="hover">
                <template #trigger>
                  <n-button text size="tiny" type="warning" class="ctask-op" :disabled="progress.cancelling"
                    @click="handleCancelSync(progress)">
                    <template #icon><n-icon :size="16"><Stop20Regular /></n-icon></template>
                  </n-button>
                </template>
                {{ progress.cancelling ? '终止中…' : '终止同步' }}
              </n-tooltip>
            </div>
          </div>
          <n-progress type="line" :percentage="progress.current" :indicator-placement="'inside'"
            :processing="progress.current < 100 && !isFailed(progress) && !progress.cancelled"
            :status="progress.cancelled ? 'warning' : (isFailed(progress) ? 'error' : (progress.current >= 100 ? 'success' : 'info'))"
            :height="14" class="ctask-progress" />
          <div class="ctask-msg" :class="{ 'is-error': isFailed(progress) }">{{ progress.message }}</div>
        </div>
      </div>
    </n-drawer-content>
  </n-drawer>
</template>

<script setup>
import { computed, markRaw } from 'vue'
import { NDrawer, NDrawerContent, NList, NListItem, NText, NTag, NProgress, NIcon, NButton, NSpace, NTooltip } from 'naive-ui'
import {
  DocumentSync20Regular,
  ArrowSync20Regular,
  ArrowClockwise20Regular,
  Stop20Regular,
  Dismiss20Regular,
  Broom20Regular,
  SaveSync20Regular,
  BoxArrowLeft20Regular,
  TextBulletListAdd20Filled,
  SignOut20Regular
} from '@vicons/fluent'

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
  },
  // 后端持久化转换任务（跨重启可见/可重试）
  conversionTasks: {
    type: Array,
    default: () => []
  },
  retryTask: {
    type: Function,
    default: null
  },
  cancelTask: {
    type: Function,
    default: null
  },
  deleteTask: {
    type: Function,
    default: null
  },
  clearFinished: {
    type: Function,
    default: null
  }
})

defineEmits(['update:show'])

const isFailed = (progress) => {
  return Boolean(progress?.failed || progress?.message?.includes('失败'))
}

// ── 持久化转换任务：只展示未完成/失败/取消的（已成功的从活动列表隐去）──
const ACTIVE_STATUSES = ['pending', 'probing', 'running', 'succeeded', 'failed', 'canceled']
const activeConversionTasks = computed(() =>
  (props.conversionTasks || [])
    .filter((t) => ACTIVE_STATUSES.includes(t?.status))
    .sort((a, b) => (b?.id || 0) - (a?.id || 0))
)
const hasFinishedTasks = computed(() =>
  (props.conversionTasks || []).some((t) => ['succeeded', 'failed', 'canceled'].includes(t?.status))
)

const isTaskFailed = (task) => task?.status === 'failed'
const isTaskActive = (task) => ['pending', 'probing', 'running'].includes(task?.status)
const canRetryTask = (task) => ['failed', 'canceled'].includes(task?.status)
const canCancelTask = (task) => ['pending', 'probing', 'running'].includes(task?.status)
const canDeleteTask = (task) => ['failed', 'canceled', 'succeeded'].includes(task?.status)

const taskStatusLabel = (task) => {
  switch (task?.status) {
    case 'pending': return '排队中'
    case 'probing': return '分析中'
    case 'running': return '转换中'
    case 'failed': return '失败'
    case 'canceled': return '已取消'
    case 'succeeded': return '已完成'
    default: return task?.status || '未知'
  }
}
const taskTagType = (task) => {
  switch (task?.status) {
    case 'failed': return 'error'
    case 'canceled': return 'warning'
    case 'succeeded': return 'success'
    default: return 'info'
  }
}
const taskPercent = (task) => {
  const total = Number(task?.progressTotal) || 100
  const cur = Number(task?.progressCurrent) || 0
  return Math.max(0, Math.min(100, Math.round((cur / total) * 100)))
}
const taskMessage = (task) => {
  if (task?.status === 'failed' && task?.error) {
    const e = String(task.error)
    return `失败: ${e.length > 160 ? e.slice(0, 160) + '…' : e}`
  }
  const engine = task?.resolvedEngine || task?.requestedEngine
  const engineLabel = {
    capture: '采集入库',
    mineru: '高质量强引擎',
    marker: 'Marker',
    fast: '快速引擎',
    anydoc: '极速引擎',
    ebook: '电子书解析'
  }[engine] || ''
  const base = task?.message || '等待处理'
  return engineLabel ? `[${engineLabel}] ${base}` : base
}
const handleRetry = (task) => {
  if (typeof props.retryTask === 'function') props.retryTask(task.id)
}
const handleCancel = (task) => {
  if (typeof props.cancelTask === 'function') props.cancelTask(task.id)
}
const handleDelete = (task) => {
  if (typeof props.deleteTask === 'function') props.deleteTask(task.id)
}
const handleClearFinished = () => {
  if (typeof props.clearFinished === 'function') props.clearFinished()
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

// ── 同步任务：状态点/标签/文案（与转换任务同款卡片风格）──
const syncStatusLabel = (progress) => {
  if (progress?.cancelled) return '已终止'
  if (isFailed(progress)) return '失败'
  if (Number(progress?.current) >= 100) return '已完成'
  return progress?.cancelling ? '终止中' : '同步中'
}
const syncTagType = (progress) => {
  if (progress?.cancelled) return 'warning'
  if (isFailed(progress)) return 'error'
  if (Number(progress?.current) >= 100) return 'success'
  return 'info'
}
const syncDotClass = (progress) => {
  if (progress?.cancelled) return 'dot-canceled'
  if (isFailed(progress)) return 'dot-failed'
  if (Number(progress?.current) >= 100) return 'dot-done'
  return 'dot-running'
}

// ── 备份/恢复/导入/导出：统一卡片（无控制按钮）──
const simpleGroups = [
  { key: 'backup', title: '备份任务', activeLabel: '备份中', icon: markRaw(SaveSync20Regular) },
  { key: 'recovery', title: '恢复任务', activeLabel: '恢复中', icon: markRaw(BoxArrowLeft20Regular) },
  { key: 'import', title: '导入任务', activeLabel: '导入中', icon: markRaw(TextBulletListAdd20Filled) },
  { key: 'export', title: '导出任务', activeLabel: '导出中', icon: markRaw(SignOut20Regular) },
]
const simpleStatusLabel = (progress, activeLabel) => {
  if (isFailed(progress)) return '失败'
  if (Number(progress?.current) >= 100) return '已完成'
  return activeLabel
}
const simpleTagType = (progress) => {
  if (isFailed(progress)) return 'error'
  if (Number(progress?.current) >= 100) return 'success'
  return 'info'
}
const simpleDotClass = (progress) => {
  if (isFailed(progress)) return 'dot-failed'
  if (Number(progress?.current) >= 100) return 'dot-done'
  return 'dot-running'
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

/* ── 转换任务区块（重设计） ── */
.section {
  margin-bottom: 16px;
}

.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
}

.section-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-weight: 600;
  font-size: 14px;
  color: var(--n-text-color, #333);
}

.section-title-icon {
  color: var(--n-color-primary, #409EFF);
}

.section-count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  border-radius: 9px;
  font-size: 11px;
  font-weight: 600;
  color: #fff;
  background: var(--n-color-primary, #409EFF);
}

.section-clear {
  color: var(--guaxi-text-secondary, #999);
}
.section-clear:hover {
  color: #ff4d4f;
}

.ctask {
  padding: 10px 12px;
  margin-bottom: 8px;
  border-radius: 10px;
  background: var(--guaxi-panel-bg, rgba(255, 255, 255, 0.03));
  border: 1px solid var(--guaxi-border, rgba(255, 255, 255, 0.08));
  transition: border-color 0.2s ease, box-shadow 0.2s ease;
}
.ctask:hover {
  border-color: var(--n-color-primary, #409EFF);
  box-shadow: 0 2px 10px rgba(64, 158, 255, 0.12);
}
.ctask.is-failed {
  border-color: rgba(255, 77, 79, 0.4);
}
.ctask.is-canceled {
  opacity: 0.75;
}

.ctask-top {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.ctask-engine-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  background: #909399;
}
.ctask-engine-dot.eng-fast { background: #67c23a; }
.ctask-engine-dot.eng-anydoc { background: #a855f7; }
.ctask-engine-dot.eng-marker { background: #e6a23c; }
.ctask-engine-dot.eng-mineru { background: #409EFF; }
.ctask-engine-dot.eng-auto { background: #909399; }
.ctask-engine-dot.eng-capture { background: #14b8a6; }
/* 同步任务状态点 */
.ctask-engine-dot.dot-running { background: #409EFF; }
.ctask-engine-dot.dot-done { background: #67c23a; }
.ctask-engine-dot.dot-failed { background: #f56c6c; }
.ctask-engine-dot.dot-canceled { background: #e6a23c; }

.ctask-name {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--n-text-color, #333);
}

.ctask-status {
  flex-shrink: 0;
}

.ctask-ops {
  display: flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
}

.ctask-op {
  border-radius: 6px;
  padding: 2px;
  transition: background 0.15s ease;
}
.ctask-op:hover {
  background: var(--guaxi-header-bg, rgba(255, 255, 255, 0.06));
}

.ctask-progress {
  margin: 4px 0 6px;
}

.ctask-msg {
  font-size: 12px;
  line-height: 1.5;
  color: var(--guaxi-text-secondary, #999);
  word-break: break-word;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.ctask-msg.is-error {
  color: #ff7875;
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

.task-op-btn {
  flex-shrink: 0;
  margin-left: 4px;
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
