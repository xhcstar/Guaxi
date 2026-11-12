// 编辑器阅读位置保持的回归测试。
//
// 背景：这套机制有五个载体（Monaco / 预览容器 / notebook / Vditor / 分屏两侧），
// 每个载体各自的保存与恢复时机都不同，而 bug 的形态又高度一致——
// **切回文件后回到开头**。它只在特定顺序下复现（例如"从编辑模式的 A 切到预览模式的 B"），
// 靠人工点界面很难穷举，所以把决策逻辑抽到 readPositionCore.js 并在 Node 里测。
//
// 覆盖两类问题：
//   1. 跨页面（home ↔ chat）往返后位置仍在；
//   2. 跨文件、跨模式切换后，位置落在正确的载体上。
import test from 'node:test'
import assert from 'node:assert/strict'

import {
  normalizeReadPath,
  previewReadKindForPath,
  readPositionKey,
  normalizeModeKey,
  expectedKeysForMode,
  createReadPositionCache,
  planReadPositionRestore,
  shouldRestoreEntry,
  resolveScrollTarget,
  READ_POSITION_CACHE_LIMIT,
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

// ───────────────────────────── 路径与 key

test('Windows 与 POSIX 路径归一到同一个 key', () => {
  const a = readPositionKey('D:\\Record\\note.md', 'markdown')
  const b = readPositionKey('D:/Record/note.md', 'markdown')
  assert.equal(a, b)
  assert.equal(a, 'markdown:D:/Record/note.md')
})

test('同一文件在不同载体上是不同的 key', () => {
  // 这是分屏模式的基础：编辑器侧与预览侧各存一份，不能互相覆盖。
  const editorKey = readPositionKey('a.md', 'editor')
  const previewKey = readPositionKey('a.md', 'markdown')
  assert.notEqual(editorKey, previewKey)
})

test('后缀决定预览载体：tex/typ 走 pdf，md 走 markdown，org 走 org', () => {
  assert.equal(previewReadKindForPath('doc.tex'), 'pdf')
  assert.equal(previewReadKindForPath('doc.typ'), 'pdf')
  assert.equal(previewReadKindForPath('doc.md'), 'markdown')
  assert.equal(previewReadKindForPath('doc.markdown'), 'markdown')
  assert.equal(previewReadKindForPath('doc.org'), 'org')
  // 普通文本没有独立预览面
  assert.equal(previewReadKindForPath('doc.txt'), '')
  assert.equal(previewReadKindForPath('doc.py'), '')
})

test('空路径不产生 key，避免写到空 key 上', () => {
  assert.equal(readPositionKey('', 'editor'), '')
  assert.equal(readPositionKey('   ', 'editor'), '')
})

test('无法识别的模式键落到 edit', () => {
  assert.equal(normalizeModeKey('preview'), 'preview')
  assert.equal(normalizeModeKey('SPLIT'), 'split')
  assert.equal(normalizeModeKey('nonsense'), 'edit')
  assert.equal(normalizeModeKey(''), 'edit')
})

// ───────────────────────────── 每种模式该存哪些位置

test('五种模式各自的期望载体', () => {
  assert.deepEqual(expectedKeysForMode('edit', 'a.md'), ['editor:a.md'])
  assert.deepEqual(expectedKeysForMode('preview', 'a.md'), ['markdown:a.md'])
  assert.deepEqual(expectedKeysForMode('preview', 'a.tex'), ['pdf:a.tex'])
  assert.deepEqual(expectedKeysForMode('explorer', 'a.md'), ['explorer:a.md'])
  assert.deepEqual(expectedKeysForMode('realtime', 'a.md'), ['realtime:a.md'])
  // 分屏两侧都要
  assert.deepEqual(expectedKeysForMode('split', 'a.md'), ['editor:a.md', 'markdown:a.md'])
  assert.deepEqual(expectedKeysForMode('split', 'a.tex'), ['editor:a.tex', 'pdf:a.tex'])
})

test('无独立预览面的文件在预览模式下记到 markdown 载体', () => {
  // 普通文本在预览模式里仍由 markdown 渲染器出图，位置只能记在它上面。
  // 如果这里返回空数组，切回该文件时会因为"没有期望 key"而完全跳过恢复。
  assert.deepEqual(expectedKeysForMode('preview', 'notes.txt'), ['markdown:notes.txt'])
  assert.deepEqual(expectedKeysForMode('split', 'notes.txt'), ['editor:notes.txt', 'markdown:notes.txt'])
})

// ───────────────────────────── 问题 2：跨文件 + 跨模式的恢复载体

test('问题2：从编辑模式的 A 切到预览模式的 B，恢复目标是预览容器而不是编辑器', () => {
  // 这正是用户报的场景：A 在编辑模式，B 记忆为预览模式。
  // 若按"当前模式(edit)"算载体，会把 B 的预览位置写进即将隐藏的编辑器。
  const plan = planReadPositionRestore({ modeKey: 'preview', path: 'B.md', modeChanging: true })
  assert.deepEqual(plan.carriers, ['markdown'])
  // 且必须等模式切换完成后才落位
  assert.equal(plan.deferUntilAfterModeSwitch, true)
})

test('同一文件在模式不变时不需要推迟落位', () => {
  const plan = planReadPositionRestore({ modeKey: 'preview', path: 'B.md', modeChanging: false })
  assert.deepEqual(plan.carriers, ['markdown'])
  assert.equal(plan.deferUntilAfterModeSwitch, false)
})

test('分屏恢复两个载体，顺序固定为编辑器在前', () => {
  const plan = planReadPositionRestore({ modeKey: 'split', path: 'a.tex', modeChanging: true })
  // 编辑器先恢复：它不会因为预览面板的 PDF 还在编译而被重置。
  assert.deepEqual(plan.carriers, ['editor', 'pdf'])
})

test('探索者与即时渲染各自恢复自己的载体', () => {
  assert.deepEqual(planReadPositionRestore({ modeKey: 'explorer', path: 'a.md', modeChanging: false }).carriers, ['explorer'])
  assert.deepEqual(planReadPositionRestore({ modeKey: 'realtime', path: 'a.md', modeChanging: false }).carriers, ['realtime'])
})

test('无独立预览面的文件在预览模式下仍恢复 markdown 载体', () => {
  const plan = planReadPositionRestore({ modeKey: 'preview', path: 'a.txt', modeChanging: false })
  assert.deepEqual(plan.carriers, ['markdown'])
})

// ───────────────────────────── 问题 1：跨页面往返

test('问题1：切到别的页面再切回，五个模式的位置都还在', () => {
  // 模拟一次完整往返：每个文件在自己模式下存一份 → 离开页面 → 回来后按模式取回。
  const cache = createReadPositionCache()
  const files = [
    { path: 'm.md', mode: 'edit', top: 900 },
    { path: 'p.md', mode: 'preview', top: 1500 },
    { path: 't.tex', mode: 'preview', top: 420 },
    { path: 's.md', mode: 'split', top: 300 },
  ]
  for (const f of files) {
    for (const key of expectedKeysForMode(f.mode, f.path)) {
      const kind = key.split(':')[0]
      cache.remember({ path: f.path, kind, scrollTop: f.top })
    }
  }

  // 离开页面（组件被 KeepAlive 移出文档）不应清空缓存
  for (const f of files) {
    for (const key of expectedKeysForMode(f.mode, f.path)) {
      assert.ok(cache.has(key), `往返后 ${key} 应仍在缓存里`)
      assert.equal(cache.get(key).scrollTop, f.top)
    }
  }
})

test('切走时只存当前文件的当前位置，不会把别的文件位置覆盖掉', () => {
  const cache = createReadPositionCache()
  cache.remember({ path: 'A.md', kind: 'editor', scrollTop: 100 })
  cache.remember({ path: 'B.md', kind: 'editor', scrollTop: 800 })

  // 在 A 上滚动后再次保存
  cache.remember({ path: 'A.md', kind: 'editor', scrollTop: 250 })

  assert.equal(cache.get(readPositionKey('A.md', 'editor')).scrollTop, 250)
  assert.equal(cache.get(readPositionKey('B.md', 'editor')).scrollTop, 800, 'B 不受影响')
})

// ───────────────────────────── 缓存本身

test('缓存是有界的，超限淘汰最久未用的', () => {
  const cache = createReadPositionCache(3)
  cache.remember({ path: 'a.md', kind: 'editor', scrollTop: 1 })
  cache.remember({ path: 'b.md', kind: 'editor', scrollTop: 2 })
  cache.remember({ path: 'c.md', kind: 'editor', scrollTop: 3 })
  // 访问 a，把它挪到最近使用
  cache.remember({ path: 'a.md', kind: 'editor', scrollTop: 1 })
  cache.remember({ path: 'd.md', kind: 'editor', scrollTop: 4 })

  assert.equal(cache.size, 3)
  assert.ok(cache.has(readPositionKey('a.md', 'editor')), 'a 刚被访问过，应保留')
  assert.ok(!cache.has(readPositionKey('b.md', 'editor')), 'b 是最久未用的，应被淘汰')
})

test('默认上限与组件里的常量一致', () => {
  const cache = createReadPositionCache()
  for (let i = 0; i < READ_POSITION_CACHE_LIMIT + 20; i += 1) {
    cache.remember({ path: `f${i}.md`, kind: 'editor', scrollTop: i })
  }
  assert.equal(cache.size, READ_POSITION_CACHE_LIMIT)
})

test('写入无效路径不会污染缓存', () => {
  const cache = createReadPositionCache()
  cache.remember({ path: '', kind: 'editor', scrollTop: 10 })
  cache.remember({ path: 'a.md', kind: '', scrollTop: 10 })
  assert.equal(cache.size, 0)
})

// ───────────────────────────── 恢复前的校验

test('不会把 A 的位置写给 B', () => {
  const entry = { path: 'A.md', kind: 'editor', scrollTop: 500 }
  assert.equal(shouldRestoreEntry(entry, 'A.md'), true)
  assert.equal(shouldRestoreEntry(entry, 'B.md'), false)
})

test('路径写法不同的同一文件仍认作同一份', () => {
  const entry = { path: 'D:\\x\\a.md', kind: 'editor', scrollTop: 500 }
  assert.equal(shouldRestoreEntry(entry, 'D:/x/a.md'), true)
})

test('空记录与空路径都拒绝恢复', () => {
  assert.equal(shouldRestoreEntry(null, 'a.md'), false)
  assert.equal(shouldRestoreEntry({ path: 'a.md' }, ''), false)
})

test('顶部（scrollTop 0）是合法位置，会被如实恢复', () => {
  // 用户主动回到开头后，切走再回来应该还在开头，不能被更早的旧位置拽回去。
  const entry = { path: 'a.md', kind: 'editor', scrollTop: 0 }
  assert.equal(shouldRestoreEntry(entry, 'a.md'), true)
  const target = resolveScrollTarget(entry, { scrollHeight: 5000, clientHeight: 800 })
  assert.equal(target.top, 0)
  assert.equal(target.source, 'pixels')
})

// ───────────────────────────── 滚动目标换算

test('像素在可滚动范围内时优先用像素', () => {
  const t = resolveScrollTarget(
    { scrollTop: 1200, scrollRatio: 0.9 },
    { scrollHeight: 4000, clientHeight: 800 },
  )
  assert.equal(t.top, 1200)
  assert.equal(t.source, 'pixels')
})

test('文档变矮导致像素越界时退回比例', () => {
  // 典型场景：切走时文档 8000px 高、位置 6000；回来时图片没加载完只有 2000px。
  const t = resolveScrollTarget(
    { scrollTop: 6000, scrollRatio: 0.75 },
    { scrollHeight: 2000, clientHeight: 500 },
  )
  assert.equal(t.source, 'ratio')
  assert.equal(t.top, Math.round(0.75 * 1500))
})

test('没有可用位置时落到顶部而不是抛错', () => {
  const t = resolveScrollTarget(null, { scrollHeight: 2000, clientHeight: 500 })
  assert.equal(t.top, 0)
  assert.equal(t.source, 'none')
})

test('内容比容器还短时目标为 0，不会算出负值', () => {
  const t = resolveScrollTarget(
    { scrollTop: 300, scrollRatio: 0.2 },
    { scrollHeight: 200, clientHeight: 500 },
  )
  assert.equal(t.top, 0)
})

test('比例兜底只在确实有可滚动空间时生效', () => {
  // maxTop = 0（内容刚好填满）时，比例没有意义，必须给 0。
  const t = resolveScrollTarget({ scrollTop: 999, scrollRatio: 0.5 }, { scrollHeight: 800, clientHeight: 800 })
  assert.equal(t.top, 0)
})
