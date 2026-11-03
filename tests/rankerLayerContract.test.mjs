// M5：引证排序器的契约测试。
//
// 断链表现：特征没落库 → 永远没有训练样本；门禁被绕过 → 静默退化；
// 召回没接排序器 → 训了也不生效；命令没注册 → 设置页按钮报错。
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const read = (p) => fs.readFileSync(path.join(root, p), 'utf8')

const storeRs = read('src-tauri/src/cognition/store.rs')
const modsRs = read('src-tauri/src/cognition.rs')
const rankerRs = read('src-tauri/src/cognition/ranker.rs')
const recallRs = read('src-tauri/src/cognition/recall.rs')
const episodeRs = read('src-tauri/src/cognition/episode.rs')
const commandsRs = read('src-tauri/src/cognition/commands.rs')
const mainRs = read('src-tauri/src/main.rs')
const runtimeRs = read('src-tauri/src/cognition/runtime.rs')
const settingVue = read('src/components/SettingViewComponent/CognitionSetting.vue')
const storeJs = read('src/stores/cognitionStore.js')

// ── 1) 表与迁移 ──
const schemaVersion = Number(storeRs.match(/COG_SCHEMA_VERSION: i64 = (\d+)/)?.[1] || 0)
assert.ok(schemaVersion >= 7, `schema 版本 ${schemaVersion} 低于排序特征引入时的 7`)
assert.match(storeRs, /add_column\(conn, "citations", "features", "TEXT"\)/)

// ── 2) 排序器：特征、训练、门禁、回滚 ──
assert.match(modsRs, /pub mod ranker;/)
assert.match(rankerRs, /pub const FEATURE_DIM: usize = 18/)
assert.match(rankerRs, /pub fn features\(/)
assert.match(rankerRs, /pub fn adjust_fused\(/)
assert.match(rankerRs, /pub fn train\(/)
assert.match(rankerRs, /pub fn current_model\(/)
assert.match(rankerRs, /pub fn reset\(/)
assert.match(rankerRs, /const MIN_AUC: f64 = 0\.55;/)
assert.match(rankerRs, /const MIN_AUC_GAIN: f64 = 0\.02;/)
assert.match(rankerRs, /fn auc\(/)
// 排序键是乘法微调，不是让模型完全接管。
assert.match(rankerRs, /fused \* \(0\.4 \+ 1\.2 \* model\.predict/)

// ── 3) 特征落库 + 召回应用 ──
assert.match(episodeRs, /pub features: Option<String>/)
assert.match(episodeRs, /citations\(episode_id,ref_kind,ref_id,kb_name,label,channel,rank,score,at,features\)/)
assert.match(recallRs, /ranker::current_model\(\)/)
assert.match(recallRs, /ranker::FeatureInput \{/)
assert.match(recallRs, /c\.adjusted = crate::cognition::ranker::adjust_fused/)
assert.match(recallRs, /features: c\.features\.clone\(\)/)
// 五个候选来源都要带特征原料。
assert.match(recallRs, /pagerank: node\.pagerank/)
assert.match(recallRs, /percept::meta_of\(&id\)/)
assert.match(recallRs, /mem\.confidence/)

// ── 4) 命令与后台周训 ──
for (const name of ['cognition_ranker_status', 'cognition_ranker_train', 'cognition_ranker_reset']) {
  assert.match(commandsRs, new RegExp(`pub (async )?fn ${name}\\(`), `${name} 未定义`)
  assert.match(mainRs, new RegExp(`cognition::commands::${name}`), `${name} 未注册`)
}
assert.match(runtimeRs, /ranker::maybe_auto_train\(\)/)

// ── 5) 前端入口 ──
assert.match(storeJs, /async loadRankerStatus\(/)
assert.match(storeJs, /async trainRanker\(/)
assert.match(storeJs, /async resetRanker\(/)
assert.match(settingVue, /引证排序器/)
assert.match(settingVue, /trainRanker/)
assert.match(settingVue, /resetRanker/)
assert.match(settingVue, /rankerStatus\.metric/)

console.log('rankerLayer: ok')
