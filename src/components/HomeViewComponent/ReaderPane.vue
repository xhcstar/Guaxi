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
                <span class="gdr-pdf-aside__note-note">{{ hl.text || hl.note || '（无笔记）' }}</span>
                <span class="gdr-pdf-aside__note-sub">
                  第 {{ hl.page }} 页<span v-if="hl.text && hl.note"> · {{ hl.note }}</span>
                </span>
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
        @undo-highlight="undoPdfHighlight"
        @redo-highlight="redoPdfHighlight"
        @go-reading-back="goReadingBack"
        @go-reading-forward="goReadingForward"
      >
        <!--
          侧栏开关：交给 PDF 阅读器的工具栏托管。

          原先这里是 absolute 浮在正文左上角的独立浮层，而工具栏固定占着顶部
          38px —— 两者空间必然重叠，实测盖住分页簇 60px，文件名与状态被挤成不可见。
          放进工具栏插槽后它参与正常排布，不再需要靠调 top 值躲避。

          仍不塞进 GuaxiPdfViewer 内部：它是通用组件，还被 LaTeX 预览、资料库阅读器
          复用；「大纲 / 批注」是独立阅读器的宿主专属入口，其它调用方用不上。
        -->
        <template #toolbar-leading>
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
      </GuaxiPdfViewer>

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
          所以这两个动作排在高亮之前。图标按钮都带 title 提示。
        -->
        <button
          class="gdr-pdf-hl-bar__icon"
          title="翻译选中的文字"
          aria-label="翻译"
          @click="openPdfTranslate"
        >
          <Icon :icon="translateIcon" width="15" height="15" />
        </button>
        <button
          class="gdr-pdf-hl-bar__icon"
          :title="pdfReadAloudPlaying ? '停止朗读' : '朗读选中的文字'"
          aria-label="朗读"
          @click="speakPdfSelection"
        >
          <Icon :icon="volumeIcon" width="15" height="15" />
        </button>
        <span class="gdr-pdf-hl-bar__sep" />
        <!-- 高亮图标直接用当前颜色落笔；调色盘图标才展开颜色。 -->
        <button
          class="gdr-pdf-hl-bar__icon"
          title="查询选中的文字"
          aria-label="查询"
          @click="queryPdfSelection"
        >
          <Icon :icon="searchQueryIcon" width="15" height="15" />
        </button>
        <button
          class="gdr-pdf-hl-bar__icon"
          title="用 AI 翻译选中的文字"
          aria-label="用 AI 翻译"
          @click="openPdfAssistantTranslate"
        >
          <Icon :icon="wandIcon" width="15" height="15" />
        </button>
        <button
          class="gdr-pdf-hl-bar__icon"
          title="把选中的文字发给 AI 提问"
          aria-label="问 AI"
          @click="askPdfSelectionWithAssistant"
        >
          <Icon :icon="botIcon" width="15" height="15" />
        </button>
        <span class="gdr-pdf-hl-bar__sep" />
        <!-- 高亮图标直接用当前颜色落笔；调色盘图标才展开颜色。 -->
        <button
          class="gdr-pdf-hl-bar__icon"
          title="高亮（当前颜色）"
          aria-label="高亮"
          @click="addPdfHighlightFromSelection(PDF_HL_PRESETS[pdfHighlightPresetIdx])"
        >
          <Icon :icon="highlighterIcon" width="15" height="15" />
        </button>
        <button
          class="gdr-pdf-hl-bar__icon"
          :class="{ 'is-on': pdfPaletteOpen }"
          title="选择高亮颜色"
          aria-label="高亮颜色"
          :aria-expanded="pdfPaletteOpen"
          @click.stop="pdfPaletteOpen = !pdfPaletteOpen"
        >
          <Icon :icon="paletteIcon" width="15" height="15" />
        </button>
        <span v-if="pdfPaletteOpen" class="gdr-pdf-hl-bar__swatches">
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
        <!--
          原先这里还有一个 ✕「取消选择」按钮。现在遵循同一套交互：点空白处即收起
          （见 onPdfGlobalMouseDown），Esc 也能关 —— 揪着鼠标去点那颗小叉，
          比直接点别处麻烦，而它就占着一颗按钮的位置。
        -->
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
import { computed, defineAsyncComponent, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Icon } from '@iconify/vue'
import closeIcon from '@iconify-icons/lucide/x'
import outlineIcon from '@iconify-icons/lucide/list-tree'
import noteIcon from '@iconify-icons/lucide/edit-3'
import translateIcon from '@iconify-icons/lucide/languages'
import folderIcon from '@iconify-icons/lucide/folder'
import convertIcon from '@iconify-icons/lucide/file-output'
import codeIcon from '@iconify-icons/lucide/code'
import volumeIcon from '@iconify-icons/lucide/volume-2'
import highlighterIcon from '@iconify-icons/lucide/highlighter'
import paletteIcon from '@iconify-icons/lucide/palette'
import searchQueryIcon from '@iconify-icons/lucide/search'
import botIcon from '@iconify-icons/lucide/bot'
import wandIcon from '@iconify-icons/lucide/wand-2'
import { openPath, openUrl } from '@tauri-apps/plugin-opener'
import { createLogger } from '../../utils/logger'
import { usePageTranslate, splitIntoParagraphs } from '../../composables/usePageTranslate'
import { useSelectionReadAloud } from '../../composables/useSelectionReadAloud'
import { useRightSiderStore } from '../../stores/rightSiderStore'

// 各阅读器按需加载：打开一本 MOBI 不该把 pdfium 阅读器、Office 阅读器一起下载并解析。
// 阅读器窗口是独立 webview，首帧速度几乎完全取决于入口加载了多少模块（见 reader-main.js）。
const GuaxiPdfViewer = defineAsyncComponent(() => import('../PdfViewer/GuaxiPdfViewer.vue'))
const DocImageReader = defineAsyncComponent(() => import('../ReaderComponent/readers/DocImageReader.vue'))
const DocEbookReader = defineAsyncComponent(() => import('../ReaderComponent/readers/DocEbookReader.vue'))
const DocHtmlReader = defineAsyncComponent(() => import('../ReaderComponent/readers/DocHtmlReader.vue'))
const DocMediaReader = defineAsyncComponent(() => import('../ReaderComponent/readers/DocMediaReader.vue'))
const DocOfficeReader = defineAsyncComponent(() => import('../ReaderComponent/readers/DocOfficeReader.vue'))
// 翻译面板是阅读器里唯一依赖 naive-ui 的组件：只在真正翻译时加载。
const GuaxiSelectionTranslatePanel = defineAsyncComponent(
  () => import('./GuaxiSelectionTranslatePanel.vue')
)

const logger = createLogger('ReaderPane')
// 「问 AI / 用 AI 翻译」把选中文字送进全局右侧助手（应用级 store，打开的是同一个对话）。
const rightSiderStore = useRightSiderStore()

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
// 撤销/重做与阅读位置：与 GuaxiConvertPDFReader 里那套工具栏对齐，
// 独立阅读器同样能反悔一次高亮、跳回上一处阅读位置。
//
// 注意：下面的栈必须在本 computed 首次求值前就已初始化 —— computed 是惰性的，
// 真正读取发生在渲染时，那时 setup 已跑完，所以这里引用后面的声明是安全的。
const pdfFeatures = computed(() => ({
  toolbar: true,
  search: true,
  pagination: true,
  outline: true,
  annotations: true,
  reverseJump: false,
  export: false,
  rebuild: false,
  undoHighlight: true,
  redoHighlight: true,
  readingHistory: true,
  canUndoHighlight: highlightUndoStack.value.length > 0,
  canRedoHighlight: highlightRedoStack.value.length > 0,
  canGoReadingBack: readingBackStack.value.length > 0,
  canGoReadingForward: readingForwardStack.value.length > 0,
}))

// ───────────────────────── 高亮撤销/重做 ─────────────────────────
// 与 GuaxiConvertPDFReader 同一套语义：一次「加」或「删」都进栈；撤销 = 反向执行。
// 重建后的高亮会拿到新的 annotId（后端重新分配），因此栈里的 id 在执行后要替换。
const highlightUndoStack = ref([])
const highlightRedoStack = ref([])

const pushHighlightHistory = (entry) => {
  highlightUndoStack.value.push(entry)
  if (highlightUndoStack.value.length > 200) highlightUndoStack.value.shift()
  // 新的操作作废重做分支，否则「重做」会跳回到已不存在的那条历史。
  highlightRedoStack.value = []
}

const undoPdfHighlight = async () => {
  const entry = highlightUndoStack.value.pop()
  if (!entry) return
  const viewer = pdfViewerRef.value
  if (!viewer) {
    highlightUndoStack.value.push(entry)
    return
  }
  try {
    if (entry.type === 'add') {
      const ok = await viewer.deleteHighlight?.(entry.page, entry.annotId)
      if (ok) {
        highlightRedoStack.value.push(entry)
        await refreshPdfHighlights()
      } else highlightUndoStack.value.push(entry)
    } else if (entry.type === 'delete') {
      const r = await viewer.reAddHighlight?.({
        page: entry.page,
        rects: entry.rects,
        color: entry.color,
        note: entry.note,
        text: entry.text,
      })
      if (r) {
        highlightRedoStack.value.push({ ...entry, annotId: r.annotId })
        await refreshPdfHighlights()
      } else highlightUndoStack.value.push(entry)
    }
  } catch (error) {
    highlightUndoStack.value.push(entry)
    logger.warn('[ReaderPane] undo highlight failed', {
      error: String(error?.message || error),
    })
  }
}

const redoPdfHighlight = async () => {
  const entry = highlightRedoStack.value.pop()
  if (!entry) return
  const viewer = pdfViewerRef.value
  if (!viewer) {
    highlightRedoStack.value.push(entry)
    return
  }
  try {
    if (entry.type === 'add') {
      const r = await viewer.reAddHighlight?.({
        page: entry.page,
        rects: entry.rects,
        color: entry.color,
        note: entry.note,
        text: entry.text,
      })
      if (r) {
        highlightUndoStack.value.push({ ...entry, annotId: r.annotId })
        await refreshPdfHighlights()
      } else highlightRedoStack.value.push(entry)
    } else if (entry.type === 'delete') {
      const ok = await viewer.deleteHighlight?.(entry.page, entry.annotId)
      if (ok) {
        highlightUndoStack.value.push(entry)
        await refreshPdfHighlights()
      } else highlightRedoStack.value.push(entry)
    }
  } catch (error) {
    highlightRedoStack.value.push(entry)
    logger.warn('[ReaderPane] redo highlight failed', {
      error: String(error?.message || error),
    })
  }
}

// ───────────────────────── 阅读位置前后 ─────────────────────────
// 只记录「跳转式」的页码变化（大纲、批注、页码输入）；连续滚动的逐页变化不记，
// 否则按一次「上一处」只会退回紧挨着的上一页，等于没用。
const readingBackStack = ref([])
const readingForwardStack = ref([])
let suppressReadingPush = false
let lastPageForHistory = 1

const pushReadingPosition = (page) => {
  if (suppressReadingPush) return
  const p = Number(page) || 0
  if (p < 1) return
  if (readingBackStack.value[readingBackStack.value.length - 1] === p) return
  readingBackStack.value.push(p)
  if (readingBackStack.value.length > 100) readingBackStack.value.shift()
  readingForwardStack.value = []
}

const gotoReadingPage = (page) => {
  const p = Number(page) || 1
  suppressReadingPush = true
  pdfViewerRef.value?.goToPage?.(p)
  nextTick(() => {
    suppressReadingPush = false
  })
}

const goReadingBack = () => {
  const target = readingBackStack.value.pop()
  if (!target) return
  readingForwardStack.value.push(pdfCurrentPage.value)
  gotoReadingPage(target)
}

const goReadingForward = () => {
  const target = readingForwardStack.value.pop()
  if (!target) return
  readingBackStack.value.push(pdfCurrentPage.value)
  gotoReadingPage(target)
}

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
  const prev = lastPageForHistory || 1
  // 只有「大跳」才记入阅读位置：连续滚动会逐页触发，那样按一次「上一处」
  // 只会退回紧挨着的上一页，等于没有历史。与 GuaxiConvertPDFReader 的判据一致。
  if (!suppressReadingPush && num > 0 && Math.abs(num - prev) > 1) {
    pushReadingPosition(prev)
  }
  lastPageForHistory = num || 1
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
      // 后端 HighlightInfo 是 snake_case；以前只读 annotId，导致列表项 key 与删除都用 undefined。
      annotId: String(h.annot_id ?? h.annotId ?? ''),
      page: h.page,
      text: String(h.text || ''),
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
  // 删除会直接改写 .pdf 文件；确认一次，再记入撤销栈（工具栏的撤销按钮可找回）。
  const confirmed = window.confirm?.(
    `删除这条批注？\n\n${hl.note || '（无笔记）'}\n第 ${hl.page} 页\n\n此操作会修改 PDF 文件本身，删除后可用撤销恢复。`
  )
  if (!confirmed) return
  try {
    // 侧栏列表里只有摘要，没有矩形 —— 而撤销「删除」要靠 rects 才能重建那条高亮。
    // 所以在真正删除之前，先从 viewer 的内存里取一次这条批注的原始矩形与颜色。
    const raw = pdfViewerRef.value?.findHighlightRaw?.(hl.page, hl.annotId)
    const ok = await pdfViewerRef.value?.deleteHighlight?.(hl.page, hl.annotId)
    if (ok) {
      pushHighlightHistory({
        type: 'delete',
        page: hl.page,
        annotId: hl.annotId,
        rects: Array.isArray(raw?.rects) ? raw.rects.map((r) => ({ ...r })) : [],
        color: raw?.color ?? hl.color,
        note: raw?.note ?? hl.note ?? '',
        text: raw?.text ?? hl.text ?? '',
      })
    }
    pdfHighlights.value = pdfHighlights.value.filter((item) => item.annotId !== hl.annotId)
    logger.info('[ReaderPane] pdf highlight deleted', { page: hl.page, ok, canUndo: !!raw })
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
/** 调色盘是否展开：默认收起，高亮图标直接用当前颜色落笔。 */
const pdfPaletteOpen = ref(false)
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
    pdfPaletteOpen.value = false
    return
  }
  pdfSelectionText.value = text
  if (placePdfSelectionToolbar()) pdfSelectionToolbar.value = true
}

const clearPdfSelection = () => {
  window.getSelection?.()?.removeAllRanges()
  pdfSelectionToolbar.value = false
  pdfSelectionText.value = ''
  pdfPaletteOpen.value = false
}

/**
 * 给当前选区加上指定颜色的高亮。
 *
 * 关键点：先把选区矩形算出来再清选区 —— addHighlightFromSelection 内部会
 * removeAllRanges，如果宿主这边先清，它拿到的就是空选区。
 */
const addPdfHighlightFromSelection = async (color) => {
  pdfPaletteOpen.value = false
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
    // 记入撤销栈：加高亮改的是 .pdf 文件本身，没有这一步用户只能重开文件找回。
    pushHighlightHistory({
      type: 'add',
      page: res.page,
      annotId: res.annotId,
      rects: Array.isArray(res.rects) ? res.rects.map((r) => ({ ...r })) : [],
      color: Array.isArray(color) ? [...color] : color,
      note: '',
      text: res.text || '',
    })
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

/** 查询：把选中文字交给系统浏览器搜索。 */
const queryPdfSelection = async () => {
  const text = String(pdfSelectionText.value || '').trim()
  if (!text) return
  const url = `https://www.bing.com/search?q=${encodeURIComponent(text)}`
  clearPdfSelection()
  try {
    await openUrl(url)
  } catch (error) {
    logger.warn('[ReaderPane] open search url failed', { error: String(error?.message || error) })
  }
}

/**
 * 把选中文字送到右侧智能助手。
 *
 * 独立阅读器窗口没有自己的助手侧栏（GuaxiConvertPDFReader 那一套），所以与 DocEbookReader
 * 一样走全局右栏：rightSiderStore 是应用级的，打开的是同一个对话。
 */
const sendPdfSelectionToAssistant = (text, options = {}) => {
  const t = String(text || '').trim()
  if (!t) return false
  try {
    rightSiderStore.setPanel('assistant')
    rightSiderStore.open()
    rightSiderStore.appendAssistantDraft(t, {
      autoSend: options.autoSend !== false,
      disableWebSearch: !!options.disableWebSearch,
    })
    return true
  } catch (error) {
    logger.warn('[ReaderPane] send selection to assistant failed', {
      error: String(error?.message || error),
    })
    return false
  }
}

const askPdfSelectionWithAssistant = () => {
  const text = String(pdfSelectionText.value || '').trim()
  if (!text) return
  sendPdfSelectionToAssistant(text, { autoSend: true })
  clearPdfSelection()
}

/**
 * 「用 AI 翻译」：直接把选中的文字拼成一条翻译指令发给助手。
 *
 * 与划词浮条上的「翻译」（走本地/免费翻译面板）是两条路：那颗求的是快，这颗求的是质量。
 * 目标语言跟随页面译文里选过的那种，读者不必为同一份文档设两遍。
 */
const openPdfAssistantTranslate = () => {
  const text = String(pdfSelectionText.value || '').trim()
  if (!text) return
  const lang = pdfTranslate.toLang?.value || 'zh'
  const label = pdfTranslate.langOptions?.find((x) => x.value === lang)?.label || lang
  const prompt = `请将以下内容翻译成${label}，保持原意并保留术语与格式，只输出译文：\n\n${text}`
  const ok = sendPdfSelectionToAssistant(prompt, { autoSend: true, disableWebSearch: true })
  if (ok) clearPdfSelection()
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

/**
 * 在浮条外按下鼠标 → 立刻收起浮条。
 *
 * 为什么非要在 mousedown 这一步做：点阅读器的灰色画布或页边时，浏览器**不会**清掉
 * 已有的文字选区（只有点在文字上才改选区），紧随其后的 mouseup 会照旧把浮条再浮出来。
 * 用户看到的就是「点了空白处，浮条还在」。这里在按下时先收起，并主动清掉选区，
 * 让后续的 syncPdfSelectionToolbar 读到空选区、不会把它重新显示。
 */
const onPdfGlobalMouseDown = (ev) => {
  if (!pdfSelectionToolbar.value) return
  const target = ev.target
  // 点在浮条自身（要按按钮）或划词译文面板上：保持浮条。
  if (target instanceof Element && target.closest('.gdr-pdf-hl-bar, .pot-selection-translate-panel')) return
  clearPdfSelection()
}

onMounted(() => {
  // 候选区在 window 上：阅读器的文本层是 transparent span，事件目标就是这些 span，
  // 挂在容器上也能收到；但键盘选择不产生 mouseup，所以 selectionchange 也要听。
  window.addEventListener('mouseup', schedulePdfSelectionSync)
  window.addEventListener('keyup', schedulePdfSelectionSync)
  document.addEventListener('selectionchange', schedulePdfSelectionSync)
  window.addEventListener('keydown', onPdfSelectionEscape, true)
  // 捕获阶段：赶在阅读器自己的 mousedown（平移/选词）之前收起，避免同一帧里又被重新浮出。
  window.addEventListener('mousedown', onPdfGlobalMouseDown, true)
})

onBeforeUnmount(() => {
  window.removeEventListener('mouseup', schedulePdfSelectionSync)
  window.removeEventListener('keyup', schedulePdfSelectionSync)
  document.removeEventListener('selectionchange', schedulePdfSelectionSync)
  window.removeEventListener('keydown', onPdfSelectionEscape, true)
  window.removeEventListener('mousedown', onPdfGlobalMouseDown, true)
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
        // 批注列表只预热，不主动展开侧栏：打开文档时更重要的是正文，
        // 侧栏留给读者自己按需要打开（点正文里的高亮仍会自动切到批注页）。
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

/* 侧栏开关：由 GuaxiPdfViewer 的 #toolbar-leading 插槽托管，参与工具栏正常排布。
   形状刻意与旁边的 .gpv__cluster 一致（同为胶囊），三等分的三个按钮看起来是
   「同一件事的三个入口」，而不是外挂上去的控制条。 */
.gdr-pdf-switch {
  display: inline-flex;
  align-items: center;
  gap: 1px;
  padding: 2px;
  border-radius: 9px;
  background: var(--gpv-cluster-bg, rgba(255, 255, 255, 0.06));
  box-shadow: inset 0 0 0 1px var(--gpv-cluster-border, rgba(255, 255, 255, 0.14));
}

/* 插槽内容挂在 GuaxiPdfViewer 内部，随之继承 PDF 的主题变量，
   所以这里刻意用 --gpv-* 而不是阅读器外壳的 --guaxi-*：
   深色 PDF 工具栏上用 --guaxi-text-primary（深墨色）会几乎看不见。 */
.gdr-pdf-switch .guaxi-doc-reader__icon-btn {
  position: relative;
  width: 24px;
  height: 24px;
  border-radius: 7px;
  color: var(--gpv-btn-text, #3b4148);
}

.gdr-pdf-switch .guaxi-doc-reader__icon-btn:hover:not(:disabled) {
  background: var(--gpv-hover-bg, rgba(15, 23, 42, 0.06));
}

.gdr-pdf-switch .guaxi-doc-reader__icon-btn:active:not(:disabled) {
  background: var(--gpv-hover-bg, rgba(15, 23, 42, 0.06));
}

.gdr-pdf-switch__badge {
  position: absolute;
  top: -1px;
  right: -1px;
  min-width: 13px;
  height: 13px;
  padding: 0 3px;
  border-radius: 7px;
  background: #2f6fdf;
  color: #fff;
  font-size: 9px;
  line-height: 13px;
  text-align: center;
  font-variant-numeric: tabular-nums;
  pointer-events: none;
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

/* 图标按钮：与电子书阅读器的划词条同一套交互（title 提示 + 悬停底色）。 */
.gdr-pdf-hl-bar__icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  border-radius: 6px;
  border: 1px solid transparent;
  background: transparent;
  color: inherit;
  cursor: pointer;
  transition: background 0.12s, border-color 0.12s;
}
.gdr-pdf-hl-bar__icon:hover { background: rgba(255, 255, 255, 0.12); }
.gdr-pdf-hl-bar__icon.is-on {
  background: rgba(255, 255, 255, 0.18);
  border-color: rgba(255, 255, 255, 0.25);
}
.gdr-pdf-hl-bar__icon:focus-visible { outline: 2px solid #2f6fdf; outline-offset: 1px; }
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
