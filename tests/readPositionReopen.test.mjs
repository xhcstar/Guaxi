// 「重启后重开上次文档」的回归测试。
//
// 位置存下来了但启动时没开任何文档，等于仍然记不住——这是用户报「重新打开软件记不住」
// 的第二个原因。这里锁住持久化层对 lastPath 的读写语义；「什么时候允许重开」的判定
// 在组件里（见 GuaxiEditor.maybeReopenLastDocument），规则同样写进了 readPositionCore 的
// 文档里，便于人工核对。
import test from 'node:test'
import assert from 'node:assert/strict'

import { createReadPositionPersistence, READ_POSITION_STORAGE_KEY } from '../src/components/HomeViewComponent/readPositionStore.js'

const createFakeStorage = (initial = {}) => {
  const data = new Map(Object.entries(initial))
  return {
    getItem: (k) => (data.has(k) ? data.get(k) : null),
    setItem: (k, v) => data.set(k, String(v)),
    removeItem: (k) => data.delete(k),
    _data: data,
  }
}

test('重开：lastPath 随快照落盘并在下次启动读回', () => {
  const storage = createFakeStorage()
  createReadPositionPersistence({
    storage,
    readSources: () => ({ positions: new Map(), modes: new Map(), lastPath: 'D:/notes/a.md' }),
  }).flush('quit')

  let restored = null
  createReadPositionPersistence({ storage, onRestore: (p) => { restored = p } }).load()
  assert.equal(restored.lastPath, 'D:/notes/a.md')
})

test('重开：只有 lastPath 没有位置时也要写盘（用户刚打开还没滚动就退出）', () => {
  const storage = createFakeStorage()
  const wrote = createReadPositionPersistence({
    storage,
    readSources: () => ({ positions: new Map(), modes: new Map(), lastPath: 'a.md' }),
  }).flush('quit')
  assert.equal(wrote, true)
  assert.ok(storage._data.has(READ_POSITION_STORAGE_KEY))
})

test('重开：lastPath 为空字符串时不影响「本来就没东西可存」的判断', () => {
  const storage = createFakeStorage()
  const wrote = createReadPositionPersistence({
    storage,
    readSources: () => ({ positions: new Map(), modes: new Map(), lastPath: '' }),
  }).flush('empty')
  assert.equal(wrote, false)
})

test('重开：旧快照没有 lastPath 字段时读回空串，不会抛错', () => {
  const storage = createFakeStorage({
    [READ_POSITION_STORAGE_KEY]: JSON.stringify({ version: 1, positions: [], modes: {} }),
  })
  const parsed = createReadPositionPersistence({ storage }).load()
  assert.equal(parsed.lastPath, '')
})
