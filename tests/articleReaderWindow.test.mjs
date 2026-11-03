// 「查看全文」独立阅读窗口的路由参数与构建契约。
//
// 背景：收件箱的「查看全文」从 n-modal 搬进独立窗口后，这条链路上任何一处漂移
// （入口 HTML 没登记、窗口 URL 与入口对不上、label 前缀掉出 capabilities、
// 入口把主应用 chunk 静态拉进来）表现都是「点开一个空白窗口」，而且 release 才复现。
// 与 tests/docReaderWindow.test.mjs 同一套思路：读源码做静态断言，不跑 Tauri。
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const read = (p) => fs.readFileSync(path.join(root, p), 'utf8')

const entryHtml = read('article-reader.html')
const entryMain = read('src/articleReaderMain.js')
const readerWindowRs = read('src-tauri/src/commands/doc_reader_window.rs')
const mainRs = read('src-tauri/src/main.rs')
const viteConfig = read('vite.config.js')
const inbox = read('src/components/BrainViewComponent/GuaxiCognitionInbox.vue')
const readerComponent = read('src/components/ReaderComponent/MarkdownArticleReader.vue')
const capabilities = read('src-tauri/capabilities/default.json')
const readerChunkCheck = read('tools/check_reader_chunks.mjs')

// 1) 入口存在，且只挂自己的入口脚本。
assert.match(entryHtml, /<script type="module" src="\/src\/articleReaderMain\.js"><\/script>/)
assert.ok(!entryHtml.includes('/src/main.js'), '阅读窗口入口不能挂主应用 main.js')

// 2) 专用入口必须登记进 Vite 的多页构建，否则开发期能开、release 里 404。
assert.match(viteConfig, /"article-reader":\s*path\.resolve\(projectRoot, "article-reader\.html"\)/)

// 3) 窗口命令：存在、打开的是 article-reader.html、参数只有 id/title/theme。
assert.match(readerWindowRs, /pub async fn create_article_reader_window\(/)
assert.match(readerWindowRs, /"\/article-reader\.html\?/)
assert.match(readerWindowRs, /query = format!\("id=\{\}&title=\{\}"/)

// 4) 窗口 label 沿用 doc-reader- 前缀：capabilities 里已放行 doc-reader-*，
//    换前缀就会让窗口里所有 invoke 被拒（看起来像「按钮全失灵」）。
assert.match(readerWindowRs, /reader_window_label\(&format!\("article:\{id\}"\)\)/)
assert.match(capabilities, /"doc-reader-\*"/)

// 5) 命令必须注册，否则前端 invoke 直接报 not found。
assert.match(mainRs, /commands::doc_reader_window::create_article_reader_window/)

// 6) 收件箱：调新命令，且不再把全文塞进弹窗。
assert.match(inbox, /invoke\('create_article_reader_window'/)
assert.ok(!/showFull/.test(inbox), '收件箱里不应再有全文弹窗状态')
assert.ok(!/fullText\.value/.test(inbox), '收件箱不应再把全文读进本地状态')

// 7) 阅读窗口里三项能力的接线：同一个 Markdown 引擎 + 朗读 + 对照翻译 + 大纲。
assert.match(entryMain, /MarkdownArticleReader\.vue/)
assert.match(readerComponent, /renderMarkdown\(/)
assert.match(readerComponent, /useReaderReadAloud\(/)
assert.match(readerComponent, /useBilingualPreview\(/)
assert.match(readerComponent, /querySelectorAll\('h1, h2, h3, h4, h5, h6'\)/)

// 8) release 的空窗口守卫必须覆盖这扇新入口（主应用 chunk 被静态拉进来 = 窗口空白）。
assert.match(readerChunkCheck, /'article-reader\.html'/)

// 9) 入口脚本不得静态引入主应用模块（App.vue / main.js / 路由）。
for (const forbidden of ["'./App.vue'", "'./main.js'", "'./router'"]) {
  assert.ok(!entryMain.includes(forbidden), `articleReaderMain.js 不应引入 ${forbidden}`)
}

console.log('articleReaderWindow: ok')
