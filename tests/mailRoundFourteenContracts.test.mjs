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
  const block = css.slice(css.indexOf('.mail-body table td,'), css.indexOf('.mail-body table td,') + 400)
  assert.ok(/border-color: var\(--mail-border\)/.test(block), '把单元格边框颜色统一成主题色')
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
