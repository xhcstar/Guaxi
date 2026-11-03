/**
 * 阅读器入口的产物依赖检查（`npm run check:reader-chunks`，构建后自动执行）。
 *
 * 盯的是一类只在 release 复现、dev 完全看不到的故障：
 *
 * 阅读器窗口走自己的入口 doc-reader.html，只挂 ReaderPane 子树。但如果 Rollup 把
 * 「阅读器与主应用共享的模块」归进了主应用入口 chunk（main-*.js），阅读器 chunk
 * 就会出现 `import ... from "./main-xxx.js"` 这条静态边。而 main chunk 的顶层正好
 * 执行着 src/main.js 的 bootstrap()，于是阅读器窗口会在异步加载某个阅读器时
 * 顺带启动整个主应用、把主应用挂到同一个 #app 上，把阅读器已经渲染好的 DOM 清空 ——
 * 表现为「点开 PDF / Office / 电子书，阅读器新窗口是空的」。
 *
 * dev 下 Vite 按源码模块图逐个请求，不会执行 main.js，所以这个问题只有构建产物里有。
 * 静态源码扫描看不出来，必须对 Rollup 的实际输出求依赖闭包，因此在 dist 上断言：
 *
 *   1. 从 doc-reader.html 出发的静态 import 闭包里，不得出现主应用入口 chunk；
 *   2. 任何会被阅读器窗口加载的 chunk（入口闭包 + 它动态 import 的分块）
 *      也不得 import 主应用入口 chunk。
 *
 * 仓库没有前端测试框架，沿用 tools/ 下其它 check 脚本的做法：直接 node 跑、失败退出非 0。
 */
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const distDir = path.join(root, 'dist')
const assetsDir = path.join(distDir, 'assets')

if (!fs.existsSync(assetsDir)) {
  console.log('readerChunks: 没有 dist/ 产物，跳过（只对构建产物生效）')
  process.exit(0)
}

/** 主应用入口 chunk：Rollup 里 index.html 的入口名是 main。 */
const isMainChunkName = (file) => /^main-[^/]*\.js$/.test(file)

/**
 * 真正危险的主应用入口 chunk —— 也就是「会自己启动应用」的那个。
 *
 * 不能只凭文件名判断：`main-*` 这个名字会被多个模块共用（实测还有几个几 KB 的
 * @vue/compat 辅助模块也叫 main-xxxx.js，它们完全无害）。危险的判据是内容：
 * 它有顶层 `mount("#app")`，导入它就会执行 main.js 的 bootstrap() 并清空页面。
 */
const mountsAppEntry = (file) => {
  if (!isMainChunkName(file)) return false
  const source = fs.readFileSync(path.join(assetsDir, file), 'utf8')
  return source.includes('mount("#app")')
}

/** 读出某个 js 文件的静态 import / 动态 import 目标（同一目录下的相对路径）。 */
const readChunkEdges = (file) => {
  const source = fs.readFileSync(path.join(assetsDir, file), 'utf8')
  const stat = new Set()
  const dyn = new Set()
  for (const m of source.matchAll(/(?:^|[;}\s])import\s*["']\.\/([^"']+)["']/g)) stat.add(m[1])
  for (const m of source.matchAll(/from\s*["']\.\/([^"']+)["']/g)) stat.add(m[1])
  for (const m of source.matchAll(/import\(\s*["']\.\/([^"']+)["']\s*\)/g)) dyn.add(m[1])
  return { stat: [...stat], dyn: [...dyn] }
}

const chunkFiles = new Set(fs.readdirSync(assetsDir).filter((f) => f.endsWith('.js')))
const edges = new Map([...chunkFiles].map((f) => [f, readChunkEdges(f)]))

/** 入口 HTML 引用的入口 chunk。 */
const entryChunkOf = (htmlFile) => {
  const html = fs.readFileSync(path.join(distDir, htmlFile), 'utf8')
  return [...html.matchAll(/src="\/assets\/([^"]+)"/g)]
    .map((m) => m[1])
    .filter((f) => chunkFiles.has(f))
}

/** 从一批起点出发，沿静态边求闭包。 */
const staticClosure = (starts) => {
  const seen = new Set()
  const stack = [...starts]
  while (stack.length) {
    const cur = stack.pop()
    if (seen.has(cur) || !edges.has(cur)) continue
    seen.add(cur)
    for (const next of edges.get(cur).stat) if (!seen.has(next)) stack.push(next)
  }
  return seen
}

// ── 断言 ────────────────────────────────────────────────
let failed = 0
const check = (name, fn) => {
  try {
    fn()
    console.log(`  ok   ${name}`)
  } catch (error) {
    failed += 1
    console.log(`  FAIL ${name}`)
    console.log(`       ${error.message.split('\n').join('\n       ')}`)
  }
}

// 独立窗口入口：doc-reader.html（文档阅读器）与 article-reader.html（外部资料全文阅读器）。
// 两者都走自己的入口、共用 reader-core，任何一方把主应用入口拉进来，都会在那个窗口里
// 执行 main.js 的 bootstrap() 并清空 #app —— 表现都是「新窗口是空的」。
const READER_ENTRIES = ['doc-reader.html', 'article-reader.html']

console.log('readerChunks:')

for (const entryHtml of READER_ENTRIES) {
  const entryChunks = entryChunkOf(entryHtml)
  console.log(`  ${entryHtml} 入口 chunk: ${entryChunks.join(', ') || '(未找到)'}`)

  check(`${entryHtml} 能找到入口 chunk`, () => {
    assert.ok(entryChunks.length > 0, `dist/${entryHtml} 里没有 <script type="module" src="/assets/...">`)
  })
  if (!entryChunks.length) continue

  const entryStatic = staticClosure(entryChunks)

  check(`${entryHtml} 入口的静态依赖闭包里没有主应用入口 chunk`, () => {
    const leaked = [...entryStatic].filter(mountsAppEntry)
    assert.deepEqual(
      leaked,
      [],
      `${entryHtml} 静态依赖了主应用入口：${leaked.join(', ')}\n` +
        '主应用入口 chunk 顶层会执行 main.js 的 bootstrap() 并 mount("#app")，\n' +
        '把阅读器已渲染的 DOM 清空。检查 vite.config.js 的 manualChunks 是否漏钉了共享模块。'
    )
  })

  check(`${entryHtml} 会用到的分块（含动态 import）也不依赖主应用入口`, () => {
    // 只沿「阅读器子树真实会加载的分块」追踪。
    //
    // 不能把入口闭包的所有动态 import 都算进来：reader-core 静态依赖了 docs chunk，
    // 而 docs 内部对 mermaid 的几十种语言数据是函数体里的动态 import（只有对应语言
    // 被渲染时才触发）。这些数据分块又反向引用 docs 的导出，把整个 dist 连成一张网，
    // 于是任何与主应用共享的模块都会被误报。真正要拦的是：阅读器自己（reader-core
    // 及其异步子阅读器）导入的模块，不能落到主应用入口上。
    const owned = new Set()
    const stack = [...entryStatic]
    while (stack.length) {
      const cur = stack.pop()
      if (owned.has(cur) || !edges.has(cur)) continue
      owned.add(cur)
      for (const next of edges.get(cur).stat) {
        if (!owned.has(next) && edges.has(next)) stack.push(next)
      }
    }
    const leaked = [...owned].filter(mountsAppEntry)
    assert.deepEqual(
      leaked,
      [],
      `这些阅读器会用到的分块依赖了主应用入口：${leaked.join(', ')}\n` +
        '打开对应类型的文档时，主应用会被整个启动并清空阅读器窗口。\n' +
        '把该分块用到的共享模块补进 vite.config.js 的 READER_SUBTREE_PATHS。'
    )
  })
}

if (failed) {
  console.log(`\nreaderChunks: ${failed} 项失败`)
  process.exit(1)
}
console.log('readerChunks: 全部通过')
