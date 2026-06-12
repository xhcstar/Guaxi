<template>
  <div class="latex-preview" :class="`theme-${theme}`">
    <div class="latex-preview__toolbar">
      <span class="latex-preview__title">{{ titleLabel }} 预览</span>
      <span class="latex-preview__status" :class="statusTone">{{ statusText }}</span>
      <div class="latex-preview__spacer" />

      <!-- 分页导航 -->
      <div v-if="hasPdf && !errorText" class="latex-preview__pager">
        <button
          type="button"
          class="latex-preview__btn"
          :disabled="currentPage <= 1"
          title="上一页"
          @click="goPrevPage"
        >‹</button>
        <input
          class="latex-preview__page-input"
          type="text"
          :value="pageInputText"
          @focus="onPageInputFocus"
          @change="onPageInputCommit"
          @keydown.enter.prevent="onPageInputCommit"
        />
        <span class="latex-preview__page-total">/ {{ totalPages }}</span>
        <button
          type="button"
          class="latex-preview__btn"
          :disabled="currentPage >= totalPages"
          title="下一页"
          @click="goNextPage"
        >›</button>
      </div>

      <span class="latex-preview__divider" />

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
      <div v-show="hasPdf && !errorText" class="latex-preview__page-wrap">
        <img
          v-if="pageImage"
          class="latex-preview__page-img"
          :src="pageImage"
          :style="{ width: `${Math.round(zoom * BASE_WIDTH)}px` }"
          alt="LaTeX 预览页"
        />
      </div>

      <div v-if="building || pageLoading" class="latex-preview__overlay">
        <span class="latex-preview__spinner" />
        <span>{{ building ? '正在构建…' : '渲染中…' }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, watch, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../../utils/logger'

const logger = createLogger('GuaxiLatexPreview')

const props = defineProps({
  // 已构建好的 PDF 临时文件路径
  pdfPath: { type: String, default: '' },
  // 是否正在构建
  building: { type: Boolean, default: false },
  // 构建错误日志（非空则展示错误，不渲染 PDF）
  errorText: { type: String, default: '' },
  // 主题：'light' | 'dark'
  theme: { type: String, default: 'light' },
  // 标题前缀（'LaTeX' / 'Typst'）
  titleLabel: { type: String, default: 'LaTeX' },
})

defineEmits(['export', 'rebuild'])

const MIN_ZOOM = 0.5
const MAX_ZOOM = 3
const zoom = ref(1.1)
// 缩放仅改变显示宽度（即时、不重新请求后端）；后端固定按整页渲染（target_width=1200）
const BASE_WIDTH = 560

const bodyRef = ref(null)
const hasPdf = ref(false)
const pageImage = ref('')
const pageLoading = ref(false)

const currentPage = ref(1)
const totalPages = ref(0)
const pageInputText = ref('1')

// 当前 PDF 路径（去掉外部传入的 query）；用于按页渲染
let activePath = ''
let renderToken = 0

const statusText = ref('等待构建')
const statusTone = ref('')

const zoomIn = () => { zoom.value = Math.min(MAX_ZOOM, +(zoom.value + 0.1).toFixed(2)) }
const zoomOut = () => { zoom.value = Math.max(MIN_ZOOM, +(zoom.value - 0.1).toFixed(2)) }

// 用后端 pdfium 渲染指定页 -> base64 PNG（pdfium 能正确渲染中文等 CJK 嵌入字体）
const renderPage = async (pageNum) => {
  if (!activePath) return
  const token = ++renderToken
  pageLoading.value = true
  try {
    const raw = await invoke('get_converted_pdf_page', {
      request: {
        filepath: activePath,
        curpage: pageNum,
        lastpage: 0,
        // 不传 scale：后端 render_image 已用 set_target_width(1200) 渲染整页，
        // 再叠加 scale 会把内容放大到超出位图宽度导致右侧被裁切。
        mode: 'image',
      },
    })
    if (token !== renderToken) return
    const resp = typeof raw === 'string' ? JSON.parse(raw) : raw
    if (resp?.error) throw new Error(resp.error)
    if (Number.isFinite(resp?.total_pages)) totalPages.value = resp.total_pages
    if (resp?.image_data) {
      pageImage.value = `data:image/png;base64,${resp.image_data}`
    }
    if (bodyRef.value) bodyRef.value.scrollTop = 0
    statusText.value = `就绪 · ${totalPages.value} 页`
    statusTone.value = 'ok'
  } catch (err) {
    if (token !== renderToken) return
    logger.error('渲染预览页失败', { pageNum, error: String(err?.message || err) })
    statusText.value = '渲染失败'
    statusTone.value = 'error'
  } finally {
    if (token === renderToken) pageLoading.value = false
  }
}

// 仅获取总页数并渲染首页（加载新 PDF 时）
const loadPdf = async (fp) => {
  const path = String(fp || '').split('?')[0]
  if (!path) {
    hasPdf.value = false
    pageImage.value = ''
    totalPages.value = 0
    activePath = ''
    statusText.value = '等待构建'
    statusTone.value = ''
    return
  }
  activePath = path
  statusText.value = '加载预览…'
  statusTone.value = ''
  // 重建后尽量停留在原页码（renderPage 会用最新 total_pages 修正）
  const targetPage = Math.max(1, currentPage.value)
  await renderPage(targetPage)
  if (statusTone.value !== 'error') {
    hasPdf.value = true
    // 修正越界页码
    const clamped = Math.min(Math.max(1, targetPage), totalPages.value || 1)
    if (clamped !== currentPage.value) {
      currentPage.value = clamped
      pageInputText.value = String(clamped)
      await renderPage(clamped)
    } else {
      pageInputText.value = String(currentPage.value)
    }
  }
}

const setCurrentPage = (target) => {
  const clamped = Math.min(Math.max(1, Math.round(target)), totalPages.value || 1)
  pageInputText.value = String(clamped)
  if (clamped === currentPage.value) return
  currentPage.value = clamped
  renderPage(clamped)
}

const goPrevPage = () => setCurrentPage(currentPage.value - 1)
const goNextPage = () => setCurrentPage(currentPage.value + 1)

const onPageInputFocus = (event) => {
  try { event.target.select() } catch { /* noop */ }
}

const onPageInputCommit = (event) => {
  const raw = Number(String(event?.target?.value || '').trim())
  if (!Number.isFinite(raw)) {
    pageInputText.value = String(currentPage.value)
    return
  }
  setCurrentPage(raw)
}

watch(() => props.pdfPath, (fp) => { loadPdf(fp) })

watch(() => props.building, (b) => {
  if (b) { statusText.value = '构建中…'; statusTone.value = '' }
})

watch(() => props.errorText, (e) => {
  if (e) { statusText.value = '构建失败'; statusTone.value = 'error' }
})

onBeforeUnmount(() => {
  renderToken += 1
})
</script>

<style scoped>
.latex-preview {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  min-height: 0;
  background: var(--lp-bg);
  color: var(--lp-text);
}

/* 主题变量：暗色用深背景，亮色用浅背景 */
.latex-preview.theme-dark {
  --lp-bg: #1e1e1e;
  --lp-text: #ddd;
  --lp-toolbar-bg: #252525;
  --lp-border: #3e4451;
  --lp-canvas-area: #525659;
  --lp-btn-text: #ddd;
  --lp-hover-bg: rgba(255, 255, 255, 0.08);
  --lp-error-bg: rgba(0, 0, 0, 0.35);
  --lp-overlay-bg: rgba(0, 0, 0, 0.45);
  --lp-overlay-text: #fff;
}
.latex-preview.theme-light {
  --lp-bg: #ffffff;
  --lp-text: #1f2328;
  --lp-toolbar-bg: #f5f6f8;
  --lp-border: #d0d7de;
  --lp-canvas-area: #eef0f3;
  --lp-btn-text: #1f2328;
  --lp-hover-bg: rgba(0, 0, 0, 0.06);
  --lp-error-bg: rgba(0, 0, 0, 0.04);
  --lp-overlay-bg: rgba(255, 255, 255, 0.7);
  --lp-overlay-text: #1f2328;
}

.latex-preview__toolbar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  border-bottom: 1px solid var(--lp-border);
  background: var(--lp-toolbar-bg);
  font-size: 12px;
}

.latex-preview__title {
  font-weight: 600;
  color: var(--lp-text);
}

.latex-preview__status { font-size: 11px; opacity: 0.7; }
.latex-preview__status.ok { color: #2da44e; opacity: 1; }
.latex-preview.theme-dark .latex-preview__status.ok { color: #98c379; }
.latex-preview__status.error { color: #cf222e; opacity: 1; }
.latex-preview.theme-dark .latex-preview__status.error { color: #e06c75; }

.latex-preview__spacer { flex: 1 1 auto; }

.latex-preview__divider {
  width: 1px;
  height: 16px;
  background: var(--lp-border);
  margin: 0 2px;
}

.latex-preview__pager {
  display: flex;
  align-items: center;
  gap: 4px;
}
.latex-preview__page-input {
  width: 40px;
  text-align: center;
  font-size: 12px;
  padding: 2px 4px;
  border: 1px solid var(--lp-border);
  border-radius: 6px;
  background: transparent;
  color: var(--lp-text);
}
.latex-preview__page-input:focus {
  outline: none;
  border-color: var(--n-color-primary, #61afef);
}
.latex-preview__page-total { font-size: 11px; opacity: 0.7; }

.latex-preview__zoom { font-size: 11px; opacity: 0.75; min-width: 38px; text-align: center; }

.latex-preview__btn {
  border: 1px solid var(--lp-border);
  background: transparent;
  color: var(--lp-btn-text);
  border-radius: 6px;
  padding: 2px 8px;
  font-size: 12px;
  cursor: pointer;
  transition: background-color 0.15s, border-color 0.15s;
}
.latex-preview__btn:hover:not(:disabled) {
  background: var(--lp-hover-bg);
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
  background: var(--lp-canvas-area);
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 12px;
  gap: 12px;
}

.latex-preview__page-wrap {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.latex-preview__page-img {
  display: block;
  max-width: none;
  height: auto;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.25);
  background: #fff;
}

.latex-preview__empty,
.latex-preview__error {
  margin: auto;
  max-width: 90%;
  text-align: center;
  color: var(--lp-text);
}
.latex-preview__error { text-align: left; width: 100%; }
.latex-preview__error-title {
  color: #cf222e;
  font-weight: 600;
  margin-bottom: 8px;
}
.latex-preview.theme-dark .latex-preview__error-title { color: #e06c75; }
.latex-preview__error-log {
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 11.5px;
  line-height: 1.45;
  background: var(--lp-error-bg);
  border: 1px solid rgba(207, 34, 46, 0.35);
  border-radius: 8px;
  padding: 10px;
  max-height: 100%;
  overflow: auto;
  color: var(--lp-text);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

.latex-preview__overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  background: var(--lp-overlay-bg);
  color: var(--lp-overlay-text);
  font-size: 13px;
  backdrop-filter: blur(1px);
}
.latex-preview__spinner {
  width: 16px;
  height: 16px;
  border: 2px solid rgba(127, 127, 127, 0.3);
  border-top-color: var(--n-color-primary, #61afef);
  border-radius: 50%;
  animation: latex-preview-spin 0.8s linear infinite;
}
@keyframes latex-preview-spin {
  to { transform: rotate(360deg); }
}
</style>
