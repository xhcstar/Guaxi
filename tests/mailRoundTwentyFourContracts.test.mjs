import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第二十四轮：
//   1. 图片和表格都还不能用鼠标拖动换位置；
//   2. 点了「放弃」，草稿箱里还是多出那封信。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')
const mailDir = 'src/components/ApplicationViewComponent/apps/mail/'

const composer = read(mailDir + 'MailComposer.vue')
const css = read(mailDir + 'mail.css')
const store = read('src/stores/mailStore.js')
const app = read(mailDir.replace('mail/', '') + 'MailApp.vue')
const bridge = read(mailDir + 'composeWindowBridge.js')
const shell = read(mailDir + 'MailComposeWindow.vue')

test('图片 / 表格：指针捕获 + 落点线，拖到任意段落之间', () => {
  assert.ok(/function onEditorPointerDown/.test(composer), '按下图片进入拖动')
  assert.ok(
    /function beginBlockDrag[\s\S]{0,1500}setPointerCapture/.test(composer),
    '拖动时捕获指针，不再依赖 window 上的 mousemove（会被原生拖拽吃掉）',
  )
  assert.ok(/function startBarDrag/.test(composer), '浮条上要有拖动把手')
  assert.ok(/startBarDrag\('image'/.test(composer) && /startBarDrag\('table'/.test(composer), '图片与表格都要能拖')
  assert.ok(/function moveTableStep/.test(composer) && /moveTableStep\(-1\)/.test(composer), '表格也要能整块上移/下移')
  assert.ok(/function updateDropTarget/.test(composer), '按指针位置算落点')
  assert.ok(
    /function onBlockDragEnd[\s\S]{0,1400}insertAdjacentElement\('beforebegin'/.test(composer),
    '松手才真正移动 DOM',
  )
  assert.ok(/class="mail-drop-indicator"/.test(composer), '拖动中要能看到落点')
  assert.ok(/\.mail-drop-indicator\s*\{[\s\S]{0,160}position: fixed/.test(css), '落点线不占版面')
  assert.ok(/body\.mail-block-dragging/.test(css), '拖动期间不许划选文字')
})

test('拖动要顺手：跟手浮层、边缘自动滚动、表格左上角把手、左右也能挪', () => {
  // 「很生硬」：以前松开才动、到边缘不滚、左右完全不动。
  assert.ok(/function createGhost/.test(composer), '拖动时有一份跟手的浮层')
  assert.ok(/\.mail-block-ghost\s*\{[\s\S]{0,160}position: fixed/.test(css), '浮层不参与版面')
  assert.ok(/function runAutoScroll/.test(composer), '拖到正文上下边缘要自动滚动')
  assert.ok(/AUTO_SCROLL_EDGE/.test(composer), '边缘有感应区')
  assert.ok(/function startTableHandleDrag/.test(composer), '表格左上角要有拖动把手')
  assert.ok(/class="mail-table-move"/.test(composer) && /\.mail-table-move\s*\{[\s\S]{0,200}position: fixed/.test(css))
  assert.ok(/function applyHorizontalDrop/.test(composer), '左右拖动写成缩进')
  assert.ok(/marginLeft/.test(composer), '水平位置落在 margin-left 上，收件端也认')
  // 图片常被浏览器包在段落里：落点要按「最近的顶层块」算，不能因此整块失效。
  assert.ok(/function topLevelAncestor/.test(composer), '按最近的顶层块定位')
  assert.ok(/dropKeep/.test(composer), '在自己段落里上下晃时不要把它拽出去')
})

test('放弃：自动保存进草稿箱的那份也要删掉', () => {
  assert.ok(
    /async discardCompose\(\)[\s\S]{0,400}_discardMailboxDraft/.test(store),
    '主界面的「直接放弃」要清掉自动草稿',
  )
  assert.ok(
    /async _discardMailboxDraft[\s\S]{0,600}return true/.test(store),
    '删除结果要能回传，删不掉就不能假装成功',
  )
  assert.ok(
    /async function onDiscardCompose[\s\S]{0,240}const cleaned = await store\.discardCompose\(\)/.test(app),
    '界面要等清理结果再提示',
  )
  assert.ok(/function discardAutoDraft/.test(bridge) && /export async function discardWindow/.test(bridge), '独立窗口也要清自动草稿')
  assert.ok(/async function onDiscard\(\)[\s\S]{0,120}discardWindow\(\)/.test(shell), '放弃按钮走清理路径')
  // 放弃之后，定时自动保存与卸载暂存都不能再把内容写回去。
  assert.ok(/if \(discarded\) return null/.test(composer), '放弃后不再自动暂存')
  assert.ok(/if \(!discarded\) stashNow\(\{ toMailbox: false \}\)/.test(composer), '卸载时不再补写暂存')
  assert.ok(
    /async function discardForget[\s\S]{0,400}discarded = true[\s\S]{0,200}stopAutosave\(\)/.test(composer),
    '放弃时先停下自动保存',
  )
})
