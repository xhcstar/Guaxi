// 语音播报的契约：手动播报要覆盖"今天需要知道的"，自动播报要受设置开关控制，
// 且必须复用现有 TTS 通道（不新开一套合成栈）。
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const read = (p) => fs.readFileSync(path.join(root, p), 'utf8')

const commandsRs = read('src-tauri/src/cognition/commands.rs')
const storeJs = read('src/stores/cognitionStore.js')
const inbox = read('src/components/BrainViewComponent/GuaxiCognitionInbox.vue')
const setting = read('src/components/SettingViewComponent/CognitionSetting.vue')

// ── 1) 状态透出开关 ──
assert.match(commandsRs, /pub voice_broadcast: bool/)
assert.match(commandsRs, /voice_broadcast: store::get_bool\("voice_broadcast", false\)/)
assert.match(storeJs, /voiceBroadcast: \(s\) => Boolean\(s\.status\?\.voiceBroadcast\)/)

// ── 2) 复用现有一句话合成通道 ──
assert.match(inbox, /import \{ useSelectionReadAloud \} from '\.\.\/\.\.\/composables\/useSelectionReadAloud'/)
assert.match(inbox, /const speech = useSelectionReadAloud\(\{ message \}\)/)

// ── 3) 手动播报：提醒 → 任务 → 简报 → 学习 ──
assert.match(inbox, /const buildBroadcastText = \(\) =>/)
assert.match(inbox, /const broadcastToday = async \(\) =>/)
assert.match(inbox, /待处理提醒/)
assert.match(inbox, /任务方面/)
assert.match(inbox, /学习方面/)
assert.match(inbox, /今日简报：/)
assert.match(inbox, /最该补的是/)
assert.match(inbox, /Speaker220Regular/)
// 播放中再点一次 = 停止。
assert.match(inbox, /if \(speech\.busy\.value\) \{\s*\n\s*speech\.stop\(\)/)

// ── 4) 自动播报受设置控制 ──
assert.match(inbox, /if \(store\.voiceBroadcast\)/)
assert.match(inbox, /speech\.speak\(text\)/)

// ── 5) 设置页开关 ──
assert.match(setting, /新提醒语音播报/)
assert.match(setting, /setVoiceBroadcast/)
assert.match(setting, /store\.setSetting\('voice_broadcast'/)

console.log('voiceBroadcast: ok')
