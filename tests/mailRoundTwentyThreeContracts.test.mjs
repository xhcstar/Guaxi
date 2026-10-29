import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第二十三轮：
//   1. 规则弹层里的「删除」确认被弹层盖住，点不到；
//   2. 编辑器里的图片还是拖不动；
//   3. 「立即对已有邮件执行」提示没命中——POP3 账户一封都没扫到。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')
const mailDir = 'src/components/ApplicationViewComponent/apps/mail/'

const composer = read(mailDir + 'MailComposer.vue')
const css = read(mailDir + 'mail.css')
const confirmHost = read(mailDir + 'MailConfirmHost.vue')
const mod = read('src-tauri/src/commands/mail/mod.rs')

test('确认框要压在所有弹层之上', () => {
  assert.ok(/mail-confirm-backdrop/.test(confirmHost), '确认框有自己的背景层类名')
  assert.ok(
    /\.mail-confirm-backdrop\s*\{[\s\S]{0,160}z-index: 200/.test(css),
    '层级高于普通弹层（80），否则在规则弹层里点删除会点到被盖住的按钮',
  )
})

test('图片移动：拦掉浏览器手势，并提供上移/下移', () => {
  const fn = composer.slice(composer.indexOf('function onEditorMouseDown'), composer.indexOf('function onEditorMouseDown') + 400)
  assert.ok(/event\.preventDefault\(\)/.test(fn), '按下图片时拦掉默认行为，否则收不到 mousemove')
  assert.ok(/function moveImageStep/.test(composer), '要有确定性的上移/下移')
  assert.ok(/moveImageStep\(-1\)/.test(composer) && /moveImageStep\(1\)/.test(composer), '浮条上两个按钮')
  assert.ok(/name="moveUp"/.test(composer) && /name="moveDown"/.test(composer), '用图标 + 提示')
})

test('规则对已有邮件执行：POP3 也要扫到、也要搬得动', () => {
  // 以前按 `!local_only` 过滤目标文件夹：POP3 的文件夹全是本地的 → 一封都扫不到。
  const scan = mod.slice(mod.indexOf('let targets: Vec<&MailFolder>'), mod.indexOf('let targets: Vec<&MailFolder>') + 500)
  assert.ok(/filter\(\|f\| f\.role != FolderRole::Outbox\)/.test(scan), '只排除发件箱')
  assert.ok(!/!f\.local_only/.test(scan), '不再用 local_only 排除')
  // 命中后的移动：POP3 走本地搬行，不能去连 IMAP。
  const move = mod.slice(mod.indexOf('async fn apply_rule_move'), mod.indexOf('async fn apply_rule_move') + 900)
  assert.ok(/pop3_sync::is_pop3\(account\)/.test(move), 'POP3 分支')
  assert.ok(/move_locally\(/.test(move), 'POP3 用本地移动')
})
