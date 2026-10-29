import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第十三轮：顶栏那个「重新发现文件夹」按钮其实只是手动触发定时同步已经在做的事，
// 用户要求「文件夹一有变化就自动同步到所有用到文件夹的地方」，于是去掉按钮、
// 把自动路径补齐。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const toolbar = read('src/components/ApplicationViewComponent/apps/mail/MailToolbar.vue')
const store = read('src/stores/mailStore.js')
const mailApp = read('src/components/ApplicationViewComponent/apps/MailApp.vue')

test('顶栏：去掉单独的「重新发现文件夹」，同步收件顺带刷新结构', () => {
  assert.ok(!/aria-label="重新发现文件夹"/.test(toolbar), '不再有单独的手动刷新按钮')
  assert.ok(!/'refresh'/.test(toolbar), '事件也一并去掉')
  assert.ok(/同步收件（同时重新发现文件夹）/.test(toolbar), '同步按钮标题要说明它包含这一步')
  assert.ok(/store\.syncNow\(\{ refreshFolders: true \}\)/.test(mailApp), '手动同步要刷新文件夹结构')
  assert.ok(/async syncNow\(\{ refreshFolders = false \} = \{\}\)/.test(store), 'syncNow 支持按需刷新结构')
})

test('文件夹变化会自动传播：本地增删改后刷新、IDLE 触发节流刷新', () => {
  // 本地增删改：建 / 删文件夹后都重读一次，避免后台刷新把旧结果写回来。
  assert.ok(/async createFolder\(name\)[\s\S]{0,1500}await this\.loadFolders\(accountId\)/.test(store), '建完要刷新')
  assert.ok(/async deleteFolder\(name\)[\s\S]{0,1500}await this\.loadFolders\(accountId\)/.test(store), '删完要刷新')
  // 服务器端（手机 / 网页）的变化：IDLE 事件 + 分钟级节流。
  assert.ok(/function refreshFolderStructureThrottled/.test(store), '要有节流的结构刷新')
  assert.ok(
    /mail-idle-change[\s\S]{0,700}refreshFolderStructureThrottled/.test(store),
    'IDLE 收到变化后要确认文件夹结构',
  )
  // 定时同步也会整体刷新（startPolling → syncAll → refreshFolders）。
  assert.ok(/startPolling[\s\S]{0,400}syncAll/.test(store), '定时同步仍会整体刷新')
  assert.ok(/async syncAll[\s\S]{0,400}await this\.refreshFolders\(\)/.test(store), '全量同步要刷新所有账户的文件夹')
})

test('所有用到文件夹的地方都读同一份数据', () => {
  // 侧栏、右键「移动」子菜单、规则弹层都从 foldersByAccount 取，刷新一处即处处同步。
  assert.ok(/const folders = computed\(\(\) => \{[\s\S]{0,120}props\.foldersByAccount\[props\.activeAccountId\]/.test(
    read('src/components/ApplicationViewComponent/apps/mail/MailSidebar.vue'),
  ), '侧栏读 foldersByAccount')
  assert.ok(/const contextFolders = computed[\s\S]{0,300}store\.foldersByAccount\[accountId\]/.test(mailApp), '移动子菜单读同一份')
  assert.ok(/:folders-by-account="store\.foldersByAccount"/.test(mailApp), '规则弹层读同一份')
})
