import test from 'node:test'
import assert from 'node:assert/strict'
import {
  buildTableHtml,
  markdownToEmailHtml,
  renderPipeTable,
  escapeHtml,
} from '../src/components/ApplicationViewComponent/apps/mail/composeTable.js'
import { STANDARD_COLORS, HIGHLIGHT_COLORS, pushRecent } from '../src/components/ApplicationViewComponent/apps/mail/palette.js'

// 写信正文的表格与 Markdown 转换。
//
// 两条硬约束（用户的真实诉求）：
//   1. 表格必须是真 <table>，任何客户端（尤其 Outlook 的 Word 引擎）都能显示；
//   2. Markdown 要转成渲染后的 HTML 再发出去，收件端不能看到一堆 # 和 |。

test('表格是真表格，并带 Outlook 认的表现属性', () => {
  const html = buildTableHtml({ rows: 3, cols: 3 })
  assert.ok(html.startsWith('<table'), '必须是 table 元素')
  assert.ok(/border="1"/.test(html), '要有 border 表现属性')
  assert.ok(/cellpadding=/.test(html) && /cellspacing=/.test(html), '要有 cellpadding / cellspacing')
  assert.ok(/<thead>/.test(html) && /bgcolor=/.test(html), '表头要有底色属性')
  // 3 列 → 3 个 th；3 行 → 表头 + 2 行数据
  assert.equal((html.match(/<th>/g) || []).length, 3)
  // 表头那一行带 bgcolor 属性，所以按 `<tr` 数而不是 `<tr>`。
  assert.equal((html.match(/<tr[\s>]/g) || []).length, 3)
})

test('表格行列数被约束在合理范围', () => {
  const small = buildTableHtml({ rows: 0, cols: 0 })
  assert.ok(small.includes('<th>'), '0 行也要有表头')
  const huge = buildTableHtml({ rows: 999, cols: 999 })
  assert.ok((huge.match(/<th>/g) || []).length <= 10, '列数上限 10')
  assert.ok((huge.match(/<tr>/g) || []).length <= 20, '行数上限 20')
})

test('管道表格转成真表格，分隔行不显示', () => {
  const html = renderPipeTable([
    '| 项目 | 状态 |',
    '| --- | --- |',
    '| 登录 | 完成 |',
  ])
  assert.ok(html.includes('<table'), '应渲染成 table')
  assert.ok(!html.includes('---'), '分隔行不能出现在结果里')
  assert.ok(html.includes('登录') && html.includes('完成'))
  assert.equal((html.match(/<th /g) || []).length, 2)
  assert.equal((html.match(/<td /g) || []).length, 2)
})

test('Markdown 标题 / 列表 / 粗体 / 链接都转成 HTML', () => {
  const html = markdownToEmailHtml('## 周报\n\n- 完成 **登录** 模块\n- 修复 [bug](https://x.y/z)')
  assert.ok(/<h2 style=/.test(html), '标题要带内联样式（客户端会剥掉 style 块）')
  assert.ok(html.includes('<strong>登录</strong>'))
  assert.ok(/<a href="https:\/\/x\.y\/z"/.test(html))
  assert.ok(html.includes('<ul') && (html.match(/<li/g) || []).length === 2)
})

test('Markdown 代码块保留原样并转义', () => {
  const html = markdownToEmailHtml('```ini\n[openssl_init]\nengines = engine_section\n```')
  assert.ok(html.includes('<pre'))
  assert.ok(html.includes('[openssl_init]'))
  assert.ok(!html.includes('<script'), '不能透传 HTML')
})

test('Markdown 里的 HTML 会被转义，不会执行', () => {
  const html = markdownToEmailHtml('<script>alert(1)</script>')
  assert.ok(!html.includes('<script>'), '尖括号必须被转义')
  assert.ok(html.includes('&lt;script&gt;'))
  assert.equal(escapeHtml('<b>&"'), '&lt;b&gt;&amp;&quot;')
})

test('Markdown 自制表格（管道语法）也转成真表格', () => {
  const md = ['| 模块 | 负责人 |', '| --- | --- |', '| 认证 | 张三 |', '| 同步 | 李四 |'].join('\n')
  const html = markdownToEmailHtml(md)
  assert.ok(html.includes('<table'), '应转成真表格')
  assert.ok(!html.includes('|---'), '不能残留管道分隔行')
  assert.ok(html.includes('张三') && html.includes('李四'))
})

test('两行普通的 | 文本不会被误判成表格', () => {
  const html = markdownToEmailHtml('| 不是表格\n| 真的不是')
  assert.ok(!html.includes('<table'), '缺分隔行时不当表格')
})

test('调色板是 5 列标准色，深色和饱和色都要有', () => {
  // 行数不固定（按常用色增减），但每行必须正好 5 个，色板才是整齐的网格。
  assert.ok(STANDARD_COLORS.length >= 3)
  assert.ok(HIGHLIGHT_COLORS.length >= 3)
  for (const row of [...STANDARD_COLORS, ...HIGHLIGHT_COLORS]) {
    assert.equal(row.length, 5)
    for (const color of row) assert.match(color, /^#[0-9a-f]{6}$/)
  }
  // 「常见颜色都要有」：文字色里必须有黑、正红、绿、蓝。
  const text = STANDARD_COLORS.flat()
  for (const color of ['#000000', '#ff0000', '#00b050', '#0070c0']) {
    assert.ok(text.includes(color), `文字色缺 ${color}`)
  }
  // 填充色既要有浅色打底，也要有饱和色。
  const fill = HIGHLIGHT_COLORS.flat()
  assert.ok(fill.includes('#ddebf7'), '填充缺浅色打底')
  assert.ok(fill.includes('#ffc000') || fill.includes('#00b050'), '填充缺饱和色')
})

test('最近用过的颜色：去重、置顶、限长', () => {
  let recent = []
  recent = pushRecent(recent, '#112233')
  recent = pushRecent(recent, '#445566')
  assert.deepEqual(recent, ['#445566', '#112233'])
  recent = pushRecent(recent, '#112233')
  assert.deepEqual(recent, ['#112233', '#445566'], '重复色置顶')
  for (let i = 0; i < 10; i += 1) recent = pushRecent(recent, `#00000${i}`)
  assert.equal(recent.length, 5, '最多记 5 个')
  assert.deepEqual(pushRecent(recent, 'red'), recent, '非法色值直接忽略')
})
