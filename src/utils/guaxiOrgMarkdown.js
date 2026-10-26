// 任务条目正文的 Markdown 渲染器（CONTENT / NOTES / 根本原因 / 自定义字段）。
//
// 为什么不复用 guaxiMiniMarkdown：
//   那是资源库文件预览用的最小子集（代码块 / 分隔线 / 图片 / 行内代码），
//   任务条目需要标题、列表、表格、引用、加粗等完整语法，而它会把 `<br>` 直接转义成字面文本。
//
// 为什么不复用 GuaxiMarker 的 renderMarkdown：
//   那条管线是异步的，还要拉起 MathJax / mermaid / 代码高亮 / 图表内嵌并依赖后端；
//   这里的调用点是一张张卡片、以及搜索时每次键入的重渲染，代价太大。
//   这里只做「纯文本 → HTML」的同步渲染，产物交给组件自己的样式。
import MarkdownIt from 'markdown-it'

// `<br>` 系列写法：用户就是这么表达换行的，CM 里它属于原始 HTML，会被 html:false 转义。
const BR_TAG_RE = /<br\s*\/?>/i
const BR_TAG_SPLIT_RE = /<br\s*\/?>/gi

const escapeHtml = (s) =>
  String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')

// 本地图片路径转成可显示的 src（assets 协议或原样），与编辑器预览保持一致。
const resolveAssetSrc = (url, convertSrc) => {
  const u = String(url || '').trim()
  if (!u) return ''
  if (/^(https?:|data:|asset:|blob:)/i.test(u)) return u
  if (typeof convertSrc === 'function') {
    try { return convertSrc(u) } catch { return u }
  }
  return u
}

const renderer = new MarkdownIt({
  // 保留默认转义：任务条目里常粘着 shell 输出，放行原始 HTML 会把 `<...>` 当成标签吞掉。
  html: false,
  xhtmlOut: true,
  // 单换行即换行：条目是随手记的，不该要求空一行才分段。
  breaks: true,
  linkify: true,
  // 日志里的 `--flag`、引号不能被排版规则改写。
  typographer: false,
})

// 把行内文本里的 `<br>` 变成真正的换行。
// 只处理 text token：代码块是块级 token，行内代码是 code_inline，都不会被误伤。
renderer.core.ruler.after('inline', 'org_br_hardbreak', (state) => {
  for (const token of state.tokens) {
    if (token.type !== 'inline' || !Array.isArray(token.children)) continue
    if (!token.children.some((child) => child.type === 'text' && BR_TAG_RE.test(child.content))) continue

    const next = []
    for (const child of token.children) {
      if (child.type !== 'text' || !BR_TAG_RE.test(child.content)) {
        next.push(child)
        continue
      }
      String(child.content).split(BR_TAG_SPLIT_RE).forEach((part, index) => {
        if (index > 0) {
          const br = new state.Token('hardbreak', 'br', 0)
          br.content = ''
          next.push(br)
        }
        if (part) {
          const text = new state.Token('text', '', 0)
          text.content = part
          next.push(text)
        }
      })
    }
    token.children = next
  }
})

const escapeRegExp = (raw) => String(raw ?? '').replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

// 搜索命中高亮。
// 放在 markdown 解析之后、渲染之前：文字仍然是 text token，插入的 <mark> 不会被转义，
// 因此「换成完整 Markdown 渲染」不会让搜索栏里已经工作的命中高亮失效。
renderer.core.ruler.after('org_br_hardbreak', 'org_search_mark', (state) => {
  const query = String(state.env?.query || '').trim()
  if (!query) return
  const re = new RegExp(escapeRegExp(query), 'gi')

  const textToken = (content) => {
    const token = new state.Token('text', '', 0)
    token.content = content
    return token
  }

  for (const token of state.tokens) {
    if (token.type !== 'inline' || !Array.isArray(token.children)) continue
    if (!token.children.some((child) => child.type === 'text')) continue

    const next = []
    for (const child of token.children) {
      if (child.type !== 'text') {
        next.push(child)
        continue
      }
      const value = String(child.content || '')
      re.lastIndex = 0
      let last = 0
      let hit = false
      let matched = re.exec(value)
      while (matched) {
        const piece = String(matched[0] || '')
        const index = Number(matched.index || 0)
        // 零长度匹配会原地打转，手动推进。
        if (!piece) {
          re.lastIndex = index + 1
          matched = re.lastIndex <= value.length ? re.exec(value) : null
          continue
        }
        hit = true
        if (index > last) next.push(textToken(value.slice(last, index)))
        const mark = new state.Token('html_inline', '', 0)
        mark.content = `<mark class="otp-hit">${escapeHtml(piece)}</mark>`
        next.push(mark)
        last = index + piece.length
        matched = re.exec(value)
      }
      if (!hit) {
        next.push(child)
        continue
      }
      if (last < value.length) next.push(textToken(value.slice(last)))
    }
    token.children = next
  }
})

const defaultImageRenderer = renderer.renderer.rules.image
renderer.renderer.rules.image = (tokens, idx, options, env, self) => {
  const token = tokens[idx]
  const rawSrc = token.attrGet('src')
  if (rawSrc) {
    const resolved = resolveAssetSrc(rawSrc, env && env.convertSrc)
    if (resolved) token.attrSet('src', resolved)
  }
  token.attrSet('class', 'otp-md-img')
  token.attrSet('loading', 'lazy')
  return defaultImageRenderer(tokens, idx, options, env, self)
}

const defaultLinkOpenRenderer =
  renderer.renderer.rules.link_open ||
  ((tokens, idx, options, env, self) => self.renderToken(tokens, idx, options))
renderer.renderer.rules.link_open = (tokens, idx, options, env, self) => {
  const token = tokens[idx]
  token.attrSet('class', 'otp-md-link')
  const href = token.attrGet('href') || ''
  if (!/^(#|mailto:)/i.test(href)) {
    token.attrSet('target', '_blank')
    token.attrSet('rel', 'noreferrer')
  }
  return defaultLinkOpenRenderer(tokens, idx, options, env, self)
}

/**
 * 渲染任务条目正文为 HTML。
 * @param {string} src
 * @param {(path: string) => string} [convertSrc] 本地图片路径转换函数（如 tauri convertFileSrc）
 * @param {{ query?: string }} [options] query 非空时把命中包进 `<mark class="otp-hit">`
 */
export function renderOrgMarkdown(src, convertSrc, options = {}) {
  const text = String(src || '')
  if (!text.trim()) return ''
  return renderer.render(text, { convertSrc, query: options.query || '' })
}

export { escapeHtml as escapeOrgHtml }
