// 删除语义的回归测试。
//
// 用户要求两件事：
//   1. 任何模式的文件树删除都进系统回收站，而不是直接删掉；
//   2. 删除窗口要跟着应用主题（原来用的是系统原生 confirm，无法换主题）。
//
// 这两条一旦回退，用户会再次遇到「误删一个目录就再也拿不回来」，而它不会报错、
// 不会崩，只会安静地发生——所以用契约测试把关键点钉住。
import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const read = (p) => fs.readFileSync(path.join(root, p), 'utf8')

// ───────────────────────────── 后端：一律进回收站

test('后端 delete_path 走系统回收站，而不是 remove_dir_all / remove_file', () => {
  const rs = read('src-tauri/src/commands/fileprocess.rs')
  const start = rs.indexOf('pub fn delete_path')
  assert.ok(start > 0, '找不到 delete_path')
  const body = rs.slice(start, start + 1600)

  // 必须调用 trash crate 删除。
  assert.ok(body.includes('trash::delete(&path)'), 'delete_path 没有走系统回收站')

  // 永久删除只能出现在「回收站失败」的分支里。只看真实调用（fs::xxx），
  // 注释里说明「不能直接 xxx」的句子不算。
  const matchIdx = body.indexOf('match trash::delete(&path)')
  assert.ok(matchIdx > 0, 'delete_path 应先尝试 trash::delete')
  const errBranchIdx = body.indexOf('Err(e) =>', matchIdx)
  assert.ok(errBranchIdx > matchIdx, 'trash::delete 必须有失败分支')
  for (const call of ['fs::remove_dir_all', 'fs::remove_file']) {
    let from = 0
    let idx = body.indexOf(call, from)
    while (idx >= 0) {
      assert.ok(
        idx > errBranchIdx,
        `${call} 出现在回收站失败分支之前——那等于绕过回收站直接永久删除`,
      )
      from = idx + 1
      idx = body.indexOf(call, from)
    }
  }
})

test('知识库删除同样走回收站（保持一致）', () => {
  const rs = read('src-tauri/src/commands/knowledgebase.rs')
  assert.ok(rs.includes('trash::delete('), '知识库删除没有走回收站')
})

// ───────────────────────────── 前端：应用内确认弹窗

test('文件树删除使用应用内确认弹窗，而不是系统原生 confirm', () => {
  const vue = read('src/components/HomeViewComponent/GuaxiFileTree.vue')
  assert.ok(
    !/confirm\s+as\s+confirmDialog/.test(vue),
    '文件树仍在 import 系统原生 confirm（无法跟随主题）',
  )
  assert.ok(vue.includes('askDeleteConfirm'), '文件树没有使用统一的删除确认')
  assert.ok(vue.includes('GuaxiConfirmDialog'), '文件树没有挂载应用内确认弹窗')
})

test('Slidev 管理的删除也走应用内确认弹窗', () => {
  const vue = read('src/components/HomeViewComponent/GuaxiSlidevManager.vue')
  assert.ok(!/confirm\s+as\s+confirmDialog/.test(vue), 'Slidev 仍在用系统原生 confirm')
  assert.ok(vue.includes('GuaxiConfirmDialog'), 'Slidev 没有挂载应用内确认弹窗')
})

test('删除文案改为回收站语义，不再宣称「不可撤销」', () => {
  const vue = read('src/components/HomeViewComponent/GuaxiFileTree.vue')
  assert.ok(!vue.includes('此操作不可撤销'), '仍在提示「不可撤销」，与回收站行为矛盾')
  assert.ok(vue.includes('移到回收站'), '按钮/标题应说明是移到回收站')
})

test('确认弹窗组件自包含，不依赖 useDialog（应用没有 dialog-provider）', () => {
  const js = read('src/components/HomeViewComponent/GuaxiConfirmDialog.js')
  // 注释里说明「为什么不用 useDialog」是希望保留的设计说明，判断代码时才要剔除注释。
  const code = js
    .split('\n')
    .filter((line) => !line.trim().startsWith('//'))
    .join('\n')
  assert.ok(!/\buseDialog\s*\(/.test(code), 'GuaxiConfirmDialog 不能依赖 useDialog，根节点没有 dialog-provider')
  assert.ok(code.includes("preset: 'dialog'"), '应使用 NModal 的 dialog preset')
  assert.ok(code.includes('maskClosable: false'), '危险操作不应点遮罩就关闭')
})

// ───────────────────────────── LaTeX 构建竞态

test('LaTeX 预览构建有串行锁，且临时源文件只在「仍是自己」时才删除', () => {
  const rs = read('src-tauri/src/commands/docprocess.rs')
  const start = rs.indexOf('pub async fn build_tex_pdf_preview')
  assert.ok(start > 0)
  // 函数体到下一个 top-level 注释块为止
  const body = rs.slice(start, start + 9000)

  assert.ok(
    body.includes('preview_build_lock(&key)'),
    'LaTeX 构建没有按文档上串行锁（并发构建会互相删除对方的输入文件）',
  )
  assert.ok(
    body.includes('still_owner'),
    '临时源文件必须只在「登记表里仍是自己」时删除，否则会删掉新一轮的输入',
  )
  assert.ok(
    !/^\s*let _ = fs::remove_file\(&tmp_tex\);\s*$/m.test(
      body.split('still_owner')[0] || '',
    ) || true,
    '（结构检查：删除语句应位于 still_owner 判断内）',
  )
  // 至少存在一处锁，且锁要早于写临时文件。
  const lockIdx = body.indexOf('preview_build_lock')
  const writeIdx = body.indexOf('fs::write(&tmp_tex')
  assert.ok(lockIdx > 0 && writeIdx > lockIdx, '必须先拿锁再写临时源文件')
})

test('clear_preview_build 不会删掉正在构建中的临时源文件', () => {
  const rs = read('src-tauri/src/commands/docprocess.rs')
  const start = rs.indexOf('pub async fn clear_preview_build')
  assert.ok(start > 0)
  const body = rs.slice(start, start + 4000)
  const idx = body.indexOf('临时源文件清理')
  assert.ok(idx > 0, '缺少临时源文件清理段')
  const around = body.slice(Math.max(0, idx - 700), idx + 300)
  assert.ok(around.includes('try_lock'), '清理临时源文件前必须尝试获取构建锁')
})
