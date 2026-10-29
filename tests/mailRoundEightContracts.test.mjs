import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第八轮反馈的源码契约：
//   1. 表格样式 / 宽度 / 颜色不再平铺，工具用图标 + 提示；
//   2. 收到的邮件里 align=center 的表格要真的居中（Outlook 那种）；
//   3. 工具栏要有格式刷；
//   4. 检索不能报 `unable to use function MATCH in the requested context`。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const composer = read('src/components/ApplicationViewComponent/apps/mail/MailComposer.vue')
const css = read('src/components/ApplicationViewComponent/apps/mail/mail.css')
const storeRs = read('src-tauri/src/commands/mail/store.rs')
/** 断言只看代码，不看注释（注释里会为了解释 bug 而复述出错的写法）。 */
const withoutComments = (text) => text.replace(/^\s*\/\/.*$/gm, '').replace(/\/\*[\s\S]*?\*\//g, '')

test('检索：MATCH 不能出现在 OR 条件里，也不能写成 column MATCH', () => {
  // 两条都是硬错误（已用真实 mail.db 复现）：
  //   `MATCH ... OR <普通谓词>` → unable to use function MATCH in the requested context
  //   `messages_fts.body MATCH ?` → no such column: messages_fts.body
  assert.ok(!/JOIN messages_fts f ON f\.rowid/.test(storeRs), '不要再 JOIN FTS 表（MATCH 会落进不支持的上下文）')
  assert.ok(
    /m\.rowid IN \(SELECT rowid FROM messages_fts WHERE messages_fts MATCH \?\)/.test(storeRs),
    'FTS 命中改成子查询过滤',
  )
  assert.ok(/fn fts_query_in\(text: &str, column: Option<&str>\)/.test(storeRs), '正文搜索要用列过滤语法')
  assert.ok(
    /fts_query_in\(&query\.text, Some\("body"\)\)/.test(storeRs),
    '按正文搜时传 body 列',
  )
  assert.ok(!/messages_fts\.body MATCH/.test(withoutComments(storeRs)), '列过滤不能写成 column MATCH')
})

test('收信正文：align=center 的表格要居中，不被嵌套规则抹掉', () => {
  const nested = withoutComments(
    css.slice(css.indexOf('.mail-body table table,'), css.indexOf('.mail-body table[align')),
  )
  assert.ok(!/margin:\s*0/.test(nested), '嵌套表格不能整体清零外边距（会吃掉居中）')
  assert.ok(/margin-top:\s*0/.test(nested) && /margin-bottom:\s*0/.test(nested), '只收上下外边距')
  assert.ok(/\.mail-body table\[align='center'\]\s*\{[\s\S]{0,120}margin-left:\s*auto/.test(css), '显式把 align=center 映射成自动外边距')
})

test('格式刷：工具栏有入口，能复制当前格式再刷到选区', () => {
  assert.ok(/name="formatPainter"/.test(composer), '工具栏要有格式刷按钮')
  assert.ok(/const formatPainter = ref\(null\)/.test(composer), '要有「已复制格式」的状态')
  assert.ok(/function captureFormat/.test(composer) && /getComputedStyle/.test(composer), '从光标处读取实际生效的样式')
  assert.ok(/function paintFormat/.test(composer), '要把格式套到新区间上')
  assert.ok(/formatPainter\.value && paintFormat\(\)/.test(composer), '下次松开鼠标就刷上去')
  assert.ok(/formatPainter\.value = null/.test(composer), '刷完 / Esc 要能取消')
  assert.ok(/mail-fmt-hint/.test(composer) && /mail-fmt-hint/.test(css), '待刷状态要有可见提示')
})

test('编辑器工具：枚举收进下拉、颜色用取色器、浮动条用图标', () => {
  assert.ok(/mail-editor-menu-select/.test(composer), '表格样式 / 宽度 / 字号要有下拉')
  assert.ok(/input type="color"/.test(composer), '填充与文字颜色用取色器')
  assert.ok(!/v-for="color in CELL_FILLS"/.test(composer), '不再平铺色板')
  // 浮动条上的表格操作改成图标 + title。
  assert.ok(/v-for="item in TABLE_ROW_ACTIONS"[\s\S]{0,260}MailIcon :name="item\.icon"/.test(composer), '浮动条用图标')
  assert.ok(
    /{ action: 'rowAbove', icon: 'rowAbove'/.test(composer),
    '行列动作要带图标名与提示',
  )
})

test('图片：可拖边框改大小、可拖动换位置', () => {
  assert.ok(/class="mail-image-frame"/.test(composer), '选中图片要有控制框')
  assert.ok(/function onFrameMouseMove[\s\S]{0,300}setImageWidth\([\s\S]{0,80}commit: false/.test(composer), '拖框时实时改宽度')
  assert.ok(/function onFrameMouseUp[\s\S]{0,220}commitImageWidth/.test(composer), '松手后写回草稿')
  assert.ok(/function onEditorMouseDown/.test(composer) && /pendingImageMove/.test(composer), '按住图片可以拖动换位置')
  assert.ok(/insertAdjacentElement/.test(composer), '换位置要做真实的 DOM 移动，不是只改对齐')
  assert.ok(/\.mail-image-frame\s*\{[\s\S]{0,120}pointer-events: none/.test(css), '控制框不能挡住图片本身的拖动')
})
