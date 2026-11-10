import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第二十九轮：性能。
//
// 实测（真实库，4.9GB / 7767 封）：
//   - 每次 `store::open()` 打开库约 50ms，热查询本身 0.0006ms；
//   - 「开库 + 事务提交 + 3 条 UPDATE」逐封做是 639ms/封，同一个事务里做整批是 0.64ms/封。
// 也就是说，正文读得慢、分类慢都不是查询慢，而是把同一件事拆成了成百上千次
// 重新开库 / 重新提交。这些断言锁住「不要再走回逐封那条路」。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const mailMod = read('src-tauri/src/commands/mail/mod.rs')
const mailStore = read('src-tauri/src/commands/mail/store.rs')
const storeJs = read('src/stores/mailStore.js')
const reader = read('src/components/ApplicationViewComponent/apps/mail/MailReader.vue')
const mainRs = read('src-tauri/src/main.rs')

test('分类：标记改动整批一次写，不再逐封开库', () => {
  assert.ok(/pub fn apply_flag_changes/.test(mailStore), '要有一批标记的写入入口')
  const flagFn = mailStore.slice(mailStore.indexOf('pub fn apply_flag_changes'))
  assert.ok(
    /for change in changes/.test(flagFn.slice(0, 2500)),
    '一个事务里循环写完整批',
  )
  // 规则执行必须累积后再写：循环体里再调 set_local_flags 就是逐封开一次库。
  const batch = mailMod.slice(mailMod.indexOf('async fn apply_rules_to_new'))
  const loopEnd = batch.indexOf('// 本地标记（已读 / 星标 / 标签）整批一次写完')
  const loop = batch.slice(0, loopEnd)
  assert.ok(/flag_changes\.push\(store::FlagChange/.test(loop), '循环里只累积，不落库')
  assert.ok(!/store::set_local_flags\(/.test(loop), '循环里不能再逐封写标记')
  assert.ok(/store::apply_flag_changes\(&flag_changes\)/.test(batch), '循环结束后一次写入')
})

test('分类：服务器搬完后整批删本地行，不再逐封 delete_message', () => {
  assert.ok(/pub fn delete_messages\(/.test(mailStore), '要有整批删除')
  assert.ok(
    /pub fn delete_messages[\s\S]{0,300}tx\.commit\(\)/.test(mailStore),
    '整批删除在一个事务里提交',
  )
  // 任何「移动 / 删除成功后清本地镜像」的地方都不该再逐封调 delete_message。
  const perUidLoops =
    mailMod.match(/for uid in &?uids \{[\s\S]{0,200}?delete_message\(/g) || []
  assert.equal(perUidLoops.length, 0, `还有 ${perUidLoops.length} 处逐封删除`)
  assert.ok(/store::delete_messages\(&account\.id, folder, (?:&)?uids\)/.test(mailMod), '改用整批删除')
})

test('打开一封邮件：摘要 / 正文 / 渲染版本一次连接读完', () => {
  assert.ok(/pub fn open_records\(/.test(mailStore), '要有一次读完的入口')
  assert.ok(/pub struct OpenRecords/.test(mailStore), '返回摘要 + 正文 + 渲染版本')
  const openFn = mailMod.slice(mailMod.indexOf('pub async fn mail_get_message'))
  const openBody = openFn.slice(0, openFn.indexOf('/// 一条会话里所有'))
  assert.ok(/store::open_records\(/.test(openBody), '打开邮件走一次读完')
  assert.ok(!/store::get_summary\(/.test(openBody), '不再单独取摘要')
  assert.ok(!/store::get_body\(/.test(openBody), '不再单独取正文')
  assert.ok(!/store::body_render_version\(/.test(openBody), '不再单独取渲染版本')
})

test('会话视图：整批读正文，逐封只补缺失的那几封', () => {
  assert.ok(/pub fn open_thread_records/.test(mailStore), '一条连接读完整条会话')
  assert.ok(/pub async fn mail_thread_details/.test(mailMod), '要有批量读正文的命令')
  assert.ok(/commands::mail::mail_thread_details/.test(mainRs), '命令要注册')
  assert.ok(/invoke\('mail_thread_details'/.test(storeJs), '会话打开先用批量命令')
  // 批量没命中的那几封（本地没缓存）要按原路补拉，不能少画。
  assert.ok(
    /prefetched\[index\][\s\S]{0,320}invoke\('mail_get_message'/.test(storeJs),
    '缺失的逐封补拉',
  )
})

test('阅读区：正文渲染结果缓存，且 DOM 没变时不重跑主题映射', () => {
  assert.ok(/const threadBodies = new Map\(\)/.test(reader), '会话正文按邮件缓存')
  assert.ok(
    /function threadBody\(message\)[\s\S]{0,200}threadBodies\.get\(cacheKey\)/.test(reader),
    '先查缓存再解析',
  )
  assert.ok(/threadBodies\.clear\(\)/.test(reader), '换会话时清掉缓存')
  assert.ok(/const bodySignature = computed/.test(reader), '正文 DOM 指纹')
  assert.ok(
    /function reapplyBodyDecor\(\)[\s\S]{0,200}signature === appliedSignature\) return/.test(reader),
    '指纹没变就跳过整棵映射',
  )
  assert.ok(/onUpdated\(reapplyBodyDecor\)/.test(reader), '每次 patch 后仍会补一遍（只是可能跳过）')
})
