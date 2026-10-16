import { createLogger } from '../utils/logger'

// 代码配色主题的唯一真相源。两条消费链共用这里的数据，保证「编辑器」和「Markdown 渲染出来的代码块」观感一致：
//   1. 编辑器侧 —— Monaco 的 rules / semanticTokenColors，以及编辑器和探索者 cell 里的
//      `.md-fence-token-*` / `.nb-python-token-*` 装饰色（它们只读 `--guaxi-syntax-*` CSS 变量）。
//   2. Markdown 侧 —— 正文代码块由渲染引擎产出 `.hljs-*` 类名，颜色来自注入进 <style> 的上游样式表。
//
// 做法上刻意不动渲染引擎：它只负责产出类名，颜色来自哪张样式表与它无关。
// 同一时刻只注入当前主题的那一张，因此不存在多张主题互相覆盖的问题，配色也能像素级等于上游主题。

import atomOneLightCss from 'highlight.js/styles/atom-one-light.css?inline'
import atomOneDarkCss from 'highlight.js/styles/atom-one-dark.css?inline'
import githubLightCss from 'highlight.js/styles/github.css?inline'
import githubDarkCss from 'highlight.js/styles/github-dark.css?inline'
import pandaLightCss from 'highlight.js/styles/panda-syntax-light.css?inline'
import pandaDarkCss from 'highlight.js/styles/panda-syntax-dark.css?inline'
import tokyoNightLightCss from 'highlight.js/styles/tokyo-night-light.css?inline'
import tokyoNightDarkCss from 'highlight.js/styles/tokyo-night-dark.css?inline'
import stackoverflowLightCss from 'highlight.js/styles/stackoverflow-light.css?inline'
import stackoverflowDarkCss from 'highlight.js/styles/stackoverflow-dark.css?inline'
import rosePineDawnCss from 'highlight.js/styles/rose-pine-dawn.css?inline'
import rosePineCss from 'highlight.js/styles/rose-pine.css?inline'

const logger = createLogger('codeThemes')

export const DEFAULT_CODE_THEME_ID = 'atom-one'

// 编辑器侧的语义调色板**从上游样式表里解析出来**，而不是另外手写一份。
// 手写过一版：结果必然和上游有偏差，而 Markdown 代码块用的正是上游原样样式表，
// 于是「编辑器里的代码块」和「正文里的代码块」对同一类语法元素显示不同颜色——那正是「不统一」的来源。
// 现在两边同源：Markdown 直接用这份 CSS，编辑器从同一份 CSS 里取色。
const HLJS_TOKEN_SOURCES = [
  ['base', ['hljs']],
  ['comment', ['hljs-comment', 'hljs-quote']],
  ['string', ['hljs-string']],
  ['keyword', ['hljs-keyword', 'hljs-doctag', 'hljs-formula']],
  ['number', ['hljs-number', 'hljs-literal']],
  ['boolean', ['hljs-literal']],
  ['regexp', ['hljs-regexp']],
  ['operator', ['hljs-operator', 'hljs-symbol', 'hljs-bullet']],
  ['punctuation', ['hljs-punctuation']],
  ['type', ['hljs-title.class_', 'hljs-type']],
  ['class', ['hljs-title.class_', 'hljs-type']],
  ['function', ['hljs-title.function_', 'hljs-title']],
  ['method', ['hljs-title.function_', 'hljs-title']],
  ['property', ['hljs-attr', 'hljs-attribute']],
  ['parameter', ['hljs-variable', 'hljs-template-variable']],
  ['variable', ['hljs-variable', 'hljs-template-variable']],
  ['constant', ['hljs-literal', 'hljs-number']],
  ['namespace', ['hljs-type']],
  ['macro', ['hljs-meta']],
  ['decorator', ['hljs-meta']],
  ['tag', ['hljs-name', 'hljs-selector-tag', 'hljs-tag']],
  ['attribute', ['hljs-attr', 'hljs-attribute']],
  ['builtin', ['hljs-built_in']],
  ['label', ['hljs-keyword']],
]

// 上游样式表没写到的类型，按语义就近取；顺序即依赖顺序。
const TOKEN_FALLBACKS = [
  ['boolean', 'number'],
  ['regexp', 'string'],
  ['punctuation', 'base'],
  ['parameter', 'variable'],
  ['method', 'function'],
  ['property', 'method'],
  ['class', 'type'],
  ['namespace', 'type'],
  ['macro', 'keyword'],
  ['decorator', 'keyword'],
  ['attribute', 'number'],
  ['builtin', 'function'],
  ['label', 'keyword'],
]

// 只认 `color:` 这条声明的值。不能取整段里的第一个十六进制色：
// 部分上游主题把 background 写在 color 前面，例如 tokyo-night-light 的
// `.hljs { background: #d5d6db; color: #565a6e }`——取第一个就会把**背景色**当成文字色，
// 结果是在浅色底上整行看不见（rose-pine-dawn 同理）。
const readDeclaredColor = (body) => {
  for (const declaration of String(body || '').split(';')) {
    const colon = declaration.indexOf(':')
    if (colon < 0) continue
    // 精确匹配 color，排除 background-color / border-color 这类
    if (declaration.slice(0, colon).trim().toLowerCase() !== 'color') continue

    const value = declaration.slice(colon + 1)
    const hash = value.indexOf('#')
    if (hash < 0) continue
    let hex = ''
    for (let i = hash + 1; i < value.length && i < hash + 10; i += 1) {
      const ch = value[i]
      if (/[0-9a-fA-F]/.test(ch)) hex += ch
      else break
    }
    if (hex) return `#${hex.toLowerCase()}`
  }
  return ''
}

// 把样式表拆成 [选择器, 颜色] 列表。只取带 color 声明的普通规则，注释与 at-rule 跳过。
const parseHljsRules = (css) => {
  const rules = []
  for (const chunk of String(css || '').replace(/\/\*[\s\S]*?\*\//g, '').split('}')) {
    const brace = chunk.indexOf('{')
    if (brace < 0) continue
    const selectors = chunk.slice(0, brace).split(',').map((item) => item.trim())
      .filter((item) => item && !item.startsWith('@'))
    if (!selectors.length) continue

    const color = readDeclaredColor(chunk.slice(brace + 1))
    if (!color) continue

    for (const selector of selectors) rules.push([selector, color])
  }
  return rules
}

// 选择器是否命中某个 hljs 类名。名字里带点（如 hljs-title.function_）表示复合选择器，要求全部命中。
const selectorMatchesHljsClass = (selector, name) => {
  const wanted = name.split('.').filter(Boolean)
  const present = selector.split('.').filter(Boolean).map((part) => part.split(/[\s:>+~[\]()]/)[0])
  return wanted.every((item) => present.includes(item))
}

// 解析结果按样式表字符串缓存：同一张表只解析一次。
const paletteCache = new Map()

/** 从一张上游样式表导出语义调色板；上游没定义的类由 TOKEN_FALLBACKS 补齐。 */
export const buildPaletteFromHljsCss = (css) => {
  const cacheKey = String(css || '')
  if (paletteCache.has(cacheKey)) return paletteCache.get(cacheKey)

  const rules = parseHljsRules(css)
  const palette = {}
  for (const [token, names] of HLJS_TOKEN_SOURCES) {
    for (const name of names) {
      const hit = rules.find(([selector]) => selectorMatchesHljsClass(selector, name))
      if (hit) {
        palette[token] = hit[1]
        break
      }
    }
  }
  for (const [target, source] of TOKEN_FALLBACKS) {
    if (!palette[target]) palette[target] = palette[source] || palette.base
  }

  paletteCache.set(cacheKey, palette)
  return palette
}

// 六个家族。每套只登记两条上游样式表——编辑器调色板由它解析得出（见 buildPaletteFromHljsCss），
// 不再维护第二份手写色值，这样「编辑器里的代码块」与「Markdown 里的代码块」必然同源同色。
// 这些样式表都随 highlight.js 依赖离线存在，无需联网、无需新增依赖。
export const GUAXI_CODE_THEMES = [
  { id: "atom-one", label: "Atom One", hljs: { light: atomOneLightCss, dark: atomOneDarkCss } },
  { id: "github", label: "GitHub", hljs: { light: githubLightCss, dark: githubDarkCss } },
  { id: "panda", label: "Panda", hljs: { light: pandaLightCss, dark: pandaDarkCss } },
  { id: "tokyo-night", label: "Tokyo Night", hljs: { light: tokyoNightLightCss, dark: tokyoNightDarkCss } },
  { id: "stackoverflow", label: "Stack Overflow", hljs: { light: stackoverflowLightCss, dark: stackoverflowDarkCss } },
  { id: "rose-pine", label: "Rosé Pine", hljs: { light: rosePineDawnCss, dark: rosePineCss } },
]

const THEME_BY_ID = new Map(GUAXI_CODE_THEMES.map((theme) => [theme.id, theme]))

export const normalizeCodeThemeId = (id) => {
  const key = String(id || '').trim()
  return THEME_BY_ID.has(key) ? key : DEFAULT_CODE_THEME_ID
}

export const getCodeTheme = (id) => THEME_BY_ID.get(normalizeCodeThemeId(id))

// 「当前激活的主题」状态放在注册表自己这里：Monaco 侧和导出侧都要读它。
let activeCodeThemeId = DEFAULT_CODE_THEME_ID

export const getActiveCodeThemeId = () => activeCodeThemeId

export const setActiveCodeThemeId = (id) => {
  activeCodeThemeId = normalizeCodeThemeId(id)
  return activeCodeThemeId
}

/** 拿到某主题在指定明暗下的语义调色板；由该主题的上游样式表解析得出，明暗缺一就退回另一个。 */
export const getCodeThemePalette = (id, appearanceTheme) => {
  const theme = getCodeTheme(id)
  const wantDark = appearanceTheme === 'dark'
  const css = (wantDark ? theme.hljs.dark : theme.hljs.light)
    || theme.hljs.dark
    || theme.hljs.light
    || ''
  return buildPaletteFromHljsCss(css)
}

export const normalizeAppearance = (theme) => (theme === 'dark' ? 'dark' : 'light')

// ───── DOM 注入 ─────
// `--guaxi-syntax-*` 变量块一次把所有家族都写进去，用 [data-guaxi-code-theme] 选择哪个生效，
// 这样切换主题时变量不需要重新注入；只有 hljs 那张样式表需要换。
const SYNTAX_VARIABLE_STYLE_ID = 'guaxi-code-theme-vars'
const HLJS_STYLE_ID = 'guaxi-code-theme-hljs'

const buildSyntaxVariableCss = () => GUAXI_CODE_THEMES.flatMap((theme) => (
  ['light', 'dark'].map((appearance) => {
    const palette = getCodeThemePalette(theme.id, appearance)
    const body = Object.entries(palette)
      .map(([token, color]) => `  --guaxi-syntax-${token}: ${color};`)
      .join('\n')
    // 特异性 (0,3,0)，稳压 App.vue 里 `:root` 与 `:root[data-guaxi-theme='dark']` 的既有定义。
    return `:root[data-guaxi-code-theme='${theme.id}'][data-guaxi-theme='${appearance}'] {\n${body}\n}`
  })
)).join('\n')

const upsertStyleElement = (id, css) => {
  if (typeof document === 'undefined') return
  let element = document.getElementById(id)
  if (!element) {
    element = document.createElement('style')
    element.id = id
    // 追加到 <head> 末尾：与打包进 bundle 的 atom-one-light 兜底样式表特异性相同时靠顺序取胜。
    document.head.appendChild(element)
  }
  if (element.textContent !== css) element.textContent = css
}

/**
 * 把代码主题落到文档上。幂等，可重复调用。
 * 只改「颜色从哪来」，不改任何渲染产出的 DOM 结构与类名。
 */
export const applyCodeThemeToDocument = (id, appearanceTheme) => {
  if (typeof document === 'undefined') return

  const themeId = normalizeCodeThemeId(id)
  const appearance = normalizeAppearance(appearanceTheme)
  const theme = getCodeTheme(themeId)

  document.documentElement.dataset.guaxiCodeTheme = themeId
  upsertStyleElement(SYNTAX_VARIABLE_STYLE_ID, buildSyntaxVariableCss())
  upsertStyleElement(HLJS_STYLE_ID, theme.hljs[appearance] || theme.hljs.light || '')

  logger.info('[CodeTheme] applied', { themeId, appearance })
}
