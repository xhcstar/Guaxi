import test from 'node:test'
import assert from 'node:assert/strict'

import {
  extOf,
  stemOf,
  classifyByExtension,
  needsReader,
  READER_KIND,
} from '../src/utils/openTargetKind.js'

import { parseDelimited, csvToRecords, normalizeDataset, parseChartFence, buildChartOption } from '../src/components/GuaxiMarkdownRender/guaxiChartData.js'

// ───────────────────────── 路径工具 ─────────────────────────

test('extOf 处理 Windows / POSIX / 无扩展名', () => {
  assert.equal(extOf('D:\\Notes\\a.MD'), 'md')
  assert.equal(extOf('/home/u/report.final.pdf'), 'pdf')
  assert.equal(extOf('Dockerfile'), '')
  assert.equal(extOf('.gitignore'), '')
  assert.equal(extOf(''), '')
})

test('stemOf 去掉最后一个扩展名', () => {
  assert.equal(stemOf('D:\\a\\report.final.pdf'), 'report.final')
  assert.equal(stemOf('noext'), 'noext')
})

// ───────────────────────── 类型分流（关键：确保文本仍走编辑器） ─────────────────────────

test('文本类扩展名必须判为编辑器，不能误入阅读器', () => {
  const textExts = ['md', 'txt', 'py', 'js', 'ts', 'json', 'yaml', 'tex', 'org', 'csv', 'svg', 'log', 'sh']
  for (const ext of textExts) {
    const result = classifyByExtension(`D:/notes/file.${ext}`)
    assert.equal(result?.reader, READER_KIND.TEXT, `${ext} 应进编辑器`)
  }
})

test('文档类扩展名判到各自的阅读器', () => {
  const cases = {
    'a.pdf': READER_KIND.PDF,
    'a.docx': READER_KIND.WORD,
    'a.doc': READER_KIND.WORD,
    'a.epub': READER_KIND.EBOOK,
    'a.azw3': READER_KIND.EBOOK,
    'a.xlsx': READER_KIND.SPREADSHEET,
    'a.pptx': READER_KIND.SLIDES,
    'a.png': READER_KIND.IMAGE,
    'a.gif': READER_KIND.IMAGE,
    'a.mp4': READER_KIND.MEDIA,
    'a.mp3': READER_KIND.MEDIA,
  }
  for (const [name, reader] of Object.entries(cases)) {
    assert.equal(classifyByExtension(`D:/notes/${name}`)?.reader, reader, name)
  }
})

test('未知扩展名返回 null（需要后端嗅探，而不是猜）', () => {
  assert.equal(classifyByExtension('D:/notes/a.xyz'), null)
  assert.equal(classifyByExtension('D:/notes/noext'), null)
})

test('needsReader 只对非文本返回 true', () => {
  assert.equal(needsReader(READER_KIND.TEXT), false)
  assert.equal(needsReader(''), false)
  assert.equal(needsReader(READER_KIND.PDF), true)
  assert.equal(needsReader(READER_KIND.EBOOK), true)
})

// ───────────────────────── CSV / 数据解析 ─────────────────────────

test('parseDelimited 支持引号内的逗号与换行', () => {
  const rows = parseDelimited('a,b\n"x,1","y\n2"\n')
  assert.deepEqual(rows, [
    ['a', 'b'],
    ['x,1', 'y\n2'],
  ])
})

test('parseDelimited 处理转义双引号与 BOM', () => {
  const rows = parseDelimited('\uFEFFname,note\n"say ""hi""",ok')
  assert.deepEqual(rows, [
    ['name', 'note'],
    ['say "hi"', 'ok'],
  ])
})

test('csvToRecords 首行当表头并做数值转换', () => {
  const records = csvToRecords('month,revenue\n一月,120\n二月,98.5\n')
  assert.deepEqual(records, [
    { month: '一月', revenue: 120 },
    { month: '二月', revenue: 98.5 },
  ])
})

test('normalizeDataset 接受 JSON 数组与 {data:[]}', () => {
  assert.deepEqual(normalizeDataset('[{"a":1}]'), [{ a: 1 }])
  assert.deepEqual(normalizeDataset('{"data":[{"a":2}]}'), [{ a: 2 }])
  // 非法 JSON 以 [ 开头时应抛错而不是静默当 CSV
  assert.throws(() => normalizeDataset('[{"a":'))
})

test('normalizeDataset 用 TSV 分隔符', () => {
  assert.deepEqual(normalizeDataset('a\tb\n1\t2', 'tsv'), [{ a: 1, b: 2 }])
})

// ───────────────────────── chart 围栏解析 ─────────────────────────

test('parseChartFence 解析 spec + 外部数据路径', () => {
  const parsed = parseChartFence('spec:\n  type: bar\n  x: month\n  y: [revenue]\ndata: ./sales.csv')
  assert.equal(parsed.spec.type, 'bar')
  assert.equal(parsed.data, './sales.csv')
  assert.equal(parsed.dataInline, false)
})

test('parseChartFence 解析内联数据块', () => {
  const parsed = parseChartFence(`spec:
  type: line
  x: day
  y: [v]
data: |
  day,v
  一,1
  二,2`)
  assert.equal(parsed.spec.type, 'line')
  assert.equal(parsed.dataInline, true)
  assert.match(parsed.data, /day,v/)
})

test('parseChartFence 支持直接贴完整 ECharts option', () => {
  const parsed = parseChartFence('{"series":[{"type":"line","data":[1,2]}]}')
  assert.deepEqual(parsed.spec.series[0].data, [1, 2])
})

test('parseChartFence 对坏 spec 不抛错（坏字段被忽略，可用字段保留）', () => {
  const parsed = parseChartFence('spec:\n  type: bar\n  broken: {')
  // 关键契约：解析绝不抛异常，否则整篇笔记渲染会失败。
  assert.equal(parsed.spec?.type, 'bar')
})

test('parseChartFence 支持块状列表写法', () => {
  const parsed = parseChartFence('spec:\n  type: line\n  x: d\n  y:\n    - a\n    - b\ndata: ./x.csv')
  assert.deepEqual(parsed.spec.y, ['a', 'b'])
  assert.equal(parsed.data, './x.csv')
})

test('parseChartFence 无 spec 时返回 null', () => {
  const parsed = parseChartFence('data: ./x.csv')
  assert.equal(parsed.spec, null)
})

// ───────────────────────── option 构建 ─────────────────────────

test('buildChartOption 生成柱状图序列', () => {
  const records = [
    { month: '一月', revenue: 10, cost: 4 },
    { month: '二月', revenue: 20, cost: 8 },
  ]
  const option = buildChartOption({ type: 'bar', x: 'month', y: ['revenue', 'cost'] }, records)
  assert.equal(option.xAxis.data.length, 2)
  assert.equal(option.series.length, 2)
  assert.deepEqual(option.series[0].data, [10, 20])
  assert.equal(option.series[0].type, 'bar')
})

test('buildChartOption 饼图用第一列名称、第一列数值', () => {
  const records = [{ name: 'A', value: 3 }, { name: 'B', value: 7 }]
  const option = buildChartOption({ type: 'pie', x: 'name', y: ['value'] }, records)
  assert.equal(option.series[0].type, 'pie')
  assert.deepEqual(option.series[0].data[1], { name: 'B', value: 7 })
})

test('buildChartOption 缺少 spec 时报中文错误', () => {
  assert.throws(() => buildChartOption(null, []), /缺少 spec/)
})

test('buildChartOption 原样透传用户自带 option', () => {
  const option = buildChartOption({ option: { series: [{ type: 'bar', data: [1] }] } }, [{ a: 1 }])
  assert.equal(option.series[0].type, 'bar')
  assert.deepEqual(option.dataset.source, [{ a: 1 }])
})

test('buildChartOption 识别「直接贴 option 本体」的写法', () => {
  const option = buildChartOption(
    { title: { text: '占比' }, series: [{ type: 'pie', radius: '60%', data: [{ name: 'A', value: 3 }] }] },
    [],
  )
  // 不能被当成 spec 走默认 line 分支（那会得到一张空图）
  assert.equal(option.series[0].type, 'pie')
  assert.equal(option.xAxis, undefined)
  assert.deepEqual(option.title, { text: '占比' })
})

// ───────────────────────── 图表导出（新） ─────────────────────────

test('buildLightOption 在未指定颜色时强制浅色', async () => {
  const { buildLightOption } = await import('../src/components/GuaxiMarkdownRender/guaxiChartData.js')
  const out = buildLightOption({
    title: { text: 'T' },
    xAxis: { type: 'category', data: ['a'] },
    yAxis: { type: 'value' },
    series: [{ type: 'bar', data: [1] }],
  })
  assert.equal(out.backgroundColor, '#ffffff')
  assert.equal(out.title.textStyle.color, '#1f2328')
  assert.equal(out.xAxis.axisLabel.color, '#4a4f57')
})

test('buildLightOption 尊重用户显式指定的配色', async () => {
  const { buildLightOption } = await import('../src/components/GuaxiMarkdownRender/guaxiChartData.js')
  const out = buildLightOption({
    backgroundColor: '#000000',
    title: { text: 'T', textStyle: { color: '#ff0000' } },
    series: [],
  })
  assert.equal(out.backgroundColor, '#000000', '显式背景色不能被覆盖')
  assert.equal(out.title.textStyle.color, '#ff0000', '显式标题色不能被覆盖')
})
