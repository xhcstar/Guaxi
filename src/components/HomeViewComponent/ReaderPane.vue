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
  <div v-if="target" class="guaxi-doc-reader" :data-reader="target.reader" @keydown.stop>
    <template v-if="target.reader === 'pdf'">
      <GuaxiPdfViewer
        class="gdr-pdf"
        :path="path"
        :theme="theme"
        :title-label="fileName"
        :initial-zoom="1.2"
        fit-mode="width"
        :features="pdfFeatures"
      />
    </template>

    <DocImageReader
      v-else-if="target.reader === 'image'"
      :path="path"
      :name="fileName"
      :mime="target.mime"
      :variant="'single'"
    />

    <DocDocxReader
      v-else-if="target.reader === 'word' && target.kind === 'docx'"
      :path="path"
      :name="fileName"
      :kind="target.kind"
    />

    <DocEbookReader
      v-else-if="target.reader === 'ebook'"
      :path="path"
      :name="fileName"
      :kind="target.kind"
    />

    <DocSpreadsheetReader
      v-else-if="target.reader === 'spreadsheet'"
      :path="path"
      :name="fileName"
      :kind="target.kind"
    />

    <!-- 幻灯片：pptx 走原生版式预览（@vue-office/pptx，离线、按需加载）；
         老格式 .ppt / .odp 没有对应渲染内核，退回转换后的只读 Markdown 大纲。 -->
    <DocPptxReader
      v-else-if="target.reader === 'slides' && target.kind === 'pptx'"
      :path="path"
      :name="fileName"
      :kind="target.kind"
    />
    <DocMarkdownReader
      v-else-if="target.reader === 'slides'"
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

    <!-- doc / rtf / odt：anydoc 转 Markdown 的只读视图。 -->
    <DocMarkdownReader
      v-else-if="target.reader === 'word'"
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
        <button class="guaxi-doc-reader__btn" @click="openExternally">在文件浏览器中打开</button>
        <button class="guaxi-doc-reader__btn" @click="$emit('convert-to-markdown', path)">转换为 Markdown</button>
        <button class="guaxi-doc-reader__btn" @click="$emit('force-editor', path)">用编辑器打开</button>
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
import { computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { openPath } from '@tauri-apps/plugin-opener'
import { createLogger } from '../../utils/logger'
import GuaxiPdfViewer from '../PdfViewer/GuaxiPdfViewer.vue'
import DocImageReader from '../ReaderComponent/readers/DocImageReader.vue'
import DocDocxReader from '../ReaderComponent/readers/DocDocxReader.vue'
import DocEbookReader from '../ReaderComponent/readers/DocEbookReader.vue'
import DocSpreadsheetReader from '../ReaderComponent/readers/DocSpreadsheetReader.vue'
import DocMarkdownReader from '../ReaderComponent/readers/DocMarkdownReader.vue'
import DocHtmlReader from '../ReaderComponent/readers/DocHtmlReader.vue'
import DocMediaReader from '../ReaderComponent/readers/DocMediaReader.vue'
import DocPptxReader from '../ReaderComponent/readers/DocPptxReader.vue'

const logger = createLogger('ReaderPane')

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

.guaxi-doc-reader__fallback {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
}
</style>
