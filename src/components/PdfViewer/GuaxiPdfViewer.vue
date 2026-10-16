<template>
  <div
    ref="rootRef"
    class="gpv"
    :class="`theme-${theme}`"
    tabindex="0"
    @keydown="onViewerKeydown"
    @mousedown.capture="focusViewerForKeyboard"
  >
    <div v-if="features.toolbar !== false" class="gpv__toolbar">
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
          aria-label="在 PDF 中搜索，Enter 下一个，Shift+Enter 上一个，Esc 清空"
          title="在 PDF 中搜索（Enter 下一个，Shift+Enter 上一个，Esc 清空）"
          @input="scheduleSearch('input')"
          @keydown="onSearchInputKeydown"
        />
        <span v-if="searchHits.length" class="gpv__search-count">
          {{ searchActiveIdx + 1 }}/{{ searchHits.length }}
        </span>
        <button
          type="button"
          class="gpv__btn gpv__btn--icon"
          title="上一个匹配（Shift+Enter）"
          aria-label="上一个匹配"
          :disabled="!searchHits.length"
          @click="prevHit"
        >
          <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
            <path d="M15.5 5.5 9 12l6.5 6.5-1.4 1.4L6.2 12l7.9-7.9 1.4 1.4Z" fill="currentColor" />
          </svg>
        </button>
        <button
          type="button"
          class="gpv__btn gpv__btn--icon"
          title="下一个匹配（Enter）"
          aria-label="下一个匹配"
          :disabled="!searchHits.length"
          @click="nextHit"
        >
          <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
            <path d="m8.5 18.5 6.5-6.5-6.5-6.5 1.4-1.4 7.9 7.9-7.9 7.9-1.4-1.4Z" fill="currentColor" />
          </svg>
        </button>
      </div>

      <span class="gpv__divider" />
      <div v-if="features.pagination !== false" class="gpv__pager" aria-label="PDF 分页导航">
        <button
          type="button"
          class="gpv__btn gpv__btn--icon"
          :disabled="!canGoPreviousPage"
          title="上一页"
          aria-label="上一页"
          @click="goPreviousPage"
        >
          <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
            <path d="M15.5 5.5 9 12l6.5 6.5-1.4 1.4L6.2 12l7.9-7.9 1.4 1.4Z" fill="currentColor" />
          </svg>
        </button>
        <span class="gpv__page-indicator">
        <input
          class="gpv__page-input"
          type="number"
          min="1"
          :max="totalPages || 1"
          v-model="pageInputValue"
          title="跳转到页码"
          aria-label="跳转到页码"
          @keydown="onPageInputKeydown"
          @blur="syncPageInput"
        />
        / {{ totalPages }}
        </span>
        <button
          type="button"
          class="gpv__btn gpv__btn--icon"
          :disabled="!canGoNextPage"
          title="下一页"
          aria-label="下一页"
          @click="goNextPage"
        >
          <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
            <path d="m8.5 18.5 6.5-6.5-6.5-6.5 1.4-1.4 7.9 7.9-7.9 7.9-1.4-1.4Z" fill="currentColor" />
          </svg>
        </button>
      </div>
      <span class="gpv__divider" />

      <div class="gpv__zoom-cluster">
        <button type="button" class="gpv__btn gpv__btn--icon" :disabled="zoom <= MIN_ZOOM" title="缩小（-）" aria-label="缩小" @click="zoomOut('toolbar')">
          <RemoveOutline />
        </button>
        <span class="gpv__zoom">{{ Math.round(zoom * 100) }}%</span>
        <button type="button" class="gpv__btn gpv__btn--icon" :disabled="zoom >= MAX_ZOOM" title="放大（+）" aria-label="放大" @click="zoomIn('toolbar')">
          <AddOutline />
        </button>
      </div>
      <button type="button" class="gpv__btn gpv__btn--icon" title="适合宽度（W）" aria-label="适合宽度" @click="fitWidth">
        <ExpandOutline />
      </button>
      <span class="gpv__divider" />
      <button type="button" class="gpv__btn gpv__btn--icon" title="向左旋转 90°（[）" aria-label="向左旋转 90°" @click="rotateLeft('toolbar')">
        <RefreshCircleOutline :rotate="90" />
      </button>
      <button type="button" class="gpv__btn gpv__btn--icon" title="向右旋转 90°（]）" aria-label="向右旋转 90°" @click="rotateRight('toolbar')">
        <RefreshCircleOutline :rotate="-90" />
      </button>
      <button
        type="button"
        class="gpv__btn gpv__btn--icon"
        :class="{ 'gpv__btn--active': panMode }"
        title="锁定拖动平移（H）。平时在页面空白处直接拖动即可平移，无需开启"
        aria-label="锁定拖动平移模式"
        @click="togglePanMode('toolbar')"
      >
        <HandRightOutline />
      </button>
      <button v-if="features.rebuild" type="button" class="gpv__btn gpv__btn--icon" title="重新构建" @click="$emit('rebuild')">
        <RefreshOutline />
      </button>
      <button
        v-if="features.export"
        type="button"
        class="gpv__btn gpv__btn--primary"
        :disabled="!hasPdf"
        title="导出 PDF"
        @click="$emit('export')"
      >导出</button>
    </div>

    <div
      class="gpv__body guaxi-reader-scrollbar"
      ref="bodyRef"
      :class="{ 'gpv__body--pan': panEffective, 'gpv__body--panning': isPanning }"
      @scroll="onScroll"
      @mousedown="onBodyMouseDown"
      @wheel="onWheel"
    >
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
          :style="pageBoxStyle(page)"
          @mousedown="onPageMouseDown(page.num, $event)"
        >
          <!-- 内层包裹：img + 文本层 + 各覆盖层都在同一未旋转坐标系内，旋转此层使三者
               一起旋转、对齐天然保持；外层 .gpv__page 取旋转后的占位尺寸以撑出正确布局。 -->
          <div class="gpv__page-inner" :style="pageInnerStyle(page)">
            <img
              v-if="page.image"
              class="gpv__page-img"
              :src="page.image"
              :width="Math.round(page.wPt * cssScale)"
              :height="Math.round(page.hPt * cssScale)"
              draggable="false"
              decoding="async"
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
                :class="{ 'is-selected': !!selectedHighlightId && hl.annot_id === selectedHighlightId }"
                :style="hlStyle(rc, hl.color)"
                :title="highlightHint(hl)"
                @click.stop="onHighlightClick(hl, page.num, $event)"
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
      </div>

      <div v-if="building" class="gpv__overlay">
        <span class="gpv__spinner" />
        <span>正在构建…</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, computed, watch, nextTick, onMounted, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  AddOutline,
  ExpandOutline,
  HandRightOutline,
  RefreshCircleOutline,
  RefreshOutline,
  RemoveOutline,
} from '@vicons/ionicons5'
import { createLogger } from '../../utils/logger'
import { markReaderScrollbarActive } from '../ReaderComponent/readerScrollbar'
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
  // 默认缩放策略：'width' | 'page' | 'none'。
  // 'none' 沿用旧行为（按 initialZoom 起始，不随容器尺寸变化重算）；LaTeX/Typst 预览传 'width'，
  // 让默认阅读区域贴合面板，并在窗口 / 分栏尺寸变化时自动重算。
  fitMode: { type: String, default: 'none' },
  // 当前选中的高亮 annot_id（由宿主维护）。选中的那一块会额外圈出来；
  // 之所以不由本组件自己记，是因为宿主还要拿它联动右侧的笔记面板。
  selectedHighlightId: { type: String, default: '' },
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
  'scroll',
])

const MIN_ZOOM = 0.4
const MAX_ZOOM = 4
const zoom = ref(props.initialZoom)
// 当前生效的自适应策略（'width' | 'page'）；'' 表示手动缩放，不再自动重算。
const activeFitMode = ref(props.fitMode === 'width' || props.fitMode === 'page' ? props.fitMode : '')
const cssScale = computed(() => zoom.value)
// 渲染 DPI：目标是渲染像素 = 显示像素 × 设备像素比（1:1 物理像素，清晰且不过采样）。
// 但取值必须落到下面这条**阶梯**上，不能直接用连续值。后端的页图磁盘缓存键是
// `p{页}@{dpi}_{mtime}.jpg`——dpi 连续的话，Ctrl+滚轮每动一格就得到一个新 dpi
// （111 → 114 → 118 …），每一格都要重新栅格化、还各留一份缓存文件；量化到阶梯后，
// 小幅缩放落回同一档，直接命中缓存，缩放因此变得跟手。
// 阶梯往上取整（宁可略清晰也不欠采样），上限仍是 384 dpi（A4 约 3300×4700，和之前一致）。
const devicePixelRatio = (typeof window !== 'undefined' && window.devicePixelRatio) || 1
const renderScale = Math.min(2, Math.max(1, devicePixelRatio))
const DPI_LADDER = [72, 96, 120, 144, 192, 240, 288, 384]
const renderDpi = computed(() => {
  const ideal = zoom.value * 72 * renderScale
  for (const d of DPI_LADDER) if (d >= ideal) return d
  return DPI_LADDER[DPI_LADDER.length - 1]
})

// ───────────────────────── 旋转 ─────────────────────────
// 角度 0/90/180/270。CSS 旋转内层包裹（pageInnerStyle），外层占位按旋转后尺寸交换宽高。
const rotation = ref(0)
const isRotated90 = computed(() => rotation.value === 90 || rotation.value === 270)
const rotateLeft = (reason = 'manual') => {
  rotation.value = (rotation.value + 270) % 360
  logger.info('PDF向左旋转', { reason, rotation: rotation.value })
}
const rotateRight = (reason = 'manual') => {
  rotation.value = (rotation.value + 90) % 360
  logger.info('PDF向右旋转', { reason, rotation: rotation.value })
}

const pageBoxStyle = (page) => {
  const w = Math.round(page.wPt * cssScale.value)
  const h = Math.round(page.hPt * cssScale.value)
  return { width: `${isRotated90.value ? h : w}px`, height: `${isRotated90.value ? w : h}px` }
}
const pageInnerStyle = (page) => {
  const w = Math.round(page.wPt * cssScale.value)
  const h = Math.round(page.hPt * cssScale.value)
  const fw = isRotated90.value ? h : w
  const fh = isRotated90.value ? w : h
  return {
    width: `${w}px`,
    height: `${h}px`,
    left: `${(fw - w) / 2}px`,
    top: `${(fh - h) / 2}px`,
    transform: rotation.value ? `rotate(${rotation.value}deg)` : 'none',
    transformOrigin: 'center center',
  }
}

const doc = usePdfiumDocument()
const { pages, totalPages, hasPdf, status, statusTone } = doc

const bodyRef = ref(null)
const rootRef = ref(null)
const searchInputRef = ref(null)
const currentPage = ref(1)
const pageInputValue = ref('1')
const canGoPreviousPage = computed(() => hasPdf.value && currentPage.value > 1)
const canGoNextPage = computed(() => hasPdf.value && currentPage.value < (totalPages.value || 1))

const pageEls = new Map()
const setPageRef = (num, el) => {
  if (el) pageEls.set(num, el)
  else pageEls.delete(num)
}

let resizeObserver = null
let resizeRenderRaf = 0
const scheduleLayoutRender = (reason = 'layout') => {
  if (resizeRenderRaf) return
  resizeRenderRaf = requestAnimationFrame(() => {
    resizeRenderRaf = 0
    updateVisible()
    renderVisible()
    logger.debug('PDF阅读器布局刷新', {
      reason,
      bodyHeight: bodyRef.value?.clientHeight || 0,
      pages: pages.length,
    })
  })
}

const engineKind = computed(() => {
  if (props.engine) return props.engine
  return /typst/i.test(props.titleLabel) ? 'typst' : 'latex'
})

// 手动缩放一律走 setZoom：它要顺带退出自适应模式，否则在 fit-mode='width' 下点了 +/− 之后，
// 任何一次容器尺寸变化都会把用户的缩放顶回适合宽度。
const zoomIn = (reason = 'manual') => setZoom(zoom.value + 0.1, reason)
const zoomOut = (reason = 'manual') => setZoom(zoom.value - 0.1, reason)
const setZoom = (z, reason = 'manual') => {
  // 手动缩放即退出自适应模式：否则之后任何一次窗口尺寸变化都会把用户的缩放顶掉。
  activeFitMode.value = ''
  zoom.value = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, +Number(z).toFixed(2)))
  logger.debug('PDF缩放设置', { reason, zoom: zoom.value })
}

// Ctrl/⌘ + 滚轮缩放：阻止浏览器默认整页缩放，改为按步缩放 PDF。
const onWheel = (ev) => {
  if (!(ev.ctrlKey || ev.metaKey)) return
  ev.preventDefault()
  const step = Math.min(0.5, 0.0015 * Math.abs(ev.deltaY) + 0.05)
  if (ev.deltaY < 0) setZoom(zoom.value + step, 'ctrl-wheel')
  else if (ev.deltaY > 0) setZoom(zoom.value - step, 'ctrl-wheel')
}
// ─────────────────── 自适应缩放（适合宽度 / 适合整页）───────────────────
// 设计说明：预览面板的宽度会随窗口和分栏拖拽变化，若只在初始时定一个 zoom，窗口一变阅读区
// 就不再贴合面板。所以把"适合宽度/整页"做成一种**持续生效的模式**：打开文档、容器尺寸变化时
// 都重算；一旦用户手动缩放（Ctrl+滚轮 / +− / 工具栏输入），立刻退出该模式，之后不再抢用户的缩放。
const AUTO_FIT_DEBOUNCE_MS = 140
let autoFitTimer = null

const computeFitZoom = (mode) => {
  const el = bodyRef.value
  const first = pages[0]
  if (!el || !first || !first.wPt || !first.hPt) return null
  const zx = (el.clientWidth - 36) / first.wPt
  if (mode === 'page') return Math.min(zx, (el.clientHeight - 36) / first.hPt)
  return zx
}

// preserveScroll：重算前记住滚动比例、算完落回，拖动窗口时不丢阅读位置。
// 新文档首次落位用 false —— 阅读位置由宿主的恢复流程负责，这里别去抢。
const applyFit = ({ reason = 'auto', preserveScroll = false } = {}) => {
  if (!activeFitMode.value) return false
  const z = computeFitZoom(activeFitMode.value)
  if (!z || !Number.isFinite(z) || z <= 0) return false
  const next = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, +z.toFixed(2)))
  if (next === zoom.value) return false
  const ratio = preserveScroll ? scrollRatio() : 0
  zoom.value = next
  logger.debug('PDF自适应缩放', { reason, mode: activeFitMode.value, zoom: next })
  if (preserveScroll) nextTick(() => restoreScroll(ratio))
  return true
}

// 容器尺寸变化时重算。ResizeObserver 在拖拽过程中会连续触发，而每次改 zoom 都要把可见页
// 按新 DPI 重新栅格化（不便宜），所以防抖。
const scheduleAutoFit = (reason = 'resize') => {
  if (!activeFitMode.value) return
  if (autoFitTimer) clearTimeout(autoFitTimer)
  autoFitTimer = setTimeout(() => {
    autoFitTimer = null
    applyFit({ reason, preserveScroll: true })
  }, AUTO_FIT_DEBOUNCE_MS)
}

const fitWidth = () => {
  activeFitMode.value = 'width'
  applyFit({ reason: 'fit-width', preserveScroll: true })
}
const fitPage = () => {
  activeFitMode.value = 'page'
  applyFit({ reason: 'fit-page', preserveScroll: true })
}

const isEditableTarget = (target) => {
  const el = target instanceof Element ? target : null
  if (!el) return false
  const tag = String(el.tagName || '').toLowerCase()
  return tag === 'input' || tag === 'textarea' || tag === 'select' || el.isContentEditable
}

const isToolbarInteractiveTarget = (target) => {
  const el = target instanceof Element ? target : null
  return Boolean(el?.closest?.('button, input, textarea, select, a, [role="button"], [contenteditable="true"]'))
}

const focusViewerForKeyboard = (ev) => {
  // 设计说明：阅读区需要获得焦点才能接收 PageUp/PageDown、缩放、旋转等快捷键；
  // 但工具栏控件自身也要保留原生键盘行为，所以只在点击非交互区域时聚焦根容器。
  if (isToolbarInteractiveTarget(ev.target)) return
  rootRef.value?.focus?.({ preventScroll: true })
}

const focusSearchInput = async (reason = 'shortcut') => {
  if (!props.features.search || !hasPdf.value || props.errorText) return
  await nextTick()
  searchInputRef.value?.focus?.()
  searchInputRef.value?.select?.()
  logger.info('PDF搜索框聚焦', { reason })
}

const scrollViewerBy = (deltaY, reason) => {
  const el = bodyRef.value
  if (!el) return
  el.scrollBy({ top: deltaY, behavior: 'smooth' })
  scheduleRenderVisible()
  logger.debug('PDF键盘滚动', { reason, deltaY })
}

const onViewerKeydown = (ev) => {
  if (isEditableTarget(ev.target)) return

  const key = ev.key
  const lowerKey = String(key || '').toLowerCase()
  const ctrlOrMeta = ev.ctrlKey || ev.metaKey
  const handled = (action) => {
    ev.preventDefault()
    ev.stopPropagation()
    logger.debug('PDF阅读器快捷键触发', { key, action })
  }

  if (ctrlOrMeta && lowerKey === 'f' && props.features.search) {
    handled('focus-search')
    focusSearchInput('ctrl-f')
    return
  }
  if (!ctrlOrMeta && key === '/') {
    handled('focus-search')
    focusSearchInput('slash')
    return
  }
  if (!ctrlOrMeta && key === 'PageUp') {
    handled('previous-page')
    goPreviousPage()
    return
  }
  if (!ctrlOrMeta && key === 'PageDown') {
    handled('next-page')
    goNextPage()
    return
  }
  if (!ctrlOrMeta && key === 'Home') {
    handled('first-page')
    goToPage(1)
    return
  }
  if (!ctrlOrMeta && key === 'End') {
    handled('last-page')
    goToPage(totalPages.value || 1)
    return
  }
  if (!ctrlOrMeta && key === 'ArrowUp') {
    handled('scroll-up')
    scrollViewerBy(-80, 'arrow-up')
    return
  }
  if (!ctrlOrMeta && key === 'ArrowDown') {
    handled('scroll-down')
    scrollViewerBy(80, 'arrow-down')
    return
  }
  if (!ctrlOrMeta && key === ' ') {
    handled(ev.shiftKey ? 'scroll-page-up' : 'scroll-page-down')
    scrollViewerBy((ev.shiftKey ? -1 : 1) * Math.max(120, (bodyRef.value?.clientHeight || 640) * 0.82), 'space')
    return
  }
  if (!ctrlOrMeta && (key === '+' || key === '=')) {
    handled('zoom-in')
    zoomIn('keyboard')
    return
  }
  if (!ctrlOrMeta && key === '-') {
    handled('zoom-out')
    zoomOut('keyboard')
    return
  }
  if (!ctrlOrMeta && lowerKey === 'w') {
    handled('fit-width')
    fitWidth()
    return
  }
  if (!ctrlOrMeta && lowerKey === 'p') {
    handled('fit-page')
    fitPage()
    return
  }
  if (!ctrlOrMeta && key === '[') {
    handled('rotate-left')
    rotateLeft('keyboard')
    return
  }
  if (!ctrlOrMeta && key === ']') {
    handled('rotate-right')
    rotateRight('keyboard')
    return
  }
  if (!ctrlOrMeta && lowerKey === 'h') {
    handled('toggle-pan')
    togglePanMode('keyboard')
  }
}

// ───────────────────────── 打开 / 重建 ─────────────────────────
let lastBase = ''
const load = async (fp, keep) => {
  const prevRatio = keep ? scrollRatio() : 0
  const r = await doc.openDocument(fp, { keepExisting: keep })
  if (!r.ok) return
  if (!keep) {
    // 换了文档：回到宿主指定的默认自适应策略（上一个文档里的手动缩放不跨文档保留）。
    activeFitMode.value = props.fitMode === 'width' || props.fitMode === 'page' ? props.fitMode : ''
  }
  emit('ready', { totalPages: totalPages.value })
  await nextTick()
  if (keep) restoreScroll(prevRatio)
  else applyFit({ reason: 'load' }) // 首帧之后页尺寸已知，按自适应策略定初始缩放
  updateVisible()
  renderVisible()
  scheduleLayoutRender('load')
  if (props.features.outline) emitOutline()
  if (searchQuery.value.trim()) runSearch()
}

const emitOutline = async () => {
  const list = await doc.getOutline()
  emit('outline', list)
}

// 设计说明：返回 null 表示“此刻量不到”，而不是 0。
// 容器被 v-show 隐藏、或被 KeepAlive 移出文档时，scrollTop 恒为 0；宿主正是拿这个比例按文件
// 记住阅读位置，把量不到的 0 当成真实读数，就会把正确位置覆盖成文档开头。
const scrollRatio = () => {
  const el = bodyRef.value
  if (!el || !el.isConnected || el.clientHeight <= 0) return null
  if (el.scrollHeight <= el.clientHeight) return 0
  return el.scrollTop / (el.scrollHeight - el.clientHeight)
}
const restoreScroll = (ratio) => {
  const el = bodyRef.value
  if (!el || !ratio) return
  el.scrollTop = ratio * (el.scrollHeight - el.clientHeight)
}

// 按比例落位，并跨帧重试到布局稳定为止。
// 为什么不能只落一次：宿主恢复阅读位置是在 emit('ready') 那一刻发起的，和 load() 后面的
// nextTick / renderVisible / scheduleLayoutRender 链是并发的；文档刚打开时页数组才 splice 重建，
// 容器可能一度没有可滚动高度，此时落位会被浏览器 clamp 到顶部——表现就是"切回旧文档停在开头"。
// Markdown / 即时渲染早就有带增长重试的落位（restoreScrollTopWithGrowthRetry），PDF 缺这一环，
// 这里补齐：等可滚动高度连续两帧不再变化，再认为落位生效。
const RESTORE_SCROLL_MAX_ATTEMPTS = 12
const RESTORE_SCROLL_RETRY_DELAY_MS = 60
const RESTORE_SCROLL_STABLE_FRAMES = 2
const restoreScrollStable = async (ratio) => {
  const target = Number(ratio)
  if (!Number.isFinite(target) || target <= 0) return false
  let lastMaxTop = -1
  let stableCount = 0
  for (let attempt = 0; attempt < RESTORE_SCROLL_MAX_ATTEMPTS; attempt += 1) {
    await new Promise((resolve) => window.requestAnimationFrame(resolve))
    const el = bodyRef.value
    if (!el || !el.isConnected) return false
    const maxTop = Math.max(0, el.scrollHeight - el.clientHeight)
    if (maxTop > 0) {
      el.scrollTop = target * maxTop
      stableCount = maxTop === lastMaxTop ? stableCount + 1 : 0
      lastMaxTop = maxTop
      // 连续两帧可滚动高度不再变化，说明布局稳定，落位有效。
      if (stableCount >= RESTORE_SCROLL_STABLE_FRAMES) return true
    }
    await new Promise((resolve) => window.setTimeout(resolve, RESTORE_SCROLL_RETRY_DELAY_MS))
  }
  return lastMaxTop > 0
}

// ─────────────────── 虚拟化：渲染可见页 ───────────────────
// 滚动分两条路：
//  · 每帧（rAF）只做廉价的 DOM 测算，更新 currentPage / 派发 page-change，保证页码跟手；
//  · 真正的栅格化/文本/高亮拉取走防抖，等滚动稳定 ~120ms 才执行——这样快速滑过的页不会
//    给单线程 pdfium worker 排一堆很快作废的整页渲染任务。
let scrollRaf = 0
const RENDER_SETTLE_MS = 120
let renderSettleTimer = null

const onScroll = () => {
  markReaderScrollbarActive(bodyRef.value)
  // 宿主（GuaxiEditor）据此按源文件记住 PDF 阅读位置，见 GuaxiLatexPreview 的转发。
  emit('scroll')
  if (!scrollRaf) {
    scrollRaf = requestAnimationFrame(() => {
      scrollRaf = 0
      updateVisible()
    })
  }
  scheduleRenderVisible()
}

const scheduleRenderVisible = () => {
  if (renderSettleTimer) clearTimeout(renderSettleTimer)
  renderSettleTimer = setTimeout(() => {
    renderSettleTimer = null
    renderVisible()
  }, RENDER_SETTLE_MS)
}

// 廉价：只算可见范围、更新当前页，不触发任何后端调用。
const updateVisible = () => {
  const el = bodyRef.value
  if (!el || !pages.length) return
  const top = el.scrollTop
  const bottom = top + el.clientHeight

  let firstVisible = currentPage.value
  let bestOverlap = 0

  for (const page of pages) {
    const node = pageEls.get(page.num)
    if (!node) continue
    const nTop = node.offsetTop
    const nBottom = nTop + node.offsetHeight
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

// 重活：对当前可见（默认含上下一屏缓冲）的页做栅格化/文本/高亮；远离可见区的已渲染页释放图片。
// bufferFactor=0 时只渲染严格可见页——缩放时用它，避免一次重渲多页大位图导致卡顿。
const renderVisible = ({ bufferFactor = 1 } = {}) => {
  const el = bodyRef.value
  if (!el || !pages.length) return
  const top = el.scrollTop
  const bottom = top + el.clientHeight
  const BUFFER = el.clientHeight * bufferFactor

  const active = []
  for (const page of pages) {
    const node = pageEls.get(page.num)
    if (!node) continue
    const nTop = node.offsetTop
    const nHeight = node.offsetHeight
    if (nTop + nHeight >= top - BUFFER && nTop <= bottom + BUFFER) {
      active.push({ page, center: nTop + nHeight / 2 })
    } else if (page.image && Math.abs(page.num - currentPage.value) > 6) {
      doc.freePage(page)
    }
  }

  // 提交顺序就是出图顺序：后端只有一个 pdfium 工作线程、按 FIFO 取任务。若按 DOM 顺序
  // （文档自上而下）提交，滚到第 300 页、缓冲区覆盖 297–303 时，会先栅格化 297 再轮到 300，
  // 用户在看的那页反而最后出图。改成按离视口中心由近及远排序，当前页永远第一个渲。
  const viewportCenter = (top + bottom) / 2
  active.sort((a, b) => Math.abs(a.center - viewportCenter) - Math.abs(b.center - viewportCenter))

  for (const { page } of active) {
    doc.renderPage(page, renderDpi.value)
    ensureText(page)
    if (props.features.annotations) doc.loadHighlights(page)
  }
}

// ─────────────────── 拖动平移（空白拖 / 空格 / 中键，带轻惯性）───────────────────
// 交互对齐主流阅读器：不必先去点工具栏。默认就是抓手——在页面空白（行间空隙、页边、
// 图区）按下即拖动平移；在文字上按下仍然是划词选择；按住空格可临时强制平移（松手还原）；
// 中键拖动始终平移；工具栏 ✋ 用来"锁死"平移模式，连文字上也能拖。
const panMode = ref(false)
const spaceHeld = ref(false)
const isPanning = ref(false)
const panEffective = computed(() => panMode.value || spaceHeld.value)
let panState = null
let panRaf = 0
const nowMs = () => (typeof performance !== 'undefined' ? performance.now() : 0)
// 拖动惯性的速度上限（px/ms）：见 onPanMove 里的说明。
const MAX_PAN_VELOCITY = 6

const togglePanMode = (reason = 'manual') => {
  panMode.value = !panMode.value
  logger.info('PDF拖动平移模式切换', { reason, enabled: panMode.value })
}

// 只有落在透明文本层的文字段上才算"要划词"；行间空隙/页边/图片区都归为可拖动，
// 这样与 Acrobat、Chrome 内置阅读器的取舍一致（那边也是拖文字选中、拖空白平移）。
const isOnSelectableText = (ev) => {
  const seg = ev.target?.closest?.('.gpv__text-seg')
  return Boolean(seg && String(seg.textContent || '').trim())
}

// 按下点是否落在原生滚动条上。滚动条属于滚动容器本身，mousedown 的 target 就是容器，
// 若不当成"空白"处理，拖滚动条滑块会被我们抢走并 preventDefault，滑块直接拖不动。
// 判据是几何位置：指针落在内容框之外、边框盒之内，那一条就是滚动条。
const isOnScrollbar = (el, ev) => {
  const rect = el.getBoundingClientRect()
  const vBar = el.offsetWidth - el.clientWidth
  const hBar = el.offsetHeight - el.clientHeight
  if (vBar > 0 && ev.clientX >= rect.left + el.clientLeft + el.clientWidth) return true
  if (hBar > 0 && ev.clientY >= rect.top + el.clientTop + el.clientHeight) return true
  return false
}

const onBodyMouseDown = (ev) => {
  const el = bodyRef.value
  if (!el) return
  // 滚动条任何时候都让给浏览器拖。
  if (ev.button === 0 && isOnScrollbar(el, ev)) return
  // 锁定平移 / 空格临时平移 = 明确要求平移，这时连文字上也要能拖；
  // 其余情况左键落在文字上就是划词，不抢。
  const locked = panMode.value || spaceHeld.value
  if (ev.button === 0 && !locked && isOnSelectableText(ev)) return
  // 右键留给上下文菜单；中键与左键进入平移。
  if (ev.button !== 0 && ev.button !== 1) return
  if (panRaf) { cancelAnimationFrame(panRaf); panRaf = 0 }
  ev.preventDefault()
  isPanning.value = true
  const t = nowMs()
  panState = {
    x: ev.clientX, y: ev.clientY,
    left: el.scrollLeft, top: el.scrollTop,
    lastX: ev.clientX, lastY: ev.clientY, vx: 0, vy: 0, t,
  }
  window.addEventListener('mousemove', onPanMove, true)
  window.addEventListener('mouseup', onPanUp, true)
}
const onPanMove = (ev) => {
  const el = bodyRef.value
  if (!panState || !el) return
  el.scrollLeft = panState.left - (ev.clientX - panState.x)
  el.scrollTop = panState.top - (ev.clientY - panState.y)
  const now = nowMs()
  const dt = now - panState.t
  if (dt > 0) {
    // 速度要封顶：dt 极小时（合成事件、卡顿后补发的一帧）算出来的速度能到上千 px/ms，
    // 惯性循环会把几百页的文档一帧甩到底。真实鼠标最猛的甩动也在 10px/ms 以内，取 6 封顶。
    const clamp = (v) => Math.max(-MAX_PAN_VELOCITY, Math.min(MAX_PAN_VELOCITY, v))
    panState.vx = clamp((ev.clientX - panState.lastX) / dt)
    panState.vy = clamp((ev.clientY - panState.lastY) / dt)
    panState.lastX = ev.clientX
    panState.lastY = ev.clientY
    panState.t = now
  }
}
const onPanUp = () => {
  window.removeEventListener('mousemove', onPanMove, true)
  window.removeEventListener('mouseup', onPanUp, true)
  isPanning.value = false
  const st = panState
  panState = null
  if (st && (Math.abs(st.vx) > 0.05 || Math.abs(st.vy) > 0.05)) startPanInertia(st.vx, st.vy)
  else scheduleRenderVisible()
}
const startPanInertia = (vx, vy) => {
  const el = bodyRef.value
  if (!el) return
  let curVx = vx
  let curVy = vy
  let last = nowMs()
  const stepFn = () => {
    const now = nowMs()
    const dt = now - last
    last = now
    el.scrollLeft -= curVx * dt
    el.scrollTop -= curVy * dt
    curVx *= 0.94
    curVy *= 0.94
    if (Math.abs(curVx) > 0.02 || Math.abs(curVy) > 0.02) {
      panRaf = requestAnimationFrame(stepFn)
    } else {
      panRaf = 0
      scheduleRenderVisible()
    }
  }
  panRaf = requestAnimationFrame(stepFn)
}

// 空格临时平移：监听挂在 window 上（不必先点中阅读器才生效，和主流阅读器一致），
// 但只在阅读器真正可见时接管——宿主用 v-show 隐藏 PDF 面板时 body 高度为 0，不该再抢空格。
// 走冒泡阶段而非捕获：让输入框等内部组件的空格优先。
const viewerVisible = () => (bodyRef.value?.clientHeight || 0) > 0
const isTypingTarget = (node) => {
  const el = node instanceof HTMLElement ? node : null
  if (!el) return false
  return el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT' || el.isContentEditable
}
const onSpaceDown = (ev) => {
  if ((ev.key !== ' ' && ev.code !== 'Space') || ev.isComposing) return
  if (ev.ctrlKey || ev.altKey || ev.metaKey) return
  if (isTypingTarget(ev.target) || !viewerVisible()) return
  // 阻止浏览器把空格当"向下翻页"——按住空格期间它只表示抓手。
  ev.preventDefault()
  spaceHeld.value = true
}
const onSpaceUp = (ev) => {
  if (ev.key !== ' ' && ev.code !== 'Space') return
  spaceHeld.value = false
}
// 切走窗口时按键抬起事件收不到，这里兜底清掉，避免回来后一直是抓手。
const releaseSpacePan = () => { spaceHeld.value = false }

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
const onHighlightClick = (hl, pageNum, ev) => {
  emit('highlight-click', {
    annotId: hl.annot_id,
    page: pageNum,
    note: hl.note || '',
    color: hl.color,
    // 按住 Alt 才是"删除"。以前是单击直接删——没有确认、也没有任何提示，
    // 光标和悬停描边都像"选中"，误删只能靠 Ctrl+Z 捞回来。
    altKey: !!ev?.altKey,
    // 带上 rects，供宿主在删除后做撤销重建。
    rects: Array.isArray(hl.rects) ? hl.rects.map((r) => ({ ...r })) : [],
  })
}

// 高亮的提示文字。删除是破坏性操作，必须在悬停时就能看出怎么触发。
const highlightHint = (hl) => {
  const tip = '点击选中，Alt+点击删除'
  const note = String(hl?.note || '').trim()
  return note ? `${note}\n（${tip}）` : tip
}

// 缩放性能：缩放时 CSS 立即把现有图按新尺寸缩放（即时反馈，可能暂时模糊），
// 真正的高 DPI 重渲做防抖、且只渲"可见页"——避免每点一下缩放就把所有已加载页全部重栅格化
// （这正是缩放卡顿的根因）。scaleX 与缩放无关，无需重新测量。
let zoomRenderTimer = null
watch(renderDpi, () => {
  if (!hasPdf.value) return
  if (zoomRenderTimer) clearTimeout(zoomRenderTimer)
  zoomRenderTimer = setTimeout(() => {
    zoomRenderTimer = null
    // 只重渲严格可见页，缩放更跟手。
    renderVisible({ bufferFactor: 0 })
  }, 180)
})

// 旋转 90/270 会交换每页外框宽高 -> 整体布局高度变化、各页 offsetTop 全变；
// 必须在 DOM 应用新尺寸后重算可见区并触发渲染，否则可见页判断用的是旧几何，
// 旋转后的页落在计算可见范围外而不渲染（表现为 90/270 空白，180/360 因不换宽高才正常）。
watch(rotation, () => {
  if (!hasPdf.value) return
  nextTick(() => {
    updateVisible()
    renderVisible()
  })
})

// ───────────────────────── 搜索 ─────────────────────────
const searchQuery = ref('')
const searchHits = ref([])
const searchActiveIdx = ref(0)
const searchRectsByPage = ref({})
let searchTimer = null
let searchSeq = 0
let lastSearchedQuery = ''

const resetSearchState = () => {
  searchSeq += 1
  searchHits.value = []
  searchActiveIdx.value = 0
  searchRectsByPage.value = {}
  lastSearchedQuery = ''
}

const runSearch = async (reason = 'manual') => {
  if (searchTimer) {
    clearTimeout(searchTimer)
    searchTimer = null
  }
  const q = searchQuery.value.trim()
  if (!q) {
    resetSearchState()
    logger.debug('PDF搜索已清空', { reason })
    return
  }
  const seq = ++searchSeq
  logger.info('PDF搜索开始', { reason, keywordLength: q.length })
  try {
    const resp = await doc.search(q)
    if (seq !== searchSeq) {
      logger.debug('PDF搜索结果已过期，忽略', { reason, keywordLength: q.length })
      return
    }
    const hits = resp.hits || []
    searchHits.value = hits
    searchActiveIdx.value = 0
    lastSearchedQuery = q
    rebuildSearchRects()
    if (hits.length) gotoSearchHit(0, 'search-result')
    status.value = `搜索：${resp.total} 个匹配`
    statusTone.value = resp.total ? 'ok' : ''
    logger.info('PDF搜索完成', { reason, total: resp.total || 0 })
  } catch (e) {
    logger.warn('PDF搜索失败', { reason, error: String(e?.message || e) })
  }
}

const scheduleSearch = (reason = 'input') => {
  if (searchTimer) clearTimeout(searchTimer)
  if (!searchQuery.value.trim()) {
    resetSearchState()
    return
  }
  searchTimer = setTimeout(() => {
    searchTimer = null
    runSearch(reason)
  }, 220)
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

const gotoSearchHit = (idx, reason = 'manual') => {
  if (!searchHits.value.length) {
    logger.debug('PDF搜索跳转跳过：无匹配', { reason })
    return
  }
  const n = searchHits.value.length
  const i = ((idx % n) + n) % n
  searchActiveIdx.value = i
  rebuildSearchRects()
  const hit = searchHits.value[i]
  const rect = hit.rects[0]
  if (rect) scrollToPagePoint(hit.page, rect.y)
  logger.info('PDF搜索命中跳转', { reason, active: i + 1, total: n, page: hit.page })
}
const nextHit = () => gotoSearchHit(searchActiveIdx.value + 1, 'next-hit')
const prevHit = () => gotoSearchHit(searchActiveIdx.value - 1, 'previous-hit')
const clearSearch = (reason = 'manual') => {
  if (searchTimer) {
    clearTimeout(searchTimer)
    searchTimer = null
  }
  searchQuery.value = ''
  resetSearchState()
  logger.info('PDF搜索清空', { reason })
}

const onSearchInputKeydown = async (ev) => {
  if (ev.isComposing) return
  if (ev.key === 'Enter') {
    ev.preventDefault()
    ev.stopPropagation()
    const q = searchQuery.value.trim()
    if (!q) {
      clearSearch('enter-empty')
      return
    }
    // 设计说明：输入内容变更后的第一次 Enter 负责提交搜索并定位到首个结果；
    // 搜索内容未变时，Enter/Shift+Enter 才作为"下一个/上一个"导航，符合常见 PDF 阅读器习惯。
    if (q !== lastSearchedQuery) {
      await runSearch(ev.shiftKey ? 'shift-enter-search' : 'enter-search')
      return
    }
    if (ev.shiftKey) prevHit()
    else nextHit()
    return
  }
  if (ev.key === 'Escape') {
    ev.preventDefault()
    ev.stopPropagation()
    if (searchQuery.value.trim()) {
      clearSearch('escape')
      return
    }
    searchInputRef.value?.blur?.()
    rootRef.value?.focus?.({ preventScroll: true })
    logger.info('PDF搜索框退出', { reason: 'escape' })
  }
}

const onPageInputKeydown = (ev) => {
  if (ev.key === 'Enter') {
    ev.preventDefault()
    ev.stopPropagation()
    commitPageInput()
    rootRef.value?.focus?.({ preventScroll: true })
    logger.info('PDF页码输入提交', { raw: pageInputValue.value })
    return
  }
  if (ev.key === 'Escape') {
    ev.preventDefault()
    ev.stopPropagation()
    syncPageInput()
    ev.currentTarget?.blur?.()
    rootRef.value?.focus?.({ preventScroll: true })
    logger.info('PDF页码输入取消', { page: currentPage.value })
  }
}

// ─────────────────── 反向：点击 PDF -> 源码 ───────────────────
let dragStartedAt = null
const onPageMouseDown = (pageNum, ev) => {
  dragStartedAt = { x: ev.clientX, y: ev.clientY, page: pageNum }
}

// 屏幕坐标 -> 页面"未旋转 pt"坐标：考虑当前旋转角，使选区/点击在旋转后仍正确。
// pageEl(.gpv__page) 是轴对齐的占位框，其中心即旋转内层的中心；先平移到中心、逆旋转，
// 再平移回未旋转的 W×H 空间，最后除以缩放得到 pt。
const screenToPagePt = (page, pageEl, clientX, clientY) => {
  const rect = pageEl.getBoundingClientRect()
  const cx = rect.left + rect.width / 2
  const cy = rect.top + rect.height / 2
  const rad = (-rotation.value * Math.PI) / 180
  const dx = clientX - cx
  const dy = clientY - cy
  const ux = dx * Math.cos(rad) - dy * Math.sin(rad)
  const uy = dx * Math.sin(rad) + dy * Math.cos(rad)
  const s = cssScale.value
  return {
    x: (ux + (page.wPt * s) / 2) / s,
    y: (uy + (page.hPt * s) / 2) / s,
  }
}

const handlePageClick = async (pageNum, ev) => {
  if (!props.features.reverseJump) return
  if (panMode.value) return
  const node = pageEls.get(pageNum)
  if (!node) return
  const page = pages[pageNum - 1]
  if (!page) return
  const { x: xPt, y: yPt } = screenToPagePt(page, node, ev.clientX, ev.clientY)
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
  scheduleRenderVisible()
}

const showFlashBox = (box) => {
  flashBox.value = box
  if (flashTimer) clearTimeout(flashTimer)
  flashTimer = setTimeout(() => { flashBox.value = null }, 1800)
}

// 供宿主按"来源锚点"定位（笔记卡的「回到原文」）：跳到锚点所在页，并把锚点范围框出来。
// rects 是页内 pt 坐标，取并集——一条摘录可能跨多行、多个矩形。
// 拿不到矩形时退化成只跳页：至少落到那一页，比完全回不去有用。
const flashAnchor = ({ page, rects } = {}) => {
  const p = Math.round(Number(page) || 0)
  if (p < 1) return false
  const list = Array.isArray(rects)
    ? rects.filter((r) => r && Number.isFinite(r.x) && Number.isFinite(r.y) && r.w > 0 && r.h > 0)
    : []
  if (!list.length) {
    goToPage(p)
    return true
  }
  const x = Math.min(...list.map((r) => r.x))
  const y = Math.min(...list.map((r) => r.y))
  const w = Math.max(...list.map((r) => r.x + r.w)) - x
  const h = Math.max(...list.map((r) => r.y + r.h)) - y
  scrollToPagePoint(p, y)
  showFlashBox({ page: p, x, y, w, h })
  return true
}

const goToPage = (n, { align = 'top', retry = 6 } = {}) => {
  const num = Math.min(Math.max(1, Math.round(Number(n) || 1)), totalPages.value || 1)
  const el = bodyRef.value
  const node = pageEls.get(num)
  if (!el || !node) {
    // 设计说明：PDF 打开和布局切换时，页数组可能已经建立，但 v-for 的页面 DOM 仍在下一帧才挂载。
    // 此时立即判定失败会让左侧阅读器停在空白态；短暂重试可保持跳页语义，同时不改变后续滚动/渲染逻辑。
    if (retry > 0) {
      logger.debug('PDF页码跳转等待页面节点', {
        targetPage: num,
        retry,
        hasBody: Boolean(el),
        hasPageNode: Boolean(node),
        totalPages: totalPages.value,
      })
      nextTick(() => {
        requestAnimationFrame(() => {
          scheduleLayoutRender('go-to-page-retry')
          goToPage(num, { align, retry: retry - 1 })
        })
      })
      return
    }
    logger.warn('PDF页码跳转失败：页面节点未就绪', { targetPage: num, totalPages: totalPages.value })
    return
  }
  const offset = align === 'center' ? el.clientHeight * 0.3 : 8
  el.scrollTo({ top: Math.max(0, node.offsetTop - offset), behavior: 'smooth' })
  currentPage.value = num
  syncPageInput()
  emit('page-change', num)
  scheduleRenderVisible()
  logger.info('PDF页码跳转完成', { targetPage: num, totalPages: totalPages.value })
}

const syncPageInput = () => {
  pageInputValue.value = String(currentPage.value || 1)
}

const commitPageInput = () => {
  const raw = String(pageInputValue.value ?? '').trim()
  const target = Number(raw)
  if (!Number.isFinite(target)) {
    logger.warn('PDF页码输入无效', { raw })
    syncPageInput()
    return
  }
  goToPage(target)
}

const goPreviousPage = () => {
  if (!canGoPreviousPage.value) return
  goToPage(currentPage.value - 1)
}

const goNextPage = () => {
  if (!canGoNextPage.value) return
  goToPage(currentPage.value + 1)
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
      // 统一用后端的"整行 bbox"来画高亮框，不要直接采信 synctex 自带的盒子：
      // synctex 的 `h` 记录是**词级**盒——width 只覆盖一个词，height/depth 常为 0，
      // 用它渲染出来的高亮只盖住行首那个词（后端又把它 clamp 成 8pt 高），
      // 视觉上就是"高亮条落在文字左上角、没盖住对应内容"。
      // typst 只返回一个点、本来就走 locatePoint 扩成整行，所以那边一直是对的；这里让 LaTeX 对齐。
      let box = { page: res.page, x: res.x, y: res.y, w: res.w || 0, h: res.h || 0 }
      const loc = await doc.locatePoint(res.page, res.x, res.y)
      if (loc && loc.found) box = { page: res.page, x: loc.x, y: loc.y, w: loc.w, h: loc.h }
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
  const page = pages[pageNum - 1]
  if (!page) return null
  const rects = []
  for (const r of sel.getRangeAt(0).getClientRects()) {
    if (r.width < 1 || r.height < 1) continue
    // 每个屏幕矩形映射两对角到页面 pt 空间，取 min/max（旋转后仍得到正确的轴对齐框）。
    const p1 = screenToPagePt(page, pageEl, r.left, r.top)
    const p2 = screenToPagePt(page, pageEl, r.right, r.bottom)
    rects.push({
      x: Math.min(p1.x, p2.x),
      y: Math.min(p1.y, p2.y),
      w: Math.abs(p2.x - p1.x),
      h: Math.abs(p2.y - p1.y),
    })
  }
  if (!rects.length) return null
  return { page: pageNum, rects, text }
}

const DEFAULT_HL_COLOR = [255, 235, 59, 90]
let hlTempSeq = 0
const normalizeHlColor = (c) => {
  if (Array.isArray(c) && c.length >= 3) {
    const v = (n, d) => Math.max(0, Math.min(255, Math.round(Number(n) ?? d)))
    return [v(c[0], 255), v(c[1], 235), v(c[2], 59), v(c[3], 90)]
  }
  return DEFAULT_HL_COLOR.slice()
}

// 乐观高亮：先就地放临时覆盖层（瞬间可见），再后台写回 .pdf，成功后把临时 id 换成真 id，失败回滚。
const addHighlightFromSelection = async ({ color, note } = {}) => {
  const info = getSelectionRects()
  if (!info) return null
  window.getSelection?.()?.removeAllRanges()

  const rgba = normalizeHlColor(color)
  const page = pages[info.page - 1]
  const tempId = `tmp-${++hlTempSeq}`
  if (page) {
    if (!Array.isArray(page.highlights)) page.highlights = []
    page.highlights.push({ annot_id: tempId, rects: info.rects, color: rgba, note: note || '', pending: true })
  }

  const annotId = await doc.addHighlight({ page: info.page, rects: info.rects, color: rgba, note })
  if (!annotId) {
    if (page && Array.isArray(page.highlights)) {
      const i = page.highlights.findIndex((h) => h.annot_id === tempId)
      if (i >= 0) page.highlights.splice(i, 1)
    }
    return null
  }
  if (page && Array.isArray(page.highlights)) {
    const it = page.highlights.find((h) => h.annot_id === tempId)
    if (it) { it.annot_id = annotId; it.pending = false }
  }
  return { annotId, page: info.page, text: info.text, rects: info.rects }
}

// 乐观删除：先就地移除覆盖层，再后台写回；失败回滚。
const deleteHighlight = async (pageNum, annotId) => {
  const page = pages[pageNum - 1]
  let removed = null
  if (page && Array.isArray(page.highlights)) {
    const i = page.highlights.findIndex((h) => h.annot_id === annotId)
    if (i >= 0) { removed = page.highlights[i]; page.highlights.splice(i, 1) }
  }
  const ok = await doc.deleteHighlight(pageNum, annotId)
  if (!ok && removed && page && Array.isArray(page.highlights)) {
    page.highlights.push(removed)
  }
  return ok
}

// 无需 DOM 选区地重建一个高亮（撤销"删除"时用）。乐观插入覆盖层 + 后台写回，返回新 annotId。
const reAddHighlight = async ({ page: pageNum, rects, color, note } = {}) => {
  if (!pageNum || !Array.isArray(rects) || !rects.length) return null
  const rgba = normalizeHlColor(color)
  const page = pages[pageNum - 1]
  const tempId = `tmp-${++hlTempSeq}`
  if (page) {
    if (!Array.isArray(page.highlights)) page.highlights = []
    page.highlights.push({ annot_id: tempId, rects, color: rgba, note: note || '', pending: true })
  }
  const annotId = await doc.addHighlight({ page: pageNum, rects, color: rgba, note })
  if (!annotId) {
    if (page && Array.isArray(page.highlights)) {
      const i = page.highlights.findIndex((h) => h.annot_id === tempId)
      if (i >= 0) page.highlights.splice(i, 1)
    }
    return null
  }
  if (page && Array.isArray(page.highlights)) {
    const it = page.highlights.find((h) => h.annot_id === tempId)
    if (it) { it.annot_id = annotId; it.pending = false }
  }
  return { annotId, page: pageNum, rects, color: rgba, note: note || '' }
}

const refresh = () => load(props.path, true)

const rotateTo = (deg) => { rotation.value = ((Math.round(deg / 90) * 90) % 360 + 360) % 360 }

defineExpose({
  forwardSync,
  goToPage,
  flashAnchor,
  search: runSearch,
  runSearch,
  setSearchQuery: (q) => { searchQuery.value = q },
  nextHit,
  prevHit,
  clearSearch,
  getOutline: doc.getOutline,
  getSelectionInfo,
  getSelectionRects,
  addHighlightFromSelection,
  reAddHighlight,
  deleteHighlight,
  setZoom,
  zoomIn,
  zoomOut,
  fitWidth,
  fitPage,
  refresh,
  scrollRatio,
  restoreScroll,
  restoreScrollStable,
  goPreviousPage,
  goNextPage,
  commitPageInput,
  // 工具栏外置（宿主驱动）所需的状态与操作
  rotateLeft,
  rotateRight,
  rotateTo,
  togglePan: () => { panMode.value = !panMode.value },
  setPanMode: (v) => { panMode.value = !!v },
  setSpacePan: (v) => { spaceHeld.value = !!v },
  state: { zoom, rotation, panMode, spaceHeld, isPanning, currentPage, pageInputValue, totalPages, searchHits, searchActiveIdx, searchQuery },
})

// ───────────────────────── watchers ─────────────────────────
watch(currentPage, () => {
  syncPageInput()
})

watch(totalPages, (nextTotal) => {
  const total = Math.max(1, Number(nextTotal || 1))
  if (currentPage.value > total) {
    currentPage.value = total
  }
  syncPageInput()
})

watch(() => props.path, (p) => {
  const base = String(p || '').split('?')[0]
  const keep = base !== '' && base === lastBase
  lastBase = base
  load(p, keep)
}, { immediate: true })

onMounted(() => {
  nextTick(() => scheduleLayoutRender('mounted'))
  if (typeof ResizeObserver !== 'undefined') {
    // 尺寸变化既要刷新布局，也要按自适应策略重算缩放（非自适应模式下 scheduleAutoFit 直接跳过）。
    resizeObserver = new ResizeObserver(() => {
      scheduleLayoutRender('resize')
      scheduleAutoFit('resize')
    })
    if (rootRef.value) resizeObserver.observe(rootRef.value)
    if (bodyRef.value) resizeObserver.observe(bodyRef.value)
    logger.debug('PDF阅读器尺寸监听已启用')
  } else {
    logger.debug('PDF阅读器尺寸监听不可用，使用挂载刷新兜底')
  }
})

if (typeof window !== 'undefined') {
  window.addEventListener('mouseup', onBodyMouseUp, true)
  window.addEventListener('keydown', onSpaceDown)
  window.addEventListener('keyup', onSpaceUp)
  window.addEventListener('blur', releaseSpacePan)
}

onBeforeUnmount(() => {
  if (flashTimer) clearTimeout(flashTimer)
  if (renderSettleTimer) clearTimeout(renderSettleTimer)
  if (zoomRenderTimer) clearTimeout(zoomRenderTimer)
  if (autoFitTimer) clearTimeout(autoFitTimer)
  if (searchTimer) clearTimeout(searchTimer)
  if (scrollRaf) cancelAnimationFrame(scrollRaf)
  if (resizeRenderRaf) cancelAnimationFrame(resizeRenderRaf)
  if (panRaf) cancelAnimationFrame(panRaf)
  if (resizeObserver) {
    resizeObserver.disconnect()
    resizeObserver = null
  }
  if (typeof window !== 'undefined') {
    window.removeEventListener('mouseup', onBodyMouseUp, true)
    window.removeEventListener('keydown', onSpaceDown)
    window.removeEventListener('keyup', onSpaceUp)
    window.removeEventListener('blur', releaseSpacePan)
    window.removeEventListener('mousemove', onPanMove, true)
    window.removeEventListener('mouseup', onPanUp, true)
  }
  doc.closeDocument()
})
</script>

<style scoped>
.gpv {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
  background: var(--gpv-bg);
  color: var(--gpv-text);
}
.gpv:focus {
  outline: none;
}
.gpv:focus-visible {
  box-shadow: inset 0 0 0 2px color-mix(in srgb, var(--n-color-primary, #61afef) 72%, transparent);
}

.gpv.theme-dark {
  --gpv-bg: #1e1e1e;
  --gpv-text: #ddd;
  --gpv-toolbar-bg: #252525;
  --gpv-border: #3e4451;
  --gpv-canvas-area: #4a4d52;
  --gpv-btn-text: #ddd;
  --gpv-hover-bg: rgba(255, 255, 255, 0.08);
  --gpv-error-bg: rgba(0, 0, 0, 0.35);
  --gpv-overlay-bg: rgba(0, 0, 0, 0.45);
  --gpv-overlay-text: #fff;
  /* 画布氛围：一层自顶部洒下的柔光。固定于可视区（background-attachment: scroll 在
     滚动容器里就是"不随内容走"），所以滚动时纸面像是在同一个光源下移动。 */
  --gpv-canvas-sheen: radial-gradient(125% 85% at 50% 0%, rgba(255, 255, 255, 0.07), rgba(255, 255, 255, 0) 62%);
  --gpv-paper-shadow: 0 1px 1px rgba(0, 0, 0, 0.45), 0 10px 26px -8px rgba(0, 0, 0, 0.7);
  --gpv-paper-ring: rgba(0, 0, 0, 0.5);
  --gpv-skeleton-bg: #3f4248;
  --gpv-skeleton-sheen: rgba(255, 255, 255, 0.06);
  --gpv-skeleton-text: #6d727b;
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
  --gpv-canvas-sheen: radial-gradient(125% 85% at 50% 0%, rgba(255, 255, 255, 0.9), rgba(255, 255, 255, 0) 60%);
  --gpv-paper-shadow: 0 1px 1px rgba(15, 23, 42, 0.08), 0 6px 18px -6px rgba(15, 23, 42, 0.28);
  --gpv-paper-ring: rgba(15, 23, 42, 0.1);
  --gpv-skeleton-bg: #f7f8fa;
  --gpv-skeleton-sheen: rgba(255, 255, 255, 0.85);
  --gpv-skeleton-text: #c3c8d0;
}

.gpv__toolbar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 5px 8px;
  border-bottom: 1px solid var(--gpv-border);
  background: var(--gpv-toolbar-bg);
  font-size: 12px;
  /* 单行固定高度：窗口变窄时不换行（否则文字被挤成多行、工具栏突然变高），
     放不下时横向滚动而非增高。 */
  flex-wrap: nowrap;
  white-space: nowrap;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: none;
}
.gpv__toolbar::-webkit-scrollbar { height: 0; display: none; }
/* 工具栏内所有项都不被压缩换行；标题/状态可省略号，留出可点击控件空间。 */
.gpv__toolbar > * { flex-shrink: 0; }
.gpv__title {
  font-weight: 600;
  color: var(--gpv-text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex-shrink: 1;
  min-width: 0;
  max-width: 40%;
}
.gpv__status {
  font-size: 11px;
  opacity: 0.7;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex-shrink: 1;
  min-width: 0;
}
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
  padding: 3px 8px;
  border: 1px solid var(--gpv-border);
  border-radius: 7px;
  background: color-mix(in srgb, var(--gpv-text) 4%, transparent);
  color: var(--gpv-text);
}
.gpv__search-input:focus { outline: none; border-color: var(--n-color-primary, #61afef); }
.gpv__search-count { font-size: 11px; opacity: 0.7; min-width: 32px; text-align: center; }

.gpv__pager {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  flex: 0 0 auto;
}
.gpv__page-indicator { font-size: 11px; opacity: 0.85; min-width: 58px; text-align: center; display: inline-flex; align-items: center; justify-content: center; gap: 4px; }
.gpv__page-input {
  width: 44px;
  font-size: 11px;
  padding: 2px 4px;
  text-align: center;
  border: 1px solid var(--gpv-border);
  border-radius: 6px;
  background: color-mix(in srgb, var(--gpv-text) 4%, transparent);
  color: var(--gpv-text);
  font-variant-numeric: tabular-nums;
}
.gpv__page-input::-webkit-inner-spin-button,
.gpv__page-input::-webkit-outer-spin-button { -webkit-appearance: none; margin: 0; }
.gpv__page-input:focus { outline: none; border-color: var(--n-color-primary, #61afef); }
.gpv__zoom {
  min-width: 42px;
  padding: 0 2px;
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  text-align: center;
  color: var(--gpv-text);
  opacity: 0.8;
  user-select: none;
}

.gpv__btn {
  border: none;
  background: transparent;
  color: var(--gpv-btn-text);
  border-radius: 6px;
  min-width: 26px;
  height: 24px;
  padding: 0 6px;
  font-size: 12px;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: background-color 0.15s, color 0.15s, box-shadow 0.15s;
}
.gpv__btn:hover:not(:disabled) { background: var(--gpv-hover-bg); }
.gpv__btn:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--n-color-primary, #61afef) 28%, transparent);
}
.gpv__btn:disabled { opacity: 0.35; cursor: not-allowed; }
.gpv__btn--primary {
  color: var(--n-color-primary, #61afef);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--n-color-primary, #61afef) 42%, transparent);
}
.gpv__btn--icon {
  width: 26px;
  height: 24px;
  padding: 0;
  /* 图标按 1em 定尺寸，字号即图标大小（与父组件工具栏里的同款图标对齐） */
  font-size: 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  line-height: 0;
}
.gpv__btn--icon svg {
  display: block;
  flex: 0 0 auto;
}
.gpv__btn--active {
  background: color-mix(in srgb, var(--n-color-primary, #61afef) 18%, transparent);
  color: var(--n-color-primary, #61afef);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--n-color-primary, #61afef) 42%, transparent);
}

/* 缩放收成一个胶囊：− 154% + 是同一件事的三个部分，不该看起来像三个独立按钮 */
.gpv__zoom-cluster {
  display: inline-flex;
  align-items: center;
  gap: 0;
  padding: 2px;
  border: 1px solid var(--gpv-border);
  border-radius: 8px;
  background: color-mix(in srgb, var(--gpv-text) 5%, transparent);
}

.gpv__body {
  flex: 1 1 auto;
  min-height: 0;
  position: relative;
  overflow: auto;
  background-color: var(--gpv-canvas-area);
  background-image: var(--gpv-canvas-sheen);
  background-repeat: no-repeat;
  /* 滚动容器里的 scroll（而非 local）= 背景不随内容走，柔光始终自可视区顶部洒下 */
  background-attachment: scroll;
  /* 默认就是抓手：主流阅读器里"拖空白即平移"是靠光标先告诉用户的。
     文字段自己声明 cursor: text，所以文字上仍是文本光标，两者天然分开。 */
  cursor: grab;
}
/* 锁定平移 / 空格临时平移：连文字上也能拖，因此禁止选中，文字段也换手型 */
.gpv__body--pan { user-select: none; }
.gpv__body--pan .gpv__text-seg { user-select: none; cursor: grab; }
/* 拖动进行中（含中键、空白直接拖）：全区域统一攥拳光标 */
.gpv__body--panning,
.gpv__body--panning .gpv__text-seg { cursor: grabbing; }
/* 非内容区不该显示抓手：错误日志要能选字复制，空态只是提示 */
.gpv__error,
.gpv__error *,
.gpv__empty { cursor: auto; }

.gpv__pages {
  display: flex;
  flex-direction: column;
  /* 不用 align-items:center —— 当某页比视口宽（放大后），居中会把左半部分推入不可滚动的
     负区间，导致只能往右看、左侧内容看不全。改为 align-items:flex-start + 每页 margin:0 auto
     居中：放得下时居中，放不下时贴左、可向右滚动看全整页（左右都能拖动）。 */
  align-items: flex-start;
  /* 让容器至少和最宽的页一样宽，保证横向滚动范围覆盖整页。 */
  width: max-content;
  min-width: 100%;
  gap: 12px;
  padding: 12px;
}

.gpv__page {
  margin-left: auto;
  margin-right: auto;
  position: relative;
  background: #fff;
  border-radius: 2px;
  /* 纸张感：一道贴边的硬阴影压出厚度，一道大范围软阴影托起离开背景的高度，
     再补一圈极淡的描边，白纸边缘在浅色画布上才不会"飘"。 */
  box-shadow: var(--gpv-paper-shadow), 0 0 0 1px var(--gpv-paper-ring);
  flex: 0 0 auto;
  overflow: hidden;
}
/* 旋转内层：尺寸/位移/旋转全部走内联样式（pageInnerStyle） */
.gpv__page-inner { position: absolute; }
.gpv__page-img { display: block; user-select: none; }
/* 未渲染页：不画斜条纹（像施工围挡），改成一道缓慢扫过的微光 + 一枚淡页码，
   既说明"这页正在来"，又和纸面本身是同一个色系。 */
.gpv__page-skeleton {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  color: var(--gpv-skeleton-text);
  background-color: var(--gpv-skeleton-bg);
  background-image: linear-gradient(
    100deg,
    rgba(255, 255, 255, 0) 30%,
    var(--gpv-skeleton-sheen) 50%,
    rgba(255, 255, 255, 0) 70%
  );
  background-size: 220% 100%;
  background-repeat: no-repeat;
  animation: gpv-skeleton-sweep 1.5s ease-in-out infinite;
}
@keyframes gpv-skeleton-sweep {
  0% { background-position: 160% 0; }
  100% { background-position: -60% 0; }
}
@media (prefers-reduced-motion: reduce) {
  .gpv__page-skeleton { animation: none; }
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
/* PDF 文字层是透明叠层，选区色沿用主题变量，和阅读器/编辑器保持一致。 */
.gpv__text-seg::selection {
  background: var(--guaxi-selection-strong-bg, rgba(97, 175, 239, 0.4));
  color: inherit;
}

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
/* 选中的高亮：描边更重、再补一圈外发光，和悬停的细橘线明确区分开。
   注意 .gpv__hl-rect 带 mix-blend-mode: multiply，所以这里靠 outline（不参与混合）
   和自身阴影来表达，不改底色——改底色会被混合模式吃掉。 */
.gpv__hl-rect.is-selected {
  outline: 2px solid var(--n-color-primary, #61afef);
  outline-offset: 1px;
  box-shadow: 0 0 0 4px color-mix(in srgb, var(--n-color-primary, #61afef) 30%, transparent);
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
