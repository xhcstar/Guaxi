import test from 'node:test'
import assert from 'node:assert/strict'

import { buildChartOption } from '../src/components/GuaxiMarkdownRender/guaxiChartData.js'
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
