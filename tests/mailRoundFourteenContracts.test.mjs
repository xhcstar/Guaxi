import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第十四轮：
//   1. 收信正文的表格边框「有破缺」——邮件按白纸写的白/浅灰边框，深色纸面上看不见；
//   2. 会话聚合行点一次删除只删掉其中一封，行还在。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const css = read('src/components/ApplicationViewComponent/apps/mail/mail.css')
const store = read('src/stores/mailStore.js')

test('收信表格：只统一边框颜色，不新增边框', () => {
  const start = css.indexOf('.mail-doc-dark .mail-body table,')
  assert.ok(start >= 0, '暗色下要统一表格边框颜色')
  const block = css.slice(start, start + 600)
  // 用比映射后的单元格表面亮一档的 --mail-doc-line，而不是纸面边框色：
  // 浅色单元格在暗色下被抬到 #2c2d33 左右，同亮度的线会整片消失（用户反馈的「表格没边框」）。
  assert.ok(/border-color:\s*var\(--mail-doc-line\)/.test(block), '边框颜色要换成深色下看得见的分隔线')
  assert.ok(/--mail-doc-line:\s*#[0-9a-f]{6}/i.test(css), '分隔线颜色本身要定义在深色纸面变量里')
  assert.ok(!/border:\s*1px solid/.test(block), '不能给所有单元格加边框（排版表会被刷成套娃框）')
})

test('删除会话行：要连同这一条聚合的所有邮件一起删', () => {
  // runBatch 是所有批量动作（删除 / 移动 / 垃圾）的唯一入口，聚合行在这里展开。
  const batch = store.slice(store.indexOf('async runBatch('), store.indexOf('async runBatch(') + 900)
  assert.ok(/row\.threadUids/.test(batch), '聚合行要按 threadUids 展开')
  assert.ok(/for \(const member of members\)/.test(batch), '展开成逐封的目标行')
  assert.ok(/const seen = new Set\(\)/.test(batch), '多选时同一封不能重复处理')
  // 列表行的 threadUids 存的是完整行（不是 uid），展开后 group 才能按账户+文件夹分组。
  assert.ok(/threadCount: bucket\.length,\s*\n\s*threadUids: bucket,/.test(store), '聚合行带着完整成员行')
})
