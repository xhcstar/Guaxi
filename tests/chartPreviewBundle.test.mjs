import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import vm from 'node:vm'
import { fileURLToPath } from 'node:url'

// 静态预览页（/browser/preview）的图表链路契约。
//
// 背景：那个页面是后端直接吐出的静态 HTML，没有打包器，页面里曾经自带一份
// 「精简版」解析器。它和软件内模块各改各的，于是软件内能画的树图、旭日、关系图、
// 热力、K 线、甘特在浏览器里全报「静态预览暂不支持该图表类型」。
//
// 现在页面用 tools/build_chart_preview_bundle.mjs 打出来的共享模块。这个测试守着
// 两件事：产物存在且可用；产物确实来自软件内的同一份模块（而不是又抄了一份）。
const here = path.dirname(fileURLToPath(import.meta.url))
const projectRoot = path.join(here, '..')
const bundlePath = path.join(projectRoot, 'src-tauri', 'assets', 'vendor', 'guaxi-chart-preview.js')

/** 在隔离上下文里加载 IIFE 产物，拿到它挂到 globalThis 上的入口。 */
function loadBundle() {
  assert.ok(fs.existsSync(bundlePath), `缺少共享图表产物：${bundlePath}（跑 npm run build:chart-preview）`)
  const code = fs.readFileSync(bundlePath, 'utf8')
  const sandbox = {}
  sandbox.window = sandbox
  vm.createContext(sandbox)
  vm.runInContext(code, sandbox)
  const api = sandbox.GuaxiChartPreview
  assert.ok(api, '产物没有挂载 GuaxiChartPreview')
  return api
}

test('共享图表产物可用，且解析语义与软件内一致', () => {
  const api = loadBundle()
  for (const name of ['parseChartFence', 'normalizeDataset', 'buildChartOption', 'buildLightOption']) {
    assert.equal(typeof api[name], 'function', `缺少入口 ${name}`)
  }

  const fence = [
    'spec:',
    '  type: bar',
    '  x: month',
    '  y: [revenue, cost]',
    'data: |',
    '  month,revenue,cost',
    '  一月,120,40',
    '  二月,180,60',
  ].join('\n')
  const parsed = api.parseChartFence(fence)
  // 产物跑在 vm 沙箱里，对象原型与主上下文不同，用 JSON 比较值。
  assert.equal(JSON.stringify(parsed.spec), JSON.stringify({ type: 'bar', x: 'month', y: ['revenue', 'cost'] }))

  const records = api.normalizeDataset(parsed.data, parsed.dataFormat)
  assert.equal(records.length, 2)
  const option = api.buildChartOption(parsed.spec, records)
  assert.equal(option.series.length, 2)
  assert.equal(option.series[0].type, 'bar')
})

test('软件内登记过的每种图表类型，预览产物都能组装出 option', () => {
  const api = loadBundle()
  // 最小记录集：覆盖各类型用到的列名。
  const records = [
    { name: 'A', a: 1, b: 2, c: 3, value: 10, path: 'X/Y', source: 'A', target: 'B', start: 1, end: 5, open: 10, close: 12, low: 9, high: 13, word: '甲', weight: 5 },
    { name: 'B', a: 2, b: 4, c: 5, value: 20, path: 'X/Z', source: 'B', target: 'C', start: 3, end: 8, open: 12, close: 15, low: 11, high: 16, word: '乙', weight: 3 },
  ]
  // map 需要预先注册的地理数据（GeoJSON），离线环境两张图一样画不出来，
  // 不属于「预览页比软件内弱」的范畴：软件内同样会拒绝，这里不列为回归项。
  const failures = []
  for (const type of Object.keys(api.CHART_TYPES)) {
    if (type === 'map') continue
    const spec = {
      type,
      x: 'name',
      y: ['value', 'b', 'c'],
      path: 'path',
      source: 'source',
      target: 'target',
      fields: ['open', 'close', 'low', 'high'],
      variant: 'gantt',
    }
    try {
      const option = api.buildChartOption(spec, records)
      assert.ok(option && option.series, `${type} 没有产出 series`)
    } catch (error) {
      failures.push(`${type}: ${error.message}`)
    }
  }
  assert.deepEqual(failures, [], `预览产物无法组装这些类型：\n${failures.join('\n')}`)
})

test('直接贴完整 ECharts option 的写法必须被当作 option，而不是 spec', () => {
  const api = loadBundle()
  const raw = '{"title":{"text":"x"},"xAxis":{"type":"category","data":["A"]},"yAxis":{},"series":[{"type":"bar","data":[5]}]}'
  const parsed = api.parseChartFence(raw)
  const option = api.buildChartOption(parsed.spec, [])
  assert.equal(option.series[0].type, 'bar')
  assert.equal(option.title.text, 'x')
})

test('bundled: 样本随产物分发，静态页不依赖文件系统', () => {
  const api = loadBundle()
  assert.equal(typeof api.isBundledRef, 'function')
  assert.ok(api.isBundledRef('bundled:charts/stock.csv'))
  const sample = api.getBundledSample('charts/stock.csv')
  assert.ok(sample && sample.text.includes('date'), '内置样本没有打进产物')
  assert.ok(api.listBundledSamples().length >= 1)
})

test('构建脚本引用的是软件内同一份模块，而不是另抄一份', () => {
  const entry = fs.readFileSync(path.join(projectRoot, 'tools', 'chart_preview_entry.js'), 'utf8')
  for (const mod of ['guaxiChartData.js', 'guaxiChartTypes.js', 'guaxiChartSamples']) {
    assert.ok(entry.includes(mod), `入口没有复用 ${mod}`)
  }
})
