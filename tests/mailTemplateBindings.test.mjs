import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse, compileScript } from '@vue/compiler-sfc'

// 模板里引用了 `<script setup>` 里并不存在的变量时，编译器不会报错：
// 它把名字当成「运行时再解析」的绑定，直到渲染那一刻才抛 TypeError。
//
// 真实事故：`MailRulesDialog.vue` 的模板用了 `senderRules` / `senderDraft` /
// `addSenderRule`，脚本里一个都没有。打开「规则」弹层时读 `item.kind` 抛错，
// Vue 的渲染中断会连带把整个页面（含设置页里的主题卡片）渲染成空白，
// 用户看到的现象是「主题配置模块丢失」。
//
// 这里用 compileScript 的绑定表做静态核对：模板里用到的每个标识符，
// 要么来自脚本绑定，要么是 v-for / 插槽引入的局部名字，要么是全局白名单。

const here = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.join(here, '..')
const mailDir = path.join(repoRoot, 'src/components/ApplicationViewComponent/apps/mail')

/** 模板里允许出现、但不来自脚本的全局名字。 */
const GLOBALS = new Set([
  'true', 'false', 'null', 'undefined', 'NaN', 'Infinity',
  'Math', 'Date', 'JSON', 'Number', 'String', 'Boolean', 'Array', 'Object',
  'Promise', 'Set', 'Map', 'RegExp', 'Error', 'console', 'window', 'document',
  'navigator', 'Intl', 'parseInt', 'parseFloat', 'isNaN', 'isFinite',
  'encodeURIComponent', 'decodeURIComponent', 'setTimeout', 'clearTimeout',
  'setInterval', 'clearInterval', 'requestAnimationFrame', '$emit', '$props',
  '$attrs', '$slots', '$refs', '$event', '$el', 'URL', 'URLSearchParams',
  'Blob', 'File', 'FileReader', 'FormData', 'structuredClone', 'crypto',
])

/** 表达式里出现的根标识符（成员访问的后半段不算）。 */
function rootIdentifiers(expression) {
  // 先剥掉表达式内的字符串字面量：`$emit('open')`、`{ color: 'red' }` 里的词
  // 是数据，不是标识符。
  const stripped = expression
    .replace(/"(?:[^"\\]|\\.)*"/g, '""')
    .replace(/'(?:[^'\\]|\\.)*'/g, "''")
    .replace(/`(?:[^`\\]|\\.)*`/g, '``')
    // 对象字面量的键（`{ left: ... }`、`{ attachment: att }`）也不是标识符引用，
    // 只保留它后面的值。
    .replace(/(?<=[{,])\s*([A-Za-z_$][\w$]*)\s*:(?!:)/g, ' ')
    // 内联箭头函数只剥掉形参声明；函数体里的引用仍然要检查。
    .replace(/\(([\w$,\s]*)\)\s*=>/g, '() =>')
    .replace(/(?<![.\w$])([A-Za-z_$][\w$]*)\s*=>/g, '() =>')
  const names = new Set()
  for (const match of stripped.matchAll(/(?<![.\w$])([A-Za-z_$][\w$]*)/g)) {
    names.add(match[1])
  }
  return names
}

/**
 * 从模板里取出「需要从脚本解析的名字」。
 *
 * 只认这几种位置：
 * - `{{ ... }}` 插值
 * - 指令 / 绑定的值：`v-if`、`v-for` 的右侧、`v-model`、`:prop`、`@event`
 *
 * 静态文本、给 DOM 的字符串字面量（`style="..."` 里的 CSS 属性名）都不算。
 */
function templateIdentifiers(template) {
  const withoutComments = template.replace(/<!--[\s\S]*?-->/g, ' ')
  const out = new Set()
  const expressions = []

  for (const match of withoutComments.matchAll(/\{\{([\s\S]*?)\}\}/g)) {
    expressions.push(match[1])
  }
  for (const match of withoutComments.matchAll(
    /(?:v-[\w:.-]+|[:@][\w:.-]+)="([^"]*)"/g,
  )) {
    // `v-for` 的值要按左右两半拆开看（左边是局部变量），统一放在下面处理。
    if (match[0].startsWith('v-for')) continue
    expressions.push(match[1])
  }
  // `v-for` 的右半边（`in list` 之后）才是要解析的；左边的循环变量是局部名字。
  const localNames = new Set()
  for (const match of withoutComments.matchAll(/v-for="([^"]*?)\s+(?:in|of)\s+([^"]*)"/g)) {
    const lhs = match[1].replace(/[()[\]{}]/g, ' ').replace(/\.\.\./g, ' ')
    for (const name of lhs.split(',')) {
      const clean = name.trim().replace(/^:\s*/, '')
      if (/^[A-Za-z_$][\w$]*$/.test(clean)) localNames.add(clean)
    }
    expressions.push(match[2])
  }
  // 内联箭头函数的形参（`(v) => $emit('x', v)`）与插槽参数一样是局部名字。
  for (const match of withoutComments.matchAll(/\(([\w$,\s]*)\)\s*=>/g)) {
    for (const name of match[1].split(',')) {
      const clean = name.trim()
      if (/^[A-Za-z_$][\w$]*$/.test(clean)) localNames.add(clean)
    }
  }
  for (const match of withoutComments.matchAll(/(?<![.\w$])([A-Za-z_$][\w$]*)\s*=>/g)) {
    localNames.add(match[1])
  }
  // 具名插槽 / 作用域插槽引入的名字
  for (const match of withoutComments.matchAll(/(?:#[\w-]+|v-slot(?::[\w-]+)?)="\{?([^"}]*)\}?"/g)) {
    for (const name of match[1].split(',')) {
      const clean = name.trim().split(':').pop().trim()
      if (/^[A-Za-z_$][\w$]*$/.test(clean)) localNames.add(clean)
    }
  }
  // `v-for` 的局部名字出现在别的表达式里是合法的，先从每个表达式里剔除。
  for (const expr of expressions) {
    for (const name of rootIdentifiers(expr)) {
      if (!localNames.has(name)) out.add(name)
    }
  }
  return { used: out, locals: localNames }
}

function checkFile(file) {
  const source = fs.readFileSync(file, 'utf8')
  const { descriptor, errors } = parse(source, { filename: file })
  assert.deepEqual(errors, [], `${path.basename(file)} 解析失败`)
  if (!descriptor.template) return []
  const compiled = compileScript(descriptor, { id: 'check', inlineTemplate: false })
  const bindings = new Set(Object.keys(compiled.bindings || {}))
  const { used, locals } = templateIdentifiers(descriptor.template.content)
  return [...used].filter(
    (name) =>
      !bindings.has(name) && !locals.has(name) && !GLOBALS.has(name) && !name.startsWith('_'),
  )
}

test('邮件组件模板里不引用脚本中不存在的变量', () => {
  const files = fs.readdirSync(mailDir).filter((f) => f.endsWith('.vue'))
  assert.ok(files.length > 0, '应该找到邮件组件')
  const failures = []
  for (const name of files) {
    const missing = checkFile(path.join(mailDir, name))
    if (missing.length) failures.push(`${name}: ${missing.join(', ')}`)
  }
  assert.deepEqual(failures, [], `模板引用了未定义的变量：\n${failures.join('\n')}`)
})
