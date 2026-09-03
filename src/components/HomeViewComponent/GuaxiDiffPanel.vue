<template>
  <div class="guaxi-diff-panel">
    <div class="dp-head">
      <div class="dp-row dp-row-main">
        <n-tag size="small" round :bordered="false" class="dp-mode">{{ modeLabel }}</n-tag>

        <n-radio-group :value="kindValue" size="small" class="dp-kind"
          :disabled="busy" @update:value="onKindChange">
          <n-radio-button value="file" title="对比任意两个文件">文件</n-radio-button>
          <n-radio-button value="dir" title="对比任意两个目录">目录</n-radio-button>
        </n-radio-group>

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
          <span class="dp-sep" />
          <n-radio-group v-model:value="dirView" size="small">
            <n-radio-button value="tree" title="按目录逐级展开">树形</n-radio-button>
            <n-radio-button value="list" title="平铺所有差异条目">列表</n-radio-button>
          </n-radio-group>
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
        <span v-else class="dp-status-text">{{ hintText }}</span>
      </div>
    </div>

    <div v-if="showDirAny && sessionId" class="dp-row dp-row-filter">
      <n-input v-model:value="dirQuery" size="tiny" clearable placeholder="过滤路径关键字"
        style="width: 190px" @update:value="onFilterChanged" />
      <n-checkbox-group v-model:value="statusFilter" @update:value="onFilterChanged">
        <n-checkbox v-for="s in statusChoices" :key="s.key" :value="s.key" size="small">
          {{ s.label }}
        </n-checkbox>
      </n-checkbox-group>
      <span class="dp-status-text">
        {{ showDirTree ? `展开 ${treeShown} 行` : `${dirEntries.length} / ${dirTotal}` }}
      </span>
    </div>

    <div ref="bodyRef" class="dp-body" :class="{ 'is-panes': splitPanes }" @scroll.passive="onScroll">
      <div v-if="busy && !rows.length" class="dp-placeholder">
        <n-spin size="small" />
        <span>{{ progressText }}</span>
      </div>
      <div v-else-if="error" class="dp-placeholder is-error">{{ error }}</div>
      <div v-else-if="!rows.length" class="dp-placeholder">{{ emptyText }}</div>

      <!-- 并排视图：左右各自横向滚动，纵向同步，中间那条竖线可以拖。
           不做成"一整块超宽内容"是因为一侧有长行就会把另一侧顶到屏幕外。 -->
      <div v-else-if="splitPanes" ref="panesRef" class="dp-panes"
        :class="{ 'is-dragging': splitDragging }">
        <div ref="leftPaneRef" class="dp-pane is-left" :style="{ width: `calc(${splitPercent}% - 3px)` }"
          @scroll.passive="onPaneScroll('left', $event)">
          <div class="dp-spacer" :style="{ height: `${totalHeight}px`, width: paneWidth('left') }">
            <div class="dp-window" :style="{ transform: `translateY(${offsetY}px)` }">
              <div v-for="row in paintedRows" :key="row.key" class="dp-line" :class="row.cls">
                <span v-if="row.kind === 'hunk'" class="dp-hunk">{{ row.header }}</span>
                <template v-else>
                  <span class="dp-no">{{ row.left.no || '' }}</span>
                  <span class="dp-cell is-fluid" :class="row.left.cls">
                    <span v-for="(seg, i) in row.left.segs" :key="i"
                      :class="{ 'dp-ink': seg.hit }">{{ seg.t }}</span>
                  </span>
                </template>
              </div>
            </div>
          </div>
        </div>
        <div class="dp-divider" title="拖动改变左右宽度 · 双击回到各占一半"
          @mousedown.prevent="startSplitDrag" @dblclick="resetSplit"></div>
        <div ref="rightPaneRef" class="dp-pane is-right"
          @scroll.passive="onPaneScroll('right', $event)">
          <div class="dp-spacer" :style="{ height: `${totalHeight}px`, width: paneWidth('right') }">
            <div class="dp-window" :style="{ transform: `translateY(${offsetY}px)` }">
              <div v-for="row in paintedRows" :key="row.key" class="dp-line" :class="row.cls">
                <span v-if="row.kind === 'hunk'" class="dp-hunk"></span>
                <template v-else>
                  <span class="dp-no">{{ row.right.no || '' }}</span>
                  <span class="dp-cell is-fluid" :class="row.right.cls">
                    <span v-for="(seg, i) in row.right.segs" :key="i"
                      :class="{ 'dp-ink': seg.hit }">{{ seg.t }}</span>
                  </span>
                </template>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div v-else class="dp-spacer" :style="{ height: `${totalHeight}px`, width: contentWidth }">
        <div class="dp-window" :style="{ transform: `translateY(${offsetY}px)` }">
          <template v-if="showDirTree">
            <div v-for="row in visibleRows" :key="row.key" class="dp-entry dp-tree-row"
              :class="[row.node ? `is-${row.node.status}` : 'is-tip',
                       { 'is-dir-entry': !!row.node?.is_dir }]"
              :style="{ paddingLeft: `${10 + row.depth * 14}px` }"
              :title="row.node ? nodeTitle(row.node) : ''" @click="onTreeClick(row)">
              <template v-if="row.node">
                <span class="dp-twist" :class="{ 'is-open': row.expanded }">
                  <span v-if="row.loading" class="dp-twist-wait">·</span>
                  <Icon v-else-if="row.node.expandable" :icon="chevronIcon" width="13" height="13" />
                </span>
                <span class="dp-entry-mark">{{ statusMark(row.node.status) }}</span>
                <Icon :icon="row.node.is_dir ? folderIcon : fileIcon" width="13" height="13"
                  class="dp-entry-icon" />
                <span class="dp-entry-rel">{{ row.node.name }}</span>
                <span v-if="row.node.renamed_from" class="dp-entry-extra">← {{ row.node.renamed_from }}</span>
                <span v-if="row.node.is_dir && row.node.child_count" class="dp-entry-extra">
                  {{ rollupText(row.node) }}
                </span>
                <span class="dp-entry-size">{{ entrySizeText(row.node) }}</span>
              </template>
              <span v-else class="dp-entry-extra">{{ row.tip }}</span>
            </div>
          </template>
          <template v-else-if="showDirList">
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

    <div v-if="dropActive" class="dp-drop-mask">
      <div class="dp-drop-half" :class="{ 'is-on': dropSide === 'left' }">
        <Icon :icon="dropIcon" width="20" height="20" />
        <span>放到左侧</span>
      </div>
      <div class="dp-drop-half" :class="{ 'is-on': dropSide === 'right' }">
        <Icon :icon="dropIcon" width="20" height="20" />
        <span>放到右侧</span>
      </div>
      <div class="dp-drop-tip">松手即对比 · 一次拖两个可直接填满两侧 · 文件和目录不能混</div>
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
import { getCurrentWindow } from '@tauri-apps/api/window'
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
import chevronIcon from '@iconify-icons/lucide/chevron-right'
import dropIcon from '@iconify-icons/lucide/download'
import { createLogger } from '../../utils/logger'
import { useDiffCompareStore } from '../../stores/diffCompareStore'

const logger = createLogger('GuaxiDiffPanel')
const message = useMessage()
const store = useDiffCompareStore()

const props = defineProps({
  visible: { type: Boolean, default: false },
})
const emit = defineEmits(['close'])

/** 行高，必须和样式里的 --dp-row-h 一致：虚拟滚动全靠它算偏移。 */
const ROW_H = 24
const OVERSCAN = 14
const DIR_PAGE = 500
const CHAR_FALLBACK = 7.2
/** 左右分栏比例、目录视图形态都记在本地，下次开窗接着用（各窗口同源，共享一份）。 */
const SPLIT_KEY = 'guaxi_diff_split_ratio'
const DIR_VIEW_KEY = 'guaxi_diff_dir_view'
const SPLIT_MIN = 0.12
const SPLIT_MAX = 0.88

const readSplitRatio = () => {
  try {
    const v = Number(localStorage.getItem(SPLIT_KEY))
    if (Number.isFinite(v) && v >= SPLIT_MIN && v <= SPLIT_MAX) return v
  } catch {
    // 读不到就一半一半
  }
  return 0.5
}
const readDirView = () => {
  try {
    return localStorage.getItem(DIR_VIEW_KEY) === 'list' ? 'list' : 'tree'
  } catch {
    return 'tree'
  }
}

const showOptions = ref(false)
const viewMode = ref('split')
/** 'tree' | 'list' —— 目录结果按层展开，还是平铺所有差异条目 */
const dirView = ref(readDirView())
const busy = ref(false)
const error = ref('')

const fileResult = ref(null)
const rows = ref([])
const maxCols = ref(80)
/** 并排视图两侧各自的最宽列数：一侧的长行不该撑开另一侧。 */
const maxColsLeft = ref(40)
const maxColsRight = ref(40)

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
/** 目录结果页（还没点进具体文件）：两种视图形态共用这个前提。 */
const showDirAny = computed(() => isDirMode.value && !drillRel.value)
const showDirTree = computed(() => showDirAny.value && dirView.value === 'tree')
const showDirList = computed(() => showDirAny.value && dirView.value === 'list')
/** 行级差异 + 并排：走左右两个独立滚动区，其余情况还是单个滚动区。 */
const splitPanes = computed(() => !showDirAny.value && viewMode.value === 'split')
const leftText = computed(() => store.leftLabel)
const rightText = computed(() => store.rightLabel)
const modeLabel = computed(() => {
  if (isDirMode.value) return '目录对比'
  if (store.mode === 'text') return '文本对比'
  return store.mode === 'buffer' ? '未保存改动' : '文件对比'
})
/** 文件 / 目录切换器的选中值：text、buffer 模式下两个都不选中，点一下就切到对应的路径对比。 */
const kindValue = computed(() => {
  if (store.mode === 'dir') return 'dir'
  return store.mode === 'file' ? 'file' : null
})
const canRun = computed(() => {
  if (busy.value) return false
  if (store.mode === 'text') return store.left.text != null && store.right.text != null
  if (store.mode === 'buffer') return !!store.left.path && store.right.text != null
  return !!(store.left.path && store.right.path)
})
const hintText = computed(() => {
  const what = isDirMode.value ? '目录' : '文件'
  return `拖两个${what}进来，或点上面的方框各选一个，然后点"对比"`
})
const emptyText = computed(() => {
  if (!store.left.path && !store.right.path && store.mode !== 'text') return hintText.value
  if (showDirAny.value && sessionId.value) return '没有符合条件的差异'
  if (!fileResult.value && !sessionId.value && !busy.value) return '两侧都填好了，点"对比"开始'
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
  if (showDirAny.value) return '100%'
  return `${colWidth.value + 128}px`
})
/**
 * 并排视图里每一侧内容的宽度。
 * `.dp-spacer` 带 `min-width:100%`，所以短文件不会缩成一条窄柱；
 * 算出来的宽度只在真的比可视区宽时才生效，也就是该出横向滚动条的时候。
 */
const paneWidth = (side) => {
  const cols = side === 'right' ? maxColsRight.value : maxColsLeft.value
  return `${52 + Math.ceil(cols * charW.value) + 24}px`
}
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

// ───── 并排视图的左右分栏 ─────
const panesRef = ref(null)
const leftPaneRef = ref(null)
const rightPaneRef = ref(null)
const splitRatio = ref(readSplitRatio())
const splitDragging = ref(false)
const splitPercent = computed(() => (splitRatio.value * 100).toFixed(3))

const persistSplit = () => {
  try {
    localStorage.setItem(SPLIT_KEY, splitRatio.value.toFixed(4))
  } catch {
    // 存不下只是下次回到一半一半
  }
}

let splitDrag = null
const onSplitDragMove = (ev) => {
  if (!splitDrag) return
  const next = splitDrag.ratio + (ev.clientX - splitDrag.x) / splitDrag.w
  splitRatio.value = Math.min(SPLIT_MAX, Math.max(SPLIT_MIN, next))
}
const endSplitDrag = () => {
  if (!splitDrag) return
  splitDrag = null
  splitDragging.value = false
  window.removeEventListener('mousemove', onSplitDragMove)
  window.removeEventListener('mouseup', endSplitDrag)
  persistSplit()
}
/** 分隔线按住就拖。监听挂在 window 上，鼠标甩出分隔线也不会中断。 */
const startSplitDrag = (ev) => {
  const host = panesRef.value
  if (!host) return
  splitDrag = { x: ev.clientX, ratio: splitRatio.value, w: host.clientWidth || 1 }
  splitDragging.value = true
  window.addEventListener('mousemove', onSplitDragMove)
  window.addEventListener('mouseup', endSplitDrag)
}
const resetSplit = () => {
  splitRatio.value = 0.5
  persistSplit()
}

/**
 * 两侧纵向滚动同步，横向各滚各的（长行只在自己这侧滚，另一侧不跟着跑偏）。
 * 互相赋值会再触发一次 scroll，用开关挡掉，下一帧放开。
 */
let syncingPanes = false
const onPaneScroll = (which, ev) => {
  const el = ev?.currentTarget
  if (!el) return
  scrollTop.value = el.scrollTop
  if (syncingPanes) return
  const other = which === 'left' ? rightPaneRef.value : leftPaneRef.value
  if (!other || Math.abs(other.scrollTop - el.scrollTop) < 1) return
  syncingPanes = true
  other.scrollTop = el.scrollTop
  requestAnimationFrame(() => {
    syncingPanes = false
  })
}

const resetScroll = () => {
  scrollTop.value = 0
  nextTick(() => {
    if (bodyRef.value) bodyRef.value.scrollTop = 0
    for (const el of [leftPaneRef.value, rightPaneRef.value]) {
      if (!el) continue
      el.scrollTop = 0
      el.scrollLeft = 0
    }
  })
}

/** 视口高度取当前真正在滚的那个容器：并排时是左面板，其余是 body。 */
const syncViewport = () => {
  const el = splitPanes.value ? leftPaneRef.value || bodyRef.value : bodyRef.value
  if (el) viewportH.value = el.clientHeight || 360
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
  // 并排视图两侧各算一份，只有统一视图才用合并后的 widest。
  let widestL = 20
  let widestR = 20
  const track = (t) => {
    const w = displayWidth(t)
    if (w > widest) widest = w
  }
  const trackL = (t) => {
    const w = displayWidth(t)
    if (w > widestL) widestL = w
    if (w > widest) widest = w
  }
  const trackR = (t) => {
    const w = displayWidth(t)
    if (w > widestR) widestR = w
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
        trackL(ln.text)
        trackR(ln.text)
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
        if (d) trackL(d.text)
        if (a) trackR(a.text)
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
  maxColsLeft.value = Math.min(widestL, 2000)
  maxColsRight.value = Math.min(widestR, 2000)
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
  if (showDirAny.value) return []
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

/** 目录结果的行由当前视图形态决定：树形按层要，列表按页要。 */
const refreshDirRows = async () => {
  if (dirView.value === 'tree') {
    if (!treeChildren.has('')) await loadTreeLevel('')
    rebuildTreeRows()
  } else {
    rebuildDirRows()
  }
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
  mixed: '里面有多种变化',
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

// ───── 目录树：一层一层地要，展开哪层算哪层 ─────
// 缓存故意用普通 Map / Set 而不是 reactive：行数组由 rebuildTreeRows 显式重建，
// 免得深层嵌套结构的响应式追踪拖慢几万条目录结果。
const treeChildren = new Map()
const treeExpanded = new Set()
const treeLoading = new Set()
const treeShown = ref(0)

const resetTree = () => {
  treeChildren.clear()
  treeExpanded.clear()
  treeLoading.clear()
  treeShown.value = 0
}

const nodeTitle = (n) => {
  const base = `${STATUS_TEXT[n.status] || n.status} · ${n.rel}`
  if (!n.is_dir) return `${base}\n左 ${formatSize(n.left_size)} · 右 ${formatSize(n.right_size)}`
  if (n.status === 'added' || n.status === 'removed') {
    return n.child_count ? `${base}（整棵子树 ${n.child_count} 项）` : base
  }
  return n.child_count ? `${base}（里面 ${n.child_count} 处差异）` : base
}

/** 目录节点右边那串小计。整棵子树只在一侧时后端不分类，退回条数。 */
const rollupText = (n) => {
  const parts = []
  if (n.added) parts.push(`+${n.added}`)
  if (n.removed) parts.push(`−${n.removed}`)
  if (n.modified) parts.push(`±${n.modified}`)
  if (n.renamed) parts.push(`↷${n.renamed}`)
  if (n.type_changed) parts.push(`⇄${n.type_changed}`)
  if (n.same) parts.push(`=${n.same}`)
  return parts.length ? parts.join(' ') : `${n.child_count} 项`
}

const rebuildTreeRows = () => {
  const out = []
  const walk = (parent, depth) => {
    const level = treeChildren.get(parent)
    if (!level) return
    const nodes = level.nodes || []
    for (const node of nodes) {
      const expanded = !!node.is_dir && treeExpanded.has(node.rel)
      out.push({
        key: `n:${node.rel}`,
        depth,
        node,
        expanded,
        loading: treeLoading.has(node.rel),
      })
      if (!expanded) continue
      const sub = treeChildren.get(node.rel)
      if (!sub) continue
      if (sub.nodes?.length) walk(node.rel, depth + 1)
      else if (!treeLoading.has(node.rel)) {
        out.push({ key: `z:${node.rel}`, depth: depth + 1, node: null, tip: '（这层没有内容）' })
      }
    }
    if (level.truncated) {
      out.push({
        key: `t:${parent}`,
        depth,
        node: null,
        tip: `这层还有 ${Math.max(0, Number(level.total || 0) - nodes.length)} 项没显示`,
      })
    }
  }
  walk('', 0)
  treeShown.value = out.length
  rows.value = out
}

/** 取一层。过滤条件跟着一起送后端，展开的层和列表视图看到的是同一套筛选。 */
const loadTreeLevel = async (parent) => {
  if (!sessionId.value) return
  treeLoading.add(parent)
  if (parent) rebuildTreeRows()
  try {
    const level = await invoke('diff_dirs_children', {
      sessionId: sessionId.value,
      parent: parent || null,
      statuses: statusFilter.value.length ? [...statusFilter.value] : null,
      query: dirQuery.value ? dirQuery.value : null,
    })
    treeChildren.set(parent, level)
  } catch (e) {
    treeExpanded.delete(parent)
    message.warning(String(e?.message || e || '展开目录失败'))
  } finally {
    treeLoading.delete(parent)
  }
}

const toggleTreeNode = async (node) => {
  if (!node?.is_dir || !node.expandable) return
  if (treeExpanded.has(node.rel)) {
    treeExpanded.delete(node.rel)
    rebuildTreeRows()
    return
  }
  treeExpanded.add(node.rel)
  if (!treeChildren.has(node.rel)) await loadTreeLevel(node.rel)
  rebuildTreeRows()
}

const onTreeClick = (row) => {
  const node = row?.node
  if (!node) return
  if (node.is_dir) {
    void toggleTreeNode(node)
    return
  }
  void openEntry(node)
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
  resetTree()
  await refreshDirRows()
  resetScroll()
}
const drillPaths = ref({ left: '', right: '' })

const run = async () => {
  if (busy.value || !canRun.value) return
  error.value = ''
  busy.value = true
  // 手动跑过一次之后，后面的交换、重跑就该自动生效了。
  store.autoRun = true
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
    if (showDirTree.value) {
      // 过滤条件变了，缓存的层级全作废，从根重新要一层。
      resetTree()
      void refreshDirRows().then(resetScroll)
      return
    }
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
  // 树的展开状态留着，返回时还是刚才那棵展开的树。
  void refreshDirRows()
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

// ───── 任意文件 / 目录：类型切换与系统拖放 ─────
/** 换一侧、换模式之前先把上一轮的结果和目录会话清干净。 */
const resetResults = async () => {
  await closeSession()
  drillRel.value = ''
  drillPaths.value = { left: '', right: '' }
  fileResult.value = null
  dirSummary.value = null
  dirEntries.value = []
  dirTotal.value = 0
  dirHasMore.value = false
  rows.value = []
  error.value = ''
  resetTree()
}

/** 路径到底是文件还是目录得问一下磁盘，光看字符串猜不出来（也顺便查出不存在的）。 */
const probeKinds = async (paths) => {
  const map = new Map()
  const list = (paths || []).filter(Boolean)
  if (!list.length) return map
  try {
    const got = await invoke('diff_probe_paths', { paths: list })
    for (const item of got || []) map.set(item.path, item.kind)
  } catch (e) {
    logger.warn('[Diff] probe paths failed', { error: String(e?.message || e || '') })
  }
  return map
}

/** 手动切"文件 / 目录"：类型对不上的已填路径留不住，只能丢掉。 */
const onKindChange = async (kind) => {
  const next = kind === 'dir' ? 'dir' : 'file'
  if (store.mode === next || busy.value) return
  const paths = [store.left.path, store.right.path].filter(Boolean)
  let keep = false
  if (paths.length && store.isPathMode) {
    const kinds = await probeKinds(paths)
    keep = paths.every((p) => kinds.get(p) === next)
  }
  await resetResults()
  store.setMode(next, { keepSides: keep })
  if (canRun.value) void run()
}

/** 把一个路径填到某一侧，必要时连模式一起切过去；两侧类型必须一致。 */
const fillSide = async (side, path, kind) => {
  const otherSide = side === 'left' ? 'right' : 'left'
  const otherPath = (otherSide === 'left' ? store.left.path : store.right.path) || ''
  if (kindValue.value !== kind) {
    let keepOther = false
    if (otherPath && store.isPathMode) {
      const kinds = await probeKinds([otherPath])
      keepOther = kinds.get(otherPath) === kind
      if (!keepOther) message.info('两侧类型必须一致，已清掉另一侧')
    }
    await resetResults()
    store.setMode(kind, { keepSides: keepOther })
  } else {
    await resetResults()
  }
  store.setSide(side, path)
  if (canRun.value) void run()
}

const dropActive = ref(false)
const dropSide = ref('left')

// onDragDropEvent 给的是物理像素，换成 CSS 像素才能和 innerWidth 比。
const sideFromX = (physicalX) => {
  const ratio = window.devicePixelRatio || 1
  const x = Number(physicalX || 0) / ratio
  return x > (window.innerWidth || 1) / 2 ? 'right' : 'left'
}

/**
 * 系统拖放落地：拖一个就填光标那一侧，拖两个直接填满两侧。
 * 文件和目录不能混着比，混了就明说，不去猜用户想比哪种。
 */
const acceptDrop = async (paths, side) => {
  const list = (paths || []).map((p) => String(p || '')).filter(Boolean)
  if (!list.length) return
  if (list.length > 2) message.info('一次只取前两个路径')
  const picked = list.slice(0, 2)
  const kinds = await probeKinds(picked)
  if (picked.some((p) => (kinds.get(p) || 'missing') === 'missing')) {
    message.warning('这个路径读不到，可能已经不在了')
    return
  }
  if (picked.length === 2) {
    if (kinds.get(picked[0]) !== kinds.get(picked[1])) {
      message.warning('只能文件对文件、目录对目录地比较')
      return
    }
    if (kinds.get(picked[0]) === 'dir') store.compareDirs(picked[0], picked[1])
    else store.compareFiles(picked[0], picked[1])
    return
  }
  await fillSide(side, picked[0], kinds.get(picked[0]))
}

// ───── 生命周期 ─────
let unlisten = null
let unlistenDrop = null
let ro = null
let optTimer = 0

const observeBody = () => {
  if (!bodyRef.value || typeof ResizeObserver === 'undefined') return
  ro = new ResizeObserver(() => syncViewport())
  ro.observe(bodyRef.value)
}

/**
 * 接管窗口级的系统拖放。只在独立对比窗口里挂：嵌到别的窗口时不该抢宿主的文件拖入。
 */
const setupDragDrop = async () => {
  if (!store.host) return
  try {
    unlistenDrop = await getCurrentWindow().onDragDropEvent((ev) => {
      const p = ev?.payload || {}
      if (p.type === 'enter' || p.type === 'over') {
        dropActive.value = true
        dropSide.value = sideFromX(p.position?.x)
        return
      }
      dropActive.value = false
      if (p.type === 'drop') void acceptDrop(p.paths, sideFromX(p.position?.x))
    })
  } catch (e) {
    logger.warn('[Diff] listen drag drop failed', { error: String(e?.message || e || '') })
  }
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
  await setupDragDrop()
  // 预填路径的请求（Header 那个"带上次路径开窗"的入口）不自动跑，等用户点"对比"。
  if (store.autoRun && canRun.value && !rows.value.length) void run()
})
onBeforeUnmount(() => {
  window.clearTimeout(filterTimer)
  window.clearTimeout(optTimer)
  endSplitDrag()
  ro?.disconnect()
  ro = null
  if (typeof unlisten === 'function') unlisten()
  unlisten = null
  if (typeof unlistenDrop === 'function') unlistenDrop()
  unlistenDrop = null
  void closeSession()
})

// 新请求进来先清场再跑：只填了一侧的请求（比如工具栏"顺手带上当前文件"）跑不起来，
// 不清的话会把上一轮的结果留在屏幕上，看着像是新对比的结果。
watch(() => store.requestSeq, async (seq) => {
  if (!seq) return
  await resetResults()
  if (store.autoRun) await run()
})

watch(viewMode, () => {
  if (!fileResult.value) return
  rows.value = buildFileRows(fileResult.value)
  resetScroll()
})

// 并排/统一之间切换换了滚动容器，视口高度得按新容器重新量一次。
watch(splitPanes, () => {
  nextTick(() => {
    syncViewport()
    measureCharWidth()
  })
})

// 树形 / 列表切换：两种视图的数据各自现取，缺哪份补哪份。
watch(dirView, async (v) => {
  try {
    localStorage.setItem(DIR_VIEW_KEY, v)
  } catch {
    // 记不住就下次还是默认树形
  }
  if (!sessionId.value || !showDirAny.value) return
  if (v === 'list' && !dirEntries.value.length) await loadPage(0, false)
  else await refreshDirRows()
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
    if (store.autoRun && canRun.value && !rows.value.length) void run()
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
  /* 拖放蒙层按面板定位 */
  position: relative;
  background: var(--guaxi-panel-bg, #1e1e1e);
  color: var(--guaxi-text-primary, #d4d4d4);
  /*
   * 字体跟主界面同一份：--guaxi-editor-font-family 定义在 App.vue 的 :root，
   * 每个窗口都拿得到。原来写的 --editor-font-family 只存在于主窗口编辑器容器内部，
   * 面板搬进独立窗口后取不到值，直接掉到浏览器兜底字体，中文尤其明显地和主界面不是一套。
   * 字号也一并调大：这页就是拿来读代码的，12.5px 太挤。行高必须跟 JS 里的 ROW_H 对齐。
   */
  --dp-code-font: var(--guaxi-editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New');
  --dp-code-size: 16px;
  --dp-row-h: 24px;
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
  font-size: 12.5px;
}

.dp-mode {
  flex: 0 0 auto;
  font-size: 11px;
}

.dp-kind {
  flex: 0 0 auto;
}

.dp-kind :deep(.n-radio-button) {
  font-size: 11px;
  padding: 0 8px;
}

/* 拖放蒙层：左右两半提示会落到哪一侧，光标在哪半就亮哪半 */
.dp-drop-mask {
  position: absolute;
  inset: 0;
  z-index: 30;
  display: flex;
  align-items: stretch;
  gap: 8px;
  padding: 8px;
  pointer-events: none;
  background: color-mix(in srgb, var(--guaxi-panel-bg, #1e1e1e) 72%, transparent);
  backdrop-filter: blur(1px);
}

.dp-drop-half {
  flex: 1 1 50%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border: 2px dashed color-mix(in srgb, var(--guaxi-border, #3a3d41) 90%, transparent);
  border-radius: 8px;
  color: var(--guaxi-text-secondary, #9da5b4);
  font-size: 13px;
}

.dp-drop-half.is-on {
  border-color: var(--guaxi-accent, #4c8bf5);
  color: var(--guaxi-accent, #4c8bf5);
  background: color-mix(in srgb, var(--guaxi-accent, #4c8bf5) 12%, transparent);
}

.dp-drop-tip {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 14px;
  text-align: center;
  font-size: 12px;
  color: var(--guaxi-text-secondary, #9da5b4);
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
  font-family: var(--dp-code-font);
  font-size: 13px;
  color: var(--guaxi-text-primary, #d4d4d4);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 42%;
}

.dp-badge {
  font-family: var(--dp-code-font);
  font-size: 13px;
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
  font-family: var(--dp-code-font);
  font-size: var(--dp-code-size);
}

/* 并排视图：滚动交给两个面板自己，外层不再滚 */
.dp-body.is-panes {
  overflow: hidden;
}

.dp-panes {
  display: flex;
  align-items: stretch;
  height: 100%;
  min-height: 0;
}

.dp-panes.is-dragging {
  cursor: col-resize;
  user-select: none;
}

.dp-pane {
  min-width: 0;
  overflow: auto;
  position: relative;
}

/* 左侧宽度由拖动比例定死，右侧吃掉剩下的。
   min-width:0 + flex-basis:0 是关键：不然右侧会按内容宽度撑爆容器。 */
.dp-pane.is-left {
  flex: 0 0 auto;
}

.dp-pane.is-right {
  flex: 1 1 0;
}

/* 两侧纵向是同步的，左边那根纵向滚动条纯属重复；横向的留着，各侧自己滚 */
.dp-pane.is-left::-webkit-scrollbar:vertical {
  width: 0;
  height: 0;
}

.dp-divider {
  flex: 0 0 6px;
  cursor: col-resize;
  background: color-mix(in srgb, var(--guaxi-border, #3a3d41) 70%, transparent);
  transition: background 0.12s ease;
}

.dp-divider:hover,
.dp-panes.is-dragging .dp-divider {
  background: var(--guaxi-accent, #4c8bf5);
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
  height: var(--dp-row-h);
  line-height: var(--dp-row-h);
  white-space: pre;
}

.dp-line.is-hunk {
  background: color-mix(in srgb, var(--guaxi-accent, #4c8bf5) 12%, transparent);
  color: var(--guaxi-text-secondary, #9da5b4);
}

.dp-hunk {
  padding-left: 10px;
  font-size: 13px;
}

.dp-no {
  flex: 0 0 52px;
  width: 52px;
  padding-right: 8px;
  text-align: right;
  color: color-mix(in srgb, var(--guaxi-text-secondary, #9da5b4) 76%, transparent);
  font-size: 13px;
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

/* 并排视图里不给单元格定宽：横向由所在面板的滚动区兜着，
   一侧长行只让这一侧出滚动条，另一侧照旧顶格显示 */
.dp-cell.is-fluid {
  flex: 1 0 auto;
  overflow: visible;
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
  height: var(--dp-row-h);
  line-height: var(--dp-row-h);
  padding: 0 10px;
  cursor: pointer;
  font-size: 13.5px;
}

.dp-entry:hover {
  background: color-mix(in srgb, var(--guaxi-accent, #4c8bf5) 14%, transparent);
}

.dp-entry.is-dir-entry {
  cursor: default;
}

/* 树里的目录是可以点开的，光标得给出可点的样子 */
.dp-tree-row.is-dir-entry {
  cursor: pointer;
}

.dp-tree-row.is-tip {
  cursor: default;
  color: var(--guaxi-text-secondary, #9da5b4);
}

.dp-tree-row.is-tip:hover {
  background: transparent;
}

.dp-twist {
  flex: 0 0 13px;
  width: 13px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--guaxi-text-secondary, #9da5b4);
  transition: transform 0.12s ease;
}

.dp-twist.is-open {
  transform: rotate(90deg);
}

.dp-twist-wait {
  font-weight: 700;
  opacity: 0.7;
}

.dp-entry.is-mixed .dp-entry-mark {
  color: var(--guaxi-text-secondary, #9da5b4);
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
  font-size: 12px;
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
  font-size: 12px;
}

.dp-back {
  flex: 0 0 auto;
}
</style>

