import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第十九轮：
//   1. 模板管理器左侧要能管理（新建 / 复制 / 删除），右侧要是真正的编辑器；
//   2. 写信时用图标打开模板库挑一条插入，而不是一个会被截断的下拉。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')
const mailDir = 'src/components/ApplicationViewComponent/apps/mail/'

const settings = read(mailDir + 'MailSettingsDrawer.vue')
const composer = read(mailDir + 'MailComposer.vue')
const css = read(mailDir + 'mail.css')

test('模板管理：左侧可新建 / 复制 / 删除', () => {
  assert.ok(/function newTemplate/.test(settings), '新建')
  assert.ok(/async function duplicateTemplate/.test(settings), '复制一份')
  assert.ok(/async function removeTemplateById/.test(settings), '列表里直接删')
  assert.ok(/mail-template-row-actions/.test(settings), '每行右侧有操作按钮')
  assert.ok(/@click\.stop="duplicateTemplate\(tpl\)"/.test(settings), '复制按钮不能顺带切到编辑')
  assert.ok(/@click\.stop="removeTemplateById\(tpl\)"/.test(settings), '删除按钮同理')
  assert.ok(/askMailConfirm/.test(settings), '删除走应用内确认')
})

test('模板编辑：富文本 + 源码切换', () => {
  const editor = read(mailDir + 'MailTemplateEditor.vue')
  assert.ok(/contenteditable="true"/.test(editor), '正文是 rich text')
  assert.ok(/document\.execCommand\('styleWithCSS'/.test(editor), '与写信编辑器同一套命令')
  assert.ok(/function toggleSource/.test(editor), '能切源码')
  assert.ok(/function cleanPasted/.test(editor), '粘贴要清洗')
  assert.ok(/watch\(\s*\(\) => props\.modelValue/.test(editor), '切换模板时正文要跟着换')
  assert.ok(/lastEmitted/.test(editor), '自己发出的值不要回写（否则光标回跳）')
  assert.ok(/\.mail-template-editor \.mail-template-body/.test(css), '编辑器样式')
})

test('写信：图标打开模板库，点一条直接套用', () => {
  assert.ok(/title="从模板库插入"/.test(composer), '工具栏要有模板库入口')
  assert.ok(/openPalette === 'template'/.test(composer), '点开是一个面板')
  assert.ok(/v-for="t in templates"/.test(composer), '列出全部模板')
  assert.ok(/function applyTemplateNow/.test(composer), '点一条就套用')
  assert.ok(/applyTemplateNow\(t\)/.test(composer), '面板里的条目绑定到套用动作')
  // 面板本身就是确认，不要再弹一个系统 confirm（本地 webview 里可能根本不弹）。
  const fn = composer.slice(composer.indexOf('function applyTemplateNow'), composer.indexOf('function applyTemplateNow') + 600)
  assert.ok(!/window\.confirm/.test(fn), '不再二次确认')
  assert.ok(!/<select[\s\S]{0,200}套用模板/.test(composer), '旧的模板下拉已移除')
})
