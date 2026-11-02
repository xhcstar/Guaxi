import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第二十一轮：「我的文件夹」里删掉所有文件夹之后，QQ 自带的「其他文件夹」会自己冒回来。
//
// 根因是 user_created 被猜出来的：v5 迁移按「有邮件就算用户在用」回填，
// imap 每次同步又按「本地已存在 / 里面有邮件」重新标一遍——服务器目录因此
// 永远被当作「用户的」，删掉下一轮同步又回来。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const imapRs = read('src-tauri/src/commands/mail/imap.rs')
const storeRs = read('src-tauri/src/commands/mail/store.rs')
const modRs = read('src-tauri/src/commands/mail/mod.rs')
const sidebar = read('src/components/ApplicationViewComponent/apps/mail/MailSidebar.vue')

test('同步不再「猜」哪些是用户的文件夹', () => {
  assert.ok(
    /let user_created = role != FolderRole::Custom;/.test(imapRs),
    '服务器下发的自定义目录一律按「不是用户的」处理',
  )
  assert.ok(!/is_known/.test(imapRs), '不再用「本地已存在」来判断')
  assert.ok(!/total > 0 \|\| unread > 0/.test(imapRs), '不再用「里面有邮件」来判断')
  // 这个启发式函数已无用，别再留着误导后来人。
  assert.ok(!/fn looks_like_provider_container/.test(imapRs), '删掉旧的容器猜测函数')
})

test('v6 迁移把被误标的自定义文件夹清回「不是用户的」', () => {
  assert.ok(/pub const SCHEMA_VERSION: i32 = 7;/.test(storeRs), 'schema 版本要 +1')
  assert.ok(
    /if version < 6 \{[\s\S]{0,700}UPDATE folders SET user_created = 0 WHERE role = 'custom'/.test(storeRs),
    '一次性清掉自定义角色的误标',
  )
  // 标准角色不受影响：侧栏的「邮箱」区块按角色渲染，本来就不看这个标记。
  assert.ok(/role = 'custom'/.test(storeRs), '只清自定义角色')
  // v7：原生 Exchange 的远端字符串 ID → 本地 uid 映射。
  assert.ok(/CREATE TABLE IF NOT EXISTS remote_ids/.test(storeRs), '远端 ID 映射表')
  assert.ok(/if version < 7 \{[\s\S]{0,500}remote_ids/.test(storeRs), 'v7 迁移建立映射表')
})

test('用户建的文件夹仍会显示（新建时把已存在的行标成用户的）', () => {
  assert.ok(
    /fn note_local_folder[\s\S]{0,600}set_folder_user_created\(account_id, name, true\)/.test(modRs),
    '同名的服务器目录被用户「新建」后要变成可见',
  )
  // 侧栏列出服务器上的普通目录，只排除模板壳；允许为空。
  assert.ok(/visibleCustomFolders\(folders\.value\)/.test(sidebar), '侧栏走共享的可见规则')
  assert.ok(/我的文件夹/.test(sidebar), '我的文件夹区块保留（空的时候只剩新建按钮）')
})
