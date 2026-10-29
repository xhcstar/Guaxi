import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第十八轮：
//   1. 切换账号默认进收件箱，而不是「全部邮件」；
//   2. 图片控制框滚动时会叠到工具栏上；图片也拖不动（原生拖拽抢了事件）；
//   3. Delete 等快捷键要跟行上的删除按钮行为一致（删整条会话）。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const store = read('src/stores/mailStore.js')
const composer = read('src/components/ApplicationViewComponent/apps/mail/MailComposer.vue')
const css = read('src/components/ApplicationViewComponent/apps/mail/mail.css')
const mailApp = read('src/components/ApplicationViewComponent/apps/MailApp.vue')

test('切账号：默认落在收件箱', () => {
  const fn = store.slice(store.indexOf('selectAccount(id)'), store.indexOf('selectAccount(id)') + 600)
  assert.ok(/this\.activeFolder = inbox\?\.name/.test(fn), '要切到收件箱')
  assert.ok(
    /this\.scope = id \? 'folder' : 'all'/.test(fn),
    '具体账户进收件箱；「全部账户」才用跨账户视图',
  )
})

test('图片控制框：裁剪在编辑区里，滚动时也跟着走', () => {
  assert.ok(/class="mail-image-frame-layer"/.test(composer), '控制框要套一层裁剪层')
  assert.ok(
    /\.mail-image-frame-layer\s*\{[\s\S]{0,160}overflow: hidden/.test(css),
    '裁剪层要 overflow:hidden',
  )
  assert.ok(/\.mail-image-frame\s*\{[\s\S]{0,80}position: absolute/.test(css), '框相对裁剪层定位')
  // 以前只在浮条可见时重算，只选图片时滚动不重算，框就停在原地叠到工具栏上。
  assert.ok(
    /function onEditBarReflow[\s\S]{0,320}else refreshImageFrame\(\)/.test(composer),
    '没有浮条时也要刷新控制框',
  )
  assert.ok(
    /imageBox\.bottom <= viewBox\.top/.test(composer),
    '整张图滚出可视区就不再画框',
  )
  assert.ok(
    /closest\?\.\('\.mail-compose-body'\)/.test(composer),
    '裁剪范围要取滚动容器的矩形（编辑区高度等于全文高度）',
  )
})

test('图片可拖动：禁掉浏览器的原生图片拖拽', () => {
  assert.ok(/\.mail-compose-editor img \{[\s\S]{0,120}-webkit-user-drag: none/.test(css), '关掉原生拖拽')
  assert.ok(/@dragstart\.prevent/.test(composer), '编辑器里拦下 dragstart')
  assert.ok(/function onEditorMouseDown/.test(composer), '位置改由指针逻辑处理')
})

test('快捷键删除：与行内删除按钮一致（整条会话）', () => {
  const fn = mailApp.slice(mailApp.indexOf('async function onDeleteCurrent'), mailApp.indexOf('async function onArchiveCurrent'))
  assert.ok(/store\.runBatch\('delete', null, \[row\]\)/.test(fn), '走 runBatch 才会按 threadUids 展开')
  assert.ok(!/store\.deleteMessages\(/.test(fn), '不再走单封删除')
})
