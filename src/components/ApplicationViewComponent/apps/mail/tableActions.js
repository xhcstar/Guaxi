/**
 * 编辑器里表格的快捷操作。
 *
 * 用户在 Outlook 里的习惯是：**右键表格 → 插入/删除行列、合并单元格、改底纹**。
 * 这里只做「算目标位置」这类纯逻辑，DOM 操作放在 MailComposer 里——
 * 这样行列索引的边界情况可以用 node --test 锁住（越界、表头保护等）。
 */

/**
 * 把光标位置换算成「第几行第几列」。
 *
 * @param {HTMLElement|null} table
 * @param {HTMLElement|null} cell  光标所在的 td / th
 * @returns {{row:number, col:number, rows:number, cols:number}|null}
 */
export function cellPosition(table, cell) {
  if (!table || !cell) return null
  const rows = collectRows(table)
  const row = rows.findIndex((r) => r.cells.includes(cell))
  if (row < 0) return null
  return {
    row,
    col: rows[row].cells.indexOf(cell),
    rows: rows.length,
    cols: Math.max(...rows.map((r) => r.cells.length), 0),
  }
}

/** 把表格的行拆成 `[{ cells: [td...], isHeader }]`，忽略嵌套表格里的行列。 */
export function collectRows(table) {
  if (!table) return []
  return [...table.rows].map((row) => ({
    // 只取「直接属于这一行」的单元格：`row.cells` 会带出嵌套表格的单元格。
    cells: [...row.children].filter((el) => /^(TD|TH)$/i.test(el.tagName)),
    isHeader: Boolean(row.closest('thead')) || [...row.children].some((el) => /^TH$/i.test(el.tagName)),
  }))
}

/**
 * 计算插入行之后的新行号，以及该行是否应该带表头样式。
 *
 * 表头保护：光标在第一行（表头）时，「在上方插入」会插到表头**之前**——
 * 那会让新行取代表头位置，读起来很怪。这里改成插在表头下方。
 */
export function insertionRowAfterInsert(rows, targetRow, where) {
  if (where === 'above') {
    // 表头行不能往下挤：插到表头之后
    return targetRow === 0 && rows[0]?.isHeader ? 1 : targetRow
  }
  return Math.min(rows.length, targetRow + 1)
}

/**
 * 表格样式的预设：与 Outlook 的「表格样式」面板对应，但只用邮件客户端认的表现属性。
 *
 * `headerBg` 会写到 `bgcolor`（Outlook 唯一稳定支持的底色写法），
 * `borderColor` 写成 `style` 上的 `border-color`（其余客户端都认）。
 */
export const TABLE_STYLES = [
  { key: 'plain', label: '无边框', headerBg: '', borderStyle: 'none' },
  { key: 'grid', label: '网格线', headerBg: '#f2f2f2', borderStyle: '1px solid #cccccc' },
  { key: 'header', label: '首行强调', headerBg: '#4472c4', borderStyle: '1px solid #cccccc' },
  { key: 'zebra', label: '隔行变色', headerBg: '#f2f2f2', borderStyle: '1px solid #cccccc', zebra: true },
]

/** 生成一格的新单元格（`<br>` 占位，否则部分客户端里会塌掉）。 */
export function createCellHtml(tag = 'td', text = '') {
  return `<${tag}>${text || '<br>'}</${tag}>`
}
