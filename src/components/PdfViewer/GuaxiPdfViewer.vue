<template>
  <div
    ref="rootRef"
    class="gpv"
    :class="`theme-${theme}`"
    tabindex="0"
    @keydown="onViewerKeydown"
    @mousedown.capture="focusViewerForKeyboard"
  >
    <!--
      工具栏：一条，不分两行，也不靠一堆竖分隔线切块。

      分组用「胶囊」表达：搜索 / 翻页 / 缩放各自成组，组内是同一件事的三个部分，
      组间只留空隙。低频动作（旋转、平移锁定、重建、导出）收进「更多」菜单，
      因为一页 PDF 里 90% 的时间只用到前两组 —— 把它们全平铺出来，
      工具栏会长到把正文挤走，这正是原先一列图标看不过来的原因。
    -->
    <div v-if="features.toolbar !== false" class="gpv__toolbar">
      <!--
        宿主专属控件的插槽（独立阅读器的大纲 / 译文 / 批注，见 ReaderPane）。

        为什么放在工具栏内部而不是浮在正文上：这些入口原本是 absolute 浮层，
        压在 fixed 高度 38px 的工具栏上，必然遮住左侧的文件名与分页控件。
        放进工具栏后它自然参与这一行的排布，不再需要靠调数字躲开。
        不传插槽时（LaTeX 预览等调用方）这里渲染为空，工具栏与原先完全一致。
      -->
      <slot name="toolbar-leading" />
      <span v-if="titleLabel" class="gpv__title" :title="titleLabel">{{ titleLabel }}</span>
      <span class="gpv__status" :class="statusTone">{{ status }}</span>
      <div class="gpv__spacer" />

      <!--
        宿主驱动的动作组（撤销/重做、阅读位置前后）。

        这些能力的数据（历史栈）在宿主手里，不在 viewer 内部，所以按「可选按钮」暴露：
        宿主要什么用 features 打开，点了只 emit 事件，具体做什么由宿主决定。
        放左侧、紧贴状态文字，是因为它们是「对刚刚那次操作的反悔/回退」，
        和右侧「找内容 / 翻页 / 调大小」不是一类动作。
      -->
      <div v-if="hostActions.length" class="gpv__cluster" aria-label="编辑与导航">
        <button
          v-for="act in hostActions"
          :key="act.key"
          type="button"
          class="gpv__btn gpv__btn--icon"
          :title="act.title"
          :aria-label="act.title"
          :disabled="act.disabled"
          @click="onHostAction(act.key)"
        >
          <component :is="act.icon" />
        </button>
      </div>

      <!-- 搜索：默认只是一个图标，点开才让输入框占用宽度 -->
      <div v-if="features.search && hasPdf && !errorText" class="gpv__search" :class="{ 'is-open': searchOpen }">
        <button
          type="button"
          class="gpv__btn gpv__btn--icon"
          :class="{ 'gpv__btn--active': searchOpen }"
          title="在 PDF 中搜索（Ctrl+F）"
          aria-label="在 PDF 中搜索"
          :aria-expanded="searchOpen"
          @click="toggleSearch"
        >
          <SearchOutline />
        </button>
        <template v-if="searchOpen">
          <input
            ref="searchInputRef"
            v-model="searchQuery"
            class="gpv__search-input"
            type="text"
            placeholder="搜索…"
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
        </template>
      </div>

      <div v-if="features.pagination !== false" class="gpv__cluster" aria-label="PDF 分页导航">
        <button
          type="button"
          class="gpv__btn gpv__btn--icon"
          :disabled="!canGoPreviousPage"
          title="上一页（PageUp）"
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
          <span class="gpv__page-total">/ {{ totalPages }}</span>
        </span>
        <button
          type="button"
          class="gpv__btn gpv__btn--icon"
          :disabled="!canGoNextPage"
          title="下一页（PageDown）"
          aria-label="下一页"
          @click="goNextPage"
        >
          <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
            <path d="m8.5 18.5 6.5-6.5-6.5-6.5 1.4-1.4 7.9 7.9-7.9 7.9-1.4-1.4Z" fill="currentColor" />
          </svg>
        </button>
      </div>

      <div class="gpv__cluster">
        <button type="button" class="gpv__btn gpv__btn--icon" :disabled="zoom <= MIN_ZOOM" title="缩小（-）" aria-label="缩小" @click="zoomOut('toolbar')">
          <RemoveOutline />
        </button>
        <button
          type="button"
          class="gpv__zoom"
          title="适合宽度（W）"
          aria-label="适合宽度"
          @click="fitWidth"
        >{{ Math.round(zoom * 100) }}%</button>
        <button type="button" class="gpv__btn gpv__btn--icon" :disabled="zoom >= MAX_ZOOM" title="放大（+）" aria-label="放大" @click="zoomIn('toolbar')">
          <AddOutline />
        </button>
      </div>

      <button type="button" class="gpv__btn gpv__btn--icon" title="适合整页（P）" aria-label="适合整页" @click="fitPage">
        <ExpandOutline />
      </button>

      <!--
        低频动作直接平铺在工具栏上，不再收进「更多」菜单。

        原先旋转 / 平移锁定 / 重建 / 导出都藏在一个「更多」下拉里：菜单用 Teleport 挂 body、按
        getBoundingClientRect 定一次位，要维护「点外面关闭」「Esc 关闭」「工具栏横向滚动时
        菜单跟不跟」三件事，而实际打开的频次很低 —— 收起来的收益抵不过多一次点击和一套浮层状态。
        现在它们就在工具栏末尾，与其它图标同等对待；放不下时由工具栏自身的横向滚动兜底
        （它本来就不换行）。
      -->
      <!--
        旋转保留，手型工具不再需要：纸面本身在任何非文字位置按下就能拖动平移，
        按住空格还能临时平移；专门挂一个「锁定平移」按钮，等于让读者为一次拖拽多点一步。
      -->
      <div class="gpv__cluster" aria-label="旋转">
        <button type="button" class="gpv__btn gpv__btn--icon" title="向左旋转 90°（[）" aria-label="向左旋转" @click="rotateLeft('toolbar')">
          <Icon :icon="RotateCcwIcon" width="16" height="16" />
        </button>
        <button type="button" class="gpv__btn gpv__btn--icon" title="向右旋转 90°（]）" aria-label="向右旋转" @click="rotateRight('toolbar')">
          <Icon :icon="RotateCwIcon" width="16" height="16" />
        </button>
      </div>

      <button v-if="features.rebuild" type="button" class="gpv__btn gpv__btn--icon" title="重新构建" aria-label="重新构建" @click="$emit('rebuild')">
        <RefreshOutline />
      </button>
      <button
        v-if="features.export"
        type="button"
        class="gpv__btn gpv__btn--icon"
        title="导出 PDF"
        aria-label="导出 PDF"
        :disabled="!hasPdf"
        @click="$emit('export')"
      >
        <DownloadOutline />
      </button>
    </div>

    <div
      class="gpv__body guaxi-reader-scrollbar"
      ref="bodyRef"
      :class="{ 'gpv__body--pan': spaceHeld, 'gpv__body--panning': isPanning }"
      @scroll="onScroll"
      @mousedown="onBodyMouseDown"
    >
      <div v-if="errorText" class="gpv__error">
        <div class="gpv__error-title">构建失败</div>
        <pre class="gpv__error-log">{{ errorText }}</pre>
      </div>
      <div v-else-if="!hasPdf && !building" class="gpv__empty">
        {{ emptyText }}
      </div>

      <!--
        连续滚动：**只为可见页建 DOM**。

        原先 v-for 遍历全部 pages，5000 页就是 5000 个 .gpv__page 节点（每个还带文本层、
        高亮层等子节点），首次挂载与每次样式变化都要重排几万次 DOM，滚动必然卡。
        现在上下各放一个 spacer 撑出真实高度，中间只渲染视口附近的页 —— DOM 节点数
        恒定在 10 页左右，与总页数无关。
      -->
      <div v-show="hasPdf && !errorText" class="gpv__pages">
        <div class="gpv__spacer" :style="{ height: `${spacerTop}px` }" aria-hidden="true" />
        <div
          v-for="page in windowedPages"
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

            <!--
              PDF 链接热区：目录跳转与外部网址。

              放在高亮层之前（z 序更低）：点在高亮上的那一层应当仍然命中高亮，
              而链接区域通常不与高亮重叠；万一重叠，读者的意图更可能是「标记这段」。
              透明但可点，光标由 .gpv__link 的 pointer 表达。
            -->
            <div
              v-for="(lk, i) in (page.links || [])"
              :key="`lk${i}`"
              class="gpv__link"
              :style="linkStyle(lk)"
              :title="lk.uri ? lk.uri : (lk.page ? `跳到第 ${lk.page} 页` : '')"
              @click.stop="onLinkClick(lk, page.num, $event)"
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
        <div class="gpv__spacer" :style="{ height: `${spacerBottom}px` }" aria-hidden="true" />
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
import { openUrl } from '@tauri-apps/plugin-opener'
import {
  AddOutline,
  ArrowRedoOutline,
  ArrowUndoOutline,
  ChevronBackOutline,
  ChevronForwardOutline,
  DownloadOutline,
  ExpandOutline,
  RefreshOutline,
  RemoveOutline,
  SearchOutline,
} from '@vicons/ionicons5'
import { Icon } from '@iconify/vue'
import RotateCcwIcon from '@iconify-icons/lucide/rotate-ccw'
import RotateCwIcon from '@iconify-icons/lucide/rotate-cw'
import { createLogger } from '../../utils/logger'
import { useReadingHistory } from '../../composables/useReadingHistory'
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
  // 宿主动作组（见 hostActions）
  'undo-highlight',
  'redo-highlight',
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
//
// 密度：档位之间的比值不能超过 ~1.3。原来的 144→192 是 1.33 倍，而 1.42（常见默认缩放）
// 恰好跨过 144 落到 192 —— 一档之差就是 3.56MP vs 2.00MP 的编码量，屏幕上看不出区别，
// 等待时间却接近翻倍。补进 168 之后，1.2~1.5 这一段都落在 168，首次出图明显更快。
const devicePixelRatio = (typeof window !== 'undefined' && window.devicePixelRatio) || 1
const renderScale = Math.min(2, Math.max(1, devicePixelRatio))
const DPI_LADDER = [72, 96, 120, 144, 168, 192, 240, 288, 384]
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

/**
 * 每页的垂直布局几何（不读 DOM）。
 *
 * 为什么必须脱离 DOM：原先滚动与渲染都通过 `el.offsetTop / offsetHeight` 取位置，
 * 这两个属性会强制同步重排（layout thrash）。5000 页的 PDF 一次滚动就要读 10000 次，
 * 每帧都卡；而且页数越多越慢。
 *
 * 页面尺寸完全由 `wPt/hPt * cssScale` 决定，垂直方向就是依次堆叠，因此位置可以纯算：
 * 累加高度即可。
 *
 * 与 .gpv__pages 的 CSS 必须严格对齐：容器有 `padding: 16px` 与 `gap: 16px`，
 * 所以首页偏移 16px、页间距 16px。改 CSS 就必须同步改这两个常量，否则滚动定位会漂。
 */
const PAGES_PADDING = 16
const PAGE_GAP = 16

const pageGeometry = computed(() => {
  const scale = cssScale.value
  const rotated = isRotated90.value
  const out = new Array(pages.length)
  let y = PAGES_PADDING
  for (let i = 0; i < pages.length; i += 1) {
    const page = pages[i]
    const w = Math.round(page.wPt * scale)
    const h = Math.round(page.hPt * scale)
    const boxH = rotated ? w : h
    out[i] = { num: page.num, top: y, height: boxH }
    y += boxH + PAGE_GAP
  }
  return out
})

/**
 * 当前应当挂载 DOM 的页区间 [start, end)。
 *
 * 与「渲染哪些页」是两件事：渲染（调后端出图）区间更窄、由 renderVisible 控制；
 * 这里只为 DOM 存活服务，缓冲更大一些，避免快速滚动时频繁增删节点。
 *
 * 关键：无论 PDF 有 10 页还是 5000 页，这里最多返回约十几页。
 */
const WINDOW_BUFFER_SCREENS = 1.5
const windowRange = ref({ start: 0, end: 0 })

// 页码 → page 对象的下标表。页码恒为 1..n 连续，直接按索引取即可，
// 不必每帧重建 Map —— 5000 页时那是一次 5000 项的分配 + 哈希，而它发生在**每次滚动**里。
const pageByNum = computed(() => {
  const table = new Array(pages.length + 1)
  for (let i = 0; i < pages.length; i += 1) table[pages[i].num] = pages[i]
  return table
})

const windowedPages = computed(() => {
  const geo = pageGeometry.value
  const { start, end } = windowRange.value
  if (!geo.length) return []
  const table = pageByNum.value
  const out = []
  const from = Math.max(0, start)
  const to = Math.min(geo.length, end)
  for (let i = from; i < to; i += 1) {
    const page = table[geo[i].num]
    if (page) out.push(page)
  }
  return out
})

/** 上方 spacer 的高度 = 第一个已挂载页之前所有页（含间距）的总高度。 */
const spacerTop = computed(() => {
  const geo = pageGeometry.value
  const { start } = windowRange.value
  if (!start || !geo.length) return 0
  const first = geo[Math.min(start, geo.length - 1)]
  return Math.max(0, first.top - PAGES_PADDING)
})

/** 下方 spacer 的高度 = 已挂载页之后剩余的总高度。 */
const spacerBottom = computed(() => {
  const geo = pageGeometry.value
  const { end } = windowRange.value
  if (!geo.length || end >= geo.length) return 0
  const last = geo[end - 1] || geo[geo.length - 1]
  const lastBottom = last.top + last.height
  const containerHeight = geo[geo.length - 1].top + geo[geo.length - 1].height
  return Math.max(0, containerHeight - lastBottom)
})

/**
 * 按滚动位置重算窗口区间。
 *
 * 二分定位可见区间，复杂度 O(log n + k)，k 是挂载页数 —— 与总页数无关。
 */
const recomputeWindow = () => {
  const el = bodyRef.value
  const geo = pageGeometry.value
  if (!el || !geo.length) {
    windowRange.value = { start: 0, end: 0 }
    return
  }
  const top = el.scrollTop
  const bottom = top + el.clientHeight
  const buffer = el.clientHeight * WINDOW_BUFFER_SCREENS
  const from = top - buffer
  const to = bottom + buffer

  const lowerBound = (value) => {
    let lo = 0
    let hi = geo.length
    while (lo < hi) {
      const mid = (lo + hi) >> 1
      if (geo[mid].top + geo[mid].height < value) lo = mid + 1
      else hi = mid
    }
    return lo
  }

  const start = lowerBound(from)
  let end = start
  while (end < geo.length && geo[end].top <= to) end += 1

  const nextStart = Math.max(0, Math.min(start, geo.length))
  const nextEnd = Math.max(nextStart, Math.min(end, geo.length))
  const prev = windowRange.value
  if (prev.start !== nextStart || prev.end !== nextEnd) {
    windowRange.value = { start: nextStart, end: nextEnd }
  }
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

/**
 * 工具栏左侧的宿主动作组。
 *
 * 撤销/重做高亮的历史栈在宿主手里（只有宿主知道一次高亮是怎么加进去的），
 * 所以这两个按钮由宿主声明、viewer 只负责画和转发。
 *
 * 「上一/下一阅读位置」则相反：位置、滚动几何、Alt+← / Alt+→ 全在 viewer 内部，
 * 宿主既拿不到也维护不了同一份数据，因此这一组直接由 viewer 自己记账、自己执行，
 * 宿主要做的只是 features.readingHistory = true 打开这两颗按钮。
 *
 * 保持顺序稳定（撤销→重做→上一处→下一处），读者靠位置记，不靠读标题。
 */
const hostActions = computed(() => {
  const f = props.features || {}
  const list = []
  if (f.undoHighlight) {
    list.push({
      key: 'undo-highlight',
      title: '撤销高亮（Ctrl+Z）',
      icon: ArrowUndoOutline,
      // 未传 can* 时按「可点」处理：宿主若没提供状态，就不该把按钮画成灰的。
      disabled: f.canUndoHighlight === false,
    })
  }
  if (f.redoHighlight) {
    list.push({
      key: 'redo-highlight',
      title: '重做高亮（Ctrl+Y）',
      icon: ArrowRedoOutline,
      disabled: f.canRedoHighlight === false,
    })
  }
  if (f.readingHistory) {
    list.push({
      key: 'go-reading-back',
      title: '上一阅读位置（Alt+←）',
      icon: ChevronBackOutline,
      disabled: !readingHistory.canBack.value,
    })
    list.push({
      key: 'go-reading-forward',
      title: '下一阅读位置（Alt+→）',
      icon: ChevronForwardOutline,
      disabled: !readingHistory.canForward.value,
    })
  }
  return list
})

const onHostAction = (key) => {
  if (key === 'go-reading-back') {
    readingHistory.back()
    return
  }
  if (key === 'go-reading-forward') {
    readingHistory.forward()
    return
  }
  emit(key)
}

// ───────────────────────── 缩放锚点 ─────────────────────────
// 缩放不能总以内容左上角为原点：那样每放大一档，正在看的那一行就往下跑出视口，
// 用户得反复手动往回滚 —— 这正是「缩放不在屏幕中心」的体感来源。
//
// 做法：改 zoom 之前把锚点记成**几何坐标**（页码 + 页内 pt），而不是像素。
// 为什么不能记像素：虚拟滚动下缩小会让附近页被卸载、spacer 顶替，内容总高在
// 同一帧内缩水，浏览器立刻把 scrollTop 夹回新的上限 —— 像素锚点在写入前就已经失效。
// 页码 + 页内 pt 只依赖 pageGeometry（纯计算，与 DOM 挂载无关），缩放前后都成立。
//
// 锚点默认取视口中心；Ctrl+滚轮传指针位置，缩放就以指针下的内容为不动点。
const captureZoomAnchor = (clientX, clientY) => {
  const el = bodyRef.value
  if (!el) return null
  const rect = el.getBoundingClientRect()
  let ax = rect.width / 2
  let ay = rect.height / 2
  if (Number.isFinite(clientX) && Number.isFinite(clientY)) {
    const px = clientX - rect.left
    const py = clientY - rect.top
    if (px >= 0 && px <= rect.width && py >= 0 && py <= rect.height) {
      ax = px
      ay = py
    }
  }
  // 屏幕内偏移 → 未缩放 pt 坐标（换算全程用滚动量本身，不做任何 DOM 测量）。
  const geo = pageGeometry.value
  if (!geo.length) return null
  const pageOffset = (el.scrollTop + ay - PAGES_PADDING) / zoom.value
  // 找到该 pt 位置所在的页（页高已按当前 zoom 换算回 pt）。
  let lo = 0
  let hi = geo.length - 1
  let idx = 0
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    if (geo[mid].top <= el.scrollTop + ay) {
      idx = mid
      lo = mid + 1
    } else {
      hi = mid - 1
    }
  }
  const item = geo[idx]
  const yInPagePt = Math.max(0, pageOffset - item.top / zoom.value)
  return { pageIdx: idx, yInPagePt, ax, ay, xPt: el.scrollLeft / zoom.value }
}

const applyZoomAnchor = (anchor) => {
  if (!anchor) return
  const el = bodyRef.value
  if (!el) return
  const geo = pageGeometry.value
  const item = geo[Math.min(anchor.pageIdx, geo.length - 1)]
  if (!item) return
  el.scrollTop = Math.max(0, item.top + anchor.yInPagePt * zoom.value - anchor.ay)
  el.scrollLeft = Math.max(0, anchor.xPt * zoom.value - anchor.ax)
}

// 手动缩放一律走 setZoom：它要顺带退出自适应模式，否则在 fit-mode='width' 下点了 +/− 之后，
// 任何一次容器尺寸变化都会把用户的缩放顶回适合宽度。
const zoomIn = (reason = 'manual') => setZoom(zoom.value + 0.1, reason)
const zoomOut = (reason = 'manual') => setZoom(zoom.value - 0.1, reason)
const setZoom = (z, reason = 'manual', anchorAt = null) => {
  const next = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, +Number(z).toFixed(2)))
  if (next === zoom.value) return
  const anchor = captureZoomAnchor(anchorAt?.clientX, anchorAt?.clientY)
  // 手动缩放即退出自适应模式：否则之后任何一次窗口尺寸变化都会把用户的缩放顶掉。
  activeFitMode.value = ''
  zoom.value = next
  // 几何是按 zoom 算出来的，必须在 DOM 应用新尺寸之后才能落滚动位置。
  nextTick(() => {
    applyZoomAnchor(anchor)
    // 缩放后可见页变了，立刻按新视口重算（不必等滚动事件）。
    if (scrollRaf) { cancelAnimationFrame(scrollRaf); scrollRaf = 0 }
    updateVisible()
  })
  logger.debug('PDF缩放设置', { reason, zoom: zoom.value })
}

// Ctrl/⌘ + 滚轮缩放：阻止浏览器默认整页缩放，改为按步缩放 PDF。
const onWheel = (ev) => {
  if (!(ev.ctrlKey || ev.metaKey)) return
  ev.preventDefault()
  const step = Math.min(0.5, 0.0015 * Math.abs(ev.deltaY) + 0.05)
  if (ev.deltaY < 0) setZoom(zoom.value + step, 'ctrl-wheel', { clientX: ev.clientX, clientY: ev.clientY })
  else if (ev.deltaY > 0) setZoom(zoom.value - step, 'ctrl-wheel', { clientX: ev.clientX, clientY: ev.clientY })
}
// ─────────────────── 自适应缩放（适合宽度 / 适合整页）───────────────────
// 设计说明：预览面板的宽度会随窗口和分栏拖拽变化，若只在初始时定一个 zoom，窗口一变阅读区
// 就不再贴合面板。所以把"适合宽度/整页"做成一种**持续生效的模式**：打开文档、容器尺寸变化时
// 都重算；一旦用户手动缩放（Ctrl+滚轮 / +− / 工具栏输入），立刻退出该模式，之后不再抢用户的缩放。
const AUTO_FIT_DEBOUNCE_MS = 140
let autoFitTimer = null

/**
 * 适合宽度 / 适合整页的目标缩放。
 *
 * 两个细节直接从实测里来：
 *
 * 1. `clientWidth` 不含纵向滚动条，但它**不占布局空间**（滚动条浮在内容上），
 *    所以可用宽度还得减去 `offsetWidth - clientWidth`，否则页面右缘会被滚动条压掉几像素。
 * 2. 内边距是 .gpv__pages 的 16px × 2 = 32px，必须与常量 PAGES_PADDING/PAGE_GAP 同源，
 *    写死 36 会在改内边距时悄悄错位。
 */
const computeFitZoom = (mode) => {
  const el = bodyRef.value
  const first = pages[0]
  if (!el || !first || !first.wPt || !first.hPt) return null
  const scrollbar = Math.max(0, el.offsetWidth - el.clientWidth)
  const pad = PAGES_PADDING * 2
  const availW = el.clientWidth - scrollbar - pad
  const zx = availW / first.wPt
  if (mode === 'page') {
    const scrollbarH = Math.max(0, el.offsetHeight - el.clientHeight)
    const availH = el.clientHeight - scrollbarH - pad
    return Math.min(zx, availH / first.hPt)
  }
  return zx
}

// preserveAnchor：重算前记住视口中心指向的内容位置、算完落回同一处，拖动窗口时不丢阅读位置。
// 为什么不用「滚动比例」：比例在两个文档/两处内容的高度分布不同时会漂，而读者关心的是
// 「我正在看的那一行别跑掉」——锚点坐标恰好就是这件事。
// 新文档首次落位用 false —— 阅读位置由宿主的恢复流程负责，这里别去抢。
const applyFit = ({ reason = 'auto', preserveAnchor = false } = {}) => {
  if (!activeFitMode.value) return false
  const z = computeFitZoom(activeFitMode.value)
  if (!z || !Number.isFinite(z) || z <= 0) return false
  const next = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, +z.toFixed(2)))
  if (next === zoom.value) return false
  const anchor = preserveAnchor ? captureZoomAnchor() : null
  zoom.value = next
  logger.debug('PDF自适应缩放', { reason, mode: activeFitMode.value, zoom: next })
  if (preserveAnchor) nextTick(() => applyZoomAnchor(anchor))
  return true
}

// 容器尺寸变化时重算。ResizeObserver 在拖拽过程中会连续触发，而每次改 zoom 都要把可见页
// 按新 DPI 重新栅格化（不便宜），所以防抖。
const scheduleAutoFit = (reason = 'resize') => {
  if (!activeFitMode.value) return
  if (autoFitTimer) clearTimeout(autoFitTimer)
  autoFitTimer = setTimeout(() => {
    autoFitTimer = null
    applyFit({ reason, preserveAnchor: true })
  }, AUTO_FIT_DEBOUNCE_MS)
}

const fitWidth = () => {
  activeFitMode.value = 'width'
  applyFit({ reason: 'fit-width', preserveAnchor: true })
}
const fitPage = () => {
  activeFitMode.value = 'page'
  applyFit({ reason: 'fit-page', preserveAnchor: true })
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
  // Ctrl+F / `/` 进入搜索时，输入框可能还收着 —— 先展开再聚焦。
  searchOpen.value = true
  await nextTick()
  searchInputRef.value?.focus?.()
  searchInputRef.value?.select?.()
  logger.info('PDF搜索框聚焦', { reason })
}

/**
 * 平滑滚动到指定位置，页面不可见时退回瞬时滚动。
 *
 * 为什么需要这层包装：WebView2 在窗口最小化 / 被系统节流 / 文档隐藏时会**整次丢弃**
 * `behavior:'smooth'` 的滚动调用——实测同一容器 smooth 滚到 3,000,000 后 scrollTop
 * 仍是 0，而同步赋值立刻到位。跳页、搜索命中、大纲跳转、正反向定位全走这里，
 * 不加兜底就会表现成「点了完全没反应」，而且只在用户切走窗口再回来时偶发。
 */
const smoothScrollTo = (el, top, reason) => {
  if (!el) return
  const hidden = typeof document !== 'undefined' && document.visibilityState === 'hidden'
  if (hidden) {
    el.scrollTop = Math.max(0, top)
    logger.debug('PDF跳转（页面不可见，瞬时落位）', { reason, top })
    return
  }
  el.scrollTo({ top: Math.max(0, top), behavior: 'smooth' })
}

const scrollViewerBy = (deltaY, reason) => {
  const el = bodyRef.value
  if (!el) return
  const hidden = typeof document !== 'undefined' && document.visibilityState === 'hidden'
  if (hidden) el.scrollTop += deltaY
  else el.scrollBy({ top: deltaY, behavior: 'smooth' })
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
    // 上一本文档的阅读位置对当前文档没有意义。
    readingHistory.reset()
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
  scheduleReadingCommit()
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
//
// 位置来自 pageGeometry（纯计算），不再逐页读 offsetTop/offsetHeight：
// 5000 页时那套写法每帧要触发 10000 次同步重排。
const updateVisible = () => {
  const el = bodyRef.value
  if (!el || !pages.length) return
  // 先更新窗口区间：DOM 只保留附近页，下面的可见页计算与它用的是同一份几何。
  recomputeWindow()
  const top = el.scrollTop
  const bottom = top + el.clientHeight

  const geo = pageGeometry.value
  let firstVisible = currentPage.value
  let bestOverlap = 0

  // 几何按 top 升序，找到第一个「下边缘越过视口上沿」的页，向后扫到离开视口为止。
  // 用二分定位起点，避免每次都从第 1 页开始线性扫。
  let lo = 0
  let hi = geo.length - 1
  let start = 0
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    if (geo[mid].top + geo[mid].height < top) {
      lo = mid + 1
    } else {
      start = mid
      hi = mid - 1
    }
  }

  for (let i = start; i < geo.length; i += 1) {
    const item = geo[i]
    if (item.top > bottom) break
    const nBottom = item.top + item.height
    const overlap = Math.min(nBottom, bottom) - Math.max(item.top, top)
    if (overlap > bestOverlap) {
      bestOverlap = overlap
      firstVisible = item.num
    }
  }
  if (firstVisible !== currentPage.value) {
    currentPage.value = firstVisible
    emit('page-change', firstVisible)
  }
}

// ─────────────────── 阅读位置（上一处 / 下一处）───────────────────
// 位置用「页码 + 页内 pt」表示，而不是像素：缩放会改变同一内容的像素坐标，
// 但 (页, 页内 pt) 在任何缩放档位下都指向同一行字。
// 记录时机见 useReadingHistory：滚动停稳后自动提交，跳转前用 markJump 记下离开的位置。
const readingPositionNow = () => {
  const el = bodyRef.value
  const geo = pageGeometry.value
  if (!el || !geo.length) return null
  // 取「视口顶边所在的那一页」而不是当前主导页：主导页可能是半屏之外的那一张，
  // 按它记位置会在恢复时把视口向下/向上错开将近一页。
  const top = el.scrollTop
  let idx = 0
  let lo = 0
  let hi = geo.length - 1
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    if (geo[mid].top <= top) {
      idx = mid
      lo = mid + 1
    } else {
      hi = mid - 1
    }
  }
  const item = geo[idx]
  return {
    page: item.num,
    yPt: Math.max(0, (top - item.top) / (zoom.value || 1)),
  }
}

const positionTopPx = (position) => {
  const geo = pageGeometry.value
  const item = geo[Math.min(Math.max(1, position.page), geo.length) - 1]
  if (!item) return 0
  return item.top + position.yPt * (zoom.value || 1)
}

const applyReadingPosition = (position) => {
  const el = bodyRef.value
  const geo = pageGeometry.value
  if (!el || !geo.length) return
  const page = Math.min(Math.max(1, position.page), geo.length)
  currentPage.value = page
  syncPageInput()
  const item = geo[page - 1]
  el.scrollTop = Math.max(0, item.top + position.yPt * (zoom.value || 1))
  recomputeWindow()
  updateVisible()
  scheduleRenderVisible()
}

const readingHistory = useReadingHistory({
  getPosition: readingPositionNow,
  applyPosition: applyReadingPosition,
  // 「差一段」按视口高度衡量：停在新的一屏才算新位置，微小滚动不污染历史。
  isFar: (a, b) => {
    const vh = bodyRef.value?.clientHeight || 600
    return Math.abs(positionTopPx(a) - positionTopPx(b)) >= Math.max(80, vh * 0.5)
  },
  // 只有开启了「阅读位置」的宿主才响应 Alt+← / Alt+→：LaTeX 预览嵌在编辑器里，
  // 那组键属于编辑器自己的定位快捷键，不能在这里被抢走。
  isActive: () =>
    !!props.features.readingHistory && !!hasPdf.value && (bodyRef.value?.clientHeight || 0) > 0,
  getContainer: () => rootRef.value,
})

const READING_COMMIT_DEBOUNCE_MS = 500
let readingCommitTimer = null
const scheduleReadingCommit = () => {
  if (readingCommitTimer) clearTimeout(readingCommitTimer)
  readingCommitTimer = setTimeout(() => {
    readingCommitTimer = null
    readingHistory.commit()
  }, READING_COMMIT_DEBOUNCE_MS)
}

// 重活：对当前可见（默认含上下一屏缓冲）的页做栅格化/文本/高亮；远离可见区的已渲染页释放图片。
// bufferFactor=0 时只渲染严格可见页——缩放时用它，避免一次重渲多页大位图导致卡顿。
const renderVisible = ({ bufferFactor = 1 } = {}) => {
  const el = bodyRef.value
  if (!el || !pages.length) return
  const top = el.scrollTop
  const bottom = top + el.clientHeight
  const BUFFER = el.clientHeight * bufferFactor

  // 可见区间完全由几何算出（不读 DOM），再按页码直接索引 page 对象。
  const geo = pageGeometry.value
  const table = pageByNum.value

  // 先用二分找到可见区间起点，只扫视口附近那几十项 —— 原实现从第 1 页线性扫到
  // 当前页，滚到 4000 页时每次滚动要白扫 4000 次比较。
  const active = []
  let lo = 0
  let hi = geo.length - 1
  let startIdx = 0
  const fromY = top - BUFFER
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    if (geo[mid].top + geo[mid].height < fromY) lo = mid + 1
    else { startIdx = mid; hi = mid - 1 }
  }
  for (let i = startIdx; i < geo.length; i += 1) {
    const item = geo[i]
    if (item.top > bottom + BUFFER) break
    const page = table[item.num]
    if (page) active.push({ page, center: item.top + item.height / 2 })
  }

  // 释放远离视口的已渲染页，避免 5000 页把内存吃穿。
  // 只扫当前页前后一个窗口：全量扫 5000 页会把「每次滚动」变成 5000 次比较，
  // 而真正可能超距的只有跳页时遗留的那几页。
  const center = currentPage.value
  const releaseFrom = Math.max(0, center - 1 - 8)
  const releaseTo = Math.min(pages.length, center + 8)
  for (let i = releaseFrom; i < releaseTo; i += 1) {
    const page = pages[i]
    if (!page) continue
    if (!page.image) continue
    if (Math.abs(page.num - center) > 6) doc.freePage(page)
  }
  // 其余已渲染页（跳页后遗留的）由 host 的 LRU 兜底释放；这里不再全量扫，
  // 否则每次滚动都是 5000 次比较。

  // 提交顺序就是出图顺序：后端只有一个 pdfium 工作线程、按 FIFO 取任务。若按 DOM 顺序
  // （文档自上而下）提交，滚到第 300 页、缓冲区覆盖 297–303 时，会先栅格化 297 再轮到 300，
  // 用户在看的那页反而最后出图。改成按离视口中心由近及远排序，当前页永远第一个渲。
  const viewportCenter = (top + bottom) / 2
  active.sort((a, b) => Math.abs(a.center - viewportCenter) - Math.abs(b.center - viewportCenter))

  for (const { page } of active) {
    doc.renderPage(page, renderDpi.value)
    ensureText(page)
    // 链接与文字层一样按页懒加载：只对可见范围的页问一次，翻走的页不再重复请求。
    doc.loadLinks(page)
    if (props.features.annotations) doc.loadHighlights(page)
  }
}

// ─────────────────── 拖动平移（空白拖 / 空格 / 中键）───────────────────
// 交互对齐主流阅读器：不必先去点工具栏。在页面空白（行间空隙、页边、图区）按下即拖动平移；
// 在文字上按下仍然是划词选择（划词是划线高亮/翻译的入口，不能被拖动顶掉）；
// 按住空格可临时强制平移（松手还原）；中键拖动始终平移。
// 不再提供「锁定平移」工具：它只为在文字上拖动而存在，而文字上拖动的常规语义是选字。
//
// 松开鼠标就停：不做惯性滑动。读者用「按住—拖动—松开」来精确停在某一段，
// 松手后纸面继续滑行反而要再拉回来一次。
const spaceHeld = ref(false)
const isPanning = ref(false)
let panState = null
const nowMs = () => (typeof performance !== 'undefined' ? performance.now() : 0)
// 刚结束一次拖动的时间戳：拖动结束时落在链接/高亮上的 click 不应被当成「点开了它」。
let panEndedAt = 0
const PAN_CLICK_SUPPRESS_MS = 250

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
  // 空格临时平移 = 明确要求平移，这时连文字上也要能拖；其余情况左键落在文字上就是划词，不抢。
  if (ev.button === 0 && !spaceHeld.value && isOnSelectableText(ev)) return
  // 右键留给上下文菜单；中键与左键进入平移。
  if (ev.button !== 0 && ev.button !== 1) return
  ev.preventDefault()
  isPanning.value = true
  panState = {
    x: ev.clientX, y: ev.clientY,
    left: el.scrollLeft, top: el.scrollTop,
    // 只有真的拖动过才算「刚拖完」：单纯点一下链接/高亮也要能触发它们的 click。
    moved: false,
  }
  window.addEventListener('mousemove', onPanMove, true)
  window.addEventListener('mouseup', onPanUp, true)
}
const onPanMove = (ev) => {
  const el = bodyRef.value
  if (!panState || !el) return
  if (Math.abs(ev.clientX - panState.x) + Math.abs(ev.clientY - panState.y) > 3) panState.moved = true
  el.scrollLeft = panState.left - (ev.clientX - panState.x)
  el.scrollTop = panState.top - (ev.clientY - panState.y)
}
const onPanUp = () => {
  window.removeEventListener('mousemove', onPanMove, true)
  window.removeEventListener('mouseup', onPanUp, true)
  isPanning.value = false
  const st = panState
  panState = null
  // 只有真的拖动过才抑制随后的 click；单纯点一下不抑制。
  if (st?.moved) panEndedAt = nowMs()
  scheduleRenderVisible()
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

/** 链接热区：与高亮同用「页内 pt × 缩放」的换算，保证与纸面文字对齐。 */
const linkStyle = (lk) => {
  const s = cssScale.value
  return {
    left: `${lk.x * s}px`,
    top: `${lk.y * s}px`,
    width: `${Math.max(lk.w * s, 4)}px`,
    height: `${Math.max(lk.h * s, 4)}px`,
  }
}

/**
 * 点击 PDF 里的链接。
 *
 * 内部跳转直接 goToPage；外部网址交给系统浏览器 —— 不能在 webview 里直接导航，
 * 那会把整个阅读器变成那个网页，读者除了关窗口没有退路（EPUB 阅读器同样处理）。
 *
 * 拖动平移期间/刚结束时也不触发：那时用户是在拖纸面，不该顺带点开链接。
 */
const onLinkClick = async (lk, pageNum, ev) => {
  if (!lk || spaceHeld.value || isPanning.value) return
  // 从按下到松开指针移动过（刚完成拖动）时，落点上的 click 不是点击意图。
  if (nowMs() - panEndedAt < PAN_CLICK_SUPPRESS_MS) return
  if (lk.uri) {
    ev?.preventDefault?.()
    try {
      await openUrl(lk.uri)
    } catch (e) {
      logger.warn('PDF外链打开失败', { uri: lk.uri, error: String(e?.message || e) })
    }
    return
  }
  if (lk.page) {
    ev?.preventDefault?.()
    logger.info('PDF内部链接跳转', { from: pageNum, to: lk.page })
    goToPage(lk.page)
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
  // 刚拖完纸面：落在高亮上的 click 是拖动的副产品，不是「选中这条批注」。
  if (isPanning.value || nowMs() - panEndedAt < PAN_CLICK_SUPPRESS_MS) return
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
    text: hl.text || '',
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
// 搜索框默认收起：工具栏上只留一个放大镜，点开才展开输入框。
// 常驻一个 130px 输入框会让「翻页 + 缩放」这组高频控件被推到右边，窄窗口里尤其明显。
const searchOpen = ref(false)
let searchTimer = null
let searchSeq = 0
let lastSearchedQuery = ''

const toggleSearch = async () => {
  searchOpen.value = !searchOpen.value
  if (searchOpen.value) {
    await nextTick()
    searchInputRef.value?.focus?.()
    return
  }
  // 收起时清空结果：否则高亮矩形会留在页面上，而搜索框已经不见了，
  // 读者没有任何入口能把它清掉。
  clearSearch('search-closed')
}

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
    // 增量搜索：每扫完一块就把已有结果显示出来，读者不必等整本扫完。
    // 首块命中后立刻跳转过去，后续块继续补进列表（缩略图式的「边搜边给」）。
    let firstChunkShown = false
    const resp = await doc.search(q, {
      onProgress: ({ hits: partial, scannedTo, total }) => {
        if (seq !== searchSeq) return
        if (!partial.length || !scannedTo) return
        searchHits.value = partial
        rebuildSearchRects()
        if (!firstChunkShown) {
          firstChunkShown = true
          gotoSearchHit(0, 'search-first-chunk')
        }
        if (total) {
          status.value = `搜索中… ${scannedTo}/${total} 页，${partial.length} 个匹配`
        }
      },
    })
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
  // 拖动判定交给 onBodyMouseUp 的位移检查：这里不能看 isPanning —— 纸面上的按下
  // 一律会先进入平移状态，而「没有位移」的按下仍然是点击。
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

// ─────────────────── 文本导出（整页 / 全文翻译用）───────────────────
/**
 * 取某一页的纯文本，按视觉行拼回段落。
 *
 * 数据源就是已加载的文本层（`page.lines`），不会额外调用后端 —— 读者能看到的那一页
 * 必然是已渲染的，它的文字一定在内存里。未加载时返回空串，由调用方先滚动到该页。
 *
 * 行拼接规则：同一行 span 内文字已是完整行；行与行之间，上一行以句末标点结尾
 * 就换行（视为段落边界），否则补一个空格 —— 中文排版里硬换行的句子要接回去，
 * 否则翻译接口会对半句话逐句翻，结果支离破碎。
 */
const pageTextAt = (num) => {
  const page = pages[Number(num) - 1]
  if (!page || !Array.isArray(page.lines) || !page.lines.length) return ''
  const rows = page.lines
    .map((l) => ({
      text: String(l.text || '').replace(/\s+$/g, ''),
      top: l.topPt,
    }))
    .filter((r) => r.text.trim())
    .sort((a, b) => a.top - b.top)
  let out = ''
  for (const row of rows) {
    if (!out) {
      out = row.text
      continue
    }
    if (/[。！？.!?:：;；】」』”’]$/.test(out)) out += '\n' + row.text
    else out += ' ' + row.text
  }
  return out.trim()
}

/**
 * 导出整本文本（逐页，空行分段）。
 *
 * 逐页读 `pageTextAt`，未渲染的页没有文本层可用 —— 这对「全文翻译」是个真实的限制：
 * 让用户翻完 5000 页再翻译没有意义。因此这里对未加载页显式跳过，并在返回值里告知
 * 实际取到的页数，由宿主决定是提示「先加载更多页」还是就按已加载的部分翻。
 */
const exportText = ({ fromPage = 1, toPage = 0 } = {}) => {
  const start = Math.max(1, Number(fromPage) || 1)
  const end = Math.min(pages.length, Number(toPage) || pages.length)
  const parts = []
  const loadedPages = []
  for (let n = start; n <= end; n += 1) {
    const t = pageTextAt(n)
    if (!t) continue
    loadedPages.push(n)
    parts.push(t)
  }
  return { text: parts.join('\n\n'), loadedPages, requested: { start, end } }
}

// ─────────────────── 正向：源码 -> PDF（含完整高亮框）───────────────────
const flashBox = ref(null)
let flashTimer = null

/**
 * 滚动到某页内的竖直位置（pt）。
 *
 * 与 goToPage 同理：不能用 `node.offsetTop` —— 目标页在虚拟滚动下可能还没挂载
 * （源码正向定位常跳到很远的页）。用 pageGeometry 算页面顶边，再加页内偏移。
 */
const scrollToPagePoint = (pageNum, yPt) => {
  const el = bodyRef.value
  if (!el) return
  const item = pageGeometry.value[pageNum - 1]
  if (!item) return
  // 跳转前把「离开的位置」记进阅读历史：搜索命中、大纲、外链都走这里。
  readingHistory.markJump()
  const target = item.top + yPt * cssScale.value - el.clientHeight * 0.35
  recomputeWindow()
  smoothScrollTo(el, target, 'page-point')
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

/**
 * 跳转到指定页。
 *
 * 滚动位置**完全由 pageGeometry 算出**，不再依赖目标页的 DOM 节点。
 *
 * 这是虚拟滚动带来的一处关键改写：以前每页都有 DOM，`node.offsetTop` 随时可读；
 * 现在只有视口附近十几页被挂载，跳到一个远处的页（比如 597 页里的第 500 页）
 * 时该节点根本不存在，旧写法会一路重试到失败。
 *
 * 正确做法是先算出目标页的理论位置直接滚过去，滚动事件里会自动重算窗口、
 * 把目标页挂载出来 —— 两者的数据源是同一份几何，位置天然一致。
 */
const goToPage = (n, { align = 'top' } = {}) => {
  const num = Math.min(Math.max(1, Math.round(Number(n) || 1)), totalPages.value || 1)
  const el = bodyRef.value
  if (!el) {
    logger.warn('PDF页码跳转失败：滚动容器未就绪', { targetPage: num })
    return
  }

  const geo = pageGeometry.value
  const item = geo[num - 1]
  if (!item) {
    logger.warn('PDF页码跳转失败：页码超出几何范围', {
      targetPage: num,
      totalPages: totalPages.value,
      geometry: geo.length,
    })
    return
  }

  const offset = align === 'center' ? el.clientHeight * 0.3 : 8
  // 首页顶边就是容器的 padding（12px），减去 offset 会得到 4px 这种「没滚到底」的值，
  // 视觉上留一截空白。直接贴顶更符合「跳到第一页」的预期。
  const top = num === 1 ? 0 : Math.max(0, item.top - offset)

  // 翻页 / 页码输入 / 目录跳转都从这里发出：先把离开的位置记进历史再滚。
  readingHistory.markJump()

  // 滚过去之后窗口会重算并挂载目标页；这里先把「当前页」定下来，
  // 避免滚动动画期间页码指示器还停在旧值。
  currentPage.value = num
  syncPageInput()
  emit('page-change', num)

  // 立即重算一次窗口：平滑滚动的第一帧之前就把目标页放进 DOM，
  // 用户不会看到一屏空白再慢慢填内容。
  recomputeWindow()

  // 目标页高优先级渲染，并顺带预热它的前后一页。
  // 翻页是「可预测的下一步」，提前把邻页栅格化好，连翻时就不再每页都等一次骨架。
  // 这里不 await：骨架只是兜底，真实目标是滚动动画停止时图已经在解码缓存里。
  const target = pages[num - 1]
  if (target) {
    doc.renderPage(target, renderDpi.value)
    doc.prefetchAdjacent(target, renderDpi.value)
  }

  smoothScrollTo(el, top, 'go-to-page')

  // 滚动结束后再校准一次（滚动过程中布局可能因图片加载而变化）。
  scheduleRenderVisible()
  logger.info('PDF页码跳转完成', { targetPage: num, totalPages: totalPages.value, top })
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
    page.highlights.push({ annot_id: tempId, rects: info.rects, color: rgba, note: note || '', text: info.text, pending: true })
  }

  const annotId = await doc.addHighlight({
    page: info.page,
    rects: info.rects,
    color: rgba,
    note,
    text: info.text,
  })
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
const reAddHighlight = async ({ page: pageNum, rects, color, note, text } = {}) => {
  if (!pageNum || !Array.isArray(rects) || !rects.length) return null
  const rgba = normalizeHlColor(color)
  const page = pages[pageNum - 1]
  const tempId = `tmp-${++hlTempSeq}`
  if (page) {
    if (!Array.isArray(page.highlights)) page.highlights = []
    page.highlights.push({ annot_id: tempId, rects, color: rgba, note: note || '', text: text || '', pending: true })
  }
  const annotId = await doc.addHighlight({ page: pageNum, rects, color: rgba, note, text })
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

/**
 * 取某条高亮的原始数据（矩形 + 颜色 + 笔记）。
 *
 * 宿主侧栏只有 `{ annotId, page, note, color }` 这样的摘要，没有矩形；
 * 而撤销「删除」需要完整 rects 才能在后端重建。数据只在 viewer 内存里有一份，
 * 所以由 viewer 提供这个查询，宿主在删除之前调一次。
 */
const findHighlightRaw = (pageNum, annotId) => {
  const page = pages[Number(pageNum) - 1]
  const list = Array.isArray(page?.highlights) ? page.highlights : []
  const hit = list.find((h) => String(h.annot_id) === String(annotId))
  if (!hit) return null
  return {
    page: Number(pageNum),
    annotId: hit.annot_id,
    note: hit.note || '',
    color: hit.color,
    rects: Array.isArray(hit.rects) ? hit.rects.map((r) => ({ ...r })) : [],
    text: hit.text || '',
  }
}

defineExpose({
  forwardSync,
  goToPage,
  flashAnchor,
  findHighlightRaw,
  search: runSearch,
  runSearch,
  setSearchQuery: (q) => { searchQuery.value = q },
  nextHit,
  prevHit,
  clearSearch,
  getOutline: doc.getOutline,
  getSelectionInfo,
  getSelectionRects,
  // 整页 / 全文翻译的数据源
  pageTextAt,
  exportText,
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
  setSpacePan: (v) => { spaceHeld.value = !!v },
  // 阅读位置前后：宿主外置工具栏（GuaxiConvertPDFReader）也走这两个入口，
  // 栈与快捷键都在 viewer 里，宿主不再自己维护一份。
  goReadingBack: readingHistory.back,
  goReadingForward: readingHistory.forward,
  state: {
    zoom,
    rotation,
    spaceHeld,
    isPanning,
    currentPage,
    pageInputValue,
    totalPages,
    searchHits,
    searchActiveIdx,
    searchQuery,
    canReadBack: readingHistory.canBack,
    canReadForward: readingHistory.canForward,
  },
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
  // Ctrl/⌘ + 滚轮：必须用非 passive 的原生监听才能 preventDefault 掉 WebView2 的整页缩放。
  // Vue 的 @wheel 走 addEventListener 默认值，但宿主页面里任何一处替换成 passive 都会静默失效；
  // 这里显式 { passive: false }，保证「缩放的是 PDF 而不是整个窗口」。
  bodyRef.value?.addEventListener('wheel', onWheel, { passive: false })
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
  if (readingCommitTimer) clearTimeout(readingCommitTimer)
  if (scrollRaf) cancelAnimationFrame(scrollRaf)
  if (resizeRenderRaf) cancelAnimationFrame(resizeRenderRaf)
  bodyRef.value?.removeEventListener('wheel', onWheel, { passive: false })
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
  /* 尺寸交给宿主，但**只靠 flex 拉伸**，绝不写 width/height。
     【为什么不能写 width: 100%】宿主（ReaderPane）靠 `left: 260px; right: 0` 给侧栏
     让位。绝对定位元素同时有 left/right 和 width 时，width 胜出、right 被忽略，
     于是阅读器仍占满整条宽度、只是整体右移 260px —— 右边缘连同页面右半截直接落到
     窗口外（被 overflow 裁掉），表现就是「缩放后右侧被截断」。更糟的是容器尺寸没变，
     ResizeObserver 收不到通知，贴合宽度的自适应永远不重算。
     【为什么用 align-self/justify-self】有些宿主把它放进 grid 或块级容器
     （GuaxiSyntaxManualWindow 的 section 就是 grid 项），没有 width 时会塌成内容宽；
     stretch 只在对应轴上生效，flex 宿主里本来就是默认值，不会改变行为。 */
  align-self: stretch;
  justify-self: stretch;
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

/*
 * 配色取向（对齐 PDF Expert 一类「内容优先」的阅读器）：
 * 画布不是主角，纸面才是。所以画布用**中性的暖灰**而不是蓝灰，饱和度压到最低；
 * 页面本身不加描边、不加圆角，只用两层阴影把它从画布上托起来。
 * 深色主题同理：画布比工具栏更深，纸面（PDF 本身是白底）自然成为画面里最亮的东西。
 *
 * 之前最刺眼的是 #4a4d52 / #eef0f3 这类偏冷的灰，配上每页一圈 1px 描边和 2px 圆角，
 * 看起来像「一堆堆在灰色桌面上的卡片」，而不是正在读的一份文档。
 */
.gpv.theme-dark {
  --gpv-bg: #1a1c1f;
  --gpv-text: #dfe3e8;
  --gpv-toolbar-bg: #202326;
  --gpv-border: #303438;
  --gpv-canvas-area: #16181a;
  --gpv-btn-text: #c9ced4;
  --gpv-hover-bg: rgba(255, 255, 255, 0.07);
  --gpv-error-bg: rgba(0, 0, 0, 0.35);
  --gpv-overlay-bg: rgba(22, 24, 26, 0.72);
  --gpv-overlay-text: #e6e9ec;
  --gpv-canvas-sheen: radial-gradient(120% 70% at 50% 0%, rgba(255, 255, 255, 0.035), rgba(255, 255, 255, 0) 70%);
  /* 深色画布上，纸面靠「一圈极淡的亮边 + 一片大范围黑影」离开背景，
     不要再用 1px 的实描边（那会在白纸边缘画出一条脏线）。 */
  --gpv-paper-shadow: 0 0 0 1px rgba(255, 255, 255, 0.05), 0 2px 6px rgba(0, 0, 0, 0.45), 0 18px 40px -12px rgba(0, 0, 0, 0.75);
  --gpv-skeleton-bg: #23262a;
  --gpv-skeleton-sheen: rgba(255, 255, 255, 0.05);
  --gpv-skeleton-text: #4a4f56;
  --gpv-cluster-bg: rgba(255, 255, 255, 0.05);
  --gpv-cluster-border: rgba(255, 255, 255, 0.07);
  --gpv-menu-bg: #26292d;
  --gpv-menu-ring: rgba(255, 255, 255, 0.1);
}
.gpv.theme-light {
  --gpv-bg: #ffffff;
  --gpv-text: #1f2328;
  --gpv-toolbar-bg: #fbfbfa;
  --gpv-border: #e2e2df;
  --gpv-canvas-area: #e8e7e3;
  --gpv-btn-text: #3b4148;
  --gpv-hover-bg: rgba(15, 23, 42, 0.06);
  --gpv-error-bg: rgba(0, 0, 0, 0.04);
  --gpv-overlay-bg: rgba(232, 231, 227, 0.78);
  --gpv-overlay-text: #1f2328;
  --gpv-canvas-sheen: radial-gradient(120% 70% at 50% 0%, rgba(255, 255, 255, 0.7), rgba(255, 255, 255, 0) 70%);
  --gpv-paper-shadow: 0 1px 2px rgba(28, 25, 23, 0.08), 0 12px 28px -10px rgba(28, 25, 23, 0.32);
  --gpv-skeleton-bg: #f2f1ed;
  --gpv-skeleton-sheen: rgba(255, 255, 255, 0.9);
  --gpv-skeleton-text: #b9b7b1;
  --gpv-cluster-bg: rgba(15, 23, 42, 0.035);
  --gpv-cluster-border: rgba(15, 23, 42, 0.06);
  --gpv-menu-bg: #ffffff;
  --gpv-menu-ring: rgba(15, 23, 42, 0.1);
}

.gpv__toolbar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 6px;
  height: 38px;
  padding: 0 10px;
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
  position: relative;
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
  max-width: 34%;
  letter-spacing: 0.01em;
}
.gpv__status {
  font-size: 11px;
  opacity: 0.55;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex-shrink: 1;
  min-width: 0;
}
.gpv__status.ok { color: #2da44e; opacity: 0.9; }
.gpv.theme-dark .gpv__status.ok { color: #98c379; }
.gpv__status.error { color: #cf222e; opacity: 1; }
.gpv.theme-dark .gpv__status.error { color: #e06c75; }
.gpv__spacer { flex: 1 1 auto; min-width: 8px; }

/*
 * 分组：同一件事的几个控件装进一枚胶囊，组与组之间只留空隙。
 * 这替代了原先一列竖分隔线 —— 分隔线切出来的是「一排等价的按钮」，
 * 而读者心里其实是三组：找内容、翻页、调大小。
 */
.gpv__cluster {
  display: inline-flex;
  align-items: center;
  gap: 1px;
  padding: 2px;
  border-radius: 9px;
  background: var(--gpv-cluster-bg);
  box-shadow: inset 0 0 0 1px var(--gpv-cluster-border);
}

.gpv__search {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 2px;
  border-radius: 9px;
  background: var(--gpv-cluster-bg);
  box-shadow: inset 0 0 0 1px var(--gpv-cluster-border);
  transition: background-color 0.15s;
}
.gpv__search.is-open { background: var(--gpv-cluster-bg); }
.gpv__search-input {
  width: 168px;
  height: 24px;
  font-size: 12px;
  padding: 0 8px;
  border: none;
  border-radius: 7px;
  background: var(--gpv-bg);
  color: var(--gpv-text);
  box-shadow: inset 0 0 0 1px var(--gpv-cluster-border);
}
.gpv__search-input:focus { outline: none; box-shadow: inset 0 0 0 1px var(--n-color-primary, #61afef); }
.gpv__search-input::placeholder { color: var(--gpv-text); opacity: 0.4; }
.gpv__search-count {
  font-size: 11px;
  opacity: 0.65;
  min-width: 38px;
  text-align: center;
  font-variant-numeric: tabular-nums;
  padding-right: 2px;
}

.gpv__page-indicator {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 0 2px;
  font-size: 11px;
  color: var(--gpv-text);
  opacity: 0.9;
}
.gpv__page-input {
  width: 38px;
  height: 20px;
  font-size: 11px;
  padding: 0 2px;
  text-align: center;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--gpv-text);
  font-variant-numeric: tabular-nums;
}
.gpv__page-input:hover { background: var(--gpv-hover-bg); }
.gpv__page-input::-webkit-inner-spin-button,
.gpv__page-input::-webkit-outer-spin-button { -webkit-appearance: none; margin: 0; }
.gpv__page-input:focus {
  outline: none;
  background: var(--gpv-bg);
  box-shadow: inset 0 0 0 1px var(--n-color-primary, #61afef);
}
.gpv__page-total { opacity: 0.6; font-variant-numeric: tabular-nums; }

/* 百分比本身就是「适合宽度」按钮：点一下立刻贴合面板，这是读者最常按的那个动作。 */
.gpv__zoom {
  min-width: 46px;
  height: 24px;
  padding: 0 4px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--gpv-text);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  text-align: center;
  cursor: pointer;
  user-select: none;
}
.gpv__zoom:hover { background: var(--gpv-hover-bg); }
.gpv__zoom:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--n-color-primary, #61afef) 28%, transparent);
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
.gpv__btn:disabled { opacity: 0.32; cursor: not-allowed; }
.gpv__btn--icon {
  width: 28px;
  height: 26px;
  padding: 0;
  /* 图标按 1em 定尺寸，字号即图标大小（与父组件工具栏里的同款图标对齐） */
  font-size: 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  line-height: 0;
  border-radius: 7px;
}
.gpv__btn--icon svg {
  display: block;
  flex: 0 0 auto;
  /* 图标必须显式定尺寸：@vicons 组件输出的是只有 viewBox 的 <svg>，
     在 flex 按钮里会按 300×150 的默认尺寸撑开 —— 同一排里手型/搜索这些
     「直出 svg」的图标因此比手写的翻页箭头大一圈。统一成 16px。 */
  width: 16px;
  height: 16px;
  opacity: 0.85;
}
.gpv__btn--icon:hover:not(:disabled) svg { opacity: 1; }
.gpv__btn--active {
  background: color-mix(in srgb, var(--n-color-primary, #61afef) 16%, transparent);
  color: var(--n-color-primary, #61afef);
}
.gpv__btn--active svg { opacity: 1; }

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
  /* 光标按区域区分，不再全区域抓手：
       · 纸面（.gpv__page）是「你在读的东西」—— 箭头，和浏览器看 PDF 一致；
       · 纸外画布才是「可拖动平移」的区域 —— 抓手。
     之前整个 body 一律 grab，打开文档第一眼就是个张开的手，而且链接、图片、
     页码跳转这些「可点」的东西也跟着变成手型，用户分不出「能拖」和「能点」。
     文字段自己声明 cursor: text，链接覆盖为 pointer，三者互不干扰。 */
  cursor: default;
}
/* 纸外的画布：拖空白即平移，用抓手告诉用户这里可以拖。 */
.gpv__pages { cursor: grab; }
/* 纸面同样可拖动（行间空隙、页边、图片区），光标就给抓手；
   文字段自己声明 cursor: text，链接/高亮声明 pointer，三者互不干扰。
   没有手型工具之后，这一条是「这里能拖」的唯一视觉提示。 */
.gpv__page { cursor: grab; }
/* PDF 里的链接：可点击（跳到内部目标或打开外部网址）。 */
.gpv__link { cursor: pointer; }
/* 按住空格临时平移：连文字上也能拖，因此禁止选中，文字段也换手型 */
.gpv__body--pan { user-select: none; }
.gpv__body--pan .gpv__text-seg { user-select: none; cursor: grab; }
/* 拖动进行中（含中键、空白直接拖）：全区域统一攥拳光标 */
.gpv__body--panning,
.gpv__body--panning .gpv__page,
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
  /* 必须显式 border-box：本文件没有全局 `* { box-sizing }`，默认 content-box 下
     `min-width: 100%` 只撑满内容盒、再往外加 24px padding，于是页宽恰好放得下时
     也会多出一条横向滚动条（实测 982 vs 958）。border-box 下 100% 就是整条可视宽。 */
  box-sizing: border-box;
  min-width: 100%;
  /* 页间距与内边距一致：纸张之间是「一叠纸的空隙」，不是卡片之间的 margin。 */
  gap: 16px;
  padding: 16px;
}

/*
 * 窗口化滚动用的占位条。
 *
 * flex 容器里必须 flex:0 0 auto，否则 spacer 会被 flex 压缩成 0 高，
 * 滚动条总长度就没了 —— 表现为「滚到一半就到底」。
 * gap 已由容器提供，spacer 自身不再加间距，否则高度会多算一份。
 */
.gpv__spacer {
  flex: 0 0 auto;
  width: 1px;
  pointer-events: none;
}

.gpv__page {
  margin-left: auto;
  margin-right: auto;
  position: relative;
  background: #fff;
  /* 不要圆角、不要描边：PDF 本身就是一张矩形页面，给它加 2px 圆角和 1px 边框，
     等于把文档画成「卡片」，放大到页边时那圈线会明显切掉纸的边缘。 */
  border-radius: 0;
  /* 纸张感全部交给阴影：贴边一层压出厚度，远处一层托起高度。 */
  box-shadow: var(--gpv-paper-shadow);
  flex: 0 0 auto;
  overflow: hidden;
  /* 图加载完成时淡入，替代「灰块突然变成页面」的跳变；
     骨架和纸面同底色，所以过渡期间看不出闪烁。 */
  transition: background-color 0.18s ease;
}
/* 旋转内层：尺寸/位移/旋转全部走内联样式（pageInnerStyle） */
.gpv__page-inner { position: absolute; }
.gpv__page-img { display: block; user-select: none; }
/*
 * 未渲染页的占位。
 *
 * 【为什么不再用灰块】原先整页铺 #f7f8fa 再扫一道高光，在浅色画布上就是一块明显的
 * 灰矩形 —— 翻页的那一瞬间，读者看到的是「一张灰卡片」而不是「这张纸正在显影」。
 * 现在改成：底色就是纸色（白），只留极淡的页码和一条细扫描线，视觉上等同于
 * 「白纸正在变清晰」，与 PDF Expert / Preview 那种即时出图的感觉一致。
 *
 * 骨架已经预先把页图解码进缓存（见 usePdfiumDocument.decodeImage），所以这里大多数
 * 时候只在首屏出现一瞬，不该喧宾夺主。
 */
.gpv__page-skeleton {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  color: var(--gpv-skeleton-text);
  background-color: #fff;
  background-image: linear-gradient(
    180deg,
    rgba(255, 255, 255, 0) 0%,
    var(--gpv-skeleton-sheen) 50%,
    rgba(255, 255, 255, 0) 100%
  );
  background-size: 100% 46%;
  background-repeat: no-repeat;
  background-position: 0 -60%;
  animation: gpv-skeleton-sweep 1.4s ease-in-out infinite;
}
@keyframes gpv-skeleton-sweep {
  0% { background-position: 0 -60%; }
  100% { background-position: 0 160%; }
}
@media (prefers-reduced-motion: reduce) {
  .gpv__page-skeleton { animation: none; }
}
/* 深色主题下「纸」仍是白的（PDF 本身白底），骨架跟着用白，才不会有灰块闪一下。 */
.gpv.theme-dark .gpv__page-skeleton {
  background-color: #fafafa;
  color: #c9c9c4;
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

/*
 * 链接热区：透明、可点。
 *
 * 不给底色 —— PDF 自己通常已经把链接画成蓝色下划线，再叠一层就是我们替书做了设计。
 * 只在悬停时给一圈极淡的描边，让「这里能点」在点下去之前就能看出来。
 */
.gpv__link {
  position: absolute;
  cursor: pointer;
  border-radius: 2px;
  background: transparent;
}
.gpv__link:hover {
  background: rgba(97, 175, 239, 0.12);
  box-shadow: 0 0 0 1px rgba(97, 175, 239, 0.35);
}
/* 锁定平移 / 空格临时平移时，链接跟随全局抓手：此时用户的意图是拖纸面，不是点链接。 */
.gpv__body--pan .gpv__link,
.gpv__body--panning .gpv__link { cursor: grab; }

/* 已保存的高亮批注覆盖层 */.gpv__hl-rect {
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
