import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第九轮：点「删除账户」后账户还在。
// 根因是删账户依赖两次 `window.confirm`：webview 一旦不弹它（返回 false），
// remove() 第一行就 return，后端根本没被调用，界面上也没有任何提示。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const dialog = read('src/components/ApplicationViewComponent/apps/mail/MailAccountDialog.vue')
const mailApp = read('src/components/ApplicationViewComponent/apps/MailApp.vue')
const rules = read('src/components/ApplicationViewComponent/apps/mail/MailRulesDialog.vue')
const store = read('src/stores/mailStore.js')
const storeRs = read('src-tauri/src/commands/mail/store.rs')

test('删账户：面板内确认，失败要说出来，成功要以后端为准', () => {
  assert.ok(!/window\.confirm\(/.test(dialog), '不再用系统 confirm')
  assert.ok(/pendingDelete/.test(dialog), '确认区跟着被点的账户渲染')
  assert.ok(/同时删除本机已缓存的邮件/.test(dialog), '本机邮件要不要一起删，仍要问')
  assert.ok(/await props\.deleteFn/.test(dialog), '要等真正的删除结果')
  assert.ok(/删除失败：/.test(dialog), '失败原因显示在面板里')
  assert.ok(
    /invoke\('mail_list_accounts'\)[\s\S]{0,400}后端没有删除这个账户/.test(store),
    '删完要核对后端，不能自己假装成功',
  )
  assert.ok(
    /tx\.execute\("DELETE FROM accounts WHERE id = \?1", params!\[id\]\)\s*\n\s*\.map_err/.test(storeRs),
    '后端不能再吞掉删除语句的错误',
  )
  assert.ok(/账户仍然存在，删除没有生效/.test(storeRs), '提交后要复核账户是否真的没了')
})

test('不可逆操作统一走应用内确认框', () => {
  assert.ok(fs.existsSync(path.join(root, 'src/components/ApplicationViewComponent/apps/mail/mailConfirm.js')), '要有确认框服务')
  assert.ok(fs.existsSync(path.join(root, 'src/components/ApplicationViewComponent/apps/mail/MailConfirmHost.vue')), '要有渲染确认框的宿主组件')
  assert.ok(/<MailConfirmHost \/>/.test(mailApp), '主界面要挂载确认框')
  assert.ok(/askMailConfirm\('删除后不再对之后收到的邮件生效/.test(rules), '删规则改走应用内确认')
  assert.ok(!/window\.confirm\(/.test(rules), '规则弹层里不再有系统 confirm')
  assert.ok(/askMailConfirm\([\s\S]{0,200}删除文件夹/.test(mailApp), '删文件夹改走应用内确认')
  assert.ok(/askMailConfirm\('打开它可能运行代码/.test(mailApp), '打开危险附件也要确认')
})
