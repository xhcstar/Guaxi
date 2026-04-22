<template>
  <section class="py-session-panel">
    <header class="py-session-panel__header">
      <div>
        <div class="py-session-panel__eyebrow">Python Session</div>
        <h3 class="py-session-panel__title">会话变量</h3>
      </div>
      <div class="py-session-panel__summary">
        <span>{{ summaryLabel }}</span>
        <span v-if="inspector.sessionId" class="py-session-panel__session-id">{{ inspector.sessionId }}</span>
      </div>
    </header>

    <div v-if="!inspector.hasActiveCell" class="py-session-panel__empty">
      选中一个 Python Cell 后，这里会展示当前共享会话里的完整变量内容。
    </div>

    <div v-else-if="!variables.length" class="py-session-panel__empty">
      当前会话里还没有可见变量。
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
          <button
            type="button"
            class="py-session-panel__copy-btn"
            title="复制变量内容"
            aria-label="复制变量内容"
            @click="copyVariablePreview(item)"
          >
            <Icon :icon="copyIcon" width="15" height="15" />
          </button>
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
import { useMessage } from 'naive-ui'
import { createLogger } from '../../utils/logger'
import { useRightSiderStore } from '../../stores/rightSiderStore'

const logger = createLogger('GuaxiNotebookSessionPanel')
const message = useMessage()
const rightSiderStore = useRightSiderStore()

const inspector = computed(() => rightSiderStore.pythonSessionInspector || {})
const variables = computed(() => (Array.isArray(inspector.value?.variables) ? inspector.value.variables : []))
const summaryLabel = computed(() => {
  if (!inspector.value?.hasActiveCell) {
    return '未选中 Python Cell'
  }
  if (String(inspector.value?.cellTitle || '').trim()) {
    return inspector.value.cellTitle
  }
  if (String(inspector.value?.cellKey || '').trim()) {
    return `Cell ${inspector.value.cellKey}`
  }
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

// Design notes:
// 1. Session variables can now be very large, so the copy action is scoped to the
//    exact preview payload shown in the right panel.
// 2. The button stays on each card to avoid forcing users to select text inside a
//    nested scrolling preview region.
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
  message.success(`已复制变量 ${String(item?.name || '')}`)
}
</script>

<style scoped>
.py-session-panel {
  /*
   * 设计说明：
   * 1. 会话变量面板内部有“页面背景 / 卡片背景 / 预览框背景”三层表面，如果都直接从
   *    `--guaxi-panel-bg` 混白得到，暗色主题下很容易变成一片发灰、文字对比不足。
   * 2. 这里为组件单独定义一组局部变量，亮色和暗色分别给出更清晰的层级，避免预览区和正文
   *    在暗色主题里融在一起。
   * 3. 变量全部基于应用现有主题色，保证和右侧栏整体风格一致，不会引入新的主题状态源。
   */
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
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--py-session-border);
}

.py-session-panel__eyebrow {
  font-size: 11px;
  letter-spacing: 0.08em;
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

.py-session-panel__session-id {
  max-width: 100%;
  word-break: break-all;
}

.py-session-panel__empty {
  margin-top: 16px;
  padding: 18px 16px;
  border-radius: 14px;
  border: 1px dashed var(--py-session-border-strong);
  color: var(--py-session-muted);
  background: var(--py-session-card-bg);
}

.py-session-panel__list {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  margin-top: 16px;
  display: grid;
  gap: 12px;
  padding-right: 2px;
}

.py-session-panel__card {
  display: grid;
  gap: 10px;
  padding: 14px;
  border-radius: 14px;
  border: 1px solid var(--py-session-border);
  background: var(--py-session-card-bg);
  box-shadow: var(--py-session-shadow);
}

.py-session-panel__card-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
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

.py-session-panel__copy-btn {
  width: 28px;
  height: 28px;
  border: 1px solid var(--py-session-border);
  border-radius: 9px;
  background: transparent;
  color: var(--py-session-muted);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 0.18s ease, border-color 0.18s ease, transform 0.18s ease;
}

.py-session-panel__copy-btn:hover {
  background: var(--py-session-hover-bg);
  border-color: var(--py-session-border-strong);
  color: var(--py-session-text);
  transform: translateY(-1px);
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
  border-radius: 12px;
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
