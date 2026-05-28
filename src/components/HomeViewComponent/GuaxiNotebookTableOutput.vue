<template>
  <div class="guaxi-notebook-table-output">
    <div
      ref="viewportRef"
      class="guaxi-notebook-table-output__viewport"
      :style="viewportStyle"
      @scroll="handleViewportScroll"
      @wheel.stop="handleViewportWheel"
      @mousemove.stop="handleViewportPointerMove"
      @pointermove.stop="handleViewportPointerMove"
      @mouseleave.stop="handleViewportPointerLeave"
    >
      <div class="guaxi-notebook-table-output__surface" :style="surfaceStyle">
        <div class="guaxi-notebook-table-output__header" :style="gridStyle">
          <div
            v-for="(column, columnIndex) in columns"
            :key="`${tableIdentity}-header-${columnIndex}`"
            class="guaxi-notebook-table-output__header-cell"
            :title="column"
            @mouseenter="handleCellTooltipEnter($event, column)"
            @mousemove="handleCellTooltipMove"
            @mouseleave="hideCellTooltip"
          >
            {{ column }}
          </div>
        </div>
        <div class="guaxi-notebook-table-output__body" :style="bodyStyle">
          <div
            v-for="rowIndex in visibleRowIndexes"
            :key="`${tableIdentity}-${rowIndex}`"
            class="guaxi-notebook-table-output__row"
            :style="getRowStyle(rowIndex)"
          >
            <div
              v-for="(column, columnIndex) in columns"
              :key="`${tableIdentity}-${rowIndex}-${columnIndex}`"
              class="guaxi-notebook-table-output__cell"
              :title="getCellDisplayText(rowIndex, columnIndex)"
              @mouseenter="handleCellTooltipEnter($event, getCellDisplayText(rowIndex, columnIndex))"
              @mousemove="handleCellTooltipMove"
              @mouseleave="hideCellTooltip"
            >
              {{ getCellDisplayText(rowIndex, columnIndex) }}
            </div>
          </div>
        </div>
      </div>
    </div>
    <div
      v-if="cellTooltip.visible"
      class="guaxi-notebook-table-output__tooltip"
      :style="cellTooltipStyle"
    >
      {{ cellTooltip.text }}
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../../utils/logger'

const props = defineProps({
  table: {
    type: Object,
    default: () => ({}),
  },
})

const logger = createLogger('GuaxiNotebookTableOutput')
const viewportRef = ref(null)
const loadedRowsMap = ref(new Map())
const loadError = ref('')
const viewportMetrics = ref({
  clientWidth: 0,
  clientHeight: 0,
  scrollWidth: 0,
  scrollHeight: 0,
  scrollLeft: 0,
  scrollTop: 0,
})
const cellTooltip = ref({
  visible: false,
  text: '',
  x: 0,
  y: 0,
})
const loadingPages = new Set()
let scrollFrameId = 0
let resizeObserver = null
let lastMetricsLogKey = ''

const ROW_HEIGHT = 36
const HEADER_HEIGHT = 36
const OVERSCAN_ROWS = 24
const PAGE_SIZE = 200
const MAX_VIEWPORT_HEIGHT = 460
const MIN_VIEWPORT_HEIGHT = 160
const MIN_COLUMN_WIDTH = 140

const inlineRows = computed(() => (Array.isArray(props.table?.rows) ? props.table.rows : []))
const columns = computed(() => (Array.isArray(props.table?.columns) ? props.table.columns.map((item) => String(item ?? '')) : []))
const columnCount = computed(() => Math.max(1, columns.value.length))
const remoteEnabled = computed(() => !!String(props.table?.resultHandle || '').trim())
const totalRows = computed(() => {
  const explicit = Number(props.table?.rowCount || 0)
  return Math.max(explicit, inlineRows.value.length)
})
const viewportHeight = computed(() => {
  if (!totalRows.value) return MIN_VIEWPORT_HEIGHT
  return Math.min(MAX_VIEWPORT_HEIGHT, Math.max(MIN_VIEWPORT_HEIGHT, totalRows.value * ROW_HEIGHT))
})
const tableWidthPx = computed(() => columnCount.value * MIN_COLUMN_WIDTH)
const viewportStyle = computed(() => ({
  '--guaxi-notebook-table-height': `${viewportHeight.value}px`,
}))

const scrollTop = ref(0)
const startIndex = computed(() => Math.max(0, Math.floor(scrollTop.value / ROW_HEIGHT) - OVERSCAN_ROWS))
const endIndex = computed(() => Math.min(
  totalRows.value,
  Math.ceil((scrollTop.value + viewportHeight.value) / ROW_HEIGHT) + OVERSCAN_ROWS,
))
const visibleRowIndexes = computed(() => {
  const indexes = []
  for (let index = startIndex.value; index < endIndex.value; index += 1) {
    indexes.push(index)
  }
  return indexes
})
const tableIdentity = computed(() => [
  String(props.table?.resultHandle || ''),
  String(props.table?.tableKey || ''),
  String(props.table?.manifestPath || ''),
  String(props.table?.tableName || ''),
].join(':'))
const gridStyle = computed(() => ({
  gridTemplateColumns: `repeat(${columnCount.value}, minmax(${MIN_COLUMN_WIDTH}px, 1fr))`,
}))
const surfaceStyle = computed(() => ({
  width: `${tableWidthPx.value}px`,
  minWidth: '100%',
  minHeight: `${HEADER_HEIGHT + totalRows.value * ROW_HEIGHT}px`,
}))
const bodyStyle = computed(() => ({
  height: `${totalRows.value * ROW_HEIGHT}px`,
}))
const cellTooltipStyle = computed(() => ({
  left: `${cellTooltip.value.x}px`,
  top: `${cellTooltip.value.y}px`,
}))

const resetLoadedRows = () => {
  loadedRowsMap.value = new Map()
  loadError.value = ''
  loadingPages.clear()
  logger.info('[NotebookTable] reset remote row cache', {
    tableKey: String(props.table?.tableKey || ''),
    manifestPath: String(props.table?.manifestPath || ''),
    totalRows: totalRows.value,
    remoteEnabled: remoteEnabled.value,
  })
}

const getInlineRow = (rowIndex) => inlineRows.value[rowIndex] || null

const getRemoteRow = (rowIndex) => {
  return loadedRowsMap.value.get(rowIndex) || null
}

const getResolvedRow = (rowIndex) => {
  if (remoteEnabled.value) {
    return getRemoteRow(rowIndex) || getInlineRow(rowIndex)
  }
  return getInlineRow(rowIndex)
}

const normalizeCellValue = (value) => {
  if (value == null) return ''
  if (typeof value === 'string') return value
  if (typeof value === 'number' || typeof value === 'boolean') return String(value)
  try {
    return JSON.stringify(value)
  } catch {
    return String(value)
  }
}

const getCellDisplayText = (rowIndex, columnIndex) => {
  const row = getResolvedRow(rowIndex)
  if (!row) {
    if (loadError.value) {
      return columnIndex === 0 ? `加载失败：${loadError.value}` : ''
    }
    return columnIndex === 0 ? '加载中...' : ''
  }
  return normalizeCellValue(Array.isArray(row) ? row[columnIndex] : '')
}

const getRowStyle = (rowIndex) => ({
  ...gridStyle.value,
  transform: `translate3d(0, ${rowIndex * ROW_HEIGHT}px, 0)`,
})

const updateViewportMetrics = (reason = 'unknown') => {
  const viewport = viewportRef.value
  if (!(viewport instanceof HTMLElement)) return

  // 设计说明：
  // 1. 这里保留浏览器默认滚动条，真实滚动与可视滚动条都由同一个 viewport 承担。
  // 2. 之前横向条消失的根因是父级 grid/flex 宽度链路没有 `min-width: 0`，viewport 拿到的是被右侧面板裁剪前的宽度。
  // 3. 记录测量值可以在用户反馈截图时确认当前视口是否已经按可见宽度收缩。
  const nextMetrics = {
    clientWidth: viewport.clientWidth,
    clientHeight: viewport.clientHeight,
    scrollWidth: Math.max(viewport.scrollWidth, tableWidthPx.value),
    scrollHeight: Math.max(viewport.scrollHeight, HEADER_HEIGHT + totalRows.value * ROW_HEIGHT),
    scrollLeft: viewport.scrollLeft,
    scrollTop: viewport.scrollTop,
  }
  viewportMetrics.value = nextMetrics

  const metricsLogKey = [
    nextMetrics.clientWidth,
    nextMetrics.clientHeight,
    nextMetrics.scrollWidth,
    nextMetrics.scrollHeight,
    tableWidthPx.value,
  ].join(':')
  if (metricsLogKey !== lastMetricsLogKey) {
    lastMetricsLogKey = metricsLogKey
    logger.info('[NotebookTable] viewport metrics updated', {
      reason,
      tableKey: String(props.table?.tableKey || ''),
      rows: totalRows.value,
      columns: columnCount.value,
      tableWidth: tableWidthPx.value,
      ...nextMetrics,
    })
  }
}

const loadRemoteRows = async (offset, limit) => {
  const pageKey = `${offset}:${limit}`
  if (!remoteEnabled.value || loadingPages.has(pageKey)) return
  loadingPages.add(pageKey)
  try {
    const response = await invoke('python_runtime_get_table_rows', {
      request: {
        handle: String(props.table?.resultHandle || ''),
        sessionId: String(props.table?.sessionId || ''),
        tableKey: String(props.table?.tableKey || ''),
        manifestPath: String(props.table?.manifestPath || ''),
        offset,
        limit,
      },
    })
    if (!response?.ok) {
      throw new Error(String(response?.error || '读取表格数据失败'))
    }

    const rows = Array.isArray(response?.rows) ? response.rows : []
    if (!rows.length) return

    loadError.value = ''
    const nextMap = new Map(loadedRowsMap.value)
    rows.forEach((row, index) => {
      nextMap.set(offset + index, Array.isArray(row) ? row : [])
    })
    loadedRowsMap.value = nextMap
    logger.info('[NotebookTable] loaded remote rows', {
      tableKey: String(props.table?.tableKey || ''),
      offset,
      limit,
      returnedRows: rows.length,
    })
  } catch (error) {
    loadError.value = String(error?.message || error || '读取表格数据失败')
    logger.error('[NotebookTable] load remote rows failed', {
      tableKey: String(props.table?.tableKey || ''),
      manifestPath: String(props.table?.manifestPath || ''),
      offset,
      limit,
      error: loadError.value,
    })
  } finally {
    loadingPages.delete(pageKey)
  }
}

const ensureVisibleWindowLoaded = () => {
  if (!remoteEnabled.value || !totalRows.value) return

  // 设计说明：
  // 1. 这里按固定页大小拉取当前可视区域及其 overscan，避免一次性把整张大表塞进前端。
  // 2. 这样导入/执行仍然保留完整数据，只是传输和渲染改成按需分片，适合大表与多 sheet 场景。
  const firstPage = Math.floor(startIndex.value / PAGE_SIZE)
  const lastPage = Math.floor(Math.max(0, endIndex.value - 1) / PAGE_SIZE)
  for (let pageIndex = firstPage; pageIndex <= lastPage; pageIndex += 1) {
    const offset = pageIndex * PAGE_SIZE
    const limit = Math.min(PAGE_SIZE, Math.max(0, totalRows.value - offset))
    if (limit <= 0) continue

    let pageLoaded = true
    for (let rowIndex = offset; rowIndex < offset + limit; rowIndex += 1) {
      if (!loadedRowsMap.value.has(rowIndex)) {
        pageLoaded = false
        break
      }
    }
    if (!pageLoaded) {
      void loadRemoteRows(offset, limit)
    }
  }
}

const syncViewportScrollTop = (nextScrollTop) => {
  const normalizedScrollTop = Number(nextScrollTop || 0)
  const nextStartIndex = Math.max(0, Math.floor(normalizedScrollTop / ROW_HEIGHT) - OVERSCAN_ROWS)
  const nextEndIndex = Math.min(
    totalRows.value,
    Math.ceil((normalizedScrollTop + viewportHeight.value) / ROW_HEIGHT) + OVERSCAN_ROWS,
  )

  // 设计说明：
  // 1. 视口自身已经负责像素级滚动，Vue 这里只需要在“可见行窗口”跨过行边界时刷新虚拟行。
  // 2. 如果每个滚动像素都改写响应式 scrollTop，表格 spacer、sticky 表头和父级布局会被高频重算，
  //    在 Tauri/Chromium 的嵌入式滚动场景里容易出现逐行扫描式闪烁。
  // 3. 因此这里把刷新粒度收敛到行窗口变化，同时仍然按当前窗口触发远程分页加载。
  if (nextStartIndex === startIndex.value && nextEndIndex === endIndex.value) return
  scrollTop.value = normalizedScrollTop
  ensureVisibleWindowLoaded()
}

const handleViewportWheel = () => {
  // 设计说明：表格内部滚轮只应该驱动表格视口，不能继续冒泡到 Notebook 根层的工具栏感应区。
  // 这里不调用 preventDefault，浏览器仍按原生方式滚动当前表格，避免破坏触控板惯性滚动。
}

const handleViewportPointerMove = () => {
  // 设计说明：Notebook 根容器通过 mousemove 控制顶部工具栏显隐。表格虚拟滚动时 DOM 行会替换，
  // 如果鼠标移动事件继续冒泡，工具栏会在滚动过程中反复显隐并触发中间区域重排。
}

const handleViewportPointerLeave = () => {
  // 设计说明：表格区域的离开事件只属于表格自身，不应被解释为 Notebook 根区域离开。
  hideCellTooltip()
}

const handleViewportScroll = (event) => {
  const nextScrollTop = Number(event?.target?.scrollTop || 0)
  updateViewportMetrics('scroll')
  if (scrollFrameId) {
    window.cancelAnimationFrame(scrollFrameId)
  }
  // 设计说明：
  // 1. 表格滚动时如果每个原生 scroll 事件都同步改写可视窗口，虚拟行和列宽都会高频重算，肉眼会看到轻微闪烁。
  // 2. 这里把滚动同步收敛到每帧一次，让滚动、远程分页加载和 DOM 重排在浏览器同一帧内完成。
  // 3. 这样既保留大表的虚拟滚动能力，也能明显减轻持续向下滚动时整表反复抖动的问题。
  scrollFrameId = window.requestAnimationFrame(() => {
    scrollFrameId = 0
    syncViewportScrollTop(nextScrollTop)
  })
}

const moveCellTooltip = (event) => {
  const viewportWidth = window.innerWidth || 0
  const viewportHeight = window.innerHeight || 0
  const tooltipWidth = 420
  const tooltipHeight = 120
  const nextX = Math.min(Math.max(12, Number(event?.clientX || 0) + 14), Math.max(12, viewportWidth - tooltipWidth - 12))
  const nextY = Math.min(Math.max(12, Number(event?.clientY || 0) + 14), Math.max(12, viewportHeight - tooltipHeight - 12))
  cellTooltip.value = {
    ...cellTooltip.value,
    x: nextX,
    y: nextY,
  }
}

const hideCellTooltip = () => {
  if (!cellTooltip.value.visible) return
  cellTooltip.value = {
    visible: false,
    text: '',
    x: 0,
    y: 0,
  }
}

const handleCellTooltipEnter = (event, rawText) => {
  const target = event?.currentTarget
  const text = String(rawText ?? '')
  if (!(target instanceof HTMLElement) || !text) {
    hideCellTooltip()
    return
  }
  if (target.scrollWidth <= target.clientWidth + 1) {
    hideCellTooltip()
    return
  }

  moveCellTooltip(event)
  cellTooltip.value = {
    ...cellTooltip.value,
    visible: true,
    text,
  }
}

const handleCellTooltipMove = (event) => {
  if (!cellTooltip.value.visible) return
  moveCellTooltip(event)
}

watch(
  () => tableIdentity.value,
  async () => {
    resetLoadedRows()
    scrollTop.value = 0
    await nextTick()
    ensureVisibleWindowLoaded()
    updateViewportMetrics('table-change')
  },
  { immediate: true },
)

watch([startIndex, endIndex, totalRows, remoteEnabled], () => {
  ensureVisibleWindowLoaded()
})

watch([tableWidthPx, viewportHeight], () => {
  nextTick(() => {
    updateViewportMetrics('layout-watch')
  })
})

onMounted(() => {
  ensureVisibleWindowLoaded()
  nextTick(() => {
    updateViewportMetrics('mounted')
    const viewport = viewportRef.value
    if (viewport instanceof HTMLElement && typeof ResizeObserver !== 'undefined') {
      resizeObserver = new ResizeObserver(() => {
        updateViewportMetrics('resize')
      })
      resizeObserver.observe(viewport)
    }
  })
})

onBeforeUnmount(() => {
  if (scrollFrameId) {
    window.cancelAnimationFrame(scrollFrameId)
    scrollFrameId = 0
  }
  if (resizeObserver) {
    resizeObserver.disconnect()
    resizeObserver = null
  }
})
</script>

<style scoped>
.guaxi-notebook-table-output {
  position: relative;
  width: 100%;
  min-width: 0;
}

.guaxi-notebook-table-output__viewport {
  max-height: var(--guaxi-notebook-table-height, 360px);
  overflow-x: auto;
  overflow-y: auto;
  overscroll-behavior: contain;
  contain: layout paint;
  border: none;
  border-radius: 0;
  background: transparent;
  scrollbar-gutter: stable;
  scrollbar-width: auto;
  scrollbar-color: color-mix(in srgb, var(--guaxi-notebook-muted, #94a3b8) 72%, transparent) transparent;
}

.guaxi-notebook-table-output__viewport::-webkit-scrollbar {
  width: 13px;
  height: 13px;
}

.guaxi-notebook-table-output__viewport::-webkit-scrollbar-track {
  background: color-mix(in srgb, var(--guaxi-notebook-panel, #ffffff) 72%, transparent);
}

.guaxi-notebook-table-output__viewport::-webkit-scrollbar-thumb {
  min-width: 48px;
  min-height: 48px;
  border: 3px solid transparent;
  border-radius: 999px;
  background: color-mix(in srgb, var(--guaxi-notebook-muted, #94a3b8) 72%, var(--guaxi-notebook-primary, #3b82f6) 14%);
  background-clip: content-box;
}

.guaxi-notebook-table-output__viewport::-webkit-scrollbar-thumb:hover {
  background: color-mix(in srgb, var(--guaxi-notebook-muted, #94a3b8) 62%, var(--guaxi-notebook-primary, #3b82f6) 28%);
  background-clip: content-box;
}

.guaxi-notebook-table-output__viewport::-webkit-scrollbar-corner {
  background: color-mix(in srgb, var(--guaxi-notebook-panel, #ffffff) 72%, transparent);
}

.guaxi-notebook-table-output__surface {
  position: relative;
  min-width: 100%;
  contain: layout paint;
}

.guaxi-notebook-table-output__header {
  position: sticky;
  top: 0;
  z-index: 2;
  display: grid;
  min-height: 36px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft, #f8fafc) 78%, var(--guaxi-notebook-panel, #ffffff) 22%);
}

.guaxi-notebook-table-output__body {
  position: relative;
  margin-top: 0;
  min-width: 100%;
  contain: layout paint;
}

.guaxi-notebook-table-output__row {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  display: grid;
  height: 36px;
  min-width: 100%;
  will-change: transform;
}

.guaxi-notebook-table-output__header-cell,
.guaxi-notebook-table-output__cell {
  min-width: 0;
  height: 36px;
  padding: 8px 10px;
  border-bottom: 1px solid var(--guaxi-notebook-border, rgba(148, 163, 184, 0.22));
  border-right: 1px solid var(--guaxi-notebook-border, rgba(148, 163, 184, 0.22));
  text-align: left;
  white-space: nowrap;
  line-height: 20px;
  box-sizing: border-box;
  overflow: hidden;
  text-overflow: ellipsis;
}

.guaxi-notebook-table-output__header-cell:first-child,
.guaxi-notebook-table-output__cell:first-child {
  border-left: 1px solid var(--guaxi-notebook-border, rgba(148, 163, 184, 0.22));
}

.guaxi-notebook-table-output__header-cell {
  font-weight: 700;
  color: var(--guaxi-notebook-text, #0f172a);
}

.guaxi-notebook-table-output__cell {
  color: var(--guaxi-notebook-text, #0f172a);
}

.guaxi-notebook-table-output__tooltip {
  position: fixed;
  z-index: 10000;
  max-width: min(420px, calc(100vw - 24px));
  max-height: 120px;
  padding: 8px 10px;
  border: 1px solid var(--guaxi-notebook-strong-border, rgba(148, 163, 184, 0.36));
  border-radius: 6px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel, #ffffff) 96%, #0f172a 4%);
  color: var(--guaxi-notebook-text, #0f172a);
  box-shadow: 0 12px 28px rgba(15, 23, 42, 0.24);
  font-size: 12px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
  overflow: auto;
  pointer-events: none;
}
</style>
