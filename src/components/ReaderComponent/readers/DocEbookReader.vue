<template>
  <div class="guaxi-doc-reader" data-reader="ebook">
    <div class="guaxi-doc-reader__bar">
      <span class="guaxi-doc-reader__title" :title="path">{{ title || name }}</span>
      <span class="guaxi-doc-reader__badge">{{ kindLabel }}</span>
      <span v-if="sections.length" class="guaxi-doc-reader__meta gdr-ebook__section-count">
        {{ activeIndex + 1 }} / {{ sections.length }}
      </span>
      <span class="guaxi-doc-reader__spacer" />

      <!-- 图标按钮统一带 title + aria-label：鼠标悬停有提示，读屏也能读。 -->
      <button
        class="guaxi-doc-reader__icon-btn"
        :class="{ 'is-on': searchOpen }"
        title="全文搜索（Ctrl+F）"
        aria-label="全文搜索"
        :aria-pressed="searchOpen"
        @click="toggleSearch"
      >
        <Icon :icon="searchIcon" width="15" height="15" />
      </button>
      <button
        class="guaxi-doc-reader__icon-btn"
        :class="{ 'is-on': panelMode === 'bookmarks' }"
        title="书签（Ctrl+D 添加当前页）"
        aria-label="书签"
        :aria-pressed="panelMode === 'bookmarks'"
        @click="togglePanel('bookmarks')"
      >
        <Icon :icon="bookmarkIcon" width="15" height="15" />
      </button>
      <button
        class="guaxi-doc-reader__icon-btn"
        :class="{ 'is-on': panelMode === 'notes' }"
        :title="`标注与笔记${annotations.length ? `（${annotations.length} 条）` : ''}`"
        aria-label="标注与笔记"
        :aria-pressed="panelMode === 'notes'"
        @click="togglePanel('notes')"
      >
        <Icon :icon="noteIcon" width="15" height="15" />
      </button>
      <button
        class="guaxi-doc-reader__icon-btn"
        :class="{ 'is-on': panelMode === 'display' }"
        title="阅读设置：字号、字体、主题"
        aria-label="阅读设置"
        :aria-pressed="panelMode === 'display'"
        @click="togglePanel('display')"
      >
        <Icon :icon="typeIcon" width="15" height="15" />
      </button>

      <span class="guaxi-doc-reader__divider" />

      <button
        class="guaxi-doc-reader__icon-btn"
        title="上一节（←）"
        aria-label="上一节"
        :disabled="activeIndex <= 0"
        @click="goSection(activeIndex - 1)"
      >
        <Icon :icon="chevronLeftIcon" width="15" height="15" />
      </button>
      <span class="gdr-ebook__progress" :title="progressLabel ? `全书进度 ${progressLabel}` : ''">{{ progressLabel }}</span>
      <button
        class="guaxi-doc-reader__icon-btn"
        title="下一节（→）"
        aria-label="下一节"
        :disabled="activeIndex >= sections.length - 1"
        @click="goSection(activeIndex + 1)"
      >
        <Icon :icon="chevronRightIcon" width="15" height="15" />
      </button>

      <span class="guaxi-doc-reader__divider" />

      <button
        class="guaxi-doc-reader__icon-btn"
        :class="{ 'is-on': tocVisible }"
        title="目录"
        aria-label="目录"
        :aria-pressed="tocVisible"
        @click="tocVisible = !tocVisible"
      >
        <Icon :icon="tocIcon" width="15" height="15" />
      </button>
    </div>


    <!-- 搜索条 -->
    <div v-if="searchOpen" class="gdr-ebook__searchbar">
      <input
        ref="searchInputRef"
        v-model="searchQuery"
        class="gdr-ebook__search-input"
        type="search"
        placeholder="在全书范围内搜索…"
        aria-label="在全书范围内搜索，Enter 下一个，Esc 关闭"
        @input="runSearch"
        @keydown.enter.prevent="nextMatch(1)"
        @keydown.esc.prevent="toggleSearch"
      />
      <span class="guaxi-doc-reader__meta gdr-ebook__search-count">
        {{ searchHits.length ? `${searchCursor + 1} / ${searchHits.length}` : (searchQuery ? '无结果' : '') }}
      </span>
      <button
        class="guaxi-doc-reader__icon-btn"
        title="上一个匹配（Shift+Enter）"
        aria-label="上一个匹配"
        :disabled="!searchHits.length"
        @click="nextMatch(-1)"
      >
        <Icon :icon="arrowUpIcon" width="15" height="15" />
      </button>
      <button
        class="guaxi-doc-reader__icon-btn"
        title="下一个匹配（Enter）"
        aria-label="下一个匹配"
        :disabled="!searchHits.length"
        @click="nextMatch(1)"
      >
        <Icon :icon="arrowDownIcon" width="15" height="15" />
      </button>
      <button
        class="guaxi-doc-reader__icon-btn"
        title="关闭搜索（Esc）"
        aria-label="关闭搜索"
        @click="toggleSearch"
      >
        <Icon :icon="closeIcon" width="15" height="15" />
      </button>
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

      <!-- 右侧面板：书签 / 笔记 / 显示设置。与左侧目录对称，可独立开关。 -->
      <aside v-if="panelMode" class="gdr-ebook__panel" :style="{ width: `${panelWidth}px` }">
        <div class="gdr-ebook__panel-head">
          <span>{{ panelTitle }}</span>
          <button
            class="guaxi-doc-reader__icon-btn"
            title="关闭面板（Esc）"
            aria-label="关闭面板"
            @click="panelMode = ''"
          >
            <Icon :icon="closeIcon" width="15" height="15" />
          </button>
        </div>

        <!-- 书签 -->
        <div v-if="panelMode === 'bookmarks'" class="gdr-ebook__panel-body">
          <button class="gdr-ebook__panel-action" @click="addBookmarkAtCurrent">
            <Icon :icon="bookmarkIcon" width="14" height="14" />
            在当前位置添加书签
          </button>
          <div v-if="!bookmarks.length" class="guaxi-doc-reader__meta" style="padding: 12px 8px">
            还没有书签。阅读时点「+ 在当前位置添加书签」。
          </div>
          <div
            v-for="bm in bookmarks"
            :key="bm.id"
            class="gdr-ebook__panel-item"
            :title="bm.label"
          >
            <button class="gdr-ebook__panel-item-main" @click="jumpToBookmark(bm)">
              <span class="gdr-ebook__panel-item-title">{{ bm.label || '未命名书签' }}</span>
              <span class="gdr-ebook__panel-item-sub">
                第 {{ bm.sectionIndex + 1 }} 节 · {{ formatTime(bm.createdAt) }}
              </span>
            </button>
            <button class="gdr-ebook__panel-item-del" title="删除书签" aria-label="删除书签" @click="removeBookmark(bm)">
              <Icon :icon="closeIcon" width="13" height="13" />
            </button>
          </div>
        </div>

        <!-- 笔记 / 高亮 -->
        <div v-else-if="panelMode === 'notes'" class="gdr-ebook__panel-body">
          <div class="guaxi-doc-reader__meta" style="padding: 8px 4px">
            在正文中选中文字后点「高亮」即可添加。共 {{ annotations.length }} 条。
          </div>
          <div v-if="!annotations.length" class="guaxi-doc-reader__meta" style="padding: 12px 8px">
            还没有标注。
          </div>
          <div v-for="an in annotations" :key="an.id" class="gdr-ebook__panel-item is-note">
            <button class="gdr-ebook__panel-item-main" @click="jumpToAnnotation(an)">
              <span class="gdr-ebook__note-quote" :style="{ borderLeftColor: an.color }">
                {{ an.text }}
              </span>
              <span v-if="an.note" class="gdr-ebook__note-body">{{ an.note }}</span>
              <span class="gdr-ebook__panel-item-sub">第 {{ an.sectionIndex + 1 }} 节 · {{ formatTime(an.createdAt) }}</span>
            </button>
            <button class="gdr-ebook__panel-item-del" title="删除标注" aria-label="删除标注" @click="removeAnnotation(an)">
              <Icon :icon="closeIcon" width="13" height="13" />
            </button>
          </div>
        </div>

        <!-- 显示设置 -->
        <div v-else-if="panelMode === 'display'" class="gdr-ebook__panel-body">
          <label class="gdr-ebook__setting">
            <span>字号</span>
            <input v-model.number="prefs.fontSize" type="range" min="12" max="30" step="1" @change="applyPrefs" />
            <span class="gdr-ebook__setting-val">{{ prefs.fontSize }}px</span>
          </label>
          <label class="gdr-ebook__setting">
            <span>行距</span>
            <input v-model.number="prefs.lineHeight" type="range" min="1.2" max="2.6" step="0.1" @change="applyPrefs" />
            <span class="gdr-ebook__setting-val">{{ prefs.lineHeight.toFixed(1) }}</span>
          </label>
          <label class="gdr-ebook__setting">
            <span>页宽</span>
            <input v-model.number="prefs.maxWidth" type="range" min="500" max="1400" step="20" @change="applyPrefs" />
            <span class="gdr-ebook__setting-val">{{ prefs.maxWidth }}px</span>
          </label>
          <div class="gdr-ebook__setting">
            <span>字体</span>
            <div class="gdr-ebook__seg">
              <button
                v-for="opt in fontOptions"
                :key="opt.value"
                :class="{ 'is-on': prefs.fontFamily === opt.value }"
                @click="setFontFamily(opt.value)"
              >{{ opt.label }}</button>
            </div>
          </div>
          <div class="gdr-ebook__setting">
            <span>主题</span>
            <div class="gdr-ebook__seg">
              <button
                v-for="opt in themeOptions"
                :key="opt.value"
                :class="{ 'is-on': prefs.theme === opt.value }"
                @click="setTheme(opt.value)"
              >{{ opt.label }}</button>
            </div>
          </div>
          <div class="gdr-ebook__setting">
            <span>翻页</span>
            <div class="gdr-ebook__seg">
              <button :class="{ 'is-on': prefs.layoutMode === 'scroll' }" @click="setLayoutMode('scroll')">滚动</button>
              <button :class="{ 'is-on': prefs.layoutMode === 'paginated' }" @click="setLayoutMode('paginated')">分页</button>
            </div>
          </div>
        </div>
      </aside>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Icon } from '@iconify/vue'
// 图标走「import 对象」路径（不是 iconRegistry 里的名字）：release 包离线可用，
// 名字写错在构建期就报模块找不到，不会变成一片空白。
import searchIcon from '@iconify-icons/lucide/search'
import bookmarkIcon from '@iconify-icons/lucide/bookmark'
import noteIcon from '@iconify-icons/lucide/edit-3'
import typeIcon from '@iconify-icons/lucide/type'
import chevronLeftIcon from '@iconify-icons/lucide/chevron-left'
import chevronRightIcon from '@iconify-icons/lucide/chevron-right'
import tocIcon from '@iconify-icons/lucide/list-tree'
import closeIcon from '@iconify-icons/lucide/x'
import arrowUpIcon from '@iconify-icons/lucide/arrow-up'
import arrowDownIcon from '@iconify-icons/lucide/arrow-down'
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

// ───────────────────────── 阅读设置 ─────────────────────────
//
// 字体与主题只影响 iframe 内部的 CSS，不改任何章节 HTML。

const FONT_STACKS = {
  serif: '"Noto Serif SC", "Songti SC", "SimSun", Georgia, serif',
  sans: '"Noto Sans SC", "PingFang SC", "Microsoft YaHei", system-ui, sans-serif',
  mono: '"Maple Mono NF CN", "JetBrainsMono Nerd Font Mono", Consolas, monospace',
}

const THEME_VARS = {
  light: { bg: '#ffffff', fg: '#1f2328', link: '#2f6fdf', border: 'rgba(128,128,128,.35)' },
  sepia: { bg: '#f5ecd9', fg: '#3b3226', link: '#8b5e34', border: 'rgba(139,94,52,.35)' },
  gray: { bg: '#e9ecef', fg: '#2b2f33', link: '#2f6fdf', border: 'rgba(43,47,51,.25)' },
  dark: { bg: '#1c1f22', fg: '#d6d9dc', link: '#6cb6ff', border: 'rgba(214,217,220,.25)' },
}

const fontOptions = [
  { value: 'serif', label: '衬线' },
  { value: 'sans', label: '黑体' },
  { value: 'mono', label: '等宽' },
]
const themeOptions = [
  { value: 'light', label: '白' },
  { value: 'sepia', label: '护眼' },
  { value: 'gray', label: '灰' },
  { value: 'dark', label: '深色' },
]

const prefs = ref({
  fontSize: 16,
  lineHeight: 1.8,
  fontFamily: 'serif',
  theme: 'light',
  layoutMode: 'scroll',
  maxWidth: 860,
})

/** 一本书的稳定标识：规范化路径。跨窗口/跨重启一致，才能恢复进度。 */
const bookKey = computed(() => String(props.path || '').replace(/\\/g, '/').trim())

// ───────────────────────── 面板状态 ─────────────────────────

const panelMode = ref('')
const panelWidth = ref(320)
const panelTitle = computed(() => {
  if (panelMode.value === 'bookmarks') return '书签'
  if (panelMode.value === 'notes') return '标注与笔记'
  if (panelMode.value === 'display') return '显示设置'
  return ''
})

const togglePanel = (mode) => {
  panelMode.value = panelMode.value === mode ? '' : mode
}

const bookmarks = ref([])
const annotations = ref([])

const formatTime = (ms) => {
  if (!ms) return ''
  try {
    return new Date(ms).toLocaleDateString('zh-CN')
  } catch {
    return ''
  }
}

// ───────────────────────── 搜索 ─────────────────────────

const searchOpen = ref(false)
const searchQuery = ref('')
const searchHits = ref([])
const searchCursor = ref(0)
const searchInputRef = ref(null)

const toggleSearch = () => {
  searchOpen.value = !searchOpen.value
  if (searchOpen.value) {
    nextTick(() => searchInputRef.value?.focus())
  } else {
    searchQuery.value = ''
    searchHits.value = []
    searchCursor.value = 0
    clearSearchHighlights()
  }
}

/** 在章节 HTML 的文本节点里找匹配，返回 [{ sectionIndex, index, snippet }]。 */
const runSearch = () => {
  const query = String(searchQuery.value || '').trim()
  if (!query) {
    searchHits.value = []
    searchCursor.value = 0
    clearSearchHighlights()
    return
  }
  const lowerQuery = query.toLowerCase()
  const hits = []
  sections.value.forEach((section, sectionIndex) => {
    const text = htmlToPlainText(section.html)
    const lower = text.toLowerCase()
    let from = 0
    while (hits.length < 2000) {
      const at = lower.indexOf(lowerQuery, from)
      if (at < 0) break
      hits.push({
        sectionIndex,
        index: at,
        snippet: text.slice(Math.max(0, at - 30), at + query.length + 30).trim(),
      })
      from = at + lowerQuery.length
    }
  })
  searchHits.value = hits
  searchCursor.value = 0
  logger.info('[DocEbookReader] search done', { query, hits: hits.length })
  if (hits.length) void goToHit(0)
}

const nextMatch = (delta) => {
  if (!searchHits.value.length) return
  const next = (searchCursor.value + delta + searchHits.value.length) % searchHits.value.length
  void goToHit(next)
}

/** 跳到第 n 个匹配：切到对应章节，再在 iframe 内高亮。 */
const goToHit = async (n) => {
  searchCursor.value = n
  const hit = searchHits.value[n]
  if (!hit) return
  if (hit.sectionIndex !== activeIndex.value) {
    activeIndex.value = hit.sectionIndex
    await nextTick()
    // 章节切换是异步写 blob 的，等 iframe load 完再高亮。
    await waitForFrame()
  }
  applySearchHighlight(hit.index, String(searchQuery.value || '').length)
}

const waitForFrame = async () => {
  for (let i = 0; i < 40; i += 1) {
    await new Promise((resolve) => setTimeout(resolve, 50))
    if (frameRef.value?.contentDocument?.body) return
  }
}

/** 在 iframe 文档里把第 n 个匹配包成 <mark>。 */
const applySearchHighlight = (charIndex, length) => {
  const doc = frameRef.value?.contentDocument
  if (!doc?.body) return
  clearSearchHighlights()
  try {
    const walker = doc.createTreeWalker(doc.body, NodeFilter.SHOW_TEXT, {
      acceptNode: (node) => (node.nodeValue && node.nodeValue.trim() ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT),
    })
    let consumed = 0
    let node = walker.nextNode()
    while (node) {
      const text = node.nodeValue || ''
      if (consumed + text.length > charIndex) {
        const start = charIndex - consumed
        const end = Math.min(text.length, start + length)
        const range = doc.createRange()
        range.setStart(node, start)
        range.setEnd(node, end)
        const mark = doc.createElement('mark')
        mark.className = 'gx-hit is-current'
        try {
          range.surroundContents(mark)
          mark.scrollIntoView({ block: 'center', behavior: 'smooth' })
        } catch {
          /* 跨节点匹配时 surroundContents 会失败，退化为只滚动到该节点 */
          node.parentElement?.scrollIntoView({ block: 'center', behavior: 'smooth' })
        }
        return
      }
      consumed += text.length
      node = walker.nextNode()
    }
  } catch (error) {
    logger.debug('[DocEbookReader] search highlight failed', {
      error: String(error?.message || error),
    })
  }
}

const clearSearchHighlights = () => {
  const doc = frameRef.value?.contentDocument
  if (!doc) return
  doc.querySelectorAll('mark.gx-hit').forEach((el) => {
    const parent = el.parentNode
    if (!parent) return
    while (el.firstChild) parent.insertBefore(el.firstChild, el)
    parent.removeChild(el)
  })
}

/** 章节 HTML → 纯文本（搜索用，不参与渲染）。 */
const htmlToPlainText = (html) => {
  const withBreaks = String(html || '')
    .replace(/<(script|style)[\s\S]*?<\/\1>/gi, ' ')
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<\/(p|div|li|h[1-6]|tr)>/gi, '\n')
  const noTags = withBreaks.replace(/<[^>]+>/g, '')
  const textarea = document.createElement('textarea')
  textarea.innerHTML = noTags
  return textarea.value
}

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
  // 显示设置全部落到这里：字号 / 行距 / 页宽 / 字体 / 主题。
  // 主题用 CSS 变量注入到 iframe 内部，避免每章正文写死颜色。
  const p = prefs.value
  const fontStack = FONT_STACKS[p.fontFamily] || FONT_STACKS.serif
  const themeVars = THEME_VARS[p.theme] || THEME_VARS.light
  const baseCss = `
    html, body { margin: 0; padding: 0; background: transparent; }
    :root {
      --gx-bg: ${themeVars.bg};
      --gx-fg: ${themeVars.fg};
      --gx-link: ${themeVars.link};
      --gx-quote: ${themeVars.border};
    }
    body {
      font-family: ${fontStack};
      line-height: ${p.lineHeight};
      font-size: ${p.fontSize * zoom.value}px;
      color: var(--gx-fg);
      background: var(--gx-bg);
      padding: 24px 32px 80px;
      max-width: ${p.maxWidth}px;
      margin: 0 auto;
      word-wrap: break-word;
    }
    img, svg, video { max-width: 100%; height: auto; }
    table { border-collapse: collapse; max-width: 100%; }
    td, th { border: 1px solid var(--gx-quote); padding: 4px 8px; }
    pre { white-space: pre-wrap; word-break: break-word; }
    blockquote { border-left: 3px solid var(--gx-quote); margin-left: 0; padding-left: 12px; opacity: .85; }
    a { color: var(--gx-link); }
    /* 搜索命中：整章内所有匹配用浅黄底，当前匹配用橙色描边。 */
    mark.gx-hit { background: rgba(255, 214, 0, .55); color: inherit; padding: 0; }
    mark.gx-hit.is-current { background: rgba(255, 140, 0, .75); outline: 2px solid rgba(255, 110, 0, .9); }
    /* 高亮标注 */
    mark.gx-annot { color: inherit; padding: 0; border-radius: 2px; }
    ${p.layoutMode === 'paginated' ? `
      html, body { height: 100%; overflow: hidden; }
      body { height: 100vh; box-sizing: border-box; overflow-y: auto; column-width: ${p.maxWidth}px; }
    ` : ''}
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
    // 滚动进度跟踪：用于保存阅读位置。
    doc.addEventListener('scroll', onFrameScroll, { passive: true })
    // 划词高亮：选中文字后浮出「高亮」小条。
    doc.addEventListener('mouseup', onFrameSelection)
    // 恢复本节滚动位置（首次进书或跳转书签后由调用方设置）。
    applyPendingRestore()
    // 本章已有标注画上底色。
    applyAnnotationHighlights()
  } catch (error) {
    logger.debug('[DocEbookReader] frame hook skipped', {
      error: String(error?.message || error),
    })
  }
  // 搜索跳转后若落在本章，需要重新打高亮（新文档里的 mark 已被替换掉）。
  if (searchHits.value.length && searchQuery.value) {
    const hit = searchHits.value[searchCursor.value]
    if (hit && hit.sectionIndex === activeIndex.value) {
      applySearchHighlight(hit.index, String(searchQuery.value).length)
    }
  }
}

// ───────────────────────── 进度持久化 ─────────────────────────

let saveStateTimer = null
/** 切章/跳书签时待恢复的滚动比例；iframe load 后消费一次。 */
let pendingRestoreRatio = null

const onFrameScroll = () => {
  if (saveStateTimer) clearTimeout(saveStateTimer)
  saveStateTimer = setTimeout(() => {
    saveStateTimer = null
    void persistState()
    updateProgressLabel()
  }, 600)
}

const currentScrollRatio = () => {
  const el = frameRef.value?.contentDocument?.documentElement
  if (!el) return 0
  const max = el.scrollHeight - el.clientHeight
  if (max <= 0) return 0
  return Math.min(1, Math.max(0, el.scrollTop / max))
}

const applyPendingRestore = () => {
  if (pendingRestoreRatio == null) return
  const ratio = pendingRestoreRatio
  pendingRestoreRatio = null
  const el = frameRef.value?.contentDocument?.documentElement
  if (!el) return
  const max = el.scrollHeight - el.clientHeight
  if (max > 0) el.scrollTop = ratio * max
  updateProgressLabel()
}

const persistState = async () => {
  if (!bookKey.value || loading.value) return
  try {
    await invoke('reader_save_state', {
      bookKey: bookKey.value,
      sectionIndex: activeIndex.value,
      scrollRatio: currentScrollRatio(),
    })
  } catch (error) {
    logger.debug('[DocEbookReader] save state failed', {
      error: String(error?.message || error),
    })
  }
}

const progressLabel = ref('')

const updateProgressLabel = () => {
  if (!sections.value.length) {
    progressLabel.value = ''
    return
  }
  const ratio = currentScrollRatio()
  const overall = Math.min(1, Math.max(0, (activeIndex.value + ratio) / sections.value.length))
  progressLabel.value = `${Math.round(overall * 100)}%`
}

// ───────────────────────── 书签 ─────────────────────────

const addBookmarkAtCurrent = async () => {
  if (!bookKey.value) return
  const section = sections.value[activeIndex.value]
  const heading = htmlToPlainText(section?.html || '').trim().slice(0, 40) || `第 ${activeIndex.value + 1} 节`
  try {
    const bm = await invoke('reader_add_bookmark', {
      bookKey: bookKey.value,
      sectionIndex: activeIndex.value,
      scrollRatio: currentScrollRatio(),
      label: heading,
    })
    bookmarks.value = [bm, ...bookmarks.value]
    logger.info('[DocEbookReader] bookmark added', { label: heading })
  } catch (error) {
    logger.warn('[DocEbookReader] add bookmark failed', { error: String(error?.message || error) })
  }
}

const jumpToBookmark = (bm) => {
  if (bm.sectionIndex !== activeIndex.value) {
    pendingRestoreRatio = bm.scrollRatio
    activeIndex.value = bm.sectionIndex
  } else {
    const el = frameRef.value?.contentDocument?.documentElement
    const max = el ? el.scrollHeight - el.clientHeight : 0
    if (el && max > 0) el.scrollTop = bm.scrollRatio * max
  }
}

const removeBookmark = async (bm) => {
  try {
    await invoke('reader_delete_bookmark', { id: bm.id })
    bookmarks.value = bookmarks.value.filter((item) => item.id !== bm.id)
  } catch (error) {
    logger.warn('[DocEbookReader] delete bookmark failed', { error: String(error?.message || error) })
  }
}

// ───────────────────────── 标注 ─────────────────────────

/** 选中文字后浮出的小操作条。 */
let selectionBar = null

const onFrameSelection = () => {
  const doc = frameRef.value?.contentDocument
  if (!doc) return
  const selection = doc.getSelection?.()
  const text = String(selection?.toString?.() || '').trim()
  removeSelectionBar()
  if (!text || text.length < 2) return

  const range = selection.getRangeAt?.(0)
  if (!range) return
  const rect = range.getBoundingClientRect()

  const bar = doc.createElement('div')
  bar.className = 'gx-selbar'
  bar.style.cssText = `position:absolute;left:${Math.max(4, rect.left)}px;top:${Math.max(4, rect.top - 34)}px;z-index:9999;display:flex;gap:4px;background:#222;border-radius:6px;padding:4px;box-shadow:0 4px 12px rgba(0,0,0,.3);`

  const colors = { '#ffd600': '黄', '#a5d6a7': '绿', '#90caf9': '蓝', '#f48fb1': '粉' }
  Object.entries(colors).forEach(([color, label]) => {
    const btn = doc.createElement('button')
    btn.textContent = label
    btn.style.cssText = `border:0;background:${color};color:#222;border-radius:4px;width:24px;height:22px;font-size:11px;cursor:pointer;`
    btn.addEventListener('mousedown', (event) => {
      event.preventDefault()
      event.stopPropagation()
      void addAnnotationFromSelection(text, color)
    })
    bar.appendChild(btn)
  })

  doc.body.appendChild(bar)
  selectionBar = bar
}

const removeSelectionBar = () => {
  try {
    selectionBar?.remove?.()
  } catch {
    /* 已移除 */
  }
  selectionBar = null
}

const addAnnotationFromSelection = async (text, color) => {
  removeSelectionBar()
  if (!bookKey.value) return
  try {
    const an = await invoke('reader_add_annotation', {
      bookKey: bookKey.value,
      sectionIndex: activeIndex.value,
      text,
      note: '',
      color,
    })
    annotations.value = [...annotations.value, an]
    applyAnnotationHighlights()
    panelMode.value = 'notes'
    logger.info('[DocEbookReader] annotation added', { chars: text.length })
  } catch (error) {
    logger.warn('[DocEbookReader] add annotation failed', { error: String(error?.message || error) })
  }
}

/** 把本章已有标注在正文里画成底色。 */
const applyAnnotationHighlights = () => {
  const doc = frameRef.value?.contentDocument
  if (!doc?.body) return
  doc.querySelectorAll('mark.gx-annot').forEach((el) => {
    const parent = el.parentNode
    if (!parent) return
    while (el.firstChild) parent.insertBefore(el.firstChild, el)
    parent.removeChild(el)
  })
  const mine = annotations.value.filter((an) => an.sectionIndex === activeIndex.value)
  if (!mine.length) return
  try {
    for (const an of mine) {
      const needle = String(an.text || '').trim()
      if (!needle) continue
      const walker = doc.createTreeWalker(doc.body, NodeFilter.SHOW_TEXT, {
        acceptNode: (node) => (node.nodeValue && node.nodeValue.trim() ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT),
      })
      let node = walker.nextNode()
      while (node) {
        const value = node.nodeValue || ''
        const at = value.indexOf(needle)
        if (at >= 0) {
          const range = doc.createRange()
          range.setStart(node, at)
          range.setEnd(node, at + needle.length)
          const mark = doc.createElement('mark')
          mark.className = 'gx-annot'
          mark.style.background = an.color || '#ffd600'
          mark.title = an.note || ''
          try {
            range.surroundContents(mark)
          } catch {
            /* 跨节点时不强求 */
          }
          break
        }
        node = walker.nextNode()
      }
    }
  } catch (error) {
    logger.debug('[DocEbookReader] annotation highlight failed', {
      error: String(error?.message || error),
    })
  }
}

const jumpToAnnotation = (an) => {
  activeIndex.value = an.sectionIndex
}

const removeAnnotation = async (an) => {
  try {
    await invoke('reader_delete_annotation', { id: an.id })
    annotations.value = annotations.value.filter((item) => item.id !== an.id)
    applyAnnotationHighlights()
  } catch (error) {
    logger.warn('[DocEbookReader] delete annotation failed', { error: String(error?.message || error) })
  }
}

// ───────────────────────── 设置持久化 ─────────────────────────

let persistPrefsTimer = null

const applyPrefs = () => {
  buildFrameDoc()
  if (persistPrefsTimer) clearTimeout(persistPrefsTimer)
  persistPrefsTimer = setTimeout(async () => {
    persistPrefsTimer = null
    if (!bookKey.value) return
    try {
      await invoke('reader_save_prefs', { bookKey: bookKey.value, prefs: prefs.value })
    } catch (error) {
      logger.debug('[DocEbookReader] save prefs failed', {
        error: String(error?.message || error),
      })
    }
  }, 400)
}

const setFontFamily = (value) => {
  prefs.value.fontFamily = value
  applyPrefs()
}

const setTheme = (value) => {
  prefs.value.theme = value
  applyPrefs()
}

const setLayoutMode = (value) => {
  prefs.value.layoutMode = value
  applyPrefs()
}

const loadReaderSettings = async () => {
  if (!bookKey.value) return null
  try {
    const [savedPrefs, savedState, savedBookmarks, savedAnnotations] = await Promise.all([
      invoke('reader_get_prefs', { bookKey: bookKey.value }),
      invoke('reader_get_state', { bookKey: bookKey.value }),
      invoke('reader_list_bookmarks', { bookKey: bookKey.value }),
      invoke('reader_list_annotations', { bookKey: bookKey.value }),
    ])
    if (savedPrefs) prefs.value = { ...prefs.value, ...savedPrefs }
    bookmarks.value = Array.isArray(savedBookmarks) ? savedBookmarks : []
    annotations.value = Array.isArray(savedAnnotations) ? savedAnnotations : []
    logger.info('[DocEbookReader] reader settings loaded', {
      bookmarks: bookmarks.value.length,
      annotations: annotations.value.length,
      hasState: !!savedState,
    })
    return savedState || null
  } catch (error) {
    logger.warn('[DocEbookReader] load reader settings failed', {
      error: String(error?.message || error),
    })
    return null
  }
}

const onKeydown = (event) => {
  if (event.target && /input|textarea/i.test(event.target.tagName || '')) return
  // Ctrl/Cmd+F：全文搜索
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'f') {
    event.preventDefault()
    if (!searchOpen.value) toggleSearch()
    else searchInputRef.value?.focus()
    return
  }
  // Ctrl/Cmd+D：加书签
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'd') {
    event.preventDefault()
    void addBookmarkAtCurrent()
    return
  }
  if (event.key === 'Escape') {
    removeSelectionBar()
    if (searchOpen.value) toggleSearch()
    else if (panelMode.value) panelMode.value = ''
    return
  }
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

    // 先读回设置与进度：prefs 会影响 buildFrameDoc 的排版，必须先于渲染。
    const savedState = await loadReaderSettings()
    if (token !== loadToken) return
    if (savedState && Number.isInteger(savedState.sectionIndex)) {
      activeIndex.value = Math.min(
        Math.max(0, savedState.sectionIndex),
        sections.value.length - 1
      )
      pendingRestoreRatio = Number(savedState.scrollRatio) || 0
    }

    buildFrameDoc()
    loading.value = false
    logger.info('[DocEbookReader] loaded', {
      path: props.path,
      sections: sections.value.length,
      toc: toc.value.length,
      resumedSection: activeIndex.value,
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
  // 卸载前把最后位置写回，否则「读完关窗」会丢掉这段进度。
  if (saveStateTimer) {
    clearTimeout(saveStateTimer)
    saveStateTimer = null
  }
  void persistState()
})

watch(frameDoc, () => {
  nextTick(() => writeFrameDocument())
})

watch(activeIndex, () => {
  buildFrameDoc()
  nextTick(() => {
    // 切节后回到顶部，否则会停在上一节的滚动位置。
    const body = frameRef.value?.contentDocument?.documentElement
    if (body && pendingRestoreRatio == null) body.scrollTop = 0
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
