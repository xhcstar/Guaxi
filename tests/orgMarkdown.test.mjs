import test from 'node:test'
import assert from 'node:assert/strict'

// 任务条目正文的 Markdown 渲染（src/utils/guaxiOrgMarkdown.js）。
//
// 锁的是用户能感知到的三件事：
//   1. 条目里的换行要换行——单换行、空行分段、`<br>` 三种写法都得生效。
//      （`<br>` 是用户在反馈里明确试过、之前却被转义成字面文本的那种写法。）
//   2. 完整 Markdown 语法要生效——标题、列表、表格、引用、加粗。
//   3. 搜索高亮要落在正文上，而不是靠外层再包一层字符串替换。
import { renderOrgMarkdown } from '../src/utils/guaxiOrgMarkdown.js'

// 只关心结构，不关心 markdown-it 的换行位置。
const flat = (html) => String(html).replace(/\s+/g, ' ').trim()

test('单换行渲染为换行，而不是被吞成一行', () => {
  const html = flat(renderOrgMarkdown('第一行\n第二行\n第三行'))
  assert.equal(html, '<p>第一行<br /> 第二行<br /> 第三行</p>')
})

test('空行分段', () => {
  const html = flat(renderOrgMarkdown('第一段\n\n第二段'))
  assert.match(html, /<p>第一段<\/p>/)
  assert.match(html, /<p>第二段<\/p>/)
})

test('<br> 被当作换行，而不是字面文本', () => {
  const html = flat(renderOrgMarkdown('第一行<br>第二行'))
  assert.equal(html, '<p>第一行<br /> 第二行</p>')
  assert.ok(!html.includes('&lt;br'), '不能把 <br> 转义后原样显示出来')
})

test('<br/> 与 <br /> 两种写法同样生效', () => {
  for (const tag of ['<br/>', '<br />', '<BR>', '<br  >']) {
    const html = flat(renderOrgMarkdown(`甲${tag}乙`))
    assert.equal(html, '<p>甲<br /> 乙</p>', `写法 ${tag} 未生效`)
  }
})

test('行内代码里的 <br> 保持原样，不被折成换行', () => {
  const html = renderOrgMarkdown('`a<br>b`')
  assert.match(html, /<code>a&lt;br&gt;b<\/code>/)
  assert.ok(!html.includes('<br'), '行内代码里不应出现真的换行标签')
})

test('代码块内不解释 <br> 与 Markdown 语法', () => {
  const src = '```\n# 不是标题\n<br>\n- 不是列表\n```'
  const html = renderOrgMarkdown(src)
  assert.match(html, /<pre><code>/)
  assert.ok(!html.includes('<h1'), '代码块里的 # 不能被当成标题')
  assert.ok(!html.includes('<br'), '代码块里的 <br> 不能被折成换行')
  assert.ok(html.includes('# 不是标题'))
})

test('完整 Markdown 语法：标题、列表、表格、引用、加粗', () => {
  const src = [
    '### 复现要点',
    '',
    '- 第一条 **加粗**',
    '- 第二条',
    '',
    '| 项目 | 数值 |',
    '| --- | --- |',
    '| SM2 | 22K |',
    '',
    '> 引用一行',
  ].join('\n')
  const html = renderOrgMarkdown(src)
  assert.match(html, /<h3>复现要点<\/h3>/)
  assert.match(html, /<ul>[\s\S]*<li>第一条 <strong>加粗<\/strong><\/li>/)
  assert.match(html, /<table>[\s\S]*<th>项目<\/th>/)
  assert.match(html, /<blockquote>[\s\S]*引用一行/)
})

test('原始 HTML 被转义，不会把 shell 的尖括号当标签吞掉', () => {
  const html = renderOrgMarkdown('if [ $a -lt 5 ]; then echo "$b"; fi # <stdin>')
  assert.ok(html.includes('&lt;stdin&gt;'), '尖括号必须显示为文本')
})

test('搜索词在正文里高亮成 mark，且不受字符转义影响', () => {
  const html = renderOrgMarkdown('第一行 libcrypto 好\n第二行 libcrypto', null, { query: 'libcrypto' })
  const marks = html.match(/<mark class="otp-hit">libcrypto<\/mark>/g) || []
  assert.equal(marks.length, 2)
})

test('搜索高亮大小写不敏感，且不会破坏已经渲染好的标签', () => {
  const html = renderOrgMarkdown('**LibCrypto** 与 libcrypto', null, { query: 'libcrypto' })
  assert.match(html, /<strong><mark class="otp-hit">LibCrypto<\/mark><\/strong>/)
  assert.match(html, /<mark class="otp-hit">libcrypto<\/mark>/)
})

test('搜索词本身含 HTML 特殊字符时不会注入标记', () => {
  const html = renderOrgMarkdown('a < b', null, { query: '<' })
  assert.ok(!html.includes('<mark class="otp-hit"><</mark>'), '匹配文本必须经过转义')
  assert.match(html, /<mark class="otp-hit">&lt;<\/mark>/)
})

test('空内容返回空串', () => {
  assert.equal(renderOrgMarkdown(''), '')
  assert.equal(renderOrgMarkdown('   \n  '), '')
  assert.equal(renderOrgMarkdown(null), '')
})

test('本地图片路径交给 convertSrc 转换，网络图片原样保留', () => {
  const convert = (p) => `asset://localhost/${p}`
  const local = renderOrgMarkdown('![图](D:/pic/a.png)', convert)
  assert.ok(local.includes('src="asset://localhost/D:/pic/a.png"'), '本地路径应经过转换')

  const remote = renderOrgMarkdown('![图](https://example.com/a.png)', convert)
  assert.ok(remote.includes('src="https://example.com/a.png"'), '网络图片不应被改写')
})

test('链接带 target/rel，站内锚点不加', () => {
  const html = renderOrgMarkdown('[外链](https://example.com)')
  assert.match(html, /target="_blank"/)
  assert.match(html, /rel="noreferrer"/)
  const anchor = renderOrgMarkdown('[锚点](#section)')
  assert.ok(!anchor.includes('target="_blank"'))
})
