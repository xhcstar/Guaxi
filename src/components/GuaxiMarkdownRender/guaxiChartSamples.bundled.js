// 随包内置样本的打包侧：把 src/assets/chart-samples/ 下的真实文件读成字符串。
//
// 只有这里用 Vite 的 `?raw` —— 纯逻辑（前缀判定 / key 归一化 / 查询器）在
// guaxiChartSamples.js，Node 测试直接加载那一份，不经过打包器。

import stockCsv from '../../assets/chart-samples/charts/stock.csv?raw'
import salesCsv from '../../assets/chart-samples/charts/2026/10/sales.csv?raw'
import architectureBoard from '../../assets/chart-samples/boards/architecture.excalidraw?raw'

import { buildSampleRegistry } from './guaxiChartSamples'

// key 必须与 guaxiChartSamples.js 的 BUNDLED_SAMPLE_FILES 一致；
// 测试会比对两边，漏一个立刻能在 npm run test:frontend 里看到。
export const bundledSamples = buildSampleRegistry({
  'charts/stock.csv': { text: stockCsv, format: 'csv' },
  'charts/2026/10/sales.csv': { text: salesCsv, format: 'csv' },
  'boards/architecture.excalidraw': { text: architectureBoard, format: 'json' },
})

export const listBundledSamples = () => bundledSamples.list()
export const getBundledSample = (ref) => bundledSamples.get(ref)
