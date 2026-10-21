<template>
  <div class="markdown-manual-window" :data-theme="normalizedTheme" :style="rootStyle">
    <div class="markdown-manual-header">
      <div class="markdown-manual-title">Markdown语法查询手册</div>
      <div class="markdown-manual-subtitle">左侧渲染预览（支持 Mermaid / MathJax），右侧原文（MarkdownTest.md）</div>
    </div>

    <div class="markdown-manual-body">
      <div class="markdown-manual-pane markdown-manual-preview" :style="previewPaneStyle">
        <div v-if="loading" class="markdown-manual-loading">正在渲染...</div>
        <div v-else ref="previewContainerRef" class="markdown-manual-card-list">
          <div
            v-for="card in cards"
            :key="card.id"
            class="markdown-manual-card markdown-manual-card-preview"
            :class="{ active: activeCardId === card.id }"
            :data-card-id="card.id"
            @click="selectCard(card.id)"
          >
            <div class="markdown-manual-card-body" v-html="card.html"></div>
          </div>
        </div>
      </div>

      <div class="markdown-manual-resize-handle" @mousedown="startResize" @dblclick="resetSplit"></div>

      <div class="markdown-manual-pane markdown-manual-source">
        <div ref="sourceContainerRef" class="markdown-manual-card-list">
          <div
            v-for="card in cards"
            :key="card.id"
            class="markdown-manual-card markdown-manual-card-source"
            :class="{ active: activeCardId === card.id }"
            :data-card-id="card.id"
            @click="selectCard(card.id)"
          >
            <button
              class="markdown-manual-card-copy"
              type="button"
              :class="{ copied: copiedCardId === card.id }"
              :title="copiedCardId === card.id ? '已复制' : '复制源码'"
              @click.stop="copyCard(card)"
            >
              <Icon
                :icon="copiedCardId === card.id ? mdiCheck : mdiContentCopy"
                width="16"
                height="16"
              />
            </button>
            <pre class="markdown-manual-card-code">{{ card.raw }}</pre>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Icon } from '@iconify/vue'
import mdiCheck from '@iconify-icons/mdi/check'
import mdiContentCopy from '@iconify-icons/mdi/content-copy'
import { renderMarkdown } from '../GuaxiMarkdownRender/GuaxiMarker.js'
import {
  renderChartEmbeds,
  disposeChartInstances,
  bindExcalidrawOpenButtons,
} from '../GuaxiMarkdownRender/guaxiChartEmbeds'
import markdownManualRaw from '../../assets/MarkdownTest.md?raw'
import { useSettingsStore } from '../../stores/settingsStore'

const logger = createLogger('GuaxiMarkdownManual')
const props = defineProps({
  theme: {
    type: String,
    default: ''
  }
})

const settingsStore = useSettingsStore()

const normalizedTheme = computed(() => {
  const fromProps = String(props.theme || '').trim().toLowerCase()
  if (fromProps === 'dark' || fromProps === 'light') return fromProps

  const fromStore = String(settingsStore.appearanceTheme || '').trim().toLowerCase()
  if (fromStore === 'dark' || fromStore === 'light') return fromStore

  return 'light'
})

const rootStyle = computed(() => {
  const base = Number(settingsStore.appearanceFontSize) || 14
  const sourceSize = Math.max(12, base)
  const previewSize = Math.max(12, base + 3)

  return {
    '--markdown-manual-source-font-size': `${sourceSize}px`,
    '--markdown-manual-preview-font-size': `${previewSize}px`,
  }
})

const markdownSource = ref(String(markdownManualRaw || ''))
const loading = ref(false)

const previewContainerRef = ref(null)
const sourceContainerRef = ref(null)

const cards = ref([])
const activeCardId = ref('')
const copiedCardId = ref('')

const previewWidthPx = ref(0)
const resizing = ref(false)
const resizeState = ref({
  startX: 0,
  startWidth: 0,
})

const clampPreviewWidth = (v) => {
  const min = 320
  const max = Math.max(min, window.innerWidth - 320)
  return Math.min(max, Math.max(min, v))
}

const previewPaneStyle = computed(() => {
  if (!previewWidthPx.value) return {}
  return {
    flex: '0 0 auto',
    width: `${previewWidthPx.value}px`,
  }
})

const onResizeMove = (e) => {
  if (!resizing.value) return
  const dx = e.clientX - resizeState.value.startX
  previewWidthPx.value = clampPreviewWidth(resizeState.value.startWidth + dx)
}

const stopResize = () => {
  if (!resizing.value) return
  resizing.value = false
  try {
    document.body.style.cursor = ''
    document.body.style.userSelect = ''
  } catch {
  }
  window.removeEventListener('mousemove', onResizeMove)
  window.removeEventListener('mouseup', stopResize)
  logger.info('[MarkdownManual] resize end:', { previewWidthPx: previewWidthPx.value })
}

const startResize = (e) => {
  e.preventDefault()
  resizing.value = true
  resizeState.value = {
    startX: e.clientX,
    startWidth: previewWidthPx.value || Math.round(window.innerWidth * 0.5),
  }
  try {
    document.body.style.cursor = 'col-resize'
    document.body.style.userSelect = 'none'
  } catch {
  }
  window.addEventListener('mousemove', onResizeMove)
  window.addEventListener('mouseup', stopResize)
  logger.info('[MarkdownManual] resize start:', resizeState.value)
}

const resetSplit = () => {
  previewWidthPx.value = clampPreviewWidth(Math.round(window.innerWidth * 0.5))
  logger.info('[MarkdownManual] resize reset:', { previewWidthPx: previewWidthPx.value })
}

const scrollToCard = (containerRef, cardId, { behavior = 'smooth' } = {}) => {
  const container = containerRef.value
  if (!container || !cardId) return
  const el = container.querySelector(`[data-card-id="${cardId}"]`)
  if (!el) return
  try {
    el.scrollIntoView({ block: 'center', behavior })
  } catch {
    container.scrollTop = Math.max(0, el.offsetTop - container.clientHeight * 0.2)
  }
}

const selectCard = (cardId) => {
  const id = String(cardId || '')
  if (!id) return
  activeCardId.value = id
  scrollToCard(previewContainerRef, id, { behavior: 'smooth' })
  scrollToCard(sourceContainerRef, id, { behavior: 'smooth' })
  logger.info('[MarkdownManual] select card:', { cardId: id })
}

const copyCard = async (card) => {
  const raw = String(card?.raw || '')
  if (!raw) return

  try {
    const { writeText } = await import('@tauri-apps/plugin-clipboard-manager')
    await writeText(raw)
  } catch (e) {
    try {
      await navigator.clipboard.writeText(raw)
    } catch (e2) {
      logger.error('[MarkdownManual] copy failed:', { e, e2 })
      return
    }
  }

  copiedCardId.value = String(card?.id || '')
  logger.info('[MarkdownManual] copied:', { cardId: copiedCardId.value, len: raw.length })
  window.setTimeout(() => {
    if (copiedCardId.value === String(card?.id || '')) copiedCardId.value = ''
  }, 1200)
}

const renderMermaidIn = async (rootEl) => {
  const root = rootEl instanceof Element ? rootEl : null
  if (!root) return

  const nodes = Array.from(root.querySelectorAll('.mermaid')).filter((el) => !el.querySelector('svg'))
  if (!nodes.length) return

  nodes.forEach((el) => {
    try {
      el.removeAttribute('data-processed')
    } catch {
    }
  })

  try {
    const { default: mermaid } = await import('mermaid')
    try {
      mermaid.initialize({ startOnLoad: false, theme: 'default', securityLevel: 'loose' })
    } catch {
    }
    await mermaid.run({ nodes })
    logger.info('[MarkdownManual] mermaid rendered:', { nodes: nodes.length })
  } catch (e) {
    logger.warn('[MarkdownManual] mermaid render failed:', e)
  }
}

/**
 * 渲染 `chart` / `excalidraw` 围栏。
 *
 * 渲染器只吐出占位 div，「正在渲染图表…」必须由 renderChartEmbeds 在 DOM 挂载后接管，
 * 否则手册里的图表示例永远停在占位状态。与编辑器预览用同一套入口，行为保持一致。
 */
const renderEmbedsIn = (rootEl) => {
  const root = rootEl instanceof Element ? rootEl : null
  if (!root) return

  try {
    renderChartEmbeds(root)
    bindExcalidrawOpenButtons(root, {
      onOpen: (ref) => {
        // 手册窗口没有白板页上下文，只把路径记到日志；不伪造一个并不存在的跳转契约。
        logger.info('[MarkdownManual] excalidraw open requested', { ref })
      },
    })
  } catch (e) {
    logger.warn('[MarkdownManual] chart embeds failed:', e)
  }
}

const parseCards = (src) => {
  const lines = String(src || '').split(/\r?\n|\r/)
  const idxs = []
  for (let i = 0; i < lines.length; i++) {
    if (/^##\s+/.test(lines[i] || '')) idxs.push(i)
  }
  if (!idxs.length || idxs[0] !== 0) idxs.unshift(0)

  const out = []
  for (let k = 0; k < idxs.length; k++) {
    const s = idxs[k]
    const e = k + 1 < idxs.length ? idxs[k + 1] : lines.length
    const raw = lines.slice(s, e).join('\n')
    if (!raw.trim()) continue
    out.push({ id: `${s + 1}-${e}`, startLine: s + 1, endLine: e, raw, html: '' })
  }

  logger.info('[MarkdownManual] parsed cards:', { cards: out.length })
  return out
}

const doRender = async () => {
  loading.value = true
  // 主题切换会整批重建卡片：旧图表实例必须先销毁，否则 ECharts 实例挂在被丢弃的
  // DOM 上，ResizeObserver 也跟着泄漏。
  try {
    disposeChartInstances(previewContainerRef.value)
  } catch {
    /* 首次渲染时容器还不存在 */
  }
  try {
    logger.info('[MarkdownManual] cards render start:', {
      theme: normalizedTheme.value,
      len: markdownSource.value.length,
      fontSize: settingsStore.appearanceFontSize,
    })

    const nextCards = parseCards(markdownSource.value)
    cards.value = nextCards

    for (const c of cards.value) {
      c.html = await renderMarkdown(c.raw, {
        preserveOriginal: true,
        mermaid: true,
        math: true,
        codeHighlight: true,
        theme: normalizedTheme.value,
        basePath: ''
      })
    }

    if (!activeCardId.value && cards.value.length) activeCardId.value = cards.value[0].id

    logger.info('[MarkdownManual] cards render done:', { cards: cards.value.length })
  } catch (e) {
    logger.error('[MarkdownManual] cards render failed:', e)
    cards.value = []
  } finally {
    loading.value = false
  }

  await nextTick()
  await renderMermaidIn(previewContainerRef.value)
  await renderEmbedsIn(previewContainerRef.value)
  await nextTick()

  if (activeCardId.value) {
    scrollToCard(previewContainerRef, activeCardId.value, { behavior: 'auto' })
    scrollToCard(sourceContainerRef, activeCardId.value, { behavior: 'auto' })
  }
}

onMounted(async () => {
  document.title = 'Markdown语法查询手册'

  try {
    await settingsStore.loadAppearanceConfig()
  } catch (e) {
    logger.warn('[MarkdownManual] loadAppearanceConfig failed:', e)
  }

  previewWidthPx.value = clampPreviewWidth(Math.round(window.innerWidth * 0.5))
  window.addEventListener('resize', resetSplit)

  logger.info('[MarkdownManual] mounted:', {
    theme: normalizedTheme.value,
    previewWidthPx: previewWidthPx.value,
    fontSize: settingsStore.appearanceFontSize,
  })

  await doRender()
})

onBeforeUnmount(() => {
  stopResize()
  window.removeEventListener('resize', resetSplit)
  try {
    disposeChartInstances(previewContainerRef.value)
  } catch {
    /* 已卸载 */
  }
})

watch(normalizedTheme, async () => {
  await doRender()
})
</script>

<style scoped>
.markdown-manual-window {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--guaxi-app-bg);
  color: var(--guaxi-text-primary);
}

.markdown-manual-header {
  padding: 12px 16px;
  background: var(--guaxi-header-bg);
  border-bottom: 1px solid var(--guaxi-border);
}

.markdown-manual-title {
  font-size: 16px;
  font-weight: 600;
  line-height: 1.2;
}

.markdown-manual-subtitle {
  margin-top: 6px;
  font-size: 12px;
  opacity: 0.7;
}

.markdown-manual-body {
  flex: 1;
  min-height: 0;
  display: flex;
}

.markdown-manual-pane {
  flex: 1;
  min-width: 0;
  overflow: auto;
  background: var(--guaxi-content-bg);
}

.markdown-manual-preview {
  background: var(--guaxi-content-bg);
}

.markdown-manual-source {
  background: var(--guaxi-panel-bg);
}

.markdown-manual-resize-handle {
  width: 6px;
  cursor: col-resize;
  background: transparent;
  position: relative;
}

.markdown-manual-resize-handle::before {
  content: '';
  position: absolute;
  inset: 0;
  margin: 0 auto;
  width: 1px;
  background: var(--guaxi-border);
}

.markdown-manual-resize-handle:hover {
  background: rgba(127, 127, 127, 0.12);
}

.markdown-manual-card-list {
  height: 100%;
  overflow: auto;
  padding: 10px;
  box-sizing: border-box;
}

.markdown-manual-card {
  position: relative;
  margin: 0 0 10px;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  background: var(--guaxi-panel-bg);
  box-sizing: border-box;
}

.markdown-manual-card.active {
  box-shadow: 0 0 0 2px rgba(0, 173, 255, 0.35);
}

.markdown-manual-card-preview .markdown-manual-card-body {
  padding: 12px;
  font-size: var(--markdown-manual-preview-font-size, 17px);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  line-height: 1.5;
}

.markdown-manual-card-preview .markdown-manual-card-body :deep(a),
.markdown-manual-card-preview .markdown-manual-card-body :deep(a:visited) {
  color: var(--guaxi-text-link, var(--primary-color, #0969da));
  text-decoration: underline;
  text-underline-offset: 2px;
}

.dark-mode .markdown-manual-card-preview .markdown-manual-card-body :deep(a),
.dark-mode .markdown-manual-card-preview .markdown-manual-card-body :deep(a:visited),
.dark .markdown-manual-card-preview .markdown-manual-card-body :deep(a),
.dark .markdown-manual-card-preview .markdown-manual-card-body :deep(a:visited),
:root[data-guaxi-theme='dark'] .markdown-manual-card-preview .markdown-manual-card-body :deep(a),
:root[data-guaxi-theme='dark'] .markdown-manual-card-preview .markdown-manual-card-body :deep(a:visited) {
  color: var(--guaxi-text-link-dark, #4acadb);
}

.markdown-manual-card-preview .markdown-manual-card-body :deep(a:hover) {
  opacity: 0.92;
}

.markdown-manual-card-preview .markdown-manual-card-body :deep(code) {
  font-family: inherit !important;
}

.markdown-manual-card-source {
  padding-top: 8px;
}

.markdown-manual-card-copy {
  position: absolute;
  top: 8px;
  right: 8px;
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  cursor: pointer;
}

.markdown-manual-card-copy {
  transition: background 0.15s ease, border-color 0.15s ease, color 0.15s ease, transform 0.1s ease;
}

.markdown-manual-card-copy:hover {
  opacity: 0.92;
}

.markdown-manual-card-copy.copied {
  background: rgba(0, 173, 255, 0.14);
  border-color: rgba(0, 173, 255, 0.55);
  color: rgba(0, 173, 255, 0.95);
  transform: scale(1.02);
}

.markdown-manual-card-code {
  margin: 0;
  padding: 30px 12px 12px;
  color: var(--guaxi-text-primary);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  font-size: var(--markdown-manual-source-font-size, 14px);
  line-height: 1.6;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
  overflow: auto;
}

.markdown-manual-loading {
  padding: 16px;
  opacity: 0.75;
}
</style>