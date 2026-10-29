import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第十五轮：正文里出现「一大块空白」，控制台刷满
// `GET cid:... net::ERR_UNKNOWN_URL_SCHEME`。
//
// 两条来路：
//   1. 粘贴图片时先写 `cid:` 再等下一帧替换 —— 父级的 inlineImages 是异步更新的，
//      那一帧常常还查不到映射，cid: 就留在正文里变成一张永远加载不出来的图；
//   2. 自动保存序列化时把 cid: 写回游离的克隆节点，浏览器真的去加载了一遍。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const composer = read('src/components/ApplicationViewComponent/apps/mail/MailComposer.vue')
const css = read('src/components/ApplicationViewComponent/apps/mail/mail.css')

test('粘贴图片：直接写本机地址，正文里不出现 cid:', () => {
  assert.ok(
    /src="\$\{convertFileSrc\(inline\.path\)\}" data-cid=/.test(composer),
    '插入时就换成可显示的 asset 地址',
  )
  assert.ok(!/src="cid:\$\{inline\.contentId\}"/.test(composer), '不再先塞 cid: 再等替换')
})

test('找不到落盘文件的 inline 图：用占位，不留坏图', () => {
  assert.ok(/MISSING_INLINE_PIXEL/.test(composer), '有 1×1 透明占位')
  assert.ok(/classList\.add\('is-missing-inline'\)/.test(composer), '标记出来便于样式处理')
  assert.ok(
    /function resolveCidImages\(\)[\s\S]{0,400}老草稿的正文里可能还留着 cid:/.test(composer),
    '没有 inlineImages 也要处理（老草稿）',
  )
  assert.ok(/\.mail-compose-editor img\.is-missing-inline/.test(css), '占位块要有样式')
})

test('粘贴富文本：从别的邮件复制来的 cid 图直接丢掉', () => {
  // 这类图片的数据带不过来，留着只会是坏图 + 一大块空白。
  assert.ok(
    /child\.tagName === 'IMG' && \/\^\\s\*cid:\/i\.test\(child\.getAttribute\('src'\) \|\| ''\)/.test(composer),
    '粘贴清洗时要识别 cid 图',
  )
  assert.ok(/node\.removeChild\(child\)\s*\n\s*continue/.test(composer), '识别到就把这个 img 丢掉')
})

test('写信编辑器：载入草稿时把嵌套引用压平成卡片列表', () => {
  // 内联回复的引用在 buildQuote 里已经压平了；从草稿恢复进独立写信窗口时没压，
  // 于是又出现「一层套一层」的卡片。
  assert.ok(/import \{ flattenQuoteCards \} from '\.\/quoteCards'/.test(composer), '编辑器要引入压平函数')
  assert.ok(
    /editorRef\.value\.innerHTML = props\.draft\.html \|\| ''[\s\S]{0,300}flattenQuoteCards\(editorRef\.value\)/.test(
      composer,
    ),
    '灌入草稿后立刻压平',
  )
  assert.ok(
    /flattenQuoteCards\(editorRef\.value\)\s*\n\s*lastHtml = editorRef\.value\.innerHTML/.test(composer),
    '压平后的结果才是自动保存的基线',
  )
})

test('序列化：在惰性 template 里改，不触发图片加载', () => {
  assert.ok(/document\.createElement\('template'\)/.test(composer), '用 template 承载副本')
  assert.ok(/host\.innerHTML = editorRef\.value\.innerHTML/.test(composer), '从 innerHTML 取副本')
  assert.ok(/const clone = host\.content/.test(composer), '在 template 的 content 上操作')
  assert.ok(/return host\.innerHTML/.test(composer), '序列化 template 的内容')
  assert.ok(!/editorRef\.value\.cloneNode\(true\)/.test(composer), '不再直接克隆到游离节点')
})
