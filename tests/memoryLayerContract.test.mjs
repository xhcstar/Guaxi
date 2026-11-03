// M3：长期记忆层的契约测试。
//
// 断链的表现都很安静：记忆表没建 → 巩固每次报错只写日志；命令没注册 → 设置页一直空；
// 召回第五路没接 → 记忆存进去了但永远召不回；权重表没加 → 下一轮调参把 memory 通道抹掉。
// 与 docReaderWindow / activityBus 同一套做法：读源码做静态断言。
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const read = (p) => fs.readFileSync(path.join(root, p), 'utf8')

const storeRs = read('src-tauri/src/cognition/store.rs')
const modsRs = read('src-tauri/src/cognition.rs')
const memoryRs = read('src-tauri/src/cognition/memory.rs')
const commandsRs = read('src-tauri/src/cognition/commands.rs')
const mainRs = read('src-tauri/src/main.rs')
const recallRs = read('src-tauri/src/cognition/recall.rs')
const evolveRs = read('src-tauri/src/cognition/evolve.rs')
const runtimeRs = read('src-tauri/src/cognition/runtime.rs')
const settingVue = read('src/components/SettingViewComponent/CognitionSetting.vue')
const storeJs = read('src/stores/cognitionStore.js')

// ── 1) 表与迁移 ──
// 只断言「不低于引入 memories 的版本」，后续里程碑会继续往上加。
const schemaVersion = Number(storeRs.match(/COG_SCHEMA_VERSION: i64 = (\d+)/)?.[1] || 0)
assert.ok(schemaVersion >= 5, `schema 版本 ${schemaVersion} 低于 memories 引入时的 5`)
assert.match(storeRs, /CREATE TABLE IF NOT EXISTS memories \(/)
assert.match(storeRs, /idx_memories_hash/)
assert.match(storeRs, /idx_memories_status/)

// ── 2) 模块与能力 ──
assert.match(modsRs, /pub mod memory;/)
assert.match(memoryRs, /pub fn add_manual\(/)
assert.match(memoryRs, /pub fn forget\(/)
assert.match(memoryRs, /pub fn list\(/)
assert.match(memoryRs, /pub async fn embed_pending\(/)
assert.match(memoryRs, /pub fn topk\(/)
assert.match(memoryRs, /pub async fn consolidate_now\(/)
assert.match(memoryRs, /pub fn sync_manual_from_markdown\(/)
assert.match(memoryRs, /pub fn sync_to_markdown\(/)
// 忘记要留墓碑：退休的行保留 text_hash，同一句话不会被自动复活。
assert.match(memoryRs, /retired 是墓碑|墓碑/)

// ── 3) 命令注册 ──
for (const name of [
  'cognition_memory_list',
  'cognition_memory_add',
  'cognition_memory_forget',
  'cognition_memory_consolidate',
  'cognition_memory_file_path',
]) {
  assert.match(commandsRs, new RegExp(`pub (async )?fn ${name}\\(`), `${name} 未定义`)
  assert.match(mainRs, new RegExp(`cognition::commands::${name}`), `${name} 未注册`)
}

// ── 4) 召回第五路 + 权重/评测闩 ──
assert.match(recallRs, /const W_MEMORY: f64 =/)
assert.match(recallRs, /\("memory"\.to_string\(\), W_MEMORY\)/)
assert.match(recallRs, /第 5 路：长期记忆/)
assert.match(recallRs, /cognition::memory::topk\(&qv, 3\)/)
assert.match(evolveRs, /"semantic", "graph", "percept", "episodic", "memory"/)
// 评测的候选生成也要包含记忆通道，否则调参永远看不到它。
const evalSection = recallRs.slice(recallRs.indexOf('pub async fn eval_channels'))
assert.match(evalSection, /out\.push\(\("memory"\.into\(\), mems\)\)/)

// ── 5) 后台：补向量 + 每日巩固 ──
assert.match(runtimeRs, /memory::embed_pending\(16\)\.await/)
assert.match(runtimeRs, /memory::consolidate_now\(\)\.await/)

// ── 6) memory.md 手动/自动双块 ──
assert.match(memoryRs, /<!-- guaxi:manual:start -->/)
assert.match(memoryRs, /<!-- guaxi:auto:start -->/)

// ── 7) 前端：设置页能列、能记、能忘、能立即巩固 ──
assert.match(storeJs, /async loadMemories\(/)
assert.match(storeJs, /async addMemory\(/)
assert.match(storeJs, /async forgetMemory\(/)
assert.match(storeJs, /async consolidateMemory\(/)
assert.match(settingVue, /name="memory"/)
assert.match(settingVue, /memoryText/)
assert.match(settingVue, /consolidateMemory/)

console.log('memoryLayer: ok')
