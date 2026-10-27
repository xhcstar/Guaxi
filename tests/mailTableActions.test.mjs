import test from 'node:test'
import assert from 'node:assert/strict'
import {
  cellPosition,
  collectRows,
  createCellHtml,
  insertionRowAfterInsert,
  TABLE_STYLES,
} from '../src/components/ApplicationViewComponent/apps/mail/tableActions.js'

// 表格右键菜单的行列逻辑。这些边界情况最容易写错：
// 表头保护、越界插入、嵌套表格干扰。

/** 造一个最小的 DOM 桩：只实现表格操作用到的那几个属性。 */
function makeTable(rowsSpec) {
  const table = { rows: [] }
  rowsSpec.forEach((cells, rowIndex) => {
    const row = {
      cells: [],
      children: [],
      closest: (sel) => (sel === 'thead' && cells.header ? {} : null),
    }
    cells.cells.forEach((tag) => {
      const cell = { tagName: tag.toUpperCase(), row, index: 0 }
      row.cells.push(cell)
      row.children.push(cell)
    })
    row.cells.forEach((c, i) => (c.index = i))
    table.rows.push(row)
  })
  return table
}

test('定位光标所在的单元格', () => {
  const table = makeTable([
    { cells: ['th', 'th'], header: true },
    { cells: ['td', 'td'] },
  ])
  const pos = cellPosition(table, table.rows[1].cells[1])
  assert.deepEqual(pos, { row: 1, col: 1, rows: 2, cols: 2 })
})

test('嵌套表格不会串行：只取直接子单元格', () => {
  const table = makeTable([{ cells: ['td', 'td'] }])
  const rows = collectRows(table)
  assert.equal(rows.length, 1)
  assert.equal(rows[0].cells.length, 2, '嵌套表格里的 td 不应被算进来')
})

test('表头行被识别出来', () => {
  const table = makeTable([
    { cells: ['th'], header: true },
    { cells: ['td'] },
  ])
  const rows = collectRows(table)
  assert.equal(rows[0].isHeader, true)
  assert.equal(rows[1].isHeader, false)
})

test('「在上方插入」不会把表头挤走', () => {
  const rows = [{ isHeader: true }, { isHeader: false }]
  // 光标在表头行，向上插入 → 落在表头之后（下标 1）
  assert.equal(insertionRowAfterInsert(rows, 0, 'above'), 1)
  // 光标在普通行，正常插在它前面
  assert.equal(insertionRowAfterInsert(rows, 1, 'above'), 1)
})

test('「在下方插入」越界时夹到最后一行之后', () => {
  const rows = [{ isHeader: true }, { isHeader: false }]
  assert.equal(insertionRowAfterInsert(rows, 1, 'below'), 2)
  assert.equal(insertionRowAfterInsert(rows, 99, 'below'), 2)
})

test('新单元格带占位，避免客户端里塌掉', () => {
  assert.equal(createCellHtml('td'), '<td><br></td>')
  assert.equal(createCellHtml('th', '标题'), '<th>标题</th>')
})

test('表格样式都是邮件客户端可用的写法', () => {
  assert.ok(TABLE_STYLES.length >= 3)
  for (const style of TABLE_STYLES) {
    if (style.headerBg) assert.match(style.headerBg, /^#[0-9a-f]{6}$/i, '底色要是 #hex')
    assert.ok(style.key && style.label)
  }
  // 无边框那档不能还画着线
  const plain = TABLE_STYLES.find((s) => s.key === 'plain')
  assert.equal(plain.borderStyle, 'none')
})

test('空表格 / 空单元格不会抛错', () => {
  assert.equal(cellPosition(null, null), null)
  assert.deepEqual(collectRows(null), [])
  const table = makeTable([])
  assert.equal(cellPosition(table, null), null)
})
