// 主编辑器使用的一组补全片段。抽成共享模块后，探索者 cell / 模板管理器 / Slidev
// 也能拿到同一份片段表，避免各入口的 snippet 表各自维护、逐渐漂移。
//
// 每个 builder 都接收 monaco 实例：CompletionItemKind / InsertTextRule 是运行期枚举，
// 不能在模块加载时写死。

export const buildMarkdownSnippets = (monaco) => {
  const createSnippet = (label, insertText, detail) => ({
    label,
    kind: monaco.languages.CompletionItemKind.Snippet,
    insertText,
    insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
    detail,
  })

  return [
    createSnippet('h1', '# ${1:标题}\n\n', '插入一级标题'),
    createSnippet('h2', '## ${1:标题}\n\n', '插入二级标题'),
    createSnippet('h3', '### ${1:标题}\n\n', '插入三级标题'),
    createSnippet('h4', '#### ${1:标题}\n\n', '插入四级标题'),
    createSnippet('h5', '##### ${1:标题}\n\n', '插入五级标题'),
    createSnippet('h6', '###### ${1:标题}\n\n', '插入六级标题'),
    createSnippet('ul', '- ${1:列表项}\n- ${2:列表项}\n', '插入无序列表'),
    createSnippet('ol', '1. ${1:第一项}\n2. ${2:第二项}\n', '插入有序列表'),
    createSnippet('taskl', '- [ ] ${1:任务}\n- [ ] ${2:任务}\n', '插入任务列表'),
    createSnippet('table', '| ${1:表头1} | ${2:表头2} |\n| --- | --- |\n| ${3:内容1} | ${4:内容2} |\n', '插入表格'),
    createSnippet('code', '```${1:language}\n${2:代码}\n```\n', '插入代码块'),
    createSnippet('pycell', '<!--py-->\n${1:# Python Cell\\nprint("Hello from Guaxi Python Cell")}\n<!--/py-->\n', '插入隐藏 Python Cell'),
    createSnippet('toc', '${1:目录}\n\n${2:目录内容}\n', '插入目录'),
    createSnippet('bold', '**${1:文本}**', '加粗文本'),
    createSnippet('italic', '*${1:文本}*', '斜体文本'),
    createSnippet('strikeout', '~~${1:文本}~~', '删除线'),
    createSnippet('inlineCode', '`${1:code}`', '内联代码'),
    createSnippet('link', '[${1:描述}](${2:链接})', '插入链接'),
    createSnippet('image', '![${1:描述}](${2:路径})', '插入图片'),
    createSnippet('quote', '> ${1:引用内容}\n', '插入引用'),
    createSnippet('hr', '\n---\n', '插入分隔线'),
    createSnippet('red', '<font color="red">${1:红色文字}</font>', '设置红色文本'),
    createSnippet('blue', '<font color="blue">${1:蓝色文字}</font>', '设置蓝色文本'),
    createSnippet('green', '<font color="green">${1:绿色文字}</font>', '设置绿色文本'),
    createSnippet('yellow', '<font color="yellow">${1:黄色文字}</font>', '设置黄色文本'),
    createSnippet('orange', '<font color="orange">${1:橙色文字}</font>', '设置橙色文本'),
    createSnippet('purple', '<font color="purple">${1:紫色文字}</font>', '设置紫色文本'),
    createSnippet('cyan', '<font color="cyan">${1:青色文字}</font>', '设置青色文本'),
    createSnippet('purple', '<font color="purple">${1:紫色文字}</font>', '设置紫色文本'),
  ]
}

export const buildLatexSnippets = (monaco) => {
  const createLatexSnippet = (key, insertText, detail, label) => ({
    key: String(key || '').toLowerCase(),
    label: label || `\\${key}`,
    kind: monaco.languages.CompletionItemKind.Snippet,
    insertText,
    insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
    detail,
  })

  return [
    createLatexSnippet('frac', '\\frac{${1:a}}{${2:b}}', '分式'),
    createLatexSnippet('sqrt', '\\sqrt{${1:x}}', '平方根'),
    createLatexSnippet('sqrtn', '\\sqrt[${1:n}]{${2:x}}', 'n 次方根', '\\sqrt[n]{x}'),
    createLatexSnippet('sum', '\\sum_{${1:i=1}}^{${2:n}} ${3:expr}', '求和'),
    createLatexSnippet('prod', '\\prod_{${1:i=1}}^{${2:n}} ${3:expr}', '连乘'),
    createLatexSnippet('int', '\\int_{${1:a}}^{${2:b}} ${3:f(x)}\\,d${4:x}', '积分'),
    createLatexSnippet('lim', '\\lim_{${1:n}\\to\\infty} ${2:expr}', '极限'),
    createLatexSnippet('cdot', '\\cdot', '点乘'),
    createLatexSnippet('times', '\\times', '乘号'),
    createLatexSnippet('pm', '\\pm', '正负号'),
    createLatexSnippet('to', '\\to', '趋于'),
    createLatexSnippet('infty', '\\infty', '无穷'),

    createLatexSnippet('left', '\\left(${1:expr}\\right)', '自动伸缩括号'),
    createLatexSnippet('text', '\\text{${1:text}}', '文本'),
    createLatexSnippet('mathbf', '\\mathbf{${1:x}}', '粗体'),
    createLatexSnippet('mathbb', '\\mathbb{${1:R}}', '黑板粗体'),
    createLatexSnippet('mathcal', '\\mathcal{${1:L}}', '花体'),

    createLatexSnippet('begin', '\\begin{${1:env}}\n  ${0}\n\\end{${1:env}}', '环境'),
    createLatexSnippet('aligned', '\\begin{aligned}\n  ${0}\n\\end{aligned}', 'aligned 环境', '\\begin{aligned}...\\end{aligned}'),
    createLatexSnippet('align', '\\begin{align}\n  ${0}\n\\end{align}', 'align 环境', '\\begin{align}...\\end{align}'),
    createLatexSnippet('cases', '\\begin{cases}\n  ${1:expr} & ${2:cond}\\\\\n  ${3:expr} & ${4:cond}\n\\end{cases}', 'cases 环境', '\\begin{cases}...\\end{cases}'),

    createLatexSnippet('alpha', '\\alpha', '希腊字母 α'),
    createLatexSnippet('beta', '\\beta', '希腊字母 β'),
    createLatexSnippet('gamma', '\\gamma', '希腊字母 γ'),
    createLatexSnippet('delta', '\\delta', '希腊字母 δ'),
    createLatexSnippet('epsilon', '\\epsilon', '希腊字母 ε'),
    createLatexSnippet('theta', '\\theta', '希腊字母 θ'),
    createLatexSnippet('lambda', '\\lambda', '希腊字母 λ'),
    createLatexSnippet('mu', '\\mu', '希腊字母 μ'),
    createLatexSnippet('pi', '\\pi', '希腊字母 π'),
    createLatexSnippet('sigma', '\\sigma', '希腊字母 σ'),
    createLatexSnippet('phi', '\\phi', '希腊字母 φ'),
    createLatexSnippet('omega', '\\omega', '希腊字母 ω'),

    createLatexSnippet('leq', '\\leq', '≤'),
    createLatexSnippet('geq', '\\geq', '≥'),
    createLatexSnippet('neq', '\\neq', '≠'),
    createLatexSnippet('approx', '\\approx', '≈'),
    createLatexSnippet('equiv', '\\equiv', '≡'),
    createLatexSnippet('subseteq', '\\subseteq', '⊆'),
    createLatexSnippet('supseteq', '\\supseteq', '⊇'),
    createLatexSnippet('in', '\\in', '∈'),
    createLatexSnippet('notin', '\\notin', '∉'),
    createLatexSnippet('sin', '\\sin', '正弦'),
    createLatexSnippet('cos', '\\cos', '余弦'),
    createLatexSnippet('tan', '\\tan', '正切'),
    createLatexSnippet('log', '\\log', '对数'),
    createLatexSnippet('ln', '\\ln', '自然对数'),
    createLatexSnippet('exp', '\\exp', '指数'),
    createLatexSnippet('partial', '\\partial', '偏导符号'),
    createLatexSnippet('nabla', '\\nabla', 'Nabla 符号'),
    createLatexSnippet('vec', '\\vec{${1:v}}', '向量'),
    createLatexSnippet('hat', '\\hat{${1:x}}', '帽子重音'),
    createLatexSnippet('bar', '\\bar{${1:x}}', '横线重音'),
    createLatexSnippet('overline', '\\overline{${1:x}}', '上划线'),
    createLatexSnippet('underline', '\\underline{${1:x}}', '下划线'),
    createLatexSnippet('label', '\\label{${1:key}}', '标签'),
    createLatexSnippet('ref', '\\ref{${1:key}}', '引用标签'),
    createLatexSnippet('eqref', '\\eqref{${1:key}}', '公式引用'),
    createLatexSnippet('cite', '\\cite{${1:key}}', '文献引用'),
    createLatexSnippet('item', '\\item ${1:text}', '列表项'),
    createLatexSnippet('emph', '\\emph{${1:text}}', '强调文本'),
    createLatexSnippet('section', '\\section{${1:title}}', '章节标题'),
    createLatexSnippet('subsection', '\\subsection{${1:title}}', '二级章节'),
    createLatexSnippet('subsubsection', '\\subsubsection{${1:title}}', '三级章节'),
    createLatexSnippet('document', '\\begin{document}\n${0}\n\\end{document}', 'document 环境', '\\begin{document}...\\end{document}'),
    createLatexSnippet('equation', '\\begin{equation}\n  ${0}\n\\end{equation}', 'equation 环境', '\\begin{equation}...\\end{equation}'),
    createLatexSnippet('matrix', '\\begin{matrix}\n  ${1:a} & ${2:b}\\\\\n  ${3:c} & ${4:d}\n\\end{matrix}', 'matrix 环境', '\\begin{matrix}...\\end{matrix}'),
    createLatexSnippet('pmatrix', '\\begin{pmatrix}\n  ${1:a} & ${2:b}\\\\\n  ${3:c} & ${4:d}\n\\end{pmatrix}', 'pmatrix 环境', '\\begin{pmatrix}...\\end{pmatrix}'),
    createLatexSnippet('bmatrix', '\\begin{bmatrix}\n  ${1:a} & ${2:b}\\\\\n  ${3:c} & ${4:d}\n\\end{bmatrix}', 'bmatrix 环境', '\\begin{bmatrix}...\\end{bmatrix}'),
    createLatexSnippet('enumerate', '\\begin{enumerate}\n  \\item ${1:item}\n\\end{enumerate}', 'enumerate 环境', '\\begin{enumerate}...\\end{enumerate}'),
    createLatexSnippet('itemize', '\\begin{itemize}\n  \\item ${1:item}\n\\end{itemize}', 'itemize 环境', '\\begin{itemize}...\\end{itemize}')
  ]
}

export const buildMermaidSnippets = (monaco) => {
  const createMermaidSnippet = (key, insertText, detail, label) => ({
    key: String(key || '').toLowerCase(),
    label: label || key,
    kind: monaco.languages.CompletionItemKind.Snippet,
    insertText,
    insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
    detail,
  })

  return [
    createMermaidSnippet('graph', 'graph ${1:TD}\n  ${2:A}[${3:Start}] --> ${4:B}[${5:End}]', 'Mermaid flowchart'),
    createMermaidSnippet('flowchart', 'flowchart ${1:TD}\n  ${2:A}[${3:Start}] --> ${4:B}[${5:End}]', 'Mermaid flowchart'),
    createMermaidSnippet('sequenceDiagram', 'sequenceDiagram\n  participant ${1:A}\n  participant ${2:B}\n  ${1:A}->>${2:B}: ${3:message}', 'Mermaid sequence diagram'),
    createMermaidSnippet('classDiagram', 'classDiagram\n  class ${1:Name} {\n    +${2:method}()\n  }', 'Mermaid class diagram'),
    createMermaidSnippet('stateDiagram-v2', 'stateDiagram-v2\n  [*] --> ${1:State}\n  ${1:State} --> [*]', 'Mermaid state diagram'),
    createMermaidSnippet('erDiagram', 'erDiagram\n  ${1:USER} ||--o{ ${2:ORDER} : ${3:places}', 'Mermaid ER diagram'),
    createMermaidSnippet('gantt', 'gantt\n  title ${1:Project}\n  dateFormat YYYY-MM-DD\n  section ${2:Phase}\n  ${3:Task} :${4:task_id}, ${5:2026-01-01}, ${6:7d}', 'Mermaid gantt'),
    createMermaidSnippet('pie', 'pie title ${1:Title}\n  "${2:A}" : ${3:40}\n  "${4:B}" : ${5:60}', 'Mermaid pie chart'),
    createMermaidSnippet('mindmap', 'mindmap\n  root((${1:Root}))\n    ${2:Topic}\n      ${3:Item}', 'Mermaid mindmap'),
    createMermaidSnippet('timeline', 'timeline\n  title ${1:Timeline}\n  ${2:2026} : ${3:Event}', 'Mermaid timeline'),
    createMermaidSnippet('gitGraph', 'gitGraph\n  commit\n  branch ${1:feature}\n  checkout ${1:feature}\n  commit\n  checkout main\n  merge ${1:feature}', 'Mermaid gitGraph'),
    createMermaidSnippet('direction', 'direction ${1:LR}', '设置子图方向'),
    createMermaidSnippet('subgraph', 'subgraph ${1:title}\n  ${2:A} --> ${3:B}\nend', 'subgraph 块'),
    createMermaidSnippet('style', 'style ${1:A} fill:${2:#f9f},stroke:${3:#333},stroke-width:${4:2px}', '节点样式'),
    createMermaidSnippet('linkStyle', 'linkStyle ${1:0} stroke:${2:#f66},stroke-width:${3:2px}', '连线样式'),
    createMermaidSnippet('click', 'click ${1:A} "${2:https://example.com}" "${3:tooltip}"', '节点点击链接'),
    createMermaidSnippet('participant', 'participant ${1:Alice}', 'sequence participant'),
    createMermaidSnippet('actor', 'actor ${1:User}', 'sequence actor'),
    createMermaidSnippet('note', 'Note ${1:right of} ${2:Alice}: ${3:text}', 'sequence note')
  ]
}
