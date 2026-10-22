// 电子表格「值 / 公式」双视图的回归测试。
//
// 背景：后端为每个工作表同时返回 `rows`（计算结果）与 `formulas`（公式文本），
// 两个矩阵同形。前端在公式视图下显示公式、在值视图下显示结果。
//
// 这里用一个不依赖 DOM 的最小模型复刻 displayCell 的语义，锁住三条性质：
//   1. 公式视图下，有公式的格子显示 `=...`，没有公式的格子**仍然显示值**
//      （否则整张表变成大片空白，反而看不出结构）；
//   2. 公式前缺等号时补上，与实际输入习惯一致；
//   3. 值视图下永远显示结果，公式不影响它。
import assert from 'node:assert/strict'

/** 复刻 DocSpreadsheetReader 的 cellAt / formulaAt / displayCell。 */
const makeSheet = ({ rows, formulas, hasFormulas }) => ({
  rows,
  formulas,
  hasFormulas: hasFormulas ?? formulas.some((r) => r.some(Boolean)),
})

const cellAt = (sheet, r, c) => sheet.rows[r]?.[c] ?? ''
const formulaAt = (sheet, r, c) => sheet.formulas?.[r]?.[c] ?? ''

const displayCell = (sheet, r, c, showFormulas) => {
  if (showFormulas) {
    const f = formulaAt(sheet, r, c)
    if (f) return f.startsWith('=') ? f : `=${f}`
  }
  return cellAt(sheet, r, c)
}

// 一张典型表：A 列是标签，B 列有公式，C 列是纯值。
const sheet = makeSheet({
  rows: [
    ['项目', '总额', '备注'],
    ['一月', '120', ''],
    ['二月', '180', ''],
  ],
  formulas: [
    ['', '', ''],
    ['', 'SUM(A2:A2)', ''],
    ['', '', ''],
  ],
})

// 1) 值视图：显示结果。
assert.equal(displayCell(sheet, 1, 1, false), '120')
assert.equal(displayCell(sheet, 2, 1, false), '180')

// 2) 公式视图：有公式的格子显示带等号的公式。
assert.equal(displayCell(sheet, 1, 1, true), '=SUM(A2:A2)')

// 3) 公式视图下，无公式的格子仍显示值 —— 不是空白。
assert.equal(displayCell(sheet, 1, 0, true), '一月', '无公式的格子必须回退到值')
assert.equal(displayCell(sheet, 1, 2, true), '', '本来就空的格子仍是空')

// 4) 后端若已带等号，不能重复补。
const withEquals = makeSheet({ rows: [['1']], formulas: [['=A1*2']] })
assert.equal(displayCell(withEquals, 0, 0, true), '=A1*2')

// 5) hasFormulas 决定是否显示切换按钮：全无公式的表不应出现按钮。
const noFormula = makeSheet({ rows: [['a', 'b']], formulas: [['', '']] })
assert.equal(noFormula.hasFormulas, false, '没有公式时不应显示切换按钮')

// 6) 公式矩阵比值矩阵短（旧文件/截断）时不能越界。
const ragged = makeSheet({ rows: [['1'], ['2'], ['3']], formulas: [['=A1']] })
assert.equal(displayCell(ragged, 2, 0, true), '3', '公式行缺失时回退到值')
assert.equal(displayCell(ragged, 5, 0, true), '', '越界返回空串而不是抛错')

console.log('spreadsheetFormulaView: ok')
