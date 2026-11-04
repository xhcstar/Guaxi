import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 会话（threading）：市面邮箱（Outlook / Gmail / Thunderbird）都按
// `References` / `In-Reply-To` 归并，主题只是兜底。
//
// 只按主题归并时，只要有人回信改了主题（`[bug 801882]…` → `BUG #131561 …`），
// 或者某个客户端回信时把 Re: 前缀吃掉，同一条会话就会被撕成两段 ——
// 用户看到的现象就是「回复丢了」。这一组契约守住归并规则与阅读区的兜底行为。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const storeRs = read('src-tauri/src/commands/mail/store.rs')
const mimeRs = read('src-tauri/src/commands/mail/mime.rs')
const imapRs = read('src-tauri/src/commands/mail/imap.rs')
const modRs = read('src-tauri/src/commands/mail/mod.rs')
const store = read('src/stores/mailStore.js')
const reader = read('src/components/ApplicationViewComponent/apps/mail/MailReader.vue')
const css = read('src/components/ApplicationViewComponent/apps/mail/mail.css')

test('库里存得下引用链，并能按消息号反查会话', () => {
  assert.ok(
    /if version < 9 \{[\s\S]{0,900}add_column\(&tx, "messages", "in_reply_to"/.test(storeRs),
    'v9 迁移补 in_reply_to 列',
  )
  assert.ok(
    /if version < 9 \{[\s\S]{0,900}add_column\(&tx, "messages", "msg_refs"/.test(storeRs),
    'v9 迁移补 msg_refs 列',
  )
  assert.ok(/CREATE TABLE IF NOT EXISTS thread_aliases/.test(storeRs), '主题别名表')
  assert.ok(
    /CREATE INDEX IF NOT EXISTS idx_messages_msgid ON messages \(account_id, message_id\)/.test(
      storeRs,
    ),
    '按消息号反查要有索引',
  )
})

test('归并顺序：引用链 → 主题别名 → 主题', () => {
  assert.ok(/pub fn message_ids\(in_reply_to: &str, references: &str\)/.test(mimeRs), '拆消息号')
  assert.ok(
    /fn resolve_thread_key[\s\S]{0,1200}lookup_thread_key_by_message_id[\s\S]{0,600}alias_thread_key/.test(
      storeRs,
    ),
    '先认引用链，再认主题别名',
  )
  // 命中多段会话（历史上被主题拆开过）要合并回一段，而不是各留一份。
  assert.ok(/fn merge_threads\(/.test(storeRs), '会话合并')
  assert.ok(
    /UPDATE messages SET thread_key=\?1 WHERE account_id=\?2 AND thread_key=\?3/.test(storeRs),
    '合并只动同账户',
  )
  // 主题被改过的回信：把新主题记成别名，后续同类回信才能继续挂回来。
  assert.ok(/fn set_thread_alias\(/.test(storeRs), '记录主题别名')
})

test('写库前一定算过会话归属', () => {
  assert.ok(
    /let thread_key = resolve_thread_key\(conn, m\)\?;/.test(storeRs),
    'upsert 用解析后的键，而不是调用方传进来的主题键',
  )
  assert.ok(
    /in_reply_to=excluded\.in_reply_to, msg_refs=excluded\.msg_refs/.test(storeRs),
    '重复同步时引用链也要更新',
  )
  // IMAP / POP3 解析出的引用头必须落库，否则归并永远只能靠主题。
  assert.ok(/in_reply_to: parsed\.in_reply_to\.clone\(\),/.test(imapRs), 'IMAP 带上引用头')
  // 正文命中缓存时也要能拼出回复链，否则本机发出去的回复会丢 In-Reply-To。
  assert.ok(/in_reply_to: summary\.in_reply_to\.clone\(\),/.test(storeRs), '缓存路径也带引用头')
  // 打开一封老邮件会重新解析原文：顺手把引用链补回去，历史会话因此自愈。
  assert.ok(
    /summary\.in_reply_to = parsed\.in_reply_to\.clone\(\);/.test(modRs),
    '打开时补写引用链',
  )
})

test('阅读区：任何入口打开都进会话视图，且不静默截断', () => {
  assert.ok(
    /if \(resolveThread && row\?\.threadKey\) \{[\s\S]{0,200}threadRowsFor\(row\)/.test(store),
    'openMessage 先解析会话',
  )
  assert.ok(/async threadRowsFor\(row\)/.test(store), '会话成员回库查，而不是只看列表那一页')
  assert.ok(/const THREAD_RENDER_LIMIT = \d+/.test(store), '渲染上限有名字')
  assert.ok(!/\.slice\(-30\)/.test(store), '不再静默只留最近 30 封')
  assert.ok(/threadTotal/.test(store), '记住会话总封数，供阅读区说明截断')
  assert.ok(/\[\.\.\.props\.thread\]\.reverse\(\)/.test(reader), '会话卡片最新在上')
  assert.ok(/mail-thread-notice/.test(reader), '截断时要说明')
  assert.ok(/\.mail-thread-notice \{/.test(css), '提示条有样式')
})

// 删账户 / 清空本地缓存之后，主题别名指向的会话键已经不存在了。
// 留着的话重新同步时，新邮件会被塞进一个空会话，同主题的新邮件也会被错并到旧键上——
// 重新拉一遍邮件反而比不拉更乱（开发期反复删号重同步会踩到）。
test('删账户与清缓存时，会话别名一起清掉', () => {
  const deleteFn = storeRs.slice(storeRs.indexOf('pub fn delete_account'))
  assert.ok(
    /DELETE FROM thread_aliases WHERE account_id = \?1/.test(deleteFn.slice(0, 4000)),
    '删账户时清掉该账户的主题别名',
  )
  const clearFn = storeRs.slice(storeRs.indexOf('pub fn clear_cache'))
  assert.ok(
    /DELETE FROM thread_aliases WHERE account_id=\?1/.test(clearFn.slice(0, 2500)),
    '按账户清缓存时清别名',
  )
  assert.ok(
    /DELETE FROM thread_aliases", \[\]/.test(clearFn.slice(0, 2500)),
    '清全部缓存时别名同样清空',
  )
})
