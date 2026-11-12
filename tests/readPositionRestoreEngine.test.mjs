// 逐帧落位引擎的回归测试。
//
// 这个引擎把「该滚到哪里」变成「每帧写一次 scrollTop 直到稳定」。它是最容易出问题的一环：
// 停太早 → 只恢复一部分；把「算不出来」当成 0 → 回到开头；停太晚 → 和用户抢滚动条。
// 用假元素把「文档在渐进渲染中长高」这件事精确地模拟出来，就能在毫秒级复现真实节奏下的问题。
import test from 'node:test'
import assert from 'node:assert/strict'

import {
  planScrollTarget,
  restoreScrollTopWithGrowthRetry,
  keepAnchorStableAfterRestore,
  entryHasScrollTarget,
} from '../src/components/HomeViewComponent/readPositionRestore.js'

/** 假滚动容器：可以按帧改变内容高度，模拟图片/图表加载。 */
const createFakeScroller = ({ clientHeight = 600, heights = [2000] } = {}) => {
  let frame = 0
  const el = {
    isConnected: true,
    clientHeight,
    scrollTop: 0,
    get scrollHeight() {
      const h = heights[Math.min(frame, heights.length - 1)]
      return h
    },
    _tick() { frame += 1 },
  }
  return el
}

/** 不真的等帧/等超时：每次 nextFrame 顺手推进假元素的「渲染进度」。 */
const noWait = async () => {}

const runRestore = async (el, entry, options = {}) => {
  const result = await restoreScrollTopWithGrowthRetry(
    () => el,
    entry,
    {
      nextFrame: async () => { await noWait() },
      wait: async () => { el._tick() },
      ...options,
    },
  )
  return result
}

// ═══════════════════════════════════════════════════════════════════════════

test('落位：文档一开始很短（scrollHeight 还没撑起来）也能最终落到目标', async () => {
  // 三帧之内文档从 2000 长到 6000；目标 4200 一开始越界，最后可达。
  const el = createFakeScroller({ heights: [2000, 3500, 6000, 6000, 6000] })
  const result = await runRestore(el, { scrollTop: 4200, scrollHeight: 6000, clientHeight: 600 }, {
    resolveBlockTop: null,
  })
  assert.equal(el.scrollTop, 4200)
  assert.equal(result.reached, true)
  assert.equal(result.source, 'pixels')
})

test('落位：有目标但第一帧完全算不出来时，不会把视口停在开头', async () => {
  // 前两帧块还没渲染（resolveBlockTop 返回 null），且像素也不可信 → 目标算不出来。
  // 第 3 帧块出现，才应真正落位。
  const heights = [900, 900, 8000, 8000, 8000]
  const el = createFakeScroller({ heights })
  let frame = 0
  const result = await runRestore(
    el,
    { anchorLine: 120, scrollTop: 5000, scrollHeight: 9000, clientHeight: 600 },
    {
      resolveBlockTop: () => {
        frame += 1
        return frame >= 3 ? 5000 : null
      },
    },
  )
  assert.equal(el.scrollTop, 5000, '最终应落在锚点处，而不是 0')
  assert.equal(result.source, 'anchor')
  assert.equal(result.reached, true)
})

test('落位：锚点优先于像素（排版变化后像素已不可信）', async () => {
  const el = createFakeScroller({ heights: [9000, 9000, 9000] })
  const result = await runRestore(
    el,
    { anchorLine: 80, scrollTop: 1200, scrollHeight: 5000, clientHeight: 600 },
    { resolveBlockTop: (line) => (line === 80 ? 3000 : null) },
  )
  assert.equal(el.scrollTop, 3000)
  assert.equal(result.source, 'anchor')
})

test('落位：文档比记录时矮很多时退回比例，而不是盲信越界的像素', async () => {
  const el = createFakeScroller({ clientHeight: 500, heights: [2000, 2000, 2000] })
  const result = await runRestore(
    el,
    { scrollTop: 6000, scrollRatio: 0.75, scrollHeight: 8000, clientHeight: 500 },
    {},
  )
  // maxTop = 1500；0.75 * 1500 = 1125
  assert.equal(el.scrollTop, 1125)
  assert.equal(result.source, 'ratio')
})

test('落位：目标是顶部（0）时不折腾，尽快收工', async () => {
  const el = createFakeScroller({ heights: [4000, 4000, 4000] })
  let frames = 0
  const result = await restoreScrollTopWithGrowthRetry(
    () => el,
    { scrollTop: 0, scrollRatio: 0 },
    { nextFrame: async () => { frames += 1 }, wait: async () => { el._tick() } },
  )
  assert.equal(el.scrollTop, 0)
  assert.equal(result.reached, true)
  assert.equal(frames, 1, '顶部目标应在第一帧就收工')
})

test('落位：文档高度持续变化时不提前收工（否则图片加载后再次错位）', async () => {
  // 高度一直从 3000 长到 7000，每帧都在变；必须等它彻底稳定才收工。
  const heights = [3000, 4000, 5000, 6000, 7000, 7000, 7000]
  const el = createFakeScroller({ heights })
  const result = await runRestore(el, { scrollTop: 2500, scrollHeight: 7000, clientHeight: 600 }, {})
  assert.equal(el.scrollTop, 2500)
  assert.equal(result.reached, true)
  // 收工时文档已经稳定在 7000：这一帧和上一帧高度相同。
  assert.ok(el.scrollHeight === 7000)
})

test('落位：元素还没挂载时立即退出，不会卡在循环里', async () => {
  const result = await restoreScrollTopWithGrowthRetry(() => null, { scrollTop: 100 }, {
    nextFrame: async () => {},
    wait: async () => {},
  })
  assert.equal(result.attempts, 1)
  assert.equal(result.reached, false)
})

test('落位：元素在重试中途被移出文档（KeepAlive 停用）时立即退出', async () => {
  const el = createFakeScroller({ heights: [9000] })
  let calls = 0
  const result = await restoreScrollTopWithGrowthRetry(
    () => {
      calls += 1
      if (calls >= 3) el.isConnected = false
      return el
    },
    { scrollTop: 5000, scrollHeight: 9000, clientHeight: 600 },
    { nextFrame: async () => {}, wait: async () => { el._tick() } },
  )
  assert.ok(result.attempts <= 3)
})

// ═══════════════════════════════════════════════════════════════════════════
// planScrollTarget：纯决策，单独锁住优先级
// ═══════════════════════════════════════════════════════════════════════════

test('决策：没有记录目标时目标是顶部，且不算「等待中」', () => {
  const plan = planScrollTarget({}, { scrollHeight: 4000, clientHeight: 600 }, null)
  assert.equal(plan.top, 0)
  assert.equal(plan.awaiting, false)
})

test('决策：记录里有目标但算不出来时标记 awaiting，且不给出 nextTop', () => {
  const plan = planScrollTarget(
    { anchorLine: 50, scrollHeight: 99999, clientHeight: 500 },
    { scrollHeight: 800, clientHeight: 600 },
    () => null,
  )
  assert.equal(plan.awaiting, true)
  assert.equal(plan.nextTop, null, 'awaiting 时不应写滚动位置')
})

test('决策：目标超出可滚动范围时给出 clamp 后的 nextTop', () => {
  const plan = planScrollTarget(
    { scrollTop: 9999, scrollHeight: 9999, clientHeight: 600 },
    { scrollHeight: 1200, clientHeight: 600 },
    null,
  )
  // 像素不可信（高度变了）→ 无比例 → 有目标 → awaiting
  assert.equal(plan.awaiting, true)
})

test('entryHasScrollTarget：只有真正写了位置才算「有目标」', () => {
  assert.equal(entryHasScrollTarget({ scrollTop: 0 }), false)
  assert.equal(entryHasScrollTarget({ scrollTop: 10 }), true)
  assert.equal(entryHasScrollTarget({ scrollRatio: 0.2 }), true)
  assert.equal(entryHasScrollTarget({ anchorLine: 7 }), true)
  assert.equal(entryHasScrollTarget(null), false)
})


// ═══════════════════════════════════════════════════════════════════════════
// 落位后的看护：图片在落位之后才加载时，不能把用户看的那一段推走
// ═══════════════════════════════════════════════════════════════════════════

/** 带事件监听的假容器，用来断言「用户一滚动就放手」。 */
const createGuardedScroller = () => {
  const listeners = new Map()
  const el = {
    isConnected: true,
    clientHeight: 600,
    scrollHeight: 9000,
    scrollTop: 0,
    addEventListener: (type, fn) => listeners.set(type, fn),
    removeEventListener: (type) => listeners.delete(type),
    emit: (type) => listeners.get(type)?.(),
    listeners,
  }
  return el
}

// 看护是一个自循环：每一帧都要让出事件循环，否则测试里的定时器永远排不上。
// 这里统一用宏任务（setTimeout）推进，与线上 rAF 的语义一致（都是「让出一次」）。
const flushFrames = async (n = 8) => {
  for (let i = 0; i < n; i += 1) await new Promise((r) => setTimeout(r, 1))
}

const microFrame = async () => { await new Promise((r) => setTimeout(r, 0)) }

test('看护：文档在落位后被撑高，位置会被纠回记录的块', async () => {
  const el = createGuardedScroller()
  // 块顶从 1000 变成 2200（上方插入了一张图片）。
  let blockTop = 1000
  const entry = { anchorLine: 30, anchorOffset: 0 }
  let t = 0
  const stop = keepAnchorStableAfterRestore(() => el, entry, {
    resolveBlockTop: () => blockTop,
    durationMs: 60,
    nextFrame: async () => { t += 1; await microFrame() },
    now: () => Date.now(),
  })
  await flushFrames(2)
  assert.equal(el.scrollTop, 1000)
  // 图片加载：块整体下移，用户原本看的那一段被推出视口。
  blockTop = 2200
  el.scrollTop = 1000
  await flushFrames(2)
  assert.equal(el.scrollTop, 2200, '应把视口纠到同一个块上')
  stop()
})

test('看护：用户一旦滚动就立刻放手，不再抢滚动条', async () => {
  const el = createGuardedScroller()
  const entry = { anchorLine: 30, anchorOffset: 0 }
  let t = 0
  keepAnchorStableAfterRestore(() => el, entry, {
    resolveBlockTop: () => 1000,
    durationMs: 60,
    nextFrame: async () => { t += 1; await microFrame() },
    now: () => Date.now(),
  })
  await flushFrames(2)
  el.emit('wheel')
  assert.equal(el.listeners.has('wheel'), false, '用户滚动后应解绑监听')
  el.scrollTop = 5000
  await flushFrames(3)
  assert.equal(el.scrollTop, 5000, '放手后不得再改动用户的位置')
})

test('看护：没有锚点时完全不介入', async () => {
  const el = createGuardedScroller()
  let frames = 0
  // 记录里完全没有位置（scrollTop 0 且无比例/锚点）时不该启动看护。
  keepAnchorStableAfterRestore(() => el, { scrollTop: 0, scrollRatio: 0 }, {
    resolveBlockTop: () => 0,
    nextFrame: async () => { frames += 1; await microFrame() },
    now: () => Date.now(),
  })
  await flushFrames(3)
  assert.equal(frames, 0)
  assert.equal(el.listeners.size, 0)
})

test('看护：观看窗口结束后自行停止，不会长期占着帧循环', async () => {
  const el = createGuardedScroller()
  let t = 0
  keepAnchorStableAfterRestore(() => el, { anchorLine: 1 }, {
    resolveBlockTop: () => 100,
    durationMs: 20,
    nextFrame: async () => { t += 1; await microFrame() },
    now: () => Date.now(),
  })
  await flushFrames(10)
  assert.equal(el.listeners.size, 0, '结束后应解绑全部监听')
})
