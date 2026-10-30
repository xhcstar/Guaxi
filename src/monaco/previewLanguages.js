// 预览代码块用哪种语言向后端请求 tree-sitter token。
//
// 单独放一个纯模块：`previewTokenHighlight` 里有 DOM/Tauri 依赖，跑不了单测，
// 而语言别名解析是最容易写错、又最容易被改坏的一段。

// 与后端 HIGHLIGHT_CONFIGS 支持的语言保持一致；不在表里的语言继续用 highlight.js 的颜色。
export const PREVIEW_TOKEN_LANGUAGES = new Set([
  'markdown',
  'javascript',
  'typescript',
  'tsx',
  'python',
  'rust',
  'bash',
  'lua',
  'java',
  'html',
  'c',
  'cpp',
  'latex',
])

const LANGUAGE_ALIASES = {
  md: 'markdown',
  mdown: 'markdown',
  js: 'javascript',
  jsx: 'javascript',
  mjs: 'javascript',
  cjs: 'javascript',
  ts: 'typescript',
  py: 'python',
  python3: 'python',
  rs: 'rust',
  sh: 'bash',
  shell: 'bash',
  zsh: 'bash',
  powershell: 'bash',
  ps1: 'bash',
  tex: 'latex',
  'c++': 'cpp',
  cc: 'cpp',
  cxx: 'cpp',
  hpp: 'cpp',
  hxx: 'cpp',
  htm: 'html',
  xml: 'html',
  vue: 'html',
}

export const normalizeLanguageId = (raw) => {
  const token = String(raw || '').trim().toLowerCase().split(/\s+/)[0]
  if (!token) return ''
  return LANGUAGE_ALIASES[token] || token
}

/** 从 `<code class="language-xxx">` 的 class 串解析出后端认识的语言 id；不支持的语言返回空串。 */
export const resolvePreviewLanguage = (className) => {
  const match = /language-([\w+#-]+)/.exec(String(className || ''))
  if (!match) return ''
  const language = normalizeLanguageId(match[1])
  return PREVIEW_TOKEN_LANGUAGES.has(language) ? language : ''
}
