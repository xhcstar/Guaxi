import MarkdownIt from 'markdown-it'
import markdownItMathjax from 'markdown-it-mathjax3'
import hljs from 'highlight.js'
import { allowFileProtocolLinks } from '../utils/markdownFileLink'
// MathJax 3.2.2 的解析崩溃补丁：必须在建出 TeX 输入 jax 之前生效，本模块求值时即安装。
import '../components/GuaxiMarkdownRender/mathjaxNullSafeCharacter'

const WORKER_LOG_PREFIX = '[NotebookMarkdownWorker]'

const logInfo = (message, payload = undefined) => {
  if (payload === undefined) {
    console.info(WORKER_LOG_PREFIX, message)
    return
  }
  console.info(WORKER_LOG_PREFIX, message, payload)
}

const logWarn = (message, payload = undefined) => {
  if (payload === undefined) {
    console.warn(WORKER_LOG_PREFIX, message)
    return
  }
  console.warn(WORKER_LOG_PREFIX, message, payload)
}

const escapeRegExp = (text) => String(text || '').replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

const safeEncode = (content) => {
  if (!content) return ''
  return encodeURIComponent(String(content))
    .replace(/'/g, '%27')
    .replace(/"/g, '%22')
    .replace(/</g, '%3C')
    .replace(/>/g, '%3E')
    .replace(/&/g, '%26')
    .replace(/\//g, '%2F')
    .replace(/\\/g, '%5C')
}

const renderPlainFenceBlock = (code, md) => {
  const escaped = md.utils.escapeHtml(code)
  return `<pre class="hljs hljs-plain"><code class="language-plaintext">${escaped}</code></pre>`
}

// ───────── MathJax 结果记忆化 ─────────
// 与 GuaxiMarker.js 同一套做法（那边有详细说明）：markdown-it-mathjax3 每次渲染都会把
// 每个公式现场编译成 SVG，单条约 0.94ms；一篇 240 公式的笔记一次渲染就有约 250ms 花在这里。
// 公式 SVG 只由 LaTeX 源码决定，按源码记忆化后，编辑时未改动的公式全部命中，
// 预览重渲染从数百毫秒降到个位数毫秒。命中时返回的就是未缓存时本会返回的同一个字符串。
const MATH_RENDER_CACHE_LIMIT = 4000
const mathRenderCache = new Map()

const escapeMathText = (text) =>
  String(text ?? '').replace(/[&<>"']/g, (c) => (
    { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]
  ))

// 单条公式失败不能拖垮整篇（详见 mathjaxNullSafeCharacter.js）。
const mathRenderFailureFallback = (kind, latex) => (
  kind === 'block'
    ? `<div class="math-block math-error">${escapeMathText(latex)}</div>`
    : `<span class="math-inline math-error">${escapeMathText(latex)}</span>`
)

const memoizeMathRender = (kind, latex, produce) => {
  const key = `${kind}\u0000${latex}`
  const cached = mathRenderCache.get(key)
  if (cached !== undefined) return cached

  let value
  try {
    value = produce()
  } catch (error) {
    // 不写缓存：补丁生效或输入变化后还应能重新尝试。
    logWarn('single formula render failed, falling back to raw LaTeX', String(error?.message || error))
    return mathRenderFailureFallback(kind, latex)
  }

  if (mathRenderCache.size >= MATH_RENDER_CACHE_LIMIT) {
    const oldest = mathRenderCache.keys().next().value
    if (oldest !== undefined) mathRenderCache.delete(oldest)
  }
  mathRenderCache.set(key, value)
  return value
}

// 后端 /api/markdown/image_process 只匹配 ![](相对路径) 这种**图片**语法（见
// markdown_image_process.rs 的 process_image_paths）。正文里没有图片时它必然原样返回，
// 却仍要付一次 HTTP 往返；预览与摘录列表每次渲染都要走这一趟，所以直接跳过。
// 返回值与真实响应完全一致（processed_content === content、mappings 为空）。
const IMAGE_MARKDOWN_RE = /!\[[^\]]*\]\([^)\s]+/

const preprocessMarkdownContent = async (content, options = {}) => {
  const chatPreprocess = !!options.chatPreprocess
  let finalContent = String(content || '')

  // No image syntax → the endpoint is a no-op. Skip the round trip.
  if (!IMAGE_MARKDOWN_RE.test(finalContent)) {
    return finalContent
  }

  try {
    const preprocessApi = chatPreprocess
      ? 'http://localhost:8888/api/markdown/chat_process'
      : 'http://localhost:8888/api/markdown/image_process'
    const response = await fetch(preprocessApi, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        content,
        base_path: options.basePath,
      }),
    })

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`)
    }

    const result = await response.json()
    finalContent = String(result?.processed_content || '')
    const mappings = Array.isArray(result?.mappings) ? result.mappings : []
    for (const mapping of mappings) {
      if (!mapping?.original_path || !mapping?.served_path) continue
      finalContent = finalContent.replace(
        new RegExp(`\\]\\(${escapeRegExp(mapping.original_path)}\\)`, 'g'),
        `](${mapping.served_path})`,
      )
    }
  } catch (error) {
    logWarn('preprocess failed, fallback to raw markdown', String(error?.message || error))
  }

  return finalContent
}

const installTaskListSupport = (md) => {
  const taskPrefixRe = /^\s*\[( |x|X)\]\s+/

  const addTokenClass = (token, cls) => {
    if (!token) return
    const prev = token.attrGet('class') || ''
    const parts = String(prev).split(/\s+/).filter(Boolean)
    if (!parts.includes(cls)) parts.push(cls)
    token.attrSet('class', parts.join(' '))
  }

  const stripPrefixFromInlineChildren = (inlineToken, prefixLen) => {
    if (!inlineToken || !Array.isArray(inlineToken.children) || prefixLen <= 0) return
    let remain = prefixLen
    for (let i = 0; i < inlineToken.children.length && remain > 0;) {
      const child = inlineToken.children[i]
      if (!child || child.type !== 'text') break
      const text = String(child.content || '')
      if (!text) {
        i += 1
        continue
      }
      if (text.length <= remain) {
        remain -= text.length
        inlineToken.children.splice(i, 1)
        continue
      }
      child.content = text.slice(remain)
      remain = 0
      break
    }
  }

  md.core.ruler.after('inline', 'guaxi_task_list_worker', (state) => {
    const tokens = state.tokens || []
    let hit = 0

    for (let i = 0; i < tokens.length; i += 1) {
      const token = tokens[i]
      if (!token || token.type !== 'list_item_open') continue

      const startLine = Array.isArray(token.map) ? token.map[0] + 1 : 0
      let inline = null
      for (let j = i + 1; j < tokens.length && tokens[j].type !== 'list_item_close'; j += 1) {
        if (tokens[j].type === 'inline') {
          inline = tokens[j]
          break
        }
      }
      if (!inline) continue

      const match = taskPrefixRe.exec(String(inline.content || ''))
      if (!match) continue

      const checked = String(match[1] || '').toLowerCase() === 'x'
      const prefix = String(match[0] || '')
      inline.content = String(inline.content || '').slice(prefix.length)
      stripPrefixFromInlineChildren(inline, prefix.length)

      addTokenClass(token, 'task-list-item')
      if (checked) addTokenClass(token, 'task-list-item-checked')
      if (startLine) token.attrSet('data-md-task-line', String(startLine))
      token.attrSet('data-md-task', checked ? '1' : '0')

      const makeHtmlToken = (html) => {
        const htmlToken = new state.Token('html_inline', '', 0)
        htmlToken.content = html
        return htmlToken
      }

      const checkboxHtml = `<input class="task-list-item-checkbox" type="checkbox"${checked ? ' checked' : ''}${startLine ? ` data-md-task-line="${startLine}"` : ''} /> `
      const children = Array.isArray(inline.children) ? inline.children : []
      inline.children = [
        makeHtmlToken(checkboxHtml),
        makeHtmlToken('<span class="task-list-item-text">'),
        ...children,
        makeHtmlToken('</span>'),
      ]
      hit += 1
    }

    if (hit) {
      logInfo('task list parsed', { items: hit })
    }

    return false
  })
}

const installTocSupport = (md) => {
  const tocMarkerRe = /^\s*(?:\[\[\s*(?:toc|\u76ee\u5f55)\s*\]\]|\[\s*(?:toc|\u76ee\u5f55)\s*\])\s*$/i

  const slugify = (text) => {
    const raw0 = String(text || '').trim().toLowerCase()
    const raw = raw0.replace(/(\d)[\.\uFF0E](\d)/g, '$1$2')
    const normalized = raw.normalize ? raw.normalize('NFKD') : raw
    const cleaned = normalized
      .replace(/[^\p{L}\p{N}\s-]/gu, ' ')
      .replace(/\s+/g, ' ')
      .trim()
    const slug = cleaned.replace(/\s+/g, '-').replace(/-+/g, '-')
    return slug || 'section'
  }

  md.core.ruler.after('inline', 'guaxi_toc_worker', (state) => {
    const tokens = state.tokens || []
    const slugCount = new Map()
    const headings = []

    for (let i = 0; i < tokens.length; i += 1) {
      const token = tokens[i]
      if (!token || token.type !== 'heading_open') continue
      const next = tokens[i + 1]
      if (!next || next.type !== 'inline') continue

      const level = Number(String(token.tag || 'h1').slice(1)) || 1
      const text = String(next.content || '').trim()
      if (!text) continue

      const base = slugify(text)
      const count = (slugCount.get(base) || 0) + 1
      slugCount.set(base, count)
      const id = count > 1 ? `${base}-${count}` : base
      token.meta = { ...(token.meta || {}), guaxiHeadingId: id }

      const startLine = Array.isArray(token.map) ? token.map[0] + 1 : 0
      headings.push({ level, text, id, startLine })
    }

    if (!headings.length) return false

    const minLevel = Math.min(...headings.map((item) => item.level))
    const buildTocHtml = () => {
      let html = '<nav class="md-toc" data-md-toc="1"><ul class="md-toc-list">'
      for (const heading of headings) {
        const lvl = Math.max(1, (heading.level - minLevel) + 1)
        html += `<li class="md-toc-item md-toc-level-${lvl}"><a class="md-toc-link" href="#${encodeURIComponent(String(heading.id))}"${heading.startLine ? ` data-md-start-line="${heading.startLine}"` : ''}>${md.utils.escapeHtml(String(heading.text))}</a></li>`
      }
      html += '</ul></nav>'
      return html
    }

    let injected = 0
    for (let i = 0; i < tokens.length - 2; i += 1) {
      const t0 = tokens[i]
      const t1 = tokens[i + 1]
      const t2 = tokens[i + 2]
      if (!t0 || !t1 || !t2) continue
      if (t0.type !== 'paragraph_open' || t1.type !== 'inline' || t2.type !== 'paragraph_close') continue
      if (!tocMarkerRe.test(String(t1.content || ''))) continue

      const htmlToken = new state.Token('html_block', '', 0)
      htmlToken.content = buildTocHtml()
      htmlToken.map = t0.map
      tokens.splice(i, 3, htmlToken)
      injected += 1
    }

    if (injected) {
      logInfo('toc generated', { items: headings.length, markers: injected })
    }

    return false
  })
}

const buildWorkerMarkdownRenderer = (sourceContent, options = {}) => {
  const {
    preserveOriginal = false,
    mermaid: enableMermaid = true,
    math: enableMath = true,
    codeHighlight: enableCodeHighlight = true,
    pythonCells: enablePythonCells = false,
    codeCells: enableCodeCells = false,
  } = options

  const md = new MarkdownIt({
    html: true,
    linkify: true,
    typographer: true,
    breaks: false,
    xhtmlOut: true,
    highlight: enableCodeHighlight
      ? (code, lang) => {
        if (lang && hljs.getLanguage(lang)) {
          try {
            return `<pre class="hljs"><code class="language-${lang}">${hljs.highlight(code, { language: lang }).value}</code></pre>`
          } catch (error) {
            logWarn(`highlight failed (${lang})`, String(error?.message || error))
          }
        }
        // Keep unlabeled fenced code blocks on the same structural path as highlighted blocks.
        // That preserves the shared copy-button enhancement and lets dark mode reuse the same code-block shell.
        return renderPlainFenceBlock(code, md)
      }
      : undefined,
  })

  // 放行 [label](file:///...) 这类本地文件链接（markdown-it 默认把 file: 拦掉）。
  allowFileProtocolLinks(md)

  const defaultParagraphClose = md.renderer.rules.paragraph_close || (() => '</p>')
  const defaultHeadingClose = md.renderer.rules.heading_close || ((tokens, idx) => `</${tokens[idx].tag}>`)

  // preserveOriginal 要按源码行号回取原始行。原先这行 split 写在规则体里，等于**每渲染
  // 一个段落就把整篇文档切一次**（段落数 × 文档长度的平方级开销）。整篇只切一次即可。
  const sourceLines = preserveOriginal ? sourceContent.split(/\r?\n|\r/) : null

  md.renderer.rules.paragraph_open = (tokens, idx) => {
    const token = tokens[idx]
    const attrs = ['class="md-paragraph"']
    if (token && Array.isArray(token.map)) {
      attrs.push(`data-md-start-line="${token.map[0] + 1}"`)
      attrs.push(`data-md-end-line="${token.map[1] + 1}"`)
      if (sourceLines) {
        const original = sourceLines[token.map[0]] || ''
        attrs.push(`data-md-original="${safeEncode(original)}"`)
      }
    }
    return `<p ${attrs.join(' ')}>`
  }

  md.renderer.rules.paragraph_close = (tokens, idx) => defaultParagraphClose(tokens, idx)

  md.renderer.rules.heading_open = (tokens, idx) => {
    const token = tokens[idx]
    const tag = token.tag || 'h1'
    const attrs = [`class="md-heading md-heading-${tag}"`]
    const headingId = token?.meta?.guaxiHeadingId
    if (headingId) {
      attrs.push(`id="${md.utils.escapeHtml(String(headingId))}"`)
    }
    if (token && Array.isArray(token.map)) {
      attrs.push(`data-md-start-line="${token.map[0] + 1}"`)
      attrs.push(`data-md-end-line="${token.map[1] + 1}"`)
    }
    return `<${tag} ${attrs.join(' ')}>`
  }

  md.renderer.rules.heading_close = (tokens, idx, renderOptions, env, self) => {
    return defaultHeadingClose(tokens, idx, renderOptions, env, self)
  }

  installTaskListSupport(md)
  installTocSupport(md)

  if (enableMermaid || enablePythonCells || enableCodeCells) {
    const defaultFence = md.renderer.rules.fence
    md.renderer.rules.fence = (tokens, idx, renderOptions, env, self) => {
      const token = tokens[idx]
      const info = String(token?.info || '').trim()

      if (enableMermaid && info === 'mermaid') {
        const content = String(token.content || '')
          .replace(/&gt;/g, '>')
          .replace(/&lt;/g, '<')
          .replace(/&amp;/g, '&')
        if (preserveOriginal) {
          const original = `\`\`\`mermaid\n${content}\n\`\`\``
          return `<pre class="mermaid" data-md-original="${safeEncode(original)}">${content}</pre>`
        }
        return `<pre class="mermaid">${content}</pre>`
      }

      return defaultFence ? defaultFence(tokens, idx, renderOptions, env, self) : self.renderToken(tokens, idx, renderOptions)
    }
  }

  if (enableMath) {
    md.use(markdownItMathjax)
    const originalInlineMath = md.renderer.rules.math_inline
    const originalBlockMath = md.renderer.rules.math_block

    // 按 LaTeX 源码记忆化：公式 SVG 只由源码决定，编辑时未改动的公式直接命中缓存。
    md.renderer.rules.math_inline = (tokens, idx, renderOptions, env, self) => {
      const token = tokens[idx]
      return memoizeMathRender('inline', token.content, () => {
        const originalLatex = `$${token.content}$`
        const mathContent = originalInlineMath
          ? originalInlineMath(tokens, idx, renderOptions, env, self)
          : `<span class="math-inline">${token.content}</span>`
        return `<span class="math-inline-container" data-md-original="${safeEncode(originalLatex)}">${mathContent}</span>`
      })
    }

    md.renderer.rules.math_block = (tokens, idx, renderOptions, env, self) => {
      const token = tokens[idx]
      return memoizeMathRender('block', token.content, () => {
        const originalLatex = `$$${token.content}$$`
        const mathContent = originalBlockMath
          ? originalBlockMath(tokens, idx, renderOptions, env, self)
          : `<div class="math-block">${token.content}</div>`
        return `<div class="math-block-container" data-md-original="${safeEncode(originalLatex)}">${mathContent}</div>`
      })
    }
  }

  return md
}

const renderMarkdownInWorker = async (content, options = {}) => {
  const sourceContent = String(content || '')
  const finalContent = await preprocessMarkdownContent(sourceContent, options)
  const md = buildWorkerMarkdownRenderer(sourceContent, options)
  return md.render(finalContent)
}

self.onmessage = async (event) => {
  const payload = event?.data || {}
  const id = payload?.id
  try {
    const html = await renderMarkdownInWorker(String(payload?.content || ''), payload?.options || {})
    self.postMessage({ id, ok: true, html })
  } catch (error) {
    logWarn('render failed', String(error?.message || error))
    self.postMessage({
      id,
      ok: false,
      error: String(error?.message || error),
    })
  }
}
