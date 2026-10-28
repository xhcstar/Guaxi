import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 邮件页几处「用户能直接看见」的约定，用源码断言锁住。
//
// 这些都是行为契约而不是实现细节：
//   1. 右键菜单只保留一个「移动」入口，其余文件夹进子菜单（Outlook 同一套）；
//   2. 子菜单列出当前账户**全部**可移动的文件夹，不能漏；
//   3. 侧栏不再有「更多文件夹」这种把用户自己的文件夹藏起来的分组；
//   4. 邮件行的光标是默认箭头，不是手型；
//   5. 正文头部展示发件人 / 收件人 / 抄送 / 时间，而不只是一行摘要。

const here = path.dirname(fileURLToPath(import.meta.url))
const read = (rel) => fs.readFileSync(path.join(here, '..', rel), 'utf8')

const mailApp = read('src/components/ApplicationViewComponent/apps/MailApp.vue')
const sidebar = read('src/components/ApplicationViewComponent/apps/mail/MailSidebar.vue')
const reader = read('src/components/ApplicationViewComponent/apps/mail/MailReader.vue')
const listPane = read('src/components/ApplicationViewComponent/apps/mail/MailListPane.vue')
const css = read('src/components/ApplicationViewComponent/apps/mail/mail.css')

test('右键菜单：只有一个「移动」入口，文件夹收进子菜单', () => {
  // 以前是「移到「X」」一长串按钮，文件夹多了会把删除等动作挤出屏幕。
  assert.ok(!/移到「\{\{ folder\.displayName \}\}」/.test(mailApp), '不应再有逐条列出的「移到」按钮')
  assert.ok(mailApp.includes('contextSubmenu'), '应当有子菜单状态')
  assert.ok(mailApp.includes('mail-context-submenu'), '应当渲染子菜单容器')
  assert.ok(/openSubmenu\('move', \$event\)/.test(mailApp), '移动条目应展开子菜单')
  // 子菜单必须挂在主菜单外面：主菜单是 overflow-y:auto 的滚动容器，
  // 写在里面的绝对定位子菜单会被裁掉（右键「移动」看不到文件夹）。
  assert.ok(
    /\.mail-context-submenu\s*\{[\s\S]{0,120}position:\s*fixed/.test(mailApp),
    '子菜单应 fixed 定位，不被主菜单的滚动容器裁掉',
  )
  const parentAt = mailApp.indexOf('is-parent')
  const parentBlock = mailApp.slice(parentAt, mailApp.indexOf('</div>', parentAt))
  assert.ok(parentAt >= 0 && !parentBlock.includes('mail-context-submenu'), '子菜单不应再嵌在父条目里')
})

test('移动子菜单：列出全部可移动文件夹，而不是只列几个角色', () => {
  assert.ok(!/const MOVE_ROLES = \[/.test(mailApp), '不应再用固定的角色白名单')
  assert.ok(
    /contextFolders[\s\S]{0,900}selectable !== false/.test(mailApp),
    '应包含所有可选的文件夹',
  )
  // 源文件夹与发件箱不能出现在「移动」目标里。
  assert.ok(/folder\.name !== current/.test(mailApp), '应排除当前所在文件夹')
  assert.ok(/folder\.role !== 'outbox'/.test(mailApp), '发件箱不是可移动目标')
})

test('侧栏：不再有「更多文件夹」分组', () => {
  // 注释里可能还会提到它（解释为什么去掉），这里只看实际的类名与计算属性。
  assert.ok(!/class="mail-folder mail-folder-more"/.test(sidebar), '不应再渲染「更多文件夹」条目')
  assert.ok(!/moreFolders/.test(sidebar), '不应保留更多文件夹的计算')
})

test('侧栏：自建文件夹全部平铺，且只显示用户自己建的', () => {
  assert.ok(sidebar.includes('primaryCustomFolders'), '自建文件夹应直接平铺')
  assert.ok(
    sidebar.includes('f.userCreated !== false'),
    '服务商带下来的模板目录不应默认出现在侧栏',
  )
})

test('邮件行：默认箭头光标，行内按钮才是手型', () => {
  const stripComments = (text) => text.replace(/\/\*[\s\S]*?\*\//g, '')
  // 只取 `.mail-row { ... }` 这一个规则块：紧随其后的行内按钮规则本来就该是手型。
  const start = css.indexOf('.mail-row {')
  const rowBlock = stripComments(css.slice(start, css.indexOf('}', start)))
  assert.ok(/cursor:\s*default/.test(rowBlock), '列表行应是默认箭头')
  assert.ok(!/cursor:\s*pointer/.test(rowBlock), '列表行不应是手型')
  assert.ok(
    /\.mail-row \.mail-star[\s\S]{0,160}cursor:\s*pointer/.test(css),
    '行内星标等按钮仍需手型',
  )
})

test('正文头部：发件人 / 收件人 / 抄送 / 时间逐项展示', () => {
  for (const label of ['发件人', '收件人', '抄送']) {
    assert.ok(reader.includes(`>${label}<`), `头部应展示「${label}」`)
  }
  assert.ok(reader.includes('mail-reader-when'), '头部应展示时间')
  assert.ok(reader.includes('fullDateLabel'), '时间应带完整提示')
})

test('正文头部：地址可悬停出卡片，并提供复制与加入通讯录', () => {
  assert.ok(reader.includes('MailAddressCard'), '阅读器应挂载地址卡片')
  assert.ok(reader.includes('mail_address_cards'), '应一次性批量取地址信息')
  const card = read('src/components/ApplicationViewComponent/apps/mail/MailAddressCard.vue')
  assert.ok(card.includes('复制地址'), '卡片应能复制邮箱地址')
  assert.ok(card.includes('加进通讯录'), '卡片应能把发件人加入通讯录')
  assert.ok(card.includes('mail_save_address_contact'), '应调用后端保存联系人')
})

test('发件箱：多次失败后在列表上方给出说明与重试', () => {
  assert.ok(listPane.includes('mail-outbox-banner'), '发件箱应有说明条')
  assert.ok(listPane.includes('retry-outbox'), '应能一键重试')
  assert.ok(mailApp.includes('onRetryOutbox'), '应实现重试处理')
})

test('存档是固定入口：缺文件夹时自动补建而不是报错', () => {
  assert.ok(
    /onArchiveCurrent[\s\S]{0,700}createFolder\('Archive'\)/.test(mailApp),
    '归档时若没有存档文件夹应自动创建',
  )
})

test('定时邮件在发件箱里可见，并能在到点前管理', () => {
  // 以前定时邮件只活在一个悬浮小条里：进了发件箱反而是空的，用户以为邮件丢了。
  assert.ok(listPane.includes('scheduledItems'), '发件箱应展示定时待发邮件')
  assert.ok(listPane.includes('manage-scheduled'), '应能从发件箱打开管理面板')
  assert.ok(mailApp.includes('folderScheduledItems'), '应计算当前发件箱的定时邮件')
  assert.ok(
    /folderScheduledItems[\s\S]{0,600}dueAt/.test(mailApp),
    '定时邮件按发送时间筛选',
  )
})

test('搜索框与输入框只有一圈焦点提示', () => {
  // 全局 :focus-visible 焦点环叠在输入框自己的边框上，看起来像「框里还有一个框」。
  assert.ok(
    /\.mail-search input:focus-visible\s*\{\s*outline:\s*0;?\s*\}/.test(css),
    '搜索框应取消内层焦点环',
  )
  assert.ok(
    /\.mail-input:focus-visible,\s*\.mail-textarea:focus-visible\s*\{\s*outline:\s*0;?\s*\}/.test(css),
    '带边框的输入框不应再叠外圈',
  )
})

test('正文表格：邮件里的 <pre> 不能撑坏表格布局', () => {
  // Outlook 会给 <pre> 写死 width:75em，自动表格布局据此把标签列压成一字宽。
  assert.ok(/\.mail-body pre\s*\{[\s\S]*?width:\s*auto !important/.test(css), '应解除 pre 的固定宽度')
  assert.ok(/\.mail-body pre\s*\{[\s\S]*?white-space:\s*pre-wrap/.test(css), '长行应允许换行')
  assert.ok(/\.mail-body td pre,[\s\S]{0,120}background:\s*transparent/.test(css), '单元格里的 pre 不应有底色')
})

test('收信规则：编辑器与弹层同底色，且条件支持从通讯录选人', () => {
  const rules = read('src/components/ApplicationViewComponent/apps/mail/MailRulesDialog.vue')
  assert.ok(/\.mail-rule-editor\s*\{[\s\S]*?background:\s*var\(--mail-panel\)/.test(rules), '编辑器底色应跟弹层一致')
  assert.ok(
    !/\.mail-rule-editor\s*\{[^}]*background:\s*var\(--mail-bg-sunken\)/.test(rules),
    '不应再用比面板更暗的凹陷色',
  )
  assert.ok(rules.includes('AddressPicker'), '条件应能打开通讯录选择器')
  assert.ok(rules.includes('togglePicker(\'from\')') && rules.includes('togglePicker(\'to\')'), '发件人和收件人都要有入口')
})

test('回复 / 转发直接长在阅读区顶部，不再弹满屏浮层', () => {
  assert.ok(reader.includes('name="inline-composer"'), '阅读区应留出内联回复插槽')
  // 插槽必须在正文滚动区里：回复占满一屏高度，原文/历史卡片排在它下面滚动查看。
  assert.ok(
    /mail-reader-scroll[\s\S]{0,400}name="inline-composer"/.test(reader),
    '回复卡片应位于正文滚动区顶部',
  )
  assert.ok(mailApp.includes('#inline-composer'), '应把回复编辑器放进插槽')
  assert.ok(/MailComposer[\s\S]{0,80}inline/.test(mailApp), '编辑器应以内联模式渲染')
  assert.ok(
    /\.mail-inline-composer\s*\{[\s\S]{0,60}height:\s*100%/.test(css),
    '内联卡片应占满正文区高度',
  )
  assert.ok(/\.mail-composer\.is-inline/.test(css), '内联卡片应有独立的尺寸/圆角样式')
})

test('写信编辑区：不叠焦点环，插入的表格跟随主题', () => {
  const composer = read('src/components/ApplicationViewComponent/apps/mail/MailComposer.vue')
  // 组件模板里引用了翻译面板却没 import，控制台会一直刷 "Failed to resolve component"。
  assert.ok(
    /defineAsyncComponent[\s\S]{0,200}GuaxiSelectionTranslatePanel/.test(composer),
    '翻译面板必须注册，否则每次渲染都会报未解析组件',
  )
  assert.ok(
    /\.mail-compose-editor:focus-visible[\s\S]{0,60}outline:\s*0/.test(css),
    '正文编辑区不应再叠全局焦点环',
  )
  assert.ok(
    /table\[data-guaxi-table\] thead th,[\s\S]{0,200}background:\s*var\(--mail-bg-sunken\)/.test(css),
    '表头底色应按主题覆盖邮件里的浅色 bgcolor',
  )
  // 这些样式只能落在「我们插入的表格 / 图片」上：引用原文里的表格是发件人排的版，
  // 套上 100% 宽与强制格线会让整块内容相对 Outlook 偏移（真实反馈过）。
  assert.ok(
    /\.mail-compose-editor table\[data-guaxi-table\] \{[\s\S]{0,200}width:\s*100%/.test(css),
    '表格宽度只作用于插入的表格',
  )
  assert.ok(
    !/\.mail-compose-editor table \{/.test(css),
    '不应对编辑区里所有表格（含引用原文）统一套样式',
  )
})
