import test from 'node:test'
import assert from 'node:assert/strict'

// 模板管理器的数据层。锁的是用户能感知的规则：
//   - 自定义模板与内置同名时，用的是自定义那份，同时仍能识别出「覆盖了内置」；
//   - 分类层级（`探索者 / Python Cell / 数据读取`）既要能按一级分类收拢，也不能丢掉二级；
//   - 搜索要能命中正文并给出片段与行号，且名称命中排在正文命中之前；
//   - 模板名会成为磁盘文件名，非法字符必须在保存前拦下。
import {
  contentSnippetOf,
  groupTemplates,
  groupTemplatesByFamily,
  mergeTemplates,
  searchTemplates,
  summarizeFamilies,
  summarizeTemplates,
  templateFamilyOf,
  templateFileOf,
  templateKindOf,
  templateVariantOf,
  textStatsOf,
  validateTemplateName,
} from '../src/components/HomeViewComponent/templateManager/templateModel.js'

const builtin = [
  { name: '会议纪要（通用）', templateType: '会议', content: '# 会议纪要\n\n## 基本信息\n' },
  { name: '日报', templateType: '工作', content: '# 日报\n\n- 今天完成：\n' },
  { name: '数据读取', templateType: '探索者 / Python Cell / 数据读取', content: 'import pandas as pd\n' },
]

test('同名自定义模板覆盖内置，但仍标记为覆盖', () => {
  const contents = new Map([['日报', '# 我的日报\n\n自定义正文']])
  const list = mergeTemplates(builtin, [{ name: '日报', template_type: '工作' }], contents)
  const daily = list.find((item) => item.name === '日报')
  assert.equal(daily.builtin, false)
  assert.equal(daily.overriddenBuiltin, true)
  assert.equal(daily.content, '# 我的日报\n\n自定义正文')
  assert.equal(list.filter((item) => item.name === '日报').length, 1)
})

test('未被读取正文的自定义模板不能拿内置正文顶包', () => {
  const list = mergeTemplates(builtin, [{ name: '日报', template_type: '工作' }], new Map())
  const daily = list.find((item) => item.name === '日报')
  assert.equal(daily.loaded, false)
  assert.equal(daily.content, '')
})

test('分类层级拆分与一级分类统计', () => {
  assert.equal(templateFamilyOf('探索者 / Python Cell / 数据读取'), '探索者 / Python Cell')
  assert.equal(templateVariantOf('探索者 / Python Cell / 数据读取'), '数据读取')
  assert.equal(templateFamilyOf('会议'), '会议')
  assert.equal(templateVariantOf('会议'), '')
  assert.equal(templateFamilyOf(''), '未分类')

  const list = mergeTemplates(builtin, [], new Map())
  const families = summarizeFamilies(list)
  const names = families.map((f) => f.family)
  assert.ok(names.includes('会议'))
  assert.ok(names.includes('探索者 / Python Cell'))
  assert.equal(families.find((f) => f.family === '探索者 / Python Cell').count, 1)
})

test('分组保持给定的分类顺序，组内按名称排序', () => {
  const list = mergeTemplates(builtin, [], new Map())
  const groups = groupTemplates(list, ['会议', '工作', '探索者 / Python Cell / 数据读取'])
  assert.deepEqual(groups.map((g) => g.type), ['会议', '工作', '探索者 / Python Cell / 数据读取'])
  assert.equal(groups[2].family, '探索者 / Python Cell')
  assert.equal(groups[2].variant, '数据读取')
})

test('按一级分类分组：Python Cell 的 20 个子类收成一组', () => {
  const list = mergeTemplates(builtin, [], new Map())
  const groups = groupTemplatesByFamily(list, ['会议', '工作', '探索者 / Python Cell / 数据读取'])
  assert.deepEqual(groups.map((g) => g.family), ['会议', '工作', '探索者 / Python Cell'])
  assert.equal(groups[2].variantized, true)
  assert.equal(groups[0].variantized, false)
})

test('搜索：名称 > 分类 > 正文，正文命中带片段与行号', () => {
  const list = mergeTemplates(builtin, [], new Map())

  const byName = searchTemplates(list, '日报')
  assert.equal(byName[0].item.name, '日报')
  assert.equal(byName[0].hit.field, 'name')

  const byContent = searchTemplates(list, 'pandas')
  assert.equal(byContent.length, 1)
  assert.equal(byContent[0].item.name, '数据读取')
  assert.equal(byContent[0].hit.field, 'content')
  assert.equal(byContent[0].hit.snippet.line, 1)
  assert.match(byContent[0].hit.snippet.match, /pandas/)

  const byType = searchTemplates(list, 'python cell')
  assert.ok(byType.some((r) => r.item.name === '数据读取'))
  assert.equal(byType[0].hit.field, 'type')

  assert.equal(searchTemplates(list, '不存在的词').length, 0)
  assert.equal(searchTemplates(list, '').length, list.length)
})

test('正文片段折叠 Markdown 控制符并定位行号', () => {
  const content = '# 标题\n\n- 第一项\n- 需要查找的关键词在这里\n'
  const snippet = contentSnippetOf(content, '关键词')
  assert.equal(snippet.line, 4)
  assert.equal(snippet.match, '关键词')
  assert.ok(!snippet.prefix.includes('#'))
  assert.equal(contentSnippetOf(content, '不存在'), null)
})

test('模板名合法性：路径分隔符与 Windows 文件名非法字符都要拦下', () => {
  assert.equal(validateTemplateName('  周报  ').value, '周报')
  assert.equal(validateTemplateName('').ok, false)
  assert.equal(validateTemplateName('a/b').ok, false)
  assert.equal(validateTemplateName('a\\b').ok, false)
  assert.equal(validateTemplateName('a..b').ok, false)
  assert.equal(validateTemplateName('a<b').ok, false)
  assert.equal(validateTemplateName('x'.repeat(81)).ok, false)
})

test('语言判定：Python Cell 模板走 python，不再写 .md 后缀', () => {
  assert.equal(templateKindOf({ name: '数据读取', type: '探索者 / Python Cell / 数据读取' }), 'python')
  assert.equal(templateKindOf({ name: 'Python Cell｜数据读取', type: '' }), 'python')
  // 名字里只是提到 Python 不算——「Python 学习笔记」是 Markdown，不是代码。
  assert.equal(templateKindOf({ name: 'Python 学习笔记', type: '学习' }), 'markdown')
  assert.equal(templateKindOf({ name: '日报', type: '工作' }), 'markdown')
  assert.equal(templateFileOf('日报'), '日报.md')
  assert.equal(templateFileOf('notes.MD'), 'notes.MD')
})

test('统计与文本信息', () => {
  const list = mergeTemplates(builtin, [{ name: '我的模板', template_type: '未分类' }], new Map())
  const stats = summarizeTemplates(list)
  assert.equal(stats.total, 4)
  assert.equal(stats.builtin, 3)
  assert.equal(stats.user, 1)
  assert.equal(stats.categories, 4)
  assert.deepEqual(textStatsOf('a\nb\nc'), { chars: 5, lines: 3 })
  assert.deepEqual(textStatsOf(''), { chars: 0, lines: 0 })
})
