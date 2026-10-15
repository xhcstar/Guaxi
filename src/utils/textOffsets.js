import { createLogger } from './logger'

const logger = createLogger('textOffsets')

/**
 * 把 DOM 容器里的文本拍平成「一个字符串 + 每个文本节点的区间」。
 *
 * 为什么不用 innerText/textContent 一把梭：
 * - innerText 按渲染结果插入换行、还会跳过 display:none 的内容，偏移会随样式漂移；
 * - 这里要的是一份**和后端切分用的完全一致**的字符串，所以按 DOM 顺序自己拼接。
 *
 * 计数单位必须是 UTF-16 码元（JS 字符串的 length 就是它）——后端也按码元给偏移，
 * 用字符数或字节数都会在中文上错位。
 *
 * @param {Node} root
 * @returns {{ text: string, nodes: Array<{ node: Text, start: number, end: number }> }}
 */
export function buildTextIndex(root) {
  if (!root || typeof document === 'undefined') return { text: '', nodes: [] }
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT)
  const nodes = []
  let text = ''
  let node = walker.nextNode()
  while (node) {
    const value = node.data || ''
    if (value) {
      nodes.push({ node, start: text.length, end: text.length + value.length })
      text += value
    }
    node = walker.nextNode()
  }
  return { text, nodes }
}

/** 找出包含该偏移的文本节点（取最后一个 start <= offset 的），偏移等于全文长度时落在末节点上。 */
function findNodeAt(nodes, offset) {
  let low = 0
  let high = nodes.length - 1
  let candidate = 0
  while (low <= high) {
    const mid = (low + high) >> 1
    if (nodes[mid].start <= offset) {
      candidate = mid
      low = mid + 1
    } else {
      high = mid - 1
    }
  }
  return nodes[candidate]
}

function clampLocal(value, node) {
  return Math.max(0, Math.min(value, node.data.length))
}

/**
 * 把 [startOffset, endOffset) 变成 DOM Range；拿不到就返回 null。
 *
 * 返回 null 的常见原因：内容在这之后重新渲染过（偏移对应的节点已经不在文档里），
 * 调用方应当跳过这一次高亮，而不是抛错中断播放。
 */
export function rangeFromOffsets(index, startOffset, endOffset) {
  const nodes = index?.nodes || []
  if (!nodes.length) return null
  const start = Math.max(0, Number(startOffset) || 0)
  const end = Math.max(start, Number(endOffset) || 0)
  const begin = findNodeAt(nodes, start)
  const finish = findNodeAt(nodes, Math.max(start, end - 1))
  if (!begin || !finish) return null
  try {
    // 用节点自己的 document 创建 Range：EPUB 的正文在 iframe 里，
    // 拿父文档的 document.createRange() 去设 iframe 内的节点会出问题。
    const owner = begin.node.ownerDocument || document
    const range = owner.createRange()
    range.setStart(begin.node, clampLocal(start - begin.start, begin.node))
    range.setEnd(finish.node, clampLocal(end - finish.start, finish.node))
    return range
  } catch (e) {
    logger.warn('构造 Range 失败', e)
    return null
  }
}

/**
 * 屏幕坐标 → 文本偏移。用于"点正文任意位置从那一句开始读"。
 *
 * 用 caret API 而不是包裹每个句子：正文是渲染出来的 HTML，往里面插节点会破坏
 * 链接/代码块/锚点，也会让每次高亮都触发重排。
 *
 * `doc` 要传内容所在的那个文档：EPUB 的正文在 iframe 里，用父文档查坐标会完全错位。
 */
export function offsetAtPoint(x, y, doc = null) {
  const target = doc || (typeof document !== 'undefined' ? document : null)
  if (!target) return null
  try {
    if (typeof target.caretRangeFromPoint === 'function') {
      const range = target.caretRangeFromPoint(x, y)
      if (range) return { node: range.startContainer, offset: range.startOffset }
    }
    if (typeof target.caretPositionFromPoint === 'function') {
      const position = target.caretPositionFromPoint(x, y)
      if (position) return { node: position.offsetNode, offset: position.offset }
    }
  } catch (e) {
    logger.warn('取光标位置失败', e)
  }
  return null
}

/** 文本节点 + 节点内偏移 → 全文偏移（元素节点退化为它包含的第一个文本节点）。 */
export function globalOffsetOf(index, node, offset) {
  const nodes = index?.nodes || []
  if (!node || !nodes.length) return null
  if (node.nodeType === Node.TEXT_NODE) {
    const entry = nodes.find((item) => item.node === node)
    return entry ? entry.start + Math.max(0, Number(offset) || 0) : null
  }
  const entry = nodes.find((item) => node.contains(item.node))
  return entry ? entry.start : null
}

/** 偏移落在第几段。段表按 start 升序，命中不了时返回 -1。 */
export function segmentIndexAtOffset(segments, offset) {
  const list = Array.isArray(segments) ? segments : []
  if (!list.length || offset === null || offset === undefined) return -1
  let low = 0
  let high = list.length - 1
  while (low <= high) {
    const mid = (low + high) >> 1
    const segment = list[mid]
    if (offset < segment.startOffset) high = mid - 1
    else if (offset >= segment.endOffset) low = mid + 1
    else return mid
  }
  // 落在段间空隙（段之间被跳过的空白）时，归到前一段，体感上更符合"点到哪读到哪"。
  return low > 0 ? low - 1 : 0
}

/**
 * 让某个 Range 出现在视口中央。容器内滚动，不带动整页。
 * @returns {boolean} 是否拿到了有效位置
 */
export function scrollRangeIntoView(range, container) {
  if (!range || !container) return false
  const rect = range.getBoundingClientRect()
  if (!rect || (rect.width === 0 && rect.height === 0)) return false
  const containerRect = container.getBoundingClientRect()
  const target = container.scrollTop + (rect.top - containerRect.top) - container.clientHeight / 2 + rect.height / 2
  const max = Math.max(0, container.scrollHeight - container.clientHeight)
  container.scrollTo({ top: Math.max(0, Math.min(target, max)), behavior: 'smooth' })
  return true
}
