import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 引用卡片不再层层叠加。
//
// 现象（用户截图）：回复 / 转发一封「本来就带着历史卡片」的邮件时，新的引用卡片
// 会把上一层整块历史包进自己里面，于是卡片一层套一层、越往里缩得越深。
// 期望：按时间从上到下列成同级卡片，最新在最上面。
//
// 做法：写信侧生成引用时先拍平，阅读侧渲染时再拍平一次（老邮件与别的客户端
// 发来的嵌套引用同样受益）。两处共用 quoteCards.js，避免各写一份而走样。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const cards = read('src/components/ApplicationViewComponent/apps/mail/quoteCards.js')
const reader = read('src/components/ApplicationViewComponent/apps/mail/MailReader.vue')
const mailApp = read('src/components/ApplicationViewComponent/apps/MailApp.vue')

test('拍平实现：把嵌套卡片提到父卡片之后', () => {
  assert.ok(/export function flattenQuoteCards/.test(cards), '应导出拍平实现')
  assert.ok(/export function collectQuoteCards/.test(cards), '卡片识别要能被阅读侧复用')
  assert.ok(
    /parent\.insertAdjacentElement\('afterend', card\)/.test(cards),
    '内层卡片要紧跟在父卡片后面（顺序＝新在上、往下更早）',
  )
  // 卡片标记必须写在 data-* 上：class 会被收件清洗层剥掉，data-* 会留着。
  assert.ok(/CARD_MARK = 'data-guaxi-quote-card'/.test(cards), '识别自己生成的卡片要靠 data 标记')
})

test('写信侧：引用生成后先拍平再交给编辑器', () => {
  assert.ok(/import \{ flattenQuoteHtml \}/.test(mailApp), 'MailApp 要引入拍平函数')
  assert.ok(
    /return \{ text: textBody, html: flattenQuoteHtml\(htmlBody\) \}/.test(mailApp),
    'buildQuote 返回的 HTML 必须是拍平后的',
  )
  // 转发以前只改主题、正文是空的，用户得自己复制原文；现在同样带上引用卡片。
  const forward = mailApp.slice(mailApp.indexOf('function onForward'))
  assert.ok(/buildQuote\(detail\)/.test(forward), '转发也要带上原文卡片')
})

test('阅读侧：渲染时同样拍平（老邮件与别的客户端的嵌套引用）', () => {
  assert.ok(/flattenQuoteCards\(doc\.body\)/.test(reader), '渲染前先拍平')
  assert.ok(
    reader.indexOf('flattenQuoteCards(doc.body)') < reader.indexOf('collectQuoteCards(doc.body)'),
    '必须先拍平再收集卡片',
  )
  assert.ok(
    !reader.includes('写道|wrote'),
    '卡片识别只留一份实现（quoteCards.js），阅读侧不再各写一份',
  )
  assert.ok(cards.includes('写道|wrote'), '识别线索本身要保留在共用实现里')
})
