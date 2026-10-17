import test from 'node:test'
import assert from 'node:assert/strict'

import {
  normalizePathKey,
  isSameFilePath,
  isPathWithin,
  resolveModelBoundPath,
  shouldApplyContentToActiveView,
  shouldPersistContentForPath,
} from '../src/utils/fileContentAttribution.js'

test('normalizePathKey 统一分隔符并去掉末尾斜杠', () => {
  assert.equal(normalizePathKey('D:\\Notes\\Test\\README.md'), 'D:/Notes/Test/README.md')
  assert.equal(normalizePathKey('D:/Notes/Test/'), 'D:/Notes/Test')
  assert.equal(normalizePathKey(''), '')
})

test('isSameFilePath 忽略分隔符差异', () => {
  assert.equal(isSameFilePath('D:\\Notes\\a.md', 'D:/Notes/a.md'), true)
  assert.equal(isSameFilePath('D:/Notes/a.md', 'D:/Notes/b.md'), false)
  assert.equal(isSameFilePath('D:/Notes/README.md', 'D:/Notes/readme.md'), false, '大小写不同应视为不同文件')
})

test('isPathWithin 覆盖目录自身与子文件', () => {
  assert.equal(isPathWithin('D:/Notes/Test/README.md', 'D:/Notes/Test'), true)
  assert.equal(isPathWithin('D:/Notes/Test', 'D:/Notes/Test'), true)
  assert.equal(isPathWithin('D:/Notes/Testing/a.md', 'D:/Notes/Test'), false, '同前缀但不是子路径')
})

test('resolveModelBoundPath 优先于 activeTab：切档瞬间内容仍归属旧文件', () => {
  const modelUriToPath = new Map([
    ['file:///D:/Notes/README.md', 'D:/Notes/README.md'],
    ['file:///D:/Notes/bow.md', 'D:/Notes/bow.md'],
  ])

  // 场景：store 已切到 bow.md，但编辑器 model 还绑着 README.md。
  const bound = resolveModelBoundPath({
    modelUriToPath,
    modelUri: 'file:///D:/Notes/README.md',
    fallbackPath: 'D:/Notes/bow.md',
  })
  assert.equal(bound, 'D:/Notes/README.md')
})

test('resolveModelBoundPath 在映射缺失时回退到调用方给出的路径', () => {
  const bound = resolveModelBoundPath({
    modelUriToPath: new Map(),
    modelUri: 'file:///unknown',
    fallbackPath: 'D:/Notes/fallback.md',
  })
  assert.equal(bound, 'D:/Notes/fallback.md')
})

test('shouldApplyContentToActiveView：旧文件内容不能写进新文件视图', () => {
  // 正常切换：内容与当前文件一致 → 允许写回
  assert.equal(shouldApplyContentToActiveView('D:/Notes/a.md', 'D:/Notes/a.md'), true)
  assert.equal(shouldApplyContentToActiveView('D:\\Notes\\a.md', 'D:/Notes/a.md'), true)

  // 切档竞态：flush 出来的是 README 的内容，而当前文件已经是 bow.md → 必须拒绝
  assert.equal(shouldApplyContentToActiveView('D:/Notes/README.md', 'D:/Notes/bow.md'), false)

  // 没有归属路径时保持旧行为（允许写回）
  assert.equal(shouldApplyContentToActiveView('', 'D:/Notes/a.md'), true)
})

test('shouldPersistContentForPath：已删除/改名的路径不再写盘', () => {
  const openPaths = ['D:/Notes/bow.md']

  // 旧文件已经不在打开列表（删除/改名）→ 不允许写回旧路径
  assert.equal(shouldPersistContentForPath({ targetPath: 'D:/Notes/README.md', openPaths }), false)

  // 目录整体被删除/改名：其下所有路径都不允许写回
  assert.equal(shouldPersistContentForPath({
    targetPath: 'D:/Notes/Test/README.md',
    openPaths: ['D:/Notes/Test/README.md'],
    removedRoot: 'D:/Notes/Test',
  }), false)

  // 正常打开中的文件仍允许写盘
  assert.equal(shouldPersistContentForPath({ targetPath: 'D:/Notes/bow.md', openPaths }), true)

  // 空路径永远不写
  assert.equal(shouldPersistContentForPath({ targetPath: '', openPaths }), false)
})
