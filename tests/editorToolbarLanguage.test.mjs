// 工具栏「按文件类型切换」的回归测试。
//
// 背景：工具栏原先整排写死，所有插入动作都过 `ensureMarkdownEditor()`——在 .typ/.tex
// 里按钮全都在，按下去要么插入 Markdown 语法（对这两种语言是废文本），要么直接提示
// 「仅在 Markdown 文件中可用」。用户要求：不支持就不显示，支持就插入对应语法。
//
// 这里锁的是**规则表**：哪种语言有哪些动作、每种动作插入什么。组件只负责照表渲染，
// 所以规则正确 = 行为正确。
import test from 'node:test'
import assert from 'node:assert/strict'

import {
  normalizeToolbarLanguage,
  supportsToolbarAction,
  toolbarActionsForLanguage,
  snippetsForLanguage,
  headingOptionsForLanguage,
  buildHeadingLine,
  SNIPPETS_BY_LANGUAGE,
} from '../src/components/HomeViewComponent/editorToolbarLanguage.js'

// ───────────────────────────── 语言归一

test('语言归一：Monaco 的 languageId 映射到三种支持语言，其余归为 other', () => {
  assert.equal(normalizeToolbarLanguage('markdown'), 'markdown')
  assert.equal(normalizeToolbarLanguage('typst'), 'typst')
  assert.equal(normalizeToolbarLanguage('latex'), 'latex')
  assert.equal(normalizeToolbarLanguage('tex'), 'latex')
  assert.equal(normalizeToolbarLanguage('python'), 'other')
  assert.equal(normalizeToolbarLanguage('plaintext'), 'other')
  assert.equal(normalizeToolbarLanguage(''), 'other')
  assert.equal(normalizeToolbarLanguage(null), 'other')
})

test('不支持的语言（代码/纯文本）没有任何工具栏动作', () => {
  // 这正是「不支持就不显示」：能力表为空，模板里 v-if 全部为假。
  assert.deepEqual(toolbarActionsForLanguage('python'), [])
  assert.deepEqual(toolbarActionsForLanguage('plaintext'), [])
  assert.deepEqual(toolbarActionsForLanguage('json'), [])
})

// ───────────────────────────── 能力矩阵

test('markdown 保留全部原有动作', () => {
  const actions = toolbarActionsForLanguage('markdown')
  for (const action of [
    'heading', 'bold', 'italic', 'strikethrough', 'link', 'blockquote',
    'inlineCode', 'color', 'codeBlock', 'bulletList', 'orderedList',
    'taskList', 'table', 'image', 'horizontalRule', 'chart', 'pythonCell',
  ]) {
    assert.ok(actions.includes(action), `markdown 缺少 ${action}`)
  }
})

test('typst 有编辑动作，但没有 Markdown 独有的删除线/任务列表/图表/Python Cell', () => {
  assert.equal(supportsToolbarAction('typst', 'bold'), true)
  assert.equal(supportsToolbarAction('typst', 'heading'), true)
  assert.equal(supportsToolbarAction('typst', 'table'), true)
  assert.equal(supportsToolbarAction('typst', 'image'), true)
  assert.equal(supportsToolbarAction('typst', 'strikethrough'), false, 'typst 没有内建删除线')
  assert.equal(supportsToolbarAction('typst', 'taskList'), false)
  assert.equal(supportsToolbarAction('typst', 'chart'), false)
  assert.equal(supportsToolbarAction('typst', 'pythonCell'), false)
})

test('latex 有编辑动作，但没有删除线（需额外宏包）与 Markdown 独有项', () => {
  assert.equal(supportsToolbarAction('latex', 'bold'), true)
  assert.equal(supportsToolbarAction('latex', 'table'), true)
  assert.equal(supportsToolbarAction('latex', 'image'), true)
  assert.equal(supportsToolbarAction('latex', 'strikethrough'), false)
  assert.equal(supportsToolbarAction('latex', 'taskList'), false)
  assert.equal(supportsToolbarAction('latex', 'chart'), false)
})

test('任何声称支持的动作都必须有对应语法实现', () => {
  for (const lang of ['markdown', 'typst', 'latex']) {
    const snippets = snippetsForLanguage(lang)
    const actions = toolbarActionsForLanguage(lang)
    const snippetKey = {
      heading: 'heading',
      bold: 'bold',
      italic: 'italic',
      strikethrough: 'strikethrough',
      link: 'link',
      blockquote: 'blockquotePrefix',
      inlineCode: 'inlineCode',
      color: 'color',
      codeBlock: 'codeBlock',
      bulletList: 'bulletListPrefix',
      orderedList: 'orderedListPrefix',
      taskList: 'taskListPrefix',
      table: 'table',
      image: 'image',
      horizontalRule: 'horizontalRule',
    }[actions.find(() => true)] // 只为取个 key 占位，下面逐项判断
    void snippetKey
    for (const action of actions) {
      const key = {
        heading: 'heading', bold: 'bold', italic: 'italic', strikethrough: 'strikethrough',
        link: 'link', blockquote: 'blockquotePrefix', inlineCode: 'inlineCode', color: 'color',
        codeBlock: 'codeBlock', bulletList: 'bulletListPrefix', orderedList: 'orderedListPrefix',
        taskList: 'taskListPrefix', table: 'table', image: 'image', horizontalRule: 'horizontalRule',
      }[action]
      if (!key) continue // chart / pythonCell 走各自独立实现，不在语法表里
      assert.ok(snippets[key] !== undefined, `${lang} 声称支持 ${action} 但没有 ${key} 实现`)
    }
  }
})

// ───────────────────────────── 各语言插入的语法

test('粗体/斜体：三种语言发出各自的语法，不是 Markdown 的 ** / *', () => {
  assert.equal(SNIPPETS_BY_LANGUAGE.markdown.bold.prefix, '**')
  assert.equal(SNIPPETS_BY_LANGUAGE.typst.bold.prefix, '*', 'typst 的粗体是单星号')
  assert.equal(SNIPPETS_BY_LANGUAGE.typst.italic.prefix, '_', 'typst 的斜体是下划线')
  assert.equal(SNIPPETS_BY_LANGUAGE.latex.bold.prefix, '\\textbf{')
  assert.equal(SNIPPETS_BY_LANGUAGE.latex.italic.prefix, '\\textit{')
})

test('标题：markdown # / typst = / latex \\section{}', () => {
  assert.equal(buildHeadingLine('markdown', { lineText: '标题', level: 2 }), '## 标题')
  assert.equal(buildHeadingLine('typst', { lineText: '标题', level: 2 }), '== 标题')
  assert.equal(buildHeadingLine('latex', { lineText: '标题', level: 1 }), '\\section{标题}')
  assert.equal(buildHeadingLine('latex', { lineText: '标题', level: 2 }), '\\subsection{标题}')
})

test('标题：切换级别会先剥掉原前缀，不会叠加出 ========', () => {
  // typst：一级改二级
  assert.equal(buildHeadingLine('typst', { lineText: '= 标题', level: 2 }), '== 标题')
  // typst：标题改回正文
  assert.equal(buildHeadingLine('typst', { lineText: '=== 标题', level: 0 }), '标题')
  // markdown：H2 改 H1
  assert.equal(buildHeadingLine('markdown', { lineText: '## 标题', level: 1 }), '# 标题')
  // latex：section 改 subsection（同时要剥掉右花括号）
  assert.equal(
    buildHeadingLine('latex', { lineText: '\\section{标题}', level: 2 }),
    '\\subsection{标题}',
  )
  // latex：改回正文，花括号也要去掉
  assert.equal(buildHeadingLine('latex', { lineText: '\\section{标题}', level: 0 }), '标题')
  // 幂等：同一级别连点两次不应变成 ========
  const once = buildHeadingLine('typst', { lineText: '标题', level: 2 })
  const twice = buildHeadingLine('typst', { lineText: once, level: 2 })
  assert.equal(twice, '== 标题')
})

test('标题：空行加标题会补占位文字，不会留下孤零零的 ==', () => {
  assert.equal(buildHeadingLine('typst', { lineText: '', level: 2 }), '== 标题')
  assert.equal(buildHeadingLine('markdown', { lineText: '', level: 1 }), '# 标题')
})

test('代码块：typst 与 markdown 都用围栏，latex 用 verbatim 环境', () => {
  assert.equal(SNIPPETS_BY_LANGUAGE.markdown.codeBlock('py', 'x'), '```py\nx\n```')
  assert.equal(SNIPPETS_BY_LANGUAGE.typst.codeBlock('', 'x'), '```\nx\n```')
  assert.equal(SNIPPETS_BY_LANGUAGE.latex.codeBlock('', 'x'), '\\begin{verbatim}\nx\n\\end{verbatim}')
})

test('图片：三种语言的语法各不相同', () => {
  assert.equal(SNIPPETS_BY_LANGUAGE.markdown.image('图', 'a.png'), '![图](a.png)')
  assert.equal(SNIPPETS_BY_LANGUAGE.typst.image('图', 'a.png'), '#image("a.png", alt: "图")')
  assert.equal(SNIPPETS_BY_LANGUAGE.latex.image('图', 'a.png'), '\\includegraphics[width=\\linewidth]{a.png}')
})

test('表格：typst 生成 #table，latex 生成 tabular，markdown 保持竖线表', () => {
  const typ = SNIPPETS_BY_LANGUAGE.typst.table(2, 3)
  assert.ok(typ.startsWith('#table('), typ)
  assert.ok(typ.includes('columns: 3'), typ)
  const tex = SNIPPETS_BY_LANGUAGE.latex.table(2, 2)
  assert.ok(tex.includes('\\begin{tabular}{cc}'), tex)
  assert.ok(tex.includes('\\end{tabular}'), tex)
  const md = SNIPPETS_BY_LANGUAGE.markdown.table(2, 2)
  assert.ok(md.includes('| --- |'), md)
})

test('分隔线：三种语言各自的写法', () => {
  assert.equal(SNIPPETS_BY_LANGUAGE.markdown.horizontalRule(), '---')
  assert.equal(SNIPPETS_BY_LANGUAGE.typst.horizontalRule(), '#line(length: 100%)')
  assert.equal(SNIPPETS_BY_LANGUAGE.latex.horizontalRule(), '\\hrule')
})

test('字体颜色：typst 用 text(fill:)，latex 用 textcolor，markdown 用内联 span', () => {
  assert.equal(
    SNIPPETS_BY_LANGUAGE.typst.color.wrap('文字', '#3b82f6'),
    '#text(fill: rgb("#3b82f6"))[文字]',
  )
  assert.equal(
    SNIPPETS_BY_LANGUAGE.latex.color.wrap('文字', '#3b82f6'),
    '\\textcolor[HTML]{3B82F6}{文字}',
  )
  assert.equal(
    SNIPPETS_BY_LANGUAGE.markdown.color.wrap('文字', '#3b82f6'),
    '<span style="color:#3b82f6">文字</span>',
  )
})

test('链接：三种语言的包裹语法', () => {
  assert.equal(SNIPPETS_BY_LANGUAGE.markdown.link.suffix, '](https://)')
  assert.equal(SNIPPETS_BY_LANGUAGE.typst.link.prefix, '#link("')
  assert.equal(SNIPPETS_BY_LANGUAGE.typst.link.suffix, '")[链接文字]')
  assert.equal(SNIPPETS_BY_LANGUAGE.latex.link.prefix, '\\href{')
})

// ───────────────────────────── 标题菜单

test('标题菜单随语言变化：markdown 六级、typst 四级、latex 用 section 命名', () => {
  assert.equal(headingOptionsForLanguage('markdown').length, 7, '正文 + H1..H6')
  assert.equal(headingOptionsForLanguage('typst').length, 4)
  assert.ok(headingOptionsForLanguage('typst').every((o) => o.label === '正文' || o.key >= 1))
  const latexLabels = headingOptionsForLanguage('latex').map((o) => o.label)
  assert.deepEqual(latexLabels, ['正文', 'Section', 'Subsection', 'Subsubsection'])
})

test('标题菜单首项都是「正文」，且 key 从 0 开始连续', () => {
  for (const lang of ['markdown', 'typst', 'latex']) {
    const options = headingOptionsForLanguage(lang)
    assert.equal(options[0].key, 0)
    assert.equal(options[0].label, '正文')
    options.forEach((o, i) => assert.equal(o.key, i))
  }
})

test('不支持的语言取不到语法表（组件据此不渲染任何编辑按钮）', () => {
  assert.equal(snippetsForLanguage('python'), null)
  assert.equal(snippetsForLanguage('plaintext'), null)
})
