import test from 'node:test'
import assert from 'node:assert/strict'

import {
  buildChartOption,
  collectFenceBlocks,
} from '../src/components/GuaxiMarkdownRender/guaxiChartData.js'
import {
  CHART_TYPES,
  normalizeChartType,
  supportedChartTypes,
} from '../src/components/GuaxiMarkdownRender/guaxiChartTypes.js'

// 覆盖 guaxiChartTypes 登记表里的**每一种**类型。
// 用意：新增图表类型时若不补最小可用 spec，这里的「逐个构造」断言会失败，
// 避免出现「登记了但根本跑不通」的类型。

const RECORD_SET = [
  { month: '一月', revenue: 120, cost: 40, open: 10, close: 12, low: 9, high: 13, a: 1, b: 2, c: 3, name: 'A', value: 10, source: 'A', target: 'B', path: 'X/Y', start: 1, end: 5, height: 170, weight: 60, size: 3 },
  { month: '二月', revenue: 180, cost: 60, open: 12, close: 15, low: 11, high: 16, a: 2, b: 4, c: 5, name: 'B', value: 20, source: 'B', target: 'C', path: 'X/Z', start: 3, end: 8, height: 175, weight: 70, size: 5 },
  { month: '三月', revenue: 160, cost: 55, open: 15, close: 13, low: 12, high: 17, a: 3, b: 6, c: 7, name: 'C', value: 30, source: 'C', target: 'A', path: 'Y/W', start: 5, end: 9, height: 180, weight: 80, size: 8 },
]

const BASE_SPEC = { x: 'month', y: ['revenue'] }

/** 默认字段推导不到的类型，显式给出最小可用 spec。 */
const SPEC_OVERRIDES = {
  scatter: { x: 'height', y: ['height', 'weight'] },
  effectscatter: { x: 'height', y: ['height', 'weight'] },
  candlestick: { fields: ['open', 'close', 'low', 'high'] },
  boxplot: { fields: ['low', 'open', 'close', 'high', 'high'] },
  heatmap: { x: 'month', y: ['name', 'value'] },
  sankey: { x: 'source', y: ['target', 'value'] },
  graph: { x: 'source', y: ['target', 'value'] },
  tree: { path: 'path', y: ['value'] },
  treemap: { path: 'path', y: ['value'] },
  sunburst: { path: 'path', y: ['value'] },
  lines: { fields: ['height', 'weight', 'height', 'weight'] },
  map: { map: 'china', x: 'name', y: ['value'] },
  liquidfill: { y: ['value'] },
  wordcloud: { x: 'name', y: ['value'] },
  pie: { x: 'name', y: ['value'] },
  doughnut: { x: 'name', y: ['value'] },
  ring: { x: 'name', y: ['value'] },
  funnel: { x: 'name', y: ['value'] },
  themeriver: { x: 'month', y: ['value', 'name'] },
  custom: { x: 'month', y: ['start', 'end'], variant: 'gantt' },
  parallel: { y: ['a', 'b', 'c'] },
  pictorialbar: { x: 'month', y: ['revenue'] },
}

test('登记表覆盖需求里列出的全部核心类型', () => {
  const required = [
    'line', 'bar', 'pie', 'scatter', 'boxplot', 'heatmap', 'effectscatter',
    'radar', 'parallel', 'sankey', 'funnel', 'gauge', 'tree', 'treemap',
    'sunburst', 'graph', 'map', 'lines', 'candlestick', 'pictorialbar',
    'themeriver', 'custom',
  ]
  for (const t of required) {
    assert.ok(CHART_TYPES[t], `缺少类型: ${t}`)
  }
})

test('扩展包类型已登记', () => {
  assert.ok(CHART_TYPES.liquidfill)
  assert.ok(CHART_TYPES.wordcloud)
})

test('每个登记的类型都能真正构造出 option', () => {
  const failures = []
  for (const type of Object.keys(CHART_TYPES)) {
    const spec = { ...BASE_SPEC, type, ...(SPEC_OVERRIDES[type] || {}) }
    try {
      const option = buildChartOption(spec, RECORD_SET)
      assert.ok(option && typeof option === 'object', `${type}: option 不是对象`)
      assert.ok(Array.isArray(option.series), `${type}: 缺少 series 数组`)
      assert.ok(option.series.length > 0, `${type}: series 为空`)
    } catch (error) {
      failures.push(`${type}: ${error.message}`)
    }
  }
  assert.deepEqual(failures, [], `以下类型构造失败:\n${failures.join('\n')}`)
})

test('别名能正确归一化', () => {
  assert.equal(normalizeChartType('kline'), 'candlestick')
  assert.equal(normalizeChartType('donut'), 'doughnut')
  assert.equal(normalizeChartType('bubble'), 'scatter')
  assert.equal(normalizeChartType('liquid'), 'liquidfill')
  assert.equal(normalizeChartType('Bar'), 'bar')
  assert.equal(normalizeChartType('theme-river'), 'themeriver')
})

test('未知类型给出可操作的错误（列出可用类型）', () => {
  assert.throws(
    () => buildChartOption({ type: 'nonexistent', ...BASE_SPEC }, RECORD_SET),
    /不支持的图表类型[\s\S]*可用类型/,
  )
})

test('supportedChartTypes 返回排序后的列表', () => {
  const list = supportedChartTypes()
  assert.ok(list.length >= 25)
  assert.deepEqual(list, [...list].sort())
})

// ───────────── 各类型的关键语义 ─────────────

test('bar 支持堆叠与横向', () => {
  const stacked = buildChartOption({ type: 'bar', x: 'month', y: ['revenue', 'cost'], stack: true }, RECORD_SET)
  assert.equal(stacked.series[0].stack, 'total')
  assert.equal(stacked.series[1].stack, 'total')

  const horizontal = buildChartOption({ type: 'bar', x: 'month', y: ['revenue'], orient: 'horizontal' }, RECORD_SET)
  assert.equal(horizontal.xAxis.type, 'value')
  assert.equal(horizontal.yAxis.type, 'category')
})

test('waterfall 生成占位/上升/下降三段系列', () => {
  const opt = buildChartOption({ type: 'waterfall', x: 'month', y: ['revenue'] }, RECORD_SET)
  assert.equal(opt.series.length, 3)
  assert.equal(opt.series[0].name, '_placeholder')
})

test('line 支持 smooth 与 step', () => {
  const opt = buildChartOption({ type: 'line', x: 'month', y: ['revenue'], smooth: true, step: 'end' }, RECORD_SET)
  assert.equal(opt.series[0].smooth, true)
  assert.equal(opt.series[0].step, 'end')
})

test('scatter 第三维映射到点大小', () => {
  const opt = buildChartOption(
    { type: 'scatter', x: 'height', y: ['height', 'weight'], size: 'size' }, RECORD_SET)
  assert.equal(opt.series[0].data[0].length, 3, '应包含第三维')
  assert.equal(typeof opt.series[0].symbolSize, 'function')
})

test('pie 支持玫瑰花图与自定义半径', () => {
  const rose = buildChartOption({ type: 'pie', x: 'name', y: ['value'], roseType: 'angle' }, RECORD_SET)
  assert.equal(rose.series[0].roseType, 'angle')

  const ring = buildChartOption({ type: 'pie', x: 'name', y: ['value'], radius: ['30%', '70%'] }, RECORD_SET)
  assert.deepEqual(ring.series[0].radius, ['30%', '70%'])
})

test('gauge 单值走指针模式，多记录走多环', () => {
  const single = buildChartOption({ type: 'gauge', y: ['value'] }, [RECORD_SET[0]])
  assert.equal(single.series[0].type, 'gauge')
  assert.equal(single.series[0].data[0].value, 10)

  const multi = buildChartOption({ type: 'gauge', x: 'name', y: ['value'] }, RECORD_SET)
  assert.equal(multi.series.length, 3)
  assert.match(multi.series[1].radius, /%/)
})

test('gauge 单值的数值与名字留足垂直间距，不会叠字', () => {
  // 回归：旧值 detail 72% / title 92% 只差 20% 半径。240px 高的画布半径 90px，
  // 两个文本框（值 22px、名字 12px）正好压在一起，截图表现为“文字糊成一团”。
  const single = buildChartOption(
    { type: 'gauge', x: 'name', y: ['value'], formatter: '{value}%' },
    [RECORD_SET[0]],
  )
  const s = single.series[0]
  const pct = (v) => Number.parseFloat(String(v)) / 100
  // offsetCenter 的百分比相对半径；文字垂直居中，半高约是行高/字号的一半。
  const valueHalf = Number(s.detail.lineHeight) / 2
  const nameHalf = Number(s.title.fontSize) / 2
  const radius = 90 // 画布 240px 高：radius 75% × (240 / 2)
  const gap = (pct(s.title.offsetCenter[1]) - pct(s.detail.offsetCenter[1])) * radius
  assert.ok(
    gap >= valueHalf + nameHalf,
    `数值与名字的垂直间隔应不小于两者半高之和：gap=${gap}px, halves=${valueHalf + nameHalf}px`,
  )
  // 名字不能压到圆环内缘（r - 线宽/2），否则会和环重叠。
  const ringInner = radius - Number(s.axisLine.lineStyle.width) / 2
  const nameBottom = pct(s.title.offsetCenter[1]) * radius + nameHalf
  assert.ok(nameBottom <= ringInner, `名字底边 ${nameBottom}px 不应越过圆环内缘 ${ringInner}px`)
})

test('gauge 多环把各圈名字错开画在圆心，不再叠字，并保留底部图例', () => {
  // 回归：多环每圈共用一个圆心，ECharts 默认把各 series 的 title 画在同一点，
  // 名字会糊成一团；但直接关掉又会让圆心完全没有说明。这里要求逐行错开且间距够用。
  const multi = buildChartOption({ type: 'gauge', x: 'name', y: ['value'] }, RECORD_SET)
  for (const s of multi.series) {
    assert.equal(s.title?.show, true, '环内仍应显示每圈的名字')
    assert.equal(s.detail?.show, false, '多环环内数值必须关闭')
    assert.ok(s.name, '系列名应取自记录名，供图例与 tooltip 使用')
  }
  const offsets = multi.series.map((s) => Number(s.title.offsetCenter[1]))
  assert.deepEqual(offsets, [-16, 0, 16], '三圈名字应按固定像素间距错开')
  // 相邻两行的间距必须大于 12px 字号，否则仍会叠字。
  assert.ok(offsets[1] - offsets[0] >= 12, '行距应不小于字号 12px')

  // 没有名称列（x）时不画空文字。
  const noNames = buildChartOption({ type: 'gauge', y: ['value'], multi: true }, RECORD_SET)
  assert.equal(noNames.series.every((s) => s.title.show === false), true)

  assert.equal(multi.legend?.bottom, 0, '图例放在底部留白处，避免压住圆环')

  // 环数多时外圈要收在 72% 以内，否则会压到底部图例上。
  const five = [...RECORD_SET, { name: 'D', value: 40 }, { name: 'E', value: 50 }]
  const many = buildChartOption({ type: 'gauge', x: 'name', y: ['value'] }, five)
  const outer = Math.max(...many.series.map((s) => Number.parseFloat(s.radius)))
  assert.ok(outer <= 72, `最外圈半径 ${outer}% 应留在图例上方`)
  // 半径仍然彼此分开，不会叠成一个环。
  const radii = many.series.map((s) => Number.parseFloat(s.radius))
  assert.equal(new Set(radii).size, radii.length, '各环半径必须互不相同')
  // 5 环时最外侧一行仍在最内圈以内（240px 高画布：45% 半径 = 54px，文字半高 6px）。
  const manyOffsets = many.series.map((s) => Math.abs(Number(s.title.offsetCenter[1])))
  assert.ok(Math.max(...manyOffsets) + 6 <= 54, '名字整叠不应越过最内圈')
})

test('scatter 使用数值轴，坐标对不会被当成类目索引', () => {
  // 回归：散点交给默认的 category 轴时，160 这类坐标会被当成“第 160 个类目”，
  // 点全被挤出绘图区——预览和 pandoc 导出里都只剩坐标轴。
  const opt = buildChartOption({ type: 'scatter', y: ['height', 'weight'] }, RECORD_SET)
  assert.equal(opt.xAxis.type, 'value')
  assert.equal(opt.yAxis.type, 'value')
  assert.equal(opt.series[0].type, 'scatter')
  assert.deepEqual(opt.series[0].data[0], [170, 60])
})

test('瀑布图图例不列出内部占位序列，标题与图例留出间距', () => {
  const opt = buildChartOption(
    { type: 'waterfall', x: 'month', y: ['value'], title: 'W' },
    RECORD_SET,
  )
  assert.ok(Array.isArray(opt.legend.data), '图例应显式给出数据项')
  assert.ok(!opt.legend.data.includes('_placeholder'), '透明占位柱不应出现在图例里')
  assert.deepEqual(opt.legend.data, ['增加', '减少'])
  // 标题 18px 字号在默认位置下底边约 38px，图例启动位置必须让开。
  assert.ok(Number(opt.legend.top) >= 42)
})

test('围栏切分只认行首，标题里的字面量不会吞掉后面的代码块', () => {
  // 回归：旧正则允许 ```chart 出现在任意位置，于是
  // 「## 10A. 内嵌图表（```chart）」这样的标题会把紧随其后的第一个图表块吞掉，
  // 那块图在 Word/EPUB 导出里就仍然是源码。
  const md = [
    '## 10A. 内嵌图表（```chart）',
    '',
    '```chart',
    'spec:',
    '  type: line',
    'data: |',
    '  x,y',
    '  1,2',
    '```',
    '',
    '### 白板（```excalidraw）',
    '',
    '```excalidraw',
    'C:/tmp/a.excalidraw',
    '```',
  ].join('\n')

  const blocks = collectFenceBlocks(md, ['chart', 'excalidraw'])
  assert.deepEqual(blocks.map((b) => b.kind), ['chart', 'excalidraw'])
  assert.match(blocks[0].body, /type: line/)
  assert.equal(blocks[1].body.trim(), 'C:/tmp/a.excalidraw')
  // lead 保留触发匹配的换行，替换整块时补回去，段落才不会粘连。
  assert.equal(blocks[0].lead, '\n')

  // 行内字面量本身不构成围栏。
  assert.equal(collectFenceBlocks('正文里的 ```chart 不是围栏', ['chart']).length, 0)
  // 没有行尾闭合的围栏也不算。
  assert.equal(collectFenceBlocks('```chart\nspec:\n  type: line', ['chart']).length, 0)
})

test('围栏切分支持 4 个反引号与 ~~~ 围栏，并按内容里的长度配对闭合', () => {
  const md = [
    '````chart',
    'spec:',
    '  type: bar',
    '```',
    'data: |',
    '  x,y',
    '  1,2',
    '````',
    '',
    '~~~mermaid',
    'graph TD',
    '  A-->B',
    '~~~',
  ].join('\n')
  const blocks = collectFenceBlocks(md, ['chart', 'mermaid'])
  assert.equal(blocks.length, 2)
  assert.match(blocks[0].body, /```/g, '内容里的三反引号不应提前闭合四反引号围栏')
  assert.equal(blocks[1].kind, 'mermaid')
})

test('sankey 从 source/target/value 三列建图', () => {
  const opt = buildChartOption({ type: 'sankey', x: 'source', y: ['target', 'value'] }, RECORD_SET)
  assert.equal(opt.series[0].type, 'sankey')
  assert.equal(opt.series[0].links.length, 3)
  assert.ok(opt.series[0].data.length >= 3, '节点应去重')
})

test('tree 由 path 列构建层级', () => {
  const opt = buildChartOption({ type: 'tree', path: 'path', y: ['value'] }, RECORD_SET)
  const root = opt.series[0].data[0]
  assert.equal(opt.series[0].type, 'tree')
  assert.ok(root.children && root.children.length > 0, '应有子节点')
})

test('graph 由 source/target 建关系图', () => {
  const opt = buildChartOption({ type: 'graph', x: 'source', y: ['target', 'value'] }, RECORD_SET)
  assert.equal(opt.series[0].type, 'graph')
  assert.equal(opt.series[0].links.length, 3)
})

test('radar 的指标上限自动推导', () => {
  const opt = buildChartOption({ type: 'radar', x: 'name', y: ['a', 'b', 'c'] }, RECORD_SET)
  assert.equal(opt.radar.indicator.length, 3)
  assert.ok(opt.radar.indicator[0].max >= 3)
})

test('map 未指定地图名时报可操作错误', () => {
  assert.throws(
    () => buildChartOption({ type: 'map', x: 'name', y: ['value'] }, RECORD_SET),
    /spec\.map/,
  )
})

test('custom 的 gantt 变体生成 custom 系列', () => {
  const opt = buildChartOption(
    { type: 'custom', variant: 'gantt', x: 'month', y: ['start', 'end'] }, RECORD_SET)
  assert.equal(opt.series[0].type, 'custom')
  assert.equal(typeof opt.series[0].renderItem, 'function')
})

test('未知变体不崩，退回可用图形', () => {
  const opt = buildChartOption(
    { type: 'custom', variant: 'unknown', x: 'month', y: ['revenue'] }, RECORD_SET)
  assert.ok(opt.series.length > 0)
})

test('seriesStyle 可覆盖任意 ECharts 字段', () => {
  const opt = buildChartOption(
    { type: 'bar', x: 'month', y: ['revenue'], seriesStyle: { itemStyle: { color: '#f00' } } },
    RECORD_SET,
  )
  assert.deepEqual(opt.series[0].itemStyle, { color: '#f00' })
})

test('heatmap 自带 visualMap 与类目轴', () => {
  const opt = buildChartOption({ type: 'heatmap', x: 'month', y: ['name', 'value'] }, RECORD_SET)
  assert.ok(opt.visualMap, '应有 visualMap')
  assert.equal(opt.xAxis.type, 'category')
  assert.equal(opt.yAxis.type, 'category')
})

test('funnel 默认按值降序', () => {
  const opt = buildChartOption({ type: 'funnel', x: 'name', y: ['value'] }, RECORD_SET)
  const values = opt.series[0].data.map((d) => d.value)
  assert.deepEqual(values, [...values].sort((a, b) => b - a))
})
