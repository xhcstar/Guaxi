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

    <label v-else class="py-session-panel__search">
      <Icon :icon="searchIcon" width="15" height="15" />
      <input v-model="variableSearch" type="text" placeholder="搜索变量或类型" />
    </label>

    <div v-if="variables.length && !filteredVariables.length" class="py-session-panel__empty">
      没有匹配的会话变量。
    </div>

    <div v-else-if="filteredVariables.length" class="py-session-panel__list">
      <article
        v-for="item in filteredVariables"
        :key="`${inspector.cellId}-${item.name}`"
        class="py-session-panel__card"
        :class="{ 'is-selected': isVariableSelected(item) }"
        role="button"
        tabindex="0"
        @click="focusVariableSource(item)"
        @keydown.enter.prevent="focusVariableSource(item)"
        @keydown.space.prevent="focusVariableSource(item)"
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
              data-tooltip="打开变量工作台"
              aria-label="在独立变量工作台中处理"
              @click.stop="openVariableWorkbench(item)"
            >
              <Icon :icon="tableIcon" width="15" height="15" />
            </button>
            <button
              type="button"
              class="py-session-panel__tool-btn"
              data-tooltip="复制为新变量"
              aria-label="复制变量为新变量"
              @click.stop="copyVariableObject(item)"
            >
              <Icon :icon="copyPlusIcon" width="15" height="15" />
            </button>
            <button
              type="button"
              class="py-session-panel__tool-btn"
              data-tooltip="复制美观打印内容"
              aria-label="复制变量美观打印内容"
              @click.stop="copyVariablePrettyText(item)"
            >
              <Icon :icon="copyIcon" width="15" height="15" />
            </button>
            <button
              type="button"
              class="py-session-panel__tool-btn"
              data-tooltip="重命名"
              aria-label="重命名变量"
              @click.stop="renameVariable(item)"
            >
              <Icon :icon="pencilIcon" width="15" height="15" />
            </button>
            <button
              type="button"
              class="py-session-panel__tool-btn danger"
              data-tooltip="删除"
              aria-label="删除变量"
              @click.stop="deleteVariable(item)"
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

        <div class="py-session-panel__hint">点击定位到对应 Python Cell</div>
      </article>
    </div>
  </section>
</template>

<script setup>
import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Icon } from '@iconify/vue'
import copyIcon from '@iconify-icons/lucide/copy'
import copyPlusIcon from '@iconify-icons/lucide/copy-plus'
import pencilIcon from '@iconify-icons/lucide/pencil'
import searchIcon from '@iconify-icons/lucide/search'
import tableIcon from '@iconify-icons/lucide/table-properties'
import trashIcon from '@iconify-icons/lucide/trash-2'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { useMessage } from 'naive-ui'
import { createLogger } from '../../utils/logger'
import { useRightSiderStore } from '../../stores/rightSiderStore'
import { useSettingsStore } from '../../stores/settingsStore'
import { filterVisibleWorkbenchSessionVariables } from './utils/guaxiDataLabWorkbenchClient'
import { emitNotebookDataLabAction } from './utils/guaxiNotebookDataLab'

const logger = createLogger('GuaxiNotebookSessionPanel')
const message = useMessage()
const rightSiderStore = useRightSiderStore()
const settingsStore = useSettingsStore()
const variableSearch = ref('')
const selectedVariableName = ref('')

const inspector = computed(() => rightSiderStore.pythonSessionInspector || {})
const appearanceTheme = computed(() => settingsStore.appearanceTheme === 'dark' ? 'dark' : 'light')
const variables = computed(() => {
  const rawVariables = Array.isArray(inspector.value?.variables) ? inspector.value.variables : []
  const visibleVariables = filterVisibleWorkbenchSessionVariables(rawVariables)
  if (visibleVariables.length !== rawVariables.length) {
    logger.info('[NotebookSessionPanel] hidden internal session variables', {
      rawVariableCount: rawVariables.length,
      visibleVariableCount: visibleVariables.length,
    })
  }
  return visibleVariables
})
const filteredVariables = computed(() => {
  const keyword = String(variableSearch.value || '').trim().toLowerCase()
  if (!keyword) return variables.value
  return variables.value.filter((item) => {
    const haystack = [
      item?.name,
      item?.variableName,
      item?.value_type,
      item?.type,
    ].map((value) => String(value || '').toLowerCase()).join(' ')
    return haystack.includes(keyword)
  })
})
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

const getVariableName = (item) => String(item?.name || item?.variableName || '').trim()

const isVariableSelected = (item) => getVariableName(item) === String(selectedVariableName.value || '')

const focusVariableSource = (item) => {
  const variableName = getVariableName(item)
  if (!variableName) return
  selectedVariableName.value = variableName
  const emitted = emitInspectorAction('focus-variable', { variableName })
  logger.info('[NotebookSessionPanel] selected session variable and requested notebook focus', {
    cellId: String(inspector.value?.cellId || ''),
    cellKey: String(inspector.value?.cellKey || ''),
    variableName,
    emitted,
  })
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
  const variableName = getVariableName(item)
  if (!variableName) return
  const targetName = String(window.prompt?.(`复制变量 ${variableName} 为`, `${variableName}_copy`) || '').trim()
  if (!targetName || targetName === variableName) return
  emitInspectorAction('variable-copy', { variableName, targetName })
}

const deleteVariable = (item) => {
  const variableName = getVariableName(item)
  if (!variableName) return
  if (!window.confirm?.(`确认删除会话变量 ${variableName}？`)) return
  emitInspectorAction('variable-delete', { variableName })
}

const buildVariablePrettyPrintCode = (variableName) => [
  'import pprint',
  `__guaxi_copy_name = ${JSON.stringify(variableName)}`,
  'try:',
  '    if __guaxi_copy_name not in globals():',
  '        raise KeyError(f"变量不存在: {__guaxi_copy_name}")',
  '    __guaxi_copy_value = globals()[__guaxi_copy_name]',
  '    if hasattr(__guaxi_copy_value, "to_string") and callable(getattr(__guaxi_copy_value, "to_string", None)):',
  '        __guaxi_copy_text = __guaxi_copy_value.to_string()',
  '    else:',
  '        __guaxi_copy_text = pprint.pformat(__guaxi_copy_value, width=100, compact=False, sort_dicts=False)',
  '    print(str(__guaxi_copy_text))',
  'finally:',
  '    for __guaxi_copy_key in ("__guaxi_copy_name", "__guaxi_copy_value", "__guaxi_copy_text", "__guaxi_copy_key"):',
  '        globals().pop(__guaxi_copy_key, None)',
].join('\n')

// 设计说明：
// 1. 右侧变量卡片不再展示 preview，因此“复制”不能继续复制摘要字段，否则用户拿到的是截断内容。
// 2. 这里只在当前持久 Python 会话里执行一段无状态 pprint/to_string 代码，并在 finally 中清理临时变量。
// 3. 不追加 Notebook 输出模块，不改预览模式和终端逻辑；前端日志记录复制来源和实际文本长度。
const copyVariablePrettyText = async (item) => {
  const variableName = getVariableName(item)
  const sessionId = String(inspector.value?.sessionId || '').trim()
  if (!variableName) return
  if (!sessionId) {
    message.warning('当前 Python 会话尚未准备好')
    return
  }

  try {
    const response = await invoke('python_exec', {
      request: {
        code: buildVariablePrettyPrintCode(variableName),
        persist: true,
        expect_json: false,
        include_session_vars: false,
        vars: {
          _guaxi_session: sessionId,
          _doc_path: String(inspector.value?.filePath || ''),
          _py_cell_id: String(inspector.value?.cellId || ''),
        },
      },
    })
    if (!response?.ok) {
      throw new Error(String(response?.stderr || response?.stdout || '复制变量内容失败'))
    }
    const text = String(response?.stdout || response?.result_repr || '').trimEnd()
    if (!text) {
      message.warning('当前变量没有可复制的文本内容')
      return
    }
    const copied = await writeClipboard(text)
    if (!copied) {
      message.error('复制变量内容失败')
      return
    }
    logger.info('[NotebookSessionPanel] copied variable pretty text', {
      cellId: String(inspector.value?.cellId || ''),
      sessionId,
      variableName,
      textLength: text.length,
    })
    message.success(`已复制变量 ${variableName} 的美观打印内容`)
  } catch (error) {
    logger.error('[NotebookSessionPanel] copy variable pretty text failed', {
      cellId: String(inspector.value?.cellId || ''),
      sessionId,
      variableName,
      error: String(error?.message || error),
    })
    message.error(`复制变量内容失败：${String(error?.message || error)}`)
  }
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
  --py-session-hover-bg: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 70%, var(--guaxi-content-bg, #ffffff) 30%);
  --py-session-selected-bg: color-mix(in srgb, #2563eb 10%, var(--guaxi-panel-bg, #ffffff) 90%);
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
  --py-session-hover-bg: #353d49;
  --py-session-selected-bg: color-mix(in srgb, #60a5fa 16%, #2a303a 84%);
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

.py-session-panel__search {
  flex: 0 0 auto;
  min-height: 34px;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  margin-top: 12px;
  border: 1px solid var(--py-session-border);
  border-radius: 8px;
  padding: 0 10px;
  background: var(--py-session-chip-bg);
  color: var(--py-session-muted);
}

.py-session-panel__search input {
  min-width: 0;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--py-session-text);
  font: inherit;
}

.py-session-panel__list {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  display: grid;
  align-content: start;
  gap: 8px;
  margin-top: 16px;
  padding-right: 2px;
}

.py-session-panel__card {
  display: grid;
  gap: 8px;
  padding: 11px 12px;
  border-radius: 8px;
  border: 1px solid var(--py-session-border);
  background: var(--py-session-card-bg);
  box-shadow: var(--py-session-shadow);
  cursor: pointer;
  outline: none;
  transition: background 0.18s ease, border-color 0.18s ease, box-shadow 0.18s ease, transform 0.18s ease;
}

.py-session-panel__card:hover,
.py-session-panel__card:focus-visible {
  background: var(--py-session-hover-bg);
  border-color: color-mix(in srgb, #2563eb 32%, var(--py-session-border-strong));
  transform: translateY(-1px);
}

.py-session-panel__card.is-selected {
  background: var(--py-session-selected-bg);
  border-color: color-mix(in srgb, #2563eb 58%, var(--py-session-border-strong));
  box-shadow: 0 0 0 3px color-mix(in srgb, #2563eb 14%, transparent), var(--py-session-shadow);
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
  font-size: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
  position: relative;
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

.py-session-panel__tool-btn::after {
  content: attr(data-tooltip);
  position: absolute;
  right: 50%;
  top: calc(100% + 8px);
  z-index: 30;
  width: max-content;
  max-width: min(240px, calc(100vw - 48px));
  padding: 6px 8px;
  border-radius: 7px;
  background: color-mix(in srgb, #111827 92%, transparent);
  color: #ffffff;
  font-size: 12px;
  line-height: 1.35;
  text-align: center;
  white-space: normal;
  overflow-wrap: anywhere;
  opacity: 0;
  pointer-events: none;
  transform: translate(50%, -4px);
  transition: opacity 0.14s ease, transform 0.14s ease;
  box-shadow: 0 10px 24px rgba(15, 23, 42, 0.18);
}

.py-session-panel__tool-btn:hover::after,
.py-session-panel__tool-btn:focus-visible::after {
  opacity: 1;
  transform: translate(50%, 0);
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

.py-session-panel__hint {
  font-size: 11px;
  color: color-mix(in srgb, var(--py-session-muted) 88%, transparent);
}
</style>
