/*
 * 挡住"release 包里出现空白图标"这一类问题。
 *
 * @iconify/vue 的 <Icon icon="mdi:xxx" /> 按名字取图标：名字在本地注册表里就直接画，
 * 不在就发 HTTP 去 api.iconify.design 现取。开发机上网所以看不出问题，装到断网、
 * 内网或者 CSP 收紧的机器上，那个图标就是一片空白。
 *
 * 所以约定两条路，二者都不碰网络：
 *   1. 名字：在 src/main.js 的 iconRegistry 里 addIcon 注册；
 *   2. 对象：直接 import 图标数据传给 :icon（只在一两处用的图标走这条更省事，
 *      名字写错在构建期就报"模块找不到"）。
 *
 * 这个脚本扫 src/ 下所有源码里写死的图标名字，凡是没注册的就报错退出。
 * 用法：npm run check:icons
 */
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

const ROOT = new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1')
const SRC = join(ROOT, 'src')
const MAIN = join(SRC, 'main.js')
const EXTS = new Set(['.vue', '.js', '.mjs', '.ts'])

// 注册表里的名字。取 `'prefix:name': someIcon,` 这种行，够了——iconRegistry 就是这个形状。
const readRegistry = () => {
  const source = readFileSync(MAIN, 'utf8')
  const block = source.match(/const iconRegistry = \{([\s\S]*?)\n\};/)
  if (!block) {
    console.error(`FAIL 没在 ${relative(ROOT, MAIN)} 里找到 iconRegistry，脚本的假设过期了`)
    process.exit(2)
  }
  const names = new Set()
  for (const m of block[1].matchAll(/['"]([a-z0-9][a-z0-9-]*:[a-z0-9][a-z0-9-]*)['"]\s*:/g)) {
    names.add(m[1])
  }
  return names
}

const walk = (dir, out = []) => {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry)
    if (statSync(full).isDirectory()) walk(full, out)
    else if (EXTS.has(entry.slice(entry.lastIndexOf('.')))) out.push(full)
  }
  return out
}

/*
 * 两条规则并集：
 *  - 已知图标集前缀的字面量（'mdi:xxx'）。文件类型图标是 `js: 'vscode-icons:file-type-js'`
 *    这种映射表，值不在 icon: 后面，只能靠前缀认。
 *  - icon: / icon= 后面带冒号的字面量。这条兜住新前缀（比如哪天引入 'ph:'）。
 * 排除注释行：注释里会举例说明写法，不该被当成真用了。
 */
const KNOWN_PREFIXES = ['mdi', 'lucide', 'vscode-icons']
const BY_PREFIX = new RegExp(`['"](?:${KNOWN_PREFIXES.join('|')}):[a-z0-9][a-z0-9-]*['"]`, 'g')
const BY_CONTEXT = /\bicon\s*[:=]\s*['"]([a-z0-9][a-z0-9-]*:[a-z0-9][a-z0-9-]*)['"]/g
const COMMENT = /^\s*(\/\/|\*|\/\*|<!--)/

const collectUsages = (files) => {
  const usages = []
  for (const file of files) {
    if (file === MAIN) continue
    const lines = readFileSync(file, 'utf8').split(/\r?\n/)
    lines.forEach((line, i) => {
      if (COMMENT.test(line)) return
      const found = new Set()
      for (const m of line.matchAll(BY_PREFIX)) found.add(m[0].slice(1, -1))
      for (const m of line.matchAll(BY_CONTEXT)) found.add(m[1])
      for (const name of found) usages.push({ name, file, line: i + 1 })
    })
  }
  return usages
}

const registered = readRegistry()
const usages = collectUsages(walk(SRC))
const missing = usages.filter((u) => !registered.has(u.name))

for (const u of missing) {
  console.log(`FAIL ${relative(ROOT, u.file).replace(/\\/g, '/')}:${u.line}  ${u.name} 没有离线注册`)
}

if (missing.length) {
  console.log('')
  console.log(`${missing.length} 个图标名字会走 api.iconify.design，断网时渲染成空白。两种改法：`)
  console.log('  · 在 src/main.js 的 iconRegistry 里补上（import 图标 + 加一行映射）')
  console.log("  · 或者改成 import 图标对象直接传给 :icon（只用一两处时更省事）")
  process.exit(1)
}

console.log(`ALL OK  ${usages.length} 处图标名字全部离线注册（注册表 ${registered.size} 个）`)
