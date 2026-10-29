import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 独立写信窗口的约定。用户提出的三条：
//   1. 写信默认开新窗口，不再挡住整个应用页；
//   2. 回复 / 转发默认在阅读区就地编辑，点按钮可以进新窗口；
//   3. 无论怎么关窗，只要没发出去就留一份草稿在草稿箱。

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const app = read('src/components/ApplicationViewComponent/apps/MailApp.vue')
const bridge = read(
  'src/components/ApplicationViewComponent/apps/mail/composeWindowBridge.js',
)
const entry = read('src/mailComposeWindow.js')
const rust = read('src-tauri/src/commands/mail_compose_window.rs')

test('「写邮件」默认开独立窗口', () => {
  assert.ok(/async function onCompose\(\)[\s\S]{0,400}openComposeInWindow/.test(app), '写邮件应开新窗口')
})

test('回复 / 转发默认就地编辑，可选新窗口', () => {
  assert.ok(
    /function onReply\(detail, all = false, \{ newWindow = false \}/.test(app),
    '回复默认不开窗',
  )
  assert.ok(
    /function onForward\(detail, \{ newWindow = false \}/.test(app),
    '转发默认不开窗',
  )
  assert.ok(app.includes('openComposeInWindow(composeIdFor('), '新窗口路径存在')
  const reader = read('src/components/ApplicationViewComponent/apps/mail/MailReader.vue')
  assert.ok(reader.includes("$emit('reply', detail, { newWindow: true })"), '阅读区应有「在新窗口里回复」按钮')
})

test('关窗一定先存草稿（只要还没发出去）', () => {
  assert.ok(
    /export async function closeWindow\(\{ save = true \} = \{\}\)[\s\S]{0,600}saveDraft\(\{ syncFolder: false \}\)/.test(
      bridge,
    ),
    'closeWindow 默认先 saveDraft（不等草稿箱同步，否则关闭会明显变慢）',
  )
  assert.ok(bridge.includes("invoke('mail_save_draft'"), '草稿要真正写进库')
  // 窗口壳的关闭走 closeWindow({ save: true })
  const shell = read('src/components/ApplicationViewComponent/apps/mail/MailComposeWindow.vue')
  assert.ok(shell.includes('closeWindow({ save: true })'), '窗口关闭按钮要保存草稿')
})

test('发送 / 定时发送成功后才跳过保存', () => {
  const shell = read('src/components/ApplicationViewComponent/apps/mail/MailComposeWindow.vue')
  // 四处「不保存」：错误页的关闭按钮（没有内容可存）+ 发送成功 + 定时发送成功 + 用户点了「直接放弃」。
  const sends = shell.match(/closeWindow\(\{ save: false \}\)/g) || []
  assert.equal(sends.length, 4, '错误页关闭 1 处 + 发送/定时发送/直接放弃各 1 处')
  const script = shell.slice(shell.indexOf('<script setup>'))
  assert.equal(
    (script.match(/closeWindow\(\{ save: false \}\)/g) || []).length,
    3,
    '脚本里只有「确实发出去了」或「用户明确放弃」才跳过保存',
  )
})

test('写信窗口有独立 html 入口与构建 entry', () => {
  assert.ok(fs.existsSync(path.join(root, 'mail-compose.html')), '需要 mail-compose.html')
  assert.ok(entry.includes('MailComposeWindow'), '入口挂载写信窗口组件')
  const vite = read('vite.config.js')
  assert.ok(vite.includes('mail-compose'), 'vite 需要把新入口收进 input')
})

test('后端：按草稿 id 复用窗口、请求走槽位而非 URL', () => {
  assert.ok(rust.includes('mail_open_compose_window'), '要能开窗')
  assert.ok(rust.includes('mail_take_compose_request'), '请求由槽位交接')
  assert.ok(/fn window_label\(draft_id: &str\)/.test(rust), '同一草稿复用同一窗口')
  assert.ok(!/ComposeRequest[\s\S]{0,200}query/.test(rust), '草稿正文不能塞进 URL')
})
