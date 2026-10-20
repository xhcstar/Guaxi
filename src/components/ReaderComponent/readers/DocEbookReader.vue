<template>
  <div class="guaxi-doc-reader" data-reader="ebook">
    <div class="guaxi-doc-reader__bar">
      <span class="guaxi-doc-reader__title" :title="path">{{ title || name }}</span>
      <span class="guaxi-doc-reader__badge">{{ kindLabel }}</span>
      <span v-if="sections.length" class="guaxi-doc-reader__meta">
        第 {{ activeIndex + 1 }} / {{ sections.length }} 节
      </span>
      <span class="guaxi-doc-reader__spacer" />
      <button class="guaxi-doc-reader__btn" title="上一节 (←)" :disabled="activeIndex <= 0" @click="goSection(activeIndex - 1)">‹ 上一节</button>
      <button class="guaxi-doc-reader__btn" title="下一节 (→)" :disabled="activeIndex >= sections.length - 1" @click="goSection(activeIndex + 1)">下一节 ›</button>
      <button class="guaxi-doc-reader__btn" :class="{ 'is-on': tocVisible }" title="目录" @click="tocVisible = !tocVisible">
        {{ tocVisible ? '隐藏目录' : '目录' }}
      </button>
      <button class="guaxi-doc-reader__btn" title="缩小" :disabled="loading || !!errorText" @click="zoomBy(1 / 1.12)">−</button>
      <span class="guaxi-doc-reader__meta" style="min-width: 44px; text-align: center">{{ Math.round(zoom * 100) }}%</span>
      <button class="guaxi-doc-reader__btn" title="放大" :disabled="loading || !!errorText" @click="zoomBy(1.12)">＋</button>
    </div>

    <div v-if="loading" class="guaxi-doc-reader__body">
      <div class="guaxi-doc-reader__center">
        <div>正在解析电子书…</div>
        <div class="guaxi-doc-reader__meta">首次打开大文件可能需要几秒</div>
      </div>
    </div>
    <div v-else-if="errorText" class="guaxi-doc-reader__body">
      <div class="guaxi-doc-reader__center">
        <div class="guaxi-doc-reader__error">{{ errorText }}</div>
      </div>
    </div>
    <div v-else class="guaxi-doc-reader__body gdr-ebook">
      <aside v-if="tocVisible" class="gdr-ebook__toc" :style="{ width: `${tocWidth}px` }">
        <button
          v-for="(item, idx) in toc"
          :key="`${idx}-${item.href}`"
          class="gdr-ebook__toc-item"
          :class="{ 'is-active': tocActiveIdx === idx }"
          :style="{ paddingLeft: `${8 + item.depth * 12}px` }"
          :title="item.label"
          @click="jumpToHref(item.href)"
        >
          {{ item.label || '(未命名章节)' }}
        </button>
        <div v-if="!toc.length" class="guaxi-doc-reader__meta" style="padding: 8px">
          本书没有可用的目录信息
        </div>
      </aside>
      <div
        v-if="tocVisible && toc.length"
        class="gdr-ebook__splitter"
        :class="{ 'is-active': resizing }"
        role="separator"
        aria-orientation="vertical"
        aria-label="拖动调整目录宽度"
        @mousedown.prevent="startTocResize"
      />
      <div class="gdr-ebook__main" :class="{ 'is-resizing': resizing }">
        <!--
          用 blob URL + src 而不是 srcdoc：
          srcdoc 文档在 sandbox 缺 allow-scripts 时，浏览器会为**每一段**脚本打印
          "Blocked script execution in 'about:srcdoc'"。脚本已经被 stripScripts 剥掉，
          但个别章节的 <script> 藏在注释/条件注释里仍可能漏网，换成 blob URL 后
          这类噪音不再出现（文档来源不再是 about:srcdoc）。
        -->
        <iframe
          ref="frameRef"
          class="gdr-ebook__frame"
          sandbox="allow-same-origin"
          referrerpolicy="no-referrer"
          @load="onFrameLoad"
        />
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../../../utils/logger'
import './readerPane.css'

const logger = createLogger('DocEbookReader')

const props = defineProps({
  path: { type: String, required: true },
  name: { type: String, default: '' },
  kind: { type: String, default: 'epub' },
})

const frameRef = ref(null)
const loading = ref(true)
const errorText = ref('')
const title = ref('')
const sections = ref([])
const toc = ref([])
const activeIndex = ref(0)
const tocVisible = ref(true)
// 拖拽目录分隔线期间为 true：给正文挂遮罩，避免事件被 iframe 吃掉。
const resizing = ref(false)
const tocWidth = ref(250)
const zoom = ref(1)
const frameDoc = ref('')

const kindLabel = computed(() => String(props.kind || '').toUpperCase())

const tocActiveIdx = computed(() => {
  // 目录项与 section 通过 href 关联；找不到就退回按索引映射。
  const current = sections.value[activeIndex.value]
  if (!current) return -1
  const idx = toc.value.findIndex((item) => item.href && current.href && item.href.endsWith(current.href))
  if (idx >= 0) return idx
  return Math.min(activeIndex.value, Math.max(0, toc.value.length - 1))
})

let loadToken = 0

const zoomBy = (factor) => {
  zoom.value = Math.min(3, Math.max(0.5, +(zoom.value * factor).toFixed(3)))
}

/**
 * 去掉章节 HTML 里的脚本与内联事件，避免浏览器控制台刷屏。
 *
 * 为什么必须主动剥离，而不是靠 sandbox 拦：
 * sandbox 不给 allow-scripts 时，浏览器照样会为每一段内联脚本打印
 * "Blocked script execution in 'about:srcdoc' because the document's frame is sandboxed"。
 * 一本排版工具导出的 epub 常常有几十个 <script>，打开一本书就是几十条红色报错，
 * 看起来像软件坏了。先剔除再渲染，沙箱仍然保留（作为纵深防御），但不再有噪音。
 */
const stripScripts = (html) => String(html || '')
  // 整段脚本块（style 必须保留：章节版式全靠它）
  .replace(/<script[^>]*>[^]*?<\/script>/gi, '')
  .replace(/<script[^>]*\/>/gi, '')
  // 内联事件处理器：onload / onclick / onerror …
  .replace(/\son[a-z]+\s*=\s*"[^"]*"/gi, '')
  .replace(/\son[a-z]+\s*=\s*'[^']*'/gi, '')
  // javascript: 伪协议链接
  .replace(/href\s*=\s*(["'])\s*javascript:[^"']*\1/gi, 'href="#"')

/**
 * 用 sandbox="allow-same-origin" 的 iframe + srcdoc 承载章节 HTML。
 *
 * 为什么不用 allow-scripts：电子书章节不需要脚本，禁掉之后即便源文件里混进了脚本
 * 也无法执行；但保留 allow-same-origin 是为了让内联 CSS 与 data URI 图片正常工作
 * （完全 sandbox 时部分 WebView2 版本会拦掉内联样式里的字体）。
 */
const buildFrameDoc = () => {
  const section = sections.value[activeIndex.value]
  if (!section) {
    frameDoc.value = '<!doctype html><html><body></body></html>'
    return
  }
  // ebook-rs 的 processed_html 通常是完整的 html 文档；若只是片段则包一层。
  // 先剥掉脚本：沙箱会拦执行，但拦的过程本身会往控制台刷大量 Blocked script execution。
  const raw = stripScripts(section.html)
  const isFullDoc = /<html[\s>]/i.test(raw)
  const themeBackground = 'transparent'
  const baseCss = `
    html, body { margin: 0; padding: 0; background: ${themeBackground}; }
    body {
      font-family: "Noto Serif SC", "Songti SC", "Microsoft YaHei", system-ui, sans-serif;
      line-height: 1.8;
      font-size: ${16 * zoom.value}px;
      color: var(--guaxi-text-primary, #1f2328);
      padding: 24px 32px 80px;
      max-width: 860px;
      margin: 0 auto;
      word-wrap: break-word;
    }
    img, svg, video { max-width: 100%; height: auto; }
    table { border-collapse: collapse; max-width: 100%; }
    td, th { border: 1px solid rgba(128,128,128,.35); padding: 4px 8px; }
    pre { white-space: pre-wrap; word-break: break-word; }
    a { color: #2f6fdf; }
  `
  if (isFullDoc) {
    frameDoc.value = raw.replace(
      /<\/head>/i,
      `<style>${baseCss}</style></head>`
    )
    if (frameDoc.value === raw) {
      // 没有 </head>：插到 <html> 后或直接前置。
      frameDoc.value = raw.replace(/<html([^>]*)>/i, `<html$1><head><style>${baseCss}</style></head>`)
      if (!/<head/i.test(frameDoc.value)) {
        frameDoc.value = `<style>${baseCss}</style>${raw}`
      }
    }
    return
  }
  frameDoc.value = `<!doctype html><html><head><meta charset="utf-8"><style>${baseCss}</style></head><body>${raw}</body></html>`
}

const goSection = (index) => {
  const next = Math.max(0, Math.min(sections.value.length - 1, index))
  if (next === activeIndex.value) return
  activeIndex.value = next
}

const jumpToHref = (href) => {
  const target = String(href || '').split('#')[0]
  if (!target) return
  const idx = sections.value.findIndex((s) => s.href === target || s.href.endsWith(target))
  if (idx >= 0) {
    goSection(idx)
    return
  }
  // 目录项可能指向 section 内的锚点：在目录 href 里找 fragment 定位。
  const sameSection = sections.value.findIndex((s) => String(href).endsWith(s.href))
  if (sameSection >= 0) goSection(sameSection)
}

/** 当前 iframe 的 blob URL，切换章节时先回收，避免泄漏。 */
let frameBlobUrl = ''

const revokeFrameBlob = () => {
  if (!frameBlobUrl) return
  try {
    URL.revokeObjectURL(frameBlobUrl)
  } catch {
    /* 忽略 */
  }
  frameBlobUrl = ''
}

/** 把章节 HTML 写进 iframe（blob URL 方式，见模板注释）。 */
const writeFrameDocument = () => {
  const iframe = frameRef.value
  if (!(iframe instanceof HTMLIFrameElement)) return
  revokeFrameBlob()
  const blob = new Blob([frameDoc.value], { type: 'text/html;charset=utf-8' })
  frameBlobUrl = URL.createObjectURL(blob)
  iframe.removeAttribute('srcdoc')
  iframe.src = frameBlobUrl
}

const onFrameLoad = () => {
  try {
    const doc = frameRef.value?.contentDocument
    if (!doc) return
    // 章节内链接：拦截后按 href 在本书内跳转，而不是把 iframe 导航到不存在的路径。
    doc.addEventListener('click', (event) => {
      const anchor = event.target?.closest?.('a')
      if (!anchor) return
      const href = anchor.getAttribute('href') || ''
      if (!href || /^(https?:|mailto:|tel:)/i.test(href)) return
      event.preventDefault()
      jumpToHref(href)
    })
  } catch (error) {
    logger.debug('[DocEbookReader] frame hook skipped', { error: String(error?.message || error) })
  }
}

const onKeydown = (event) => {
  if (event.target && /input|textarea/i.test(event.target.tagName || '')) return
  if (event.key === 'ArrowLeft') {
    event.preventDefault()
    goSection(activeIndex.value - 1)
  } else if (event.key === 'ArrowRight') {
    event.preventDefault()
    goSection(activeIndex.value + 1)
  }
}

let tocResize = null

/**
 * 开始拖拽目录分隔线。
 *
 * 关键在 iframe：右侧正文是 <iframe>，鼠标一旦移进去，mousemove 就由 iframe 自己的
 * 文档接收，父窗口再也收不到事件 —— 表现就是「拖到一半就断、没法改宽度」。
 * 拖拽期间给正文盖一层透明遮罩（.is-resizing），事件始终落在父窗口上。
 */
const startTocResize = (event) => {
  event.preventDefault()
  tocResize = { startX: event.clientX, startWidth: tocWidth.value }
  resizing.value = true
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
  window.addEventListener('mousemove', onTocResize)
  window.addEventListener('mouseup', stopTocResize)
}

const onTocResize = (event) => {
  if (!tocResize) return
  tocWidth.value = Math.max(150, Math.min(520, tocResize.startWidth + event.clientX - tocResize.startX))
}

const stopTocResize = () => {
  tocResize = null
  resizing.value = false
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
  window.removeEventListener('mousemove', onTocResize)
  window.removeEventListener('mouseup', stopTocResize)
}

const loadBook = async () => {
  const token = ++loadToken
  loading.value = true
  errorText.value = ''
  sections.value = []
  toc.value = []
  activeIndex.value = 0

  try {
    const payload = await invoke('read_document_for_reader', {
      path: props.path,
      kind: props.kind,
    })
    if (token !== loadToken) return

    if (!payload || payload.format !== 'ebook') {
      throw new Error('后端未返回电子书结构')
    }
    title.value = payload.title || ''
    sections.value = Array.isArray(payload.sections) ? payload.sections : []
    toc.value = Array.isArray(payload.toc) ? payload.toc : []
    if (!sections.value.length) throw new Error('这本书没有可显示的正文')

    buildFrameDoc()
    loading.value = false
    logger.info('[DocEbookReader] loaded', {
      path: props.path,
      sections: sections.value.length,
      toc: toc.value.length,
    })
  } catch (error) {
    if (token !== loadToken) return
    loading.value = false
    errorText.value = String(error?.message || error || error)
    logger.warn('[DocEbookReader] load failed', {
      path: props.path,
      kind: props.kind,
      error: String(error?.message || error),
    })
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeydown)
  void loadBook()
})

onBeforeUnmount(() => {
  loadToken += 1
  window.removeEventListener('keydown', onKeydown)
  stopTocResize()
  revokeFrameBlob()
})

watch(frameDoc, () => {
  nextTick(() => writeFrameDocument())
})

watch(activeIndex, () => {
  buildFrameDoc()
  nextTick(() => {
    // 切节后回到顶部，否则会停在上一节的滚动位置。
    const body = frameRef.value?.contentDocument?.documentElement
    if (body) body.scrollTop = 0
  })
})

watch(zoom, () => {
  buildFrameDoc()
})

watch(
  () => props.path,
  () => {
    void loadBook()
  }
)
</script>
