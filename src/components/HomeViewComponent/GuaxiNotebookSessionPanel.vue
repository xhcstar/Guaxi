<template>
  <section class="py-session-panel">
    <header class="py-session-panel__header">
      <div class="py-session-panel__title-block">
        <div class="py-session-panel__eyebrow">Session Vars</div>
        <h3 class="py-session-panel__title">会话变量</h3>
      </div>
      <div class="py-session-panel__summary">
        <span>{{ summaryLabel }}</span>
        <span v-if="inspector.hasActiveCell">{{ variables.length }} 个变量</span>
      </div>
    </header>

    <div v-if="!inspector.hasActiveCell" class="py-session-panel__empty">
      选中一个 Python Cell 后，会显示当前内存会话里仍然存在的变量。
    </div>

    <div v-else-if="!variables.length" class="py-session-panel__empty">
      当前内存会话里还没有可见变量。运行 Python Cell 后这里会刷新。
    </div>

    <div v-else class="py-session-panel__list">
      <article
        v-for="item in variables"
        :key="`${inspector.cellId}-${item.name}`"
        class="py-session-panel__card"
      >
        <div class="py-session-panel__card-head">
          <div class="py-session-panel__identity">
            <span class="py-session-panel__name">{{ item.name }}</span>
            <span class="py-session-panel__type">{{ item.value_type || item.type || 'value' }}</span>
          </div>
          <div class="py-session-panel__card-actions">
            <button
              type="button"
              class="py-session-panel__tool-btn"
              title="在独立变量工作台中处理"
              aria-label="在独立变量工作台中处理"
              @click="openVariableWorkbench(item)"
            >
              <Icon :icon="tableIcon" width="15" height="15" />
            </button>
            <button
              type="button"
              class="py-session-panel__tool-btn"
              title="复制变量为新变量"
              aria-label="复制变量为新变量"
              @click="copyVariableObject(item)"
            >
              <Icon :icon="copyPlusIcon" width="15" height="15" />
            </button>
            <button
              type="button"
              class="py-session-panel__tool-btn"
              title="复制变量预览文本"
              aria-label="复制变量预览文本"
              @click="copyVariablePreview(item)"
            >
              <Icon :icon="copyIcon" width="15" height="15" />
            </button>
            <button
              type="button"
              class="py-session-panel__tool-btn"
              title="重命名变量"
              aria-label="重命名变量"
              @click="renameVariable(item)"
            >
              <Icon :icon="pencilIcon" width="15" height="15" />
            </button>
            <button
              type="button"
              class="py-session-panel__tool-btn danger"
              title="删除变量"
              aria-label="删除变量"
              @click="deleteVariable(item)"
            >
              <Icon :icon="trashIcon" width="15" height="15" />
            </button>
          </div>
        </div>

        <div class="py-session-panel__meta">
          <span v-if="item.length != null">len {{ item.length }}</span>
          <span v-if="item.rows != null">rows {{ item.rows }}</span>
          <span v-if="item.columns != null">cols {{ item.columns }}</span>
          <span v-if="formatShape(item.shape)">shape {{ formatShape(item.shape) }}</span>
        </div>

        <pre class="py-session-panel__preview">{{ item.preview || '' }}</pre>
      </article>
    </div>
  </section>
</template>

<script setup>
import { computed } from 'vue'
import { Icon } from '@iconify/vue'
import copyIcon from '@iconify-icons/lucide/copy'
import copyPlusIcon from '@iconify-icons/lucide/copy-plus'
import pencilIcon from '@iconify-icons/lucide/pencil'
import tableIcon from '@iconify-icons/lucide/table-properties'
import trashIcon from '@iconify-icons/lucide/trash-2'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { useMessage } from 'naive-ui'
import { createLogger } from '../../utils/logger'
import { useRightSiderStore } from '../../stores/rightSiderStore'
import { useSettingsStore } from '../../stores/settingsStore'
import { emitNotebookDataLabAction } from './utils/guaxiNotebookDataLab'

const logger = createLogger('GuaxiNotebookSessionPanel')
const message = useMessage()
const rightSiderStore = useRightSiderStore()
const settingsStore = useSettingsStore()

const inspector = computed(() => rightSiderStore.pythonSessionInspector || {})
const appearanceTheme = computed(() => settingsStore.appearanceTheme === 'dark' ? 'dark' : 'light')
const variables = computed(() => (Array.isArray(inspector.value?.variables) ? inspector.value.variables : []))
const summaryLabel = computed(() => {
  if (!inspector.value?.hasActiveCell) return '未选中 Python Cell'
  if (String(inspector.value?.cellTitle || '').trim()) return inspector.value.cellTitle
  if (String(inspector.value?.cellKey || '').trim()) return `Cell ${inspector.value.cellKey}`
  return '当前 Python Cell'
})

const formatShape = (shape) => {
  if (!Array.isArray(shape) || !shape.length) return ''
  return `(${shape.map((item) => String(item ?? '')).join(', ')})`
}

const writeClipboard = async (text) => {
  const value = String(text || '')
  if (!value) return false

  if (navigator?.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(value)
      return true
    } catch (error) {
      logger.warn('[NotebookSessionPanel] navigator clipboard write failed', {
        error: String(error?.message || error),
      })
    }
  }

  try {
    const mod = await import('@tauri-apps/plugin-clipboard-manager')
    await mod.writeText(value)
    return true
  } catch (error) {
    logger.error('[NotebookSessionPanel] clipboard write failed', {
      error: String(error?.message || error),
    })
    return false
  }
}

const emitInspectorAction = (action, extra = {}) => {
  if (!inspector.value?.hasActiveCell) {
    message.warning('请先选中一个 Python Cell')
    return false
  }

  const detail = {
    action: String(action || ''),
    filePath: String(inspector.value?.filePath || ''),
    cellId: String(inspector.value?.cellId || ''),
    cellKey: String(inspector.value?.cellKey || ''),
    stableId: String(inspector.value?.stableId || ''),
    ...extra,
  }
  const emitted = emitNotebookDataLabAction(detail)
  logger.info('[NotebookSessionPanel] emit session variable action', {
    action: detail.action,
    cellId: detail.cellId,
    variableName: String(detail.variableName || ''),
    emitted,
  })
  return emitted
}

const openVariableWorkbench = async (item) => {
  const variableName = String(item?.variableName || item?.name || '').trim()
  if (!variableName) {
    message.warning('变量名为空，无法打开变量工作台')
    return
  }
  if (!inspector.value?.hasActiveCell) {
    message.warning('请先选中一个 Python Cell')
    return
  }

  const sessionId = String(inspector.value?.sessionId || '').trim()
  if (!sessionId) {
    message.warning('当前 Python 会话尚未准备好')
    return
  }

  const dataKind = String(item?.dataKind || item?.value_type || item?.type || '').trim() || 'value'
  const label = `data-lab-${sessionId.replace(/[^a-zA-Z0-9_:-]/g, '_')}`.slice(0, 110)
  const params = new URLSearchParams({
    sessionId,
    variableName,
    dataKind,
    filePath: String(inspector.value?.filePath || ''),
    cellId: String(inspector.value?.cellId || ''),
    cellKey: String(inspector.value?.cellKey || ''),
    stableId: String(inspector.value?.stableId || ''),
    theme: appearanceTheme.value,
  })

  try {
    const existing = await WebviewWindow.getByLabel(label)
    if (existing) {
      await existing.show()
      await existing.setFocus()
      logger.info('[NotebookSessionPanel] focus existing variable workbench', { label, sessionId, variableName })
      return
    }

    const workbench = new WebviewWindow(label, {
      url: `/data-lab-workbench?${params.toString()}`,
      title: '变量工作台',
      width: 1280,
      height: 820,
      minWidth: 980,
      minHeight: 680,
      visible: true,
    })
    workbench.once('tauri://created', () => {
      logger.info('[NotebookSessionPanel] variable workbench created', { label, sessionId, variableName, dataKind })
    })
    workbench.once('tauri://error', (event) => {
      logger.error('[NotebookSessionPanel] variable workbench create failed', {
        label,
        variableName,
        error: String(event?.payload || ''),
      })
      message.error('打开变量工作台失败')
    })
  } catch (error) {
    logger.error('[NotebookSessionPanel] open variable workbench failed', {
      variableName,
      error: String(error?.message || error),
    })
    message.error('打开变量工作台失败')
  }
}

const renameVariable = (item) => {
  const variableName = String(item?.name || item?.variableName || '').trim()
  if (!variableName) return
  const targetName = String(window.prompt?.(`将变量 ${variableName} 重命名为`, `${variableName}_next`) || '').trim()
  if (!targetName || targetName === variableName) return
  emitInspectorAction('variable-rename', { variableName, targetName })
}

const copyVariableObject = (item) => {
  const variableName = String(item?.name || item?.variableName || '').trim()
  if (!variableName) return
  const targetName = String(window.prompt?.(`复制变量 ${variableName} 为`, `${variableName}_copy`) || '').trim()
  if (!targetName || targetName === variableName) return
  emitInspectorAction('variable-copy', { variableName, targetName })
}

const deleteVariable = (item) => {
  const variableName = String(item?.name || item?.variableName || '').trim()
  if (!variableName) return
  if (!window.confirm?.(`确认删除会话变量 ${variableName}？`)) return
  emitInspectorAction('variable-delete', { variableName })
}

// 设计说明：
// 1. 会话变量可能很大，右侧面板的复制只复制当前卡片显示的 preview，避免误把大对象完整写入剪贴板导致卡顿。
// 2. 真正的变量复制走 Python 会话内的 variable-copy 动作，保留类型和结构。
// 3. 两条路径分别记录日志，便于排查“复制文本”和“复制变量对象”被混用的问题。
const copyVariablePreview = async (item) => {
  const text = String(item?.preview || '')
  if (!text) {
    message.warning('当前变量没有可复制的文本内容')
    return
  }

  const copied = await writeClipboard(text)
  if (!copied) {
    message.error('复制变量内容失败')
    return
  }

  logger.info('[NotebookSessionPanel] copied variable preview', {
    cellId: String(inspector.value?.cellId || ''),
    variableName: String(item?.name || ''),
    textLength: text.length,
  })
  message.success(`已复制变量 ${String(item?.name || '')} 的预览`)
}
</script>

<style scoped>
.py-session-panel {
  --py-session-text: var(--guaxi-text-primary, #1f2328);
  --py-session-muted: var(--guaxi-text-secondary, #57606a);
  --py-session-border: color-mix(in srgb, var(--guaxi-border, #d0d7de) 78%, transparent);
  --py-session-border-strong: color-mix(in srgb, var(--guaxi-border, #d0d7de) 92%, transparent);
  --py-session-card-bg: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 92%, var(--guaxi-content-bg, #ffffff) 8%);
  --py-session-chip-bg: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 82%, var(--guaxi-content-bg, #ffffff) 18%);
  --py-session-preview-bg: color-mix(in srgb, var(--guaxi-content-bg, #ffffff) 88%, var(--guaxi-panel-bg, #ffffff) 12%);
  --py-session-hover-bg: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 70%, var(--guaxi-content-bg, #ffffff) 30%);
  --py-session-shadow: 0 10px 24px rgba(15, 23, 42, 0.08);
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 18px 16px;
  background: var(--guaxi-panel-bg);
  color: var(--py-session-text);
}

:global(:root[data-guaxi-theme='dark']) .py-session-panel {
  --py-session-text: #e8edf6;
  --py-session-muted: #aeb8ca;
  --py-session-border: rgba(151, 164, 186, 0.18);
  --py-session-border-strong: rgba(151, 164, 186, 0.28);
  --py-session-card-bg: #2a303a;
  --py-session-chip-bg: #303744;
  --py-session-preview-bg: #3a404b;
  --py-session-hover-bg: #353d49;
  --py-session-shadow: 0 16px 36px rgba(0, 0, 0, 0.28);
}

.py-session-panel__header {
  flex: 0 0 auto;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--py-session-border);
}

.py-session-panel__title-block {
  min-width: 0;
}

.py-session-panel__eyebrow {
  font-size: 11px;
  letter-spacing: 0;
  text-transform: uppercase;
  color: var(--py-session-muted);
}

.py-session-panel__title {
  margin: 4px 0 0;
  font-size: 18px;
  line-height: 1.2;
  color: var(--py-session-text);
}

.py-session-panel__summary {
  min-width: 0;
  display: grid;
  gap: 4px;
  justify-items: end;
  text-align: right;
  font-size: 12px;
  color: var(--py-session-muted);
}

.py-session-panel__empty {
  margin-top: 16px;
  padding: 18px 16px;
  border-radius: 8px;
  border: 1px dashed var(--py-session-border-strong);
  color: var(--py-session-muted);
  background: var(--py-session-card-bg);
  line-height: 1.7;
}

.py-session-panel__list {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  display: grid;
  align-content: start;
  gap: 12px;
  margin-top: 16px;
  padding-right: 2px;
}

.py-session-panel__card {
  display: grid;
  gap: 10px;
  padding: 14px;
  border-radius: 8px;
  border: 1px solid var(--py-session-border);
  background: var(--py-session-card-bg);
  box-shadow: var(--py-session-shadow);
}

.py-session-panel__card-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
}

.py-session-panel__identity {
  min-width: 0;
  display: grid;
  gap: 4px;
}

.py-session-panel__name {
  font-weight: 700;
  font-size: 15px;
  word-break: break-word;
  color: var(--py-session-text);
}

.py-session-panel__type {
  font-size: 12px;
  color: var(--py-session-muted);
}

.py-session-panel__card-actions {
  flex: 0 0 auto;
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 6px;
}

.py-session-panel__tool-btn {
  width: 28px;
  height: 28px;
  border: 1px solid var(--py-session-border);
  border-radius: 8px;
  background: transparent;
  color: var(--py-session-muted);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 0.18s ease, border-color 0.18s ease, transform 0.18s ease;
}

.py-session-panel__tool-btn:hover {
  background: var(--py-session-hover-bg);
  border-color: var(--py-session-border-strong);
  transform: translateY(-1px);
}

.py-session-panel__tool-btn.danger {
  color: #b42318;
  border-color: color-mix(in srgb, #d92d20 38%, var(--py-session-border));
}

.py-session-panel__meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.py-session-panel__meta span {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 8px;
  border-radius: 999px;
  background: var(--py-session-chip-bg);
  font-size: 11px;
  color: var(--py-session-muted);
}

.py-session-panel__preview {
  margin: 0;
  padding: 12px;
  max-height: 280px;
  overflow: auto;
  border-radius: 8px;
  border: 1px solid var(--py-session-border);
  background: var(--py-session-preview-bg);
  color: var(--py-session-text);
  font-family: var(--editor-font-family, "Maple Mono NF CN", "JetBrainsMono Nerd Font Mono", Consolas, monospace);
  font-size: 13px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
