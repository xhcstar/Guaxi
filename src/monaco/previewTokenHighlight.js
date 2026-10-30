import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../utils/logger'
import { buildTokenHtml } from './tokenHtml.js'

// 设计说明：
// 1. Markdown 预览里的代码块一直由 highlight.js 上色，但 hljs 的类名比 tree-sitter 粗：
//    它产不出「类方法 / 命名空间 / 装饰器」，同一条 `.figure` 在预览里只能落回函数色，
//    而编辑器围栏和 Notebook cell 是分开的——这就是最后剩下的那点不一致。
// 2. 后端正则有一份 tree-sitter token 服务（`syntax_highlight_tokens`，结果带 LRU 缓存）。
//    这里在预览 DOM 挂载后，把同一段代码换成 `guaxi-tok-*` span，三个入口就落到同一批桶上。
// 3. 只改已经渲染出来的代码块，不动渲染管线：渲染器（主线程 / worker）仍然同步出 HTML，
//    所以导出、打印拿到的是原来的 `.hljs-*` 结构，不受影响。

const logger = createLogger('PreviewTokens')

// 与后端 HIGHLIGHT_CONFIGS 支持的语言保持一致；不在表里的语言继续用 hljs 的颜色。
const SUPPORTED_LANGUAGES = new Set([
  'markdown',
  'javascript',
  'typescript',
  'tsx',
  'python',
  'rust',
  'bash',
  'lua',
  'java',
  'html',
  'c',
  'cpp',
  'latex',
])

const LANGUAGE_ALIASES = {
  md: 'markdown',
  mdown: 'markdown',
  js: 'javascript',
  jsx: 'javascript',
  mjs: 'javascript',
  cjs: 'javascript',
  ts: 'typescript',
  py: 'python',
  python3: 'python',
  rs: 'rust',
  sh: 'bash',
  shell: 'bash',
  zsh: 'bash',
  powershell: 'bash',
  ps1: 'bash',
  tex: 'latex',
  'c++': 'cpp',
  cc: 'cpp',
  cxx: 'cpp',
  hpp: 'cpp',
  hxx: 'cpp',
  htm: 'html',
  xml: 'html',
  vue: 'html',
}

const CACHE_LIMIT = 240
const MAX_CODE_LENGTH = 200_000

const tokenCache = new Map()
let started = false
// 失败只冷却一段时间，不做永久禁用：应用启动早期的一次失败不应该让整场预览都不上色。
const FAILURE_COOLDOWN_MS = 30_000
let failureCooldownUntil = 0

const rememberTokens = (key, html) => {
  if (tokenCache.size >= CACHE_LIMIT) {
    const oldest = tokenCache.keys().next().value
    if (oldest != null) tokenCache.delete(oldest)
  }
  tokenCache.set(key, html)
}

const isCandidate = (codeEl) => (
  codeEl
  && codeEl.tagName === 'CODE'
  && codeEl.dataset.guaxiTokens !== '1'
  && codeEl.dataset.guaxiTokens !== 'pending'
  && !!resolveLanguage(codeEl)
)

/**
 * 取一段代码的 token HTML；不支持的语言 / 超长代码返回空串，调用方保留 hljs 的结果。
 * 预览渲染和导出两条链路共用这里，缓存也共享。
 */
const fetchTokenHtml = async (code, language) => {
  if (!language || !code.trim() || code.length > MAX_CODE_LENGTH) return ''
  if (Date.now() < failureCooldownUntil) return ''
  const cacheKey = `${language}
${code}`
  const cached = tokenCache.get(cacheKey)
  if (cached != null) return cached

  try {
    const response = await invoke('syntax_highlight_tokens', { request: { code, language } })
    if (!response?.supported || !Array.isArray(response.spans) || !response.spans.length) return ''
    const html = buildTokenHtml(code, response.spans)
    rememberTokens(cacheKey, html)
    return html
  } catch (error) {
    failureCooldownUntil = Date.now() + FAILURE_COOLDOWN_MS
    logger.warn('[PreviewTokens] tree-sitter token request failed, keep highlight.js colors', {
      language,
      error: String(error?.message || error),
    })
    return ''
  }
}

const upgradeBlock = async (codeEl) => {
  if (!isCandidate(codeEl)) return
  const language = resolveLanguage(codeEl)
  const code = codeEl.textContent || ''
  if (!language || !code.trim() || code.length > MAX_CODE_LENGTH) {
    codeEl.dataset.guaxiTokens = 'skip'
    return
  }

  codeEl.dataset.guaxiTokens = 'pending'
  const html = await fetchTokenHtml(code, language)
  if (!html) {
    codeEl.dataset.guaxiTokens = 'skip'
    return
  }
  // 等待期间这段代码可能已经被重渲染（流式输出场景），内容一变就放弃这次结果。
  if (codeEl.isConnected && codeEl.textContent === code) {
    codeEl.innerHTML = html
    codeEl.dataset.guaxiTokens = '1'
  } else {
    delete codeEl.dataset.guaxiTokens
  }
}

/**
 * 把一段已经渲染好的 Markdown HTML 里的代码块也换成 tree-sitter 着色。
 *
 * 导出 / 打印走的是「渲染成 HTML 字符串 → 写文件」，没有真实 DOM 可观察，
 * 所以在字符串层面做同一件事，导出的 PDF/HTML 与屏幕上保持同一套颜色。
 */
export const upgradeHtmlCodeBlocks = async (html) => {
  const source = String(html || '')
  if (!source || typeof document === 'undefined') return source

  const template = document.createElement('template')
  template.innerHTML = source
  const blocks = Array.from(template.content.querySelectorAll('pre > code[class*="language-"]'))
  if (!blocks.length) return source

  await Promise.all(blocks.map(async (codeEl) => {
    const language = resolveLanguage(codeEl)
    const code = codeEl.textContent || ''
    if (!language) return
    const tokenHtml = await fetchTokenHtml(code, language)
    if (tokenHtml) codeEl.innerHTML = tokenHtml
  }))

  return template.innerHTML
}

const collectBlocks = (root) => {
  if (!root?.querySelectorAll) return []
  return Array.from(root.querySelectorAll('pre > code[class*="language-"]'))
}

/**
 * 启动预览代码块的二次上色。幂等，重复调用只生效一次。
 *
 * 用 IntersectionObserver 而不是「渲染完立刻整篇请求」：
 * 长文档里大部分代码块在视口外，等滚到附近再请求，避免一次性几十个 IPC。
 */
export const startPreviewTokenHighlight = () => {
  if (started || typeof window === 'undefined' || typeof document === 'undefined') return
  started = true

  const observer = typeof IntersectionObserver === 'function'
    ? new IntersectionObserver((entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue
        observer.unobserve(entry.target)
        void upgradeBlock(entry.target)
      }
    }, { rootMargin: '320px 0px' })
    : null

  const enqueue = (codeEl) => {
    if (!isCandidate(codeEl)) return
    if (observer) observer.observe(codeEl)
    else void upgradeBlock(codeEl)
  }

  collectBlocks(document.body).forEach(enqueue)

  const mutationObserver = new MutationObserver((records) => {
    for (const record of records) {
      for (const node of record.addedNodes) {
        if (node.nodeType !== 1) continue
        if (node.matches?.('pre > code[class*="language-"]')) {
          enqueue(node)
          continue
        }
        collectBlocks(node).forEach(enqueue)
      }
    }
  })
  mutationObserver.observe(document.body, { childList: true, subtree: true })
}
