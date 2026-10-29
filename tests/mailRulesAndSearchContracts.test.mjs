import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 这一批反馈的源码契约：
//   1. 收信规则弹层里「发件人名单」和「收信规则」分成两个 tab；
//   2. 从通讯录加人后，输入框要能向下长，让加进去的地址都看得见；
//   3. 「移到文件夹」按当前账户隔离、能就地新建、能刷新；
//   4. 设置页的页签不再压在说明文字上；
//   5. 按发件人 / 收件人搜索不再要求写完整邮箱，命中词要在列表与正文里高亮；
//   6. 邮件行可以直接拖到左侧文件夹上完成移动 / 删除；
//   7. 编辑器自己的表格 / 图片样式不再套到引用原文上（内容不再相对 Outlook 偏移）。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const rules = read('src/components/ApplicationViewComponent/apps/mail/MailRulesDialog.vue')
const picker = read('src/components/ApplicationViewComponent/apps/mail/MailFolderPicker.vue')
const settings = read('src/components/ApplicationViewComponent/apps/mail/MailSettingsDrawer.vue')
const sidebar = read('src/components/ApplicationViewComponent/apps/mail/MailSidebar.vue')
const listPane = read('src/components/ApplicationViewComponent/apps/mail/MailListPane.vue')
const rowContent = read('src/components/ApplicationViewComponent/apps/mail/MailRowContent.vue')
const reader = read('src/components/ApplicationViewComponent/apps/mail/MailReader.vue')
const highlight = read('src/components/ApplicationViewComponent/apps/mail/highlight.js')
const mailApp = read('src/components/ApplicationViewComponent/apps/MailApp.vue')
const css = read('src/components/ApplicationViewComponent/apps/mail/mail.css')
const store = read('src/stores/mailStore.js')
const rust = read('src-tauri/src/commands/mail/store.rs')

test('收信规则弹层：两个模块分成两个 tab，不再挤在同一列', () => {
  assert.ok(/const TABS = \[/.test(rules), '应有页签定义')
  assert.ok(rules.includes("key: 'rules'") && rules.includes("key: 'senders'"), '规则与名单各一页')
  assert.ok(/v-show="activeTab === 'rules'"/.test(rules), '规则面板按页签显隐')
  assert.ok(/v-show="activeTab === 'senders'"/.test(rules), '名单面板按页签显隐')
  // 以前名单夹在规则列表中间，「添加」按钮看起来像在给规则加条件。
  assert.ok(!/mail-section-divider/.test(rules), '不应再用分隔线把两块塞在一列里')
})

test('规则条件：从通讯录加人后输入框向下长，而不是把地址截断', () => {
  assert.ok(/textarea[\s\S]{0,200}mail-rule-grow/.test(rules), '发件人 / 收件人应改用可长高的 textarea')
  assert.ok(/function autoGrow/.test(rules) && /function resizeGrowables/.test(rules), '应实现自动长高')
  assert.ok(
    /\.mail-rule-grow\s*\{[\s\S]{0,200}max-height:\s*96px/.test(rules),
    '长高要有上限，不能把整个弹层撑开',
  )
  assert.ok(
    /appendAddresses[\s\S]{0,400}nextTick\(resizeGrowables\)/.test(rules),
    '追加地址后要立刻重算高度，用户才看得到加了谁',
  )
})

test('规则「移到文件夹」：按当前账户取文件夹，悬浮面板里就地新建与刷新', () => {
  assert.ok(mailApp.includes(':active-account-id="store.activeAccountId"'), '要把当前账户传给弹层')
  assert.ok(/const ruleAccountId = computed/.test(rules), '文件夹列表要绑定到一个账户')
  assert.ok(
    /selectableMoveTargets\(props\.foldersByAccount/.test(rules),
    '发件箱与模板壳不是归档目的地',
  )
  // 选择与新建都在悬浮面板里完成：不再用 window.prompt，也不用原生 select。
  assert.ok(/<MailFolderPicker/.test(rules), '改用悬浮文件夹面板')
  assert.ok(!/window\.prompt/.test(rules), '不应再用系统输入框')
  assert.ok(/const tree = computed/.test(picker) && /function sortNodes/.test(picker), '面板把带分隔符的名字组成树')
  assert.ok(picker.includes('startCreate') && picker.includes('commitCreate'), '面板内联新建文件夹')
  assert.ok(picker.includes("emit('refresh')"), '打开面板时要能刷新文件夹表')
  assert.ok(!/window\.prompt\(/.test(picker), '面板内也不该出现系统对话框')
  // 新建后要能立刻选中新文件夹：store 要把建好的那条返回出来。
  assert.ok(/return \(this\.foldersByAccount\[accountId\] \|\| \[\]\)\.find/.test(store), 'createFolder 应返回新文件夹')
})

test('设置页：页签与说明在滚动容器之外，不再压住文字', () => {
  const head = settings.indexOf('mail-settings-head')
  const body = settings.indexOf('class="mail-modal-body"')
  assert.ok(head >= 0 && body > head, '页签应在 .mail-modal-body 之前')
  assert.ok(/\.mail-settings-head\s*\{/.test(css), '应有独立的头部样式')
  assert.ok(!/\.mail-settings-tabs\s*\{[^}]*position:\s*sticky/.test(css), '页签不再 sticky，压不到下面的说明')
})

test('搜索：按发件人 / 收件人不再要求写完整邮箱', () => {
  assert.ok(/fn like_terms/.test(rust) && /fn push_like_with_alternatives/.test(rust), '字段检索应改用子串匹配')
  assert.ok(/"from" => Some\(&\["m\.from_addr", "m\.from_name"\]\[\.\.\]\)/.test(rust), '发件人要同时匹配地址与显示名')
  assert.ok(/"to" => Some\(&\["m\.to_json", "m\.cc_json"\]\[\.\.\]\)/.test(rust), '收件人要覆盖 to 与 cc')
  assert.ok(/field_like\.is_none\(\)/.test(rust), '这几列不再走 FTS 的 token 匹配')
  // 通讯录里存的名字：把对应邮箱一起并进来搜，否则信头没写姓名的邮件会漏掉。
  assert.ok(/pub text_alternatives: Vec<String>/.test(read('src-tauri/src/commands/mail/model.rs')), '查询结构要能带备选地址')
  assert.ok(/async searchAlternativesFor\(text\)/.test(store) && /textAlternatives: await this\.searchAlternativesFor/.test(store), '前端要展开通讯录候选')
})

test('搜索：命中词在列表行与正文里高亮', () => {
  assert.ok(/export function segments/.test(highlight) && /export function highlightHtml/.test(highlight))
  assert.ok(listPane.includes('highlightTokens') && rowContent.includes('MailHighlightSegments'), '列表行要渲染高亮片段')
  assert.ok(reader.includes('applySearchHighlight'), '正文要就地高亮')
  assert.ok(mailApp.includes(':search-query="store.searchQuery"'), '阅读区要拿到当前搜索词')
  assert.ok(/\.mail-hl\s*\{/.test(css), '高亮要有样式')
  // 重复执行必须幂等：正文每次 patch 都会被重跑一遍。
  assert.ok(/export function clearHighlight/.test(highlight), '重跑前先摘掉旧标记')
})

test('拖拽：邮件行可以直接放到左侧文件夹上（含删除 / 垃圾）', () => {
  assert.ok(/data-drop-folder/.test(sidebar), '标准文件夹与自建文件夹都要带可命中标记')
  assert.ok(/mailDrag\.hoverFolder === (entry\.folder|folder)\.name/.test(sidebar), '悬停中的落点要有高亮')
  assert.ok(/\.mail-folder\.is-drop-target/.test(css), '高亮样式要存在')
  assert.ok(/folder\?\.role === 'trash'/.test(mailApp) && /runBatch\('delete'/.test(mailApp), '放到「已删除」按删除处理')
  assert.ok(/folder\?\.role === 'junk'/.test(mailApp), '放到「垃圾邮件」按垃圾处理')
})

test('编辑器：表格 / 图片样式只作用于自己插入的内容', () => {
  assert.ok(/data-guaxi-table/.test(read('src/components/ApplicationViewComponent/apps/mail/composeTable.js')), '插入的表格要带标记')
  assert.ok(
    /\.mail-compose-editor table\[data-guaxi-table\] \{/.test(css),
    '表格样式只命中带标记的表格',
  )
  assert.ok(/data-guaxi-img/.test(css), '图片的块级排版只作用于插入的图片')
  // 右键菜单挂在视口上（fixed）：编辑区是 overflow 滚动容器，绝对定位的菜单
  // 要么被裁掉半截，要么只能自己开滚动条——两条都是用户明确不要的。
  const composer = read('src/components/ApplicationViewComponent/apps/mail/MailComposer.vue')
  assert.ok(
    /function clampEditorMenu[\s\S]{0,900}window\.innerWidth/.test(composer),
    '菜单要按视口夹位置，不能溢出屏幕',
  )
  assert.ok(
    /\.mail-editor-menu \{[\s\S]{0,220}position: fixed/.test(css),
    '菜单应 fixed 定位，不受编辑区滚动容器裁切',
  )
  const menuBlock = css.slice(css.indexOf('.mail-editor-menu {'), css.indexOf('.mail-editor-menu-head'))
  assert.ok(
    !/max-height|overflow-y/.test(menuBlock),
    '菜单不该靠滚动条装内容：表格分支用两栏排版放全',
  )
  assert.ok(/\.mail-editor-menu\.is-columns/.test(css), '表格菜单应有两栏排版的样式')
  assert.ok(
    !/mail-compose-subbar/.test(composer) && !/mail-compose-subbar/.test(css),
    '图片/表格不该再占一整条第二行工具栏',
  )
  assert.ok(/const editBar = ref\(null\)/.test(composer), '图片/表格改为贴着目标浮出小工具条')
})

const { searchTokens, segments } = await import(
  '../src/components/ApplicationViewComponent/apps/mail/highlight.js'
)

test('高亮切词：多词 AND、重叠不套娃、大小写不敏感', () => {
  assert.deepEqual(searchTokens(' 发票 invoice 发票 '), ['invoice', '发票'])
  const parts = segments('Invoice 发票与收据', searchTokens('invo 发票'))
  assert.deepEqual(
    parts.map((part) => [part.text, part.hit]),
    [
      ['Invo', true],
      ['ice ', false],
      ['发票', true],
      ['与收据', false],
    ],
  )
  // 空词表原样返回，不产生多余的片段。
  assert.deepEqual(segments('普通文本', []), [{ text: '普通文本', hit: false }])
})
