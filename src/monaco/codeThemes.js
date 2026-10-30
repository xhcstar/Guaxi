import { createLogger } from '../utils/logger'
import { buildSyntaxTokenCss, resolveSyntaxPalette, stripThemeColors } from './syntaxPalette'
import { GUAXI_CODE_THEME_ROLES } from './codeThemeRoles'

// 代码配色主题的唯一真相源。所有入口共用同一份语义色板：
//   1. 编辑器侧 —— Monaco 的 rules / semanticTokenColors，以及编辑器和探索者 cell 里的
//      `.md-fence-token-*` / `.nb-python-token-*` 装饰色（它们只读 `--guaxi-syntax-*` CSS 变量）。
//   2. Markdown 侧 —— 正文代码块由渲染引擎产出 `.hljs-*` 类名，颜色规则由同一份色板生成。
//
// 上游 highlight.js 样式表现在只提供「非颜色」部分（背景、内边距、斜体），
// 颜色一律来自色板派生。原先直接注入上游样式表时，主题里有几个色就只能用几个色，
// 参数、变量、类成员只能撞同一个橙色。

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
// 上游 hljs 没有 gruvbox，Monokai 只有暗色，这两套是本地补的（见 codeStyles/）。
import monokaiLightCss from './codeStyles/monokai-light.css?inline'
import monokaiDarkCss from './codeStyles/monokai-dark.css?inline'
import gruvboxLightCss from './codeStyles/gruvbox-light.css?inline'
import gruvboxDarkCss from './codeStyles/gruvbox-dark.css?inline'

const logger = createLogger('codeThemes')

export const DEFAULT_CODE_THEME_ID = 'atom-one'

// 语义色板由 syntaxPalette 解析：先从上游主题取出 9 个角色色，
// 再按「角色 + 明度/微色相档位」展开成 27 个语义桶。
// 预览侧 `.hljs-*` 的颜色规则由同一份色板生成（见 buildSyntaxTokenCss），
// 上游样式表只保留背景、内边距这类非颜色声明，避免两套颜色并存。
// 八个家族。多数主题只登记两条上游样式表——角色色由它解析得出（见 syntaxPalette.resolveSyntaxPalette），
// 不再维护第二份手写色值；Monokai / Gruvbox 例外，角色色在 codeThemeRoles.js 里显式给出。
// 这些样式表都随 highlight.js 依赖离线存在，无需联网、无需新增依赖。
export const GUAXI_CODE_THEMES = [
  { id: "atom-one", label: "Atom One", hljs: { light: atomOneLightCss, dark: atomOneDarkCss } },
  { id: "github", label: "GitHub", hljs: { light: githubLightCss, dark: githubDarkCss } },
  { id: "panda", label: "Panda", hljs: { light: pandaLightCss, dark: pandaDarkCss } },
  { id: "tokyo-night", label: "Tokyo Night", hljs: { light: tokyoNightLightCss, dark: tokyoNightDarkCss } },
  { id: "stackoverflow", label: "Stack Overflow", hljs: { light: stackoverflowLightCss, dark: stackoverflowDarkCss } },
  { id: "rose-pine", label: "Rosé Pine", hljs: { light: rosePineDawnCss, dark: rosePineCss } },
  // 这两套是人工挑色的主题：hljs 类名和语义角色对不上，角色色见 codeThemeRoles.js。
  {
    id: "monokai",
    label: "Monokai",
    hljs: { light: monokaiLightCss, dark: monokaiDarkCss },
    roles: GUAXI_CODE_THEME_ROLES.monokai,
  },
  {
    id: "gruvbox",
    label: "Gruvbox",
    hljs: { light: gruvboxLightCss, dark: gruvboxDarkCss },
    roles: GUAXI_CODE_THEME_ROLES.gruvbox,
  },
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
  return resolveSyntaxPalette(css, wantDark ? 'dark' : 'light', theme.roles?.[wantDark ? 'dark' : 'light'])
}

export const normalizeAppearance = (theme) => (theme === 'dark' ? 'dark' : 'light')

// ───── DOM 注入 ─────
// `--guaxi-syntax-*` 变量块一次把所有家族都写进去，用 [data-guaxi-code-theme] 选择哪个生效，
// 这样切换主题时变量不需要重新注入；只有 hljs 那张样式表需要换。
const SYNTAX_VARIABLE_STYLE_ID = 'guaxi-code-theme-vars'
const HLJS_STYLE_ID = 'guaxi-code-theme-hljs'
const TOKEN_STYLE_ID = 'guaxi-code-theme-tokens'

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
  // 上游样式表剥掉 color 声明后再注入：底色/内边距/斜体沿用主题，颜色交给下面的 token 规则。
  upsertStyleElement(HLJS_STYLE_ID, stripThemeColors(theme.hljs[appearance] || theme.hljs.light || ''))
  upsertStyleElement(TOKEN_STYLE_ID, buildSyntaxTokenCss())

  logger.info('[CodeTheme] applied', { themeId, appearance })
}
