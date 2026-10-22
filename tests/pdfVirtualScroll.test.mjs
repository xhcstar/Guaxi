// PDF 虚拟滚动的几何与窗口化回归测试。
//
// 背景：5000 页 PDF 卡顿的根因是「每页一个 DOM 节点」+「每次滚动逐页读 offsetTop
// 触发同步重排」。修复方式是把页面位置改成纯计算，并且只给视口附近的页建 DOM。
// 这里用一个不依赖浏览器的最小模型复现那套算法，锁住三条性质：
//   1. 任意页数下，挂载的页数都有界（与总页数无关）；
//   2. spacer 高度 + 已挂载页高度 = 容器总高度（滚动条长度不能缩水）；
//   3. 二分定位的可见区间与朴素线性扫描一致。
import assert from 'node:assert/strict'

const PAGES_PADDING = 12
const PAGE_GAP = 12
const WINDOW_BUFFER_SCREENS = 1.5

/** 复刻 GuaxiPdfViewer 的 pageGeometry。 */
const buildGeometry = (count, pageH) => {
  const out = new Array(count)
  let y = PAGES_PADDING
  for (let i = 0; i < count; i += 1) {
    out[i] = { num: i + 1, top: y, height: pageH }
    y += pageH + PAGE_GAP
  }
  return out
}

const lowerBound = (geo, value) => {
  let lo = 0
  let hi = geo.length
  while (lo < hi) {
    const mid = (lo + hi) >> 1
    if (geo[mid].top + geo[mid].height < value) lo = mid + 1
    else hi = mid
  }
  return lo
}

/** 复刻 recomputeWindow。 */
const computeWindow = (geo, { scrollTop, clientHeight }) => {
  const top = scrollTop
  const bottom = top + clientHeight
  const buffer = clientHeight * WINDOW_BUFFER_SCREENS
  const start = lowerBound(geo, top - buffer)
  let end = start
  while (end < geo.length && geo[end].top <= bottom + buffer) end += 1
  return { start, end }
}

/** spacer 高度（复刻 spacerTop / spacerBottom）。 */
const spacers = (geo, { start, end }) => {
  const top = start > 0 ? Math.max(0, geo[Math.min(start, geo.length - 1)].top - PAGES_PADDING) : 0
  const lastBottom = end > 0 ? geo[end - 1].top + geo[end - 1].height : PAGES_PADDING
  const containerHeight = geo.length ? geo[geo.length - 1].top + geo[geo.length - 1].height : 0
  const bottom = end < geo.length ? Math.max(0, containerHeight - lastBottom) : 0
  return { top, bottom }
}

// 1) 挂载页数有界：5000 页 PDF 在任意滚动位置都只挂载十几页。
const big = buildGeometry(5000, 800)
const viewport = 900
let maxMounted = 0
for (let scrollTop = 0; scrollTop < 5000 * 812; scrollTop += 977) {
  const range = computeWindow(big, { scrollTop, clientHeight: viewport })
  maxMounted = Math.max(maxMounted, range.end - range.start)
}
assert.ok(
  maxMounted <= 20,
  `挂载页数必须有界，实际最多 ${maxMounted} 页`
)
assert.ok(maxMounted > 0, '必须至少挂载一页')

// 2) 滚动条总长不缩水：spacer + 已挂载页 + gap 必须等于容器总高度。
for (const scrollTop of [0, 100000, 2000000]) {
  const geo = big
  const range = computeWindow(geo, { scrollTop, clientHeight: viewport })
  const sp = spacers(geo, range)
  const mounted = geo.slice(range.start, range.end)
  const mountedHeight = mounted.reduce((sum, p) => sum + p.height, 0)
  // 容器高度 = 首尾 padding + 所有页高 + 所有 gap
  const containerHeight = PAGES_PADDING * 2 + geo.length * 800 + (geo.length - 1) * PAGE_GAP
  // 已挂载部分占据的跨度 = 最后一页底 - 第一页顶
  const spanTop = mounted.length ? mounted[0].top : 0
  const spanBottom = mounted.length ? mounted[mounted.length - 1].top + mounted[mounted.length - 1].height : 0
  const span = spanBottom - spanTop
  const total = sp.top + span + sp.bottom
  // 允许首尾 padding 的差额（spacer 不含容器 padding）。
  assert.ok(
    Math.abs(total + PAGES_PADDING * 2 - containerHeight) <= PAGE_GAP * 2,
    `spacer 高度必须能撑出完整滚动范围：total=${total} container=${containerHeight}`
  )
  assert.ok(mountedHeight > 0 || geo.length === 0, '挂载页必须有高度')
}

// 3) 二分定位与线性扫描结果一致。
for (const scrollTop of [0, 50000, 1234567, 3999999]) {
  const linearStart = big.findIndex((p) => p.top + p.height >= scrollTop - viewport * WINDOW_BUFFER_SCREENS)
  const range = computeWindow(big, { scrollTop, clientHeight: viewport })
  assert.equal(
    range.start,
    linearStart < 0 ? big.length : linearStart,
    `二分起点必须与线性扫描一致 (scrollTop=${scrollTop})`
  )
}

// 4) 小文档（10 页）不会因为窗口化而少挂载。
const small = buildGeometry(10, 800)
const smallRange = computeWindow(small, { scrollTop: 0, clientHeight: 4000 })
assert.equal(smallRange.start, 0)
assert.equal(smallRange.end, 10, '视口足够大时必须挂载全部页')

console.log('pdfVirtualScroll: ok')

// ───────────────────────── 跳页：不能依赖目标页已挂载 ─────────────────────────
//
// 回归背景：虚拟滚动只挂载视口附近十几页。跳到远处的页（597 页里的第 500 页）时
// 目标节点根本不存在，旧实现靠 `node.offsetTop` + 重试，必然失败并报
// 「页面节点未就绪」。正确做法是用同一份几何算出目标位置再滚过去。

/** 复刻 goToPage 的位置计算。 */
const pageScrollTop = (geo, pageNum, { viewportHeight = 900, align = 'top' } = {}) => {
  const item = geo[pageNum - 1]
  if (!item) return null
  const offset = align === 'center' ? viewportHeight * 0.3 : 8
  // 首页贴顶：容器有 12px padding，减 offset 会留下 4px 空白。
  if (pageNum === 1) return 0
  return Math.max(0, item.top - offset)
}

// 1) 跳到很远的页也能算出位置，且不需要该页在 DOM 里。
const far = pageScrollTop(big, 500, { viewportHeight: viewport })
assert.ok(far !== null, '第 500 页必须能算出滚动位置')
assert.ok(far > 0, '远页的滚动位置应大于 0')

// 2) 滚到那个位置后，窗口应当把第 500 页包含进来 —— 这是「跳过去就能看到」的充要条件。
const afterJump = computeWindow(big, { scrollTop: far, clientHeight: viewport })
assert.ok(
  afterJump.start <= 499 && afterJump.end > 499,
  `滚动到第 500 页的位置后窗口必须覆盖它，实际 [${afterJump.start}, ${afterJump.end})`
)

// 3) 位置与几何一致：滚过去后视口顶部正好落在目标页顶边附近。
const targetPage = big[499]
assert.ok(Math.abs((far + 8) - targetPage.top) < 1, '滚动位置应精确对齐目标页顶边')

// 4) 首页不能算出负数。
assert.equal(pageScrollTop(big, 1), 0, '第 1 页应滚到容器顶部')

// 5) 越过末页返回 null（而不是崩）。
assert.equal(pageScrollTop(big, 99999), null)

// 6) 页内竖直偏移定位（源码正向跳转）同样不依赖 DOM。
const scrollToPoint = (geo, pageNum, yPt, scale = 1, viewportHeight = 900) => {
  const item = geo[pageNum - 1]
  if (!item) return null
  return Math.max(0, item.top + yPt * scale - viewportHeight * 0.35)
}
const pointTop = scrollToPoint(big, 300, 200)
assert.ok(pointTop !== null && pointTop > 0, '页内定位必须能算出位置')
const pointWindow = computeWindow(big, { scrollTop: pointTop, clientHeight: viewport })
assert.ok(
  pointWindow.start <= 299 && pointWindow.end > 299,
  '页内定位后窗口必须覆盖第 300 页'
)

console.log('pdfJumpToPage: ok')
