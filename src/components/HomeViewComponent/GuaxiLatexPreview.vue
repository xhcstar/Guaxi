<template>
  <div class="latex-preview" :class="`theme-${theme}`">
    <div class="latex-preview__toolbar">
      <span class="latex-preview__title">{{ titleLabel }} 预览</span>
      <span class="latex-preview__status" :class="statusTone">{{ statusText }}</span>
      <div class="latex-preview__spacer" />

      <!-- 搜索 -->
      <div v-if="hasPdf && !errorText" class="latex-preview__search">
        <input
          ref="searchInputRef"
          v-model="searchQuery"
          class="latex-preview__search-input"
          type="text"
          placeholder="在 PDF 中搜索"
          @keydown.enter.prevent="runSearch"
        />
        <span v-if="searchHits.length" class="latex-preview__search-count">
          {{ searchActiveIdx + 1 }}/{{ searchHits.length }}
        </span>
        <button
          type="button"
          class="latex-preview__btn"
          title="上一个匹配"
          :disabled="!searchHits.length"
          @click="gotoSearchHit(searchActiveIdx - 1)"
        >‹</button>
        <button
          type="button"
          class="latex-preview__btn"
          title="下一个匹配"
          :disabled="!searchHits.length"
          @click="gotoSearchHit(searchActiveIdx + 1)"
        >›</button>
      </div>

      <span class="latex-preview__divider" />

      <span class="latex-preview__page-indicator">{{ currentPage }} / {{ totalPages }}</span>

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

    <div class="latex-preview__body" ref="bodyRef" @scroll="onScroll">
      <div v-if="errorText" class="latex-preview__error">
        <div class="latex-preview__error-title">构建失败</div>
        <pre class="latex-preview__error-log">{{ errorText }}</pre>
      </div>
      <div v-else-if="!hasPdf && !building" class="latex-preview__empty">
        暂无预览，开始编辑或点击 ⟳ 构建
      </div>

      <!-- 连续滚动：每页一个占位容器，可见页才渲染图片/文本层 -->
      <div v-show="hasPdf && !errorText" class="latex-preview__pages">
        <div
          v-for="page in pages"
          :key="page.num"
          :ref="(el) => setPageRef(page.num, el)"
          class="latex-preview__page"
          :data-page="page.num"
          :style="{
            width: `${Math.round(page.wPt * cssScale)}px`,
            height: `${Math.round(page.hPt * cssScale)}px`,
          }"
          @mousedown="onPageMouseDown(page.num, $event)"
        >
          <img
            v-if="page.image"
            class="latex-preview__page-img"
            :src="page.image"
            :width="Math.round(page.wPt * cssScale)"
            :height="Math.round(page.hPt * cssScale)"
            draggable="false"
            alt="预览页"
          />
          <div v-else class="latex-preview__page-skeleton">{{ page.num }}</div>

          <!-- 透明文本层：可选择/复制 -->
          <div v-if="page.segments" class="latex-preview__textlayer">
            <span
              v-for="(seg, i) in page.segments"
              :key="i"
              class="latex-preview__text-seg"
              :style="segStyle(seg)"
            >{{ seg.text }}</span>
          </div>

          <!-- 搜索高亮 -->
          <div
            v-for="(rect, i) in (searchRectsByPage[page.num] || [])"
            :key="`s${i}`"
            class="latex-preview__search-rect"
            :class="{ active: rect.active }"
            :style="rectStyle(rect)"
          />

          <!-- 正向同步定位标记 -->
          <div
            v-if="flashMarker && flashMarker.page === page.num"
            class="latex-preview__flash"
            :style="{ top: `${flashMarker.y * cssScale}px` }"
          />
        </div>
      </div>

      <div v-if="building" class="latex-preview__overlay">
        <span class="latex-preview__spinner" />
        <span>正在构建…</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, computed, watch, nextTick, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../../utils/logger'

const logger = createLogger('GuaxiLatexPreview')

const props = defineProps({
  // 已构建好的 PDF 临时文件路径
  pdfPath: { type: String, default: '' },
  // 源文件路径（.tex / .typ）：双向定位需要
  sourcePath: { type: String, default: '' },
  building: { type: Boolean, default: false },
  errorText: { type: String, default: '' },
  theme: { type: String, default: 'light' },
  // 'LaTeX' | 'Typst'
  titleLabel: { type: String, default: 'LaTeX' },
})

const emit = defineEmits(['export', 'rebuild', 'reverse-jump'])

const MIN_ZOOM = 0.5
const MAX_ZOOM = 3
const zoom = ref(1.3)
// CSS 像素/pt：直接等于 zoom（A4 595pt × 1.3 ≈ 773px，舒适）
const cssScale = computed(() => zoom.value)
// 渲染 DPI：随 zoom 提升后端真重渲染，保证清晰（含 2x 余量）
const renderDpi = computed(() => Math.min(480, Math.max(96, Math.round(zoom.value * 144))))

const bodyRef = ref(null)
const searchInputRef = ref(null)
const hasPdf = ref(false)
const totalPages = ref(0)
const currentPage = ref(1)

// 每页：{ num, wPt, hPt, image, dpi, segments, rendering, textLoading }
const pages = reactive([])
const pageEls = new Map()
const setPageRef = (num, el) => {
  if (el) pageEls.set(num, el)
  else pageEls.delete(num)
}

const statusText = ref('等待构建')
const statusTone = ref('')

// 当前 PDF 路径（去掉 query）
let activePath = ''
let loadToken = 0

const engine = computed(() => (/typst/i.test(props.titleLabel) ? 'typst' : 'latex'))

const zoomIn = () => { zoom.value = Math.min(MAX_ZOOM, +(zoom.value + 0.1).toFixed(2)) }
const zoomOut = () => { zoom.value = Math.max(MIN_ZOOM, +(zoom.value - 0.1).toFixed(2)) }

// ───────────────────────── 打开 PDF ─────────────────────────
const loadPdf = async (fp) => {
  const path = String(fp || '').split('?')[0]
  const token = ++loadToken
  // 记住旧滚动比例，重建后尽量回到原处
  const prevRatio = scrollRatio()

  if (!path) {
    resetState()
    return
  }
  activePath = path
  statusText.value = '加载预览…'
  statusTone.value = ''
  try {
    const resp = await invoke('pdf_open', { path })
    if (token !== loadToken) return
    pages.splice(0, pages.length)
    resp.pages.forEach((p, i) => {
      pages.push({
        num: i + 1,
        wPt: p.w_pt,
        hPt: p.h_pt,
        image: '',
        dpi: 0,
        segments: null,
        rendering: false,
        textLoading: false,
      })
    })
    totalPages.value = resp.total_pages
    hasPdf.value = resp.total_pages > 0
    statusText.value = `就绪 · ${resp.total_pages} 页`
    statusTone.value = 'ok'
    await nextTick()
    restoreScroll(prevRatio)
    updateVisible()
    // PDF 重建后刷新搜索（命中坐标可能变化）
    if (searchQuery.value.trim()) runSearch()
  } catch (err) {
    if (token !== loadToken) return
    logger.error('打开 PDF 失败', { error: String(err?.message || err) })
    statusText.value = '加载失败'
    statusTone.value = 'error'
  }
}

const resetState = () => {
  pages.splice(0, pages.length)
  pageEls.clear()
  hasPdf.value = false
  totalPages.value = 0
  currentPage.value = 1
  activePath = ''
  statusText.value = '等待构建'
  statusTone.value = ''
  searchHits.value = []
  searchRectsByPage.value = {}
}

const scrollRatio = () => {
  const el = bodyRef.value
  if (!el || el.scrollHeight <= el.clientHeight) return 0
  return el.scrollTop / (el.scrollHeight - el.clientHeight)
}
const restoreScroll = (ratio) => {
  const el = bodyRef.value
  if (!el || !ratio) return
  el.scrollTop = ratio * (el.scrollHeight - el.clientHeight)
}

// ─────────────────── 虚拟化：渲染可见页 ───────────────────
let scrollRaf = 0
const onScroll = () => {
  if (scrollRaf) return
  scrollRaf = requestAnimationFrame(() => {
    scrollRaf = 0
    updateVisible()
  })
}

const updateVisible = () => {
  const el = bodyRef.value
  if (!el || !pages.length) return
  const top = el.scrollTop
  const bottom = top + el.clientHeight
  const BUFFER = el.clientHeight // 预渲染上下各一屏

  let firstVisible = currentPage.value
  let bestOverlap = 0

  for (const page of pages) {
    const node = pageEls.get(page.num)
    if (!node) continue
    const nTop = node.offsetTop
    const nBottom = nTop + node.offsetHeight
    const inRange = nBottom >= top - BUFFER && nTop <= bottom + BUFFER
    if (inRange) {
      renderPage(page)
      loadText(page)
    } else if (page.image && Math.abs(page.num - currentPage.value) > 6) {
      // 远离视口的页释放图片，控制内存
      page.image = ''
      page.dpi = 0
    }
    // 计算当前页（与视口重叠最多者）
    const overlap = Math.min(nBottom, bottom) - Math.max(nTop, top)
    if (overlap > bestOverlap) {
      bestOverlap = overlap
      firstVisible = page.num
    }
  }
  if (firstVisible !== currentPage.value) currentPage.value = firstVisible
}

const renderPage = async (page) => {
  const dpi = renderDpi.value
  if (page.rendering) return
  if (page.image && page.dpi === dpi) return
  page.rendering = true
  try {
    const resp = await invoke('pdf_render_page', {
      request: { path: activePath, page: page.num, dpi },
    })
    page.image = `data:image/png;base64,${resp.image_data}`
    page.dpi = resp.dpi
  } catch (err) {
    logger.error('渲染页失败', { page: page.num, error: String(err?.message || err) })
  } finally {
    page.rendering = false
  }
}

const loadText = async (page) => {
  if (page.segments || page.textLoading) return
  page.textLoading = true
  try {
    const resp = await invoke('pdf_page_text', { path: activePath, page: page.num })
    page.segments = resp.segments || []
  } catch (err) {
    page.segments = []
  } finally {
    page.textLoading = false
  }
}

// 文本片段定位（pt -> css px）
const segStyle = (seg) => {
  const s = cssScale.value
  return {
    left: `${seg.x * s}px`,
    top: `${seg.y * s}px`,
    width: `${seg.w * s}px`,
    height: `${seg.h * s}px`,
    fontSize: `${Math.max(6, seg.h * s * 0.92)}px`,
    lineHeight: `${seg.h * s}px`,
  }
}
const rectStyle = (rect) => {
  const s = cssScale.value
  return {
    left: `${rect.x * s}px`,
    top: `${rect.y * s}px`,
    width: `${rect.w * s}px`,
    height: `${rect.h * s}px`,
  }
}

// 缩放后重渲可见页（提高清晰度）
watch(renderDpi, () => {
  if (!hasPdf.value) return
  for (const page of pages) {
    if (page.image && page.dpi !== renderDpi.value) renderPage(page)
  }
})

// ───────────────────────── 搜索 ─────────────────────────
const searchQuery = ref('')
const searchHits = ref([]) // [{ page, rects:[{x,y,w,h}], snippet }]
const searchActiveIdx = ref(0)
const searchRectsByPage = ref({}) // page -> [{x,y,w,h,active}]

const runSearch = async () => {
  const q = searchQuery.value.trim()
  if (!q || !activePath) {
    searchHits.value = []
    searchRectsByPage.value = {}
    return
  }
  try {
    const resp = await invoke('pdf_search', { path: activePath, query: q })
    searchHits.value = resp.hits || []
    searchActiveIdx.value = 0
    rebuildSearchRects()
    if (searchHits.value.length) gotoSearchHit(0)
    statusText.value = `搜索：${resp.total} 个匹配`
    statusTone.value = resp.total ? 'ok' : ''
  } catch (err) {
    logger.error('PDF 搜索失败', { error: String(err?.message || err) })
  }
}

const rebuildSearchRects = () => {
  const byPage = {}
  searchHits.value.forEach((hit, idx) => {
    if (!byPage[hit.page]) byPage[hit.page] = []
    hit.rects.forEach((r) => {
      byPage[hit.page].push({ ...r, active: idx === searchActiveIdx.value })
    })
  })
  searchRectsByPage.value = byPage
}

const gotoSearchHit = (idx) => {
  if (!searchHits.value.length) return
  const n = searchHits.value.length
  const i = ((idx % n) + n) % n
  searchActiveIdx.value = i
  rebuildSearchRects()
  const hit = searchHits.value[i]
  const rect = hit.rects[0]
  if (rect) scrollToPagePoint(hit.page, rect.y)
}

// ─────────────────── 反向：点击 PDF -> 源码 ───────────────────
let dragStartedAt = null
const onPageMouseDown = (pageNum, ev) => {
  dragStartedAt = { x: ev.clientX, y: ev.clientY, page: pageNum }
}

const handlePageClick = async (pageNum, ev) => {
  const node = pageEls.get(pageNum)
  if (!node) return
  const rect = node.getBoundingClientRect()
  const xPt = (ev.clientX - rect.left) / cssScale.value
  const yPt = (ev.clientY - rect.top) / cssScale.value
  try {
    let res
    if (engine.value === 'typst') {
      res = await invoke('typst_inverse_search', {
        sourcePath: props.sourcePath,
        page: pageNum,
        x: xPt,
        y: yPt,
      })
    } else {
      res = await invoke('synctex_inverse', {
        pdfPath: activePath,
        page: pageNum,
        x: xPt,
        y: yPt,
      })
    }
    if (res && res.line) {
      emit('reverse-jump', {
        line: res.line,
        column: res.column || 1,
        file: res.file || '',
      })
    }
  } catch (err) {
    logger.warn('反向定位失败', { error: String(err?.message || err) })
  }
}

// 区分"点击定位"与"拖拽选择文本"：位移小才算点击
const onBodyMouseUp = (ev) => {
  if (!dragStartedAt) return
  const moved = Math.abs(ev.clientX - dragStartedAt.x) + Math.abs(ev.clientY - dragStartedAt.y)
  const sel = window.getSelection?.()
  const hasSelection = sel && String(sel).length > 0
  if (moved < 4 && !hasSelection) {
    handlePageClick(dragStartedAt.page, ev)
  }
  dragStartedAt = null
}

// ─────────────────── 正向：源码 -> PDF（供父组件调用）───────────────────
const flashMarker = ref(null) // { page, y }
let flashTimer = null

const scrollToPagePoint = (pageNum, yPt) => {
  const el = bodyRef.value
  const node = pageEls.get(pageNum)
  if (!el || !node) return
  const target = node.offsetTop + yPt * cssScale.value - el.clientHeight * 0.35
  el.scrollTo({ top: Math.max(0, target), behavior: 'smooth' })
}

const forwardSync = async ({ line, column = 1 } = {}) => {
  if (!hasPdf.value || !line) return
  try {
    let res
    if (engine.value === 'typst') {
      res = await invoke('typst_forward_search', {
        sourcePath: props.sourcePath,
        line,
        column,
      })
    } else {
      res = await invoke('synctex_forward', {
        pdfPath: activePath,
        sourcePath: props.sourcePath,
        line,
        column,
      })
    }
    if (res && res.page) {
      scrollToPagePoint(res.page, res.y)
      flashMarker.value = { page: res.page, y: res.y }
      if (flashTimer) clearTimeout(flashTimer)
      flashTimer = setTimeout(() => { flashMarker.value = null }, 1600)
    }
  } catch (err) {
    logger.warn('正向定位失败', { error: String(err?.message || err) })
  }
}

defineExpose({ forwardSync })

// ───────────────────────── watchers ─────────────────────────
watch(() => props.pdfPath, (fp) => { loadPdf(fp) })

watch(() => props.building, (b) => {
  if (b) { statusText.value = '构建中…'; statusTone.value = '' }
})

watch(() => props.errorText, (e) => {
  if (e) { statusText.value = '构建失败'; statusTone.value = 'error' }
})

if (typeof window !== 'undefined') {
  window.addEventListener('mouseup', onBodyMouseUp, true)
}

onBeforeUnmount(() => {
  loadToken += 1
  if (flashTimer) clearTimeout(flashTimer)
  if (scrollRaf) cancelAnimationFrame(scrollRaf)
  if (typeof window !== 'undefined') {
    window.removeEventListener('mouseup', onBodyMouseUp, true)
  }
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

.latex-preview__title { font-weight: 600; color: var(--lp-text); }
.latex-preview__status { font-size: 11px; opacity: 0.7; }
.latex-preview__status.ok { color: #2da44e; opacity: 1; }
.latex-preview.theme-dark .latex-preview__status.ok { color: #98c379; }
.latex-preview__status.error { color: #cf222e; opacity: 1; }
.latex-preview.theme-dark .latex-preview__status.error { color: #e06c75; }
.latex-preview__spacer { flex: 1 1 auto; }

.latex-preview__divider { width: 1px; height: 16px; background: var(--lp-border); margin: 0 2px; }

.latex-preview__search { display: flex; align-items: center; gap: 4px; }
.latex-preview__search-input {
  width: 130px;
  font-size: 12px;
  padding: 2px 6px;
  border: 1px solid var(--lp-border);
  border-radius: 6px;
  background: transparent;
  color: var(--lp-text);
}
.latex-preview__search-input:focus { outline: none; border-color: var(--n-color-primary, #61afef); }
.latex-preview__search-count { font-size: 11px; opacity: 0.7; min-width: 32px; text-align: center; }

.latex-preview__page-indicator { font-size: 11px; opacity: 0.75; min-width: 48px; text-align: center; }
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
.latex-preview__btn:hover:not(:disabled) { background: var(--lp-hover-bg); border-color: var(--n-color-primary, #61afef); }
.latex-preview__btn:disabled { opacity: 0.4; cursor: not-allowed; }
.latex-preview__btn--primary { border-color: var(--n-color-primary, #61afef); color: var(--n-color-primary, #61afef); }

.latex-preview__body {
  flex: 1 1 auto;
  min-height: 0;
  position: relative;
  overflow: auto;
  background: var(--lp-canvas-area);
}

.latex-preview__pages {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 12px;
}

.latex-preview__page {
  position: relative;
  background: #fff;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.25);
  flex: 0 0 auto;
}

.latex-preview__page-img { display: block; user-select: none; }

.latex-preview__page-skeleton {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #bbb;
  font-size: 24px;
  background: repeating-linear-gradient(45deg, #fafafa, #fafafa 10px, #f0f0f0 10px, #f0f0f0 20px);
}

/* 透明文本层：文字本身透明，仅用于选择/复制 */
.latex-preview__textlayer {
  position: absolute;
  inset: 0;
  overflow: hidden;
  line-height: 1;
}
.latex-preview__text-seg {
  position: absolute;
  color: transparent;
  white-space: pre;
  transform-origin: 0 0;
  cursor: text;
  user-select: text;
}
.latex-preview__text-seg::selection { background: rgba(97, 175, 239, 0.4); }

.latex-preview__search-rect {
  position: absolute;
  background: rgba(255, 213, 0, 0.35);
  border: 1px solid rgba(255, 170, 0, 0.6);
  pointer-events: none;
  border-radius: 1px;
}
.latex-preview__search-rect.active {
  background: rgba(255, 145, 0, 0.5);
  border-color: rgba(255, 120, 0, 0.9);
}

.latex-preview__flash {
  position: absolute;
  left: 0;
  right: 0;
  height: 0;
  border-top: 2px solid var(--n-color-primary, #61afef);
  box-shadow: 0 0 8px 2px rgba(97, 175, 239, 0.6);
  pointer-events: none;
  animation: latex-preview-flash 1.6s ease-out forwards;
}
@keyframes latex-preview-flash {
  0% { opacity: 1; }
  70% { opacity: 0.8; }
  100% { opacity: 0; }
}

.latex-preview__empty,
.latex-preview__error {
  margin: auto;
  max-width: 90%;
  text-align: center;
  color: var(--lp-text);
  padding: 24px;
}
.latex-preview__error { text-align: left; width: 100%; }
.latex-preview__error-title { color: #cf222e; font-weight: 600; margin-bottom: 8px; }
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
@keyframes latex-preview-spin { to { transform: rotate(360deg); } }
</style>
