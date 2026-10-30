import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第十轮：
//   1. 存草稿不再判断「有没有内容 / 有没有收件人」——空草稿也要能存；
//   2. 发送旁边要有「放弃」入口，退出前让用户选「存草稿」还是「直接丢」；
//   3. 存完草稿 / 移动邮件后，侧栏的角标要实时更新。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const composer = read('src/components/ApplicationViewComponent/apps/mail/MailComposer.vue')
const shell = read('src/components/ApplicationViewComponent/apps/mail/MailComposeWindow.vue')
const bridge = read('src/components/ApplicationViewComponent/apps/mail/composeWindowBridge.js')
const store = read('src/stores/mailStore.js')
const mailApp = read('src/components/ApplicationViewComponent/apps/MailApp.vue')
const mod = read('src-tauri/src/commands/mail/mod.rs')
const palette = read('src/components/ApplicationViewComponent/apps/mail/palette.js')

test('存草稿：不再用「内容为空」把用户挡回去', () => {
  // 三个入口以前都会因为空内容直接 return：关编辑器 / 关独立窗口 / 定时暂存。
  assert.ok(!/const hasContent/.test(store), 'store.closeCompose 不再判断内容')
  assert.ok(!/const hasContent/.test(bridge), '独立窗口 saveDraft 不再判断内容')
  assert.ok(!/const hasContent/.test(composer), '编辑器 stashNow 不再判断内容')
  assert.ok(/哪怕收件人、主题、正文全空/.test(store), '注释说明为什么允许空草稿')
  // 后端草稿组装本来就不要求收件人（mime::build_draft），这里确认没被改回去。
  assert.ok(/mime::build_draft\(&request, &from, &account\.signature\)/.test(mod), '草稿走放宽校验的组装')
  // 但 lettre 自己的信封校验不放过空收件人（`missing destination address`），
  // 草稿路径必须自己补一封占位信封，否则「什么都不填」仍然存不进去。
  const mime = read('src-tauri/src/commands/mail/mime.rs')
  assert.ok(/!require_recipients[\s\S]{0,200}builder\.envelope\(/.test(mime), '空草稿要补占位信封')
  assert.ok(/build_draft_allows_completely_empty_message/.test(mime), '空草稿要有单测盯着')
})

test('放弃：发送旁边有入口，关之前让用户选存草稿还是直接丢', () => {
  assert.ok(/title="放弃这次编辑"/.test(composer), '发送旁要有放弃按钮')
  assert.ok(/discardWithDraft/.test(composer) && /存为草稿并关闭/.test(composer), '可选「存为草稿并关闭」')
  assert.ok(/discardForget/.test(composer) && /直接放弃/.test(composer), '可选「直接放弃」')
  assert.ok(/mail_delete_stashed_draft/.test(composer), '直接放弃要把本地暂存也删掉')
  assert.ok(/'discard'/.test(composer), '要能通知父级「不保存」')
  assert.ok(/function onDiscardCompose[\s\S]{0,200}store\.discardCompose\(\)/.test(mailApp), '主界面收到放弃就清掉编辑状态')
  assert.ok(/discardCompose\(\) \{/.test(store), 'store 要有不落库的放弃路径')
  assert.ok(/async function onDiscard\(\)[\s\S]{0,120}closeWindow\(\{ save: false \}\)/.test(shell), '独立窗口放弃时关窗且不存')
  assert.ok(/@close="onSaveAndClose"/.test(shell), '独立窗口里「存为草稿并关闭」也要真的存')
})

test('颜色：工具栏与单元格菜单共用一套常用色', () => {
  // 反馈原话：「常见颜色都要有，而不是仅仅都是偏白色的亮色」，以及「统一一下颜色」。
  for (const color of ['#000000', '#c00000', '#ff0000', '#ffc000', '#00b050', '#0070c0', '#7030a0']) {
    assert.ok(palette.includes(color), `常用色缺 ${color}`)
  }
  const fills = palette.slice(palette.indexOf('HIGHLIGHT_COLORS = ['))
  for (const color of ['#ddebf7', '#ffc000', '#ff7c80', '#00b050', '#0070c0']) {
    assert.ok(fills.includes(color), `填充色缺 ${color}`)
  }
  // 三处选颜色的地方都从 palette.js 取，不再各有一套。
  assert.ok(/export const TEXT_COLOR_LIST = STANDARD_COLORS\.flat\(\)/.test(palette), '文字色要拉平成共享清单')
  assert.ok(/export const FILL_COLOR_LIST = HIGHLIGHT_COLORS\.flat\(\)/.test(palette), '填充色要拉平成共享清单')
  assert.ok(/v-for="\(row, index\) in STANDARD_COLORS"/.test(composer), '工具栏字体颜色用共享色板')
  assert.ok(/v-for="\(row, index\) in HIGHLIGHT_COLORS"/.test(composer), '工具栏突出显示用共享色板')
  assert.ok(/FILL_COLOR_LIST : TEXT_COLOR_LIST/.test(composer), '单元格菜单用共享色板')
  assert.ok(/type="color"/.test(composer), '保留自定义取色器')
})

test('侧栏角标：存完草稿要重算草稿夹的计数', () => {
  // 「草稿」的数量读 folders 表；以前存完不重算，角标一直不动。
  assert.ok(
    /refresh_folder_totals\(&account\.id, &drafts\)/.test(mod),
    'IMAP 存完草稿要重算草稿夹计数',
  )
  assert.ok(
    /refresh_folder_totals\(&account\.id, "Drafts"\)/.test(mod),
    '本地草稿（POP3 / 离线兜底）也要重算',
  )
  // 移动 / 删除走的是前端先动角标、再等服务端 STATUS 校正。
  assert.ok(/adjustFolderCount\(accountId, folder, \{ total: -total, unread: -unseen \}\)/.test(store), '移动后来源夹角标先减')
  assert.ok(/adjustFolderCount\(accountId, target, \{ total, unread: unseen \}\)/.test(store), '目标夹角标先加')
  assert.ok(/scheduleFolderRefresh\(this, accountId\)/.test(store), '随后用服务端计数校正')
  // POP3 的邮件全在本机：移动 / 删除后就地重算，重启后不会读回旧数字。
  // 取「函数体」范围而不是「往后数 N 个字符」：函数体一长就会假失败。
  const moveStart = mod.indexOf('fn move_locally')
  assert.ok(moveStart >= 0, 'move_locally 必须还在')
  const moveBody = mod.slice(moveStart).match(/^[\s\S]*?\r?\n\}\r?\n/)?.[0] || ''
  assert.ok(
    /refresh_folder_totals\(account_id, target\)/.test(moveBody),
    'POP3 本地移动后目标夹也要重算',
  )
  assert.ok(
    (mod.match(/refresh_folder_totals\(&account_id, &folder\)/g) || []).length >= 2,
    'POP3 的两种删除入口都要重算来源夹',
  )
})
