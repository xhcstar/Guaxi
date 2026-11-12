// 按文档语言决定「工具栏显示哪些按钮、每个按钮插入什么语法」。
//
// 为什么单独成模块
//
// 原先工具栏整块写死在模板里，而且所有插入动作最终都过 `ensureMarkdownEditor()`——
// 那个函数看到语言不是 markdown 就直接提示「仅在 Markdown 文件中可用」并放弃。
// 结果是在 .typ / .tex 里，一长排按钮全都在，但按下去要么插入 Markdown 语法
// （对 typst/latex 是废文本），要么干脆什么都不做。
//
// 这里只做**纯描述**：给定语言，返回该语言支持哪些动作、每个动作插入什么。
// 不碰 DOM、不碰 Monaco，因此能在 Node 里逐条断言，组件只负责照表渲染与执行。
//
// 语言标识沿用 Monaco 的 languageId（见 GuaxiEditor 的 EXT_TO_LANGUAGE）：
//   markdown / typst / latex / 其它
// 「其它」一律返回空能力表 —— 工具栏不显示任何 Markdown 专用按钮，
// 这正是用户要的「不支持就不显示」，也避免用户误触后看到「仅 Markdown 可用」的提示。

/**
 * 工具栏动作的能力位。
 *
 * 每个动作都带 `languages` 白名单：只有当前文档语言的标识在里面时才渲染。
 * 未列出的动作视为「与语言无关」（撤销/重做这类），由组件自己决定是否显示。
 */
export const TOOLBAR_LANGUAGES = {
  markdown: 'markdown',
  typst: 'typst',
  latex: 'latex',
}

/** 把 Monaco languageId 归一成我们认识的三种之一，其余返回 'other'。 */
export const normalizeToolbarLanguage = (languageId) => {
  const id = String(languageId || '').trim().toLowerCase()
  if (id === 'markdown') return 'markdown'
  if (id === 'typst') return 'typst'
  if (id === 'latex' || id === 'tex') return 'latex'
  return 'other'
}

// ───────────────────────────── 各语言的插入语法

const MARKDOWN_SNIPPETS = {
  bold: { prefix: '**', suffix: '**', placeholder: '粗体文本' },
  italic: { prefix: '*', suffix: '*', placeholder: '斜体文本' },
  strikethrough: { prefix: '~~', suffix: '~~', placeholder: '删除线' },
  inlineCode: { prefix: '`', suffix: '`', placeholder: 'code' },
  link: { prefix: '[', suffix: '](https://)', placeholder: '链接文字' },
  // Markdown 没有颜色语法，靠 markdown-it 的 html:true 保留内联样式。
  color: { wrap: (text, color) => `<span style="color:${color}">${text}</span>` },
  heading: (level, text) => (level > 0 ? `${'#'.repeat(level)} ${text}` : text),
  headingPrefixPattern: (level) => new RegExp(`^#{${level}}\\s+`),
  anyHeadingPrefixPattern: /^#{1,6}\s+/,
  codeBlock: (lang, body) => '```' + (lang || '') + '\n' + (body || '') + '\n```',
  image: (alt, src) => `![${alt || '图片'}](${src || ''})`,
  table: (rows, cols) => {
    const head = `| ${Array.from({ length: cols }, (_, i) => `列 ${i + 1}`).join(' | ')} |`
    const sep = `| ${Array.from({ length: cols }, () => '---').join(' | ')} |`
    const body = Array.from({ length: Math.max(0, rows - 1) }, () => `| ${Array.from({ length: cols }, () => '   ').join(' | ')} |`)
    return [head, sep, ...body].join('\n')
  },
  horizontalRule: () => '---',
  bulletListPrefix: '- ',
  orderedListPrefix: '1. ',
  taskListPrefix: '- [ ] ',
  blockquotePrefix: '> ',
}

const TYPST_SNIPPETS = {
  // Typst 的强调语法：*粗体* 与 _斜体_（Markdown 的 ** / * 在 typst 里含义不同）。
  bold: { prefix: '*', suffix: '*', placeholder: '粗体文本' },
  italic: { prefix: '_', suffix: '_', placeholder: '斜体文本' },
  // Typst 没有删除线语法；用 `#strike[...]` 需要额外包，这里给内置的 `#strike` 不存在，
  // 因此改用最接近的内置能力：`#text(fill: ...)` 不可替代。为不插入无效语法，直接不提供。
  inlineCode: { prefix: '`', suffix: '`', placeholder: 'code' },
  link: { prefix: '#link("', suffix: '")[链接文字]', placeholder: 'https://' },
  // Typst 用 text(fill:) 上色。
  color: { wrap: (text, color) => `#text(fill: rgb("${color}"))[${text}]` },
  // Typst 有数值层级标题，但编辑器工具栏选择的是「几级标题」，用等号数量表达。
  heading: (level, text) => (level > 0 ? `${'='.repeat(level)} ${text}` : text),
  headingPrefixPattern: (level) => new RegExp(`^${'='.repeat(level)}\\s+`),
  anyHeadingPrefixPattern: /^=+\s+/,
  // Typst 代码块：三个反引号的原始块，可带语言标注。
  codeBlock: (lang, body) => '```' + (lang || '') + '\n' + (body || '') + '\n```',
  image: (alt, src) => `#image("${src || ''}", alt: "${alt || '图片'}")`,
  table: (rows, cols) => {
    const cells = Array.from({ length: rows }, () =>
      Array.from({ length: cols }, () => '[]').join(', '))
    return `#table(\n  columns: ${cols},\n${cells.map((row) => `  ${row},`).join('\n')}\n)`
  },
  horizontalRule: () => '#line(length: 100%)',
  bulletListPrefix: '- ',
  orderedListPrefix: '+ ',
  taskListPrefix: '- [ ] ',
  blockquotePrefix: '> ',
}

const LATEX_SNIPPETS = {
  bold: { prefix: '\\textbf{', suffix: '}', placeholder: '粗体文本' },
  italic: { prefix: '\\textit{', suffix: '}', placeholder: '斜体文本' },
  // LaTeX 没有原生删除线；需要 ulem 宏包。插入会报错，因此不提供。
  inlineCode: { prefix: '\\texttt{', suffix: '}', placeholder: 'code' },
  link: { prefix: '\\href{', suffix: '}{链接文字}', placeholder: 'https://' },
  color: { wrap: (text, color) => `\\textcolor[HTML]{${String(color).replace('#', '').toUpperCase()}}{${text}}` },
  // LaTeX 的章节层级：1..3 映射到 section / subsection / subsubsection。
  heading: (level, text) => {
    const command = ['', 'section', 'subsection', 'subsubsection', 'paragraph', 'subparagraph'][level] || 'section'
    return `\\${command}{${text}}`
  },
  headingPrefixPattern: (level) => {
    const command = ['', 'section', 'subsection', 'subsubsection', 'paragraph', 'subparagraph'][level] || 'section'
    return new RegExp(`^\\\\${command}\\*?\\{`)
  },
  anyHeadingPrefixPattern: /^\\(section|subsection|subsubsection|paragraph|subparagraph)\*?\{/,
  // LaTeX 代码块：用 verbatim 环境（不需要额外宏包）。
  codeBlock: (lang, body) => `\\begin{verbatim}\n${body || ''}\n\\end{verbatim}`,
  image: (alt, src) => `\\includegraphics[width=\\linewidth]{${src || ''}}`,
  table: (rows, cols) => {
    const spec = 'c'.repeat(Math.max(1, cols))
    const body = Array.from({ length: rows }, () =>
      Array.from({ length: cols }, () => ' ').join(' & ') + ' \\\\').join('\n')
    return `\\begin{tabular}{${spec}}\n${body}\n\\hline\n\\end{tabular}`
  },
  horizontalRule: () => '\\hrule',
  bulletListPrefix: '\\item ',
  orderedListPrefix: '\\item ',
  taskListPrefix: '\\item ',
  blockquotePrefix: '> ',
}

export const SNIPPETS_BY_LANGUAGE = {
  markdown: MARKDOWN_SNIPPETS,
  typst: TYPST_SNIPPETS,
  latex: LATEX_SNIPPETS,
}

/**
 * 每个动作在哪些语言下可用。
 *
 * 这里刻意**逐项列出**而不是「有 snippets 就算支持」：
 * typst 没有删除线、latex 的删除线要额外宏包，两者都不该出现在工具栏里。
 * 任何没列出的语言（other）自然得到空能力表。
 */
export const TOOLBAR_CAPABILITIES = {
  heading: ['markdown', 'typst', 'latex'],
  bold: ['markdown', 'typst', 'latex'],
  italic: ['markdown', 'typst', 'latex'],
  strikethrough: ['markdown'],
  link: ['markdown', 'typst', 'latex'],
  blockquote: ['markdown', 'typst', 'latex'],
  inlineCode: ['markdown', 'typst', 'latex'],
  color: ['markdown', 'typst', 'latex'],
  codeBlock: ['markdown', 'typst', 'latex'],
  bulletList: ['markdown', 'typst', 'latex'],
  orderedList: ['markdown', 'typst', 'latex'],
  // 任务列表是 Markdown 特有的复选框语法；typst/latex 无对应语义，不提供。
  taskList: ['markdown'],
  table: ['markdown', 'typst', 'latex'],
  image: ['markdown', 'typst', 'latex'],
  horizontalRule: ['markdown', 'typst', 'latex'],
  // 图表（数据入资源库）与 Python Cell 都依赖 Markdown 管线的扩展渲染。
  chart: ['markdown'],
  pythonCell: ['markdown'],
  // 字体颜色在 typst/latex 里也能用，但两者的颜色模型差异大（rgb / HTML），
  // 色板插入后用户在编译报错里不易理解，先只给 markdown。
  // （color 动作本身支持三语言，这里不重复列出。）
}

/** 某语言下某个动作是否可用。 */
export const supportsToolbarAction = (languageId, action) => {
  const lang = normalizeToolbarLanguage(languageId)
  const allowed = TOOLBAR_CAPABILITIES[action]
  return Array.isArray(allowed) && allowed.includes(lang)
}

/** 某语言下可用的动作集合（便于组件一次性判断、也便于测试枚举）。 */
export const toolbarActionsForLanguage = (languageId) => {
  const lang = normalizeToolbarLanguage(languageId)
  return Object.entries(TOOLBAR_CAPABILITIES)
    .filter(([, languages]) => languages.includes(lang))
    .map(([action]) => action)
}

/** 取某语言的插入语法表；不支持的返回 null。 */
export const snippetsForLanguage = (languageId) => {
  const lang = normalizeToolbarLanguage(languageId)
  return SNIPPETS_BY_LANGUAGE[lang] || null
}

/** 标题层级下拉项：markdown 用 H1..H6，typst/latex 至多少数几级。 */
export const headingOptionsForLanguage = (languageId) => {
  const lang = normalizeToolbarLanguage(languageId)
  if (lang === 'typst') {
    return [
      { label: '正文', key: 0 },
      { label: '一级标题', key: 1 },
      { label: '二级标题', key: 2 },
      { label: '三级标题', key: 3 },
    ]
  }
  if (lang === 'latex') {
    return [
      { label: '正文', key: 0 },
      { label: 'Section', key: 1 },
      { label: 'Subsection', key: 2 },
      { label: 'Subsubsection', key: 3 },
    ]
  }
  return [
    { label: '正文', key: 0 },
    { label: 'H1', key: 1 },
    { label: 'H2', key: 2 },
    { label: 'H3', key: 3 },
    { label: 'H4', key: 4 },
    { label: 'H5', key: 5 },
    { label: 'H6', key: 6 },
  ]
}

/**
 * 生成「切换标题级别」的行内文本。
 *
 * 三种语言的标题都是**行首**语法（# / = / \section{），所以统一的处理是：
 * 先剥掉原有前缀，再按目标级别重新加。剥与加都必须用该语言自己的规则，
 * 否则 typst 文档会被识别成「没有标题」而叠加出 `======== 标题`。
 */
export const buildHeadingLine = (languageId, { lineText, level }) => {
  const lang = normalizeToolbarLanguage(languageId)
  const snippets = snippetsForLanguage(lang)
  if (!snippets) return String(lineText ?? '')
  const text = String(lineText ?? '')
  // LaTeX 的标题是 `\section{...}`，剥前缀同时要剥右花括号。
  if (lang === 'latex') {
    const m = text.match(/^\\(section|subsection|subsubsection|paragraph|subparagraph)\*?\{(.*)\}\s*$/)
    const inner = m ? m[2] : text
    return level > 0 ? snippets.heading(level, inner) : inner
  }
  const stripped = text.replace(snippets.anyHeadingPrefixPattern, '')
  return level > 0 ? snippets.heading(level, stripped || '标题') : stripped
}
