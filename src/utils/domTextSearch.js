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

/**
 * 语法高亮会把一个标识符切成多个文本节点，例如 `circle_area` 变成
 * `<span class="hljs-title function_">circle_area</span>`、`Circle` 与 `c` 各占一个 span。
 *
 * 如果仍然「一个文本节点一个文本节点地搜」，命中数就取决于高亮在那一刻把文本切成了什么样子：
 * 同一个词在 `Circle` 节点里能命中，在 `circle_area` 节点里也能命中，但跨节点的
 * `cir` + `cle` 永远命中不了。而 tree-sitter 的二次上色是**异步**的、会在搜索之后
 * 重写代码块，于是同一个关键词在不同时刻会得到不同的命中数（用户实测 18 vs 2）。
 *
 * 这里把「一个可搜索容器」当作一条连续的文本来匹配：容器内所有可搜索文本节点拼接后
 * 只跑一次正则，再按偏移把命中映射回各个节点。代码块、段落、标题各自是一个容器，
 * 因此不会把两段无关的文字拼在一起命中。
 */
const SEARCH_BLOCK_SELECTOR = 'pre, p, li, h1, h2, h3, h4, h5, h6, td, th, dt, dd, blockquote, figcaption, summary'

/** 与 SEARCH_BLOCK_SELECTOR 对应的标签集合：判断「这个元素自身就是一个可搜索块」。 */
const SEARCH_BLOCK_TAGS = new Set(
  SEARCH_BLOCK_SELECTOR.split(',').map((s) => s.trim().toUpperCase()),
)
const isSearchBlockElement = (el) => !!el?.tagName && SEARCH_BLOCK_TAGS.has(String(el.tagName).toUpperCase())

/** 收集容器内可搜索的文本节点（跳过被排除的子树）。 */
const collectBlockTextNodes = (block, excludeSelector, hitClass) => {
  const nodes = []
  const walker = document.createTreeWalker(block, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const text = String(node?.nodeValue || '')
      if (!text) return NodeFilter.FILTER_REJECT
      const parent = node.parentElement
      if (!parent) return NodeFilter.FILTER_REJECT
      if (excludeSelector && parent.closest(excludeSelector)) return NodeFilter.FILTER_REJECT
      if (parent.closest(`.${hitClass}`)) return NodeFilter.FILTER_REJECT
      return NodeFilter.FILTER_ACCEPT
    },
  })
  let current = walker.nextNode()
  while (current) {
    nodes.push(current)
    current = walker.nextNode()
  }
  return nodes
}

/**
 * 找出 root 里所有「可搜索块」：最内层匹配 SEARCH_BLOCK_SELECTOR 的元素。
 * 没有块级结构的散落文本（例如裸文本节点）也要能被搜到，所以另外收集
 * 「不在任何块内的文本节点」并各自当作一个块。
 */
const collectSearchBlocks = (root, excludeSelector, hitClass) => {
  const innerMost = []
  const all = root.querySelectorAll(SEARCH_BLOCK_SELECTOR)
  for (const el of all) {
    // 只保留最内层：`<li><p>x</p></li>` 里交给 p，避免同一段文字被搜两遍。
    if (el.querySelector(SEARCH_BLOCK_SELECTOR)) continue
    if (excludeSelector && el.closest(excludeSelector)) continue
    if (el.closest(`.${hitClass}`)) continue
    const nodes = collectBlockTextNodes(el, excludeSelector, hitClass)
    if (nodes.length) innerMost.push({ el, nodes })
  }

  // 裸文本：不在任何可搜索块内的文本节点，按「最近的块级边界」归组。
  //
  // 规则要同时满足两件事：
  // 1. `<div>知识<span>管理</span></div>` 中 `知识` 与 `管理` 必须能拼起来搜——
  //    它们之间只有行内元素（span），属于同一段文字。
  // 2. `<div><p>知识</p><p>管理</p></div>` 中两段不能被拼起来——`p` 是块级边界。
  //
  // 做法：从文本节点向上走，遇到第一个「块级容器」就停下，那一段连续行内内容归为一组。
  // 若一路走到 root 都没遇到块级元素，则整棵树视为同一段（例如 `<div>知识<span>管理</span></div>`，
  // div 本身就是这段文字的唯一容器）。
  const INLINE_TAGS = new Set([
    'SPAN', 'A', 'EM', 'STRONG', 'CODE', 'B', 'I', 'U', 'S', 'SMALL', 'SUB', 'SUP',
    'MARK', 'INS', 'DEL', 'LABEL', 'FONT', 'BDI', 'BDO', 'CITE', 'Q', 'ABBR', 'BR', 'WBR',
  ])
  /** 是否是「行内元素」（不含块级语义）。未知标签按块级处理，宁可少拼不可错拼。 */
  const isInlineElement = (el) => !!el?.tagName && INLINE_TAGS.has(String(el.tagName).toUpperCase())

  /** 找这段文字所属的「连续行内内容」容器。 */
  const findLooseContainer = (node) => {
    let el = node.parentElement
    while (el && el !== root) {
      // 块级边界：到此为止，这段文字归它。
      if (isSearchBlockElement(el)) return el
      // 第一个非行内元素就是容纳这段文字的容器。
      if (!isInlineElement(el)) return el
      el = el.parentElement
    }
    // 一路都是行内元素、或直接走到 root：交给 root 聚合整段。
    return root
  }

  const looseGroups = new Map()
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const text = String(node?.nodeValue || '')
      if (!text) return NodeFilter.FILTER_REJECT
      const parent = node.parentElement
      if (!parent) return NodeFilter.FILTER_REJECT
      if (excludeSelector && parent.closest(excludeSelector)) return NodeFilter.FILTER_REJECT
      if (parent.closest(`.${hitClass}`)) return NodeFilter.FILTER_REJECT
      if (parent.closest(SEARCH_BLOCK_SELECTOR)) return NodeFilter.FILTER_REJECT
      return NodeFilter.FILTER_ACCEPT
    },
  })
  let current = walker.nextNode()
  while (current) {
    const container = findLooseContainer(current)
    if (container) {
      if (!looseGroups.has(container)) looseGroups.set(container, [])
      looseGroups.get(container).push(current)
    }
    current = walker.nextNode()
  }
  for (const [el, nodes] of looseGroups) innerMost.push({ el, nodes })
  return innerMost
}

/**
 * 把一个块内的命中映射回具体文本节点并切分出高亮 span。
 *
 * 关键点：一个命中**可能横跨多个文本节点**（`cir` + `cle`）。逐个节点切开会产生
 * 两段各自包着 span 的碎片，用户看到的是两个高亮而不是一次命中。所以这里按
 * 「节点内可见的命中片段」重建整块内容：每个命中只生成**一个** span，跨节点时
 * 把中间那些节点的文本全都收进同一个 span。
 *
 * @param {Array<{node: Text, start: number, end: number, text: string}>} nodesWithOffset
 *        按文档顺序排列、带全局起始偏移的文本节点
 * @param {Array<{start:number, end:number, text:string}>} hits 块内全局偏移区间
 * @param {string} hitClass
 * @returns {HTMLElement[]} 新建的高亮 span（文档顺序）
 */
const applyHitsToNodes = (nodesWithOffset, hits, hitClass) => {
  if (!hits.length || !nodesWithOffset.length) return []

  // 对每个命中，找出它覆盖到的节点范围 [fromIdx, toIdx]。
  // 命中之间不会重叠（正则的 exec 不产生重叠匹配），所以这些范围也互不重叠。
  const plans = []
  for (const hit of hits) {
    let fromIdx = -1
    let toIdx = -1
    for (let i = 0; i < nodesWithOffset.length; i += 1) {
      const { start, text } = nodesWithOffset[i]
      const end = start + text.length
      if (hit.end <= start || hit.start >= end) continue
      if (fromIdx < 0) fromIdx = i
      toIdx = i
    }
    if (fromIdx < 0) continue
    plans.push({ hit, fromIdx, toIdx })
  }
  if (!plans.length) return []

  // 从后往前重建每个受影响的节点，避免替换后偏移失效。
  // 同一节点可能同时被「上一个命中尾部」「本节点命中」「下一个命中开头」覆盖，
  // 所以先把该节点的切分点算全再一次性重写。
  const created = []
  const plansByNode = new Map()
  plans.forEach((plan) => {
    for (let i = plan.fromIdx; i <= plan.toIdx; i += 1) {
      if (!plansByNode.has(i)) plansByNode.set(i, [])
      plansByNode.get(i).push(plan)
    }
  })

  const indexes = [...plansByNode.keys()].sort((a, b) => b - a)
  for (const i of indexes) {
    const { node, start, text } = nodesWithOffset[i]
    const parent = node.parentNode
    if (!parent) continue
    const nodeEnd = start + text.length

    // 收集本节点上的所有切割边界（相对节点内偏移）。
    const cuts = []
    for (const plan of plansByNode.get(i)) {
      const localStart = Math.max(0, plan.hit.start - start)
      const localEnd = Math.min(text.length, plan.hit.end - start)
      if (localEnd > localStart) cuts.push({ start: localStart, end: localEnd, plan })
    }
    if (!cuts.length) continue
    cuts.sort((a, b) => a.start - b.start)

    const fragment = document.createDocumentFragment()
    let lastIndex = 0
    for (const cut of cuts) {
      if (cut.start > lastIndex) {
        fragment.appendChild(document.createTextNode(text.slice(lastIndex, cut.start)))
      }
      const { hit } = cut.plan
      // 命中在本节点的可见部分。跨节点时，整段命中文字由「起始节点」一次性输出，
      // 后续节点的可见部分直接跳过（它们已经被前面那个 span 收走了）。
      if (hit.start >= start) {
        const span = document.createElement('span')
        span.className = hitClass
        span.textContent = hit.text
        fragment.appendChild(span)
        created.push({ span, hit })
      }
      lastIndex = cut.end
    }
    if (lastIndex < text.length) {
      fragment.appendChild(document.createTextNode(text.slice(lastIndex)))
    }
    parent.replaceChild(fragment, node)
    void nodeEnd
  }

  // 按文档顺序（命中的 start 升序）还原。
  created.sort((a, b) => a.hit.start - b.hit.start)
  return created.map((entry) => entry.span)
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

  const blocks = collectSearchBlocks(root, excludeSelector, hitClass)
  const matches = []

  blocks.forEach(({ nodes }) => {
    if (!nodes.length) return
    // 把块内所有文本节点拼成一条连续文本，用它在整个块上跑正则。
    const joined = nodes.map((node) => String(node.nodeValue || '')).join('')
    if (!joined) return

    const nodesWithOffset = []
    let offset = 0
    for (const node of nodes) {
      const text = String(node.nodeValue || '')
      nodesWithOffset.push({ node, start: offset, text })
      offset += text.length
    }

    regex.lastIndex = 0
    const hits = []
    let matched = regex.exec(joined)
    while (matched) {
      const index = Number(matched.index || 0)
      const matchedText = String(matched[0] || '')
      // 零长度匹配（例如正则模式下的 `a*`）会让 exec 原地打转，必须手动推进。
      if (!matchedText) {
        regex.lastIndex = index + 1
        matched = regex.lastIndex <= joined.length ? regex.exec(joined) : null
        continue
      }
      hits.push({ start: index, end: index + matchedText.length, text: matchedText })
      matched = regex.exec(joined)
    }
    if (!hits.length) return

    matches.push(...applyHitsToNodes(nodesWithOffset, hits, hitClass))
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
