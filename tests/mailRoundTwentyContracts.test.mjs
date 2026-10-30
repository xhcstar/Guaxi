import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第二十轮：
//   1. 图片还是拖不动（只有左对齐）；
//   2. 放弃菜单是透明的，指针还能穿透去触发下面图标的提示；
//   3. 「存为草稿并关闭」很慢——关窗前等了草稿箱同步。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const composer = read('src/components/ApplicationViewComponent/apps/mail/MailComposer.vue')
const css = read('src/components/ApplicationViewComponent/apps/mail/mail.css')
const store = read('src/stores/mailStore.js')
const bridge = read('src/components/ApplicationViewComponent/apps/mail/composeWindowBridge.js')
const mod = read('src-tauri/src/commands/mail/mod.rs')

test('图片移动：按坐标算落点，不依赖命中测试', () => {
  // 以前用 elementFromPoint 找块：指针落在图片上/内边距上/末尾空白时都命中不到块，
  // 表现就是「拖了没反应」。改成按鼠标纵向位置插到第一个中线在指针下方的块前面。
  assert.ok(/function updateDropTarget/.test(composer), '按 Y 坐标决定插到哪')
  assert.ok(!/document\.elementFromPoint/.test(composer), '不再依赖命中测试')
  assert.ok(
    /for \(const child of editor\.children\)[\s\S]{0,200}clientY < rect\.top \+ rect\.height \/ 2/.test(composer),
    '用块的中线判断插在前面还是后面',
  )
  // 控制框上有专门的拖动把手，不必正好按在图片上。
  assert.ok(/class="mail-image-move"/.test(composer), '控制框要有拖动把手')
  assert.ok(/function startFrameMove/.test(composer), '把手要能开始拖动')
  assert.ok(/\.mail-image-move\s*\{[\s\S]{0,400}cursor: move/.test(css), '把手的光标要是 move')
})

test('放弃菜单：fixed + 不透明 + 盖在最上层', () => {
  const block = css.slice(css.indexOf('.mail-discard-panel {'), css.indexOf('.mail-discard-title'))
  assert.ok(/position: fixed/.test(block), '不要在操作条里绝对定位（会被页头层叠压住）')
  assert.ok(/background: var\(--mail-panel, #[0-9a-f]{6}\)/.test(block), '背景要有兜底色，不能透明')
  assert.ok(/z-index: 7\d/.test(block), '层级要高于工具栏')
  assert.ok(/function toggleDiscardPanel/.test(composer), '打开时按按钮位置算坐标')
  assert.ok(/discardPos/.test(composer) && /getBoundingClientRect/.test(composer), '用按钮矩形定位')
})

test('存草稿并关闭：不等草稿箱同步', () => {
  // 前端：关窗路径显式关掉同步
  assert.ok(/syncAfterAppend: false/.test(store), '内联关闭不等同步')
  assert.ok(/saveDraft\(\{ syncFolder: false \}\)/.test(bridge), '独立窗口关闭不等同步')
  // 后端：append 成功后同步放到后台，马上返回
  assert.ok(/sync_after_append: Option<bool>/.test(mod), '命令要有这个开关')
  assert.ok(
    /if !sync_after_append \{[\s\S]{0,600}spawn_blocking[\s\S]{0,600}refresh_folder_totals/.test(mod),
    '关掉同步时把同步丢到后台，并在后台补计数',
  )
  assert.ok(/"deferred": true/.test(mod), '要告诉前端这次没有 UID')
})
