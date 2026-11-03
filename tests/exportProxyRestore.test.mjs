// 外链图片走本地代理后，导出的 HTML 必须把代理地址还原成原始外链：
// 导出的文件要离开应用打开，而代理只存在于本机 8888。
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const editor = fs.readFileSync(path.join(root, 'src/components/HomeViewComponent/GuaxiEditor.vue'), 'utf8')

assert.match(editor, /const restoreRemoteImageProxyUrls = \(html\) =>/)
const htmlExport = editor.slice(editor.indexOf('const exportMarkdownRenderedHtml'))
assert.ok(
  htmlExport
    .slice(0, 400)
    .includes('restoreRemoteImageProxyUrls(await renderMarkdownForExportHtml(context))'),
  'HTML 导出没有还原代理地址',
)
// PDF 导出刻意不还原：它在当前进程内打印，走代理更稳。
const pdfSection = editor.slice(editor.indexOf('const exportMarkdownRenderedPdf'))
assert.ok(!pdfSection.slice(0, 400).includes('restoreRemoteImageProxyUrls(await renderMarkdownForExportHtml'))

console.log('exportProxyRestore: ok')
