// 任务条目字段正文的渲染支撑：缓存、并发队列、视口调度与搜索高亮。
//
// 这些状态必须放在模块作用域：几十张卡片 × 每卡多个分区，如果每个组件实例各持一份
// 队列与缓存，「全局并发上限」就变成了「每块 3 个」，滚动一次会同时向本地后端
// 发起上百个图片预处理请求，而且同一份内容会被重复渲染。
import { renderMarkdown, addCopyButtons, addMediaViewerButtons } from '../../GuaxiMarkdownRender/GuaxiMarker'
import { createLogger } from '../../../utils/logger'

const logger = createLogger('guaxiOrgFieldRender')

const RENDER_CACHE_LIMIT = 500
const MAX_CONCURRENT_RENDERS = 3

const renderCache = new Map()
const inflightRenders = new Map()
const renderQueue = []
let activeRenderCount = 0

export const orgFieldCacheKey = ({ content = '', theme = 'default', basePath = '' }) =>
  `${theme}\u0000${basePath}\u0000${content}`

const rememberRendered = (key, value) => {
  if (renderCache.has(key)) renderCache.delete(key)
  renderCache.set(key, value)
  while (renderCache.size > RENDER_CACHE_LIMIT) {
    renderCache.delete(renderCache.keys().next().value)
  }
}

const pumpRenderQueue = () => {
  while (activeRenderCount < MAX_CONCURRENT_RENDERS && renderQueue.length) {
    const task = renderQueue.shift()
    activeRenderCount += 1
    Promise.resolve()
      .then(task.run)
      .then(task.resolve, task.reject)
      .finally(() => {
        activeRenderCount -= 1
        pumpRenderQueue()
      })
  }
}

/**
 * 渲染一段任务正文为 HTML。
 * 相同「内容 + 主题 + 基准路径」只算一次；同一时刻最多 3 个渲染在跑。
 */
export const renderOrgFieldMarkdown = ({ content, theme = 'default', basePath = '' }) => {
  const source = String(content || '')
  if (!source.trim()) return Promise.resolve('')

  const key = orgFieldCacheKey({ content: source, theme, basePath })
  if (renderCache.has(key)) return Promise.resolve(renderCache.get(key))
  if (inflightRenders.has(key)) return inflightRenders.get(key)

  const promise = new Promise((resolve, reject) => {
    renderQueue.push({
      run: () => renderMarkdown(source, {
        basePath,
        theme,
        // 任务正文常常一行一个步骤，主预览的 CommonMark 规则会把它们并成一段。
        breaks: true,
        // 只有真写了围栏才走 mermaid，避免每张卡片都初始化一次图表引擎。
        mermaid: /```\s*mermaid\b/i.test(source),
        math: true,
        codeHighlight: true,
        chartEmbeds: false,
      }),
      resolve,
      reject,
    })
    pumpRenderQueue()
  })
    .then((html) => {
      rememberRendered(key, String(html || ''))
      return String(html || '')
    })
    .finally(() => {
      inflightRenders.delete(key)
    })

  inflightRenders.set(key, promise)
  return promise
}

// ───────── Mermaid 收尾（懒加载，只在内容里真有围栏时才 import） ─────────
let mermaidModulePromise = null
let mermaidInitialized = false

const ensureMermaid = async () => {
  if (!mermaidModulePromise) {
    mermaidModulePromise = import('mermaid').then((mod) => mod.default || mod)
  }
  const mermaid = await mermaidModulePromise
  if (!mermaidInitialized) {
    mermaidInitialized = true
    mermaid.initialize({ startOnLoad: false, theme: 'default', securityLevel: 'loose' })
  }
  return mermaid
}

export const renderMermaidIn = async (container) => {
  if (!container?.querySelectorAll) return
  const nodes = Array.from(container.querySelectorAll('pre.mermaid')).filter((node) => {
    if (node.getAttribute('data-processed') === 'true') return false
    return !node.querySelector('svg')
  })
  if (!nodes.length) return
  try {
    const mermaid = await ensureMermaid()
    for (const node of nodes) {
      try {
        await mermaid.run({ nodes: [node] })
      } catch (error) {
        logger.warn('[OrgFieldRender] mermaid node failed', { error: String(error?.message || error) })
      }
    }
  } catch (error) {
    logger.warn('[OrgFieldRender] mermaid unavailable', { error: String(error?.message || error) })
  }
}

/** 渲染完成后挂上与主编辑器一致的代码块 / 媒体浮动按钮。 */
export const decorateOrgFieldContainer = (container, theme = 'default') => {
  if (!container) return
  try {
    addCopyButtons(container, { theme })
  } catch (error) {
    logger.debug('[OrgFieldRender] copy buttons failed', { error: String(error?.message || error) })
  }
  try {
    addMediaViewerButtons(container, { theme })
  } catch (error) {
    logger.debug('[OrgFieldRender] media buttons failed', { error: String(error?.message || error) })
  }
}

// ───────── 搜索高亮：在已渲染的 DOM 上按文本节点包裹，不重跑渲染管线 ─────────
export const clearQueryHighlights = (container, className = 'otp-hit') => {
  container.querySelectorAll(`mark.${className}`).forEach((mark) => {
    const parent = mark.parentNode
    if (!parent) return
    parent.replaceChild(document.createTextNode(mark.textContent || ''), mark)
    parent.normalize()
  })
}

export const applyQueryHighlight = (container, query, className = 'otp-hit') => {
  if (!container) return
  clearQueryHighlights(container, className)
  const needle = String(query || '').trim()
  if (!needle) return

  const lowerNeedle = needle.toLowerCase()
  const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      if (!node.nodeValue || !node.nodeValue.trim()) return NodeFilter.FILTER_REJECT
      // 浮动按钮的文字（复制 / 查看图片…）不是正文，命中了也不该染色。
      if (node.parentElement?.closest('button')) return NodeFilter.FILTER_REJECT
      // 代码块要一起高亮：日志缩进四格就会被 markdown 解析成 <pre>，
      // 早先这里整块跳过，导致「搜到了、也跳过去了，但那个词没亮」。
      // 替换的是文本节点本身，hljs 的 span 结构原样保留，不会破坏上色。
      return NodeFilter.FILTER_ACCEPT
    },
  })

  const textNodes = []
  while (walker.nextNode()) textNodes.push(walker.currentNode)

  for (const textNode of textNodes) {
    const text = textNode.nodeValue || ''
    const lowerText = text.toLowerCase()
    let index = lowerText.indexOf(lowerNeedle)
    if (index < 0) continue

    const fragment = document.createDocumentFragment()
    let cursor = 0
    while (index >= 0) {
      if (index > cursor) fragment.appendChild(document.createTextNode(text.slice(cursor, index)))
      const mark = document.createElement('mark')
      mark.className = className
      mark.textContent = text.slice(index, index + needle.length)
      fragment.appendChild(mark)
      cursor = index + needle.length
      index = lowerText.indexOf(lowerNeedle, cursor)
    }
    if (cursor < text.length) fragment.appendChild(document.createTextNode(text.slice(cursor)))
    textNode.parentNode?.replaceChild(fragment, textNode)
  }
}

// ───────── 视口调度：所有实例共用一个 IntersectionObserver ─────────
let sharedObserver = null
const observerTasks = new WeakMap()

const ensureSharedObserver = () => {
  if (sharedObserver) return sharedObserver
  if (typeof IntersectionObserver !== 'function') return null
  sharedObserver = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue
      const task = observerTasks.get(entry.target)
      if (!task) continue
      observerTasks.delete(entry.target)
      sharedObserver?.unobserve(entry.target)
      task()
    }
  }, {
    root: null,
    rootMargin: '320px 0px 320px 0px',
    threshold: 0.01,
  })
  return sharedObserver
}

/** 元素进入视口附近时执行一次 cb；不支持 IntersectionObserver 时立即执行。 */
export const observeOnceVisible = (element, task) => {
  if (!element) {
    task()
    return () => {}
  }
  const observer = ensureSharedObserver()
  if (!observer) {
    task()
    return () => {}
  }
  observerTasks.set(element, task)
  observer.observe(element)
  return () => {
    observerTasks.delete(element)
    observer.unobserve(element)
  }
}
