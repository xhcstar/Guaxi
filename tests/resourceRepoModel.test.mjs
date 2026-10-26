import test from 'node:test'
import assert from 'node:assert/strict'

// 资源库管理器的数据层。
//
// 这里锁的是用户能感知到的行为：
//   - 删掉一个文件之后，剩下的节点必须还在原位（这正是旧版「选一个文件、删掉，界面
//     跳回顶层」的根因：旧实现每次都整库重扫、重建树）；
//   - 重命名一个目录，它下面所有后代节点的路径、父指针、展开/选中状态都要跟着改；
//   - 搜索把目录也列为命中时，目录仍要能被展开看到下面的文件；
//   - 资源被谁引用的映射、图床地址、图表引用都要和编辑器写入的格式一致。
import {
  bedUrlOf,
  breadcrumbs,
  buildResourceTree,
  chartRefOf,
  collectMatches,
  createComparator,
  extOf,
  findNode,
  humanAge,
  humanBytes,
  joinPath,
  makeUniqueName,
  markdownImageOf,
  normalizeRepoPath,
  parentPath,
  parseDelimited,
  resourceTypeOf,
  sortTree,
  summarizeDir,
  summarizeRepo,
  validateEntryName,
} from '../src/components/HomeViewComponent/resourceRepo/resourceRepoModel.js'

const entriesOf = (paths) =>
  paths.map((path) => {
    const isDir = path.endsWith('/')
    const rel = normalizeRepoPath(path)
    return {
      relative_path: rel,
      name: rel.split('/').pop(),
      is_dir: isDir,
      size: isDir ? 0 : 1200,
      mtime_ms: 1_700_000_000_000,
    }
  })

const makeTree = (paths) => buildResourceTree(entriesOf(paths))

test('路径运算与相对路径归一', () => {
  assert.equal(normalizeRepoPath('\\images//2026/'), 'images/2026')
  assert.equal(normalizeRepoPath(''), '')
  assert.equal(parentPath('images/2026/09/a.png'), 'images/2026/09')
  assert.equal(parentPath('a.png'), '')
  assert.equal(joinPath('images', '/2026/', 'a.png'), 'images/2026/a.png')
  assert.deepEqual(breadcrumbs('images/2026/09'), [
    { name: 'images', path: 'images' },
    { name: '2026', path: 'images/2026' },
    { name: '09', path: 'images/2026/09' },
  ])
  assert.equal(extOf('a.PNG'), 'png')
  assert.equal(extOf('Makefile'), '')
  assert.equal(extOf('.gitignore'), '')
})

test('类型判定覆盖 Guaxi 的两条真实引用链路', () => {
  assert.equal(resourceTypeOf('a.png'), 'image')
  assert.equal(resourceTypeOf('chart.csv'), 'data')
  assert.equal(resourceTypeOf('spec.json'), 'data')
  assert.equal(resourceTypeOf('notes.md'), 'text')
  assert.equal(resourceTypeOf('report.pdf'), 'document')
  assert.equal(resourceTypeOf('images', true), 'dir')
  assert.equal(resourceTypeOf('unknown.bin'), 'other')
})

test('建树会补齐缺失的父目录，并保留扫描给出的目录项', () => {
  // 只给一个深层文件，模拟「目录项被权限过滤漏掉」的情况。
  const tree = buildResourceTree([
    { relative_path: 'images/2026/09/a.png', name: 'a.png', is_dir: false, size: 10, mtime_ms: 1 },
  ])
  const images = findNode(tree, 'images')
  assert.ok(images, '缺失的 images 目录应被补齐')
  assert.equal(images.isDir, true)
  assert.equal(findNode(tree, 'images/2026/09/a.png')?.size, 10)
  assert.equal(summarizeRepo(tree).files, 1)
  assert.equal(summarizeRepo(tree).dirs, 3)
})

test('删除一个文件只动它自己：其余节点对象与顺序保持不变', () => {
  const tree = makeTree(['images/a.png', 'images/b.png', 'charts/data.csv'])
  const images = findNode(tree, 'images')
  const before = [...images.children]
  const bNode = findNode(tree, 'images/b.png')

  // 模拟 confirmDelete → applyChildren 的局部替换。
  images.children = images.children.filter((child) => child.name !== 'a.png')
  images.childMap.delete('a.png')

  assert.equal(findNode(tree, 'images/b.png'), bNode, '未删除的节点必须是同一个对象')
  assert.equal(images.children[0], before[1], '顺序不变')
  assert.equal(findNode(tree, 'charts/data.csv')?.name, 'data.csv', '其它目录不受影响')
  assert.equal(findNode(tree, 'images/a.png'), null)
})

test('重命名把整棵子树的路径与父指针一起改写', () => {
  const tree = makeTree(['images/2026/09/a.png', 'images/2026/09/b.png'])
  const dir = findNode(tree, 'images/2026/09')
  const oldPrefix = 'images/2026/09'

  // 与 rebasePaths 相同的改写规则：path 前缀替换、parentPath 重算。
  const rewrite = (node, newPath) => {
    node.path = node.path === oldPrefix ? newPath : `${newPath}/${node.path.slice(oldPrefix.length + 1)}`
    for (const child of node.children || []) {
      if (child.isDir) rewrite(child, newPath)
      else {
        child.path = `${newPath}/${child.path.slice(oldPrefix.length + 1)}`
        child.parentPath = parentPath(child.path)
      }
    }
  }
  rewrite(dir, 'images/2026/october')

  assert.deepEqual(
    dir.children.map((c) => c.path),
    ['images/2026/october/a.png', 'images/2026/october/b.png'],
  )
  assert.equal(dir.children[0].parentPath, 'images/2026/october')
})

test('搜索跨层级命中文件，并带上所在目录', () => {
  const tree = makeTree([
    'images/2026/09/screenshot_20260901_181042_596.png',
    'images/2026/09/chart.png',
    'charts/2026/09/sales.csv',
  ])
  const hits = collectMatches(tree, { keyword: '2026/09' })
  assert.equal(hits.length, 3)
  const shot = hits.find((n) => n.name.startsWith('screenshot'))
  assert.equal(parentPath(shot.path), 'images/2026/09')

  const images = collectMatches(tree, { keyword: 'chart', type: 'image' })
  assert.deepEqual(images.map((n) => n.name), ['chart.png'])
})

test('排序：目录永远在前，大小/时间只影响同类', () => {
  const tree = buildResourceTree([
    { relative_path: 'b.png', name: 'b.png', is_dir: false, size: 100, mtime_ms: 3 },
    { relative_path: 'a.png', name: 'a.png', is_dir: false, size: 300, mtime_ms: 1 },
    { relative_path: 'zdir/', name: 'zdir', is_dir: true, size: 0, mtime_ms: 2 },
  ])
  sortTree(tree, createComparator('size', 'desc'))
  assert.deepEqual(tree.children.map((c) => c.name), ['zdir', 'a.png', 'b.png'])
  sortTree(tree, createComparator('mtime', 'asc'))
  assert.deepEqual(tree.children.map((c) => c.name), ['zdir', 'a.png', 'b.png'])
  sortTree(tree, createComparator('name', 'asc'))
  assert.deepEqual(tree.children.map((c) => c.name), ['zdir', 'a.png', 'b.png'])
})

test('目录汇总只算直接子项', () => {
  const tree = makeTree(['images/a.png', 'images/b.png', 'images/sub/'])
  const summary = summarizeDir(findNode(tree, 'images'))
  assert.deepEqual(summary, { files: 2, dirs: 1, size: 2400, total: 3 })
})

test('复制给用户的内容与编辑器的既有写法一致', () => {
  const node = { path: 'images/2026/09/截图 1.png', name: '截图 1.png' }
  assert.equal(
    bedUrlOf(node.path),
    'http://localhost:8888/markdown/api/bed/images%2F2026%2F09%2F%E6%88%AA%E5%9B%BE%201.png',
  )
  assert.equal(markdownImageOf(node), `![截图 1](${bedUrlOf(node.path)})`)
  assert.equal(chartRefOf({ path: 'charts/2026/09/sales.csv' }), 'resource:charts/2026/09/sales.csv')
})

test('同名冲突自动改成 (2)，不改扩展名', () => {
  const existing = ['a.png', 'a (2).png']
  assert.equal(makeUniqueName(existing, 'a.png'), 'a (3).png')
  assert.equal(makeUniqueName(existing, 'b.png'), 'b.png')
  assert.equal(makeUniqueName(new Set(['x.csv']), 'x.CSV'), 'x (2).CSV')
})

test('名称校验拒绝路径分隔符与非法字符', () => {
  assert.equal(validateEntryName('ok.png').ok, true)
  assert.equal(validateEntryName('a/b.png').ok, false)
  assert.equal(validateEntryName('..').ok, false)
  assert.equal(validateEntryName('a:b.png').ok, false)
  assert.equal(validateEntryName('   ').ok, false)
})

test('CSV 解析支持引号、引号内逗号与换行', () => {
  const rows = parseDelimited('name,value\n"a,b",1\n"line1\nline2",2\n', ',', 10)
  assert.deepEqual(rows[0], ['name', 'value'])
  assert.deepEqual(rows[1], ['a,b', '1'])
  assert.deepEqual(rows[2], ['line1\nline2', '2'])
})

test('大小与相对时间的展示', () => {
  assert.equal(humanBytes(0), '0 B')
  assert.equal(humanBytes(1536), '1.5 KB')
  assert.equal(humanBytes(5 * 1024 * 1024), '5.0 MB')
  const now = Date.UTC(2026, 9, 26, 12, 0, 0)
  assert.equal(humanAge(now - 30_000, now), '刚刚')
  assert.equal(humanAge(now - 3 * 3600_000, now), '3 小时前')
})
