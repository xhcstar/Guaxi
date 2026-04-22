<template>
  <div class="guaxi-notebook-table-output">
    <div
      ref="viewportRef"
      class="guaxi-notebook-table-output__viewport"
      :style="{ '--guaxi-notebook-table-height': `${viewportHeight}px` }"
      @scroll="handleViewportScroll"
    >
      <table class="guaxi-notebook-table-output__table">
        <thead>
          <tr>
            <th v-for="column in columns" :key="column">{{ column }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="topSpacerPx > 0" class="is-spacer" aria-hidden="true">
            <td :colspan="columnCount" :style="{ height: `${topSpacerPx}px` }"></td>
          </tr>
          <tr
            v-for="rowIndex in visibleRowIndexes"
            :key="`${tableIdentity}-${rowIndex}`"
            class="guaxi-notebook-table-output__row"
          >
            <td
              v-for="(column, columnIndex) in columns"
              :key="`${tableIdentity}-${rowIndex}-${columnIndex}`"
              :title="getCellDisplayText(rowIndex, columnIndex)"
            >
              {{ getCellDisplayText(rowIndex, columnIndex) }}
            </td>
          </tr>
          <tr v-if="bottomSpacerPx > 0" class="is-spacer" aria-hidden="true">
            <td :colspan="columnCount" :style="{ height: `${bottomSpacerPx}px` }"></td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onMounted, ref, watch } from 'vue'
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
const refreshTick = ref(0)
const loadedRowsMap = ref(new Map())
const loadingPages = new Set()

const ROW_HEIGHT = 36
const OVERSCAN_ROWS = 24
const PAGE_SIZE = 200
const MAX_VIEWPORT_HEIGHT = 460
const MIN_VIEWPORT_HEIGHT = 160

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
const topSpacerPx = computed(() => startIndex.value * ROW_HEIGHT)
const bottomSpacerPx = computed(() => Math.max(0, (totalRows.value - endIndex.value) * ROW_HEIGHT))
const tableIdentity = computed(() => [
  String(props.table?.resultHandle || ''),
  String(props.table?.tableKey || ''),
  String(props.table?.manifestPath || ''),
  String(props.table?.tableName || ''),
].join(':'))

const bumpRefresh = () => {
  refreshTick.value += 1
}

const resetLoadedRows = () => {
  loadedRowsMap.value = new Map()
  loadingPages.clear()
  bumpRefresh()
}

const getInlineRow = (rowIndex) => inlineRows.value[rowIndex] || null

const getRemoteRow = (rowIndex) => {
  refreshTick.value
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
    return columnIndex === 0 ? '加载中...' : ''
  }
  return normalizeCellValue(Array.isArray(row) ? row[columnIndex] : '')
}

const loadRemoteRows = async (offset, limit) => {
  const pageKey = `${offset}:${limit}`
  if (!remoteEnabled.value || loadingPages.has(pageKey)) return
  loadingPages.add(pageKey)
  try {
    const response = await invoke('python_runtime_get_table_rows', {
      request: {
        handle: String(props.table?.resultHandle || ''),
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

    const nextMap = new Map(loadedRowsMap.value)
    rows.forEach((row, index) => {
      nextMap.set(offset + index, Array.isArray(row) ? row : [])
    })
    loadedRowsMap.value = nextMap
    bumpRefresh()
    logger.info('[NotebookTable] loaded remote rows', {
      tableKey: String(props.table?.tableKey || ''),
      offset,
      limit,
      returnedRows: rows.length,
    })
  } catch (error) {
    logger.error('[NotebookTable] load remote rows failed', {
      tableKey: String(props.table?.tableKey || ''),
      offset,
      limit,
      error: String(error?.message || error),
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

const handleViewportScroll = (event) => {
  scrollTop.value = Number(event?.target?.scrollTop || 0)
  ensureVisibleWindowLoaded()
}

watch(
  () => tableIdentity.value,
  async () => {
    resetLoadedRows()
    scrollTop.value = 0
    await nextTick()
    ensureVisibleWindowLoaded()
  },
  { immediate: true },
)

watch(
  () => [startIndex.value, endIndex.value, totalRows.value, remoteEnabled.value],
  () => {
    ensureVisibleWindowLoaded()
  },
)

onMounted(() => {
  ensureVisibleWindowLoaded()
})
</script>

<style scoped>
.guaxi-notebook-table-output {
  width: 100%;
  min-width: 0;
}

.guaxi-notebook-table-output__viewport {
  max-height: var(--guaxi-notebook-table-height, 360px);
  overflow: auto;
  border: 1px solid var(--guaxi-notebook-strong-border, rgba(148, 163, 184, 0.3));
  border-radius: 12px;
  background: var(--guaxi-notebook-panel-soft, #f8fafc);
}

.guaxi-notebook-table-output__table {
  width: max-content;
  min-width: 100%;
  border-collapse: collapse;
}

.guaxi-notebook-table-output__table thead th {
  position: sticky;
  top: 0;
  z-index: 1;
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft, #f8fafc) 78%, var(--guaxi-notebook-panel, #ffffff) 22%);
}

.guaxi-notebook-table-output__table th,
.guaxi-notebook-table-output__table td {
  min-width: 120px;
  height: 36px;
  padding: 8px 10px;
  border-bottom: 1px solid var(--guaxi-notebook-border, rgba(148, 163, 184, 0.22));
  text-align: left;
  vertical-align: top;
  white-space: nowrap;
}

.guaxi-notebook-table-output__table td {
  color: var(--guaxi-notebook-text, #0f172a);
}

.guaxi-notebook-table-output__table tr.is-spacer td {
  padding: 0;
  border-bottom: none;
  background: transparent;
}
</style>
