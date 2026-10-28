/**
 * 搜索关键词高亮。
 *
 * 列表与正文共用一份词表：与后端一致，按空白切分（多个词是 AND 关系）。
 *
 * 两处渲染方式不同：
 * - 列表行是模板渲染，用 `segments()` 把文本切成片段交给 `<mark>`；
 * - 正文是 `v-html` 进来的，不能整段重写（会破坏邮件的 DOM），
 *   只能在文本节点上就地插入 `<mark>`。
 */

/** 把搜索框里的内容切成词表（去重、长词优先，避免短词把长词切碎）。 */
export function searchTokens(query) {
  const seen = new Set()
  const out = []
  for (const raw of String(query || '').split(/\s+/)) {
    const token = raw.trim()
    if (!token) continue
    const key = token.toLowerCase()
    if (seen.has(key)) continue
    seen.add(key)
    out.push(token)
  }
  return out.sort((a, b) => b.length - a.length)
}

/**
 * 把一段文本切成「命中 / 未命中」的片段。
 *
 * 用逐字符打标再合并的方式处理重叠：`mail` 与 `ail` 同时命中时不会切出
 * 互相嵌套的片段（那会渲染成一层套一层的 <mark>）。
 */
export function segments(text, tokens) {
  const source = String(text ?? '')
  if (!source || !tokens?.length) return [{ text: source, hit: false }]
  const lower = source.toLowerCase()
  const marks = new Array(source.length).fill(false)
  for (const token of tokens) {
    const needle = String(token || '').toLowerCase()
    if (!needle) continue
    let index = lower.indexOf(needle)
    while (index >= 0) {
      for (let i = index; i < index + needle.length; i += 1) marks[i] = true
      index = lower.indexOf(needle, index + needle.length)
    }
  }
  const out = []
  let start = 0
  for (let i = 1; i <= source.length; i += 1) {
    if (i === source.length || marks[i] !== marks[start]) {
      out.push({ text: source.slice(start, i), hit: marks[start] })
      start = i
    }
  }
  return out.length ? out : [{ text: source, hit: false }]
}

const SKIP_TAGS = new Set(['SCRIPT', 'STYLE', 'TEXTAREA', 'MARK', 'SVG'])

/** 摘掉上一次的高亮标记（还原成纯文本节点），保证重复执行结果稳定。 */
export function clearHighlight(root) {
  if (!root?.querySelectorAll) return
  for (const mark of root.querySelectorAll('mark.mail-hl')) {
    const parent = mark.parentNode
    if (!parent) continue
    parent.replaceChild(document.createTextNode(mark.textContent || ''), mark)
    parent.normalize()
  }
}

/** 正文里就地高亮：正文 DOM 归浏览器 / 邮件自己管，这里只包文本节点。 */
export function highlightHtml(root, tokens) {
  if (!root?.querySelectorAll) return
  clearHighlight(root)
  if (!tokens?.length) return
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      if (!node.nodeValue || !node.nodeValue.trim()) return NodeFilter.FILTER_REJECT
      const parent = node.parentElement
      if (!parent || SKIP_TAGS.has(parent.tagName)) return NodeFilter.FILTER_REJECT
      return NodeFilter.FILTER_ACCEPT
    },
  })
  const targets = []
  for (let node = walker.nextNode(); node; node = walker.nextNode()) targets.push(node)
  for (const node of targets) {
    const parts = segments(node.nodeValue, tokens)
    if (!parts.some((part) => part.hit)) continue
    const fragment = document.createDocumentFragment()
    for (const part of parts) {
      if (!part.hit) {
        fragment.appendChild(document.createTextNode(part.text))
        continue
      }
      const mark = document.createElement('mark')
      mark.className = 'mail-hl'
      mark.textContent = part.text
      fragment.appendChild(mark)
    }
    node.parentNode?.replaceChild(fragment, node)
  }
}
