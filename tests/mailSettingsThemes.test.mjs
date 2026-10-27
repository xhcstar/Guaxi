import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse, compileScript } from '@vue/compiler-sfc'

// 「设置页里的主题配置不见了」这条真实反馈，根因不在主题代码本身：
// 同一个页面上的 `MailRulesDialog` 模板引用了脚本里不存在的 `senderRules` /
// `senderDraft`，Vue 渲染抛错后整棵树都停了，主题卡片因此是空的。
//
// 这个测试从两头锁住它：
// 1. 设置抽屉确实把 `themes` 渲染成卡片（不是被某个 v-if 挡住）；
// 2. 规则弹层需要的 senderRules 系列绑定齐全（渲染不再中断）。

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const mailDir = path.join(root, 'src/components/ApplicationViewComponent/apps/mail')
const read = (name) => fs.readFileSync(path.join(mailDir, name), 'utf8')

test('设置抽屉把主题渲染成可点击的卡片', () => {
  const drawer = read('MailSettingsDrawer.vue')
  assert.ok(drawer.includes('v-for="theme in themes"'), '主题应逐个渲染')
  assert.ok(drawer.includes('mail-theme-card'), '应有主题卡片样式类')
  assert.ok(drawer.includes('@click="$emit(\'update\', { theme: theme.id })"'), '点卡片应切换主题')
  // 主题网格不能被 v-if 收起来：它就是用户要找的那块配置。
  const gridBlock = drawer.slice(drawer.indexOf('mail-theme-grid'), drawer.indexOf('mail-theme-grid') + 120)
  assert.ok(!gridBlock.includes('v-if'), '主题网格不应被 v-if 隐藏')
})

test('设置抽屉收到的是非空主题列表', () => {
  const app = fs.readFileSync(
    path.join(root, 'src/components/ApplicationViewComponent/apps/MailApp.vue'),
    'utf8',
  )
  assert.ok(app.includes(':themes="themes"'), '应把主题列表传给设置抽屉')
  // 内置主题来自后端命令，后端必须仍然注册着它。
  const main = fs.readFileSync(path.join(root, 'src-tauri/src/main.rs'), 'utf8')
  assert.ok(main.includes('commands::mail::mail_builtin_themes'), '内置主题命令必须注册')
  const mod = fs.readFileSync(path.join(root, 'src-tauri/src/commands/mail/theme.rs'), 'utf8')
  assert.ok(/pub fn builtin_themes\(\) -> Vec<CustomTheme> \{\s*vec!\[ink\(\), paper\(\)\]/.test(mod),
    '后端应返回墨 / 纸两套内置主题')
})

test('规则弹层的发件人名单绑定齐全（渲染不再中断整个页面）', () => {
  const source = read('MailRulesDialog.vue')
  const { descriptor } = parse(source, { filename: 'MailRulesDialog.vue' })
  const compiled = compileScript(descriptor, { id: 'check', inlineTemplate: false })
  const bindings = Object.keys(compiled.bindings || {})
  for (const name of ['senderRules', 'senderDraft', 'addSenderRule']) {
    assert.ok(bindings.includes(name), `模板用到 ${name}，脚本里必须有对应绑定`)
  }
  // 「添加」按钮要真的能保存一条名单。
  assert.ok(source.includes("emit('save-sender-rule'"), '应能保存发件人名单')
})
