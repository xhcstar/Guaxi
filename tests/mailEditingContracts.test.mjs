import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 写信 / 阅读两处「用户直接看得见」的约定。
//
// 对应的反馈（逐条）：
//   1. 新窗口里的引用历史必须和阅读区同一套配色，不能白底浅字；
//   2. 划词翻译只在 Alt+划词时出现，关得掉、拖得动；
//   3. 插进正文的图片 / 表格要能改大小、位置和样式；
//   4. 删账户时要问一次「本机邮件要不要一起删」；
//   5. 只要没发送，退出编辑就必须把刚写的内容存进草稿箱（且不能堆一串空壳）；
//   6. 收信正文里不能出现突兀的黑色块。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const composer = read('src/components/ApplicationViewComponent/apps/mail/MailComposer.vue')
const reader = read('src/components/ApplicationViewComponent/apps/mail/MailReader.vue')
const translate = read(
  'src/components/ApplicationViewComponent/apps/mail/useMailSelectionTranslate.js',
)
const colors = read('src/components/ApplicationViewComponent/apps/mail/colors.js')
const css = read('src/components/ApplicationViewComponent/apps/mail/mail.css')
const app = read('src/components/ApplicationViewComponent/apps/MailApp.vue')
const bridge = read('src/components/ApplicationViewComponent/apps/mail/composeWindowBridge.js')
const shell = read('src/components/ApplicationViewComponent/apps/mail/MailComposeWindow.vue')
const accountDialog = read('src/components/ApplicationViewComponent/apps/mail/MailAccountDialog.vue')
const store = read('src/stores/mailStore.js')
const rustMail = read('src-tauri/src/commands/mail/mod.rs')

test('新窗口的引用历史：与阅读区共用同一套配色映射，且不把映射结果写进草稿', () => {
  assert.ok(composer.includes('applyBodyThemeToElements'), '写信区应复用 colors.js 的映射')
  assert.ok(/data-guaxi-quote-card/.test(composer), '只对引用卡片做映射，不动用户自己写的内容')
  assert.ok(
    /function serializeEditorHtml[\s\S]{0,600}applyBodyThemeToElements\(themed, 'light'\)/.test(composer),
    '序列化草稿前要把映射结果还原成邮件原值',
  )
  assert.ok(
    /props\.themeMode === 'light' \? 'light' : 'dark'/.test(composer),
    '映射要跟着当前主题走',
  )
})

test('划词翻译：只认 Alt+划词，面板内的点击不触发', () => {
  assert.ok(translate.includes('altKey'), '必须检查 Alt 修饰键')
  assert.ok(
    /closest\?\.\('\.guaxi-selection-translate-panel'\)/.test(translate),
    '面板内部的点击不能再次触发取词（否则关不掉、拖不动）',
  )
  // 阅读区的滚动容器里包含内联回复卡片：那里交给写信区自己处理，避免弹出两个面板。
  assert.ok(
    /function onReaderMouseUp[\s\S]{0,240}mail-inline-composer/.test(reader),
    '阅读区应放行内联回复里的划词',
  )
  assert.ok(
    /function onTranslateOutside[\s\S]{0,200}handleDocumentMouseDown/.test(composer),
    '写信区点空白处要能收起面板',
  )
})

test('图片：能调宽度、能改位置、能删', () => {
  assert.ok(composer.includes('IMAGE_WIDTHS'), '图片应有宽度预设')
  assert.ok(/function setImageWidth[\s\S]{0,300}img\.style\.width/.test(composer), '宽度写到图片上')
  assert.ok(
    /function alignImage[\s\S]{0,400}marginLeft/.test(composer),
    '位置用左右外边距实现，收件端也认',
  )
  assert.ok(/function removeSelectedImage/.test(composer), '应能直接从正文删图')
  assert.ok(css.includes('.mail-edit-bar'), '浮条要有样式（原来的第二行工具栏已合并掉）')
})

test('表格：工具行给出行列、样式与单元格内容工具', () => {
  assert.ok(composer.includes('TABLE_ROW_ACTIONS'), '应有插入 / 删除行列的入口')
  assert.ok(composer.includes('TABLE_STYLES'), '应能选表格样式预设')
  assert.ok(composer.includes('CELL_FONT_SIZES'), '应能改单元格字号')
  assert.ok(composer.includes('CELL_FILLS'), '应能改单元格填充')
  assert.ok(
    /function tableAction\(action, payload, context = null\)/.test(composer),
    '右键菜单与工具行要共用同一套实现',
  )
  assert.ok(/cellPosition\(table, cell\)/.test(composer), '表格动作要以光标所在单元格为锚点')
})

test('删账户：先问账户，再问本机邮件要不要一起删', () => {
  const remove = accountDialog.slice(accountDialog.indexOf('function remove(account)'))
  assert.ok(/window\.confirm\([\s\S]{0,900}window\.confirm\(/.test(remove), '应该问两次')
  assert.ok(/emit\('deleted', account\.id, deleteLocalMail\)/.test(remove), '要把选择传给上层')
  assert.ok(
    /async deleteAccount\(id, \{ deleteLocalMail = true \} = \{\}\)/.test(store),
    'store 要接收这个选择',
  )
  assert.ok(
    /invoke\('mail_delete_account', \{ accountId: id, deleteLocalMail \}\)/.test(store),
    '选择要传给后端',
  )
  assert.ok(
    /pub async fn mail_delete_account\([\s\S]{0,200}delete_local_mail/.test(rustMail),
    '后端要按选择决定是否清空本机邮件',
  )
  assert.ok(/purge_local_artifacts/.test(rustMail), '「不留痕迹」要连附件缓存与本地草稿一起删')
})

test('草稿：没发出去就一定留一份，且是「替换」而不是越存越多', () => {
  // 独立窗口以前只写本地暂存，草稿箱里根本没有用户写的内容。
  assert.ok(shell.includes(':save-mailbox-draft="saveDraftToMailbox"'), '写信窗口要能写进草稿箱')
  assert.ok(/export async function saveDraftToMailbox/.test(bridge), '桥接层要暴露这个动作')
  assert.ok(composer.includes('saveMailboxDraft'), '编辑器用它做自动保存')
  assert.ok(
    /signature === initialMailboxSignature/.test(composer),
    '一个字都没动时不该在草稿箱里造空壳',
  )
  assert.ok(
    /mail_discard_draft'[\s\S]{0,200}previous\.folder/.test(composer),
    '写新的成功后再删上一份自动草稿',
  )
  assert.ok(/props\.draft\.mailboxDraft = /.test(composer), '要把自动草稿记到草稿对象上')
  assert.ok(/async _discardMailboxDraft/.test(store), '关闭 / 发送时清掉自动草稿')
  // 草稿点开是「继续写」，不是只读预览。
  assert.ok(/function isDraftRow/.test(app), '要能识别草稿行')
  assert.ok(/async function resumeDraft/.test(app), '点开草稿应继续编辑')
  assert.ok(/if \(isDraftRow\(row\)\)[\s\S]{0,120}resumeDraft/.test(app))
})

test('收信正文：<pre> 不再铺成一块黑板', () => {
  // 正文里给 <pre> 上底色的那条规则（不是设置换行的第一条）。
  const rule = css
    .split(/\r?\n(?=\.mail-body|\/\*)/)
    .find((block) => /\.mail-body pre/.test(block) && /background/.test(block))
  assert.ok(rule, '应能找到 pre 的背景规则')
  assert.ok(!/var\(--mail-code-bg\)/.test(rule), 'pre 不能用界面最暗的底色')
  assert.ok(/color-mix/.test(rule), 'pre 应改成纸面上的半透明底色')
  // 近黑背景在暗色下抬成表面，而不是留下一个「洞」。
  assert.ok(/if \(lum < 0\.022\)/.test(colors), '近黑底色应被抬到纸面上方一档')
})
