import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第三十轮：通讯录点人打不开。
//
// 现象：点任何一个联系人都只剩「从左侧选一个联系人查看详情」，
// 控制台 `[ContactsStore] 打开联系人失败 Wrong number of parameters passed to query. Got 4, needed 3`。
// 根因在 `messages_with` 的 SQL：`?1..?n` 在 SELECT 与 WHERE 里各写一遍
// （SQLite 里同号只算一个参数），偏移却按「两遍地址」算，于是 1 个地址绑 4 个参数给 3 个占位符。
// 另外 `Promise.all` 让「往来邮件」这个附属列表的失败连坐了联系人本身。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const contactsStore = read('src-tauri/src/commands/mail/contacts/store.rs')
const storeJs = read('src/stores/contactsStore.js')

test('往来邮件 SQL：收件人 LIKE 的编号从地址个数 +1 往后排', () => {
  const fn = contactsStore.slice(contactsStore.indexOf('fn messages_with_sql'))
  const body = fn.slice(0, fn.indexOf('/// 列出与某联系人的往来邮件'))
  // 地址参数只绑一遍：地址出现两次 IN（同号复用），也只 push 一次。
  const pushes = body.match(/args\.push\(/g) || []
  assert.equal(pushes.length, 3, '地址 + 收件人 LIKE + LIMIT，共三次绑定')
  assert.ok(
    /LOWER\(to_json\) LIKE \?\{\}[\s\S]{0,60}n \+ 1/.test(body),
    'LIKE 用 ?n+1',
  )
  assert.ok(/LIMIT \?\{\}[\s\S]{0,40}n \+ 2/.test(body), 'LIMIT 用 ?n+2')
  assert.ok(
    !/for email in emails \{\s*let like/.test(body),
    '别再按「两遍地址」补参数',
  )
})

test('通讯录详情：往来邮件列表失败不连坐联系人本身', () => {
  const open = storeJs.slice(storeJs.indexOf('async open(id)'))
  const body = open.slice(0, open.indexOf('closeActive()'))
  assert.ok(
    /invoke\('mail_contact_messages'[^)]*\)\.catch\(/.test(body),
    '附属列表要自己吞掉错误，返回空数组',
  )
  assert.ok(/invoke\('mail_get_contact'/.test(body), '联系人本体仍然是硬依赖')
  assert.ok(/this\.error = ''/.test(body), '每次打开先清掉上一次的报错')
})
