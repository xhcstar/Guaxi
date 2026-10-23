// 语法手册示例必须「打开就能画」的回归测试。
//
// 背景：手册里的示例曾经写成 `./data/stock.csv` 与 `resource:charts/...`，
// 而手册窗口没有笔记基准目录、`resource:` 又指向用户自己的资源库 —— 用户打开手册
// 看到的是一块「读取数据文件失败」。示例引用必须是随包样本（bundled:）。
//
// 这里同时锁住四件事：
//   1. 手册里每一段可执行的 chart / excalidraw 围栏都能取到随包样本；
//   2. 打包侧注册的 key 与样本清单一致（漏登记会在运行时报「没有内置样本」）；
//   3. 样本数据能走通「解析围栏 → 规整记录 → 组装 option」整条纯函数链路；
//   4. bundled: 被识别为路径引用，而不是被当成内联数据。
import assert from 'node:assert/strict'
import { readFileSync, existsSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

import {
  parseChartFence,
  buildChartOption,
  normalizeDataset,
} from '../src/components/GuaxiMarkdownRender/guaxiChartData.js'
import {
  BUNDLED_SAMPLE_FILES,
  isBundledRef,
  buildSampleRegistry,
} from '../src/components/GuaxiMarkdownRender/guaxiChartSamples.js'

const here = dirname(fileURLToPath(import.meta.url))
const repoRoot = join(here, '..')
const manual = readFileSync(join(repoRoot, 'src/assets/MarkdownTest.md'), 'utf8')

// ───── 1) 样本清单里的每个文件都必须真实存在 ─────

const sampleDir = join(repoRoot, 'src/assets/chart-samples')
for (const key of BUNDLED_SAMPLE_FILES) {
  assert.ok(existsSync(join(sampleDir, key)), `样本文件缺失：src/assets/chart-samples/${key}`)
}

// 用真实文件内容搭一个和打包侧等价的注册表，后面用它验证手册引用。
const samples = buildSampleRegistry(
  Object.fromEntries(
    BUNDLED_SAMPLE_FILES.map((key) => {
      const format = key.endsWith('.excalidraw') || key.endsWith('.json')
        ? 'json'
        : key.endsWith('.tsv')
          ? 'tsv'
          : 'csv'
      return [key, { text: readFileSync(join(sampleDir, key), 'utf8'), format }]
    })
  )
)
assert.equal(samples.list().length, BUNDLED_SAMPLE_FILES.length, '清单与注册表必须完全对应')

// 打包侧（guaxiChartSamples.bundled.js）用了 `?raw`，Node 加载不了；
// 但它的 key 必须与清单逐项一致，否则运行时会出现「手册引用了，打包侧却没登记」。
// 用源码文本比对来兜住这个缺口（漏登记 = 手册里那块图报「没有内置样本」）。
const bundledSource = readFileSync(
  join(repoRoot, 'src/components/GuaxiMarkdownRender/guaxiChartSamples.bundled.js'),
  'utf8'
)
for (const key of BUNDLED_SAMPLE_FILES) {
  assert.ok(
    bundledSource.includes(`'${key}'`),
    `打包侧没有登记样本 key：${key}（请同步 guaxiChartSamples.bundled.js）`
  )
  assert.ok(
    bundledSource.includes(`chart-samples/${key}?raw`),
    `打包侧没有把文件读进包：chart-samples/${key}?raw`
  )
}

// ───── 2) 手册里每段可执行的围栏都指向能取到的样本 ─────

// 先剥离 ````（四个及以上反引号）的外层围栏：里面装的是「写法模板」，
// 是给人看/复制的源码，不是会被渲染的可执行示例。
const manualWithoutTemplates = manual.replace(/^`{4,}[^\n]*\r?\n[\s\S]*?^`{4,}[ \t]*$/gm, '')

const chartFences = [...manualWithoutTemplates.matchAll(/^```chart\r?\n([\s\S]*?)^```$/gm)].map((m) => m[1])
const boardFences = [...manualWithoutTemplates.matchAll(/^```excalidraw\r?\n([\s\S]*?)^```$/gm)].map((m) => m[1].trim())

assert.ok(chartFences.length >= 10, `手册应当保留足量的图表示例，实际 ${chartFences.length}`)
assert.ok(boardFences.length >= 1, '手册应当保留白板示例')

let externalRefCount = 0
for (const body of chartFences) {
  const parsed = parseChartFence(body)
  if (!parsed.data || parsed.dataInline) continue
  externalRefCount += 1
  assert.ok(
    isBundledRef(parsed.data),
    `手册里的可执行图表示例不能依赖本机文件或用户资源库：${parsed.data}`
  )
  assert.ok(samples.get(parsed.data), `内置样本缺失：${parsed.data}`)
}
assert.ok(externalRefCount >= 2, '手册应当至少演示两次「数据来自外部文件」')

for (const body of boardFences) {
  assert.ok(isBundledRef(body), `白板示例必须引用随包样本，实际是：${body}`)
  assert.ok(samples.get(body), `内置样本缺失：${body}`)
}

// 反过来：手册不该再把资源库示例写成可执行围栏（那在用户机器上必然失败）。
const liveResourceFence = chartFences.some((body) => {
  const parsed = parseChartFence(body)
  return parsed.data && !parsed.dataInline && /^resource:/i.test(parsed.data)
})
assert.equal(liveResourceFence, false, 'resource: 只能作为写法模板，不能作为可执行示例')

// ───── 3) bundled: 必须被当作路径，而不是内联数据 ─────

const refFence = parseChartFence(
  'spec:\n  type: bar\n  x: month\n  y: [revenue]\ndata: bundled:charts/2026/10/sales.csv'
)
assert.equal(refFence.dataInline, false, 'bundled: 引用不能被当成内联数据')
assert.equal(refFence.data, 'bundled:charts/2026/10/sales.csv')
assert.equal(refFence.dataFormat, 'auto', '外部引用由读取方按文件类型判定格式')

// ───── 4) 样本能走通到 ECharts option ─────

const sales = samples.get('bundled:charts/2026/10/sales.csv')
assert.equal(sales.format, 'csv')
const records = normalizeDataset(sales.text, sales.format)
assert.equal(records.length, 12, '月度营收样本应有 12 行')
assert.equal(records[0].month, '一月')
assert.equal(typeof records[0].revenue, 'number', '数值列必须转成 number')

const option = buildChartOption(
  { type: 'bar', x: 'month', y: ['revenue', 'cost'], title: '季度营收对比' },
  records
)
assert.equal(option.series.length, 2, '多序列应产出两条 series')
assert.equal(option.series[0].type, 'bar')

const stock = samples.get('bundled:charts/stock.csv')
assert.equal(stock.format, 'csv')
const stockRecords = normalizeDataset(stock.text, stock.format)
assert.ok(stockRecords.length > 20, '收盘价样本应有足够多的数据点')
assert.equal(typeof stockRecords[0].date, 'string', '日期列保持字符串')
assert.equal(typeof stockRecords[0].close, 'number')
const lineOption = buildChartOption({ type: 'line', x: 'date', y: ['close'] }, stockRecords)
assert.equal(lineOption.series[0].type, 'line')
assert.equal(lineOption.series[0].data.length, stockRecords.length)

// ───── 5) 白板样本是合法场景 JSON，且有可渲染元素 ─────

const scene = JSON.parse(samples.get('bundled:boards/architecture.excalidraw').text)
assert.ok(Array.isArray(scene.elements) && scene.elements.length > 0, '白板样本必须含图形元素')
assert.ok(scene.elements.some((el) => el.isDeleted !== true), '白板样本不能全是已删除元素')

// ───── 6) 未命中时返回 null，靠调用方展示清单 ─────

assert.equal(samples.get('bundled:charts/nope.csv'), null)

// 大小写、反斜杠、有无前缀都不影响命中（调用方已判定过前缀，这里只是容错）。
assert.ok(samples.get('BUNDLED:charts/stock.csv'))
assert.ok(samples.get('bundled:\\charts\\stock.csv'))
assert.ok(samples.get('bundled:./charts/stock.csv'))
assert.ok(samples.get('charts/stock.csv'), '裸 key 也应命中，便于内部直接按清单取')

console.log('chartSamples: ok')
