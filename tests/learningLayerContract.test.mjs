// M4：学习状态层的契约测试。
//
// 断链的表现同样安静：表没建 → 看板永远空；FSRS 没接上 → 复习间隔还是 SM-2 的乘法；
// 看板没接命令 → 页签空转；反馈后不更新掌握度 → 看完的复习要等第二天才动。
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const read = (p) => fs.readFileSync(path.join(root, p), 'utf8')

const storeRs = read('src-tauri/src/cognition/store.rs')
const modsRs = read('src-tauri/src/cognition.rs')
const fsrsRs = read('src-tauri/src/cognition/fsrs.rs')
const learningRs = read('src-tauri/src/cognition/learning.rs')
const nudgeRs = read('src-tauri/src/cognition/nudge.rs')
const commandsRs = read('src-tauri/src/cognition/commands.rs')
const mainRs = read('src-tauri/src/main.rs')
const runtimeRs = read('src-tauri/src/cognition/runtime.rs')
const inbox = read('src/components/BrainViewComponent/GuaxiCognitionInbox.vue')
const storeJs = read('src/stores/cognitionStore.js')

// ── 1) 表与迁移 ──
const schemaVersion = Number(storeRs.match(/COG_SCHEMA_VERSION: i64 = (\d+)/)?.[1] || 0)
assert.ok(schemaVersion >= 6, `schema 版本 ${schemaVersion} 低于学习状态引入时的 6`)
assert.match(storeRs, /CREATE TABLE IF NOT EXISTS topic_mastery \(/)
assert.match(storeRs, /idx_mastery_rank/)
for (const column of ['stability', 'difficulty', 'fsrs_state', 'elapsed_days', 'scheduled_days']) {
  assert.match(storeRs, new RegExp(`add_column\\(conn, "review_state", "${column}"`), `review_state 缺列 ${column}`)
}

// ── 2) FSRS：默认权重、检索率、排期 ──
assert.match(modsRs, /pub mod fsrs;/)
assert.match(fsrsRs, /pub const DEFAULT_W: \[f64; 19\]/)
assert.match(fsrsRs, /pub fn retrievability\(/)
assert.match(fsrsRs, /pub fn schedule\(/)
assert.match(fsrsRs, /next_stability_lapse/)
assert.match(fsrsRs, /next_stability_good/)

// ── 3) 学习模块：刷新、单节点更新、概览 ──
assert.match(modsRs, /pub mod learning;/)
assert.match(learningRs, /pub fn refresh_mastery\(/)
assert.match(learningRs, /pub fn touch_node\(/)
assert.match(learningRs, /pub fn overview\(/)
assert.match(learningRs, /0\.5 \* agg\.max_progress/)
assert.match(learningRs, /0\.6 \* retention \+ 0\.4 \* reliability/)

// ── 4) 复习反馈：走 FSRS，并就地更新掌握度 ──
assert.match(nudgeRs, /crate::cognition::fsrs::schedule\(/)
assert.match(nudgeRs, /crate::cognition::learning::touch_node\(node_id\)/)
// SM-2 的「间隔 × ease」乘法必须已经不在反馈路径里。
assert.ok(
  !/interval as f64 \* ease/.test(nudgeRs),
  'review_feedback 仍在用 SM-2 的 interval × ease',
)

// ── 5) 命令与后台 ──
for (const name of ['cognition_learning_overview', 'cognition_learning_refresh']) {
  assert.match(commandsRs, new RegExp(`pub fn ${name}\\(`), `${name} 未定义`)
  assert.match(mainRs, new RegExp(`cognition::commands::${name}`), `${name} 未注册`)
}
assert.match(runtimeRs, /learning::refresh_mastery\(\)/)

// ── 6) 前端：学习页签与 store ──
assert.match(inbox, /<n-tab-pane name="learning">/)
assert.match(inbox, /class="learn-stats"/)
assert.match(inbox, /refreshLearning/)
assert.match(storeJs, /async loadLearning\(/)
assert.match(storeJs, /async refreshLearning\(/)
// bootstrap 要带上学习概览，否则首次进大脑页永远空白。
assert.match(storeJs, /this\.loadLearning\(\)/)

console.log('learningLayer: ok')
