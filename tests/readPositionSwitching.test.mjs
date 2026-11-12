// 阅读位置「跨文件 × 跨模式 × 跨页面 × 跨重启」的组合回归测试。
//
// 为什么单列一个文件：这些用例是笛卡尔积，量很大（150 种切换组合），
// 混进精确定位的用例里会让失败信息难以定位。这里的每一条都直接对应用户描述的一种操作序列。
import test from 'node:test'
import assert from 'node:assert/strict'

import {
  readPositionKey,
  expectedKeysForMode,
  createReadPositionCache,
  planReadPositionRestore,
  buildPersistPayload,
} from '../src/components/HomeViewComponent/readPositionCore.js'
import {
  createReadPositionPersistence,
  READ_POSITION_STORAGE_KEY,
} from '../src/components/HomeViewComponent/readPositionStore.js'

const MODES = ['edit', 'preview', 'split', 'explorer', 'realtime']
const FILES = ['a.md', 'b.md', 'c.tex']

const createFakeStorage = (initial = {}) => {
  const data = new Map(Object.entries(initial))
  return {
    getItem: (k) => (data.has(k) ? data.get(k) : null),
    setItem: (k, v) => data.set(k, String(v)),
    removeItem: (k) => data.delete(k),
    _data: data,
  }
}

// ═══════════════════════════════════════════════════════════════════════════
// 一、切换矩阵
//
// 用户原话：「相同文档不同模式、不同文档相同模式、不同文档不同模式之间切换时，
// 经常就无法回到原来的位置」。这三类各覆盖一遍。
// ═══════════════════════════════════════════════════════════════════════════

test('矩阵：任意「文档 A + 模式 X → 文档 B + 模式 Y」都能取回 B 在 Y 下的位置', () => {
  let scenarioCount = 0
  for (const fileA of FILES) {
    for (const modeX of MODES) {
      for (const fileB of FILES) {
        if (fileB === fileA) continue
        for (const modeY of MODES) {
          const cache = createReadPositionCache()
          const seed = (file, mode, top) => {
            for (const key of expectedKeysForMode(mode, file)) {
              cache.remember({ path: file, kind: key.split(':')[0], scrollTop: top })
            }
          }
          seed(fileA, modeX, 111)
          seed(fileB, modeY, 222)

          // 切到 B，按它自己的模式 Y 决定恢复哪些载体（不是按当前模式 X）。
          const plan = planReadPositionRestore({
            modeKey: modeY,
            path: fileB,
            modeChanging: modeX !== modeY,
          })
          assert.ok(plan.carriers.length > 0, `${fileA}/${modeX} → ${fileB}/${modeY} 应有恢复载体`)
          for (const carrier of plan.carriers) {
            const entry = cache.get(readPositionKey(fileB, carrier))
            assert.ok(entry, `${fileA}/${modeX} → ${fileB}/${modeY} 缺少 ${carrier} 载体`)
            assert.equal(
              entry.scrollTop,
              222,
              `${fileA}/${modeX} → ${fileB}/${modeY} 取到了别处的位置`,
            )
          }
          // A 的位置不能被 B 覆盖。
          for (const key of expectedKeysForMode(modeX, fileA)) {
            assert.equal(cache.get(key).scrollTop, 111, `${fileA} 的位置被 ${fileB} 覆盖`)
          }
          scenarioCount += 1
        }
      }
    }
  }
  // 3 文件 × 5 模式 × 2 个其它文件 × 5 模式 = 150 种组合。
  assert.equal(scenarioCount, 150)
})

test('矩阵：同一文档在五种模式之间往返，各自位置互不覆盖', () => {
  const cache = createReadPositionCache()
  // 注意 edit 与 split 共用同一个 Monaco 载体（就是那台编辑器），分屏的预览侧与纯预览
  // 共用同一个 markdown 载体（就是那个预览容器）。这是设计使然：它们物理上是同一个 DOM。
  // 真正必须互相隔离的是「编辑器 vs 预览」「探索者 vs 其它」「不同文件」。
  const tops = { edit: 100, preview: 200, split: 300, explorer: 400, realtime: 500 }
  for (const mode of MODES) {
    for (const key of expectedKeysForMode(mode, 'a.md')) {
      cache.remember({ path: 'a.md', kind: key.split(':')[0], scrollTop: tops[mode] })
    }
  }
  // 五种模式全部登记过之后，每个 key 上留下的都是最后写它的那个模式的值。
  const expectedByCarrier = {
    // split 最后写 editor，所以 editor 上是 split 的值。
    editor: tops.split,
    // split 也写 markdown，所以 markdown 上是 split 的值。
    markdown: tops.split,
    explorer: tops.explorer,
    realtime: tops.realtime,
  }
  for (const [carrier, expected] of Object.entries(expectedByCarrier)) {
    const entry = cache.get(readPositionKey('a.md', carrier))
    assert.ok(entry, `${carrier} 应有记录`)
    assert.equal(entry.scrollTop, expected, `${carrier} 的值不对`)
  }
})

test('矩阵：编辑器与预览是两套独立载体，切模式不会互相覆盖', () => {
  const cache = createReadPositionCache()
  cache.remember({ path: 'a.md', kind: 'editor', scrollTop: 100 })
  cache.remember({ path: 'a.md', kind: 'markdown', scrollTop: 900 })
  assert.equal(cache.get(readPositionKey('a.md', 'editor')).scrollTop, 100)
  assert.equal(cache.get(readPositionKey('a.md', 'markdown')).scrollTop, 900)
})

test('矩阵：探索者与其他四种模式完全隔离', () => {
  const cache = createReadPositionCache()
  cache.remember({ path: 'a.md', kind: 'editor', scrollTop: 100 })
  cache.remember({ path: 'a.md', kind: 'markdown', scrollTop: 200 })
  cache.remember({ path: 'a.md', kind: 'explorer', scrollTop: 400 })
  cache.remember({ path: 'a.md', kind: 'realtime', scrollTop: 500 })
  assert.equal(cache.get(readPositionKey('a.md', 'explorer')).scrollTop, 400)
  assert.equal(cache.get(readPositionKey('a.md', 'editor')).scrollTop, 100)
})

test('矩阵：不同文档同一模式，位置严格按路径隔离', () => {
  const cache = createReadPositionCache()
  cache.remember({ path: 'a.md', kind: 'markdown', scrollTop: 111 })
  cache.remember({ path: 'b.md', kind: 'markdown', scrollTop: 222 })
  assert.equal(cache.get(readPositionKey('a.md', 'markdown')).scrollTop, 111)
  assert.equal(cache.get(readPositionKey('b.md', 'markdown')).scrollTop, 222)
  // Windows / POSIX 写法指向同一份记录，不会因为路径分隔符分裂成两条。
  cache.remember({ path: 'D:/x/c.md', kind: 'markdown', scrollTop: 333 })
  assert.equal(cache.get(readPositionKey('D:\\x\\c.md', 'markdown')).scrollTop, 333)
})

// ═══════════════════════════════════════════════════════════════════════════
// 二、跳转后再切换
// ═══════════════════════════════════════════════════════════════════════════

test('跳转：大纲点击后切走再切回，回到点击处而不是更早的位置', () => {
  const cache = createReadPositionCache()
  cache.remember({ path: 'a.md', kind: 'markdown', scrollTop: 800, anchorLine: 10 })
  // 点大纲跳到第 300 行 → 滚动把新位置写进缓存
  cache.remember({ path: 'a.md', kind: 'markdown', scrollTop: 6400, anchorLine: 300 })
  const entry = cache.get(readPositionKey('a.md', 'markdown'))
  assert.equal(entry.scrollTop, 6400)
  assert.equal(entry.anchorLine, 300)
})

test('跳转：预览里点大纲不会污染编辑器侧的位置', () => {
  const cache = createReadPositionCache()
  cache.remember({ path: 'a.md', kind: 'markdown', scrollTop: 6400, anchorLine: 300 })
  assert.equal(cache.get(readPositionKey('a.md', 'editor')), null)
})

test('跳转：锚点链接打开的另一份文档有自己的位置，不会被来源文档的锚点覆盖', () => {
  const cache = createReadPositionCache()
  cache.remember({ path: 'source.md', kind: 'markdown', scrollTop: 500 })
  cache.remember({ path: 'target.md', kind: 'markdown', scrollTop: 9000 })
  // 打开 target.md 的第 12 行，随后用户继续滚动 —— 记录仍属于 target.md。
  cache.remember({ path: 'target.md', kind: 'markdown', scrollTop: 4200, anchorLine: 12 })
  assert.equal(cache.get(readPositionKey('source.md', 'markdown')).scrollTop, 500)
  assert.equal(cache.get(readPositionKey('target.md', 'markdown')).scrollTop, 4200)
})

// ═══════════════════════════════════════════════════════════════════════════
// 三、跨页面往返（KeepAlive 停用 → 重新激活）
// ═══════════════════════════════════════════════════════════════════════════

test('跨页面：五个模式各自往返一次，位置都在', () => {
  const cache = createReadPositionCache()
  for (const mode of MODES) {
    for (const key of expectedKeysForMode(mode, 'a.md')) {
      cache.remember({ path: 'a.md', kind: key.split(':')[0], scrollTop: 4242 })
    }
  }
  // 离开并返回不会清空缓存，恢复载体也仍然是各自那一个。
  for (const mode of MODES) {
    const plan = planReadPositionRestore({ modeKey: mode, path: 'a.md', modeChanging: false })
    for (const carrier of plan.carriers) {
      assert.ok(cache.get(readPositionKey('a.md', carrier)), `${mode} 往返后丢失 ${carrier}`)
    }
  }
})

// ═══════════════════════════════════════════════════════════════════════════
// 四、重启往返
// ═══════════════════════════════════════════════════════════════════════════

test('重启：预览 / PDF / 探索者三种载体与每文件模式完整往返', () => {
  const storage = createFakeStorage()
  const positions = new Map([
    ['markdown:a.md', { path: 'a.md', kind: 'markdown', scrollTop: 2100, anchorLine: 77, anchorOffset: 8, scrollHeight: 9000, clientHeight: 800 }],
    ['pdf:b.tex', { path: 'b.tex', kind: 'pdf', page: 12, pageYPt: 88, scrollRatio: 0.44 }],
    ['explorer:c.md', { path: 'c.md', kind: 'explorer', scrollTop: 3000, cellKey: 'cell-7', lineNumber: 150 }],
  ])
  const modes = new Map([['a.md', 'preview'], ['b.tex', 'split'], ['c.md', 'explorer']])
  createReadPositionPersistence({ storage, readSources: () => ({ positions, modes }) }).flush('quit')

  // 新进程：内存全空，只能靠 storage。
  let restored = null
  createReadPositionPersistence({ storage, onRestore: (p) => { restored = p } }).load()
  assert.ok(restored)

  const byKey = new Map(restored.positions.map((p) => [p.key, p]))
  assert.equal(byKey.get('markdown:a.md').anchorLine, 77, 'Markdown 锚点丢失')
  assert.equal(byKey.get('markdown:a.md').scrollHeight, 9000, '文档高度丢失，像素可信度判断会失效')
  assert.equal(byKey.get('pdf:b.tex').page, 12, 'PDF 页码丢失')
  assert.equal(byKey.get('explorer:c.md').cellKey, 'cell-7', '探索者 cellKey 丢失')
  assert.deepEqual(restored.modes, { 'a.md': 'preview', 'b.tex': 'split', 'c.md': 'explorer' })
})

test('重启：恢复后按文件+模式重新落位，目标就是上次停下的那一处', () => {
  const storage = createFakeStorage()
  createReadPositionPersistence({
    storage,
    readSources: () => ({
      positions: new Map([
        ['markdown:a.md', { path: 'a.md', kind: 'markdown', scrollTop: 2100, anchorLine: 77 }],
        ['editor:b.md', { path: 'b.md', kind: 'editor', scrollTop: 540 }],
      ]),
      modes: new Map([['a.md', 'preview']]),
    }),
  }).flush('quit')

  let restored = null
  createReadPositionPersistence({ storage, onRestore: (p) => { restored = p } }).load()
  const restoreCache = createReadPositionCache()
  for (const item of restored.positions) restoreCache.remember(item)

  const modeKey = restored.modes['a.md'] || 'edit'
  assert.equal(modeKey, 'preview')
  const plan = planReadPositionRestore({ modeKey, path: 'a.md', modeChanging: true })
  assert.deepEqual(plan.carriers, ['markdown'])
  assert.equal(restoreCache.get(readPositionKey('a.md', 'markdown')).anchorLine, 77)
})

test('重启：用户主动回到顶部这一状态也会被记住', () => {
  const storage = createFakeStorage()
  createReadPositionPersistence({
    storage,
    readSources: () => ({
      positions: new Map([['markdown:a.md', { path: 'a.md', kind: 'markdown', scrollTop: 0, anchorLine: 0 }]]),
      modes: new Map([['a.md', 'preview']]),
    }),
  }).flush('quit')
  let restored = null
  createReadPositionPersistence({ storage, onRestore: (p) => { restored = p } }).load()
  assert.equal(restored.positions.length, 1, '顶部位置应被落盘')
  assert.equal(restored.positions[0].scrollTop, 0)
})

test('重启：落盘条数有上限，不会把 localStorage 撑爆', () => {
  const positions = new Map()
  for (let i = 0; i < 500; i += 1) {
    positions.set(`markdown:f${i}.md`, { path: `f${i}.md`, kind: 'markdown', scrollTop: i })
  }
  const payload = buildPersistPayload({ positions, limit: 160 })
  assert.equal(payload.positions.length, 160)
  assert.equal(payload.positions[0].path, 'f0.md', '按 LRU 顺序保留最近的记录')
})

test('重启：同一个 key 落盘不会重复（避免快照随滚动不断变大）', () => {
  const storage = createFakeStorage()
  const positions = new Map([['markdown:a.md', { path: 'a.md', kind: 'markdown', scrollTop: 10 }]])
  const persistence = createReadPositionPersistence({ storage, readSources: () => ({ positions, modes: new Map() }) })
  persistence.flush('one')
  positions.set('markdown:a.md', { path: 'a.md', kind: 'markdown', scrollTop: 20 })
  persistence.flush('two')
  const saved = JSON.parse(storage._data.get(READ_POSITION_STORAGE_KEY))
  assert.equal(saved.positions.length, 1)
  assert.equal(saved.positions[0].scrollTop, 20)
})
