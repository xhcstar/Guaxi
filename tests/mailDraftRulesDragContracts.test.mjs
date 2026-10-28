import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 这一批反馈的源码契约：
//   1. 规则新建的文件夹要有内容 —— 保存时可以立刻对已有邮件执行一遍；
//   2. 邮件行要真的能拖到左侧文件夹（从行内文字起拖时也要算）；
//   3. 没发送的邮件在关闭时必须进草稿箱，而且**收件人为空也要存得下来**，
//      成败还要说给用户听（以前只写日志，用户以为功能坏了）；
//   4. 右键「移动」要能看到全部文件夹，并且随账户实时更新；
//   5. 顶栏有打开本机 .eml 的入口。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const mailApp = read('src/components/ApplicationViewComponent/apps/MailApp.vue')
const rules = read('src/components/ApplicationViewComponent/apps/mail/MailRulesDialog.vue')
const picker = read('src/components/ApplicationViewComponent/apps/mail/MailFolderPicker.vue')
const listPane = read('src/components/ApplicationViewComponent/apps/mail/MailListPane.vue')
const sidebar = read('src/components/ApplicationViewComponent/apps/mail/MailSidebar.vue')
const toolbar = read('src/components/ApplicationViewComponent/apps/mail/MailToolbar.vue')
const store = read('src/stores/mailStore.js')
const mime = read('src-tauri/src/commands/mail/mime.rs')
const mod = read('src-tauri/src/commands/mail/mod.rs')
const rulesRs = read('src-tauri/src/commands/mail/rules.rs')
const mainRs = read('src-tauri/src/main.rs')

test('草稿：收件人为空也要能存，并且把成败说给用户', () => {
  // 控制台里真实出现过「自动保存草稿失败 收件人为空」——草稿不该要求收件人。
  assert.ok(/pub fn build_draft/.test(mime) && /require_recipients/.test(mime), '草稿要走放宽校验的组装路径')
  assert.ok(/mime::build_draft\(&request, &from, &account.signature\)/.test(mod), '存草稿用 build_draft')
  assert.ok(/mime::build\(&request, &from, &account\.signature\)\?/.test(mod), '正式发送仍然要求收件人')
  // 服务器不可达 / 没有草稿箱时也要留下东西，不能一句「失败」就丢掉内容。
  assert.ok(/fn save_draft_copy_locally/.test(mod), '草稿要有本机兜底')
  assert.ok(
    /return \{ saved: true, localOnly: Boolean\(saved\?\.localOnly\) \}/.test(store),
    'closeCompose 要返回结果',
  )
  assert.ok(/async function onCloseCompose/.test(mailApp) && /@close="onCloseCompose"/.test(mailApp), '关闭要有可见提示')
  assert.ok(mailApp.includes('已存入草稿箱'), '成功要提示')
  assert.ok(mailApp.includes('草稿没有保存'), '失败也要提示')
  // 独立写信窗口同样要能提示（它没有主界面的 toast）。
  const shell = read('src/components/ApplicationViewComponent/apps/mail/MailComposeWindow.vue')
  assert.ok(/mail-compose-notice/.test(shell) && /function showNotice/.test(shell), '写信窗口要有自己的提示条')
})

test('规则：保存后可立即对已有邮件执行，并支持「所有文件夹」', () => {
  assert.ok(/保存后立即对已有的邮件执行/.test(rules), '规则编辑器要有这个开关')
  assert.ok(/saveRuleFn/.test(rules) && mailApp.includes(':save-rule-fn="onSaveRuleFromDialog"'), '保存要走函数 prop（要拿返回的 id）')
  assert.ok(/mail_apply_rules_to_existing/.test(mailApp), '主窗口调用立即执行')
  assert.ok(/pub async fn mail_apply_rules_to_existing/.test(mod), '后端要有立即执行已有邮件的命令')
  assert.ok(mainRs.includes('mail_apply_rules_to_existing'), '命令要注册')
  // 「所有文件夹」以前映射成空数组，而后端把空当成「仅收件箱」——选了等于没选。
  assert.ok(/folderRoles: cond\.folder === 'inbox' \? \[\] : \['\*'\]/.test(rules), '所有文件夹要显式传 *')
  assert.ok(/role == "\*"/.test(rulesRs) || /=== "\*"/.test(rulesRs), '后端要认 * 这个通配')
})

test('拖拽：不依赖 webview 的原生拖放，用鼠标事件自己实现', () => {
  // 前两轮分别修过 dataTransfer 类型与 body 标记，都没解决：Windows 上 Tauri 的
  // webview 接管了原生拖放，页面里的 dragstart / drop 收不到。这里改成自己跟踪
  // mousedown → mousemove → mouseup，落点用坐标命中测试。
  assert.ok(/import \{ mailDrag, resetMailDrag \} from '\.\/mailDragState'/.test(listPane), '拖拽状态要走共享模块')
  assert.ok(/const DRAG_THRESHOLD = 5/.test(listPane), '要有起拖阈值，避免手抖就挪邮件')
  assert.ok(/function onRowMouseDown/.test(listPane) && /@mousedown="onRowMouseDown/.test(listPane), '按下时要记录起点')
  assert.ok(/function onWindowMouseMove/.test(listPane), '要在窗口级跟踪移动')
  assert.ok(/elementFromPoint/.test(listPane), '落点用坐标命中测试（不依赖任何拖放 API）')
  assert.ok(/emit\('drop-on-folder', folder, rows\)/.test(listPane), '松手时把目标夹与这几封一起交上去')
  assert.ok(!/dragstart=/.test(listPane), '不要再挂 HTML5 dragstart')
  assert.ok(/data-drop-folder/.test(sidebar) && /mailDrag\.hoverFolder/.test(sidebar), '侧栏要按共享状态高亮落点')
  assert.ok(/class="mail-drag-ghost"/.test(listPane), '拖动时要有跟着鼠标的影子')
  assert.ok(/justDragged/.test(listPane), '拖完那一下不能顺手打开邮件')
})

test('顶栏：有打开本机 .eml 的入口', () => {
  assert.ok(/\$emit\('open-eml'\)/.test(toolbar) && toolbar.includes("'open-eml'"), '顶栏要有按钮与事件')
  assert.ok(/async function onOpenEml/.test(mailApp) && /invoke\('mail_read_eml'/.test(mailApp), '主窗口要解析并打开')
  assert.ok(fs.existsSync(path.join(root, 'src/components/ApplicationViewComponent/apps/mail/MailEmlDialog.vue')), '要有查看弹层')
  assert.ok(/pub async fn mail_read_eml/.test(mod), '后端要有读取 .eml 的命令')
  assert.ok(mainRs.includes('mail_read_eml'), '命令要注册')
})

test('规则执行完要顺带同步目标文件夹，否则新建的夹点进去还是空的', () => {
  // IMAP MOVE 之后本地库只有「源夹删了行」，目标夹一条都没有：
  // 用户反馈的「文件夹里没有内容」正是这个，不补一次同步就一直空着。
  assert.ok(/fn rule_target_folders/.test(mod), '要知道规则把邮件送去哪些夹')
  assert.ok(/r\.action\.move_to\.clone\(\), r\.action\.copy_to\.clone\(\)/.test(mod), '移动与复制的目标都算')
  assert.ok(/async fn sync_folder_once/.test(mod), '要有单夹增量同步的兜底')
  assert.ok(
    /if !batch\.is_empty\(\)[\s\S]{0,1600}sync_folder_once\(&account, &dest\)\.await/.test(mod),
    '执行完规则要同步目标夹',
  )
  assert.ok(/!pop3_sync::is_pop3\(&account\)/.test(mod), 'POP3 没有 IMAP，不能走这条同步')
  // 「命中」不等于「搬走了」：离线时 MOVE 整批失败，只报命中数就是提示成功、夹子还是空的。
  assert.ok(/"moved": moved/.test(mod), '要回报真正处理掉的封数')
  assert.ok(/const moved = Number\(result\?\.moved\)/.test(mailApp), '前端要读真正处理数')
  assert.ok(/实际处理 \$\{moved\} 封/.test(mailApp), '部分失败要说清楚，不能只报命中')
})

test('规则与文件夹面板不再弹系统对话框', () => {
  assert.ok(!/window\.alert\(/.test(rules), '保存失败要就地显示')
  assert.ok(!/window\.alert\(/.test(picker), '建夹失败要就地显示')
  assert.ok(/const saveError = ref/.test(rules) && /mail-rule-error/.test(rules), '规则要有行内错误行')
  assert.ok(/const createError = ref/.test(picker), '面板要有行内错误')
  assert.ok(/正在对已有邮件执行/.test(rules), '扫描要有进行时文案，不然像卡住')
})
