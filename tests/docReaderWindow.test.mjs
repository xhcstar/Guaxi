// 独立阅读器窗口的路由参数回归测试。
//
// 背景：文档阅读器从编辑器标签页搬到独立窗口后，窗口靠 URL query 传
// path / name / kind / reader / theme。这条链路一旦出错（参数名拼错、
// 中文路径没编码、reader 判定丢失），表现就是「点开文档弹出一个空白窗口」，
// 而且不会有任何前端报错 —— 正是最该用测试锁住的那种静默失败。
import assert from 'node:assert/strict'
import { classifyByExtension, READER_KIND } from '../src/utils/openTargetKind.js'

// 1) 阅读器类型判定的稳定性：这些扩展名必须走阅读器，而不是 Monaco。
const readerCases = [
  ['.epub', 'ebook'],
  ['.mobi', 'ebook'],
  ['.azw3', 'ebook'],
  ['.pdf', 'pdf'],
  ['.docx', 'word'],
  ['.xlsx', 'spreadsheet'],
  ['.pptx', 'slides'],
  ['.png', 'image'],
]
for (const [ext, expectedReader] of readerCases) {
  const target = classifyByExtension(`D:/docs/sample${ext}`)
  assert.equal(
    target?.reader,
    expectedReader,
    `${ext} 应判定为 ${expectedReader}，实际 ${target?.reader}`
  )
  assert.notEqual(target.reader, READER_KIND.TEXT, `${ext} 不应走文本编辑器`)
}

// 2) 文本类型必须仍是 text —— 窗口拆分不能把 Markdown 从编辑器里抢走。
for (const ext of ['.md', '.markdown', '.txt']) {
  const target = classifyByExtension(`D:/docs/note${ext}`)
  assert.equal(target?.reader, READER_KIND.TEXT, `${ext} 必须留在编辑器`)
}

// 3) 中文与空格路径：窗口参数用 encodeURIComponent 编码，解码后必须原样还原。
const trickyPath = 'D:/我的 文档/拜占庭的失落之城.mobi'
const encoded = encodeURIComponent(trickyPath)
assert.ok(!encoded.includes('/'), '编码后不应残留路径分隔符')
assert.equal(decodeURIComponent(encoded), trickyPath, '编码往返必须无损')

// 4) 编码后的值能安全拼进 query（不会产生额外的 & 或 =）。
assert.ok(!encoded.includes('&') && !encoded.includes('='), '编码后不应含 query 分隔符')

// 5) 反斜杠路径同样要能被判定（Windows 文件树传的是反斜杠）。
const winTarget = classifyByExtension('D:\\docs\\sample.mobi')
assert.equal(winTarget?.reader, 'ebook', 'Windows 反斜杠路径也必须能判定')

console.log('docReaderWindow: ok')
