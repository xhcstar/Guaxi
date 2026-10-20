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
        <!--
          v-if 而不是常驻：iframe 一旦插入就会先创建一个 about:blank 文档；
          只有 frameSrc 就绪才挂载，且挂载时直接 src 到 blob，跳过空白文档阶段。

          sandbox 保留 allow-same-origin：需要读 contentDocument 来拦截书籍内链接，
          让 `<a href="ch2.xhtml">` 在阅读器内跳转，而不是导航到不存在的地址。
          脚本安全性由「剥离 + 自检」保证（见 sanitizeSectionHtml），而不是靠沙箱。
        -->
        <iframe
          v-if="frameSrc"
          ref="frameRef"
          class="gdr-ebook__frame"
          :sandbox="frameSandbox"
          referrerpolicy="no-referrer"
          :src="frameSrc"
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
import { sanitizeSectionHtml } from './ebookSanitize.js'

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
/** 绑定给 iframe 的 blob URL（模板用 v-if + :src，先算好再挂载）。 */
const frameSrc = ref('')

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
/**
 * iframe 的沙箱策略。
 *
 * 保留 allow-same-origin 是为了读 contentDocument 做书籍内跳转；
 * 不能加 allow-scripts —— 全书正文都不需要执行脚本。
 */
const frameSandbox = 'allow-same-origin'

/** 净化并自检（剥离逻辑与判据见 ebookSanitize.js）；退化时记一条日志便于排查。 */
const sanitizeSection = (html) => {
  const result = sanitizeSectionHtml(html)
  if (result.degraded) {
    logger.warn('[DocEbookReader] script survived stripping, degraded to plain text', {
      path: props.path,
      section: activeIndex.value,
    })
  }
  return result.html
}

/**
 * 把当前章节 HTML 组装成完整的 iframe 文档串。
 *
 * 负责三件事：
 * 1. 净化（剥离脚本与嵌套容器 + 自检）；
 * 2. 注入阅读器基础样式（字号随缩放联动、正文最大宽度、图片自适应）；
 * 3. 处理「完整文档 / 片段」两种输入形态。
 *
 * 注意：这里只更新 frameDoc 这个 ref，真正写进 iframe 由 writeFrameDocument 负责。
 */
const buildFrameDoc = () => {
  const section = sections.value[activeIndex.value]
  if (!section) {
    frameDoc.value = '<!doctype html><html><body></body></html>'
    return
  }

  const raw = sanitizeSection(section.html)
  const isFullDoc = /<html[\s>]/i.test(raw)
  const baseCss = `
    html, body { margin: 0; padding: 0; background: transparent; }
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
    // 完整文档：把基础样式插在 </head> 前；没有 head 就补一个。
    let next = raw.replace(/<\/head>/i, `<style>${baseCss}</style></head>`)
    if (next === raw) {
      next = raw.replace(/<html([^>]*)>/i, `<html$1><head><style>${baseCss}</style></head>`)
      if (!/<head/i.test(next)) {
        next = `<style>${baseCss}</style>${raw}`
      }
    }
    frameDoc.value = next
    return
  }

  frameDoc.value = `<!doctype html><html><head><meta charset="utf-8"><style>${baseCss}</style></head><body>${raw}</body></html>`
}

/** 切到指定章节（越界自动收敛）。 */
const goSection = (index) => {
  const next = Math.max(0, Math.min(sections.value.length - 1, index))
  if (next === activeIndex.value) return
  activeIndex.value = next
}

/**
 * 按 href 跳到书籍内某一节。
 *
 * 目录项与章节内的相对链接都走这里：先按完整 href 精确匹配，
 * 再退一步用「谁以这个 href 结尾」做兜底（章节文件可能在子目录里）。
 * 目录项若指向节内锚点（`ch1.xhtml#part2`），只切到对应的节。
 */
const jumpToHref = (href) => {
  const target = String(href || '').split('#')[0]
  if (!target) return
  const idx = sections.value.findIndex(
    (s) => s.href === target || String(s.href).endsWith(target)
  )
  if (idx >= 0) {
    goSection(idx)
    return
  }
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

/**
 * 生成章节文档的 blob URL，驱动 iframe 挂载。
 *
 * 先把 frameSrc 置空让 v-if 销毁旧 iframe，再挂新的：
 * 直接改 src 会让同一个浏览上下文经历一次导航，旧文档的脚本会被重新解析并再报一次拦截。
 */
const writeFrameDocument = () => {
  revokeFrameBlob()
  frameSrc.value = ''
  const blob = new Blob([frameDoc.value], { type: 'text/html;charset=utf-8' })
  frameBlobUrl = URL.createObjectURL(blob)
  nextTick(() => {
    frameSrc.value = frameBlobUrl
  })
}

const onFrameLoad = () => {
  // 保留 allow-same-origin 就是为了这里：读一次内部文档，挂上链接拦截，
  // 让 `<a href="ch2.xhtml">` 在阅读器内跳转，而不是把 iframe 导航到不存在的地址。
  try {
    const doc = frameRef.value?.contentDocument
    if (!doc) return
    doc.addEventListener('click', (event) => {
      const anchor = event.target?.closest?.('a')
      if (!anchor) return
      const href = anchor.getAttribute('href') || ''
      if (!href || /^(https?:|mailto:|tel:|#)/i.test(href)) return
      event.preventDefault()
      jumpToHref(href)
    })
  } catch (error) {
    logger.debug('[DocEbookReader] frame hook skipped', {
      error: String(error?.message || error),
    })
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
