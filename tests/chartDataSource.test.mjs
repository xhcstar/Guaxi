// 图表数据来源管理（资源库固化）的纯函数回归测试。
//
// 背景：图表如果只引用本机绝对路径，数据文件一挪、或笔记换机器打开就画不出来。
// 修复方向是「数据文件固化进资源库 + resource: 引用 + 读不到时回退」，
// 这些测试锁住路径生成与围栏组装的语义。
import assert from 'node:assert/strict'
import {
  CHART_DATA_EXTENSIONS,
  buildChartDataResourceTarget,
  buildChartFenceWithDataRef,
} from '../src/components/HomeViewComponent/utils/guaxiEditingCommands.js'

// 1) 支持的扩展名固定，避免有人误加二进制类型。
assert.deepEqual(CHART_DATA_EXTENSIONS, ['csv', 'tsv', 'json'])

// 2) 数据文件保留原始文件名（不像图片那样加时间戳）——引用必须跨机器稳定。
const target = buildChartDataResourceTarget('D:/datasets/Sales Q3.csv', new Date(2026, 9, 21))
assert.equal(target.fileName, 'Sales-Q3.csv')
assert.equal(target.relativePath, 'charts/2026/10/Sales-Q3.csv')
assert.equal(target.ref, 'resource:charts/2026/10/Sales-Q3.csv')

// 3) 反斜杠路径同样被规范化。
const winTarget = buildChartDataResourceTarget('C:\\data\\metrics.tsv', new Date(2026, 0, 5))
assert.equal(winTarget.relativePath, 'charts/2026/01/metrics.tsv')

// 4) 不支持的扩展名必须显式报错，而不是悄悄生成一个画不出来的围栏。
assert.throws(() => buildChartDataResourceTarget('D:/x.png'), /不支持的数据文件类型/)

// 5) 围栏里写的是 resource: 引用，不是本机绝对路径。
const fence = buildChartFenceWithDataRef({
  ref: target.ref,
  type: 'bar',
  x: 'month',
  y: 'revenue',
  title: '营收',
})
assert.ok(fence.startsWith('```chart\n'), '围栏必须以 ```chart 开头')
assert.ok(fence.trimEnd().endsWith('```'), '围栏必须闭合')
assert.ok(fence.includes(`data: ${target.ref}`), 'data 必须是 resource: 引用')
assert.ok(!/[a-zA-Z]:\//.test(fence), '围栏里不能出现本机盘符路径')
assert.ok(fence.includes('title: 营收'))

console.log('chartDataSource: ok')
