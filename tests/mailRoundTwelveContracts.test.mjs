import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第十二轮：
//   1. 设置页「当前目录」永远停在「（读取中）」——读取函数从来没被调用过；
//   2. 邮件模板独立成页，左边列表、右边大编辑区；
//   3. 弹层太小，而且点外面 / 从里往外拖一下就关掉（会丢正在填的内容）。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')
const mailDir = 'src/components/ApplicationViewComponent/apps/mail/'

const settings = read(mailDir + 'MailSettingsDrawer.vue')
const css = read(mailDir + 'mail.css')
const mailApp = read('src/components/ApplicationViewComponent/apps/MailApp.vue')

test('存储位置：打开设置就读取，失败也要说明原因', () => {
  assert.ok(/onMounted\(loadStorage\)/.test(settings), '打开设置时要真的去读存储信息')
  assert.ok(/mail_storage_info/.test(settings), '用后端命令读取当前目录')
  assert.ok(/storageError/.test(settings) && /读取存储位置失败/.test(settings), '失败要说原因，不能永远停在「读取中」')
})

test('邮件模板：独立页签 + 左侧列表 + 大编辑区', () => {
  assert.ok(/key: 'templates'/.test(settings), '模板要有自己的页签')
  assert.ok(/'邮件模板'/.test(settings), '页签名是「邮件模板」')
  assert.ok(/class="mail-templates-layout"/.test(settings), '模板页要分栏')
  assert.ok(/class="mail-templates-list"/.test(settings), '左侧是已有模板列表')
  assert.ok(/mail-template-body/.test(settings), '右侧正文编辑区')
  assert.ok(/\.mail-template-body\s*\{[\s\S]{0,160}min-height: 300px/.test(css), '正文编辑区要够大')
  // 编辑现有模板要有明确入口：点列表项载入、保存覆盖、另存为新。
  assert.ok(/function editTemplate/.test(settings) && /editingTemplateId/.test(settings), '点列表项要载入编辑')
  assert.ok(/保存修改/.test(settings) && /另存为新模板/.test(settings), '覆盖与另存要分开')
  assert.ok(/saveTemplateFn/.test(settings) && /:save-template-fn="store\.saveTemplate"/.test(mailApp), '保存要拿回结果（函数 prop）')
  assert.ok(/removeCurrentTemplate/.test(settings) && /askMailConfirm/.test(settings), '删模板走应用内确认')
})

test('弹层：变大了，而且不会点外面/拖一下就消失', () => {
  assert.ok(/\.mail-modal \{[\s\S]{0,120}width: min\(820px/.test(css), '默认弹层加宽')
  assert.ok(/\.mail-modal\.is-wide \{[\s\S]{0,80}width: min\(1120px/.test(css), '宽弹层加宽')
  assert.ok(/\.mail-settings-modal \{[\s\S]{0,200}width: min\(1040px/.test(css), '设置窗口加宽')
  // 表单类弹层：点外面不再关闭（会丢正在填的内容）。
  for (const file of ['MailSettingsDrawer.vue', 'MailRulesDialog.vue', 'ContactsDialog.vue', 'MailAccountDialog.vue']) {
    const src = read(mailDir + file)
    assert.ok(!/mail-modal-backdrop"[^>]*@click\.self/.test(src), `${file} 不该点外面就关`)
  }
  // 只读类弹层：改成 mousedown 判定，从里往外拖选文字松手也不会误关。
  assert.ok(/@mousedown\.self="\$emit\('close'\)"/.test(read(mailDir + 'MailEmlDialog.vue')), '.eml 查看用 mousedown 判定')
})
