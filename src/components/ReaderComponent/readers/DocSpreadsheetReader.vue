<template>
  <div class="guaxi-doc-reader" data-reader="spreadsheet">
    <div class="guaxi-doc-reader__bar">
      <span class="guaxi-doc-reader__title" :title="path">{{ name }}</span>
      <span class="guaxi-doc-reader__badge">{{ kind.toUpperCase() }}</span>
      <span v-if="activeSheet" class="guaxi-doc-reader__meta">
        {{ activeSheet.rows.length }} 行 × {{ maxCols }} 列
      </span>
      <span class="guaxi-doc-reader__spacer" />
      <label class="guaxi-doc-reader__meta" style="display: inline-flex; align-items: center; gap: 4px">
        <input v-model="firstRowAsHeader" type="checkbox" />
        首行为表头
      </label>
      <button class="guaxi-doc-reader__btn" title="跳到首行" @click="scrollToRow(0)">⤒</button>
    </div>

    <div v-if="loading" class="guaxi-doc-reader__body">
      <div class="guaxi-doc-reader__center">正在读取表格…</div>
    </div>
    <div v-else-if="errorText" class="guaxi-doc-reader__body">
      <div class="guaxi-doc-reader__center">
        <div class="guaxi-doc-reader__error">{{ errorText }}</div>
      </div>
    </div>
    <div v-else class="guaxi-doc-reader__body gdr-sheet">
      <div v-if="sheets.length > 1" class="gdr-sheet__tabs">
        <button
          v-for="(sheet, idx) in sheets"
          :key="`${idx}-${sheet.name}`"
          class="gdr-sheet__tab"
          :class="{ 'is-active': idx === activeSheetIdx }"
          @click="activeSheetIdx = idx"
        >
          {{ sheet.name || `Sheet${idx + 1}` }}
        </button>
      </div>
      <div v-if="payloadTruncated" class="gdr-sheet__notice">
        表格较大，已截断到前 {{ MAX_ROWS_DISPLAY }} 行显示；完整内容可用 Python Cell 读取。
      </div>
      <div ref="scrollRef" class="gdr-sheet__scroll" @scroll.passive="onScroll">
        <table>
          <colgroup>
            <col style="width: 56px" />
            <col v-for="c in maxCols" :key="`col-${c}`" style="width: 140px" />
          </colgroup>
          <thead>
            <tr>
              <th class="is-rowhead">#</th>
              <th v-for="c in maxCols" :key="`h-${c}`">
                {{ firstRowAsHeader ? headerCell(c - 1) : columnLabel(c - 1) }}
              </th>
            </tr>
          </thead>
          <tbody>
            <!-- 纯 HTML 表格 + 窗口化行渲染：只渲染视口附近的行，百万行也不会卡死。
                 这里用内联的占位行撑出总高度，滚动时按偏移换算可见区间。 -->
            <tr v-if="topSpacer > 0" :style="{ height: `${topSpacer}px` }">
              <td :colspan="maxCols + 1" style="padding: 0; border: 0" />
            </tr>
            <tr v-for="rowIndex in visibleRows" :key="`r-${rowIndex}`">
              <td class="is-rowhead">{{ rowIndex + 1 }}</td>
              <td v-for="c in maxCols" :key="`c-${rowIndex}-${c}`" :title="cellAt(rowIndex, c - 1)">
                {{ cellAt(rowIndex, c - 1) }}
              </td>
            </tr>
            <tr v-if="bottomSpacer > 0" :style="{ height: `${bottomSpacer}px` }">
              <td :colspan="maxCols + 1" style="padding: 0; border: 0" />
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../../../utils/logger'
import './readerPane.css'

const logger = createLogger('DocSpreadsheetReader')

const props = defineProps({
  path: { type: String, required: true },
  name: { type: String, default: '' },
  kind: { type: String, default: 'xlsx' },
})

// 行高与后端上限保持一致（后端 200_000，前端只渲染可视区）。
const ROW_HEIGHT = 22
const OVERSCAN = 12
const MAX_ROWS_DISPLAY = 200000

const loading = ref(true)
const errorText = ref('')
const sheets = ref([])
const activeSheetIdx = ref(0)
const firstRowAsHeader = ref(true)
const payloadTruncated = ref(false)
const scrollRef = ref(null)
const scrollTop = ref(0)
const viewportHeight = ref(600)

let loadToken = 0

const activeSheet = computed(() => sheets.value[activeSheetIdx.value] || null)

const maxCols = computed(() => {
  const sheet = activeSheet.value
  if (!sheet) return 0
  let max = 0
  for (const row of sheet.rows) {
    if (row.length > max) max = row.length
    if (max >= 512) break
  }
  return Math.max(1, Math.min(512, max))
})

const dataRowCount = computed(() => {
  const sheet = activeSheet.value
  if (!sheet) return 0
  return Math.max(0, sheet.rows.length - (firstRowAsHeader.value ? 1 : 0))
})

const startRow = computed(() => (firstRowAsHeader.value ? 1 : 0))

const visibleRange = computed(() => {
  const start = Math.max(0, Math.floor(scrollTop.value / ROW_HEIGHT) - OVERSCAN)
  const count = Math.ceil(viewportHeight.value / ROW_HEIGHT) + OVERSCAN * 2
  const end = Math.min(dataRowCount.value, start + count)
  return { start, end }
})

const visibleRows = computed(() => {
  const { start, end } = visibleRange.value
  const rows = []
  for (let i = start; i < end; i += 1) rows.push(startRow.value + i)
  return rows
})

const topSpacer = computed(() => visibleRange.value.start * ROW_HEIGHT)
const bottomSpacer = computed(() => {
  const remaining = dataRowCount.value - visibleRange.value.end
  return Math.max(0, remaining * ROW_HEIGHT)
})

const columnLabel = (index) => {
  // 0 → A, 25 → Z, 26 → AA（与电子表格软件一致）。
  let n = index
  let out = ''
  do {
    out = String.fromCharCode(65 + (n % 26)) + out
    n = Math.floor(n / 26) - 1
  } while (n >= 0)
  return out
}

const headerCell = (col) => {
  const sheet = activeSheet.value
  if (!sheet || !sheet.rows.length) return columnLabel(col)
  const value = sheet.rows[0]?.[col]
  return value && String(value).trim() ? String(value) : columnLabel(col)
}

const cellAt = (rowIndex, col) => {
  const sheet = activeSheet.value
  if (!sheet) return ''
  const row = sheet.rows[rowIndex]
  if (!row) return ''
  return row[col] ?? ''
}

const onScroll = () => {
  const el = scrollRef.value
  if (!el) return
  scrollTop.value = el.scrollTop
  viewportHeight.value = el.clientHeight || viewportHeight.value
}

const scrollToRow = (rowIndex) => {
  const el = scrollRef.value
  if (!el) return
  el.scrollTop = Math.max(0, (rowIndex - startRow.value) * ROW_HEIGHT)
  onScroll()
}

const loadSheet = async () => {
  const token = ++loadToken
  loading.value = true
  errorText.value = ''
  sheets.value = []
  activeSheetIdx.value = 0

  try {
    const payload = await invoke('read_spreadsheet', { path: props.path })
    if (token !== loadToken) return
    sheets.value = Array.isArray(payload?.sheets) ? payload.sheets : []
    payloadTruncated.value = !!payload?.truncated
    if (!sheets.value.length) throw new Error('没有可显示的工作表')
    loading.value = false
    logger.info('[DocSpreadsheetReader] loaded', {
      path: props.path,
      sheets: sheets.value.length,
      truncated: payloadTruncated.value,
    })
    await nextTick()
    onScroll()
  } catch (error) {
    if (token !== loadToken) return
    loading.value = false
    errorText.value = String(error?.message || error || error)
    logger.warn('[DocSpreadsheetReader] load failed', {
      path: props.path,
      error: String(error?.message || error),
    })
  }
}

let resizeObserver = null

onMounted(() => {
  void loadSheet()
  if (typeof ResizeObserver !== 'undefined' && scrollRef.value) {
    resizeObserver = new ResizeObserver(() => onScroll())
    resizeObserver.observe(scrollRef.value)
  }
})

onBeforeUnmount(() => {
  loadToken += 1
  resizeObserver?.disconnect()
  resizeObserver = null
})

watch(activeSheetIdx, async () => {
  await nextTick()
  if (scrollRef.value) scrollRef.value.scrollTop = 0
  scrollTop.value = 0
  onScroll()
})

watch(firstRowAsHeader, () => {
  if (scrollRef.value) scrollRef.value.scrollTop = 0
  scrollTop.value = 0
})

watch(
  () => props.path,
  () => {
    void loadSheet()
  }
)
</script>
