import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第二十五轮：
//   1. 邮件组（分发列表）地址永远进不了通讯录；
//   2. 规则「对已有邮件执行」看起来一直卡住（搬几百封要跑几十分钟）。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const mod = read('src-tauri/src/commands/mail/mod.rs')
const store = read('src-tauri/src/commands/mail/store.rs')
const contacts = read('src-tauri/src/commands/mail/contacts/mod.rs')
const rules = read('src-tauri/src/commands/mail/rules.rs')

test('收件人 JSON 按 MailAddress 解析，邮件组地址才能进通讯录', () => {
  // messages.to_json / cc_json 是 `[{"name","address"}]`；以前按 LabeledValue 反序列化，
  // 少了 value 字段 → 整段失败 → 只收进发件人，收件人里的组地址永远不进通讯录。
  assert.ok(/serde_json::from_str::<Vec<MailAddress>>\(json\)/.test(mod), '同步路径按 MailAddress 解析')
  assert.ok(
    /serde_json::from_str::<Vec<crate::commands::mail::model::MailAddress>>\(json\)/.test(contacts),
    '重建路径同样按 MailAddress 解析',
  )
  assert.ok(/label: item\.name,[\s\S]{0,80}value: item\.address/.test(mod), '转成通讯录的 label/value')
  // 自己发出的邮件也要收件人：组地址往往只出现在「已发送」里。
  assert.ok(
    /for message in items \{[\s\S]{0,400}recipients_of\(&message\.to_json\)/.test(mod),
    '发出去的收件人也要收录',
  )
  assert.ok(/backfill_recipients_once/.test(contacts), '升级后自动补一次老邮件的收件人')
  assert.ok(/backfill_recipients_once\(\)/.test(mod), 'init 时后台触发补齐')
  // 发送成功后主动收录收件人（组地址只发不收时也能记住）。
  assert.ok(
    /mail: 发送成功[\s\S]{0,2200}touch_from_message/.test(mod),
    '发送成功后把收件人写进通讯录',
  )
})

test('规则批量执行：不再每封重算整个文件夹、不再每封连一次服务器', () => {
  const batch = mod.slice(mod.indexOf('async fn apply_rule_move_batch'), mod.indexOf('async fn forward_by_rule'))
  assert.ok(/uids: &\[u32\]/.test(batch), '移动按 UID 批量执行')
  assert.ok(/imap::move_messages\(&acc, "", &folder_owned, &uids_owned/.test(batch), 'IMAP 一次搬一批')
  assert.ok(/move_locally\(&account\.id, folder, uids, target\)/.test(batch), 'POP3 一次搬一批')
  // 命中动作先收集，循环结束后统一执行。
  assert.ok(/HashMap<\(String, String\), Vec<u32>>/.test(mod), '按「源夹 → 目标夹」合并移动')
  assert.ok(
    /for \(\(folder, target\), uids\) in moves[\s\S]{0,120}apply_rule_move_batch/.test(mod),
    '循环外批量移动',
  )
  // 计数改成 SQL，不再 list_recent(..., u32::MAX) 全表物化。
  assert.ok(/pub fn folder_counts/.test(store), 'store 提供 COUNT 计数')
  const totals = mod.slice(mod.indexOf('fn refresh_folder_totals'), mod.indexOf('fn refresh_folder_totals') + 600)
  assert.ok(/store::folder_counts/.test(totals), '重算角标用 SQL 计数')
  assert.ok(!/list_recent/.test(totals), '不再为计数读取整个文件夹')
  // 规则只看正文纯文本：别顺带解密 HTML 与附件清单。
  assert.ok(/pub fn get_body_text/.test(store), '提供只读 text 的入口')
  assert.ok(/get_body_text\(&msg\.account_id/.test(mod), '规则匹配走纯文本')
  // 先用不需要正文的条件筛一遍：几千封里只有真正候选才解密正文。
  assert.ok(/pub fn body_relevant/.test(rules), '提供「这条规则是否还可能要正文」的预判')
  assert.ok(/body_relevant\(&picked, &cheap_ctx\)/.test(mod), '对已有邮件执行时先按轻条件筛')
  // POP3 本地搬行时原文缓存也要跟着走，否则搬进规则文件夹后附件打不开。
  assert.ok(
    /move_locally[\s\S]{0,1600}cached_raw_body\(account_id, folder, \*uid\)/.test(mod),
    '移动时保留原邮件缓存',
  )
})
