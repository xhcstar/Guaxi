import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 表格源码对齐是 Markdown 格式化的第二段（后端 comrak 之后）。
// 它按 `|` 切列，而图表围栏里的内联 CSV 也用 `|` 分隔——一旦被当成表格，
// 数据列会被加空格、补分隔行，图表直接读不出来。
//
// 这里用源码断言而不是直接 import：该模块依赖 Tauri 的 logger，
// node:test 进程加载不了。而被保护的行为是「哪些行必须跳过」，源码层面足够确定。
const here = path.dirname(fileURLToPath(import.meta.url))
const source = fs.readFileSync(
  path.join(here, '..', 'src', 'utils', 'markdownTableFormat.js'),
  'utf8',
)

test('Notebook cell 的判定函数有定义（曾因缺失导致任何含 | 的文档格式化抛错）', () => {
  assert.match(source, /const isNotebookCellStartLine\s*=/, 'isNotebookCellStartLine 未定义')
  assert.match(source, /const isNotebookCellEndLine\s*=/, 'isNotebookCellEndLine 未定义')
})

test('cell 判定覆盖 math / py / pyout 三种区块', () => {
  assert.match(source, /<!--math/, '缺少 math cell 起点判定')
  assert.match(source, /<!--py/, '缺少 py cell 起点判定')
  assert.match(source, /guaxi:pyout:/, '缺少 pyout cell 判定')
})

test('表格对齐按围栏整体跳过（图表与代码块都不能被切列）', () => {
  // 围栏进入/退出逻辑必须存在，且位于表格扫描之前
  assert.match(source, /inFence/, '缺少围栏状态')
  const fenceIdx = source.indexOf('const fence = /^\\s*(`{3,}|~{3,})/')
  const tableIdx = source.indexOf('if (inFence || !isPotentialTableLine(line))')
  assert.ok(fenceIdx > 0, '缺少围栏识别')
  assert.ok(tableIdx > fenceIdx, '围栏判断必须早于表格处理')
})

test('格式化入口不会再因为缺少 text 变量而报错', () => {
  const fnStart = source.indexOf('const formatMarkdownTablesInText')
  assert.ok(fnStart > 0)
  const body = source.slice(fnStart, fnStart + 400)
  assert.match(body, /const text = String\(content \?\? ''\)/, '入口缺少 text 定义')
})
