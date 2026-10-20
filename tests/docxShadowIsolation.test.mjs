import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

// 回归测试：Word 阅读器必须把 docx 样式限制在 Shadow DOM 内。
//
// 背景（真实事故）：docx-preview 生成的是 `p.MsoNormal`、`tr.first-row td` 这类
// **无前缀的全局选择器**。把这些样式注入 document.head 后，打开 Word 文档会把整个
// 应用改形（实测是主界面被一个巨大黑块覆盖）。
//
// 这个测试直接检查组件源码的结构约束，因为样式隔离是「写在代码结构里」的事实，
// 不需要跑浏览器也能验证。一旦有人把 styleHost 改回 head / document，测试立刻失败。

const SOURCE = readFileSync(
  new URL('../src/components/ReaderComponent/readers/DocDocxReader.vue', import.meta.url),
  'utf8',
)

test('docx 样式容器创建在 Shadow DOM 内，而不是页面 head', () => {
  assert.match(SOURCE, /attachShadow\(\{\s*mode:\s*'open'\s*\}\)/, '必须使用 attachShadow')
  assert.match(SOURCE, /const styleHost = document\.createElement\('div'\)/, '样式容器应是影子树里的 div')
  assert.match(SOURCE, /root\.appendChild\(styleHost\)/, '样式容器必须挂进影子根')
})

test('不得再把样式容器指向 document.head（事故根因）', () => {
  assert.doesNotMatch(SOURCE, /document\.head\s*;?\s*$/m, '不能把 head 作为样式容器')
  assert.doesNotMatch(SOURCE, /renderAsync\([^)]*,\s*document\.head\s*,/, 'renderAsync 的第三个参数不能是 document.head')
  assert.doesNotMatch(SOURCE, /renderAsync\([^)]*,\s*undefined\s*,/, '也不能传 undefined（会退回正文容器）')
})

test('renderAsync 的正文与样式容器都来自影子根', () => {
  const call = SOURCE.match(/renderAsync\(([\s\S]*?)\)\s*\n/)
  assert.ok(call, '应能定位 renderAsync 调用')
  const args = call[1]
  assert.match(args, /body/, '第二个参数应为影子树内的 body 容器')
  assert.match(args, /styleHost/, '第三个参数应为影子树内的 styleHost')
})
