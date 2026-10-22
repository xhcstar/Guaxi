// 把 Excalidraw 官方导出器打成**静态页可直接加载**的单文件 IIFE。
//
// 为什么需要它：软件内的白板预览走 React 组件包（@excalidraw/excalidraw），
// 而 `http://127.0.0.1:8888/browser/preview` 是后端直接吐出的静态 HTML ——
// 没有打包器、没有模块解析、没有 React。要让那里的 ```excalidraw 围栏也能渲染，
// 只能预先构建一个自足的脚本，由 Rust 侧用 include_str! 随二进制分发。
//
// 产物：src-tauri/assets/vendor/excalidraw-export.js
//   - IIFE，挂到 window.GuaxiExcalidrawExport.exportToSvg
//   - 字体与图片都以 data URL 内联，离线可用
//   - 构建时注入 process/global 垫片：包内有少量代码按 Node 环境取值
//
// 何时需要重跑：升级 @excalidraw/excalidraw 之后。
// 用法：node ./tools/build_excalidraw_export_bundle.mjs
import fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { build } from 'vite'

const log = (...args) => console.log('[build_excalidraw_export]', ...args)

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const PROJECT_ROOT = path.resolve(__dirname, '..')
const ENTRY = path.join(PROJECT_ROOT, 'node_modules', '@excalidraw', 'excalidraw', 'dist', 'prod', 'index.js')
const OUT_DIR = path.join(PROJECT_ROOT, 'src-tauri', 'assets', 'vendor')
const OUT_FILE = path.join(OUT_DIR, 'excalidraw-export.js')

// 包内的 Node 垫片：静态页里既没有 process 也没有 global，
// 缺了它们 bundle 会在加载阶段直接抛 ReferenceError。
const NODE_SHIM = `window.process=${JSON.stringify({
  env: { NODE_ENV: 'production', DEV: false, PROD: true },
  platform: 'browser',
  argv: [],
  versions: {},
  browser: true,
})};window.global=window;\n`

async function main() {
  log('start', { ENTRY, OUT_FILE })

  await fs.mkdir(OUT_DIR, { recursive: true })

  // write:false 让我们先拿到产物文本、做一次自检，再决定是否落盘。
  // configFile:false —— 必须绕开项目根的 vite.config：它的 manualChunks 与 lib 模式互斥，
  // 而且那是给应用构建用的，不该影响这份独立产物。
  const result = await build({
    configFile: false,
    root: PROJECT_ROOT,
    logLevel: 'warn',
    resolve: {
      alias: {
        react: path.join(PROJECT_ROOT, 'node_modules', 'react'),
        'react-dom': path.join(PROJECT_ROOT, 'node_modules', 'react-dom'),
        'react/jsx-runtime': path.join(PROJECT_ROOT, 'node_modules', 'react', 'jsx-runtime.js'),
        'react/jsx-dev-runtime': path.join(PROJECT_ROOT, 'node_modules', 'react', 'jsx-dev-runtime.js'),
      },
      dedupe: ['react', 'react-dom'],
    },
    build: {
      write: false,
      target: 'es2020',
      minify: 'esbuild',
      lib: {
        entry: ENTRY,
        name: 'GuaxiExcalidrawExport',
        formats: ['iife'],
        fileName: () => 'excalidraw-export.js',
      },
      rollupOptions: {
        output: {
          inlineDynamicImports: true,
          exports: 'named',
          intro: NODE_SHIM,
        },
      },
    },
  })

  const chunk = result[0]?.output?.[0]
  if (!chunk || typeof chunk.code !== 'string') {
    throw new Error('vite 未返回可用的 bundle 产物')
  }
  if (!chunk.code.includes('exportToSvg')) {
    throw new Error('产物里找不到 exportToSvg，导出接口可能已改名')
  }

  await fs.writeFile(OUT_FILE, chunk.code, 'utf8')
  log('done', { OUT_FILE, bytes: Buffer.byteLength(chunk.code) })
}

main().catch((error) => {
  console.error('[build_excalidraw_export] failed:', error)
  process.exitCode = 1
})
