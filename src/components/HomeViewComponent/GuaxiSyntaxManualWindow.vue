<template>
  <div class="syntax-manual-window" :data-theme="normalizedTheme">
    <div class="syntax-manual-window__header">
      <div>
        <div class="syntax-manual-window__title">{{ manualTitle }}</div>
        <div class="syntax-manual-window__subtitle">
          左侧源码（点击行号定位），右侧使用 {{ engineLabel }} 引擎渲染的 PDF 预览
        </div>
      </div>
      <div class="syntax-manual-window__actions">
        <n-button size="small" secondary @click="copySource">复制示例</n-button>
        <n-button size="small" type="primary" :loading="previewBuilding" @click="buildManualPreview">重新渲染</n-button>
      </div>
    </div>

    <div class="syntax-manual-window__meta">
      <span class="syntax-manual-window__badge">{{ engineLabel }}</span>
      <span class="syntax-manual-window__path" :title="sourcePath">{{ sourcePath || `未传入 ${extensionLabel} 文件路径` }}</span>
    </div>

    <n-alert v-if="sourcePathMissing" type="warning" class="syntax-manual-window__alert">
      请从已打开的 {{ extensionLabel }} 文件中进入手册。手册会借用该文件所在目录调用内置渲染引擎。
    </n-alert>

    <div class="syntax-manual-window__body">
      <section class="syntax-manual-window__source">
        <div class="syntax-manual-window__panel-title">可渲染完整示例源码</div>
        <div class="syntax-manual-window__code" role="list">
          <button
            v-for="(line, index) in sourceLines"
            :key="`${normalizedLanguage}-${index}`"
            type="button"
            class="syntax-manual-window__line"
            :class="{ active: activeSourceLine === index + 1 }"
            role="listitem"
            @click="handleSourceLineClick(index + 1)"
          >
            <span class="syntax-manual-window__line-number">{{ index + 1 }}</span>
            <code class="syntax-manual-window__line-code">{{ line || ' ' }}</code>
          </button>
        </div>
      </section>

      <div class="syntax-manual-window__resize" @mousedown="startResize" @dblclick="resetSplit"></div>

      <section class="syntax-manual-window__preview">
        <GuaxiPdfViewer
          ref="pdfViewerRef"
          :path="previewRealPdfPath"
          :source-path="sourcePath"
          :building="previewBuilding"
          :error-text="previewError"
          :theme="normalizedTheme"
          :engine="engineKind"
          :title-label="`${engineLabel} 手册预览`"
          empty-text="暂无预览，点击重新渲染生成示例 PDF"
          :initial-zoom="1.08"
          :features="previewFeatures"
          @rebuild="buildManualPreview"
        />
      </section>
    </div>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { NAlert, NButton, useMessage } from 'naive-ui'
import GuaxiPdfViewer from '../PdfViewer/GuaxiPdfViewer.vue'
import { createLogger } from '../../utils/logger'
import typstManualSample from '../../assets/syntax-manual/typst-manual.typ?raw'
import latexManualSample from '../../assets/syntax-manual/latex-manual.tex?raw'

const logger = createLogger('GuaxiSyntaxManualWindow')
const message = useMessage()

const props = defineProps({
  language: { type: String, default: 'typst' },
  sourcePath: { type: String, default: '' },
  theme: { type: String, default: 'light' },
})

const normalizedLanguage = computed(() => (
  String(props.language || '').trim().toLowerCase() === 'latex' ? 'latex' : 'typst'
))
const normalizedTheme = computed(() => (
  String(props.theme || '').trim().toLowerCase() === 'dark' ? 'dark' : 'light'
))
const sourcePath = computed(() => String(props.sourcePath || '').trim())
const sourcePathMissing = computed(() => !sourcePath.value)
const engineKind = computed(() => (normalizedLanguage.value === 'typst' ? 'typst' : 'latex'))
const engineLabel = computed(() => (normalizedLanguage.value === 'typst' ? 'Typst' : 'LaTeX'))
const extensionLabel = computed(() => (normalizedLanguage.value === 'typst' ? '.typ' : '.tex'))
const manualTitle = computed(() => `${engineLabel.value} 语法查询手册`)

const previewPdfPath = ref('')
const previewBuilding = ref(false)
const previewError = ref('')
const pdfViewerRef = ref(null)
const activeSourceLine = ref(0)
const sourceRatio = ref(0.48)
let previewSeq = 0
let resizeState = null

const sampleSource = computed(() => (
  normalizedLanguage.value === 'typst' ? typstManualSample : latexManualSample
))
const sourceLines = computed(() => String(sampleSource.value || '').split(/\r?\n/))
const previewRealPdfPath = computed(() => String(previewPdfPath.value || '').split('?')[0])

const previewFeatures = {
  search: true,
  reverseJump: false,
  outline: false,
  export: false,
  rebuild: true,
}

const buildManualPreview = async () => {
  if (sourcePathMissing.value) return
  const seq = ++previewSeq
  const command = normalizedLanguage.value === 'typst'
    ? 'build_typ_pdf_preview'
    : 'build_tex_pdf_preview'

  previewBuilding.value = true
  previewError.value = ''
  logger.info('[SyntaxManualWindow] build sample preview start', {
    language: normalizedLanguage.value,
    sourcePath: sourcePath.value,
    command,
    sampleLength: sampleSource.value.length,
  })

  try {
    const pdfPath = await invoke(command, {
      sourcePath: sourcePath.value,
      content: sampleSource.value,
    })
    if (seq !== previewSeq) return
    previewPdfPath.value = `${pdfPath}?manual=${seq}`
    logger.info('[SyntaxManualWindow] build sample preview success', {
      language: normalizedLanguage.value,
      pdfPath,
      seq,
    })
  } catch (error) {
    if (seq !== previewSeq) return
    previewError.value = String(error?.message || error || '渲染失败')
    logger.error('[SyntaxManualWindow] build sample preview failed', {
      language: normalizedLanguage.value,
      sourcePath: sourcePath.value,
      error: previewError.value,
    })
  } finally {
    if (seq === previewSeq) previewBuilding.value = false
  }
}

const handleSourceLineClick = async (line) => {
  activeSourceLine.value = line
  logger.info('[SyntaxManualWindow] source line selected for preview sync', {
    language: normalizedLanguage.value,
    sourcePath: sourcePath.value,
    line,
  })
  try {
    await pdfViewerRef.value?.forwardSync?.({ line, column: 1 })
  } catch (error) {
    logger.warn('[SyntaxManualWindow] preview forward sync failed', {
      language: normalizedLanguage.value,
      line,
      error: String(error?.message || error),
    })
  }
}

const copySource = async () => {
  try {
    await navigator.clipboard.writeText(sampleSource.value)
    logger.info('[SyntaxManualWindow] sample source copied', {
      language: normalizedLanguage.value,
      length: sampleSource.value.length,
    })
    message.success('示例源码已复制')
  } catch (error) {
    logger.warn('[SyntaxManualWindow] copy sample source failed', {
      language: normalizedLanguage.value,
      error: String(error?.message || error),
    })
    message.error('复制失败')
  }
}

const startResize = (event) => {
  resizeState = {
    startX: Number(event?.clientX || 0),
    startRatio: sourceRatio.value,
    width: Number(event?.currentTarget?.parentElement?.getBoundingClientRect?.().width || window.innerWidth),
  }
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
  window.addEventListener('mousemove', handleResizeMove, true)
  window.addEventListener('mouseup', stopResize, true)
}

const handleResizeMove = (event) => {
  if (!resizeState) return
  const delta = Number(event?.clientX || 0) - resizeState.startX
  const next = resizeState.startRatio + delta / Math.max(1, resizeState.width)
  sourceRatio.value = Math.min(0.72, Math.max(0.28, next))
}

const stopResize = () => {
  resizeState = null
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
  window.removeEventListener('mousemove', handleResizeMove, true)
  window.removeEventListener('mouseup', stopResize, true)
}

const resetSplit = () => {
  sourceRatio.value = 0.48
}

watch(
  () => [normalizedLanguage.value, sourcePath.value],
  () => {
    previewPdfPath.value = ''
    previewError.value = ''
    activeSourceLine.value = 0
    buildManualPreview()
  },
  { immediate: true }
)

onBeforeUnmount(() => {
  stopResize()
})
</script>

<style scoped>
.syntax-manual-window {
  --syntax-source-ratio: v-bind(sourceRatio);
  display: flex;
  flex-direction: column;
  height: 100vh;
  min-width: 0;
  min-height: 0;
  box-sizing: border-box;
  padding: 12px;
  background: var(--guaxi-app-bg);
  color: var(--guaxi-text-primary);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', system-ui;
}

.syntax-manual-window__header {
  display: flex;
  flex: 0 0 auto;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding-bottom: 10px;
}

.syntax-manual-window__title {
  font-size: 20px;
  font-weight: 800;
  line-height: 1.25;
}

.syntax-manual-window__subtitle {
  margin-top: 4px;
  color: var(--guaxi-text-secondary);
  font-size: 12px;
}

.syntax-manual-window__actions {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 8px;
}

.syntax-manual-window__meta {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  min-width: 0;
  gap: 10px;
  padding-bottom: 10px;
}

.syntax-manual-window__badge {
  flex: 0 0 auto;
  padding: 3px 8px;
  border: 1px solid rgba(37, 99, 235, 0.28);
  border-radius: 999px;
  background: rgba(37, 99, 235, 0.08);
  color: #1d4ed8;
  font-size: 12px;
  font-weight: 700;
}

.syntax-manual-window__path {
  min-width: 0;
  overflow: hidden;
  color: var(--guaxi-text-secondary);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.syntax-manual-window__alert {
  flex: 0 0 auto;
  margin-bottom: 10px;
}

.syntax-manual-window__body {
  display: grid;
  grid-template-columns: minmax(320px, calc((100% - 10px) * var(--syntax-source-ratio))) 10px minmax(420px, 1fr);
  flex: 1 1 auto;
  min-height: 0;
}

.syntax-manual-window__source,
.syntax-manual-window__preview {
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-panel-bg);
}

.syntax-manual-window__source {
  display: flex;
  flex-direction: column;
}

.syntax-manual-window__panel-title {
  flex: 0 0 auto;
  padding: 9px 12px;
  border-bottom: 1px solid var(--guaxi-border);
  font-weight: 700;
}

.syntax-manual-window__code {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  padding: 8px 0;
  background: rgba(15, 23, 42, 0.04);
  font-size: 12px;
  line-height: 1.62;
  tab-size: 2;
}

.syntax-manual-window__line {
  display: grid;
  grid-template-columns: 58px minmax(0, 1fr);
  width: 100%;
  border: 0;
  padding: 0;
  background: transparent;
  color: inherit;
  cursor: pointer;
  font: inherit;
  text-align: left;
}

.syntax-manual-window__line:hover,
.syntax-manual-window__line.active {
  background: rgba(37, 99, 235, 0.1);
}

.syntax-manual-window__line-number {
  position: sticky;
  left: 0;
  z-index: 1;
  padding: 0 10px;
  border-right: 1px solid rgba(148, 163, 184, 0.22);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 92%, transparent);
  color: var(--guaxi-text-secondary);
  text-align: right;
  user-select: none;
}

.syntax-manual-window__line-code {
  min-width: 0;
  padding: 0 12px;
  white-space: pre;
}

.syntax-manual-window__resize {
  position: relative;
  cursor: col-resize;
}

.syntax-manual-window__resize::before {
  position: absolute;
  top: 0;
  bottom: 0;
  left: 4px;
  width: 2px;
  border-radius: 999px;
  background: var(--guaxi-border);
  content: '';
}

.syntax-manual-window__preview {
  display: flex;
}

.syntax-manual-window__preview :deep(.gpv) {
  width: 100%;
  height: 100%;
}

:global(:root[data-guaxi-theme='dark']) .syntax-manual-window__badge,
.syntax-manual-window[data-theme='dark'] .syntax-manual-window__badge {
  border-color: rgba(125, 211, 252, 0.34);
  background: rgba(14, 165, 233, 0.14);
  color: #bae6fd;
}

:global(:root[data-guaxi-theme='dark']) .syntax-manual-window__code,
.syntax-manual-window[data-theme='dark'] .syntax-manual-window__code {
  background: rgba(2, 6, 23, 0.28);
}

@media (max-width: 960px) {
  .syntax-manual-window__header {
    flex-direction: column;
  }

  .syntax-manual-window__actions {
    justify-content: flex-end;
    width: 100%;
  }

  .syntax-manual-window__body {
    grid-template-columns: 1fr;
    grid-template-rows: minmax(240px, 45%) 10px minmax(320px, 1fr);
  }

  .syntax-manual-window__resize {
    cursor: row-resize;
  }
}
</style>
