import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { attachmentIcon } from '../src/components/ApplicationViewComponent/apps/mail/attachmentIcons.js'

// 第二十八轮：
//   1. 工单邮件里的「孤儿 <li>」把圆点画到了表格外；
//   2. 附件清单里所有文件共用一个「📄」，看不出文件类型。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')
const mailDir = 'src/components/ApplicationViewComponent/apps/mail/'

const css = read(mailDir + 'mail.css')
const reader = read(mailDir + 'MailReader.vue')
const composer = read(mailDir + 'MailComposer.vue')

test('孤儿 <li> 的圆点回到行内，不再越过单元格边框', () => {
  // Trac / Bugzilla 常把附件清单写成散落的 <li>：没有 <ul> 包着，直接放在 <td> 里。
  // 默认 outside 会把标记画在内容盒之外，一直往左挤到表格边框外。
  assert.ok(
    /\.mail-body li:not\(ul li\):not\(ol li\):not\(menu li\)\s*\{[\s\S]{0,80}list-style-position:\s*inside/.test(
      css,
    ),
    '不在列表容器里的 <li> 要改用 inside',
  )
  // 真正的列表要保留默认（outside）：缩进与悬挂缩进都来自列表容器，不能被一起改掉。
  assert.ok(
    !/\.mail-body (ul|ol)\s*li\s*\{[\s\S]{0,60}list-style-position:\s*inside/.test(css),
    'ul / ol 里的 li 不动',
  )
})

test('附件图标按文件类型区分，而不是统一一个纸图标', () => {
  const byName = (filename) => attachmentIcon({ filename }).body
  const xls = byName('Unit_Test_Report_Bug_135985.xls')
  const pdf = byName('设计说明.pdf')
  const zip = byName('logs.zip')
  const doc = byName('需求.docx')
  const img = byName('截图.png')
  const unk = byName('blob.dat')
  for (const [label, body] of Object.entries({ xls, pdf, zip, doc, img, unk })) {
    assert.ok(body && typeof body === 'string', `${label} 应能解析出一个图标`)
  }
  const distinct = new Set([xls, pdf, zip, doc, img])
  assert.equal(distinct.size, 5, '表格 / PDF / 压缩包 / 文档 / 图片应是五个不同图标')
  assert.notEqual(xls, unk, '未知类型与表格不能共用图标')
})

test('文件名没有扩展名时按 content-type 兜底', () => {
  const byExcelMime = attachmentIcon({
    filename: 'report',
    contentType: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  })
  const byXlsName = attachmentIcon({ filename: 'report.xls' })
  assert.equal(byExcelMime.body, byXlsName.body, 'MIME 指代表格时要和 .xls 同一个图标')
  const octetStream = attachmentIcon({
    filename: 'payload.bin',
    contentType: 'application/octet-stream',
  })
  assert.ok(octetStream.body, 'octet-stream 也不能是空图标')
})

test('阅读区与写信区都改用类型图标', () => {
  for (const [label, source] of [
    ['阅读区', reader],
    ['写信区', composer],
  ]) {
    assert.ok(/attachmentIcon\(/.test(source), `${label}要按文件类型取图标`)
    assert.ok(!/iconFor\(att\.contentType\)/.test(source), `${label}不再用统一的纸图标`)
    assert.ok(/<Icon[\s\S]{0,80}:icon="attachmentIcon\(/.test(source), `${label}用彩色类型图标渲染`)
  }
  // 危险附件仍用警告标记：类型图标会把「这是可执行文件」这件事藏起来。
  assert.ok(/isDangerous\(att\.name\)/.test(composer), '危险类型仍单独标出')
})
