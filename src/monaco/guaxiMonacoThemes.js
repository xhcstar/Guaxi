import {
  buildGuaxiMonacoThemeRules,
  buildGuaxiSemanticTokenColors,
} from './guaxiSyntaxTokens'

// 设计说明：
// 1. Monaco 的 theme 名称是全局注册表，同名 `guaxi-dark/guaxi-light` 被后注册者覆盖。
// 2. 编辑器、Git 面板、Slidev 面板和 Notebook cell 都会使用这两个名字，因此语法 token 色板必须来自同一个文件。
// 3. 各入口仍可保留自己的 editor 背景、补全面板等 UI 色值，但 rules 与 semanticTokenColors 不能再分叉。

export const GUAXI_MONACO_THEME_RULES = {
  light: buildGuaxiMonacoThemeRules('light'),
  dark: buildGuaxiMonacoThemeRules('dark'),
}

export const GUAXI_MONACO_SEMANTIC_TOKEN_COLORS = {
  light: buildGuaxiSemanticTokenColors('light'),
  dark: buildGuaxiSemanticTokenColors('dark'),
}
