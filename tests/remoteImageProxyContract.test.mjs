// 远程图片代理的契约：地址与路由必须一致，旧前缀保留兼容，导出 HTML 要还原原图地址。
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const read = (p) => fs.readFileSync(path.join(root, p), 'utf8')
const rs = read('src-tauri/src/guaxiservices/markdown_image_process.rs')
const editor = read('src/components/HomeViewComponent/GuaxiEditor.vue')

// 1) 生成的地址与注册路由一致（曾经的 404 就是这两者差一个前缀）。
assert.ok(rs.includes('http://localhost:8888/markdown/api/remote_image?url={}'))
assert.ok(rs.includes('#[get("/markdown/api/remote_image")]'))
// 2) 旧前缀保留为兼容别名：缓存过的 HTML 里可能还留着它。
assert.ok(rs.includes('#[get("/api/markdown/remote_image")]'))
assert.ok(rs.includes('remote_image_legacy'))
// 3) 两个 Markdown 端点都走「Markdown 图片 + 原始 HTML 图片」的完整改写。
const handlerCount = (rs.match(/process_all_images\(&req\.content, &base_path\)/g) || []).length
assert.equal(handlerCount, 2, 'image_process / chat_process 应都使用 process_all_images')
// 4) 导出 HTML 还原代理地址；PDF 不还原。
assert.ok(editor.includes('const restoreRemoteImageProxyUrls = (html) =>'))
const htmlExport = editor.slice(editor.indexOf('const exportMarkdownRenderedHtml'))
assert.ok(htmlExport.slice(0, 400).includes('restoreRemoteImageProxyUrls(await renderMarkdownForExportHtml(context))'))

console.log('remoteImageProxy: ok')
