#set document(title: "Typst Syntax Manual / Typst 语法手册", author: "Guaxi")
#set page(
  paper: "a4",
  margin: (x: 1.65cm, y: 1.55cm),
  numbering: "1 / 1",
  header: align(right)[#text(size: 8pt, fill: gray)[Guaxi Typst Manual]],
)
#set text(
  font: ("Noto Serif CJK SC", "Microsoft YaHei", "SimSun", "Arial"),
  size: 10pt,
  lang: "zh",
)
#set par(justify: true, first-line-indent: 1.6em, leading: 0.68em)
#set heading(numbering: "1.1")
#set list(indent: 1.25em, body-indent: 0.55em)
#set enum(indent: 1.25em, body-indent: 0.55em)

#let accent = rgb("#2563eb")
#let soft = rgb("#eff6ff")
#let border = rgb("#bfdbfe")
#let muted = rgb("#64748b")

#let tag(body) = box(
  fill: rgb("#ecfeff"),
  stroke: rgb("#67e8f9"),
  radius: 999pt,
  inset: (x: 6pt, y: 2pt),
)[#text(size: 8pt, fill: rgb("#155e75"))[#body]]

#let callout(title, body) = block(
  fill: soft,
  stroke: border,
  radius: 4pt,
  inset: 8pt,
  width: 100%,
)[
  #text(weight: "bold", fill: accent)[#title]
  #v(4pt)
  #body
]

#show strong: set text(fill: rgb("#b91c1c"), weight: "bold")
#show emph: set text(fill: rgb("#7c3aed"))

#align(center)[
  #text(size: 21pt, weight: "bold")[Typst Syntax Manual]
  #linebreak()
  #text(size: 15pt, weight: "bold")[Typst 语法查询手册]
  #linebreak()
  #text(size: 9pt, fill: muted)[A complete bilingual renderable document for Guaxi's built-in Typst engine]
]

#v(8pt)
#callout[设计目标 / Design Goal][
  这份文档是可以直接被 Typst 引擎渲染的完整示例，不依赖网络和外部图片文件。
  It demonstrates Chinese and English mixed text, layout settings, headings,
  table of contents, lists, tables, math, links, references, footnotes, code,
  figures, reusable functions, columns, page breaks, and raw blocks.
]

#outline(title: [目录 / Table of Contents])
#pagebreak()

= 文档结构 / Document Structure <sec-structure>

Typst uses plain text plus lightweight markup. 中文段落可以与 English sentence
自然混排；行内代码如 `#set page(...)` 会保持等宽显示。Typst 中以 `#`
进入代码模式，以 `=` 声明标题，以 `$...$` 编写数学公式。

下面是一段普通段落，用来观察版面、首行缩进、两端对齐和字体回退。
Guaxi 的手册窗口会把左侧源码发送给内置 Typst 渲染引擎，并在右侧显示 PDF。
This makes the manual useful as both a reference and a rendering test page.

== 页面与字体 / Page And Font Settings

页面设置通常放在文档开头：

```typst
#set page(paper: "a4", margin: (x: 1.65cm, y: 1.55cm), numbering: "1 / 1")
#set text(font: ("Noto Serif CJK SC", "Microsoft YaHei", "Arial"), size: 10pt)
#set heading(numbering: "1.1")
```

#callout[排版提示 / Typesetting Tip][
  对中文文档，建议设置 CJK 字体回退列表；对英文技术文档，建议把代码、
  数学和正文保持清晰区分。Use `#set` for global style and `#show` for rules
  that transform matching elements.
]

= 文本语法 / Text Markup

Typst supports *strong text*, _emphasis_, `inline raw code`, smart punctuation,
links like #link("https://typst.app")[Typst Website], and footnotes#footnote[
脚注可以包含中文说明，也可以包含 English explanation. Footnotes are rendered
at the bottom of the page.
].

You can combine English and 中文 in one sentence: 研究笔记 Research Notes are often
written bilingually, especially when equations, package names, and commands are
already in English.

#quote(block: true, attribution: [Guaxi Manual])[
  引用块适合展示摘录、定义和重要说明。A block quote can also carry an
  attribution, which is useful in notes and academic writing.
]

= 列表 / Lists

== 无序列表 / Unordered Lists

- Markdown-like writing habit: simple, readable, and fast.
- 中文项目：支持嵌套内容、行内公式 $a^2 + b^2 = c^2$ 和强调。
  - 子项目 / nested item
  - Another nested item with #tag[inline tag]

== 有序列表 / Ordered Lists

+ 打开 `.typ` 文件。
+ 点击工具栏中的书本图标。
+ 在左侧点击源码行，右侧预览会尝试定位到对应位置。

== 术语列表 / Term Lists

/ Markup: Text-oriented syntax such as headings, lists, and emphasis.
/ Code mode: Expressions that start with `#`, for example `#text(weight: "bold")[...]`.
/ Math mode: Formula syntax written inside dollar signs, for example `$ integral_0^1 x dif x $`.

= 表格 / Tables <sec-table>

#table(
  columns: (1.1fr, 1.2fr, 1.7fr),
  inset: 5pt,
  stroke: rgb("#cbd5e1"),
  align: (left, left, left),
  table.header(
    [语法项目 / Feature],
    [示例 / Syntax],
    [说明 / Purpose],
  ),
  [Heading], [`= Title`], [生成目录结构和可跳转大纲。],
  [Text], [`*bold*`, `_emph_`], [强调、术语和说明性文本。],
  [Math], [`$ x^2 + y^2 $`], [行内公式和展示公式。],
  [Figure], [`#figure(...)`], [图片、图形和标题说明。],
  [Function], [`#let name(args) = ...`], [复用布局片段。],
)

表 @sec-table 展示了常见语法项目。Technically this is a normal Typst table, but
with a header row and fractional columns.

= 数学 / Mathematics

Inline math: $ integral_0^1 x^2 dif x = 1/3 $.

展示公式 / display math:

$ sum_(i=1)^n i = (n (n + 1)) / 2 $

矩阵和向量 / matrix and vector:

$ mat(1, 2; 3, 4) vec(1, 0) = vec(1, 3) $

分段函数 / piecewise function:

$ f(x) := cases(
  0 "if" x < 0,
  x^2 "if" x >= 0,
) $

= 代码块 / Code Blocks

Raw blocks keep code untouched and can carry a language tag:

```typst
#let warning(body) = block(fill: yellow.lighten(80%), inset: 6pt)[body]
#warning[Pay attention to units: 1cm, 12pt, 2em.]
```

```python
def area(radius):
    return 3.14159 * radius ** 2
```

= 图片与图形 / Images And Figures <sec-figure>

为了保证手册在离线环境中一定可渲染，这里使用 Typst 原生图形生成“图片占位”。
真实图片可用 `#image("path/to/image.png", width: 70%)` 插入；如果你在项目目录中放置图片，
相对路径会以当前 `.typ` 文件所在目录为基准解析。

#figure(
  stack(
    spacing: 0pt,
    rect(width: 9cm, height: 34pt, fill: rgb("#dbeafe"), stroke: accent),
    rect(width: 9cm, height: 34pt, fill: rgb("#bfdbfe"), stroke: accent),
    rect(width: 9cm, height: 34pt, fill: rgb("#93c5fd"), stroke: accent),
  ),
  caption: [A generated image-like figure. 这是无需外部文件的图片/图形示例。]
) <fig-generated>

See @fig-generated for a figure reference. 参见 @fig-generated。

= 链接、引用与交叉引用 / Links And References

- 外部链接：#link("https://typst.app/docs/")[Typst Documentation]
- 章节引用：@sec-structure
- 图形引用：@fig-generated
- 标签文本：#tag[draft] #tag[review] #tag[final]

= 函数、变量与 Show Rules / Functions And Rules

#let key-value(key, value) = grid(
  columns: (32%, 1fr),
  gutter: 8pt,
  [#text(weight: "bold")[#key]],
  [#value],
)

#key-value[函数 / Function][`#let name(args) = body` creates reusable content.]
#key-value[变量 / Variable][`#let accent = rgb("#2563eb")` stores reusable values.]
#key-value[规则 / Rule][`#show strong: set text(...)` changes matching elements.]

= 多栏与分页 / Columns And Page Breaks

#columns(2, gutter: 14pt)[
  左栏中文：多栏排版适合摘要、词汇表、并列说明和短篇讲义。
  在手册中使用多栏可以快速检查分页和流式布局。

  Right column English: Columns are content containers. They can include lists:
  - compact
  - readable
  - printable
]

#pagebreak(weak: true)

= 附录 / Appendix

This final section exists to make the document long enough for scrolling,
searching, and source-to-preview navigation. 最后一节用于测试滚动、搜索和定位。

#callout[外部图片写法 / External Image Syntax][
  ```typst
  #figure(
    image("assets/example.png", width: 70%),
    caption: [An external image.]
  )
  ```
  本手册没有引用外部文件，是为了保证分发后无需额外资源也能渲染成功。
]
