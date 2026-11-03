// 任务接入的契约：分类白名单是唯一闸门，任务页签与到期提醒必须共用它。
// 断链表现：关掉「工作」后到期提醒仍生成；或大脑任务页签绕过白名单。
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const read = (p) => fs.readFileSync(path.join(root, p), 'utf8')

const tasksRs = read('src-tauri/src/cognition/tasks.rs')
const nudgeRs = read('src-tauri/src/cognition/nudge.rs')
const modsRs = read('src-tauri/src/cognition.rs')
const commandsRs = read('src-tauri/src/cognition/commands.rs')
const mainRs = read('src-tauri/src/main.rs')
const storeJs = read('src/stores/cognitionStore.js')
const inbox = read('src/components/BrainViewComponent/GuaxiCognitionInbox.vue')
const setting = read('src/components/SettingViewComponent/CognitionSetting.vue')

// ── 1) 白名单语义 ──
assert.match(modsRs, /pub mod tasks;/)
assert.match(tasksRs, /pub fn allowed_categories\(/)
assert.match(tasksRs, /pub fn set_categories\(/)
assert.match(tasksRs, /pub fn overview\(/)
assert.match(tasksRs, /CATEGORIES: \[\(&str, &str\); 3\] = \[\("work", "工作"\), \("life", "生活"\), \("study", "学习"\)\]/)
// 缺省 → 全开；显式空数组 → 全关（两者必须区分）。
assert.match(tasksRs, /Some\(raw\) = raw\.filter/)
assert.match(tasksRs, /空数组表示全部关闭|空数组 → 全关|显式空数组/)
assert.match(tasksRs, /normalize\(list\)/)

// ── 2) 到期提醒读取同一白名单 ──
assert.match(nudgeRs, /crate::cognition::tasks::allowed_categories\(\)/)
assert.match(nudgeRs, /allowed\.iter\(\)\.any\(\|c\| c == &t\.manage_type\)/)

// ── 3) 命令注册 ──
for (const name of [
  'cognition_task_categories',
  'cognition_set_task_categories',
  'cognition_task_overview',
]) {
  assert.ok(commandsRs.includes(`pub fn ${name}(`), `${name} 未定义`)
  assert.ok(mainRs.includes(`cognition::commands::${name}`), `${name} 未注册`)
}

// ── 4) 前端：任务页签、分类开关、设置页 ──
assert.match(storeJs, /async loadTaskCategories\(/)
assert.match(storeJs, /async loadTasks\(/)
assert.match(storeJs, /async setTaskCategories\(/)
assert.match(inbox, /<n-tab-pane name="tasks">/)
assert.match(inbox, /toggleTaskCategory/)
assert.match(inbox, /taskGroups/)
// 「打开任务管理」必须走独立窗口命令，而不是切主窗口路由（路由方式会让任务页
// 与大脑页抢同一个工作区，且页面是按独立窗口设计的）。
assert.match(inbox, /create_task_manager_window/)
assert.match(setting, /任务接入/)
assert.match(setting, /setTaskCategory/)

console.log('taskIntegration: ok')
