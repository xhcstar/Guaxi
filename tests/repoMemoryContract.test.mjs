// M2：Repo 自有材料接入感知与记忆的契约测试。
//
// 这条链路任何一段断开都不会报错，只会让「大脑对我的资料一无所知」：
// 摘录不进感知、读完的书记不成情节、资料库新增文件永远等不到初次索引。
// 与 docReaderWindow / activityBus 同一套做法：读源码做静态断言。
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const read = (p) => fs.readFileSync(path.join(root, p), 'utf8')

const perceptRs = read('src-tauri/src/cognition/percept.rs')
const readingRs = read('src-tauri/src/cognition/reading.rs')
const digestRs = read('src-tauri/src/cognition/library_digest.rs')
const commandsRs = read('src-tauri/src/cognition/commands.rs')
const runtimeRs = read('src-tauri/src/cognition/runtime.rs')
const cognitiveMods = read('src-tauri/src/cognition.rs')
const readerRs = read('src-tauri/src/commands/guaxireader.rs')

// ── 1) 自有材料入库口：纯文本、免质量闸门、可指定状态 ──
assert.match(perceptRs, /pub fn ingest_owned\(input: PerceptInput, state: &str\)/)
assert.match(perceptRs, /state: "new",/)
assert.match(perceptRs, /match state \{\s*\n\s*"new" \| "triaged" \| "linked" \| "archived"/)
// 默认可信度：用户自己的材料最高。
assert.match(perceptRs, /"manual" \| "chat" \| "note" \| "excerpt" => 1\.0/)
assert.match(perceptRs, /"file" \| "library" => 0\.9/)

// ── 2) 摘录/笔记：写盘后进感知（linked，不占收件箱） ──
assert.match(readerRs, /percept::ingest_owned\(input, "linked"\)/)
assert.match(readerRs, /fn kb_name_from_libraries_path/)
assert.match(readerRs, /source_kind: "note"\.to_string\(\)/)

// ── 3) 阅读情节：打开/进度/读完 → episodes(kind=read, outcome=ok) ──
assert.match(cognitiveMods, /pub mod reading;/)
assert.match(readingRs, /pub fn note_events\(/)
assert.match(readingRs, /pub fn close_stale\(/)
assert.match(readingRs, /pub async fn reinforce_finished\(/)
assert.match(readingRs, /episode::start\("read", "", "", &title, &kb\)/)
assert.match(readingRs, /episode::finish\(&session\.episode_id, "读完", "ok", 0, elapsed\)/)
assert.match(readingRs, /const FINISH_PROGRESS: f64 = 0\.98;/)
assert.match(readingRs, /FINISH_COOLDOWN_MS/)
assert.match(readingRs, /profile::reinforce_from_node/)

// 活动命令要把同一批事件交给阅读情节归并，并在读完时异步强化画像。
assert.match(commandsRs, /reading::note_events\(&reading_events\)/)
assert.match(commandsRs, /reading::reinforce_finished\(finished\)\.await/)

// ── 4) 资料库增量：扫描 markdown/md 目录，按水位增量入库 ──
assert.match(cognitiveMods, /pub mod library_digest;/)
assert.match(digestRs, /for sub in \["markdown", "md"\]/)
assert.match(digestRs, /library_digest_cursor/)
assert.match(digestRs, /percept::ingest_owned\(input, "linked"\)/)
assert.match(digestRs, /source_kind: "library"\.to_string\(\)/)

// ── 5) 日常维护：收尾放弃的阅读情节 + 跑资料库增量 ──
assert.match(runtimeRs, /reading::close_stale\(24\)/)
assert.match(runtimeRs, /library_digest::run\(\)/)

console.log('repoMemory: ok')
