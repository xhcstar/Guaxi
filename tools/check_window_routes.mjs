/**
 * 窗口 URL ↔ 路由 ↔ 多页入口 的一致性检查（`npm run check:window-routes`）。
 *
 * 盯的是一类很难从代码里看出来的故障：App 窗口的 URL 是一个**路由**，但仓库根目录
 * 恰好有一个同名入口 HTML。Tauri 的资源解析（manager/mod.rs 的 get_asset）和开发期
 * Vite 的 html fallback 都会先尝试 `<path>.html`，路由于是被入口整个遮蔽 —— 窗口起来
 * 的是另一个页面的组件，参数名对不上就停在「缺少文件路径」这类提示上（曾经真的发生过：
 * 入口叫 reader.html，而知识库的阅读窗口用的是 `/reader` 路由）。
 *
 * 因此这里断言两件事：
 *   1. 任何路由的首段，都不能与根目录某个入口 HTML 同名；
 *   2. Rust / 前端打开的每个应用内窗口 URL，都必须能对上一条路由或一个入口。
 *
 * 仓库没有前端测试框架，沿用 tools/ 下其它 check 脚本的做法：直接 node 跑、失败退出非 0。
 */
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')

const walk = (dir, filter) =>
  fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(dir, entry.name)
    if (entry.isDirectory()) return walk(full, filter)
    return filter(full) ? [full] : []
  })

const firstSegment = (url) =>
  url
    .replace(/\{[^}]*\}/g, '')          // format! 的插值占位符，如 /library-site{}
    .replace(/^\//, '')
    .split(/[?#]/)[0]
    .split('/')[0]

// ── 事实收集 ─────────────────────────────────────────────
const entryFiles = fs.readdirSync(root).filter((name) => name.endsWith('.html'))
const entryNames = new Set(entryFiles.map((name) => name.replace(/\.html$/, '')))

const routerSource = fs.readFileSync(path.join(root, 'src', 'router', 'index.js'), 'utf8')
const routePaths = [...routerSource.matchAll(/path:\s*'([^']+)'/g)].map((m) => m[1])
const routeSegments = new Set(
  routePaths.map((p) => p.replace(/^\//, '').split('/')[0]).filter(Boolean)
)

/** Rust 侧用 WebviewUrl::App 打开的窗口。 */
const rustUrls = []
for (const file of walk(path.join(root, 'src-tauri', 'src'), (f) => f.endsWith('.rs'))) {
  const text = fs.readFileSync(file, 'utf8')
  for (const m of text.matchAll(/WebviewUrl::App\((?:format!\()?"([^"]+)"/g)) {
    rustUrls.push({ url: m[1], file: path.relative(root, file) })
  }
}

/** 前端用 new WebviewWindow({ url }) 打开的窗口（只认纯字面量 URL）。 */
const jsUrls = []
const frontendFiles = walk(path.join(root, 'src'), (f) => /\.(vue|js|mjs)$/.test(f))
for (const file of frontendFiles) {
  const text = fs.readFileSync(file, 'utf8')
  for (const m of text.matchAll(/new WebviewWindow\([^)]*?url:\s*['"`](\/[^'"`$]*)['"`]/gs)) {
    jsUrls.push({ url: m[1], file: path.relative(root, file) })
  }
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

console.log('windowRoutes:')
console.log(`  入口: ${entryFiles.join(', ')}`)
console.log(`  路由: ${routePaths.length} 条，窗口 URL: ${rustUrls.length + jsUrls.length} 处`)

check('路由首段不与入口 HTML 同名（否则入口会遮蔽路由）', () => {
  const collisions = [...routeSegments].filter((seg) => entryNames.has(seg))
  assert.deepEqual(collisions, [], `这些路由被同名入口遮蔽了：${collisions.join(', ')}`)
})

check('Rust 打开的窗口 URL 都能落到路由或入口上', () => {
  const dangling = rustUrls.filter(({ url }) => {
    const seg = firstSegment(url)
    return !routeSegments.has(seg) && !entryFiles.includes(seg)
  })
  assert.deepEqual(
    dangling,
    [],
    dangling.map(({ url, file }) => `${file}: ${url} 既没有路由也没有入口`).join('\n')
  )
})

check('前端打开的窗口 URL 都能落到路由上', () => {
  const dangling = jsUrls.filter(({ url }) => !routeSegments.has(firstSegment(url)))
  assert.deepEqual(
    dangling,
    [],
    dangling.map(({ url, file }) => `${file}: ${url} 没有对应路由`).join('\n')
  )
})

check('public/ 里的 HTML 不会遮蔽同名路由', () => {
  const publicDir = path.join(root, 'public')
  if (!fs.existsSync(publicDir)) return
  const publicHtml = walk(publicDir, (f) => f.endsWith('.html')).map((f) =>
    path.relative(publicDir, f).replace(/\\/g, '/').replace(/\.html$/, '')
  )
  const routePathsNoSlash = routePaths.map((p) => p.replace(/^\//, ''))
  const collisions = publicHtml.filter((p) => routePathsNoSlash.includes(p))
  assert.deepEqual(collisions, [], `public/ 下的 HTML 遮蔽了路由：${collisions.join(', ')}`)
})

if (failed) {
  console.log(`\nwindowRoutes: ${failed} 项失败`)
  process.exit(1)
}
console.log('windowRoutes: 全部通过')
