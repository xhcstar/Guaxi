<template>
  <!--
    阅读器承载层。

    设计约束（零回归的关键）：
    1. 本组件是**纯覆盖层**：以 position:absolute; inset:0 盖在 `editor-container` 之上。
       Monaco 的 DOM、model、布局与生命周期完全不受影响 —— 不 hide、不 unmount、不 resize。
       切回文本标签时本组件整体卸载，Monaco 仍是它离开时的样子。
    2. 只处理「由 openTargetKind 判定为非文本」的标签；文本标签根本不会挂载本组件。
    3. 所有后端调用都是新增命令；PDF 复用 pdfium 会话与虚拟滚动，不另起一套渲染。
  -->
  <div v-if="target" ref="pdfRootEl" class="guaxi-doc-reader" :data-reader="target.reader" @keydown.stop>
    <template v-if="target.reader === 'pdf'">
      <!--
        PDF 的辅助侧栏：大纲 / 批注 二选一。

        为什么做成互斥而不是两个独立面板：PDF 正文本身是横向空间最紧张的
        （大页面 + 缩放），同时开两个侧栏会把正文挤到不可用的宽度。
      -->
      <aside v-if="pdfAside" class="gdr-pdf-aside" :style="{ width: `${asideWidth}px` }">
        <div class="gdr-pdf-aside__head">
          <div class="gdr-pdf-aside__tabs" role="tablist">
            <button
              class="gdr-pdf-aside__tab"
              :class="{ 'is-on': pdfAside === 'outline' }"
              role="tab"
              :aria-selected="pdfAside === 'outline'"
              title="文档大纲"
              @click="pdfAside = 'outline'"
            >
              大纲
            </button>
            <button
              class="gdr-pdf-aside__tab"
              :class="{ 'is-on': pdfAside === 'notes' }"
              role="tab"
              :aria-selected="pdfAside === 'notes'"
              :title="`批注${pdfHighlights.length ? `（${pdfHighlights.length} 条）` : ''}`"
              @click="pdfAside = 'notes'"
            >
              批注
            </button>
            <button
              class="gdr-pdf-aside__tab"
              :class="{ 'is-on': pdfAside === 'translate' }"
              role="tab"
              :aria-selected="pdfAside === 'translate'"
              title="本页译文"
              @click="pdfAside = 'translate'"
            >
              译文
            </button>
          </div>
          <button
            class="guaxi-doc-reader__icon-btn"
            title="关闭侧栏"
            aria-label="关闭侧栏"
            @click="pdfAside = ''"
          >
            <Icon :icon="closeIcon" width="15" height="15" />
          </button>
        </div>

        <div v-if="pdfAside === 'outline'" class="gdr-pdf-aside__body">
          <div v-if="!outlineItems.length" class="gdr-pdf-aside__empty">
            这份 PDF 没有内嵌目录。
          </div>
          <button
            v-for="(item, idx) in outlineItems"
            :key="`${idx}-${item.title}`"
            class="gdr-pdf-aside__item"
            :class="{ 'is-active': outlineActiveIdx === idx }"
            :style="{ paddingLeft: `${8 + (item.depth || 0) * 12}px` }"
            :title="item.title"
            @click="jumpToOutline(item, idx)"
          >
            {{ item.title || '未命名' }}
          </button>
        </div>

        <!-- 译文：只翻当前页，结果按段并排给出原/译对照。 -->
        <div v-else-if="pdfAside === 'translate'" class="gdr-pdf-aside__body">
          <div class="gdr-xlate">
            <div class="gdr-xlate__head">
              <span class="gdr-xlate__title">{{ typeof pageTranslateTarget === 'number' ? `第 ${pageTranslateTarget} 页` : pageTranslateTarget }}</span>
              <button
                class="gdr-xlate__btn"
                type="button"
                :disabled="pageTranslateBusy"
                @click="translateCurrentPage"
              >
                {{ pageTranslateBusy ? '翻译中…' : pageTranslateRows.length ? '重新翻译' : '翻译本页' }}
              </button>
            </div>
            <div v-if="pageTranslateBusy" class="gdr-xlate__progress">
              <span class="gdr-xlate__bar" :style="{ width: `${Math.round(pageTranslateProgress * 100)}%` }" />
            </div>
            <!-- 范围：单页 vs 全文。全文只翻「已经加载过文本层」的页，见 translateWholeDoc。 -->
            <div class="gdr-xlate__scope">
              <button
                class="gdr-xlate__scope-btn"
                :class="{ 'is-on': xlateScope === 'page' }"
                type="button"
                @click="xlateScope = 'page'"
              >本页</button>
              <button
                class="gdr-xlate__scope-btn"
                :class="{ 'is-on': xlateScope === 'book' }"
                type="button"
                @click="xlateScope = 'book'"
              >全文</button>
              <span v-if="xlateScope === 'book'" class="gdr-xlate__scope-note">
                只翻已加载的页，滚动过的部分才可翻
              </span>
            </div>
            <div v-if="pageTranslateError" class="gdr-xlate__error">{{ pageTranslateError }}</div>
            <div v-if="!pageTranslateRows.length && !pageTranslateBusy" class="gdr-pdf-aside__empty">
              把这一页的文字翻成中文。段落与原文逐段对照，方便边读边核对。
            </div>
            <div v-for="(row, i) in pageTranslateRows" :key="i" class="gdr-xlate__row">
              <p class="gdr-xlate__src">{{ row.source }}</p>
              <p v-if="row.target" class="gdr-xlate__dst">{{ row.target }}</p>
              <p v-else-if="pageTranslateBusy" class="gdr-xlate__pending">…</p>
            </div>
          </div>
        </div>

        <div v-else class="gdr-pdf-aside__body">
          <div v-if="!pdfHighlights.length" class="gdr-pdf-aside__empty">
            还没有批注。在正文中选中文字后点工具栏的高亮按钮即可添加。
          </div>
          <div v-for="hl in pdfHighlights" :key="hl.annotId" class="gdr-pdf-aside__note">
            <button class="gdr-pdf-aside__note-main" @click="jumpToHighlight(hl)">
              <span class="gdr-pdf-aside__note-swatch" :style="{ background: rgbaToCss(hl.color) }" />
              <span class="gdr-pdf-aside__note-text">
                <span class="gdr-pdf-aside__note-note">{{ hl.note || '（无笔记）' }}</span>
                <span class="gdr-pdf-aside__note-sub">第 {{ hl.page }} 页</span>
              </span>
            </button>
            <button
              class="gdr-pdf-aside__note-del"
              title="删除这条批注"
              aria-label="删除这条批注"
              @click.stop="removePdfHighlight(hl)"
            >
              <Icon :icon="closeIcon" width="13" height="13" />
            </button>
          </div>
        </div>
      </aside>

      <GuaxiPdfViewer
        ref="pdfViewerRef"
        class="gdr-pdf"
        :class="{ 'gdr-pdf--with-aside': !!pdfAside }"
        :path="path"
        :theme="theme"
        :title-label="fileName"
        :initial-zoom="1.2"
        fit-mode="width"
        :features="pdfFeatures"
        @outline="onOutline"
        @page-change="onPdfPageChange"
        @highlight-click="onPdfHighlightClick"
      />

      <!--
        划词高亮条：浮在选区附近。

        为什么由宿主提供、而不是塞进 GuaxiPdfViewer 的工具栏：阅读器是通用组件，
        LaTeX 预览那边的高亮靠编辑器侧驱动，它不需要这个按钮；而「读完一段想划下来」
        是独立阅读场景最核心的动作，缺了它阅读器就只能看不能记。

        只在真正选中文字时出现：不做常驻按钮，避免和缩放/翻页这些「随时可用」的控件
        混在一起，也让「选中 → 高亮」这个因果一眼可见。
      -->
      <div
        v-if="pdfSelectionToolbar"
        class="gdr-pdf-hl-bar"
        :style="{ left: `${pdfSelectionAnchor.left}px`, top: `${pdfSelectionAnchor.top}px` }"
        @mousedown.prevent
      >
        <span class="gdr-pdf-hl-bar__hint">{{ pdfSelectionPreview }}</span>
        <!--
          译 / 读：读完一段想弄懂它，比「想标记它」更常见，
          所以这两个动作排在高亮色之前。
        -->
        <button class="gdr-pdf-hl-bar__action" title="翻译选中的文字" @click="openPdfTranslate">
          翻译
        </button>
        <button
          class="gdr-pdf-hl-bar__action"
          :title="pdfReadAloudPlaying ? '停止朗读' : '朗读选中的文字'"
          @click="speakPdfSelection"
        >
          {{ pdfReadAloudPlaying ? '停止' : '朗读' }}
        </button>
        <span class="gdr-pdf-hl-bar__sep" />
        <span class="gdr-pdf-hl-bar__swatches">
          <button
            v-for="(c, i) in PDF_HL_PRESETS"
            :key="i"
            class="gdr-pdf-hl-bar__swatch"
            :class="{ 'is-on': i === pdfHighlightPresetIdx }"
            :style="{ background: rgbaToCss(c) }"
            :title="'用此颜色高亮'"
            @click="addPdfHighlightFromSelection(c)"
          />
        </span>
        <button class="gdr-pdf-hl-bar__cancel" title="取消选择（Esc）" @click="clearPdfSelection">
          Esc
        </button>
      </div>

      <!-- 划词译文面板：与电子书 / 编辑器同一套组件，交互一致。 -->
      <GuaxiSelectionTranslatePanel
        v-if="pdfTranslate.visible.value"
        :pos="pdfTranslate.pos.value"
        :raw="pdfTranslate.raw.value"
        :result="pdfTranslate.result.value"
        :error="pdfTranslate.error.value"
        :busy="pdfTranslate.busy.value"
        :pinned="pdfTranslate.pinned.value"
        :items="pdfTranslate.items.value"
        :from-lang="pdfTranslate.fromLang.value"
        :to-lang="pdfTranslate.toLang.value"
        :lang-options="pdfTranslate.langOptions"
        :word-meta="pdfTranslate.wordMeta.value"
        @translate="pdfTranslate.translate(pdfTranslate.raw.value)"
        @copy="pdfTranslate.copyResult()"
        @swap-langs="pdfTranslate.swapLangs()"
        @toggle-pin="pdfTranslate.togglePin()"
        @update:pos="(v) => { pdfTranslate.pos.value = v }"
        @update:from-lang="(v) => { pdfTranslate.fromLang.value = v }"
        @update:to-lang="(v) => { pdfTranslate.toLang.value = v }"
        @close="pdfTranslate.close()"
      />

      <!--
        侧栏开关：浮在正文左上角。

        为什么不放进 PDF 阅读器的工具栏：GuaxiPdfViewer 是通用组件，还被
        LaTeX 预览、资料库阅读器等多处复用；把「大纲/批注」这类宿主专属的入口
        塞进它，会让其它调用方也背上这两个用不上的按钮。
      -->
      <div v-if="!pdfAside" class="gdr-pdf-switch">
        <button
          class="guaxi-doc-reader__icon-btn"
          :disabled="!outlineItems.length"
          :title="outlineItems.length ? '显示文档大纲' : '这份 PDF 没有内嵌目录'"
          aria-label="显示文档大纲"
          @click="pdfAside = 'outline'"
        >
          <Icon :icon="outlineIcon" width="15" height="15" />
        </button>
        <button
          class="guaxi-doc-reader__icon-btn"
          title="本页译文"
          aria-label="本页译文"
          @click="pdfAside = 'translate'"
        >
          <Icon :icon="translateIcon" width="15" height="15" />
        </button>
        <button
          class="guaxi-doc-reader__icon-btn"
          title="显示批注列表"
          aria-label="显示批注列表"
          @click="pdfAside = 'notes'"
        >
          <Icon :icon="noteIcon" width="15" height="15" />
          <span v-if="pdfHighlights.length" class="gdr-pdf-switch__badge">{{ pdfHighlights.length }}</span>
        </button>
      </div>
    </template>

    <DocImageReader
      v-else-if="target.reader === 'image'"
      :path="path"
      :name="fileName"
      :mime="target.mime"
      :variant="'single'"
    />

    <!--
      Office 文档（word / spreadsheet / slides）统一交给 DocOfficeReader。

      【为什么不再按格式分派到各自的只读渲染器】
      docx-preview、@vue-office/pptx、calamine 拼出来的版式永远是「差不多但不对」：
      分页、母版、图表、修订都还原不了。更糟的是它把同一份文件变成两种东西 ——
      在这里看到的样子和用 Office 打开的样子不一样，用户无法信任其中任何一个。
      现在只有一条路：交给真正会编辑它的程序（本机 Office/WPS，否则 WordCraft 系列），
      直接在原文件上工作，Guaxi 不产出任何中间产物。
    -->
    <DocOfficeReader
      v-else-if="isOfficeReader"
      :path="path"
      :name="fileName"
      :kind="target.kind"
      :size="target.size"
    />

    <DocEbookReader
      v-else-if="target.reader === 'ebook'"
      :path="path"
      :name="fileName"
      :kind="target.kind"
    />

    <DocHtmlReader
      v-else-if="target.reader === 'html'"
      :path="path"
      :name="fileName"
    />

    <DocMediaReader
      v-else-if="target.reader === 'media'"
      :path="path"
      :name="fileName"
      :kind="target.kind"
    />

    <!-- 兜底：无法识别的二进制。给出可操作出口，而不是空白。 -->
    <div v-else class="guaxi-doc-reader__fallback">
      <div class="guaxi-doc-reader__bar">
        <span class="guaxi-doc-reader__title" :title="path">{{ fileName }}</span>
        <span class="guaxi-doc-reader__badge">BINARY</span>
        <span class="guaxi-doc-reader__meta">{{ sizeLabel }}</span>
        <span class="guaxi-doc-reader__spacer" />
        <!-- 兜底面板的三条出路都带图标但仍保留文字：这是「无路可走」的时刻，
             图标单独一个（folder / file-output / code）不足以说明后果。 -->
        <button class="guaxi-doc-reader__btn" @click="openExternally">
          <Icon :icon="folderIcon" width="14" height="14" />
          在文件浏览器中打开
        </button>
        <button class="guaxi-doc-reader__btn" @click="$emit('convert-to-markdown', path)">
          <Icon :icon="convertIcon" width="14" height="14" />
          转换为 Markdown
        </button>
        <button class="guaxi-doc-reader__btn" @click="$emit('force-editor', path)">
          <Icon :icon="codeIcon" width="14" height="14" />
          用编辑器打开
        </button>
      </div>
      <div class="guaxi-doc-reader__body">
        <div class="guaxi-doc-reader__center">
          <div>该类型暂无可用的阅读器。</div>
          <div class="guaxi-doc-reader__meta">可以尝试转换为 Markdown，或用系统程序打开。</div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Icon } from '@iconify/vue'
import closeIcon from '@iconify-icons/lucide/x'
import outlineIcon from '@iconify-icons/lucide/list-tree'
import noteIcon from '@iconify-icons/lucide/edit-3'
import translateIcon from '@iconify-icons/lucide/languages'
import folderIcon from '@iconify-icons/lucide/folder'
import convertIcon from '@iconify-icons/lucide/file-output'
import codeIcon from '@iconify-icons/lucide/code'
import { openPath } from '@tauri-apps/plugin-opener'
import { createLogger } from '../../utils/logger'
import GuaxiPdfViewer from '../PdfViewer/GuaxiPdfViewer.vue'
import DocImageReader from '../ReaderComponent/readers/DocImageReader.vue'
import DocEbookReader from '../ReaderComponent/readers/DocEbookReader.vue'
import DocHtmlReader from '../ReaderComponent/readers/DocHtmlReader.vue'
import DocMediaReader from '../ReaderComponent/readers/DocMediaReader.vue'
import DocOfficeReader from '../ReaderComponent/readers/DocOfficeReader.vue'
import GuaxiSelectionTranslatePanel from './GuaxiSelectionTranslatePanel.vue'
import { usePageTranslate, splitIntoParagraphs } from '../../composables/usePageTranslate'
import { useSelectionReadAloud } from '../../composables/useSelectionReadAloud'

const logger = createLogger('ReaderPane')

/** word / spreadsheet / slides 三个 reader 分组，现在都由 DocOfficeReader 承接。 */
const isOfficeReader = computed(() =>
  ['word', 'spreadsheet', 'slides'].includes(String(props.target?.reader || ''))
)

const props = defineProps({
  /** 阅读器描述对象（openTargetKind.resolveOpenTarget 的返回值）。null 表示不接管。 */
  target: { type: Object, default: null },
  path: { type: String, required: true },
  fileName: { type: String, default: '' },
  theme: { type: String, default: 'light' },
})

defineEmits(['force-editor', 'convert-to-markdown'])

// PDF 阅读器在「打开一份现成 PDF」时不需要 LaTeX 双向定位/重建/导出，
// 但搜索、分页、大纲、高亮批注都是有用的，全部打开。
const pdfFeatures = computed(() => ({
  toolbar: true,
  search: true,
  pagination: true,
  outline: true,
  annotations: true,
  reverseJump: false,
  export: false,
  rebuild: false,
}))

const sizeLabel = computed(() => {
  const size = Number(props.target?.size || 0)
  if (!size) return ''
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
  return `${(size / 1024 / 1024).toFixed(1)} MB`
})

// ───────────────────────── PDF 侧栏（大纲 / 批注）─────────────────────────

const pdfViewerRef = ref(null)
/** '' | 'outline' | 'notes' —— 互斥，正文只让出一条宽度。 */
const pdfAside = ref('')
const asideWidth = ref(260)
const outlineItems = ref([])
const outlineActiveIdx = ref(-1)
const pdfHighlights = ref([])

/**
 * 接收 PDF 阅读器解析出的大纲。
 *
 * 阅读器把嵌套结构拍平成带 depth 的数组，这里直接按 depth 缩进渲染 ——
 * 比递归组件简单，且大纲通常只有几十到几百项。
 */
const onOutline = (raw) => {
  const flat = []
  const walk = (items, depth) => {
    if (!Array.isArray(items)) return
    for (const item of items) {
      if (flat.length >= 2000) return
      flat.push({ title: String(item?.title || item?.label || ''), page: Number(item?.page) || 0, depth })
      walk(item?.children || item?.subitems, depth + 1)
    }
  }
  walk(raw, 0)
  outlineItems.value = flat
  // 有大纲才自动展开侧栏；没有的话保持关闭，避免开出一个空面板。
  if (flat.length && !pdfAside.value) pdfAside.value = 'outline'
  logger.info('[ReaderPane] pdf outline ready', { items: flat.length })
}

/** 大纲项高亮跟随当前页：取最后一个起始页不晚于当前页的条目。 */
const onPdfPageChange = (page) => {
  const num = Number(page) || 0
  pdfCurrentPage.value = num || 1
  let idx = -1
  outlineItems.value.forEach((item, i) => {
    if (item.page && item.page <= num) idx = i
  })
  outlineActiveIdx.value = idx
}

const jumpToOutline = (item, idx) => {
  outlineActiveIdx.value = idx
  const page = Number(item?.page) || 0
  if (!page) return
  try {
    pdfViewerRef.value?.goToPage?.(page)
  } catch (error) {
    logger.warn('[ReaderPane] outline jump failed', {
      page,
      error: String(error?.message || error),
    })
  }
}

/** [r,g,b,a] → css rgba。a 是 0..255。 */
const rgbaToCss = (color) => {
  if (!Array.isArray(color) || color.length < 3) return 'rgba(255, 235, 59, .5)'
  const [r, g, b, a] = color
  const alpha = a == null ? 0.5 : Number(a) / 255
  return `rgba(${r}, ${g}, ${b}, ${alpha.toFixed(3)})`
}

/** 重新拉取全书批注（新增/删除后调用）。 */
const refreshPdfHighlights = async () => {
  if (props.target?.reader !== 'pdf') return
  try {
    const list = await invoke('pdf_list_highlights', { path: props.path })
    pdfHighlights.value = (Array.isArray(list) ? list : []).map((h) => ({
      annotId: h.annotId,
      page: h.page,
      note: h.note || '',
      color: h.color,
    }))
    logger.info('[ReaderPane] pdf highlights loaded', { count: pdfHighlights.value.length })
  } catch (error) {
    // 读不到批注不该影响阅读本身。
    logger.warn('[ReaderPane] list pdf highlights failed', {
      error: String(error?.message || error),
    })
  }
}

const jumpToHighlight = (hl) => {
  const page = Number(hl?.page) || 0
  if (!page) return
  try {
    pdfViewerRef.value?.goToPage?.(page)
  } catch (error) {
    logger.warn('[ReaderPane] highlight jump failed', {
      page,
      error: String(error?.message || error),
    })
  }
}

const removePdfHighlight = async (hl) => {
  // 删除会直接改写 .pdf 文件，且 PDF 阅读器没有撤销栈 —— 必须先确认。
  const confirmed = window.confirm?.(
    `删除这条批注？\n\n${hl.note || '（无笔记）'}\n第 ${hl.page} 页\n\n此操作会修改 PDF 文件本身，无法撤销。`
  )
  if (!confirmed) return
  try {
    await pdfViewerRef.value?.deleteHighlight?.(hl.page, hl.annotId)
    pdfHighlights.value = pdfHighlights.value.filter((item) => item.annotId !== hl.annotId)
    logger.info('[ReaderPane] pdf highlight deleted', { page: hl.page })
  } catch (error) {
    logger.warn('[ReaderPane] delete pdf highlight failed', {
      error: String(error?.message || error),
    })
  }
}

/** 用户点了正文里的高亮：切到批注页并定位到那一条。 */
const onPdfHighlightClick = (payload) => {
  const annotId = String(payload?.annotId || payload?.annot_id || '')
  if (!annotId) return
  pdfAside.value = 'notes'
  nextTick(() => {
    const el = document.querySelector(`[data-pdf-annot="${annotId}"]`)
    el?.scrollIntoView({ block: 'center', behavior: 'smooth' })
  })
}

/** 阅读器新增了高亮：面板开着时同步刷新列表。 */
const onPdfHighlightAdded = () => {
  if (pdfAside.value === 'notes') void refreshPdfHighlights()
}

// ───────────────────────── 划词高亮（独立阅读场景）─────────────────────────
// 用户在正文里选中一段文字 → 底部浮出颜色条 → 点一个颜色即写入 .pdf。
// 写回由 GuaxiPdfViewer.addHighlightFromSelection 负责（乐观覆盖层 + 后台落盘），
// 这里只管交互与「高亮后同步批注列表」。

/** 高亮色板：与转换阅读器一致，最后一位是 0..255 的 alpha。 */
const PDF_HL_PRESETS = [
  [255, 235, 59, 110],   // 黄
  [76, 217, 100, 110],   // 绿
  [64, 196, 255, 110],   // 蓝
  [255, 145, 180, 110],  // 粉
  [255, 159, 67, 110],   // 橙
]
const pdfHighlightPresetIdx = ref(0)
const pdfSelectionToolbar = ref(false)
const pdfSelectionText = ref('')
/** 浮动条的左上角（相对阅读器容器）。 */
const pdfSelectionAnchor = ref({ left: 0, top: 0 })
/** 浮条上展示的选区摘要：太长会把浮条撑到半屏，宁可在浮条上截断。 */
const pdfSelectionPreview = computed(() => {
  const t = String(pdfSelectionText.value || '').trim().replace(/\s+/g, ' ')
  if (!t) return ''
  return t.length > 24 ? `${t.slice(0, 24)}…` : t
})
const pdfRootEl = ref(null)

/**
 * 选好文字后算浮条位置。
 *
 * 用选区第一个 range 的屏幕矩形，而不是跟随鼠标：缩放、旋转、滚动之后鼠标位置
 * 与选区早已无关，而矩形始终贴着那段文字。浮条放在选区下方；下方放不下就翻到上方。
 * 坐标换算到阅读器容器内，浮条因此用 absolute 定位、不受滚动影响。
 */
const placePdfSelectionToolbar = () => {
  const sel = window.getSelection?.()
  if (!sel || !sel.rangeCount) return false
  const rect = sel.getRangeAt(0).getBoundingClientRect()
  if (!rect || (!rect.width && !rect.height)) return false
  const root = pdfRootEl.value?.getBoundingClientRect?.()
  if (!root) return false
  const BAR_H = 34
  const left = Math.min(
    Math.max(8, rect.left - root.left + rect.width / 2),
    Math.max(8, root.width - 8)
  )
  const below = rect.bottom - root.top + 8
  const top = below + BAR_H <= root.height ? below : Math.max(8, rect.top - root.top - BAR_H - 8)
  // 浮条以 left 为中心，交给 CSS 的 translateX(-50%) 对齐。
  pdfSelectionAnchor.value = { left: Math.round(left), top: Math.round(top) }
  return true
}

/** 鼠标抬起 / 键盘选择后调用：有选中就浮出，没选中就收起。 */
const syncPdfSelectionToolbar = () => {
  if (props.target?.reader !== 'pdf') return
  const text = String(window.getSelection?.() || '').trim()
  if (!text) {
    pdfSelectionToolbar.value = false
    pdfSelectionText.value = ''
    return
  }
  pdfSelectionText.value = text
  if (placePdfSelectionToolbar()) pdfSelectionToolbar.value = true
}

const clearPdfSelection = () => {
  window.getSelection?.()?.removeAllRanges()
  pdfSelectionToolbar.value = false
  pdfSelectionText.value = ''
}

/**
 * 给当前选区加上指定颜色的高亮。
 *
 * 关键点：先把选区矩形算出来再清选区 —— addHighlightFromSelection 内部会
 * removeAllRanges，如果宿主这边先清，它拿到的就是空选区。
 */
const addPdfHighlightFromSelection = async (color) => {
  const idx = PDF_HL_PRESETS.indexOf(color)
  if (idx >= 0) pdfHighlightPresetIdx.value = idx
  const viewer = pdfViewerRef.value
  if (!viewer?.addHighlightFromSelection) return
  try {
    const res = await viewer.addHighlightFromSelection({ color })
    pdfSelectionToolbar.value = false
    pdfSelectionText.value = ''
    if (!res) {
      logger.warn('[ReaderPane] highlight skipped: no valid selection')
      return
    }
    logger.info('[ReaderPane] pdf highlight added', { page: res.page, annotId: res.annotId })
    // 新增的批注要让侧栏列表跟上；没开着也刷新一下，下次展开就是新的。
    void refreshPdfHighlights()
  } catch (error) {
    logger.warn('[ReaderPane] add pdf highlight failed', {
      error: String(error?.message || error),
    })
  }
}

/** 选区变化（键盘 Shift+方向键、双击选词）也要跟手。 */
// 划词翻译与朗读：PDF 阅读器的选区就在主文档里（透明文本层），所以用默认选区来源即可。
const pdfReadAloud = useSelectionReadAloud({})
const pdfReadAloudPlaying = pdfReadAloud.busy
const pdfTranslate = usePageTranslate({ getContainer: () => pdfRootEl.value || null })

const openPdfTranslate = () => {
  const text = String(pdfSelectionText.value || '').trim()
  if (!text) return
  pdfTranslate.translateText(text, pdfSelectionAnchor.value)
  pdfSelectionToolbar.value = false
}

const speakPdfSelection = () => {
  const text = String(pdfSelectionText.value || '').trim()
  if (!text) return
  if (pdfReadAloudPlaying.value) {
    pdfReadAloud.stop()
    return
  }
  void pdfReadAloud.toggle(text)
  pdfSelectionToolbar.value = false
}

/**
 * 全文翻译。
 *
 * 【为什么只翻已加载的页】文本层来自已渲染页，未渲染的页在内存里没有文字。
 * 让用户「先滚完 5000 页」显然不可行，所以这里如实按已加载范围翻，并在界面上说明。
 * 想要更完整的全文翻译，正确的做法是后端提供整本文本抽取 —— 那是另一件事，
 * 不能靠前端在这里假装做到。
 */
const translateWholeDoc = async () => {
  const viewer = pdfViewerRef.value
  if (!viewer?.exportText) return
  pageTranslateError.value = ''
  const { text, loadedPages } = viewer.exportText()
  const t = String(text || '').trim()
  if (!t) {
    pageTranslateError.value = '还没有任何页加载过文本层，先翻几页再试。'
    return
  }
  if (loadedPages.length < 3) {
    pageTranslateError.value = `目前只有 ${loadedPages.length} 页可用，先多翻几页再试全文翻译。`
    return
  }
  pageTranslateTarget.value = `全文（${loadedPages.length} 页）`
  const paragraphs = splitIntoParagraphs(t)
  if (!paragraphs.length) return
  pageTranslateRows.value = paragraphs.map((source) => ({ source, target: '' }))
  pageTranslateBusy.value = true
  pageTranslateProgress.value = 0
  try {
    const results = await pdfTranslate.translateParagraphs(paragraphs, (partial, done) => {
      pageTranslateRows.value = paragraphs.map((source, i) => ({
        source,
        target: String(partial[i] || ''),
      }))
      pageTranslateProgress.value = done / paragraphs.length
    })
    const failed = results.filter((x) => !x).length
    if (failed) pageTranslateError.value = `${failed} 段没有翻译成功，可以重试。`
    logger.info('[ReaderPane] whole doc translated', { pages: loadedPages.length, paragraphs: paragraphs.length, failed })
  } catch (error) {
    pageTranslateError.value = String(error?.message || error)
  } finally {
    pageTranslateBusy.value = false
  }
}

/** 当前页（供页码联动，也决定「本页译文」翻哪一页）。 */
const pdfCurrentPage = ref(1)

// ───────────────────────── 本页译文（整页翻译）─────────────────────────
// 与划词翻译的区别：这是「整页读完想对照一遍」的场景，结果要能留在侧栏里慢慢看，
// 而不是划一下、看了一眼就消失。
const pageTranslateRows = ref([])
const pageTranslateBusy = ref(false)
const pageTranslateProgress = ref(0)
const pageTranslateError = ref('')
const pageTranslateTarget = ref(1)
/** 'page' | 'book'：译文范围。 */
const xlateScope = ref('page')

const translateCurrentPage = async () => {
  if (xlateScope.value === 'book') {
    await translateWholeDoc()
    return
  }
  const viewer = pdfViewerRef.value
  if (!viewer?.pageTextAt) return
  pageTranslateError.value = ''
  const page = Number(pdfCurrentPage.value) || 1
  pageTranslateTarget.value = page
  const text = String(viewer.pageTextAt(page) || '').trim()
  if (!text) {
    pageTranslateError.value = '这一页还没有加载文本层，先滚动到这一页再试。'
    return
  }
  const paragraphs = splitIntoParagraphs(text)
  if (!paragraphs.length) {
    pageTranslateError.value = '这一页没有可翻译的文字（可能是扫描件或纯图片页）。'
    return
  }
  pageTranslateRows.value = paragraphs.map((source) => ({ source, target: '' }))
  pageTranslateBusy.value = true
  pageTranslateProgress.value = 0
  try {
    const results = await pdfTranslate.translateParagraphs(paragraphs, (partial, done) => {
      pageTranslateRows.value = paragraphs.map((source, i) => ({
        source,
        target: String(partial[i] || ''),
      }))
      pageTranslateProgress.value = done / paragraphs.length
    })
    pageTranslateRows.value = paragraphs.map((source, i) => ({
      source,
      target: String(results[i] || ''),
    }))
    const failed = results.filter((t) => !t).length
    if (failed) pageTranslateError.value = `${failed} 段没有翻译成功，可以点「重新翻译」重试。`
    logger.info('[ReaderPane] page translated', { page, paragraphs: paragraphs.length, failed })
  } catch (error) {
    pageTranslateError.value = String(error?.message || error)
    logger.warn('[ReaderPane] page translate failed', { error: pageTranslateError.value })
  } finally {
    pageTranslateBusy.value = false
  }
}

let pdfSelectionSyncTimer = null
const schedulePdfSelectionSync = () => {
  if (pdfSelectionSyncTimer) clearTimeout(pdfSelectionSyncTimer)
  pdfSelectionSyncTimer = setTimeout(() => {
    pdfSelectionSyncTimer = null
    syncPdfSelectionToolbar()
  }, 80)
}

const onPdfSelectionEscape = (ev) => {
  if (ev.key !== 'Escape') return
  if (!pdfSelectionToolbar.value) return
  ev.preventDefault()
  ev.stopPropagation()
  clearPdfSelection()
}

onMounted(() => {
  // 候选区在 window 上：阅读器的文本层是 transparent span，事件目标就是这些 span，
  // 挂在容器上也能收到；但键盘选择不产生 mouseup，所以 selectionchange 也要听。
  window.addEventListener('mouseup', schedulePdfSelectionSync)
  window.addEventListener('keyup', schedulePdfSelectionSync)
  document.addEventListener('selectionchange', schedulePdfSelectionSync)
  window.addEventListener('keydown', onPdfSelectionEscape, true)
})

onBeforeUnmount(() => {
  window.removeEventListener('mouseup', schedulePdfSelectionSync)
  window.removeEventListener('keyup', schedulePdfSelectionSync)
  document.removeEventListener('selectionchange', schedulePdfSelectionSync)
  window.removeEventListener('keydown', onPdfSelectionEscape, true)
  if (pdfSelectionSyncTimer) clearTimeout(pdfSelectionSyncTimer)
})


watch(
  () => [props.path, props.target?.reader],
  ([nextPath, reader]) => {
    pdfHighlights.value = []
    outlineItems.value = []
    pdfAside.value = ''
    if (reader === 'pdf' && nextPath) {
      // 稍等阅读器打开文档，避免与它自己的加载撞在一起。
      setTimeout(() => {
        void refreshPdfHighlights()
        // 批注存在时默认打开批注页，让用户看到「这本书有笔记」。
        if (pdfHighlights.value.length) pdfAside.value = 'notes'
      }, 1200)
    }
  },
  { immediate: true }
)

const openExternally = async () => {
  try {
    await openPath(props.path)
  } catch (error) {
    logger.warn('[ReaderPane] openPath failed', {
      path: props.path,
      error: String(error?.message || error),
    })
  }
}
</script>

<style scoped>
.gdr-pdf {
  position: absolute;
  inset: 0;
}

/* 有侧栏时正文整体右移，避免被面板盖住。 */
.gdr-pdf--with-aside {
  left: 260px;
}

.gdr-pdf-aside {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  z-index: 9;
  display: flex;
  flex-direction: column;
  min-width: 200px;
  max-width: 45%;
  border-right: 1px solid var(--guaxi-border, #e1e4e8);
  background: var(--guaxi-panel-bg, #fafbfc);
}

.gdr-pdf-aside__head {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 0 6px 0 8px;
  border-bottom: 1px solid var(--guaxi-border, #e1e4e8);
  min-height: 34px;
}

.gdr-pdf-aside__tabs {
  display: flex;
  gap: 2px;
}

.gdr-pdf-aside__tab {
  padding: 4px 10px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font-size: 12px;
  cursor: pointer;
  opacity: 0.7;
}

.gdr-pdf-aside__tab:hover {
  opacity: 1;
  background: var(--guaxi-hover-bg, rgba(15, 23, 42, 0.06));
}

.gdr-pdf-aside__tab.is-on {
  opacity: 1;
  font-weight: 600;
  background: rgba(47, 111, 223, 0.12);
  color: #2f6fdf;
}

.gdr-pdf-aside__body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 6px;
}

.gdr-pdf-aside__empty {
  padding: 12px 10px;
  font-size: 12px;
  line-height: 1.6;
  opacity: 0.6;
}

.gdr-pdf-aside__item {
  display: block;
  width: 100%;
  padding: 5px 8px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: inherit;
  font-size: 12px;
  text-align: left;
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.gdr-pdf-aside__item:hover {
  background: var(--guaxi-hover-bg, rgba(15, 23, 42, 0.06));
}

.gdr-pdf-aside__item.is-active {
  background: rgba(47, 111, 223, 0.12);
  color: #2f6fdf;
}

.gdr-pdf-aside__item:focus-visible,
.gdr-pdf-aside__tab:focus-visible,
.gdr-pdf-aside__note-main:focus-visible {
  outline: 2px solid #2f6fdf;
  outline-offset: 1px;
}

.gdr-pdf-aside__note {
  display: flex;
  align-items: stretch;
  gap: 2px;
  border-radius: 6px;
  margin-bottom: 2px;
}

.gdr-pdf-aside__note:hover {
  background: var(--guaxi-hover-bg, rgba(15, 23, 42, 0.06));
}

.gdr-pdf-aside__note-main {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 7px 8px;
  border: 0;
  background: transparent;
  color: inherit;
  text-align: left;
  cursor: pointer;
}

.gdr-pdf-aside__note-swatch {
  flex: 0 0 auto;
  width: 3px;
  align-self: stretch;
  min-height: 18px;
  border-radius: 2px;
}

.gdr-pdf-aside__note-text {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.gdr-pdf-aside__note-note {
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
}

.gdr-pdf-aside__note-sub {
  font-size: 11px;
  opacity: 0.55;
}

.gdr-pdf-aside__note-del {
  flex: 0 0 auto;
  width: 24px;
  border: 0;
  background: transparent;
  color: inherit;
  opacity: 0.45;
  cursor: pointer;
  border-radius: 6px;
}

.gdr-pdf-aside__note-del:hover {
  opacity: 1;
  color: #e06c75;
}

/* 侧栏开关：浮在正文左上角，不占布局高度。 */
.gdr-pdf-switch {
  position: absolute;
  left: 8px;
  top: 8px;
  z-index: 9;
  display: flex;
  gap: 2px;
  padding: 2px;
  border-radius: 8px;
  background: var(--guaxi-panel-bg, #fafbfc);
  border: 1px solid var(--guaxi-border, #e1e4e8);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
}

.gdr-pdf-switch__badge {
  position: absolute;
  top: -3px;
  right: -3px;
  min-width: 14px;
  height: 14px;
  padding: 0 3px;
  border-radius: 7px;
  background: #2f6fdf;
  color: #fff;
  font-size: 9px;
  line-height: 14px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}

/* 划词高亮条：浮在选区附近，深色玻璃底 + 高对比按钮。
   translateX(-50%) 与 placePdfSelectionToolbar 里「以 left 为中心」的算法配套，
   宽度因此可以随内容伸缩而不用在 JS 里再量一次。 */
.gdr-pdf-hl-bar {
  position: absolute;
  z-index: 20;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 10px;
  background: rgba(24, 26, 30, 0.92);
  border: 1px solid rgba(255, 255, 255, 0.14);
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.35);
  color: rgba(255, 255, 255, 0.9);
  font-size: 12px;
  white-space: nowrap;
  user-select: none;
}
.gdr-pdf-hl-bar__hint {
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  opacity: 0.72;
}
.gdr-pdf-hl-bar__swatches { display: inline-flex; gap: 5px; }
.gdr-pdf-hl-bar__swatch {
  width: 18px;
  height: 18px;
  padding: 0;
  border-radius: 50%;
  border: 1px solid rgba(0, 0, 0, 0.28);
  cursor: pointer;
  transition: transform 0.12s, box-shadow 0.12s;
}
.gdr-pdf-hl-bar__swatch:hover { transform: scale(1.14); }
.gdr-pdf-hl-bar__swatch.is-on {
  box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.85);
}
.gdr-pdf-hl-bar__cancel {
  margin-left: 2px;
  padding: 2px 7px;
  border-radius: 6px;
  border: 1px solid rgba(255, 255, 255, 0.18);
  background: transparent;
  color: inherit;
  font-size: 11px;
  cursor: pointer;
}
.gdr-pdf-hl-bar__cancel:hover { background: rgba(255, 255, 255, 0.1); }

/* 译 / 读：和颜色圆点并列，但用文字按钮 —— 这两个是「动作」，不是「取值」。 */
.gdr-pdf-hl-bar__action {
  padding: 2px 8px;
  border-radius: 6px;
  border: 1px solid rgba(255, 255, 255, 0.2);
  background: rgba(255, 255, 255, 0.06);
  color: inherit;
  font-size: 11px;
  cursor: pointer;
  transition: background 0.12s;
}
.gdr-pdf-hl-bar__action:hover { background: rgba(255, 255, 255, 0.16); }
/* 译文侧栏：原文/译文逐段对照。原文用稍弱的颜色，译文是正文色。 */
.gdr-xlate {
  display: flex;
  flex-direction: column;
  gap: 10px;
  height: 100%;
  min-height: 0;
  overflow: auto;
  padding: 2px;
}
.gdr-xlate__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  position: sticky;
  top: 0;
  background: inherit;
  padding: 2px 0;
}
.gdr-xlate__title { font-size: 12px; font-weight: 600; }
.gdr-xlate__btn {
  padding: 4px 10px;
  border-radius: 7px;
  border: 1px solid color-mix(in srgb, var(--n-color-primary, #61afef) 45%, transparent);
  background: transparent;
  color: var(--n-color-primary, #61afef);
  font-size: 11.5px;
  cursor: pointer;
}
.gdr-xlate__btn:disabled { opacity: 0.5; cursor: not-allowed; }
.gdr-xlate__progress {
  height: 2px;
  border-radius: 1px;
  background: color-mix(in srgb, currentColor 12%, transparent);
  overflow: hidden;
}
.gdr-xlate__bar {
  display: block;
  height: 100%;
  background: var(--n-color-primary, #61afef);
  transition: width 0.2s;
}
.gdr-xlate__scope {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-wrap: wrap;
}
.gdr-xlate__scope-btn {
  padding: 2px 9px;
  border-radius: 6px;
  border: 1px solid color-mix(in srgb, currentColor 18%, transparent);
  background: transparent;
  color: inherit;
  font-size: 11px;
  cursor: pointer;
  opacity: 0.72;
}
.gdr-xlate__scope-btn.is-on {
  opacity: 1;
  border-color: color-mix(in srgb, var(--n-color-primary, #61afef) 55%, transparent);
  color: var(--n-color-primary, #61afef);
}
.gdr-xlate__scope-note {
  font-size: 10.5px;
  opacity: 0.5;
}

.gdr-xlate__error {
  font-size: 11.5px;
  color: #cf222e;
  line-height: 1.6;
}
.gdr-xlate__row {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 8px 9px;
  border-radius: 8px;
  background: color-mix(in srgb, currentColor 4%, transparent);
}
.gdr-xlate__src {
  margin: 0;
  font-size: 11.5px;
  line-height: 1.65;
  opacity: 0.62;
}
.gdr-xlate__dst {
  margin: 0;
  font-size: 12.5px;
  line-height: 1.75;
}
.gdr-xlate__pending { margin: 0; opacity: 0.4; }

.gdr-pdf-hl-bar__sep {
  width: 1px;
  height: 14px;
  background: rgba(255, 255, 255, 0.18);
}

.guaxi-doc-reader__fallback {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
}
</style>
