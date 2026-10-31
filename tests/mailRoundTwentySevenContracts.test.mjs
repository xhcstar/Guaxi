import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第二十七轮：暗色链接可读、选区/光标、去掉 Markdown、字号、收发提速。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')
const mailDir = 'src/components/ApplicationViewComponent/apps/mail/'

const composer = read(mailDir + 'MailComposer.vue')
const css = read(mailDir + 'mail.css')
const colors = read(mailDir + 'colors.js')
const theme = read(mailDir + 'theme.js')
const storeJs = read('src/stores/mailStore.js')
const mailMod = read('src-tauri/src/commands/mail/mod.rs')
const mailModel = read('src-tauri/src/commands/mail/model.rs')
const mailStore = read('src-tauri/src/commands/mail/store.rs')
const contactStore = read('src-tauri/src/commands/mail/contacts/store.rs')
const contactMod = read('src-tauri/src/commands/mail/contacts/mod.rs')

test('暗色正文里的链接：按实际对比度兜底成主题链接色', () => {
  assert.ok(/const DARK_LINK = 'rgb\(154, 180, 255\)'/.test(colors), '深色纸面的链接色')
  assert.ok(
    /el\.matches\?\.\('a'\)[\s\S]{0,700}contrastRatio[\s\S]{0,300}DARK_LINK/.test(colors),
    '链接按实际颜色与背后底色的对比度兜底',
  )
  assert.ok(/\.mail-compose-editor a \{/.test(css), '编辑器里的链接也有主题色')
})

test('选区与光标：统一主题色，光标在亮底上也看得见', () => {
  assert.ok(/\.mail-body ::selection,\s*\n\.mail-compose-editor ::selection/.test(css), '阅读区与编辑器共用选区')
  assert.ok(/color-mix\(in srgb, var\(--mail-accent\) 34%, transparent\)/.test(css), '选区用主题色而不是默认蓝')
  assert.ok(
    /\.mail-body,\s*\n\.mail-compose-editor,\s*\n\.mail-compose-source \{\s*\n\s*caret-color: var\(--mail-accent\)/.test(css),
    '光标用主题色',
  )
})

test('写信编辑器不再提供 Markdown 转换', () => {
  assert.ok(!/convertMarkdown/.test(composer), '没有转换入口')
  assert.ok(!/Markdown 转成富文本/.test(composer), '没有对应按钮/提示')
  assert.ok(!/markdownToEmailHtml/.test(composer), '不再引用 Markdown 转换器')
  assert.ok(!/\.mail-md-mark/.test(css), '按钮样式一并删掉')
})

test('正文默认 16px，老设置自动迁移；界面文字整体 +1px', () => {
  assert.ok(/readingSize: 16/.test(storeJs), '前端默认 16')
  assert.ok(/fn default_reading_size\(\) -> u32 \{[\s\S]{0,120}16/.test(mailModel), '后端默认 16')
  assert.ok(/settings_reading_size_v1/.test(mailStore), '老设置一次性迁移')
  assert.ok(/Number\(readingSize \|\| 16\)/.test(theme), '兜底也是 16')
  assert.ok(/rowHeight: 78/.test(theme), '行高跟着字号抬高')
  for (const rule of ['\\.mail-row-subject', '\\.mail-row-from', '\\.mail-input,\\s*\\n\\.mail-textarea', '\\.mail-reader-subject']) {
    assert.ok(new RegExp(rule).test(css), `界面字号规则存在：${rule}`)
  }
})

test('收发提速：通讯录批量写、已发送副本和收录都不挡发送返回', () => {
  assert.ok(/pub fn touch_addresses/.test(contactStore), '通讯录支持一次连接批量写')
  assert.ok(/touch_addresses\(&own, &entries\)/.test(mailMod), '同步收信整批收录')
  assert.ok(
    /touch_addresses\(&own, &entries\)\?/.test(contactMod),
    '重建通讯录也整批写',
  )
  const sent = mailMod.slice(mailMod.indexOf('if save_to_sent {'), mailMod.indexOf('mail: 发送成功'))
  assert.ok(/spawn_blocking\(move \|\| \{[\s\S]{0,240}save_sent_copy/.test(sent), 'POP3 已发送副本后台写')
  assert.ok(/spawn_blocking\(move \|\| \{[\s\S]{0,320}append_to_folder/.test(sent), 'IMAP 已发送后台追加')
  assert.ok(!/\.await/.test(sent), '发送路径不再等已发送副本')
})
