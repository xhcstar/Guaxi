import test from 'node:test'
import assert from 'node:assert/strict'

import { buildGitFileTreeRows } from '../src/components/HomeViewComponent/utils/gitFileTree.js'

// 树形分组只负责“把扁平路径展开成可渲染的行序列”，不涉及 Vue。
// 这里守住三个容易退化的性质：目录先于文件、折叠后子项消失、重名目录不会丢文件。

const files = [
  { path: 'README.md', staged: false },
  { path: 'src/main.rs', staged: false },
  { path: 'src/commands/git.rs', staged: false },
  { path: 'src/commands/mail.rs', staged: true },
  { path: 'docs/guide.md', staged: false },
]

test('树形模式：目录排在文件前，并带上层级', () => {
  const rows = buildGitFileTreeRows(files, { mode: 'tree', prefix: 't' })

  assert.deepEqual(
    rows.map((row) => `${row.depth}:${row.type}:${row.type === 'folder' ? row.name : row.file.path}`),
    [
      '0:folder:docs',
      '1:file:docs/guide.md',
      '0:folder:src',
      '1:folder:commands',
      '2:file:src/commands/git.rs',
      '2:file:src/commands/mail.rs',
      '1:file:src/main.rs',
      '0:file:README.md',
    ],
  )
})

test('折叠目录后不再产生子行', () => {
  const rows = buildGitFileTreeRows(files, {
    mode: 'tree',
    prefix: 't',
    collapsed: new Set(['src']),
  })
  const paths = rows.map((row) => row.path || row.file?.path)
  assert.ok(paths.includes('src'))
  assert.ok(!paths.includes('src/main.rs'))
  assert.ok(!paths.includes('src/commands'))
  assert.equal(rows.find((row) => row.path === 'src').count, 3)
})

test('列表模式按完整路径排序且没有目录行', () => {
  const rows = buildGitFileTreeRows(files, { mode: 'list', prefix: 't' })
  assert.ok(rows.every((row) => row.type === 'file'))
  assert.deepEqual(
    rows.map((row) => row.file.path),
    ['docs/guide.md', 'README.md', 'src/commands/git.rs', 'src/commands/mail.rs', 'src/main.rs'],
  )
})

test('行 key 在同名文件分处不同分组时保持唯一', () => {
  const rows = buildGitFileTreeRows(files, { mode: 'tree', prefix: 'work' })
  const keys = rows.map((row) => row.key)
  assert.equal(new Set(keys).size, keys.length)
})
