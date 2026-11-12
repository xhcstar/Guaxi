// 阅读位置「精确定位 + 跨重启记忆」的回归测试。
//
// 与 editorReadPosition.test.mjs 的分工：
//   - 那边锁「恢复载体与缓存 key 的选择」（该恢复谁、存在哪个 key 下）；
//   - 这边锁「恢复的精度与持久化」（落在文档的哪个字节、关掉软件后还在不在）。
//
// 用户报的两个问题正好对应这两个文件：
//   1. 阅读位置只能回到大概位置 → 内容锚点 / 像素可信度 / 落位收敛；
//   2. 各种切换偶发失灵、重启记不住 → 落盘结构与读写时机。
import test from 'node:test'
import assert from 'node:assert/strict'

import {
  makeContentAnchor,
  resolveAnchorTarget,
  isRestoreSettled,
  isPixelTrustworthy,
  buildPersistPayload,
  parsePersistPayload,
  READ_POSITION_STORE_VERSION,
} from '../src/components/HomeViewComponent/readPositionCore.js'
import {
  createReadPositionPersistence,
  READ_POSITION_STORAGE_KEY,
} from '../src/components/HomeViewComponent/readPositionStore.js'

// ═══════════════════════════════════════════════════════════════════════════
// 一、精确定位：内容锚点
//
// 「只能回到大概位置」的根因：像素依赖文档高度，比例依赖总高度，二者在图片加载、
// 窗口缩放、字体变化后都会漂。而 Markdown 渲染出的每个块都带 data-md-start-line，
// 行号在任何排版下都指向同一段文字，所以「行号 + 块内偏移」才是精确坐标。
// ═══════════════════════════════════════════════════════════════════════════

test('锚点：行号非法时返回 null，而不是伪造一个 0 行锚点', () => {
  assert.equal(makeContentAnchor({ anchorLine: 0 }), null)
  assert.equal(makeContentAnchor({ anchorLine: -3 }), null)
  assert.equal(makeContentAnchor({ anchorLine: NaN }), null)
  assert.equal(makeContentAnchor({}), null)
  assert.equal(makeContentAnchor(), null)
})

test('锚点：行号与块内偏移一起保留，偏移缺省为 0', () => {
  assert.deepEqual(makeContentAnchor({ anchorLine: 42, anchorOffset: 13 }), {
    anchorLine: 42,
    anchorOffset: 13,
  })
  assert.deepEqual(makeContentAnchor({ anchorLine: 42 }), { anchorLine: 42, anchorOffset: 0 })
})

test('锚点：按行号换算出目标 scrollTop（块顶 + 块内偏移）', () => {
  const entry = { anchorLine: 120, anchorOffset: 35 }
  // 渲染后第 120 行对应的块顶部在 2450px。
  const resolved = resolveAnchorTarget(entry, (line) => (line === 120 ? 2450 : null))
  assert.deepEqual(resolved, { top: 2485, source: 'anchor' })
})

test('锚点：块还没渲染出来时返回 null，让调用方退回像素/比例而不是滚到顶部', () => {
  const entry = { anchorLine: 120, anchorOffset: 0 }
  assert.equal(resolveAnchorTarget(entry, () => null), null)
  assert.equal(resolveAnchorTarget({ anchorLine: 0 }, () => 100), null)
  assert.equal(resolveAnchorTarget(entry, undefined), null)
})

test('锚点：负偏移不能把目标算成负数', () => {
  const entry = { anchorLine: 3, anchorOffset: -50 }
  assert.equal(resolveAnchorTarget(entry, () => 10).top, 0)
})

test('锚点：窗口变窄导致排版重排时仍落在同一段文字（像素此时已不可信）', () => {
  // 记录时：第 80 行在 1200px 处。
  const entry = { anchorLine: 80, anchorOffset: 0, scrollTop: 1200, scrollHeight: 20000, clientHeight: 900 }
  // 重排后：同一行因为换行变多，现在在 1680px 处。
  const anchor = resolveAnchorTarget(entry, () => 1680)
  // 若走像素，会落在 1200（错误的那一段）；锚点给出正确的新位置。
  assert.equal(anchor.top, 1680)
  assert.equal(anchor.source, 'anchor')
})

// ═══════════════════════════════════════════════════════════════════════════
// 二、像素可信度：什么时候允许用像素
//
// 像素是「同尺寸下最准」的，问题是文档或视口一变它就不再指向同一处。
// 这条判断决定恢复时是相信像素、还是改用锚点/比例。
// ═══════════════════════════════════════════════════════════════════════════

test('像素可信度：文档高度与视口尺寸都没变时，像素可信', () => {
  const entry = { scrollHeight: 10000, clientHeight: 800 }
  assert.equal(isPixelTrustworthy(entry, { scrollHeight: 10000, clientHeight: 800 }), true)
})

test('像素可信度：图片加载使文档长高后，像素不可信（会落到别的段落）', () => {
  const entry = { scrollHeight: 10000, clientHeight: 800 }
  assert.equal(isPixelTrustworthy(entry, { scrollHeight: 14000, clientHeight: 800 }), false)
})

test('像素可信度：窗口被拉高后像素不可信', () => {
  const entry = { scrollHeight: 10000, clientHeight: 800 }
  assert.equal(isPixelTrustworthy(entry, { scrollHeight: 10000, clientHeight: 1200 }), false)
})

test('像素可信度：旧记录没存尺寸时仍然放行，交给可滚动范围判断兜底', () => {
  assert.equal(isPixelTrustworthy({ scrollTop: 500 }, { scrollHeight: 9000, clientHeight: 800 }), true)
})

// ═══════════════════════════════════════════════════════════════════════════
// 三、落位收敛：逐帧重试什么时候停
//
// 预览是渐进渲染：图片、公式、图表会在之后的若干帧里继续把文档撑高。
// 停太早 → 只恢复一部分；停太晚 → 和用户抢滚动条。这条规则决定的就是那一刻。
// ═══════════════════════════════════════════════════════════════════════════

test('落位收敛：目标为 0 时立刻结束，不会跟着文档撑高反复滚动', () => {
  assert.equal(isRestoreSettled({ reachable: true, targetTop: 0, heightStableFrames: 0 }), true)
})

test('落位收敛：目标可达但文档还在长高时不能收工（否则图片加载后再次错位）', () => {
  assert.equal(isRestoreSettled({ reachable: true, targetTop: 1200, heightStableFrames: 0 }), false)
  assert.equal(isRestoreSettled({ reachable: true, targetTop: 1200, heightStableFrames: 1 }), false)
  assert.equal(isRestoreSettled({ reachable: true, targetTop: 1200, heightStableFrames: 2 }), true)
})

test('落位收敛：目标越界时等高度稳定后收工，保留 clamp 后的结果', () => {
  assert.equal(isRestoreSettled({ reachable: false, targetTop: 99999, heightStableFrames: 0 }), false)
  assert.equal(isRestoreSettled({ reachable: false, targetTop: 99999, heightStableFrames: 2 }), true)
})

test('落位收敛：目标不是数字时直接结束，不会无限重试', () => {
  assert.equal(isRestoreSettled({ reachable: false, targetTop: NaN, heightStableFrames: 0 }), true)
})

test('落位收敛：记录里有目标、但这一帧还算不出来时必须继续等（不能停在开头）', () => {
  // 典型场景：切回来的瞬间预览容器还是空的，块尚未渲染，resolveAnchorTarget 返回 null，
  // targetTop 算成 0。若此时收工，用户看到的就是文档开头——这正是「切回来回到开头」。
  assert.equal(
    isRestoreSettled({ reachable: false, targetTop: 0, heightStableFrames: 9, awaitingTarget: true }),
    false,
  )
})

test('落位收敛：重试用尽后不再等待，避免永远挂着', () => {
  // awaitingTarget 只在 attempt < maxAttempts 时成立；用尽后由调用方传 false，
  // 这条断言锁住「false + 目标 0」仍然会收工。
  assert.equal(
    isRestoreSettled({ reachable: false, targetTop: 0, heightStableFrames: 0, awaitingTarget: false }),
    true,
  )
})

// ═══════════════════════════════════════════════════════════════════════════
// 四、持久化：关掉软件再打开也要记得
//
// 此前所有位置只存在组件内存的 Map 里，进程退出即消失，这就是「重启记不住」的根因。
// 落盘结构必须带版本号、必须能被脏数据安全降级，否则一次坏写入会让编辑器启动异常。
// ═══════════════════════════════════════════════════════════════════════════

test('落盘：payload 带版本号，条目只保留恢复所需字段', () => {
  const positions = new Map()
  positions.set('markdown:a.md', {
    path: 'a.md',
    kind: 'markdown',
    scrollTop: 1234.6,
    scrollRatio: 0.123456789,
    anchorLine: 88,
    anchorOffset: 12,
    scrollHeight: 12000,
    clientHeight: 900,
    savedAt: 111,
    // 不该被写进去的运行时字段
    viewState: { huge: true },
  })
  const payload = buildPersistPayload({ positions, modes: new Map([['a.md', 'preview']]) })
  assert.equal(payload.version, READ_POSITION_STORE_VERSION)
  assert.equal(payload.positions.length, 1)
  const entry = payload.positions[0]
  assert.equal(entry.path, 'a.md')
  assert.equal(entry.scrollTop, 1235, '像素取整')
  assert.equal(entry.scrollRatio, 0.1235, '比例保留四位')
  assert.equal(entry.anchorLine, 88)
  assert.equal(entry.anchorOffset, 12)
  assert.equal(entry.viewState, undefined, 'Monaco viewState 不可序列化，必须剔除')
  assert.equal(payload.modes['a.md'], 'preview')
})

test('落盘：PDF 的页号与页内偏移会被保留（跨缩放稳定的坐标）', () => {
  const positions = new Map([
    ['pdf:a.tex', { path: 'a.tex', kind: 'pdf', page: 7, pageYPt: 120.5, scrollRatio: 0.3 }],
  ])
  const entry = buildPersistPayload({ positions }).positions[0]
  assert.equal(entry.page, 7)
  assert.equal(entry.pageYPt, 120.5)
})

test('落盘：探索者的 cellKey 会被保留，重启后能锚回同一个单元格', () => {
  const positions = new Map([
    ['explorer:a.md', { path: 'a.md', kind: 'explorer', scrollTop: 900, cellKey: 'cell-3', lineNumber: 42 }],
  ])
  const entry = buildPersistPayload({ positions }).positions[0]
  assert.equal(entry.cellKey, 'cell-3')
  assert.equal(entry.lineNumber, 42)
})

test('落盘：版本不匹配时整体丢弃，不尝试猜测旧结构', () => {
  const parsed = parsePersistPayload({ version: 999, positions: [{ key: 'markdown:a.md', path: 'a.md' }] })
  assert.deepEqual(parsed, { positions: [], modes: {} })
})

test('落盘：脏数据（null / 字符串 / 空对象）都被安全降级', () => {
  assert.deepEqual(parsePersistPayload(null), { positions: [], modes: {} })
  assert.deepEqual(parsePersistPayload('oops'), { positions: [], modes: {} })
  assert.deepEqual(parsePersistPayload({ version: READ_POSITION_STORE_VERSION }), { positions: [], modes: {} })
})

test('落盘：条目按 path 重新推导 key，旧 key 不一致时不会写到错误载体', () => {
  const parsed = parsePersistPayload({
    version: READ_POSITION_STORE_VERSION,
    positions: [
      { key: 'markdown:D:\\x\\a.md', path: 'D:\\x\\a.md', scrollTop: 10 },
      // 没有 kind 的条目应被丢弃
      { key: '', path: 'b.md', scrollTop: 20 },
    ],
    modes: { 'D:\\x\\a.md': 'preview' },
  })
  assert.equal(parsed.positions.length, 1)
  assert.equal(parsed.positions[0].key, 'markdown:D:/x/a.md')
  assert.equal(parsed.positions[0].kind, 'markdown')
  assert.equal(parsed.modes['D:/x/a.md'], 'preview')
})

test('落盘：未知模式键归一到 edit，不会写回非法值', () => {
  const parsed = parsePersistPayload({
    version: READ_POSITION_STORE_VERSION,
    modes: { 'a.md': 'nonsense' },
  })
  assert.equal(parsed.modes['a.md'], 'edit')
})

/** 最小的 localStorage 替身，可注入故障。 */
const createFakeStorage = (initial = {}) => {
  const data = new Map(Object.entries(initial))
  return {
    getItem: (k) => (data.has(k) ? data.get(k) : null),
    setItem: (k, v) => data.set(k, String(v)),
    removeItem: (k) => data.delete(k),
    _data: data,
  }
}

test('落盘控制器：flush 把内存位置写进 storage，load 能原样读回', () => {
  const storage = createFakeStorage()
  const positions = new Map([['markdown:a.md', { path: 'a.md', kind: 'markdown', scrollTop: 500, anchorLine: 20 }]])
  const modes = new Map([['a.md', 'preview']])
  const persistence = createReadPositionPersistence({
    storage,
    readSources: () => ({ positions, modes }),
  })
  assert.equal(persistence.flush('test'), true)
  assert.ok(storage._data.has(READ_POSITION_STORAGE_KEY))

  let restored = null
  const reader = createReadPositionPersistence({
    storage,
    onRestore: (parsed) => { restored = parsed },
  })
  reader.load()
  assert.ok(restored)
  assert.equal(restored.positions[0].scrollTop, 500)
  assert.equal(restored.modes['a.md'], 'preview')
})

test('落盘控制器：storage 写失败时静默返回 false，不抛给调用方', () => {
  const storage = {
    getItem: () => null,
    setItem: () => { throw new Error('QuotaExceededError') },
    removeItem: () => {},
  }
  const persistence = createReadPositionPersistence({
    storage,
    readSources: () => ({ positions: new Map([['markdown:a.md', { path: 'a.md', kind: 'markdown', scrollTop: 1 }]]) }),
  })
  assert.equal(persistence.flush('test'), false)
})

test('落盘控制器：坏的 JSON 视为没存过，不让启动失败', () => {
  const storage = createFakeStorage({ [READ_POSITION_STORAGE_KEY]: '{not json' })
  const persistence = createReadPositionPersistence({ storage })
  assert.deepEqual(persistence.load(), { positions: [], modes: {}, lastPath: '' })
})

test('落盘控制器：没有任何可存内容时不写盘（避免开机立刻写一个空快照）', () => {
  const storage = createFakeStorage()
  const persistence = createReadPositionPersistence({
    storage,
    readSources: () => ({ positions: new Map(), modes: new Map() }),
  })
  assert.equal(persistence.flush('empty'), false)
  assert.equal(storage._data.has(READ_POSITION_STORAGE_KEY), false)
})

test('落盘控制器：防抖写盘只落最后一次，滚动过程中不会每帧写盘', async () => {
  const storage = createFakeStorage()
  let writes = 0
  const realSet = storage.setItem
  storage.setItem = (k, v) => { writes += 1; realSet(k, v) }

  const positions = new Map([['markdown:a.md', { path: 'a.md', kind: 'markdown', scrollTop: 100 }]])
  const persistence = createReadPositionPersistence({
    storage,
    delayMs: 10,
    readSources: () => ({ positions, modes: new Map() }),
  })
  for (let i = 0; i < 20; i += 1) {
    positions.set('markdown:a.md', { path: 'a.md', kind: 'markdown', scrollTop: i })
    persistence.schedule('scroll')
  }
  assert.equal(writes, 0, '防抖窗口内不应写盘')
  await new Promise((resolve) => setTimeout(resolve, 80))
  assert.equal(writes, 1, '二十次滚动只应写一次盘')
  // 落盘的是最后一次的值，而不是中间某一帧。
  const saved = JSON.parse(storage._data.get(READ_POSITION_STORAGE_KEY))
  assert.equal(saved.positions[0].scrollTop, 19)
})

test('落盘控制器：flush 会取消待写入，离开页面时立刻落盘不会重复写', async () => {
  const storage = createFakeStorage()
  let writes = 0
  const realSet = storage.setItem
  storage.setItem = (k, v) => { writes += 1; realSet(k, v) }
  const positions = new Map([['markdown:a.md', { path: 'a.md', kind: 'markdown', scrollTop: 42 }]])
  const persistence = createReadPositionPersistence({
    storage,
    delayMs: 10,
    readSources: () => ({ positions, modes: new Map() }),
  })
  persistence.schedule('scroll')
  persistence.flush('deactivate')
  assert.equal(writes, 1, 'flush 应立刻写一次')
  await new Promise((resolve) => setTimeout(resolve, 60))
  assert.equal(writes, 1, '防抖定时器已被取消，不应再写第二次')
})
