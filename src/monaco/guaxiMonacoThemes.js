import {
  buildGuaxiMonacoThemeRules,
  buildGuaxiSemanticTokenColors,
} from './guaxiSyntaxTokens'
import {
  DEFAULT_CODE_THEME_ID,
  getCodeThemePalette,
  setActiveCodeThemeId,
} from './codeThemes'

// 设计说明：
// 1. Monaco 的 theme 名称是全局注册表，同名 `guaxi-dark/guaxi-light` 被后注册者覆盖。
// 2. 编辑器、Git 面板、Slidev 面板和 Notebook cell 都会使用这两个名字，因此语法 token 色板必须来自同一个文件。
// 3. 各入口仍可保留自己的 editor 背景、补全面板等 UI 色值，但 rules 与 semanticTokenColors 不能再分叉。
//
// 代码主题功能：这两个对象改成「可变」的，由 refreshGuaxiMonacoThemePalettes 按当前激活的代码主题就地刷新。
// 消费方读的是 `GUAXI_MONACO_THEME_RULES.light`，所以刷新后再注册即可换色，7 处调用点都不必改签名。

export const GUAXI_MONACO_THEME_RULES = {
  light: buildGuaxiMonacoThemeRules(getCodeThemePalette(DEFAULT_CODE_THEME_ID, 'light')),
  dark: buildGuaxiMonacoThemeRules(getCodeThemePalette(DEFAULT_CODE_THEME_ID, 'dark')),
}

export const GUAXI_MONACO_SEMANTIC_TOKEN_COLORS = {
  light: buildGuaxiSemanticTokenColors(getCodeThemePalette(DEFAULT_CODE_THEME_ID, 'light')),
  dark: buildGuaxiSemanticTokenColors(getCodeThemePalette(DEFAULT_CODE_THEME_ID, 'dark')),
}

// `editor.foreground` 是「没被任何 token 规则命中的字符」的颜色。以前写死 #383a42/#abb2bf，
// 换主题后正文、LaTeX 可选参数、Typst 强调这些落不到规则上的字符就纹丝不动。
export const GUAXI_MONACO_EDITOR_FOREGROUND = {
  light: getCodeThemePalette(DEFAULT_CODE_THEME_ID, 'light').base,
  dark: getCodeThemePalette(DEFAULT_CODE_THEME_ID, 'dark').base,
}

/** 按代码主题 + 明暗重算共享色板（就地替换，保持对象引用不变）。 */
export const refreshGuaxiMonacoThemePalettes = (codeThemeId) => {
  const active = setActiveCodeThemeId(codeThemeId)
  GUAXI_MONACO_THEME_RULES.light = buildGuaxiMonacoThemeRules(getCodeThemePalette(active, 'light'))
  GUAXI_MONACO_THEME_RULES.dark = buildGuaxiMonacoThemeRules(getCodeThemePalette(active, 'dark'))
  GUAXI_MONACO_SEMANTIC_TOKEN_COLORS.light = buildGuaxiSemanticTokenColors(getCodeThemePalette(active, 'light'))
  GUAXI_MONACO_SEMANTIC_TOKEN_COLORS.dark = buildGuaxiSemanticTokenColors(getCodeThemePalette(active, 'dark'))
  GUAXI_MONACO_EDITOR_FOREGROUND.light = getCodeThemePalette(active, 'light').base
  GUAXI_MONACO_EDITOR_FOREGROUND.dark = getCodeThemePalette(active, 'dark').base
  return active
}

// Monaco 的 setTheme 是全局单例（整个 app 共用一份 monaco），
// 所以代码主题变化时只要在这里调一次，所有 Monaco 实例会一起跟着变，不必逐个通知宿主组件。
let lastMonacoRef = null

/**
 * 「代码主题变了，请把你自己的同名主题重新注册一遍」的回调。
 *
 * Monaco 的 defineTheme 是**注册时快照**：上面那两个共享对象后来被就地换新，
 * 已经注册过的主题不会跟着变。以前只有本文件里的 defineGuaxiMonacoThemes 会被重注册，
 * 而编辑器（GuaxiEditor）是**自己**定义 guaxi-light/guaxi-dark 的（UI chrome 色值不同），
 * 于是换主题时编辑器的语法与语义配色纹丝不动——Markdown 正文高亮走的正是语义 token，一起被冻住。
 *
 * 所以谁定义同名主题，谁就在这里登记一下自己的重注册函数。
 */
const codeThemeReapplyListeners = new Set()

/** 登记一个重注册回调，返回反注册函数。 */
export const onGuaxiMonacoCodeThemeReapply = (listener) => {
  if (typeof listener !== 'function') return () => {}
  codeThemeReapplyListeners.add(listener)
  return () => codeThemeReapplyListeners.delete(listener)
}

/** 代码主题变化后：刷新共享色板 → 用同名重新注册 → 让全局主题重新生效。 */
export const reapplyGuaxiMonacoCodeTheme = (codeThemeId, appearanceTheme) => {
  refreshGuaxiMonacoThemePalettes(codeThemeId)
  for (const listener of codeThemeReapplyListeners) {
    try {
      listener(codeThemeId, appearanceTheme)
    } catch (error) {
      console.warn?.('[theme] code theme reapply listener failed', error)
    }
  }
  if (!lastMonacoRef) return
  try {
    defineGuaxiMonacoThemes(lastMonacoRef)
    lastMonacoRef.editor.setTheme(getGuaxiMonacoTheme(appearanceTheme))
  } catch (error) {
    console.warn?.('[theme] reapply code theme failed', error)
  }
}

/**
 * 注册 guaxi-light / guaxi-dark 两个完整主题（含编辑器 UI 色值）。
 *
 * 从 GitPanel 搬过来的：差异对比窗口也要用同一套主题，但它的面板是懒加载 Monaco 的，
 * 不能反过来去 import 一个 .vue 组件。色值必须和 GitPanel 时期逐字一致，
 * 否则同一份主题会被不同入口注册成不同颜色。
 *
 * @param {typeof import('monaco-editor')} monaco
 */
export function defineGuaxiMonacoThemes(monaco) {
  // 记住实例：代码主题切换时要用它重新注册同名主题并让全局主题生效。
  lastMonacoRef = monaco
  monaco.editor.defineTheme('guaxi-light', {
    base: 'vs',
    inherit: true,
    // 设计说明：语法与语义 token 色板统一从共享模块读取，避免 GitPanel 后注册同名主题时覆盖 Notebook/编辑器的 Python 高亮。
    rules: GUAXI_MONACO_THEME_RULES.light,
    semanticHighlighting: true,
    semanticTokenColors: GUAXI_MONACO_SEMANTIC_TOKEN_COLORS.light,
    colors: {
      'editor.background': '#fafafa',
      'editor.foreground': GUAXI_MONACO_EDITOR_FOREGROUND.light,
      'editorLineNumber.foreground': '#a0a1a7',
      'editorLineNumber.activeForeground': '#383a42',
      'editorCursor.foreground': '#4078f2',
      'editor.selectionBackground': '#d7dae0',
      'editor.inactiveSelectionBackground': '#e3f2fd',
      'editor.lineHighlightBackground': '#f0f0f0',
      'editorBracketMatch.background': '#e1f5fe',
      'editorBracketMatch.border': '#90caf9',
      'editor.selectionHighlightBackground': '#ffe0b2',
      'editor.foldBackground': '#f7f9fc',
      'editorWhitespace.foreground': '#d0d0d0',
      'editorIndentGuide.background': '#d0d0d0',
      'editorIndentGuide.activeBackground': '#b0bec5',
      'editorHoverWidget.background': '#ffffff',
      'editorHoverWidget.border': '#e0e0e0',
      'editorSuggestWidget.background': '#ffffff',
      'editorSuggestWidget.border': '#e0e0e0',
      'editorSuggestWidget.foreground': '#383a42',
      'editorSuggestWidget.selectedBackground': '#e3f2fd',
      'editorWidget.background': '#ffffff',
      'editorWidget.border': '#e0e0e0',
      'editorGutter.background': '#fafafa',
      // 设计说明：Git diff 的整行背景保持低透明度，优先保证注释/灰色文本可读；
      // 字符级差异略强一点，便于在浅色主题中精确定位实际变更。
      // 设计说明：保留鲜艳红/绿基色，同时降低 alpha，做出更通透的色膜质感；
      // 字符级背景仍高于整行背景，确保真正变更的位置足够清晰。
      'diffEditor.insertedLineBackground': '#16a34a18',
      'diffEditor.removedLineBackground': '#dc262618',
      'diffEditor.insertedTextBackground': '#22c55e40',
      'diffEditor.removedTextBackground': '#ef444440',
      'scrollbar.shadow': '#00000033'
    }
  })

  monaco.editor.defineTheme('guaxi-dark', {
    base: 'vs-dark',
    inherit: true,
    // 设计说明：暗色 Python 高亮的根因是多个入口重复注册 `guaxi-dark` 且 token 色值不同；这里改为共享色板。
    rules: GUAXI_MONACO_THEME_RULES.dark,
    semanticHighlighting: true,
    semanticTokenColors: GUAXI_MONACO_SEMANTIC_TOKEN_COLORS.dark,
    colors: {
      'editor.background': '#282c34',
      'editor.foreground': GUAXI_MONACO_EDITOR_FOREGROUND.dark,
      'editorLineNumber.foreground': '#5c6370',
      'editorLineNumber.activeForeground': '#abb2bf',
      'editorCursor.foreground': '#61afef',
      'editor.selectionBackground': '#3e4451',
      'editor.inactiveSelectionBackground': '#323842',
      'editor.lineHighlightBackground': '#2c313a',
      'editor.lineHighlightBorder': '#00000000',
      'editorBracketMatch.background': '#3b4048',
      'editorBracketMatch.border': '#528bff',
      'editor.selectionHighlightBackground': '#3e445155',
      'editor.findMatchBackground': '#3b5f8a66',
      'editor.findMatchHighlightBackground': '#3e4451aa',
      'editor.findRangeHighlightBackground': '#3e445155',
      'editor.foldBackground': '#2c313a',
      'editorWhitespace.foreground': '#3b4048',
      'editorIndentGuide.background': '#3b4048',
      'editorIndentGuide.activeBackground': '#4b5263',
      'editorHoverWidget.background': '#282c34',
      'editorHoverWidget.border': '#3b4048',
      'editorSuggestWidget.background': '#282c34',
      'editorSuggestWidget.border': '#3b4048',
      'editorSuggestWidget.foreground': '#abb2bf',
      'editorSuggestWidget.selectedBackground': '#2c313a',
      'editorWidget.background': '#282c34',
      'editorWidget.border': '#3b4048',
      'input.background': '#21252b',
      'input.foreground': '#abb2bf',
      'input.border': '#3b4048',
      'input.placeholderForeground': '#7f8898',
      'focusBorder': '#528bff80',
      'editorGutter.background': '#282c34',
      // 设计说明：暗色 diff 中红/绿背景容易和注释 token 叠加变浑浊；
      // 线级颜色只做轻提示，字符级颜色保留稍高对比，贴近 VS Code 的阅读体验。
      // 设计说明：暗色主题里红/绿更容易显得厚重，因此线级 alpha 更轻；
      // 字符级背景略强一点，兼顾透明质感和变更定位。
      'diffEditor.insertedLineBackground': '#22c55e1c',
      'diffEditor.removedLineBackground': '#ef44441c',
      'diffEditor.insertedTextBackground': '#22c55e46',
      'diffEditor.removedTextBackground': '#ef444446',
      'scrollbar.shadow': '#00000066'
    }
  })
}

/** 主题名按外观设置二选一。 */
export const getGuaxiMonacoTheme = (appearanceTheme) =>
  (appearanceTheme === 'dark' ? 'guaxi-dark' : 'guaxi-light')

/** 按扩展名挑 Monaco 语言；认不出就是 plaintext。 */
export const getLanguageForFile = (path) => {
  const extension = String(path || '').split('.').pop().toLowerCase()
  switch (extension) {
    case 'js': return 'javascript'
    case 'ts': return 'typescript'
    case 'py': return 'python'
    case 'rb': return 'ruby'
    case 'go': return 'go'
    case 'rs': return 'rust'
    case 'java': return 'java'
    case 'c': return 'c'
    case 'cpp': return 'cpp'
    case 'h': return 'cpp'
    case 'hpp': return 'cpp'
    case 'cs': return 'csharp'
    case 'php': return 'php'
    case 'html': return 'html'
    case 'css': return 'css'
    case 'scss': return 'scss'
    case 'less': return 'less'
    case 'json': return 'json'
    case 'xml': return 'xml'
    case 'yml': return 'yaml'
    case 'yaml': return 'yaml'
    case 'md': return 'markdown'
    case 'sh': return 'shell'
    case 'vue': return 'vue'
    default: return 'plaintext'
  }
}
