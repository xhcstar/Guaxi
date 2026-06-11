<template>
  <div class="latex-preview">
    <div class="latex-preview__toolbar">
      <span class="latex-preview__title">LaTeX 预览</span>
      <span class="latex-preview__status" :class="statusTone">{{ statusText }}</span>
      <div class="latex-preview__spacer" />
      <button
        type="button"
        class="latex-preview__btn"
        :disabled="zoom <= MIN_ZOOM"
        title="缩小"
        @click="zoomOut"
      >−</button>
      <span class="latex-preview__zoom">{{ Math.round(zoom * 100) }}%</span>
      <button
        type="button"
        class="latex-preview__btn"
        :disabled="zoom >= MAX_ZOOM"
        title="放大"
        @click="zoomIn"
      >＋</button>
      <button
        type="button"
        class="latex-preview__btn"
        title="重新构建"
        @click="$emit('rebuild')"
      >⟳</button>
      <button
        type="button"
        class="latex-preview__btn latex-preview__btn--primary"
        :disabled="!hasPdf"
        title="导出 PDF"
        @click="$emit('export')"
      >导出</button>
    </div>

    <div class="latex-preview__body" ref="bodyRef">
      <div v-if="errorText" class="latex-preview__error">
        <div class="latex-preview__error-title">构建失败</div>
        <pre class="latex-preview__error-log">{{ errorText }}</pre>
      </div>
      <div v-else-if="!hasPdf && !building" class="latex-preview__empty">
        暂无预览，开始编辑或点击 ⟳ 构建
      </div>
      <div v-show="hasPdf && !errorText" ref="pagesRef" class="latex-preview__pages"></div>

      <div v-if="building" class="latex-preview__overlay">
        <span class="latex-preview__spinner" />
        <span>正在构建…</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, watch, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import * as pdfjsLib from 'pdfjs-dist/build/pdf.mjs'
import { createLogger } from '../../utils/logger'

pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
  'pdfjs-dist/build/pdf.worker.min.mjs',
  import.meta.url
).toString()

const logger = createLogger('GuaxiLatexPreview')

const props = defineProps({
  // 已构建好的 PDF 临时文件路径
  pdfPath: { type: String, default: '' },
  // 是否正在构建
  building: { type: Boolean, default: false },
  // 构建错误日志（非空则展示错误，不渲染 PDF）
  errorText: { type: String, default: '' },
})

defineEmits(['export', 'rebuild'])

const MIN_ZOOM = 0.5
const MAX_ZOOM = 3
const zoom = ref(1.1)

const bodyRef = ref(null)
const pagesRef = ref(null)
const hasPdf = ref(false)

let pdfDoc = null
let renderToken = 0

const statusText = ref('等待构建')
const statusTone = ref('')

const zoomIn = () => { zoom.value = Math.min(MAX_ZOOM, +(zoom.value + 0.1).toFixed(2)) }
const zoomOut = () => { zoom.value = Math.max(MIN_ZOOM, +(zoom.value - 0.1).toFixed(2)) }

const decodeBase64ToBytes = (b64) => {
  const bin = window.atob(String(b64 || ''))
  const out = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i += 1) out[i] = bin.charCodeAt(i)
  return out
}

const clearPages = () => {
  if (pagesRef.value) pagesRef.value.innerHTML = ''
}

const renderAllPages = async () => {
  const token = ++renderToken
  if (!pdfDoc || !pagesRef.value) return
  // 记录滚动比例，重渲染（缩放）后尽量保持位置
  const bodyEl = bodyRef.value
  const prevRatio = bodyEl && bodyEl.scrollHeight > 0
    ? bodyEl.scrollTop / bodyEl.scrollHeight
    : 0

  clearPages()
  const dpr = Math.max(1, Number(window.devicePixelRatio) || 1)

  for (let pageNum = 1; pageNum <= pdfDoc.numPages; pageNum += 1) {
    if (token !== renderToken) return
    const page = await pdfDoc.getPage(pageNum)
    const viewport = page.getViewport({ scale: zoom.value })

    const canvas = document.createElement('canvas')
    canvas.className = 'latex-preview__canvas'
    const ctx = canvas.getContext('2d', { alpha: false })
    canvas.width = Math.max(1, Math.floor(viewport.width * dpr))
    canvas.height = Math.max(1, Math.floor(viewport.height * dpr))
    canvas.style.width = `${Math.floor(viewport.width)}px`
    canvas.style.height = `${Math.floor(viewport.height)}px`
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)

    if (token !== renderToken) return
    pagesRef.value.appendChild(canvas)
    try {
      await page.render({ canvasContext: ctx, viewport }).promise
    } catch (err) {
      if (token !== renderToken) return
      logger.warn('渲染 PDF 页面失败', { pageNum, error: String(err?.message || err) })
    }
  }

  if (token === renderToken && bodyEl) {
    bodyEl.scrollTop = prevRatio * bodyEl.scrollHeight
  }
}

const loadPdf = async (fp) => {
  const path = String(fp || '')
  if (!path) {
    hasPdf.value = false
    clearPages()
    statusText.value = '等待构建'
    statusTone.value = ''
    return
  }
  try {
    statusText.value = '加载预览…'
    statusTone.value = ''
    const b64 = await invoke('read_file_bytes_base64', { path })
    const bytes = decodeBase64ToBytes(b64)
    const task = pdfjsLib.getDocument({ data: bytes })
    const next = await task.promise
    try { pdfDoc?.destroy?.() } catch { /* noop */ }
    pdfDoc = next
    hasPdf.value = true
    await renderAllPages()
    statusText.value = `就绪 · ${pdfDoc.numPages} 页`
    statusTone.value = 'ok'
  } catch (err) {
    logger.error('加载预览 PDF 失败', { error: String(err?.message || err) })
    hasPdf.value = false
    statusText.value = '加载失败'
    statusTone.value = 'error'
  }
}

watch(() => props.pdfPath, (fp) => { loadPdf(fp) })

watch(() => props.building, (b) => {
  if (b) { statusText.value = '构建中…'; statusTone.value = '' }
})

watch(() => props.errorText, (e) => {
  if (e) { statusText.value = '构建失败'; statusTone.value = 'error' }
})

watch(zoom, () => { if (hasPdf.value) renderAllPages() })

onBeforeUnmount(() => {
  renderToken += 1
  try { pdfDoc?.destroy?.() } catch { /* noop */ }
  pdfDoc = null
})
</script>

<style scoped>
.latex-preview {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  min-height: 0;
  background: var(--guaxi-content-bg, #1e1e1e);
  color: var(--guaxi-text-primary, #ddd);
}

.latex-preview__toolbar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  border-bottom: 1px solid var(--guaxi-border, #3e4451);
  background: var(--guaxi-header-bg, #252525);
  font-size: 12px;
}

.latex-preview__title {
  font-weight: 600;
  color: var(--guaxi-text-primary, #ddd);
}

.latex-preview__status { font-size: 11px; opacity: 0.7; }
.latex-preview__status.ok { color: #98c379; opacity: 1; }
.latex-preview__status.error { color: #e06c75; opacity: 1; }

.latex-preview__spacer { flex: 1 1 auto; }

.latex-preview__zoom { font-size: 11px; opacity: 0.75; min-width: 38px; text-align: center; }

.latex-preview__btn {
  border: 1px solid var(--guaxi-border, #3e4451);
  background: transparent;
  color: var(--guaxi-text-primary, #ddd);
  border-radius: 6px;
  padding: 2px 8px;
  font-size: 12px;
  cursor: pointer;
  transition: background-color 0.15s, border-color 0.15s;
}
.latex-preview__btn:hover:not(:disabled) {
  background: var(--guaxi-hover-bg, rgba(255, 255, 255, 0.08));
  border-color: var(--n-color-primary, #61afef);
}
.latex-preview__btn:disabled { opacity: 0.4; cursor: not-allowed; }
.latex-preview__btn--primary {
  border-color: var(--n-color-primary, #61afef);
  color: var(--n-color-primary, #61afef);
}

.latex-preview__body {
  flex: 1 1 auto;
  min-height: 0;
  position: relative;
  overflow: auto;
  background: #525659;
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 12px;
  gap: 12px;
}

.latex-preview__pages {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}

:deep(.latex-preview__canvas) {
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.4);
  background: #fff;
}

.latex-preview__empty,
.latex-preview__error {
  margin: auto;
  max-width: 90%;
  text-align: center;
  color: #ddd;
}
.latex-preview__error { text-align: left; width: 100%; }
.latex-preview__error-title {
  color: #e06c75;
  font-weight: 600;
  margin-bottom: 8px;
}
.latex-preview__error-log {
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 11.5px;
  line-height: 1.45;
  background: rgba(0, 0, 0, 0.35);
  border: 1px solid rgba(224, 108, 117, 0.4);
  border-radius: 8px;
  padding: 10px;
  max-height: 100%;
  overflow: auto;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

.latex-preview__overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  background: rgba(0, 0, 0, 0.35);
  color: #fff;
  font-size: 13px;
  backdrop-filter: blur(1px);
}
.latex-preview__spinner {
  width: 16px;
  height: 16px;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-top-color: #fff;
  border-radius: 50%;
  animation: latex-preview-spin 0.8s linear infinite;
}
@keyframes latex-preview-spin {
  to { transform: rotate(360deg); }
}
</style>
