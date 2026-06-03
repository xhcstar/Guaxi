<template>
  <section class="math-ws-panel">
    <header class="math-ws-panel__header">
      <div class="math-ws-panel__title-block">
        <div class="math-ws-panel__eyebrow">Math Workspace</div>
        <h3 class="math-ws-panel__title">数学工作区</h3>
      </div>
      <div class="math-ws-panel__summary">
        <span>{{ summaryLabel }}</span>
        <span v-if="inspector.hasActiveCell">{{ symbols.length }} 变量 · {{ carriers.length }} 工作台</span>
      </div>
    </header>

    <div v-if="!inspector.hasActiveCell" class="math-ws-panel__empty">
      选中一个数学块（点击其工具栏的工作区按钮）后，这里会显示该会话里的工作区变量与可在独立工作台中复跑的重型结果。
    </div>

    <template v-else>
      <!-- 工作区符号（只读）：__GUAXIMATH_WS__ -->
      <div class="math-ws-panel__section-title">工作区变量</div>
      <div v-if="!symbols.length" class="math-ws-panel__empty math-ws-panel__empty--soft">
        当前会话还没有数学工作区变量。运行 `f = sin(x)/x` 这类赋值后会刷新。
      </div>
      <div v-else class="math-ws-panel__list">
        <article
          v-for="item in symbols"
          :key="`sym-${item.name}`"
          class="math-ws-panel__card"
        >
          <div class="math-ws-panel__identity">
            <Icon :icon="variableIcon" width="15" height="15" />
            <span class="math-ws-panel__name">{{ item.name }}</span>
          </div>
          <span class="math-ws-panel__type">{{ item.type || 'symbol' }}</span>
        </article>
      </div>

      <!-- 可复跑 carrier：__GUAXIMATH_CARRIERS__ -->
      <div class="math-ws-panel__section-title">数值 / 仿真工作台</div>
      <div v-if="!carriers.length" class="math-ws-panel__empty math-ws-panel__empty--soft">
        还没有重型结果。运行 `nsolve(...)`、`minimize(...)`、`odeint(...)`、`phase_portrait(...)` 等会在这里登记一个可复跑工作台。
      </div>
      <div v-else class="math-ws-panel__list">
        <article
          v-for="item in carriers"
          :key="`carrier-${item.key}`"
          class="math-ws-panel__card math-ws-panel__card--carrier"
        >
          <div class="math-ws-panel__card-head">
            <div class="math-ws-panel__identity">
              <Icon :icon="item.kind === 'simulation' ? activityIcon : sigmaIcon" width="15" height="15" />
              <span class="math-ws-panel__name">{{ item.label || item.command }}</span>
            </div>
            <button
              type="button"
              class="math-ws-panel__tool-btn"
              data-tooltip="在独立数学工作台中打开（参数滑块 / 收敛回放）"
              aria-label="打开数学工作台"
              @click="openMathWorkbench(item)"
            >
              <Icon :icon="externalLinkIcon" width="15" height="15" />
            </button>
          </div>
          <div class="math-ws-panel__meta">
            <span class="math-ws-panel__chip">{{ item.command }}</span>
            <span v-if="item.summary">{{ item.summary }}</span>
          </div>
        </article>
      </div>
    </template>
  </section>
</template>

<script setup>
import { computed } from 'vue'
import { Icon } from '@iconify/vue'
import sigmaIcon from '@iconify-icons/lucide/sigma'
import variableIcon from '@iconify-icons/lucide/variable'
import activityIcon from '@iconify-icons/lucide/activity'
import externalLinkIcon from '@iconify-icons/lucide/external-link'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { useMessage } from 'naive-ui'
import { createLogger } from '../../utils/logger'
import { useMathWorkspaceStore } from '../../stores/mathWorkspaceStore'
import { useSettingsStore } from '../../stores/settingsStore'

// 设计说明：本面板与 Python Cell 的 GuaxiNotebookSessionPanel 完全独立——
// 不 import 它、不共享状态，只读自己的 mathWorkspaceStore，并用通用 WebviewWindow 开独立工作台。
const logger = createLogger('GuaxiNotebookMathWorkspacePanel')
const message = useMessage()
const mathWorkspaceStore = useMathWorkspaceStore()
const settingsStore = useSettingsStore()

const inspector = computed(() => mathWorkspaceStore.inspector || {})
const symbols = computed(() => Array.isArray(inspector.value?.workspaceSymbols) ? inspector.value.workspaceSymbols : [])
const carriers = computed(() => Array.isArray(inspector.value?.carriers) ? inspector.value.carriers : [])
const appearanceTheme = computed(() => settingsStore.appearanceTheme === 'dark' ? 'dark' : 'light')

const summaryLabel = computed(() => {
  if (!inspector.value?.hasActiveCell) return '未选中数学块'
  if (String(inspector.value?.cellTitle || '').trim()) return inspector.value.cellTitle
  if (String(inspector.value?.cellKey || '').trim()) return `Cell ${inspector.value.cellKey}`
  return '当前数学块'
})

const openMathWorkbench = async (item) => {
  const sessionId = String(inspector.value?.sessionId || '').trim()
  const carrierKey = String(item?.key || '').trim()
  if (!sessionId) {
    message.warning('当前数学会话尚未准备好')
    return
  }
  if (!carrierKey) {
    message.warning('该工作台缺少标识')
    return
  }
  const label = `math-lab-${sessionId.replace(/[^a-zA-Z0-9_:-]/g, '_')}`.slice(0, 110)
  const params = new URLSearchParams({
    sessionId,
    carrierKey,
    command: String(item?.command || ''),
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
      logger.info('[MathWorkspacePanel] focus existing math workbench', { label, sessionId, carrierKey })
      return
    }
    const workbench = new WebviewWindow(label, {
      url: `/math-workbench?${params.toString()}`,
      title: '数学工作台',
      width: 1180,
      height: 800,
      minWidth: 900,
      minHeight: 620,
      visible: true,
    })
    workbench.once('tauri://created', () => {
      logger.info('[MathWorkspacePanel] math workbench created', { label, sessionId, carrierKey })
    })
    workbench.once('tauri://error', (event) => {
      logger.error('[MathWorkspacePanel] math workbench create failed', {
        label, carrierKey, error: String(event?.payload || ''),
      })
      message.error('打开数学工作台失败')
    })
  } catch (error) {
    logger.error('[MathWorkspacePanel] open math workbench failed', {
      carrierKey, error: String(error?.message || error),
    })
    message.error('打开数学工作台失败')
  }
}
</script>

<style scoped>
.math-ws-panel {
  --mw-text: var(--guaxi-text-primary, #1f2328);
  --mw-muted: var(--guaxi-text-secondary, #57606a);
  --mw-border: color-mix(in srgb, var(--guaxi-border, #d0d7de) 78%, transparent);
  --mw-border-strong: color-mix(in srgb, var(--guaxi-border, #d0d7de) 92%, transparent);
  --mw-card-bg: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 92%, var(--guaxi-content-bg, #ffffff) 8%);
  --mw-chip-bg: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 82%, var(--guaxi-content-bg, #ffffff) 18%);
  --mw-hover-bg: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 70%, var(--guaxi-content-bg, #ffffff) 30%);
  --mw-accent: #0891b2;
  --mw-shadow: 0 10px 24px rgba(15, 23, 42, 0.08);
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 18px 16px;
  background: var(--guaxi-panel-bg);
  color: var(--mw-text);
  overflow: auto;
}

:global(:root[data-guaxi-theme='dark']) .math-ws-panel {
  --mw-text: #e8edf6;
  --mw-muted: #aeb8ca;
  --mw-border: rgba(151, 164, 186, 0.18);
  --mw-border-strong: rgba(151, 164, 186, 0.28);
  --mw-card-bg: #2a303a;
  --mw-chip-bg: #303744;
  --mw-hover-bg: #353d49;
  --mw-shadow: 0 16px 36px rgba(0, 0, 0, 0.28);
}

.math-ws-panel__header {
  flex: 0 0 auto;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--mw-border);
}

.math-ws-panel__eyebrow {
  font-size: 11px;
  text-transform: uppercase;
  color: var(--mw-muted);
}

.math-ws-panel__title {
  margin: 4px 0 0;
  font-size: 18px;
  line-height: 1.2;
  color: var(--mw-text);
}

.math-ws-panel__summary {
  display: grid;
  gap: 4px;
  justify-items: end;
  text-align: right;
  font-size: 12px;
  color: var(--mw-muted);
}

.math-ws-panel__empty {
  margin-top: 16px;
  padding: 16px;
  border-radius: 8px;
  border: 1px dashed var(--mw-border-strong);
  color: var(--mw-muted);
  background: var(--mw-card-bg);
  line-height: 1.7;
}

.math-ws-panel__empty--soft {
  margin-top: 8px;
  padding: 12px;
  font-size: 13px;
}

.math-ws-panel__section-title {
  margin-top: 18px;
  margin-bottom: 8px;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.02em;
  color: var(--mw-muted);
}

.math-ws-panel__list {
  display: grid;
  align-content: start;
  gap: 8px;
}

.math-ws-panel__card {
  display: grid;
  gap: 8px;
  padding: 11px 12px;
  border-radius: 8px;
  border: 1px solid var(--mw-border);
  background: var(--mw-card-bg);
  box-shadow: var(--mw-shadow);
}

.math-ws-panel__card--carrier {
  border-left: 3px solid color-mix(in srgb, var(--mw-accent) 60%, var(--mw-border-strong));
}

.math-ws-panel__card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.math-ws-panel__identity {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--mw-text);
}

.math-ws-panel__name {
  font-weight: 700;
  font-size: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.math-ws-panel__type {
  font-size: 12px;
  color: var(--mw-muted);
}

.math-ws-panel__tool-btn {
  position: relative;
  width: 28px;
  height: 28px;
  flex: 0 0 auto;
  border: 1px solid var(--mw-border);
  border-radius: 8px;
  background: transparent;
  color: var(--mw-muted);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 0.18s ease, border-color 0.18s ease, transform 0.18s ease;
}

.math-ws-panel__tool-btn:hover {
  background: var(--mw-hover-bg);
  border-color: var(--mw-border-strong);
  color: var(--mw-accent);
  transform: translateY(-1px);
}

.math-ws-panel__tool-btn::after {
  content: attr(data-tooltip);
  position: absolute;
  right: 0;
  top: calc(100% + 8px);
  z-index: 30;
  width: max-content;
  max-width: min(240px, calc(100vw - 48px));
  padding: 6px 8px;
  border-radius: 7px;
  background: color-mix(in srgb, #111827 92%, transparent);
  color: #fff;
  font-size: 12px;
  line-height: 1.35;
  white-space: normal;
  opacity: 0;
  pointer-events: none;
  transform: translateY(-4px);
  transition: opacity 0.14s ease, transform 0.14s ease;
}

.math-ws-panel__tool-btn:hover::after {
  opacity: 1;
  transform: translateY(0);
}

.math-ws-panel__meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--mw-muted);
}

.math-ws-panel__chip {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 8px;
  border-radius: 999px;
  background: var(--mw-chip-bg);
  font-family: var(--guaxi-notebook-mono-font, monospace);
  font-size: 11px;
}
</style>
