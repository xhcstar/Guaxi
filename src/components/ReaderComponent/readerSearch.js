// 阅读器正文搜索：规范化 + 位置映射 + 跨节点命中。
//
// 【为什么单独一个模块】
// 电子书 / 文本 / Markdown 三个阅读器此前各写了一份「遍历文本节点 + indexOf + 包 span」，
// 三份都踩同一个坑：匹配在 `textContent.toLowerCase()` 上做，切片却回到原始文本，
// 于是全角标点、不换行空格、软连字符一出现，偏移就对不上 —— 表现为
// 「搜的是这个词，高亮落在旁边几个字上」。三份各修一遍不如共用一份。
//
// 【为什么不直接在整段 textContent 上搜】
// 一句话可能被 <em>/<a>/<strong> 切开（电子书里极常见）。只在单个文本节点内搜，
// 「拜占庭」被标成 <em>拜占庭</em> 时搜不到；能搜到时，命中又落不到正确节点。
// 这里先把容器内的文本拼成一串（记录每段在串里的起点），在整串上找，再把命中
// 映射回「节点 + 节点内区间」，跨节点的命中会拆成多段分别包。

/** 零宽 / 软连字符：删掉，不占位。 */
const ZERO_WIDTH = new Set([0x00ad, 0x200b, 0x200c, 0x200d, 0x2060, 0xfeff])

const isSpaceCode = (code) => (
  code === 0x20 || code === 0x09 || code === 0x0a || code === 0x0d ||
  code === 0x00a0 || code === 0x3000 || (code >= 0x2000 && code <= 0x200a)
)

/**
 * 规范化 + 位置映射。
 *
 * @param {string} raw
 * @returns {{ text: string, map: number[] }}
 *   text 是用于匹配的串；map[i] 是 text[i] 在 raw 中的下标。
 *
 * 规则（与 DocEbookReader 里那份保持完全一致）：
 *   1. 零宽字符删除；
 *   2. 连续空白折成一个半角空格；
 *   3. 全角 ASCII（！-～）折半角（一一映射，长度不变）；
 *   4. 逐字符小写（个别字符小写后长度会变，这类保持原样，保证 1:1）。
 */
export function normalizeWithMap(raw) {
  const src = String(raw || '')
  let text = ''
  const map = []
  let i = 0
  while (i < src.length) {
    const code = src.charCodeAt(i)
    if (ZERO_WIDTH.has(code)) {
      i += 1
      continue
    }
    if (isSpaceCode(code)) {
      text += ' '
      map.push(i)
      i += 1
      while (i < src.length && isSpaceCode(src.charCodeAt(i))) i += 1
      continue
    }
    let out = src[i]
    if (code >= 0xff01 && code <= 0xff5e) out = String.fromCharCode(code - 0xfee0)
    const lower = out.toLowerCase()
    if (lower.length === 1) out = lower
    text += out
    map.push(i)
    i += 1
  }
  return { text, map }
}

/** 收集容器内的正文文本节点（跳过脚本、样式与已高亮的标记）。 */
export function collectTextNodes(root) {
  if (!root) return []
  const out = []
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      if (!node.nodeValue) return NodeFilter.FILTER_REJECT
      const parent = node.parentElement
      if (!parent) return NodeFilter.FILTER_REJECT
      if (parent.closest('script, style, noscript')) return NodeFilter.FILTER_REJECT
      return NodeFilter.FILTER_ACCEPT
    },
  })
  let node = walker.nextNode()
  while (node) {
    out.push(node)
    node = walker.nextNode()
  }
  return out
}

/**
 * 在容器里找全部命中。
 *
 * @param {HTMLElement} root
 * @param {string} query
 * @param {{limit?: number}} [opts]
 * @returns {Array<Array<{node: Text, start: number, end: number}>>}
 *   每个命中是一组「节点内区间」；跨节点的命中会有多段。
 */
export function findMatchesInDom(root, query, { limit = 2000 } = {}) {
  const nodes = collectTextNodes(root)
  if (!nodes.length) return []
  const starts = []
  let raw = ''
  for (const node of nodes) {
    starts.push(raw.length)
    raw += node.nodeValue || ''
  }
  const normalized = normalizeWithMap(raw)
  const needle = normalizeWithMap(query).text
  if (!needle) return []

  const matches = []
  let from = 0
  while (matches.length < limit) {
    const at = normalized.text.indexOf(needle, from)
    if (at < 0) break
    const startRaw = normalized.map[at]
    const endRaw = normalized.map[at + needle.length - 1] + 1
    from = at + Math.max(1, needle.length)

    const parts = []
    for (let i = 0; i < nodes.length; i += 1) {
      const nodeStart = starts[i]
      const nodeEnd = nodeStart + (nodes[i].nodeValue || '').length
      const s = Math.max(startRaw, nodeStart)
      const e = Math.min(endRaw, nodeEnd)
      if (e <= s) continue
      parts.push({ node: nodes[i], start: s - nodeStart, end: e - nodeStart })
    }
    if (parts.length) matches.push(parts)
  }
  return matches
}

/**
 * 把命中包成 `<span class="...">`，返回所有生成的元素（按文档顺序）。
 *
 * 一个命中可能拆成多段：每段单独包，视觉上仍是连续的高亮。
 * 从后往前处理，避免前面的包裹改变后面节点的位置。
 */
export function wrapMatches(matches, className) {
  const created = []
  for (const parts of matches) {
    // 倒序：同一节点里可能有多个区间，后面的先包，前面的边界不受影响。
    for (let k = parts.length - 1; k >= 0; k -= 1) {
      const { node, start, end } = parts[k]
      const raw = node.nodeValue || ''
      if (start < 0 || end > raw.length || end <= start) continue
      try {
        const doc = node.ownerDocument || document
        const range = doc.createRange()
        range.setStart(node, start)
        range.setEnd(node, end)
        const span = doc.createElement('span')
        span.className = className
        range.surroundContents(span)
        created.push(span)
      } catch {
        /* 节点在包裹过程中被别的高亮改写时跳过；宁可少一个，不要整段搜索失败 */
      }
    }
  }
  // surroundContents 是倒序执行的，返回时按文档顺序排好，供「当前命中」按序号跳转。
  created.sort((a, b) => {
    if (a === b) return 0
    // eslint-disable-next-line no-bitwise
    return (a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING) ? -1 : 1
  })
  return created
}

/** 拆掉之前包的高亮（把 span 换成它的子节点），恢复原始 DOM。 */
export function unwrapMatches(root, className) {
  if (!root) return
  root.querySelectorAll(`span.${className}`).forEach((el) => {
    const parent = el.parentNode
    if (!parent) return
    while (el.firstChild) parent.insertBefore(el.firstChild, el)
    parent.removeChild(el)
    parent.normalize?.()
  })
}
