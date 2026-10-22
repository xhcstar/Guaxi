import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 图表导出链路的三条硬约束。这些不是「实现细节」，而是会导致用户可见故障的契约：
//
//   1. 导出前必须关掉动画 —— 否则 SVG 里的进场动画 clipPath 停在 0%（零宽），
//      折线 / 树图连线 / 标签被整条裁掉，静态消费方（PNG、PDF、<img>）只看到一张空底图。
//   2. 词云必须等异步布局 —— 它的文字是在离屏 canvas 上逐轮摆放的，setOption 返回时还没画。
//   3. 导出的 SVG 必须自带白色背板 —— 深色界面导出的透明底图贴进文档就「看不见内容」。
//
// 这里用源码断言而不是跑 ECharts：这些行为依赖真实 DOM 与 Canvas 布局，
// 跑起来需要浏览器环境；而回归风险恰恰在于「有人把这些调用删了」，源码断言足够直接。
const here = path.dirname(fileURLToPath(import.meta.url))
const exportSource = fs.readFileSync(
  path.join(here, '..', 'src', 'components', 'GuaxiMarkdownRender', 'guaxiChartExport.js'),
  'utf8',
)

test('导出前关闭动画（避免 clipPath 停在 0% 把曲线裁掉）', () => {
  assert.match(exportSource, /function freezeChartAnimation/, '缺少 freezeChartAnimation')
  assert.match(
    exportSource,
    /animation:\s*false/,
    'freezeChartAnimation 必须显式关闭 animation',
  )
  assert.match(
    exportSource,
    /setOption\(freezeChartAnimation\(buildLightOption\(option\)\),\s*true\)/,
    'renderChartSvg 必须用冻结动画后的 option 渲染',
  )
  // series 上也可能挂动画（用户可直接贴整段 ECharts option）
  assert.match(exportSource, /series\.map/, 'series 级别的 animation 也要逐条覆盖')
})

test('词云导出前等待异步布局完成', () => {
  assert.match(exportSource, /function waitForWordCloudLayout/, '缺少词云布局等待')
  assert.match(
    exportSource,
    /hasWordCloudSeries\(option\)\)\s*await waitForWordCloudLayout\(instance\)/,
    'renderChartSvg 必须在词云场景下等待布局',
  )
  // 等待要有上限，坏数据不能把导出卡死
  assert.match(exportSource, /timeoutMs\s*=\s*\d+/, '等待必须有超时上限')
})

test('导出的 option 带白色背板', () => {
  // buildLightOption 负责补 backgroundColor；renderChartSvg 必须用它的结果渲染
  assert.match(exportSource, /buildLightOption/, 'renderChartSvg 必须走 buildLightOption')
})
