<template>
  <div class="syntax-manual-window" :data-theme="normalizedTheme">
    <div class="syntax-manual-window__header">
      <div>
        <div class="syntax-manual-window__title">{{ manualTitle }}</div>
      </div>
      <div class="syntax-manual-window__actions">
        <n-button size="small" secondary @click="copySource">复制源码</n-button>
        <n-button size="small" secondary @click="resetEditableSource">恢复示例</n-button>
        <n-button size="small" type="primary" :loading="previewBuilding" @click="buildManualPreview">重新渲染</n-button>
      </div>
    </div>

    <n-alert v-if="sourcePathMissing" type="warning" class="syntax-manual-window__alert">
      请从已打开的 {{ extensionLabel }} 文件中进入手册。手册会借用该文件所在目录调用内置渲染引擎。
    </n-alert>

    <div class="syntax-manual-window__body">
      <section class="syntax-manual-window__source">
        <div class="syntax-manual-window__panel-title">可编辑完整示例源码</div>
        <div class="syntax-manual-window__code" @wheel="handleCodeWheel">
          <div
            class="syntax-manual-window__line-gutter"
            role="list"
            @wheel.prevent.stop="handleGutterWheel"
          >
            <div class="syntax-manual-window__line-gutter-content" :style="lineGutterStyle">
              <button
                v-for="(_, index) in sourceLines"
                :key="`${normalizedLanguage}-${index}`"
                type="button"
                class="syntax-manual-window__line-number"
                :class="{ active: activeSourceLine === index + 1 }"
                role="listitem"
                @click="handleSourceLineClick(index + 1)"
              >
                {{ index + 1 }}
              </button>
            </div>
          </div>
          <textarea
            ref="sourceEditorRef"
            v-model="editableSource"
            class="syntax-manual-window__editor"
            wrap="off"
            spellcheck="false"
            :aria-label="`${engineLabel} 手册示例源码`"
            @input="handleSourceInput"
            @scroll="handleSourceScroll"
            @blur="handleSourceBlur"
          ></textarea>
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
          @reverse-jump="handlePreviewReverseJump"
        />
      </section>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
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
const editableSource = ref('')
const pdfViewerRef = ref(null)
const sourceEditorRef = ref(null)
const activeSourceLine = ref(0)
const sourceRatio = ref(0.48)
const sourceScrollTop = ref(0)
let previewSeq = 0
let resizeState = null

const sampleSource = computed(() => (
  normalizedLanguage.value === 'typst' ? typstManualSample : latexManualSample
))
const sourceLines = computed(() => String(editableSource.value || '').split(/\r?\n/))
const previewRealPdfPath = computed(() => String(previewPdfPath.value || '').split('?')[0])
const lineGutterStyle = computed(() => ({
  transform: `translateY(-${sourceScrollTop.value}px)`,
}))

const previewFeatures = {
  search: true,
  reverseJump: true,
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
  const content = editableSource.value

  previewBuilding.value = true
  previewError.value = ''
  // 左侧源码现在允许编辑，渲染时必须使用当前编辑区内容，而不是内置原始模板。
  logger.info('[SyntaxManualWindow] build sample preview start', {
    language: normalizedLanguage.value,
    sourcePath: sourcePath.value,
    command,
    sampleLength: content.length,
    lineCount: sourceLines.value.length,
  })

  try {
    const pdfPath = await invoke(command, {
      sourcePath: sourcePath.value,
      content,
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
  await focusEditorLine(line)
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

const focusEditorLine = async (line, { scroll = false } = {}) => {
  await nextTick()
  const editor = sourceEditorRef.value
  if (!editor) return
  const normalized = String(editableSource.value || '').replace(/\r\n/g, '\n')
  const lines = normalized.split('\n')
  const targetIndex = Math.max(0, Math.min(lines.length - 1, Number(line || 1) - 1))
  const offset = lines
    .slice(0, targetIndex)
    .reduce((total, item) => total + item.length + 1, 0)
  editor.focus()
  editor.setSelectionRange(offset, offset)
  if (scroll) {
    const lineHeight = getEditorLineHeight()
    editor.scrollTop = Math.max(0, targetIndex * lineHeight - editor.clientHeight * 0.35)
    sourceScrollTop.value = Number(editor.scrollTop || 0)
  }
}

const getEditorLineHeight = () => {
  const editor = sourceEditorRef.value
  if (!editor) return 24
  const value = Number.parseFloat(window.getComputedStyle(editor).lineHeight || '24')
  return Number.isFinite(value) && value > 0 ? value : 24
}

const handlePreviewReverseJump = async ({ line, column = 1 } = {}) => {
  const targetLine = Math.max(1, Number(line || 1))
  activeSourceLine.value = targetLine
  await focusEditorLine(targetLine, { scroll: true })
  logger.info('[SyntaxManualWindow] preview reverse jump to editable source', {
    language: normalizedLanguage.value,
    line: targetLine,
    column,
  })
}

const handleSourceScroll = (event) => {
  sourceScrollTop.value = Number(event?.target?.scrollTop || 0)
}

const handleGutterWheel = (event) => {
  const editor = sourceEditorRef.value
  if (!editor) return
  editor.scrollTop += Number(event?.deltaY || 0)
  sourceScrollTop.value = Number(editor.scrollTop || 0)
}

const handleCodeWheel = (event) => {
  if (event?.target === sourceEditorRef.value) return
  const editor = sourceEditorRef.value
  if (!editor) return
  editor.scrollTop += Number(event?.deltaY || 0)
  sourceScrollTop.value = Number(editor.scrollTop || 0)
}

const handleSourceInput = () => {
  previewError.value = ''
}

const handleSourceBlur = () => {
  logger.info('[SyntaxManualWindow] editable source changed', {
    language: normalizedLanguage.value,
    length: editableSource.value.length,
    lineCount: sourceLines.value.length,
  })
}

const copySource = async () => {
  try {
    await navigator.clipboard.writeText(editableSource.value)
    logger.info('[SyntaxManualWindow] sample source copied', {
      language: normalizedLanguage.value,
      length: editableSource.value.length,
    })
    message.success('当前源码已复制')
  } catch (error) {
    logger.warn('[SyntaxManualWindow] copy sample source failed', {
      language: normalizedLanguage.value,
      error: String(error?.message || error),
    })
    message.error('复制失败')
  }
}

const resetEditableSource = () => {
  editableSource.value = sampleSource.value
  activeSourceLine.value = 0
  sourceScrollTop.value = 0
  previewError.value = ''
  nextTick(() => {
    if (sourceEditorRef.value) sourceEditorRef.value.scrollTop = 0
  })
  logger.info('[SyntaxManualWindow] editable source reset to bundled sample', {
    language: normalizedLanguage.value,
    length: editableSource.value.length,
    lineCount: sourceLines.value.length,
  })
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
    resetEditableSource()
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
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding-bottom: 8px;
}

.syntax-manual-window__title {
  font-size: 20px;
  font-weight: 800;
  line-height: 1.25;
}

.syntax-manual-window__actions {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 8px;
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
  display: grid;
  grid-template-columns: 64px minmax(0, 1fr);
  grid-template-rows: minmax(0, 1fr);
  flex: 1 1 auto;
  min-height: 0;
  overflow: hidden;
  background: rgba(15, 23, 42, 0.04);
  font-size: 16px;
  line-height: 1.7;
  tab-size: 2;
}

.syntax-manual-window__line-gutter {
  min-width: 0;
  min-height: 0;
  max-height: 100%;
  overflow: hidden;
  border-right: 1px solid rgba(148, 163, 184, 0.22);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 92%, transparent);
}

.syntax-manual-window__line-gutter-content {
  padding: 8px 0;
  will-change: transform;
}

.syntax-manual-window__line-number {
  display: block;
  width: 100%;
  height: 1.7em;
  border: 0;
  padding: 0 10px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  cursor: pointer;
  font: inherit;
  line-height: inherit;
  text-align: right;
  user-select: none;
}

.syntax-manual-window__line-number:hover,
.syntax-manual-window__line-number.active {
  background: rgba(37, 99, 235, 0.12);
  color: var(--guaxi-text-primary);
}

.syntax-manual-window__editor {
  min-width: 0;
  min-height: 0;
  max-height: 100%;
  width: 100%;
  height: 100%;
  box-sizing: border-box;
  border: 0;
  outline: 0;
  resize: none;
  overflow: auto;
  padding: 8px 12px;
  background: transparent;
  color: inherit;
  font: inherit;
  line-height: inherit;
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
