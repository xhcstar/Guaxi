/**
 * src/utils/searchPreview.js 的自检脚本（`npm run check:search-preview`）。
 *
 * 盯的是一件事：内容搜索的每一行里，第一处命中必须看得见。
 * 仓库没有前端测试框架，所以用 node:assert 直接跑纯函数。
 */
import assert from 'node:assert/strict'
import {
  PREVIEW_LEAD,
  PREVIEW_MAX,
  buildPreviewSegments,
  normalizeMatchRanges,
  stripLineEnd
} from '../src/utils/searchPreview.js'

/** 把片段拼回去，用来断言"窗口里的原文没被切坏"。 */
const joinAll = (res) => res.segments.map((s) => s.text).join('')
/** 只把高亮片段拼起来，用来断言"命中确实进了窗口"。 */
const joinHits = (res) => res.segments.filter((s) => s.hit).map((s) => s.text).join('')

let failed = 0
const check = (name, fn) => {
  try {
    fn()
    console.log(`  ok   ${name}`)
  } catch (error) {
    failed += 1
    console.log(`  FAIL ${name}`)
    console.log(`       ${error.message.split('\n').join('\n       ')}`)
  }
}

console.log('searchPreview:')

check('去掉 grep 带回来的行尾换行', () => {
  assert.equal(stripLineEnd('abc\r\n'), 'abc')
  assert.equal(stripLineEnd('abc\n\n'), 'abc')
  assert.equal(stripLineEnd('a\nb\n'), 'a\nb')
  assert.equal(stripLineEnd(undefined), '')
  const res = buildPreviewSegments({ preview: 'hello\n', matches: [{ start: 0, end: 5 }] })
  assert.equal(joinAll(res), 'hello')
})

check('掐掉行首缩进，但不算作省略', () => {
  const res = buildPreviewSegments({
    preview: '        const x = 1',
    matches: [{ start: 14, end: 15 }]
  })
  assert.equal(joinAll(res), 'const x = 1')
  // 缩进是排版噪声，掐掉不需要提示用户"这里省略了"——否则每行都顶着一个"…"
  assert.equal(res.leadCut, false)
  assert.equal(res.tailCut, false)
})

check('缩进之外又右移了才算省略', () => {
  const res = buildPreviewSegments({
    preview: `${' '.repeat(8)}${'a'.repeat(60)}needle`,
    matches: [{ start: 68, end: 74 }]
  })
  assert.equal(res.leadCut, true, '越过了真正的内容，必须给省略号')
  assert.equal(joinHits(res), 'needle')
  assert.equal(res.segments[0].text, 'a'.repeat(PREVIEW_LEAD))
})

check('搜的就是空白时，缩进一个字都不能掐', () => {
  const res = buildPreviewSegments({
    preview: '\t\tvalue',
    matches: [{ start: 0, end: 2 }]
  })
  assert.equal(joinAll(res), '\t\tvalue')
  assert.equal(joinHits(res), '\t\t')
})

check('命中太靠后就右移窗口，只留 PREVIEW_LEAD 个字符上文', () => {
  const line = `${'x'.repeat(500)}needle${'y'.repeat(500)}`
  const res = buildPreviewSegments({ preview: line, matches: [{ start: 500, end: 506 }] })
  assert.equal(joinHits(res), 'needle', '第一处命中必须落在窗口里')
  assert.equal(res.segments[0].text, 'x'.repeat(PREVIEW_LEAD), '命中前应恰好留 PREVIEW_LEAD 个字符')
  assert.equal(res.leadCut, true)
  assert.equal(res.tailCut, true)
  assert.equal(Array.from(joinAll(res)).length, PREVIEW_MAX)
})

check('命中就在行首时不平移', () => {
  const res = buildPreviewSegments({
    preview: 'needle 在最前面',
    matches: [{ start: 0, end: 6 }]
  })
  assert.equal(res.leadCut, false)
  assert.equal(res.segments[0].hit, true)
})

check('超长行按 PREVIEW_MAX 截尾并标记 tailCut', () => {
  const line = 'z'.repeat(PREVIEW_MAX + 50)
  const res = buildPreviewSegments({ preview: line, matches: [{ start: 0, end: 1 }] })
  assert.equal(Array.from(joinAll(res)).length, PREVIEW_MAX)
  assert.equal(res.tailCut, true)
  const short = buildPreviewSegments({ preview: 'zzz', matches: [{ start: 0, end: 1 }] })
  assert.equal(short.tailCut, false)
})

check('窗口内原文不被切坏（片段拼接 = 原行切片）', () => {
  const term = '参数高效微调'
  const head = '不重要的前缀 '
  const mid = ' 中间 '
  const line = `${head}${term}${mid}${term} 尾巴`
  const first = Array.from(head).length
  const second = first + Array.from(term + mid).length
  const res = buildPreviewSegments({
    preview: line,
    matches: [
      { start: first, end: first + term.length },
      { start: second, end: second + term.length }
    ]
  })
  assert.equal(joinAll(res), line, '没有截断时应完整还原')
  assert.equal(joinHits(res), term + term, '两处命中都要高亮')
  assert.ok(res.segments.every((s) => s.text.length > 0), '不应产生空片段')
})

check('重叠区间合并，不产出交叉片段', () => {
  const res = buildPreviewSegments({
    preview: 'abcdefghij',
    matches: [{ start: 2, end: 6 }, { start: 4, end: 8 }]
  })
  assert.equal(joinAll(res), 'abcdefghij')
  assert.equal(joinHits(res), 'cdefgh')
  assert.equal(res.segments.filter((s) => s.hit).length, 1, '重叠的两段应合成一段')
})

check('越界 / 空 / 脏区间被丢掉或钳住', () => {
  assert.deepEqual(normalizeMatchRanges([{ start: 5, end: 3 }], 10), [])
  assert.deepEqual(normalizeMatchRanges([{ start: 2, end: 2 }], 10), [])
  assert.deepEqual(normalizeMatchRanges([{ start: -3, end: 4 }], 10), [{ start: 0, end: 4 }])
  assert.deepEqual(normalizeMatchRanges([{ start: 8, end: 99 }], 10), [{ start: 8, end: 10 }])
  assert.deepEqual(normalizeMatchRanges([{ start: 20, end: 30 }], 10), [])
  assert.deepEqual(normalizeMatchRanges([{ start: NaN, end: 3 }, null, undefined], 10), [])
  assert.deepEqual(normalizeMatchRanges(null, 10), [])
  // 顺序乱的也要排好，否则切片会倒着走
  assert.deepEqual(
    normalizeMatchRanges([{ start: 6, end: 8 }, { start: 1, end: 3 }], 10),
    [{ start: 1, end: 3 }, { start: 6, end: 8 }]
  )
})

check('后端给的是码点下标：emoji 行不能错位', () => {
  // 🌟 是代理对，JS 字符串下标里占 2 个码元，后端算的是 1 个标量
  const line = '🌟🌟needle'
  assert.equal(line.length, 10, '前提：UTF-16 长度是 10')
  assert.equal(Array.from(line).length, 8, '前提：码点长度是 8')
  const res = buildPreviewSegments({ preview: line, matches: [{ start: 2, end: 8 }] })
  assert.equal(joinHits(res), 'needle', '按码点切才能对上后端下标')
  assert.equal(joinAll(res), line)
})

check('没有 matches 也不炸，照样给出可读的一行', () => {
  const res = buildPreviewSegments({ preview: '   纯文本，没有命中区间' })
  assert.equal(joinAll(res), '纯文本，没有命中区间')
  assert.equal(joinHits(res), '')
  assert.deepEqual(buildPreviewSegments(null).segments, [])
  assert.deepEqual(buildPreviewSegments({}).segments, [])
})

check('tooltip 的整行按码点截断并补省略号', () => {
  const res = buildPreviewSegments(
    { preview: '一'.repeat(50), matches: [{ start: 0, end: 1 }] },
    { titleMax: 10 }
  )
  assert.equal(res.full, `${'一'.repeat(10)}…`)
  const emoji = buildPreviewSegments({ preview: '🌟'.repeat(8) }, { titleMax: 4 })
  assert.equal(emoji.full, `${'🌟'.repeat(4)}…`, '不能把代理对劈成半个字符')
  const short = buildPreviewSegments({ preview: 'abc\n' }, { titleMax: 10 })
  assert.equal(short.full, 'abc', '不超长就原样给，不加省略号')
})

check('lead / max 可以按面板宽度覆盖', () => {
  const line = `${'a'.repeat(100)}HIT${'b'.repeat(100)}`
  const res = buildPreviewSegments({ preview: line, matches: [{ start: 100, end: 103 }] },
    { lead: 3, max: 20 })
  assert.equal(res.segments[0].text, 'aaa')
  assert.equal(joinHits(res), 'HIT')
  assert.equal(joinAll(res).length, 20)
})

if (failed > 0) {
  console.log(`\n${failed} 项未通过`)
  process.exit(1)
}
console.log('\n全部通过')
