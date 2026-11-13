// 关联文档（双链）导出的回归约束：
// 1) 复制被链接文档必须走后端命令，不能用 fs 插件的 mkdir/copyFile —— 插件只放行
//    $TEMP，导出目录不在 scope 内，会以 "forbidden path" 直接放弃整条链路。
// 2) 关联文档表必须同时登记绝对路径与原始链接文本：卡片改写按前者查，Markdown
//    链接改写按后者查，缺一种就会出现「Word/EPUB 里只有裸文本」。
// 3) 导出 HTML/PDF 渲染前必须像实时预览一样预处理 [[双链]]，否则卡片不会生成。
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const editor = fs.readFileSync(path.join(root, 'src/components/HomeViewComponent/GuaxiEditor.vue'), 'utf8')

const collectSection = editor.slice(
  editor.indexOf('const collectExportLinkedDocuments'),
  editor.indexOf('const rewriteExportDocumentLinks'),
)

assert.ok(collectSection.length > 0, '找不到 collectExportLinkedDocuments')

assert.ok(
  !collectSection.includes("await import('@tauri-apps/plugin-fs')"),
  '关联文档复制不能再用 fs 插件：allow-mkdir 只放行 $TEMP，会静默放弃打包',
)
assert.ok(
  collectSection.includes("invoke('read_file_bytes_base64'"),
  '关联文档复制应走 read_file_bytes_base64',
)
assert.ok(
  collectSection.includes("invoke('write_file_bytes_base64'"),
  '关联文档复制应走 write_file_bytes_base64（它会自动创建父目录）',
)

// 双键登记：abs 给卡片改写，原始 target 给 Markdown 链接改写。
assert.ok(collectSection.includes('assets.set(abs, destRel)'), '关联文档表缺少绝对路径键')
assert.ok(collectSection.includes('assets.set(target, destRel)'), '关联文档表缺少原始链接文本键')

// 导出 HTML/PDF 的双链预处理。
const exportHtmlSection = editor.slice(
  editor.indexOf('const renderMarkdownForExportHtml'),
  editor.indexOf('const restoreRemoteImageProxyUrls'),
)
assert.ok(
  exportHtmlSection.includes('preprocessObsidianWikilinks('),
  '导出 HTML/PDF 渲染前必须先预处理 [[双链]]，否则卡片无法生成',
)

// Word/EPUB 的链接改写仍按原始链接文本查表。
const pandocRewriteSection = editor.slice(
  editor.indexOf('const rewriteExportLinkedMarkdown'),
  editor.indexOf('const exportMarkdownPandoc'),
)
assert.ok(
  pandocRewriteSection.includes('assets.get(target)'),
  'Word/EPUB 的双链改写应按原始链接文本查表',
)

console.log('exportLinkedDocCards: ok')
