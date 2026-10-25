import test from 'node:test'
import assert from 'node:assert/strict'

import { buildLines } from '../src/components/PdfViewer/usePdfTextLayer.js'

// 回归测试：PDF 文本层按行生成透明 span。双栏/表格的左右两列常常共享同一基线，
// 若并进同一个 span，浏览器会把「两列之间的大段空白」也涂进选区 ——
// 表现为划一个词却出现满屏蓝色方块。

const ch = (c, x, y = 0, w = 6, h = 10, fs = 10) => ({
  c,
  x,
  y,
  w,
  h,
  font_size: fs,
})

test('同一基线的左右双栏拆成两个 span', () => {
  const lines = buildLines([ch('A', 0), ch('B', 6), ch('C', 200), ch('D', 206)])
  assert.equal(lines.length, 2, '两栏必须各自成段')
  assert.equal(lines[0].text, 'AB')
  assert.equal(lines[1].text, 'CD')
  assert.ok(lines[1].leftPt > 100)
})

test('普通词间距仍合并为一行并补空格', () => {
  const lines = buildLines([ch('A', 0), ch('B', 12)]) // 间隙 6pt = 0.6em
  assert.equal(lines.length, 1)
  assert.match(lines[0].text, /^A\s+B$/, `实际: ${lines[0].text}`)
})

test('表格列距也会拆开，避免选区跨列', () => {
  assert.equal(buildLines([ch('1', 0), ch('2', 60)]).length, 2)
})

test('不同基线的行仍然分开', () => {
  const lines = buildLines([ch('上', 0, 0), ch('下', 0, 40)])
  assert.equal(lines.length, 2)
  assert.equal(lines[0].text, '上')
  assert.equal(lines[1].text, '下')
})

test('空输入返回空数组', () => {
  assert.deepEqual(buildLines([]), [])
  assert.deepEqual(buildLines(null), [])
})
