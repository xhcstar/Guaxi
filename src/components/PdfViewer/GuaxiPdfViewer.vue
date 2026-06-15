<template>
  <div class="gpv" :class="`theme-${theme}`">
    <div class="gpv__toolbar">
      <span v-if="titleLabel" class="gpv__title">{{ titleLabel }}</span>
      <span class="gpv__status" :class="statusTone">{{ status }}</span>
      <div class="gpv__spacer" />

      <!-- 搜索 -->
      <div v-if="features.search && hasPdf && !errorText" class="gpv__search">
        <input
          ref="searchInputRef"
          v-model="searchQuery"
          class="gpv__search-input"
          type="text"
          placeholder="在 PDF 中搜索"
          @keydown.enter.prevent="runSearch"
        />
        <span v-if="searchHits.length" class="gpv__search-count">
          {{ searchActiveIdx + 1 }}/{{ searchHits.length }}
        </span>
        <button type="button" class="gpv__btn" title="上一个匹配" :disabled="!searchHits.length" @click="prevHit">‹</button>
        <button type="button" class="gpv__btn" title="下一个匹配" :disabled="!searchHits.length" @click="nextHit">›</button>
      </div>

      <span class="gpv__divider" />
      <span class="gpv__page-indicator">{{ currentPage }} / {{ totalPages }}</span>
      <span class="gpv__divider" />

      <button type="button" class="gpv__btn" :disabled="zoom <= MIN_ZOOM" title="缩小" @click="zoomOut">−</button>
      <span class="gpv__zoom">{{ Math.round(zoom * 100) }}%</span>
      <button type="button" class="gpv__btn" :disabled="zoom >= MAX_ZOOM" title="放大" @click="zoomIn">＋</button>
      <button type="button" class="gpv__btn" title="适合宽度" @click="fitWidth">⤢</button>
      <button v-if="features.rebuild" type="button" class="gpv__btn" title="重新构建" @click="$emit('rebuild')">⟳</button>
      <button
        v-if="features.export"
        type="button"
        class="gpv__btn gpv__btn--primary"
        :disabled="!hasPdf"
        title="导出 PDF"
        @click="$emit('export')"
      >导出</button>
    </div>

    <div class="gpv__body" ref="bodyRef" @scroll="onScroll">
      <div v-if="errorText" class="gpv__error">
        <div class="gpv__error-title">构建失败</div>
        <pre class="gpv__error-log">{{ errorText }}</pre>
      </div>
      <div v-else-if="!hasPdf && !building" class="gpv__empty">
        {{ emptyText }}
      </div>

      <!-- 连续滚动：每页一个占位容器，可见页才渲染图片/文本层 -->
      <div v-show="hasPdf && !errorText" class="gpv__pages">
        <div
          v-for="page in pages"
          :key="page.num"
          :ref="(el) => setPageRef(page.num, el)"
          class="gpv__page"
          :data-page="page.num"
          :style="{
            width: `${Math.round(page.wPt * cssScale)}px`,
            height: `${Math.round(page.hPt * cssScale)}px`,
          }"
          @mousedown="onPageMouseDown(page.num, $event)"
        >
          <img
            v-if="page.image"
            class="gpv__page-img"
            :src="page.image"
            :width="Math.round(page.wPt * cssScale)"
            :height="Math.round(page.hPt * cssScale)"
            draggable="false"
            alt="预览页"
          />
          <div v-else class="gpv__page-skeleton">{{ page.num }}</div>

          <!-- 透明文本层：可选择/复制（按行，scaleX 对齐渲染图）-->
          <div v-if="page.lines" class="gpv__textlayer">
            <span
              v-for="(line, i) in page.lines"
              :key="i"
              class="gpv__text-seg"
              :style="lineStyle(line)"
            >{{ line.text }}</span>
          </div>

          <!-- 搜索高亮 -->
          <div
            v-for="(rect, i) in (searchRectsByPage[page.num] || [])"
            :key="`s${i}`"
            class="gpv__search-rect"
            :class="{ active: rect.active }"
            :style="rectStyle(rect)"
          />

          <!-- 已保存的高亮批注覆盖层 -->
          <template v-for="hl in (page.highlights || [])" :key="hl.annot_id">
            <div
              v-for="(rc, ri) in hl.rects"
              :key="`${hl.annot_id}-${ri}`"
              class="gpv__hl-rect"
              :style="hlStyle(rc, hl.color)"
              :title="hl.note || ''"
              @click.stop="onHighlightClick(hl, page.num)"
            />
          </template>

          <!-- 定位高亮框（完整包裹，替代旧的下划线）-->
          <div
            v-if="flashBox && flashBox.page === page.num"
            class="gpv__flash-box"
            :style="boxStyle(flashBox)"
          />
        </div>
      </div>

      <div v-if="building" class="gpv__overlay">
        <span class="gpv__spinner" />
        <span>正在构建…</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, computed, watch, nextTick, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../../utils/logger'
import usePdfiumDocument from './usePdfiumDocument'

const logger = createLogger('GuaxiPdfViewer')

const props = defineProps({
  // PDF 路径（可带 ?ts= 强制刷新查询）
  path: { type: String, default: '' },
  // 源文件路径（.tex / .typ）：双向定位需要
  sourcePath: { type: String, default: '' },
  building: { type: Boolean, default: false },
  errorText: { type: String, default: '' },
  theme: { type: String, default: 'light' },
  // '' | 'latex' | 'typst'
  engine: { type: String, default: '' },
  titleLabel: { type: String, default: '' },
  emptyText: { type: String, default: '暂无内容' },
  initialZoom: { type: Number, default: 1.3 },
  // 消费者按需开启的能力
  features: {
    type: Object,
    default: () => ({ search: true, reverseJump: true, outline: false, export: false, rebuild: false }),
  },
})

const emit = defineEmits([
  'reverse-jump',
  'export',
  'rebuild',
  'page-change',
  'outline',
  'selection',
  'ready',
  'highlight-click',
])

const MIN_ZOOM = 0.4
const MAX_ZOOM = 4
const zoom = ref(props.initialZoom)
const cssScale = computed(() => zoom.value)
// 渲染 DPI：随 zoom 提升后端真重渲染，保证清晰（含 2x 余量）
const renderDpi = computed(() => Math.min(480, Math.max(96, Math.round(zoom.value * 144))))

const doc = usePdfiumDocument()
const { pages, totalPages, hasPdf, status, statusTone } = doc

const bodyRef = ref(null)
const searchInputRef = ref(null)
const currentPage = ref(1)

const pageEls = new Map()
const setPageRef = (num, el) => {
  if (el) pageEls.set(num, el)
  else pageEls.delete(num)
}

const engineKind = computed(() => {
  if (props.engine) return props.engine
  return /typst/i.test(props.titleLabel) ? 'typst' : 'latex'
})

const zoomIn = () => { zoom.value = Math.min(MAX_ZOOM, +(zoom.value + 0.1).toFixed(2)) }
const zoomOut = () => { zoom.value = Math.max(MIN_ZOOM, +(zoom.value - 0.1).toFixed(2)) }
const setZoom = (z) => { zoom.value = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, +Number(z).toFixed(2))) }
const fitWidth = () => {
  const el = bodyRef.value
  const first = pages[0]
  if (!el || !first || !first.wPt) return
  const avail = el.clientWidth - 36
  setZoom(avail / first.wPt)
}
const fitPage = () => {
  const el = bodyRef.value
  const first = pages[0]
  if (!el || !first || !first.wPt || !first.hPt) return
  const z = Math.min((el.clientWidth - 36) / first.wPt, (el.clientHeight - 36) / first.hPt)
  setZoom(z)
}

// ───────────────────────── 打开 / 重建 ─────────────────────────
let lastBase = ''
const load = async (fp, keep) => {
  const prevRatio = keep ? scrollRatio() : 0
  const r = await doc.openDocument(fp, { keepExisting: keep })
  if (!r.ok) return
  emit('ready', { totalPages: totalPages.value })
  await nextTick()
  if (keep) restoreScroll(prevRatio)
  updateVisible()
  if (props.features.outline) emitOutline()
  if (searchQuery.value.trim()) runSearch()
}

const emitOutline = async () => {
  const list = await doc.getOutline()
  emit('outline', list)
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
  const BUFFER = el.clientHeight

  let firstVisible = currentPage.value
  let bestOverlap = 0

  for (const page of pages) {
    const node = pageEls.get(page.num)
    if (!node) continue
    const nTop = node.offsetTop
    const nBottom = nTop + node.offsetHeight
    const inRange = nBottom >= top - BUFFER && nTop <= bottom + BUFFER
    if (inRange) {
      doc.renderPage(page, renderDpi.value)
      ensureText(page)
      if (props.features.annotations) doc.loadHighlights(page)
    } else if (page.image && Math.abs(page.num - currentPage.value) > 6) {
      doc.freePage(page)
    }
    const overlap = Math.min(nBottom, bottom) - Math.max(nTop, top)
    if (overlap > bestOverlap) {
      bestOverlap = overlap
      firstVisible = page.num
    }
  }
  if (firstVisible !== currentPage.value) {
    currentPage.value = firstVisible
    emit('page-change', firstVisible)
  }
}

// 加载字符层并测量 scaleX（让透明文本宽度对齐渲染图，原生选择高亮因此连续）。
const ensureText = async (page) => {
  if (page.scaled || page.textLoading) return
  await doc.loadText(page)
  if (!page.lines || !page.lines.length) { page.scaled = true; return }
  await nextTick()
  measurePageText(page)
}

const measurePageText = (page) => {
  const node = pageEls.get(page.num)
  if (!node || !page.lines) return
  const spans = node.querySelectorAll('.gpv__text-seg')
  if (spans.length !== page.lines.length) return
  const s = cssScale.value
  page.lines.forEach((line, i) => {
    const span = spans[i]
    const natural = span.scrollWidth || span.getBoundingClientRect().width
    if (natural > 0 && line.widthPt > 0) {
      line.scaleX = +((line.widthPt * s) / natural).toFixed(4)
    }
  })
  page.scaled = true
}

const lineStyle = (line) => {
  const s = cssScale.value
  return {
    left: `${line.leftPt * s}px`,
    top: `${line.topPt * s}px`,
    height: `${line.heightPt * s}px`,
    fontSize: `${Math.max(6, line.fontSizePt * s)}px`,
    lineHeight: `${line.heightPt * s}px`,
    transform: `scaleX(${line.scaleX || 1})`,
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
const boxStyle = (box) => {
  const s = cssScale.value
  const h = Math.max(box.h * s, 6)
  return {
    left: `${box.x * s}px`,
    top: `${box.y * s}px`,
    width: `${Math.max(box.w * s, 8)}px`,
    height: `${h}px`,
  }
}
const hlStyle = (rc, color) => {
  const s = cssScale.value
  const [r, g, b, a] = Array.isArray(color) ? color : [255, 235, 59, 90]
  return {
    left: `${rc.x * s}px`,
    top: `${rc.y * s}px`,
    width: `${rc.w * s}px`,
    height: `${rc.h * s}px`,
    background: `rgba(${r}, ${g}, ${b}, ${Math.max(0, Math.min(1, a / 255))})`,
  }
}
const onHighlightClick = (hl, pageNum) => {
  emit('highlight-click', { annotId: hl.annot_id, page: pageNum, note: hl.note || '', color: hl.color })
}

// 缩放后重渲可见/已加载页（提高清晰度）；scaleX 与缩放无关，无需重新测量。
watch(renderDpi, () => {
  if (!hasPdf.value) return
  for (const page of pages) {
    if (page.image && page.dpi !== renderDpi.value) doc.renderPage(page, renderDpi.value)
  }
})

// ───────────────────────── 搜索 ─────────────────────────
const searchQuery = ref('')
const searchHits = ref([])
const searchActiveIdx = ref(0)
const searchRectsByPage = ref({})

const runSearch = async () => {
  const q = searchQuery.value.trim()
  if (!q) {
    searchHits.value = []
    searchRectsByPage.value = {}
    return
  }
  const resp = await doc.search(q)
  searchHits.value = resp.hits || []
  searchActiveIdx.value = 0
  rebuildSearchRects()
  if (searchHits.value.length) gotoSearchHit(0)
  status.value = `搜索：${resp.total} 个匹配`
  statusTone.value = resp.total ? 'ok' : ''
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
const nextHit = () => gotoSearchHit(searchActiveIdx.value + 1)
const prevHit = () => gotoSearchHit(searchActiveIdx.value - 1)
const clearSearch = () => {
  searchQuery.value = ''
  searchHits.value = []
  searchRectsByPage.value = {}
}

// ─────────────────── 反向：点击 PDF -> 源码 ───────────────────
let dragStartedAt = null
const onPageMouseDown = (pageNum, ev) => {
  dragStartedAt = { x: ev.clientX, y: ev.clientY, page: pageNum }
}

const handlePageClick = async (pageNum, ev) => {
  if (!props.features.reverseJump) return
  const node = pageEls.get(pageNum)
  if (!node) return
  const rect = node.getBoundingClientRect()
  const xPt = (ev.clientX - rect.left) / cssScale.value
  const yPt = (ev.clientY - rect.top) / cssScale.value
  try {
    let res
    if (engineKind.value === 'typst') {
      res = await invoke('typst_inverse_search', { sourcePath: props.sourcePath, page: pageNum, x: xPt, y: yPt })
    } else {
      res = await invoke('synctex_inverse', { pdfPath: doc.getActivePath(), page: pageNum, x: xPt, y: yPt })
    }
    if (res && res.line) {
      emit('reverse-jump', { line: res.line, column: res.column || 1, file: res.file || '' })
    }
  } catch (e) {
    logger.warn('反向定位失败', { error: String(e?.message || e) })
  }
}

const onBodyMouseUp = (ev) => {
  // 选择文本时同时上报，供翻译/批注等消费者使用
  const sel = window.getSelection?.()
  const selText = sel ? String(sel) : ''
  if (selText.length > 0) {
    const info = getSelectionInfo()
    if (info) emit('selection', info)
  }
  if (!dragStartedAt) return
  const moved = Math.abs(ev.clientX - dragStartedAt.x) + Math.abs(ev.clientY - dragStartedAt.y)
  if (moved < 4 && !selText) handlePageClick(dragStartedAt.page, ev)
  dragStartedAt = null
}

const getSelectionInfo = () => {
  const sel = window.getSelection?.()
  if (!sel || !sel.rangeCount) return null
  const text = String(sel).trim()
  if (!text) return null
  let node = sel.anchorNode
  let pageNum = currentPage.value
  while (node) {
    if (node.nodeType === 1 && node.classList?.contains('gpv__page')) {
      pageNum = Number(node.getAttribute('data-page')) || pageNum
      break
    }
    node = node.parentNode
  }
  return { text, page: pageNum }
}

// ─────────────────── 正向：源码 -> PDF（含完整高亮框）───────────────────
const flashBox = ref(null)
let flashTimer = null

const scrollToPagePoint = (pageNum, yPt) => {
  const el = bodyRef.value
  const node = pageEls.get(pageNum)
  if (!el || !node) return
  const target = node.offsetTop + yPt * cssScale.value - el.clientHeight * 0.35
  el.scrollTo({ top: Math.max(0, target), behavior: 'smooth' })
}

const showFlashBox = (box) => {
  flashBox.value = box
  if (flashTimer) clearTimeout(flashTimer)
  flashTimer = setTimeout(() => { flashBox.value = null }, 1800)
}

const goToPage = (n, { align = 'top' } = {}) => {
  const num = Math.min(Math.max(1, Math.round(Number(n) || 1)), totalPages.value || 1)
  const el = bodyRef.value
  const node = pageEls.get(num)
  if (!el || !node) return
  const offset = align === 'center' ? el.clientHeight * 0.3 : 8
  el.scrollTo({ top: Math.max(0, node.offsetTop - offset), behavior: 'smooth' })
}

const forwardSync = async ({ line, column = 1 } = {}) => {
  if (!hasPdf.value || !line) return
  try {
    let res
    if (engineKind.value === 'typst') {
      res = await invoke('typst_forward_search', { sourcePath: props.sourcePath, line, column })
    } else {
      res = await invoke('synctex_forward', {
        pdfPath: doc.getActivePath(),
        sourcePath: props.sourcePath,
        line,
        column,
      })
    }
    if (res && res.page) {
      let box = { page: res.page, x: res.x, y: res.y, w: res.w || 0, h: res.h || 0 }
      // 没有 w/h（typst 只给点）时，向后端取整行 bbox 做完整高亮框。
      if (!box.w || !box.h) {
        const loc = await doc.locatePoint(res.page, res.x, res.y)
        if (loc && loc.found) box = { page: res.page, x: loc.x, y: loc.y, w: loc.w, h: loc.h }
      }
      scrollToPagePoint(box.page, box.y)
      showFlashBox(box)
    }
  } catch (e) {
    logger.warn('正向定位失败', { error: String(e?.message || e) })
  }
}

// ─────────────────── 高亮：选区 -> 写回 .pdf ───────────────────
// 把当前 DOM 选区的 client rects 转成"页内左上原点 pt"矩形，供后端创建高亮批注。
const getSelectionRects = () => {
  const sel = window.getSelection?.()
  if (!sel || !sel.rangeCount) return null
  const text = String(sel).trim()
  if (!text) return null
  let node = sel.anchorNode
  let pageEl = null
  while (node) {
    if (node.nodeType === 1 && node.classList?.contains('gpv__page')) {
      pageEl = node
      break
    }
    node = node.parentNode
  }
  if (!pageEl) return null
  const pageNum = Number(pageEl.getAttribute('data-page'))
  const base = pageEl.getBoundingClientRect()
  const s = cssScale.value
  const rects = []
  for (const r of sel.getRangeAt(0).getClientRects()) {
    if (r.width < 1 || r.height < 1) continue
    rects.push({
      x: (r.left - base.left) / s,
      y: (r.top - base.top) / s,
      w: r.width / s,
      h: r.height / s,
    })
  }
  if (!rects.length) return null
  return { page: pageNum, rects, text }
}

const addHighlightFromSelection = async ({ color, note } = {}) => {
  const info = getSelectionRects()
  if (!info) return null
  const annotId = await doc.addHighlight({ page: info.page, rects: info.rects, color, note })
  window.getSelection?.()?.removeAllRanges()
  if (!annotId) return null
  return { annotId, page: info.page, text: info.text, rects: info.rects }
}

const deleteHighlight = (page, annotId) => doc.deleteHighlight(page, annotId)

const refresh = () => load(props.path, true)

defineExpose({
  forwardSync,
  goToPage,
  search: runSearch,
  setSearchQuery: (q) => { searchQuery.value = q },
  nextHit,
  prevHit,
  clearSearch,
  getOutline: doc.getOutline,
  getSelectionInfo,
  getSelectionRects,
  addHighlightFromSelection,
  deleteHighlight,
  setZoom,
  zoomIn,
  zoomOut,
  fitWidth,
  fitPage,
  refresh,
  scrollRatio,
  restoreScroll,
})

// ───────────────────────── watchers ─────────────────────────
watch(() => props.path, (p) => {
  const base = String(p || '').split('?')[0]
  const keep = base !== '' && base === lastBase
  lastBase = base
  load(p, keep)
}, { immediate: true })

if (typeof window !== 'undefined') {
  window.addEventListener('mouseup', onBodyMouseUp, true)
}

onBeforeUnmount(() => {
  if (flashTimer) clearTimeout(flashTimer)
  if (scrollRaf) cancelAnimationFrame(scrollRaf)
  if (typeof window !== 'undefined') {
    window.removeEventListener('mouseup', onBodyMouseUp, true)
  }
  doc.closeDocument()
})
</script>

<style scoped>
.gpv {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  min-height: 0;
  background: var(--gpv-bg);
  color: var(--gpv-text);
}

.gpv.theme-dark {
  --gpv-bg: #1e1e1e;
  --gpv-text: #ddd;
  --gpv-toolbar-bg: #252525;
  --gpv-border: #3e4451;
  --gpv-canvas-area: #525659;
  --gpv-btn-text: #ddd;
  --gpv-hover-bg: rgba(255, 255, 255, 0.08);
  --gpv-error-bg: rgba(0, 0, 0, 0.35);
  --gpv-overlay-bg: rgba(0, 0, 0, 0.45);
  --gpv-overlay-text: #fff;
}
.gpv.theme-light {
  --gpv-bg: #ffffff;
  --gpv-text: #1f2328;
  --gpv-toolbar-bg: #f5f6f8;
  --gpv-border: #d0d7de;
  --gpv-canvas-area: #eef0f3;
  --gpv-btn-text: #1f2328;
  --gpv-hover-bg: rgba(0, 0, 0, 0.06);
  --gpv-error-bg: rgba(0, 0, 0, 0.04);
  --gpv-overlay-bg: rgba(255, 255, 255, 0.7);
  --gpv-overlay-text: #1f2328;
}

.gpv__toolbar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  border-bottom: 1px solid var(--gpv-border);
  background: var(--gpv-toolbar-bg);
  font-size: 12px;
}
.gpv__title { font-weight: 600; color: var(--gpv-text); }
.gpv__status { font-size: 11px; opacity: 0.7; }
.gpv__status.ok { color: #2da44e; opacity: 1; }
.gpv.theme-dark .gpv__status.ok { color: #98c379; }
.gpv__status.error { color: #cf222e; opacity: 1; }
.gpv.theme-dark .gpv__status.error { color: #e06c75; }
.gpv__spacer { flex: 1 1 auto; }
.gpv__divider { width: 1px; height: 16px; background: var(--gpv-border); margin: 0 2px; }

.gpv__search { display: flex; align-items: center; gap: 4px; }
.gpv__search-input {
  width: 130px;
  font-size: 12px;
  padding: 2px 6px;
  border: 1px solid var(--gpv-border);
  border-radius: 6px;
  background: transparent;
  color: var(--gpv-text);
}
.gpv__search-input:focus { outline: none; border-color: var(--n-color-primary, #61afef); }
.gpv__search-count { font-size: 11px; opacity: 0.7; min-width: 32px; text-align: center; }

.gpv__page-indicator { font-size: 11px; opacity: 0.75; min-width: 48px; text-align: center; }
.gpv__zoom { font-size: 11px; opacity: 0.75; min-width: 38px; text-align: center; }

.gpv__btn {
  border: 1px solid var(--gpv-border);
  background: transparent;
  color: var(--gpv-btn-text);
  border-radius: 6px;
  padding: 2px 8px;
  font-size: 12px;
  cursor: pointer;
  transition: background-color 0.15s, border-color 0.15s;
}
.gpv__btn:hover:not(:disabled) { background: var(--gpv-hover-bg); border-color: var(--n-color-primary, #61afef); }
.gpv__btn:disabled { opacity: 0.4; cursor: not-allowed; }
.gpv__btn--primary { border-color: var(--n-color-primary, #61afef); color: var(--n-color-primary, #61afef); }

.gpv__body {
  flex: 1 1 auto;
  min-height: 0;
  position: relative;
  overflow: auto;
  background: var(--gpv-canvas-area);
}

.gpv__pages {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 12px;
}

.gpv__page {
  position: relative;
  background: #fff;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.25);
  flex: 0 0 auto;
}
.gpv__page-img { display: block; user-select: none; }
.gpv__page-skeleton {
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
.gpv__textlayer {
  position: absolute;
  inset: 0;
  overflow: hidden;
  line-height: 1;
}
.gpv__text-seg {
  position: absolute;
  color: transparent;
  white-space: pre;
  transform-origin: 0 0;
  cursor: text;
  user-select: text;
}
.gpv__text-seg::selection { background: rgba(97, 175, 239, 0.4); }

.gpv__search-rect {
  position: absolute;
  background: rgba(255, 213, 0, 0.35);
  border: 1px solid rgba(255, 170, 0, 0.6);
  pointer-events: none;
  border-radius: 1px;
}
.gpv__search-rect.active {
  background: rgba(255, 145, 0, 0.5);
  border-color: rgba(255, 120, 0, 0.9);
}

/* 已保存的高亮批注覆盖层 */
.gpv__hl-rect {
  position: absolute;
  border-radius: 1px;
  cursor: pointer;
  mix-blend-mode: multiply;
}
.gpv__hl-rect:hover {
  outline: 1px solid rgba(255, 140, 0, 0.8);
}

/* 定位高亮：完整包裹文字的框 + 淡出 */
.gpv__flash-box {
  position: absolute;
  background: rgba(97, 175, 239, 0.28);
  border: 1.5px solid var(--n-color-primary, #61afef);
  border-radius: 2px;
  box-shadow: 0 0 8px 1px rgba(97, 175, 239, 0.5);
  pointer-events: none;
  animation: gpv-flash 1.8s ease-out forwards;
}
@keyframes gpv-flash {
  0% { opacity: 1; }
  70% { opacity: 0.85; }
  100% { opacity: 0; }
}

.gpv__empty,
.gpv__error {
  margin: auto;
  max-width: 90%;
  text-align: center;
  color: var(--gpv-text);
  padding: 24px;
}
.gpv__error { text-align: left; width: 100%; }
.gpv__error-title { color: #cf222e; font-weight: 600; margin-bottom: 8px; }
.gpv.theme-dark .gpv__error-title { color: #e06c75; }
.gpv__error-log {
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 11.5px;
  line-height: 1.45;
  background: var(--gpv-error-bg);
  border: 1px solid rgba(207, 34, 46, 0.35);
  border-radius: 8px;
  padding: 10px;
  max-height: 100%;
  overflow: auto;
  color: var(--gpv-text);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

.gpv__overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  background: var(--gpv-overlay-bg);
  color: var(--gpv-overlay-text);
  font-size: 13px;
  backdrop-filter: blur(1px);
}
.gpv__spinner {
  width: 16px;
  height: 16px;
  border: 2px solid rgba(127, 127, 127, 0.3);
  border-top-color: var(--n-color-primary, #61afef);
  border-radius: 50%;
  animation: gpv-spin 0.8s linear infinite;
}
@keyframes gpv-spin { to { transform: rotate(360deg); } }
</style>
