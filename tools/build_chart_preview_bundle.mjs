// 把预览页要用的「图表解析 / option 组装」纯逻辑打成单文件 IIFE。
//
// 为什么需要它：`http://localhost:8888/browser/preview` 是后端直接吐出的静态 HTML，
// 页面里没有打包器，不能 import ES 模块。之前的做法是在页面里再写一份精简版解析器，
// 于是两边迟早漂移 —— 软件内已支持的树图 / 旭日 / 关系 / 热力 / K线 / 甘特，
// 以及「直接贴整段 ECharts option」的写法，在浏览器里统统报错。
//
// 现在把软件内同一批纯函数模块（不碰 DOM / Tauri）打进一个 IIFE，
// 由 Rust 侧 include_str! 随二进制分发。产物挂到 window.GuaxiChartPreview。
//
// 产物：src-tauri/assets/vendor/guaxi-chart-preview.js
// 何时需要重跑：改过 guaxiChartData / guaxiChartTypes / guaxiChartSamples 之后。
// 用法：node ./tools/build_chart_preview_bundle.mjs
import fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { build } from 'vite'

const log = (...args) => console.log('[build_chart_preview_bundle]', ...args)

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const PROJECT_ROOT = path.resolve(__dirname, '..')
const ENTRY = path.join(__dirname, 'chart_preview_entry.js')
const OUT_DIR = path.join(PROJECT_ROOT, 'src-tauri', 'assets', 'vendor')
const OUT_FILE = path.join(OUT_DIR, 'guaxi-chart-preview.js')

// 自检：缺了任何一个入口，页面就只能退化成「图表渲染失败」。
const REQUIRED_EXPORTS = [
  'parseChartFence',
  'normalizeDataset',
  'buildChartOption',
  'buildLightOption',
  'CHART_TYPES',
]

async function main() {
  log('start', { ENTRY, OUT_FILE })

  await fs.mkdir(OUT_DIR, { recursive: true })

  const result = await build({
    configFile: false,
    root: PROJECT_ROOT,
    logLevel: 'warn',
    build: {
      write: false,
      target: 'es2020',
      minify: 'esbuild',
      lib: {
        entry: ENTRY,
        formats: ['iife'],
        name: 'GuaxiChartPreviewBundle',
        fileName: () => 'guaxi-chart-preview.js',
      },
      rollupOptions: {
        output: {
          inlineDynamicImports: true,
          // 入口只往 globalThis 上挂号，不需要模块系统参与。
          exports: 'none',
        },
      },
    },
  })

  const chunk = result[0]?.output?.[0]
  if (!chunk || typeof chunk.code !== 'string') {
    throw new Error('vite 未返回可用的 bundle 产物')
  }
  for (const name of REQUIRED_EXPORTS) {
    if (!chunk.code.includes(name)) {
      throw new Error(`产物里找不到 ${name}，图表模块的导出可能已改名`)
    }
  }
  if (!chunk.code.includes('GuaxiChartPreview')) {
    throw new Error('产物里找不到 window.GuaxiChartPreview 挂载点')
  }

  await fs.writeFile(OUT_FILE, chunk.code, 'utf8')
  log('done', { OUT_FILE, bytes: Buffer.byteLength(chunk.code) })
}

main().catch((error) => {
  console.error('[build_chart_preview_bundle] failed:', error)
  process.exitCode = 1
})
