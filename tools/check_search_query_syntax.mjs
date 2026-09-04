import assert from 'node:assert/strict'
import { parseSearchQuery, parseTimeValue, matchesPathFilters } from '../src/utils/searchQuerySyntax.js'

// 搜索框查询语法的自检。仓库里没有前端测试框架，
// 这个脚本只依赖 node 自带的 assert，`npm run check:search-syntax` 直接跑。

// 固定一个"现在"，相对时间才有确定答案：2026-09-04 12:00 本地时间
const NOW = new Date(2026, 8, 4, 12, 0, 0, 0).getTime()
const parse = (raw) => parseSearchQuery(raw, { now: NOW })

let passed = 0
const check = (name, fn) => {
  try {
    fn()
    passed += 1
  } catch (err) {
    console.error(`FAIL ${name}\n  ${err.message}`)
    process.exitCode = 1
  }
}

check('记号从搜索词里剥掉', () => {
  const r = parse('配置 加载 ext:md')
  assert.equal(r.text, '配置 加载')
  assert.deepEqual(r.exts, ['md'])
})

check('扩展名归一化并去重', () => {
  const r = parse('ext:.MD,txt ext:md type:rs')
  assert.deepEqual(r.exts, ['md', 'txt', 'rs'])
})

check('引号内的空格不切词', () => {
  const r = parse('path:"我的 笔记" 配置')
  assert.deepEqual(r.paths, ['我的 笔记'])
  assert.equal(r.text, '配置')
})

check('反斜杠路径统一成正斜杠', () => {
  assert.deepEqual(parse('path:src\\utils').paths, ['src/utils'])
})

check('不认识的键留在搜索词里', () => {
  // title: 要交给 tantivy 的查询语法；C:\ 和 http:// 本来就是用户想搜的字符串
  assert.equal(parse('title:配置').text, 'title:配置')
  assert.equal(parse('C:\\Users').text, 'C:\\Users')
  assert.equal(parse('http://example.com').text, 'http://example.com')
})

check('只敲了键还没敲值时先吞掉', () => {
  // 否则输入到一半会拿 "ext:" 当字面量去搜，结果闪一下变空
  const r = parse('配置 ext:')
  assert.equal(r.text, '配置')
  assert.deepEqual(r.exts, [])
  assert.equal(r.tokens[0].pending, true)
})

check('相对时间按天数折算', () => {
  assert.equal(parse('after:7d').after, NOW - 7 * 86400000)
  assert.equal(parse('after:2w').after, NOW - 14 * 86400000)
  assert.equal(parse('after:1y').after, NOW - 365 * 86400000)
})

check('today 取当天零点，before 取当天末尾', () => {
  const start = new Date(2026, 8, 4).getTime()
  assert.equal(parseTimeValue('today', 'after', NOW), start)
  assert.equal(parseTimeValue('today', 'before', NOW), start + 86400000 - 1)
})

check('日期只写到月份时补成整月', () => {
  assert.equal(parseTimeValue('2026-02', 'after', NOW), new Date(2026, 1, 1).getTime())
  assert.equal(parseTimeValue('2026-02', 'before', NOW), new Date(2026, 2, 1).getTime() - 1)
})

check('日期写到天时 before 覆盖到当天末尾', () => {
  assert.equal(parseTimeValue('2026/03/15', 'before', NOW), new Date(2026, 2, 15).getTime() + 86400000 - 1)
})

check('时间写错就退回普通搜索词', () => {
  const r = parse('after:不是日期 配置')
  assert.equal(r.after, null)
  assert.equal(r.text, 'after:不是日期 配置')
  assert.equal(r.invalid.length, 1)
})

check('大于号开头进命令模式', () => {
  assert.equal(parse('>重建索引').command, '重建索引')
  assert.equal(parse('  >  重建 ').command, '重建')
  // 只敲了一个 > 也算命令模式，此时应列出全部命令
  assert.equal(parse('>').command, '')
  // 夹在中间的 > 不算
  assert.equal(parse('a>b').command, null)
})

check('路径筛选按子串且多条取交集', () => {
  assert.equal(matchesPathFilters('src/utils/foo.js', ['utils']), true)
  assert.equal(matchesPathFilters('src\\utils\\foo.js', ['src/utils']), true)
  assert.equal(matchesPathFilters('src/utils/foo.js', ['Utils', 'src']), true)
  assert.equal(matchesPathFilters('src/utils/foo.js', ['tests']), false)
  assert.equal(matchesPathFilters('anything', []), true)
})

check('空输入不报错', () => {
  const r = parse('')
  assert.equal(r.text, '')
  assert.equal(r.command, null)
  assert.equal(parseSearchQuery(undefined).text, '')
})

console.log(`searchQuerySyntax: ${passed} 项检查通过`)
