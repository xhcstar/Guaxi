// 随包内置的图表 / 白板示例数据 —— 纯逻辑部分。
//
// 为什么需要它：语法手册里的示例必须「打开就能画出来」，而示例数据文件如果写成
// `./data/stock.csv` 或 `resource:charts/...`，在用户机器上一定不存在 —— 手册窗口没有
// 笔记基准目录，`resource:` 指向的又是用户自己的资源库。示例跑不起来，等于没教。
//
// 数据文件本体放在 src/assets/chart-samples/ 下，由 `guaxiChartSamples.bundled.js`
// 用 Vite 的 `?raw` 打进前端包，围栏里写 `bundled:charts/stock.csv` 即可引用。
// 整条链路不碰文件系统、不依赖 Tauri，因此手册窗口、阅读器、导出三条路径行为一致。
//
// 这个文件刻意不 import 任何打包器专属的东西（`?raw` 等）：Node 测试要直接加载它，
// 去验证「样本清单」与「手册里的引用」始终对得上。

/** 内置样本清单：`bundled:` 之后写的就是这些 key。 */
export const BUNDLED_SAMPLE_FILES = [
  'charts/stock.csv',
  'charts/2026/10/sales.csv',
  'boards/architecture.excalidraw',
]

/** `bundled:` 前缀判定（大小写不敏感）。 */
export const isBundledRef = (ref) => /^bundled:/i.test(String(ref || '').trim())

/**
 * 归一化 `bundled:` 引用：去前缀、统一正斜杠、去前导斜杠、忽略大小写。
 *
 * 用户手写路径时正反斜杠混用很常见，样本引用不该因此失败。
 */
export const normalizeBundledKey = (ref) =>
  String(ref || '')
    .replace(/^bundled:/i, '')
    .replace(/\\/g, '/')
    .replace(/^\.?\/*/, '')
    .trim()
    .toLowerCase()

/**
 * 用一组「key → 文本」条目构建查询器。
 *
 * @param {Record<string, {text: string, format: string}>} entries
 */
export function buildSampleRegistry(entries) {
  const table = new Map()
  for (const [key, value] of Object.entries(entries || {})) {
    table.set(normalizeBundledKey(key), value)
  }

  return {
    list: () => BUNDLED_SAMPLE_FILES.filter((key) => table.has(normalizeBundledKey(key))),
    get(ref) {
      const key = normalizeBundledKey(ref)
      const hit = table.get(key)
      if (!hit) return null
      return { text: String(hit.text || ''), format: String(hit.format || ''), key }
    },
  }
}
