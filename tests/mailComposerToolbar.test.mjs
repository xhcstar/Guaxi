import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 写信工具栏的几条可见约定：
// 1. 对齐 / 列表 / 缩进等不再用汉字或符号当图标；
// 2. 每个工具按钮都有 title（悬停提示）；
// 3. 字体下拉列出本机全部字体，而不只是写死的 10 个；
// 4. 下拉列表跟随主题（深色下不能弹出一块白）。

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const composer = fs.readFileSync(
  path.join(root, 'src/components/ApplicationViewComponent/apps/mail/MailComposer.vue'),
  'utf8',
)
const css = fs.readFileSync(
  path.join(root, 'src/components/ApplicationViewComponent/apps/mail/mail.css'),
  'utf8',
)
const icons = fs.readFileSync(
  path.join(root, 'src/components/ApplicationViewComponent/apps/mail/MailIcon.vue'),
  'utf8',
)

test('对齐按钮用图标而不是汉字', () => {
  assert.ok(!/label: '左'|label: '中'|label: '右'|label: '两端'/.test(composer), '对齐不应再用汉字')
  for (const icon of ['alignLeft', 'alignCenter', 'alignRight', 'alignJustify']) {
    assert.ok(composer.includes(`icon: '${icon}'`), `对齐应使用 ${icon} 图标`)
    assert.ok(icons.includes(`${icon}:`), `MailIcon 里要有 ${icon}`)
  }
})

test('只在工具栏里用符号冒充图标的按钮都换成了图标', () => {
  // 这些符号以前直接当按钮内容：•≡ / 1≡ / ⇤ / ⇥ / ❝ / — / ↗ / { } / ⌫ / ↶ / ↷ / 📎 / 📚 / 🖼
  for (const glyph of ['•≡', '1≡', '⇤', '⇥', '❝', '{ }', '⌫', '↶', '↷', '📎', '📚']) {
    assert.ok(!composer.includes(glyph), `工具栏不应再出现字符图标 ${glyph}`)
  }
  for (const icon of ['listBullet', 'listOrdered', 'indentIncrease', 'indentDecrease',
    'quote', 'divider', 'link', 'code', 'clearFormat', 'undo', 'redo',
    'paperclip', 'book', 'image']) {
    assert.ok(icons.includes(`${icon}:`), `MailIcon 里要有 ${icon}`)
  }
})

test('工具栏按钮都带悬停提示', () => {
  // 抓出所有带 mail-fmt-btn 的 <button ...>。标签跨行、属性值里还含 `=>`，
  // 所以不能用「到第一个 > 为止」这种粗糙匹配。
  const buttons = [...composer.matchAll(/<button\b((?:[^>"']|"[^"]*"|'[^']*')*)>/g)].filter(
    (m) => m[1].includes('mail-fmt-btn'),
  )
  assert.ok(buttons.length > 10, `工具栏按钮数量异常：${buttons.length}`)
  for (const [, attrs] of buttons) {
    assert.ok(/\stitle="/.test(attrs) || /\s:title="/.test(attrs), `按钮缺少 title：${attrs.slice(0, 90)}`)
  }
})

test('字体下拉列出本机全部字体', () => {
  assert.ok(composer.includes('list_system_fonts'), '字体应来自系统字体枚举命令')
  assert.ok(composer.includes('systemFonts'), '应保存系统字体列表')
  assert.ok(composer.includes('<optgroup'), '常用字体与本机字体应分组展示')
})

test('下拉列表跟随主题（深色下不是白底）', () => {
  assert.ok(/\.mail-select option[\s\S]{0,200}background-color: var\(--mail-panel\)/.test(css),
    'option 应按主题着色')
  assert.ok(/color-scheme: light dark/.test(css), '下拉应声明 color-scheme，避免系统画成白底')
})
