import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第二十二轮：显示规则改成「服务器上有就列出来」，只排除服务商的模板壳；
// 并且这些操作都不能把前端卡在网络上。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const mailDir = 'src/components/ApplicationViewComponent/apps/mail/'
const sidebar = read(mailDir + 'MailSidebar.vue')
const mailApp = read('src/components/ApplicationViewComponent/apps/MailApp.vue')
const rules = read(mailDir + 'MailRulesDialog.vue')
const store = read('src/stores/mailStore.js')
const imapRs = read('src-tauri/src/commands/mail/imap.rs')

test('模板壳识别：结构判断，前后端同一套规则', () => {
  const tree = read(mailDir + 'folderTree.js')
  assert.ok(/export function providerShellPrefixes/.test(tree), '按结构找模板壳')
  assert.ok(/count >= 2/.test(tree), '子树里至少两个标准目录才算壳，避免误伤')
  assert.ok(/export function visibleCustomFolders/.test(tree), '侧栏用同一份规则')
  assert.ok(/export function selectableMoveTargets/.test(tree), '移动目标用同一份规则')
  // 后端：一样的判定，用来跳过 STATUS
  assert.ok(/fn provider_shells/.test(imapRs), '后端也要认这套壳')
  assert.ok(/\*count >= 2/.test(imapRs), '后端阈值一致')
  assert.ok(/if selectable && !in_shell/.test(imapRs), '模板壳不做 STATUS（不同步）')
})

test('三处列表都走共享规则', () => {
  assert.ok(/visibleCustomFolders\(folders\.value\)/.test(sidebar), '侧栏')
  assert.ok(/selectableMoveTargets\(store\.foldersByAccount/.test(mailApp), '右键「移动」子菜单')
  assert.ok(/selectableMoveTargets\(props\.foldersByAccount/.test(rules), '规则里的文件夹选择')
})

test('文件夹刷新不阻塞界面：先亮本地，再等服务器', () => {
  const fn = store.slice(store.indexOf('async loadFolders('), store.indexOf('async loadFolders(') + 1200)
  assert.ok(/if \(refresh && !\(this\.foldersByAccount\[accountId\] \|\| \[\]\)\.length\)/.test(fn), '没有列表时才补读本地')
  assert.ok(/mail_list_folders/.test(fn), '先用本地那份顶上')
  assert.ok(/mail_refresh_folders/.test(fn), '服务器那份回来再覆盖')
  // 切文件夹本身不 await 网络
  const selectFolder = store.slice(store.indexOf('selectFolder(name)'), store.indexOf('selectFolder(name)') + 500)
  assert.ok(/this\.syncFolders\(\[name\]\)/.test(selectFolder), '同步在后台跑')
  assert.ok(!/await this\.syncFolders/.test(selectFolder), '切文件夹不等同步')
})
