import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// POP3 本地库为什么会越用越大、同一封邮件为什么会重复出现。
//
// 三个都是用户能直接看见的现象，锁在源码里：
//   1. 原文缓存的 params! 顺序和列顺序不一致 → 整段加密原文被写进 account_id，
//      真正的 raw 列没存上（附件每次重新下载），还留下一堆对不上邮件的垃圾行
//      （实测库里 3566 行、约 500 MB）；
//   2. 规则自动归类会换 uid，UIDL→uid 映射没跟着走 → 同步时以为「本地已删除」，
//      清掉映射，下一轮原样再下一遍（实测 SSL 夹 1414 行只有 207 封不同的邮件）；
//   3. 「服务器上保留副本」时清掉 UIDL，等于允许这封被重新下载。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const store = read('src-tauri/src/commands/mail/store.rs')
const mod = read('src-tauri/src/commands/mail/mod.rs')
const pop3 = read('src-tauri/src/commands/mail/pop3_sync.rs')

test('原文缓存：参数顺序必须和列顺序一一对应', () => {
  const at = store.indexOf('INSERT INTO message_bodies (account_id, folder, uid, raw)')
  assert.ok(at >= 0, 'cache_raw_body_on 的写入语句要还在')
  const stmt = store.slice(at, at + 400)
  assert.ok(
    /params!\[account_id, folder, uid, payload\]/.test(stmt),
    'payload 必须落在 raw 那一列；写进 account_id 会把整段原文变成主键',
  )
})

test('POP3 本地移动尽量保留 uid，换号时把 UIDL 映射一起挪', () => {
  const at = mod.indexOf('fn move_locally')
  assert.ok(at >= 0)
  const body = mod.slice(at).match(/^[\s\S]*?\r?\n\}\r?\n/)?.[0] || ''
  // 保留原号是主路径：映射记的就是这个号。
  assert.ok(/else \{\s*\*uid\s*\}/.test(body), '目标夹没占用时就该沿用原 uid')
  // 只有确实撞号才换新号，并且必须同步改映射。
  assert.ok(/pop3_remap_uid\(account_id, \*uid, free\)/.test(body), '换号要把 UIDL 映射跟过去')
  assert.ok(
    /fn pop3_remap_uid/.test(store) && /fn next_free_uid/.test(store),
    '映射改号与全局取号两个入口都要有',
  )
})

test('同步：搬走的邮件不算「本地已删除」，保留副本时不清 UIDL', () => {
  assert.ok(
    /message_exists_any_folder/.test(pop3),
    '判断「还在不在本地」必须看所有文件夹，只查收件箱会把归类走掉的邮件当成删了',
  )
  assert.ok(
    !/get_summary\(&account\.id, INBOX, uid\)/.test(pop3),
    '不能再只查收件箱',
  )
  const keepAt = pop3.indexOf('if keep_on_server')
  assert.ok(keepAt >= 0, '保留副本的分支要单独处理')
  const keepBlock = pop3.slice(keepAt, keepAt + 400)
  assert.ok(
    /continue;/.test(keepBlock) && !/pop3_forget_uidl/.test(keepBlock),
    '保留副本时清掉 UIDL，下一轮这封就会被当成新邮件再下一遍',
  )
})
