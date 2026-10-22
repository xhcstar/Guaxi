/*
 * 找出「把已经是响应式数组/对象的值又加了 .value」这类错误。
 *
 * 为什么需要这个脚本：`reactive([...])` 返回的是数组本身，`pages.value` 恒为
 * undefined。这种写法**编译期完全静默**，只在运行时抛
 * "Cannot read properties of undefined (reading 'length')" ——
 * PDF 阅读器就踩过一次，排查时只能靠浏览器控制台的堆栈。
 *
 * 规则故意保守：只报「明确用 reactive() 声明、且同文件里没有同名 ref()」的名字，
 * 避免把 `foo.value` 里的普通对象（比如 DOM 元素的 value 属性）误报成问题。
 *
 * 用法：npm run check:refs
 */
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

const ROOT = new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1')
const SRC = join(ROOT, 'src')
const EXTS = new Set(['.vue', '.js', '.mjs', '.ts'])

const walk = (dir, out = []) => {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry)
    if (statSync(full).isDirectory()) walk(full, out)
    else if (EXTS.has(full.slice(full.lastIndexOf('.')))) out.push(full)
  }
  return out
}

/** 只看 <script setup> 里的代码，避免把模板中的同名文本当成声明。 */
const scriptOf = (source) => {
  const m = source.match(/<script setup>([\s\S]*?)<\/script>/)
  return m ? m[1] : source
}

const collect = (script, kind) => {
  const names = new Set()
  const re = new RegExp(`\\bconst\\s+(\\w+)\\s*=\\s*(?:shallow)?${kind}\\s*\\(`, 'g')
  for (const m of script.matchAll(re)) names.add(m[1])
  return names
}

let problems = 0
const files = walk(SRC)

for (const file of files) {
  const script = scriptOf(readFileSync(file, 'utf8'))
  // <template> 里的 @click="foo.value" 也算：模板同样会炸。
  const full = readFileSync(file, 'utf8')

  const reactiveNames = collect(script, 'reactive')
  const refNames = collect(script, 'ref')

  for (const name of reactiveNames) {
    if (refNames.has(name)) continue // 同名既 ref 又 reactive：先出声明的那份说了算，跳过
    const re = new RegExp(`\\b${name}\\.value\\b`, 'g')
    const hits = [...full.matchAll(re)]
    if (!hits.length) continue
    console.log(`FAIL ${relative(ROOT, file)}: reactive(${name}) 被当成 ref 用（${hits.length} 处 ${name}.value）`)
    problems += hits.length
  }
}

if (problems) {
  console.error(`\n共 ${problems} 处可疑用法。reactive() 的返回值直接用，不要加 .value。`)
  process.exit(1)
}
console.log(`ALL OK  ${files.length} 个文件，未发现 reactive/ref 混用`)
