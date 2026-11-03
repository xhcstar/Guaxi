// 活动总线（M1）的契约测试。
//
// 盯的是「记录链路是否真的接通」：表、命令、注册、五个采集点缺任何一个，
// 表现都是设置页里一直显示"还没有记录"，而不是报错。与 docReaderWindow /
// articleReaderWindow 同一套做法：读源码做静态断言，不跑 Tauri。
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const read = (p) => fs.readFileSync(path.join(root, p), 'utf8')

const storeRs = read('src-tauri/src/cognition/store.rs')
const activityRs = read('src-tauri/src/cognition/activity.rs')
const commandsRs = read('src-tauri/src/cognition/commands.rs')
const mainRs = read('src-tauri/src/main.rs')
const runtimeRs = read('src-tauri/src/cognition/runtime.rs')
const readerRs = read('src-tauri/src/commands/guaxireader.rs')
const beacon = read('src/composables/useActivityBeacon.js')
const viteConfig = read('vite.config.js')

// ── 1) 持久层：schema 版本与 activity 表 ──
// 版本号只断言「不低于引入 activity 的版本」，后续里程碑会继续往上加。
const schemaVersion = Number(storeRs.match(/COG_SCHEMA_VERSION: i64 = (\d+)/)?.[1] || 0)
assert.ok(schemaVersion >= 4, `schema 版本 ${schemaVersion} 低于 activity 引入时的 4`)
assert.match(storeRs, /CREATE TABLE IF NOT EXISTS activity \(/)
assert.match(storeRs, /idx_activity_at/)
assert.match(storeRs, /idx_activity_app/)

// ── 2) 后端模块与命令 ──
assert.match(activityRs, /pub fn log_events\(/)
assert.match(activityRs, /pub fn recent\(/)
assert.match(activityRs, /pub fn stats\(/)
assert.match(activityRs, /pub fn prune\(/)
assert.match(commandsRs, /pub fn cognition_activity_log\(/)
assert.match(commandsRs, /pub fn cognition_activity_recent\(/)
assert.match(commandsRs, /pub fn cognition_activity_stats\(/)
assert.match(mainRs, /cognition::commands::cognition_activity_log/)
assert.match(mainRs, /cognition::commands::cognition_activity_recent/)
assert.match(mainRs, /cognition::commands::cognition_activity_stats/)

// 日常维护里要按保留期裁剪，否则这张 append-only 表会一直长。
assert.match(runtimeRs, /activity::prune\(/)

// ── 3) 摘录：后端统一入口记一条（五个阅读器都走 update_notes_file） ──
assert.match(readerRs, /kind: "note"\.to_string\(\)/)

// ── 4) 前端采集器：批量提交 + 后台 flush + 节流 API ──
assert.match(beacon, /invoke\('cognition_activity_log'/)
assert.match(beacon, /visibilitychange/)
assert.match(beacon, /pagehide/)
assert.match(beacon, /export function trackActivity\(/)
assert.match(beacon, /export function trackActivityOnce\(/)
assert.match(beacon, /export function trackReaderProgress\(/)
// 阅读器与主应用共用同一个采集器，必须钉进 reader-core，否则阅读窗口会把主应用 chunk 拉进来。
assert.match(viteConfig, /"\/src\/composables\/useActivityBeacon\.js"/)

// ── 5) 五个采集点 ──
const hooks = [
  ['Repo 打开', 'src/components/RepoViewComponent/composables/useContextMenu.js', /app:\s*"repo",\s*\n\s*kind:\s*"open"/],
  ['搜索提交', 'src/components/HomeViewComponent/GuaxiHeader.vue', /app:\s*'search',\s*\n\s*kind:\s*'search'/],
  ['编辑器保存', 'src/stores/fileTreeStore.js', /app:\s*"editor",\s*\n\s*kind:\s*"edit"/],
]
for (const [label, file, pattern] of hooks) {
  assert.match(read(file), pattern, `${label} 没有接入活动总线`)
}

const readers = [
  'src/components/ReaderComponent/GuaxiMarkdownReader.vue',
  'src/components/ReaderComponent/GuaxiTextReader.vue',
  'src/components/ReaderComponent/GuaxiConvertPDFReader.vue',
  'src/components/ReaderComponent/GauxiWordReader.vue',
  'src/components/ReaderComponent/GuaxiEpubReader.vue',
  'src/components/ReaderComponent/readers/DocEbookReader.vue',
  'src/components/HomeViewComponent/ReaderPane.vue',
]
for (const file of readers) {
  const text = read(file)
  assert.match(text, /trackReaderProgress\(/, `${file} 没有上报阅读进度`)
}

// ── 6) 调试视图：设置页能读到最近活动与概览 ──
assert.match(read('src/stores/cognitionStore.js'), /async loadActivity\(/)
assert.match(read('src/stores/cognitionStore.js'), /async refreshActivityStats\(/)
const setting = read('src/components/SettingViewComponent/CognitionSetting.vue')
assert.match(setting, /name="activities"/)
assert.match(setting, /loadActivity/)
assert.match(setting, /activityStats/)

console.log('activityBus: ok')
