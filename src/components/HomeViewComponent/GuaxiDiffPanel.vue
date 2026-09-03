<template>
  <div class="guaxi-diff-panel">
    <div class="dp-head">
      <div class="dp-row dp-row-main">
        <n-tag size="small" round :bordered="false" class="dp-mode">{{ modeLabel }}</n-tag>

        <button type="button" class="dp-slot" :title="store.left.path || '点击选择'"
          :class="{ 'is-empty': !leftText }" @click="pickSide('left')">
          <Icon :icon="isDirMode ? folderIcon : fileIcon" width="13" height="13" />
          <span class="dp-slot-text">{{ leftText || '选择左侧…' }}</span>
        </button>

        <n-button text size="tiny" title="交换两侧" @click="onSwap">
          <Icon :icon="swapIcon" width="15" height="15" />
        </n-button>

        <button type="button" class="dp-slot" :title="store.right.path || '点击选择'"
          :class="{ 'is-empty': !rightText }" @click="pickSide('right')">
          <Icon :icon="isDirMode ? folderIcon : fileIcon" width="13" height="13" />
          <span class="dp-slot-text">{{ rightText || '选择右侧…' }}</span>
        </button>

        <div class="dp-actions">
          <n-button size="tiny" secondary :disabled="!canRun" :loading="busy" @click="run">对比</n-button>
          <n-button v-if="busy && isDirMode" size="tiny" tertiary type="warning" @click="cancelDir">中止</n-button>
          <n-button text size="tiny" :title="showOptions ? '收起选项' : '对比选项'"
            :class="{ 'is-on': showOptions }" @click="showOptions = !showOptions">
            <Icon :icon="settingsIcon" width="15" height="15" />
          </n-button>
          <n-button text size="tiny" title="关闭" @click="emit('close')">
            <Icon :icon="closeIcon" width="15" height="15" />
          </n-button>
        </div>
      </div>
      <div v-if="showOptions" class="dp-row dp-row-options">
        <template v-if="!isDirMode">
          <label class="dp-field">
            算法
            <n-select v-model:value="opts.algorithm" size="tiny" :options="algorithmOptions" style="width: 104px" />
          </label>
          <label class="dp-field">
            上下文
            <n-input-number v-model:value="opts.contextLines" size="tiny" :min="0" :max="200"
              :show-button="false" style="width: 54px" />
          </label>
          <n-checkbox v-model:checked="opts.ignoreWhitespace" size="small">忽略空白</n-checkbox>
          <n-checkbox v-model:checked="opts.ignoreTrailingWhitespace" size="small">忽略行尾空白</n-checkbox>
          <n-checkbox v-model:checked="opts.ignoreCase" size="small">忽略大小写</n-checkbox>
          <n-checkbox v-model:checked="opts.ignoreBlankLines" size="small">忽略空行</n-checkbox>
          <span class="dp-sep" />
          <n-radio-group v-model:value="viewMode" size="small">
            <n-radio-button value="split">并排</n-radio-button>
            <n-radio-button value="unified">统一</n-radio-button>
          </n-radio-group>
        </template>
        <template v-else>
          <n-checkbox v-model:checked="dirOpts.quick" size="small" title="只比大小和修改时间，不读内容">
            快速模式
          </n-checkbox>
          <n-checkbox v-model:checked="dirOpts.detectRenames" size="small">检测改名</n-checkbox>
          <n-checkbox v-model:checked="dirOpts.includeHidden" size="small">含隐藏文件</n-checkbox>
          <n-checkbox v-model:checked="dirOpts.respectGitignore" size="small">遵循 .gitignore</n-checkbox>
          <n-checkbox v-model:checked="dirOpts.includeSame" size="small">显示相同项</n-checkbox>
          <label class="dp-field">
            排除
            <n-input v-model:value="excludeText" size="tiny" placeholder="node_modules,*.log"
              style="width: 168px" />
          </label>
        </template>
      </div>
      <div class="dp-row dp-row-status">
        <template v-if="drillRel">
          <n-button text size="tiny" class="dp-back" @click="backToDir">
            <Icon :icon="backIcon" width="14" height="14" />
            返回目录差异
          </n-button>
          <span class="dp-status-rel" :title="drillRel">{{ drillRel }}</span>
          <span class="dp-sep" />
        </template>

        <template v-if="busy">
          <span class="dp-status-text">{{ progressText }}</span>
        </template>
        <template v-else-if="error">
          <span class="dp-status-text is-error" :title="error">{{ error }}</span>
        </template>
        <template v-else-if="isDirMode && dirSummary">
          <span class="dp-badge is-add">+{{ dirSummary.added }}</span>
          <span class="dp-badge is-del">−{{ dirSummary.removed }}</span>
          <span class="dp-badge is-mod">±{{ dirSummary.modified }}</span>
          <span v-if="dirSummary.renamed" class="dp-badge is-ren">↷{{ dirSummary.renamed }}</span>
          <span v-if="dirSummary.type_changed" class="dp-badge is-type">⇄{{ dirSummary.type_changed }}</span>
          <span class="dp-status-text">
            共 {{ dirTotal }} 条 · 扫描 {{ dirSummary.left_files }}/{{ dirSummary.right_files }} 个文件
            · 读取 {{ formatSize(dirSummary.compared_bytes) }}
            · {{ dirSummary.elapsed_ms }} ms
          </span>
          <span v-if="dirSummary.cancelled" class="dp-status-text is-warn">已中止，结果不完整</span>
          <span v-else-if="dirSummary.entries_truncated" class="dp-status-text is-warn">条目已截断</span>
        </template>
        <template v-else-if="fileResult">
          <span class="dp-badge is-add">+{{ fileResult.stats.added }}</span>
          <span class="dp-badge is-del">−{{ fileResult.stats.removed }}</span>
          <span class="dp-status-text">
            {{ fileResult.stats.hunks }} 处改动 ·
            {{ fileResult.left.line_count }} → {{ fileResult.right.line_count }} 行 ·
            {{ formatSize(fileResult.left.size) }} / {{ formatSize(fileResult.right.size) }} ·
            {{ fileResult.elapsed_ms }} ms
          </span>
          <span v-if="fileResult.message" class="dp-status-text is-warn" :title="fileResult.message">
            {{ fileResult.message }}
          </span>
          <span v-if="fileResult.hunks_truncated" class="dp-status-text is-warn">显示已截断</span>
        </template>
        <span v-else class="dp-status-text">选择两侧后点"对比"</span>
      </div>
    </div>

    <div v-if="showDirList && sessionId" class="dp-row dp-row-filter">
      <n-input v-model:value="dirQuery" size="tiny" clearable placeholder="过滤路径关键字"
        style="width: 190px" @update:value="onFilterChanged" />
      <n-checkbox-group v-model:value="statusFilter" @update:value="onFilterChanged">
        <n-checkbox v-for="s in statusChoices" :key="s.key" :value="s.key" size="small">
          {{ s.label }}
        </n-checkbox>
      </n-checkbox-group>
      <span class="dp-status-text">{{ dirEntries.length }} / {{ dirTotal }}</span>
    </div>

    <div ref="bodyRef" class="dp-body" @scroll.passive="onScroll">
      <div v-if="busy && !rows.length" class="dp-placeholder">
        <n-spin size="small" />
        <span>{{ progressText }}</span>
      </div>
      <div v-else-if="error" class="dp-placeholder is-error">{{ error }}</div>
      <div v-else-if="!rows.length" class="dp-placeholder">{{ emptyText }}</div>
      <div v-else class="dp-spacer" :style="{ height: `${totalHeight}px`, width: contentWidth }">
        <div class="dp-window" :style="{ transform: `translateY(${offsetY}px)` }">
          <template v-if="showDirList">
            <div v-for="row in visibleRows" :key="row.key" class="dp-entry"
              :class="[`is-${row.entry.status}`, { 'is-dir-entry': row.entry.is_dir }]"
              :title="entryTitle(row.entry)" @click="openEntry(row.entry)">
              <span class="dp-entry-mark">{{ statusMark(row.entry.status) }}</span>
              <Icon :icon="row.entry.is_dir ? folderIcon : fileIcon" width="13" height="13" class="dp-entry-icon" />
              <span class="dp-entry-rel">{{ row.entry.rel }}</span>
              <span v-if="row.entry.renamed_from" class="dp-entry-extra">← {{ row.entry.renamed_from }}</span>
              <span v-if="row.entry.child_count" class="dp-entry-extra">含 {{ row.entry.child_count }} 项</span>
              <span class="dp-entry-size">{{ entrySizeText(row.entry) }}</span>
            </div>
          </template>
          <template v-else>
            <div v-for="row in paintedRows" :key="row.key" class="dp-line" :class="row.cls">
              <span v-if="row.kind === 'hunk'" class="dp-hunk">{{ row.header }}</span>
              <template v-else-if="viewMode === 'split'">
                <span class="dp-no">{{ row.left.no || '' }}</span>
                <span class="dp-cell" :class="row.left.cls" :style="cellStyle">
                  <span v-for="(seg, i) in row.left.segs" :key="i" :class="{ 'dp-ink': seg.hit }">{{ seg.t }}</span>
                </span>
                <span class="dp-no">{{ row.right.no || '' }}</span>
                <span class="dp-cell" :class="row.right.cls" :style="cellStyle">
                  <span v-for="(seg, i) in row.right.segs" :key="i" :class="{ 'dp-ink': seg.hit }">{{ seg.t }}</span>
                </span>
              </template>
              <template v-else>
                <span class="dp-no">{{ row.left.no || '' }}</span>
                <span class="dp-no">{{ row.right.no || '' }}</span>
                <span class="dp-sign">{{ row.sign }}</span>
                <span class="dp-cell" :class="row.cell.cls" :style="cellStyle">
                  <span v-for="(seg, i) in row.cell.segs" :key="i" :class="{ 'dp-ink': seg.hit }">{{ seg.t }}</span>
                </span>
              </template>
            </div>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>
<script setup>
/**
 * 差异对比面板：任意两个文件 / 两个目录的内容差异，和 Git 无关。
 *
 * 性能取舍（对应 Rust 侧 commands/diffcompare.rs）：
 * 1. 差异一律由 Rust 算完再回传 hunk，前端不做行级 diff，也不给 Monaco 建巨型 model；
 *    100 MB 文件在 Monaco 里必崩，这里只渲染视口内的那几十行。
 * 2. 行是等高的，走自绘虚拟滚动：40 万行结果和 40 行结果的渲染成本一样。
 * 3. 目录结果留在 Rust 侧的 LRU 会话里，前端按页取，过滤和搜索也在 Rust 侧做。
 * 4. 行内高亮区间由 Rust 按 UTF-16 code unit 给出，JS 可以直接 slice。
 */
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import {
  NButton, NCheckbox, NCheckboxGroup, NInput, NInputNumber,
  NRadioButton, NRadioGroup, NSelect, NSpin, NTag, useMessage,
} from 'naive-ui'
import { Icon } from '@iconify/vue'
import fileIcon from '@iconify-icons/lucide/file-text'
import folderIcon from '@iconify-icons/lucide/folder'
import swapIcon from '@iconify-icons/lucide/arrow-left-right'
import settingsIcon from '@iconify-icons/lucide/sliders-horizontal'
import closeIcon from '@iconify-icons/lucide/x'
import backIcon from '@iconify-icons/lucide/corner-up-left'
import { createLogger } from '../../utils/logger'
import { useDiffCompareStore } from '../../stores/diffCompareStore'

const logger = createLogger('GuaxiDiffPanel')
const message = useMessage()
const store = useDiffCompareStore()

const props = defineProps({
  visible: { type: Boolean, default: false },
})
const emit = defineEmits(['close'])

const ROW_H = 20
const OVERSCAN = 14
const DIR_PAGE = 500
const CHAR_FALLBACK = 7.2

const showOptions = ref(false)
const viewMode = ref('split')
const busy = ref(false)
const error = ref('')

const fileResult = ref(null)
const rows = ref([])
const maxCols = ref(80)

const sessionId = ref('')
const dirSummary = ref(null)
const dirEntries = ref([])
const dirTotal = ref(0)
const dirHasMore = ref(false)
const dirLoadingPage = ref(false)
const dirQuery = ref('')
const statusFilter = ref([])
const progress = reactive({ phase: '', scanned: 0, compared: 0, elapsed: 0 })

/** 从目录结果点进某一条时记住相对路径，返回时不用重扫。 */
const drillRel = ref('')

const opts = reactive({
  algorithm: 'patience',
  contextLines: 3,
  ignoreWhitespace: false,
  ignoreTrailingWhitespace: false,
  ignoreCase: false,
  ignoreBlankLines: false,
})

const dirOpts = reactive({
  quick: false,
  detectRenames: false,
  includeHidden: false,
  respectGitignore: false,
  includeSame: false,
})
const excludeText = ref('')

const algorithmOptions = [
  { label: 'Patience', value: 'patience' },
  { label: 'Myers', value: 'myers' },
  { label: 'LCS', value: 'lcs' },
]

const statusChoices = [
  { key: 'modified', label: '已改' },
  { key: 'added', label: '新增' },
  { key: 'removed', label: '删除' },
  { key: 'renamed', label: '改名' },
  { key: 'type_changed', label: '类型变' },
  { key: 'same', label: '相同' },
]
const isDirMode = computed(() => store.mode === 'dir')
const showDirList = computed(() => isDirMode.value && !drillRel.value)
const leftText = computed(() => store.leftLabel)
const rightText = computed(() => store.rightLabel)
const modeLabel = computed(() => {
  if (isDirMode.value) return '目录对比'
  if (store.mode === 'text') return '文本对比'
  return store.mode === 'buffer' ? '未保存改动' : '文件对比'
})
const canRun = computed(() => {
  if (busy.value) return false
  if (store.mode === 'text') return store.left.text != null && store.right.text != null
  if (store.mode === 'buffer') return !!store.left.path && store.right.text != null
  return !!(store.left.path && store.right.path)
})
const emptyText = computed(() => {
  if (!store.left.path && !store.right.path && store.mode !== 'text') return '选择两侧后点"对比"'
  if (showDirList.value && sessionId.value) return '没有符合条件的差异'
  if (fileResult.value?.identical) return fileResult.value.message || '两侧内容完全相同'
  if (fileResult.value?.mode === 'binary') return fileResult.value.message || '二进制文件，不做行级对比'
  if (fileResult.value?.mode === 'too_large') return fileResult.value.message || '文件超过大小上限，只对比元信息'
  if (fileResult.value?.mode === 'too_divergent') return fileResult.value.message || '差异过大，只给统计'
  return '暂无差异内容'
})
const progressText = computed(() => {
  if (!isDirMode.value) return '正在计算差异…'
  const phase = { scan: '扫描目录', compare: '比对内容', rename: '匹配改名' }[progress.phase] || '准备中'
  return `${phase}：已扫描 ${progress.scanned} 项 · 已比对 ${progress.compared} 个文件 · ${progress.elapsed} ms`
})

// ───── 虚拟滚动 ─────
const bodyRef = ref(null)
const scrollTop = ref(0)
const viewportH = ref(360)
const charW = ref(CHAR_FALLBACK)

const totalHeight = computed(() => rows.value.length * ROW_H)
const startIndex = computed(() => Math.max(0, Math.floor(scrollTop.value / ROW_H) - OVERSCAN))
const endIndex = computed(() => Math.min(
  rows.value.length,
  Math.ceil((scrollTop.value + viewportH.value) / ROW_H) + OVERSCAN,
))
const visibleRows = computed(() => rows.value.slice(startIndex.value, endIndex.value))
const offsetY = computed(() => startIndex.value * ROW_H)

const colWidth = computed(() => Math.max(260, Math.ceil(maxCols.value * charW.value) + 16))
const cellStyle = computed(() => ({ width: `${colWidth.value}px` }))
const contentWidth = computed(() => {
  if (showDirList.value) return '100%'
  const cols = viewMode.value === 'split' ? 2 : 1
  const gutter = viewMode.value === 'split' ? 112 : 128
  return `${colWidth.value * cols + gutter}px`
})
// 等宽字体下 CJK 占两格：列宽按"显示宽度"算，两侧才对得齐。
// 超长行（压缩后的 js）没必要精确量，扫到 2000 列就停手。
const displayWidth = (s) => {
  const n = Math.min(s.length, 2000)
  let w = 0
  for (let i = 0; i < n; i += 1) {
    const c = s.charCodeAt(i)
    w += c > 0x2e7f && c < 0xff61 ? 2 : 1
  }
  return w
}

const measureCharWidth = () => {
  try {
    const el = bodyRef.value
    if (!el) return
    const cs = window.getComputedStyle(el)
    const ctx = document.createElement('canvas').getContext('2d')
    if (!ctx) return
    ctx.font = `${cs.fontSize} ${cs.fontFamily}`
    const w = ctx.measureText('M'.repeat(64)).width / 64
    if (w > 3 && w < 32) charW.value = w
  } catch {
    // 量不到就用兜底值，只影响横向滚动余量
  }
}

const EMPTY_CELL = Object.freeze({ no: null, text: '', inline: null, cls: 'is-blank' })

const resetScroll = () => {
  scrollTop.value = 0
  nextTick(() => {
    if (bodyRef.value) bodyRef.value.scrollTop = 0
  })
}

const syncViewport = () => {
  if (bodyRef.value) viewportH.value = bodyRef.value.clientHeight || 360
}

const onScroll = (ev) => {
  const el = ev.currentTarget
  scrollTop.value = el.scrollTop
  if (!showDirList.value || !dirHasMore.value || dirLoadingPage.value) return
  if (el.scrollHeight - el.scrollTop - el.clientHeight < 640) void loadNextPage()
}
// ───── 行模型 ─────
// 一个 hunk 里的行序固定是 ctx* del* add* ctx*，并排视图按 run 把 del/add 一一配对。
const buildFileRows = (result) => {
  const out = []
  let widest = 60
  const track = (t) => {
    const w = displayWidth(t)
    if (w > widest) widest = w
  }
  const split = viewMode.value === 'split'
  let seq = 0
  for (const hunk of result?.hunks || []) {
    out.push({ key: `h${hunk.index}`, kind: 'hunk', cls: 'is-hunk', header: hunk.header })
    const lines = hunk.lines || []
    if (!split) {
      for (const ln of lines) {
        track(ln.text)
        out.push({
          key: `u${seq++}`,
          kind: 'uni',
          cls: `is-${ln.tag}`,
          sign: ln.tag === 'del' ? '−' : ln.tag === 'add' ? '+' : ' ',
          left: { no: ln.left_no },
          right: { no: ln.right_no },
          cell: { no: null, text: ln.text, inline: ln.inline, cls: `is-${ln.tag}` },
        })
      }
      continue
    }
    let i = 0
    while (i < lines.length) {
      if (lines[i].tag === 'ctx') {
        const ln = lines[i]
        track(ln.text)
        out.push({
          key: `p${seq++}`,
          kind: 'pair',
          cls: 'is-ctx',
          left: { no: ln.left_no, text: ln.text, inline: null, cls: '' },
          right: { no: ln.right_no, text: ln.text, inline: null, cls: '' },
        })
        i += 1
        continue
      }
      const dels = []
      while (i < lines.length && lines[i].tag === 'del') dels.push(lines[i++])
      const adds = []
      while (i < lines.length && lines[i].tag === 'add') adds.push(lines[i++])
      if (!dels.length && !adds.length) {
        i += 1
        continue
      }
      for (let k = 0; k < Math.max(dels.length, adds.length); k += 1) {
        const d = dels[k]
        const a = adds[k]
        if (d) track(d.text)
        if (a) track(a.text)
        out.push({
          key: `p${seq++}`,
          kind: 'pair',
          cls: 'is-change',
          left: d ? { no: d.left_no, text: d.text, inline: d.inline, cls: 'is-del' } : EMPTY_CELL,
          right: a ? { no: a.right_no, text: a.text, inline: a.inline, cls: 'is-add' } : EMPTY_CELL,
        })
      }
    }
  }
  maxCols.value = Math.min(widest, 2000)
  return out
}
// 行内高亮切段只对视口内的行做：Rust 给的区间已经是 UTF-16 下标，直接 slice。
const paint = (cell) => {
  const text = cell?.text || ''
  const ranges = cell?.inline
  if (!ranges || !ranges.length) {
    return { no: cell?.no, cls: cell?.cls || '', segs: [{ t: text, hit: false }] }
  }
  const segs = []
  let pos = 0
  for (const r of ranges) {
    const a = Math.max(pos, Math.min(Number(r[0]) || 0, text.length))
    const b = Math.max(a, Math.min(Number(r[1]) || 0, text.length))
    if (a > pos) segs.push({ t: text.slice(pos, a), hit: false })
    if (b > a) segs.push({ t: text.slice(a, b), hit: true })
    pos = b
  }
  if (pos < text.length) segs.push({ t: text.slice(pos), hit: false })
  return { no: cell?.no, cls: cell?.cls || '', segs }
}

const paintedRows = computed(() => {
  if (showDirList.value) return []
  return visibleRows.value.map((r) => {
    if (r.kind === 'hunk') return r
    if (r.kind === 'uni') return { ...r, cell: paint(r.cell) }
    return { ...r, left: paint(r.left), right: paint(r.right) }
  })
})

const rebuildDirRows = () => {
  rows.value = dirEntries.value.map((e, i) => ({
    key: `${i}:${e.status}:${e.rel}`,
    kind: 'entry',
    entry: e,
  }))
}

const formatSize = (n) => {
  const v = Number(n) || 0
  if (v < 1024) return `${v} B`
  if (v < 1024 * 1024) return `${(v / 1024).toFixed(1)} KB`
  if (v < 1024 * 1024 * 1024) return `${(v / 1024 / 1024).toFixed(1)} MB`
  return `${(v / 1024 / 1024 / 1024).toFixed(2)} GB`
}

const STATUS_MARK = { added: '+', removed: '−', modified: '±', renamed: '↷', type_changed: '⇄', same: '=' }
const statusMark = (s) => STATUS_MARK[s] || '·'
const STATUS_TEXT = {
  added: '仅右侧有', removed: '仅左侧有', modified: '内容不同',
  renamed: '改名', type_changed: '文件/目录类型不同', same: '内容相同',
}
const entryTitle = (e) => {
  const base = `${STATUS_TEXT[e.status] || e.status} · ${e.rel}`
  if (e.is_dir) return e.child_count ? `${base}（整棵子树 ${e.child_count} 项）` : base
  return `${base}\n左 ${formatSize(e.left_size)} · 右 ${formatSize(e.right_size)}`
}

const entrySizeText = (e) => {
  if (e.is_dir) return ''
  if (e.status === 'added') return formatSize(e.right_size)
  if (e.status === 'removed') return formatSize(e.left_size)
  return `${formatSize(e.left_size)} → ${formatSize(e.right_size)}`
}

// ───── 请求 ─────
const fileOptionsPayload = () => ({
  algorithm: opts.algorithm,
  contextLines: Number(opts.contextLines) || 0,
  ignoreWhitespace: opts.ignoreWhitespace,
  ignoreTrailingWhitespace: opts.ignoreTrailingWhitespace,
  ignoreCase: opts.ignoreCase,
  ignoreBlankLines: opts.ignoreBlankLines,
})

const applyFileResult = (res) => {
  fileResult.value = res
  rows.value = buildFileRows(res)
  resetScroll()
}

const runFileDiff = async (leftPath, rightPath) => {
  const res = await invoke('diff_files', {
    leftPath,
    rightPath,
    options: fileOptionsPayload(),
  })
  applyFileResult(res)
}

const runTextDiff = async () => {
  const res = await invoke('diff_texts', {
    leftText: store.left.text ?? '',
    rightText: store.right.text ?? '',
    leftLabel: store.left.label || '左侧',
    rightLabel: store.right.label || '右侧',
    options: fileOptionsPayload(),
  })
  applyFileResult(res)
}
const runBufferDiff = async () => {
  const res = await invoke('diff_buffer_vs_file', {
    path: store.left.path,
    bufferText: store.right.text ?? '',
    options: fileOptionsPayload(),
  })
  applyFileResult(res)
}

const closeSession = async () => {
  const sid = sessionId.value
  if (!sid) return
  sessionId.value = ''
  try {
    await invoke('diff_dirs_close', { sessionId: sid })
  } catch (e) {
    logger.debug('[Diff] close session failed', { error: String(e?.message || e || '') })
  }
}

const dirOptionsPayload = () => {
  const excludeGlobs = excludeText.value
    .split(/[,;\n]/)
    .map((s) => s.trim())
    .filter(Boolean)
  return {
    quick: dirOpts.quick,
    detectRenames: dirOpts.detectRenames,
    includeHidden: dirOpts.includeHidden,
    respectGitignore: dirOpts.respectGitignore,
    includeSame: dirOpts.includeSame,
    pageSize: DIR_PAGE,
    excludeGlobs: excludeGlobs.length ? excludeGlobs : null,
  }
}

const runDirDiff = async () => {
  await closeSession()
  const sid = `dir-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
  sessionId.value = sid
  progress.phase = 'scan'
  progress.scanned = 0
  progress.compared = 0
  progress.elapsed = 0
  dirEntries.value = []
  rows.value = []
  const res = await invoke('diff_dirs', {
    leftRoot: store.left.path,
    rightRoot: store.right.path,
    sessionId: sid,
    options: dirOptionsPayload(),
  })
  dirSummary.value = res.summary
  dirEntries.value = res.entries || []
  dirTotal.value = Number(res.summary?.total ?? dirEntries.value.length)
  dirHasMore.value = !!res.has_more
  rebuildDirRows()
  resetScroll()
}
const drillPaths = ref({ left: '', right: '' })

const run = async () => {
  if (busy.value) return
  error.value = ''
  busy.value = true
  try {
    if (store.mode === 'text') {
      await runTextDiff()
    } else if (store.mode === 'buffer') {
      await runBufferDiff()
    } else if (drillRel.value) {
      await runFileDiff(drillPaths.value.left, drillPaths.value.right)
    } else if (isDirMode.value) {
      await runDirDiff()
    } else {
      await runFileDiff(store.left.path, store.right.path)
    }
  } catch (e) {
    error.value = String(e?.message || e || '对比失败')
    rows.value = []
    if (!drillRel.value) fileResult.value = null
    logger.warn('[Diff] compare failed', { mode: store.mode, error: error.value })
  } finally {
    busy.value = false
  }
}

const loadPage = async (offset, append) => {
  if (!sessionId.value) return
  dirLoadingPage.value = true
  try {
    const page = await invoke('diff_dirs_page', {
      sessionId: sessionId.value,
      offset,
      limit: DIR_PAGE,
      statuses: statusFilter.value.length ? [...statusFilter.value] : null,
      query: dirQuery.value ? dirQuery.value : null,
    })
    const list = page.entries || []
    dirEntries.value = append ? dirEntries.value.concat(list) : list
    dirTotal.value = Number(page.total || 0)
    dirHasMore.value = !!page.has_more
    rebuildDirRows()
    if (!append) resetScroll()
  } catch (e) {
    message.warning(String(e?.message || e || '取分页失败'))
  } finally {
    dirLoadingPage.value = false
  }
}

const loadNextPage = () => loadPage(dirEntries.value.length, true)
let filterTimer = 0
const onFilterChanged = () => {
  window.clearTimeout(filterTimer)
  filterTimer = window.setTimeout(() => {
    void loadPage(0, false)
  }, 180)
}

const cancelDir = async () => {
  if (!sessionId.value) return
  try {
    const stopped = await invoke('diff_dirs_cancel', { sessionId: sessionId.value })
    if (!stopped) message.info('这轮扫描已经结束了')
  } catch (e) {
    message.warning(String(e?.message || e || '中止失败'))
  }
}

const openEntry = async (entry) => {
  if (!entry || entry.is_dir || busy.value) return
  error.value = ''
  busy.value = true
  try {
    const leftRel = entry.renamed_from || entry.rel
    const [lp] = await invoke('diff_dirs_entry_paths', { sessionId: sessionId.value, rel: leftRel })
    const pair = await invoke('diff_dirs_entry_paths', { sessionId: sessionId.value, rel: entry.rel })
    drillPaths.value = { left: lp, right: pair[1] }
    drillRel.value = entry.rel
    await runFileDiff(lp, pair[1])
  } catch (e) {
    error.value = String(e?.message || e || '打开该项失败')
    rows.value = []
  } finally {
    busy.value = false
  }
}

const backToDir = () => {
  drillRel.value = ''
  drillPaths.value = { left: '', right: '' }
  fileResult.value = null
  error.value = ''
  rebuildDirRows()
  resetScroll()
}

const onSwap = () => {
  store.swap()
  if (!store.left.path || !store.right.path) return
  backToDir()
}
const pickSide = async (side) => {
  if (store.mode === 'text') return
  const dir = isDirMode.value
  const current = side === 'left' ? store.left.path : store.right.path
  try {
    const picked = await openDialog({
      directory: dir,
      multiple: false,
      title: `选择${side === 'left' ? '左' : '右'}侧${dir ? '目录' : '文件'}`,
      defaultPath: current || store.left.path || undefined,
    })
    const next = Array.isArray(picked) ? picked[0] : picked
    if (!next) return
    if (store.mode === 'buffer') {
      // 缓冲区对比的两侧是同一个文件的两个版本，手动挑一侧就退回普通文件对比。
      drillRel.value = ''
      store.compareFiles(side === 'left' ? next : store.left.path, side === 'right' ? next : store.left.path)
      return
    }
    store.setSide(side, next)
    if (store.left.path && store.right.path) {
      drillRel.value = ''
      void run()
    }
  } catch (e) {
    message.warning(String(e?.message || e || '选择失败'))
  }
}

// ───── 生命周期 ─────
let unlisten = null
let ro = null
let optTimer = 0

const observeBody = () => {
  if (!bodyRef.value || typeof ResizeObserver === 'undefined') return
  ro = new ResizeObserver(() => syncViewport())
  ro.observe(bodyRef.value)
}

onMounted(async () => {
  await nextTick()
  syncViewport()
  measureCharWidth()
  observeBody()
  try {
    unlisten = await listen('diff-dirs-progress', (ev) => {
      const p = ev?.payload
      if (!p || p.session_id !== sessionId.value) return
      progress.phase = p.phase
      progress.scanned = Number(p.scanned || 0)
      progress.compared = Number(p.compared || 0)
      progress.elapsed = Number(p.elapsed_ms || 0)
    })
  } catch (e) {
    logger.warn('[Diff] listen progress failed', { error: String(e?.message || e || '') })
  }
  if (canRun.value && !rows.value.length) void run()
})
onBeforeUnmount(() => {
  window.clearTimeout(filterTimer)
  window.clearTimeout(optTimer)
  ro?.disconnect()
  ro = null
  if (typeof unlisten === 'function') unlisten()
  unlisten = null
  void closeSession()
})

watch(() => store.requestSeq, (seq) => {
  if (!seq) return
  drillRel.value = ''
  drillPaths.value = { left: '', right: '' }
  fileResult.value = null
  dirSummary.value = null
  error.value = ''
  void run()
})

watch(viewMode, () => {
  if (!fileResult.value) return
  rows.value = buildFileRows(fileResult.value)
  resetScroll()
})

// 文件选项改了就重算（目录选项太贵，必须手动点"对比"）。
watch(
  () => [
    opts.algorithm, opts.contextLines, opts.ignoreWhitespace,
    opts.ignoreTrailingWhitespace, opts.ignoreCase, opts.ignoreBlankLines,
  ].join('|'),
  () => {
    if (!fileResult.value || busy.value) return
    window.clearTimeout(optTimer)
    optTimer = window.setTimeout(() => { void run() }, 260)
  },
)

watch(() => props.visible, (v) => {
  if (!v) return
  nextTick(() => {
    syncViewport()
    measureCharWidth()
    if (canRun.value && !rows.value.length) void run()
  })
})
</script>
<style scoped>
.guaxi-diff-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  background: var(--guaxi-panel-bg, #1e1e1e);
  color: var(--guaxi-text-primary, #d4d4d4);
}

.dp-head {
  flex: 0 0 auto;
  border-bottom: 1px solid color-mix(in srgb, var(--guaxi-border, #3a3d41) 82%, transparent);
  background: var(--toolbar-bg, var(--guaxi-panel-bg, #252526));
}

.dp-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 10px;
  min-height: 30px;
  flex-wrap: wrap;
}

.dp-row-options,
.dp-row-status,
.dp-row-filter {
  border-top: 1px solid color-mix(in srgb, var(--guaxi-border, #3a3d41) 60%, transparent);
  font-size: 12px;
}

.dp-mode {
  flex: 0 0 auto;
  font-size: 11px;
}

.dp-slot {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
  flex: 1 1 160px;
  max-width: 340px;
  padding: 3px 8px;
  border-radius: 5px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border, #3a3d41) 84%, transparent);
  background: color-mix(in srgb, var(--guaxi-panel-bg, #2d2d30) 92%, transparent);
  color: var(--guaxi-text-primary, #d4d4d4);
  font-size: 12px;
  cursor: pointer;
}

.dp-slot:hover {
  border-color: color-mix(in srgb, var(--guaxi-accent, #4c8bf5) 60%, transparent);
}

.dp-slot.is-empty {
  color: var(--guaxi-text-secondary, #9da5b4);
  border-style: dashed;
}

.dp-slot-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  direction: rtl;
  text-align: left;
}

.dp-actions {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.dp-actions :deep(.n-button.is-on) {
  color: var(--guaxi-accent, #4c8bf5);
}

.dp-field {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--guaxi-text-secondary, #9da5b4);
}

.dp-sep {
  width: 1px;
  height: 14px;
  background: color-mix(in srgb, var(--guaxi-border, #3a3d41) 80%, transparent);
}

.dp-status-text {
  color: var(--guaxi-text-secondary, #9da5b4);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dp-status-text.is-error {
  color: color-mix(in srgb, #ef4444 84%, var(--guaxi-text-primary));
}

.dp-status-text.is-warn {
  color: color-mix(in srgb, #f59e0b 84%, var(--guaxi-text-primary));
}

.dp-status-rel {
  font-family: var(--editor-font-family, 'Maple Mono NF CN', Consolas, monospace);
  color: var(--guaxi-text-primary, #d4d4d4);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 42%;
}

.dp-badge {
  font-family: var(--editor-font-family, 'Maple Mono NF CN', Consolas, monospace);
  font-size: 11px;
  padding: 0 5px;
  border-radius: 4px;
  background: color-mix(in srgb, var(--guaxi-panel-bg, #2d2d30) 70%, transparent);
}

.dp-badge.is-add { color: #4ade80; }
.dp-badge.is-del { color: #f87171; }
.dp-badge.is-mod { color: #60a5fa; }
.dp-badge.is-ren { color: #c084fc; }
.dp-badge.is-type { color: #fbbf24; }

.dp-body {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  position: relative;
  font-family: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace);
  font-size: 12.5px;
}

.dp-spacer {
  position: relative;
  min-width: 100%;
}

.dp-window {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  will-change: transform;
}

.dp-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  height: 100%;
  min-height: 120px;
  color: var(--guaxi-text-secondary, #9da5b4);
  font-size: 12.5px;
  padding: 0 16px;
  text-align: center;
}

.dp-placeholder.is-error {
  color: color-mix(in srgb, #ef4444 84%, var(--guaxi-text-primary));
}

.dp-line {
  display: flex;
  align-items: stretch;
  height: 20px;
  line-height: 20px;
  white-space: pre;
}

.dp-line.is-hunk {
  background: color-mix(in srgb, var(--guaxi-accent, #4c8bf5) 12%, transparent);
  color: var(--guaxi-text-secondary, #9da5b4);
}

.dp-hunk {
  padding-left: 10px;
  font-size: 11.5px;
}

.dp-no {
  flex: 0 0 52px;
  width: 52px;
  padding-right: 8px;
  text-align: right;
  color: color-mix(in srgb, var(--guaxi-text-secondary, #9da5b4) 76%, transparent);
  font-size: 11.5px;
  user-select: none;
}

.dp-sign {
  flex: 0 0 16px;
  width: 16px;
  text-align: center;
  user-select: none;
}

.dp-cell {
  display: inline-block;
  flex: 0 0 auto;
  overflow: hidden;
  padding: 0 4px;
}

.dp-cell.is-del {
  background: color-mix(in srgb, #f87171 16%, transparent);
}

.dp-cell.is-add {
  background: color-mix(in srgb, #4ade80 16%, transparent);
}

.dp-cell.is-blank {
  background: color-mix(in srgb, var(--guaxi-text-secondary, #9da5b4) 8%, transparent);
}

.dp-ink {
  border-radius: 2px;
  background: color-mix(in srgb, #fbbf24 34%, transparent);
}

.dp-cell.is-del .dp-ink {
  background: color-mix(in srgb, #f87171 40%, transparent);
}

.dp-cell.is-add .dp-ink {
  background: color-mix(in srgb, #4ade80 40%, transparent);
}

.dp-entry {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 20px;
  line-height: 20px;
  padding: 0 10px;
  cursor: pointer;
  font-size: 12px;
}

.dp-entry:hover {
  background: color-mix(in srgb, var(--guaxi-accent, #4c8bf5) 14%, transparent);
}

.dp-entry.is-dir-entry {
  cursor: default;
}

.dp-entry-mark {
  flex: 0 0 12px;
  text-align: center;
  font-weight: 600;
}

.dp-entry.is-added .dp-entry-mark { color: #4ade80; }
.dp-entry.is-removed .dp-entry-mark { color: #f87171; }
.dp-entry.is-modified .dp-entry-mark { color: #60a5fa; }
.dp-entry.is-renamed .dp-entry-mark { color: #c084fc; }
.dp-entry.is-type_changed .dp-entry-mark { color: #fbbf24; }
.dp-entry.is-same { color: var(--guaxi-text-secondary, #9da5b4); }

.dp-entry-icon {
  flex: 0 0 auto;
  opacity: 0.72;
}

.dp-entry-rel {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dp-entry-extra {
  flex: 0 0 auto;
  color: var(--guaxi-text-secondary, #9da5b4);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 28%;
}

.dp-entry-size {
  margin-left: auto;
  flex: 0 0 auto;
  padding-left: 10px;
  color: var(--guaxi-text-secondary, #9da5b4);
  font-size: 11px;
}

.dp-back {
  flex: 0 0 auto;
}
</style>

