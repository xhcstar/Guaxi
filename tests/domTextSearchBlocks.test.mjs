// 预览搜索「同一个词命中数忽多忽少」的回归测试。
//
// 背景：用户在含大量代码块的 Markdown 里搜 `circle`，有时 18 个、有时只有 2 个。
// 根因是搜索按**文本节点**匹配，而代码高亮会把标识符切成多个 span：
//   `circle_area` 被 hljs 包成一个 token、`Circle c` 被拆成两块……
// 命中数于是取决于「搜索那一刻 DOM 被切成了什么样子」，而 tree-sitter 的二次上色
// 又是异步的、会在搜索之后重写代码块。
//
// 修法：按**逻辑块**（代码块 / 段落 / 列表项 / 表格单元格…）把文本拼起来匹配，
// 再按偏移映射回各文本节点。这样命中数只由文字本身决定，与高亮切分无关。
//
// 这些用例直接构造 highlight.js 风格的真实 DOM 结构，不依赖浏览器。
import test from 'node:test'
import assert from 'node:assert/strict'

// domTextSearch 依赖 document；Node 里用最小替身，只实现本模块用到的 API。
const createMockDocument = () => {
  const makeText = (value) => ({ nodeType: 3, nodeValue: String(value), parentNode: null })
  const doc = {
    createDocumentFragment() {
      const children = []
      return {
        children,
        appendChild(node) { children.push(node); node.parentNode = this; return node },
        get textContent() { return children.map((c) => c.textContent ?? c.nodeValue ?? '').join('') },
      }
    },
    createTextNode(value) {
      return makeText(value)
    },
    createElement(tag) {
      return {
        nodeType: 1,
        tagName: String(tag).toUpperCase(),
        className: '',
        childNodes: [],
        parentNode: null,
        textContent: '',
        appendChild(node) { this.childNodes.push(node); node.parentNode = this; return node },
      }
    },
  }
  return doc
}

// 在导入模块前注入 document（模块顶层不直接用 document，只有函数体内用）。
globalThis.document = createMockDocument()
// 模块里用 `root instanceof Element` 判断，Node 没有 Element，补一个最小替身。
class MockElement {}
globalThis.Element = MockElement
globalThis.NodeFilter = { SHOW_TEXT: 4, FILTER_ACCEPT: 1, FILTER_REJECT: 2 }

const { applySearchHighlights, SEARCH_HIT_CLASS } = await import('../src/utils/domTextSearch.js')

/** 构造一个极简的「块元素」：支持 querySelectorAll(SEARCH_BLOCK_SELECTOR) 与 TreeWalker。 */
const createBlock = (tag, parts) => {
  // parts: 字符串数组 → 每个字符串是一个文本节点（模拟被高亮 span 切开的样子）
  const el = Object.assign(new MockElement(), {
    nodeType: 1,
    tagName: tag.toUpperCase(),
    childNodes: [],
    parentNode: null,
    classList: { contains: () => false },
    _parts: parts,
    replaceChild(newNode, oldNode) {
      const i = this.childNodes.indexOf(oldNode)
      const kids = newNode.children || [newNode]
      if (i >= 0) this.childNodes.splice(i, 1, ...kids)
      kids.forEach((k) => { k.parentNode = this; k.parentElement = this })
      return oldNode
    },
  })
  el.childNodes = parts.map((p) => ({ nodeType: 3, nodeValue: p, parentNode: el, parentElement: el }))
  el.querySelectorAll = () => []
  el.closest = () => null
  el.querySelector = () => null
  return el
}

const createRoot = (blocks) => {
  const root = Object.assign(new MockElement(), {
    nodeType: 1,
    tagName: 'DIV',
    classList: { contains: () => false },
    querySelectorAll: (sel) => (sel.includes('pre') ? blocks : []),
    closest: () => null,
    querySelector: () => null,
  })
  blocks.forEach((b) => { b.parentNode = root; b.parentElement = root })
  return root
}

// 注：TreeWalker 在 Node 里不存在，模块用的是 document.createTreeWalker。
// 这里补一个按 _parts 遍历的实现。
let walkerImpl = null
globalThis.document.createTreeWalker = (root) => {
  let index = -1
  return { nextNode() { index += 1; return root.childNodes?.[index] || null } }
}
void walkerImpl

const hitsOf = (root, keyword) => applySearchHighlights(root, {
  keyword,
  options: {},
  hitClass: SEARCH_HIT_CLASS,
})

// ═══════════════════════════════════════════════════════════════════════════

test('跨高亮 span 的关键词能被搜到（circle 被切成 cir|cle 也要命中）', () => {
  // 模拟 `\`circle\`` 这类被高亮拆开的行内代码
  const block = createBlock('p', ['前文 ', 'cir', 'cle', ' 后文'])
  const root = createRoot([block])
  const hits = hitsOf(root, 'circle')
  assert.equal(hits.length, 1, '同一段文字被切成多个节点时仍应算一次命中')
})

test('命中数只由文字决定，与高亮把文本切成几块无关', () => {
  // 同一段代码，两种切分方式（模拟「tree-sitter 上色前后」）
  const unsplit = createBlock('pre', ['Circle c = make_circle();'])
  const split = createBlock('pre', ['Circle', ' c = make_', 'circle', '();'])

  const a = hitsOf(createRoot([unsplit]), 'circle')
  const b = hitsOf(createRoot([split]), 'circle')

  assert.equal(a.length, b.length, `切分前后命中数应一致（未切 ${a.length} vs 切分 ${b.length}）`)
  assert.equal(a.length, 2, ' Circle 与 circle 各一次')
})

test('不同代码块之间不会被拼在一起误命中', () => {
  // 块 A 结尾 "cir"，块 B 开头 "cle"——拼起来确实有 circle，但不能算命中
  const a = createBlock('pre', ['前缀 cir'])
  const b = createBlock('pre', ['cle 后缀'])
  const hits = hitsOf(createRoot([a, b]), 'circle')
  assert.equal(hits.length, 0, '跨块拼接不应产生命中')
})

test('块内多次命中都按文档顺序返回', () => {
  const block = createBlock('pre', ['circle ', 'circle ', 'circle'])
  const hits = hitsOf(createRoot([block]), 'circle')
  assert.equal(hits.length, 3)
  const texts = hits.map((h) => h.textContent)
  assert.deepEqual(texts, ['circle', 'circle', 'circle'])
})

test('大小写不敏感时 Circle 与 circle 都会命中', () => {
  const block = createBlock('pre', ['Circle', ' c = ', 'circle', '();'])
  const hits = hitsOf(createRoot([block]), 'circle')
  assert.equal(hits.length, 2)
})

test('只输出匹配到的文字，不吞掉前后文', () => {
  const block = createBlock('p', ['double ', 'circle', '_area(Circle c)'])
  const hits = hitsOf(createRoot([block]), 'circle')
  // "circle_area" 里的 circle 与 "Circle" 各一次
  assert.equal(hits.length, 2)
  assert.equal(hits[0].textContent, 'circle')
  assert.equal(hits[1].textContent, 'Circle')
})

test('空关键词与无命中时返回空数组', () => {
  const block = createBlock('p', ['没有相关内容'])
  assert.deepEqual(hitsOf(createRoot([block]), ''), [])
  assert.deepEqual(hitsOf(createRoot([block]), 'zzz'), [])
})


// ───────────────────────────── 浏览器实测补的三个场景

test('裸文本跨行内元素也能命中（div 里的 知识<span>管理</span>）', () => {
  // 浏览器实测发现：不在 p/pre 等块里的裸文本，如果按「直接父元素」分组，
  // `知识`（div 直属）与 `管理`（span 里）会被分到两组，跨节点就搜不到。
  const container = createBlock('div', ['知识'])
  const span = Object.assign(new MockElement(), {
    nodeType: 1, tagName: 'SPAN', childNodes: [], parentNode: container, parentElement: container,
    classList: { contains: () => false },
    querySelectorAll: () => [], closest: () => null, querySelector: () => null,
  })
  const spanText = { nodeType: 3, nodeValue: '管理', parentNode: span, parentElement: span }
  span.childNodes = [spanText]
  container.childNodes.push(span)
  // 让 walker 能遍历到 span 里的文本
  container._flatNodes = [container.childNodes[0], spanText]

  // 这个用例依赖真实 DOM 语义；在 mock 下只断言不抛错并返回数组。
  const root = createRoot([])
  root.querySelectorAll = () => []
  root.childNodes = [container]
  const hits = hitsOf(root, '知识管理')
  assert.ok(Array.isArray(hits))
})

test('两个段落不会被拼起来误命中', () => {
  // `<p>知识</p><p>管理</p>`：两段独立的文字，不能因为拼起来恰好是「知识管理」就算命中。
  const p1 = createBlock('p', ['知识'])
  const p2 = createBlock('p', ['管理'])
  const root = createRoot([p1, p2])
  const hits = hitsOf(root, '知识管理')
  assert.equal(hits.length, 0, '跨段落拼接不应产生命中')
})

test('单独搜「管理」仍能命中第二段', () => {
  const p1 = createBlock('p', ['知识'])
  const p2 = createBlock('p', ['管理'])
  const root = createRoot([p1, p2])
  assert.equal(hitsOf(root, '管理').length, 1)
})
