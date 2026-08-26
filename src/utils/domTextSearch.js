/**
 * DOM 文本搜索 / 高亮内核。
 *
 * 设计说明：
 * 1. 预览（GuaxiEditor）和探索者（GuaxiNotebookView）原本各自实现了一份几乎相同的
 *    TreeWalker + 文本切分逻辑，行为却有细微差异——预览把 `<code>` 整体排除掉了，
 *    导致 `CMakeLists.txt` 这类行内代码在预览下搜不到，而探索者能搜到。
 *    统一到这里之后，两个模式的搜索能力天然一致。
 * 2. 本模块不依赖 Vue，只操作传入的 DOM 子树，方便被任何宿主复用与单独测试。
 * 3. 排除选择器只排除“非内容元素”（脚本、表单控件、Monaco 容器、查找控件自身），
 *    代码相关的 `code` / `pre` 一律参与搜索。
 */

export const SEARCH_HIT_CLASS = 'guaxi-search-hit'
export const SEARCH_HIT_ACTIVE_CLASS = 'guaxi-search-hit-active'

export const DEFAULT_SEARCH_EXCLUDE_SELECTOR = [
  'script',
  'style',
  'noscript',
  'textarea',
  'input',
  'select',
  'button',
  '.monaco-editor',
  '.guaxi-find-widget',
].join(', ')

export const escapeRegExp = (raw) => String(raw ?? '').replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

// 中文、日文等没有 \b 词边界的概念，用“前后不是字母/数字/下划线”的断言代替，
// 这样“全字匹配”对 CJK 文本也是可用的（\p{L} 需要 u 标志）。
const UNICODE_BOUNDARY_LEFT = '(?<![\\p{L}\\p{N}_])'
const UNICODE_BOUNDARY_RIGHT = '(?![\\p{L}\\p{N}_])'

/**
 * 依据关键词与选项构造全局正则。非法正则返回 null，调用方据此显示“无匹配”而不是抛错。
 */
export function buildSearchRegex(keyword, options = {}) {
  const raw = String(keyword ?? '')
  if (!raw) return null

  const { caseSensitive = false, wholeWord = false, useRegex = false } = options
  const body = useRegex ? raw : escapeRegExp(raw)

  // 逐个候选尝试：优先 Unicode 断言，环境不支持 \p{...} 时退回 \b。
  const attempts = wholeWord
    ? [
        [`${UNICODE_BOUNDARY_LEFT}(?:${body})${UNICODE_BOUNDARY_RIGHT}`, caseSensitive ? 'gu' : 'giu'],
        [`\\b(?:${body})\\b`, caseSensitive ? 'g' : 'gi'],
      ]
    : [[body, caseSensitive ? 'g' : 'gi']]

  for (const [source, flags] of attempts) {
    try {
      return new RegExp(source, flags)
    } catch {
      // 尝试下一种写法
    }
  }
  return null
}

/**
 * 把此前插入的高亮 span 还原成普通文本节点。返回还原的数量。
 */
export function clearSearchHighlights(root, hitClass = SEARCH_HIT_CLASS) {
  if (!(root instanceof Element)) return 0

  const marks = root.querySelectorAll(`span.${hitClass}`)
  let restored = 0
  marks.forEach((mark) => {
    const parent = mark.parentNode
    if (!parent) return
    parent.replaceChild(document.createTextNode(mark.textContent || ''), mark)
    // normalize 把相邻文本节点合回一个，避免反复搜索后文本节点被切得越来越碎。
    parent.normalize?.()
    restored += 1
  })
  return restored
}

const collectSearchableTextNodes = (root, regex, excludeSelector, hitClass) => {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const text = String(node?.nodeValue || '')
      if (!text) return NodeFilter.FILTER_REJECT
      const parent = node.parentElement
      if (!parent) return NodeFilter.FILTER_REJECT
      if (excludeSelector && parent.closest(excludeSelector)) return NodeFilter.FILTER_REJECT
      if (parent.closest(`.${hitClass}`)) return NodeFilter.FILTER_REJECT
      regex.lastIndex = 0
      return regex.test(text) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT
    },
  })

  const nodes = []
  let current = walker.nextNode()
  while (current) {
    nodes.push(current)
    current = walker.nextNode()
  }
  return nodes
}

/**
 * 在 root 子树里高亮所有命中，返回按文档顺序排列的高亮 span 数组。
 *
 * @param {Element} root 搜索范围
 * @param {object} params
 * @param {string} params.keyword 关键词（正则模式下就是正则源码）
 * @param {object} [params.options] { caseSensitive, wholeWord, useRegex }
 * @param {string} [params.hitClass] 高亮 span 的类名
 * @param {string} [params.extraExcludeSelector] 宿主追加的排除选择器（复制按钮、工具条等）
 */
export function applySearchHighlights(root, params = {}) {
  const {
    keyword = '',
    options = {},
    hitClass = SEARCH_HIT_CLASS,
    extraExcludeSelector = '',
  } = params

  if (!(root instanceof Element)) return []
  const regex = buildSearchRegex(keyword, options)
  if (!regex) return []

  const excludeSelector = [DEFAULT_SEARCH_EXCLUDE_SELECTOR, extraExcludeSelector]
    .filter(Boolean)
    .join(', ')

  const textNodes = collectSearchableTextNodes(root, regex, excludeSelector, hitClass)
  const matches = []

  textNodes.forEach((node) => {
    const text = String(node.nodeValue || '')
    const parent = node.parentNode
    if (!parent) return

    regex.lastIndex = 0
    let lastIndex = 0
    let matched = regex.exec(text)
    if (!matched) return

    const fragment = document.createDocumentFragment()
    let nodeHitCount = 0
    while (matched) {
      const index = Number(matched.index || 0)
      const matchedText = String(matched[0] || '')
      // 零长度匹配（例如正则模式下的 `a*`）会让 exec 原地打转，必须手动推进。
      if (!matchedText) {
        regex.lastIndex = index + 1
        matched = regex.lastIndex <= text.length ? regex.exec(text) : null
        continue
      }
      if (index > lastIndex) {
        fragment.appendChild(document.createTextNode(text.slice(lastIndex, index)))
      }
      const span = document.createElement('span')
      span.className = hitClass
      span.textContent = matchedText
      fragment.appendChild(span)
      matches.push(span)
      nodeHitCount += 1
      lastIndex = index + matchedText.length
      matched = regex.exec(text)
    }

    // 本节点一个命中都没有（只有零长度匹配）时保持原样，避免无谓地把文本节点切碎。
    if (!nodeHitCount) return
    if (lastIndex < text.length) {
      fragment.appendChild(document.createTextNode(text.slice(lastIndex)))
    }
    parent.replaceChild(fragment, node)
  })

  return matches
}

/**
 * 把 index 归一化到 [0, total)（支持环绕），并把激活类切到对应命中上。
 * 返回归一化后的下标；没有命中时返回 -1。
 */
export function setActiveSearchHit(matches, index, params = {}) {
  const {
    activeClass = SEARCH_HIT_ACTIVE_CLASS,
    scroll = true,
    scrollBehavior = 'smooth',
  } = params

  const list = Array.isArray(matches) ? matches.filter(Boolean) : []
  const total = list.length
  if (!total) return -1

  let next = Number(index)
  if (!Number.isFinite(next)) next = 0
  next = ((next % total) + total) % total

  list.forEach((el) => el?.classList?.remove(activeClass))
  const active = list[next]
  if (active) {
    active.classList.add(activeClass)
    if (scroll) {
      active.scrollIntoView({ block: 'center', inline: 'nearest', behavior: scrollBehavior })
    }
  }
  return next
}
