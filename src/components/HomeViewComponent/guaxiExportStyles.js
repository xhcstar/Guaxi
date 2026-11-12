// 设计说明：
// 1. 这里集中存放“Markdown 导出为 PDF / HTML”时注入到打印外壳里的 CSS。
// 2. 之前这些样式直接以多段模板字符串硬编码在 GuaxiEditor.vue 的 buildPrintHtml 里，
//    导致那个函数有几百行、几乎无法阅读；拆出来后 buildPrintHtml 只负责拼装结构。
// 3. 拆分只搬运字符串、不改变任何选择器/取值，导出结果与之前保持一致。
import { getActiveCodeThemeId, getCodeThemePalette } from '../../monaco/codeThemes'
import { buildSyntaxTokenCssForPalette } from '../../monaco/syntaxPalette'

// 打印外壳 + highlight.js 浅色主题：
// 预览 worker 已经把代码块渲染成 hljs-* token，导出外壳必须自带一份浅色高亮主题，
// 否则 PDF/HTML 里会只剩代码结构而没有颜色。
export const EXPORT_SHELL_CSS = `
  @page {
    size: A4;
    margin: 16mm;
  }

  html, body {
    margin: 0;
    padding: 0;
    background: #ffffff;
  }

  body {
    font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', monospace;
    color: #000000;
  }

  .export-root {
    box-sizing: border-box;
    padding: 12px 8px 24px 8px;
  }

  .export-content {
    font-size: 16px;
    line-height: 1.6;
  }

  .export-content code,
  .export-content pre {
    font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  }

  .export-content pre {
    background: #f5f5f5;
    padding: 10px 12px;
    border-radius: 4px;
    overflow-x: auto;
  }

  .export-content pre.hljs,
  .export-content .hljs {
    color: #24292f;
    background: #f6f8fa;
  }

  /* chart / excalidraw 导出块。
   * 导出时占位块已经被替换成内联 SVG（见 materializeChartEmbedsForExport），
   * 这里只需要保证「自适应纸宽 + 不会被分页切断 + 不出现屏幕专用的工具条」。
   */
  .export-content .guaxi-chart-block,
  .export-content .guaxi-excalidraw-embed {
    margin: 14px 0;
    padding: 10px;
    border: 1px solid #e1e4e8;
    border-radius: 8px;
    background: #ffffff;
    break-inside: avoid;
    page-break-inside: avoid;
  }

  .export-content .guaxi-chart-block__canvas,
  .export-content .guaxi-excalidraw-embed__stage {
    width: 100%;
    min-height: 0;
  }

  .export-content .guaxi-chart-block svg,
  .export-content .guaxi-excalidraw-embed svg {
    display: block;
    max-width: 100%;
    height: auto;
    margin: 0 auto;
  }

  /* 屏幕上的「查看 / 导出 / 在别处打开」交互在成品里没有意义，直接隐藏。 */
  .export-content .guaxi-chart-block__tools,
  .export-content .guaxi-excalidraw-embed__caption {
    display: none;
  }

  .export-content .guaxi-chart-block__placeholder,
  .export-content .guaxi-chart-block__error,
  .export-content .guaxi-excalidraw-embed__error {
    padding: 12px;
    color: #b42318;
    font-size: 13px;
  }

  .export-content .hljs-doctag,
  .export-content .hljs-keyword,
  .export-content .hljs-meta .hljs-keyword,
  .export-content .hljs-template-tag,
  .export-content .hljs-template-variable,
  .export-content .hljs-type,
  .export-content .hljs-variable.language_ {
    color: #cf222e;
  }

  .export-content .hljs-title,
  .export-content .hljs-title.class_,
  .export-content .hljs-title.class_.inherited__,
  .export-content .hljs-title.function_ {
    color: #8250df;
  }

  .export-content .hljs-attr,
  .export-content .hljs-attribute,
  .export-content .hljs-literal,
  .export-content .hljs-meta,
  .export-content .hljs-number,
  .export-content .hljs-operator,
  .export-content .hljs-selector-attr,
  .export-content .hljs-selector-class,
  .export-content .hljs-selector-id,
  .export-content .hljs-variable {
    color: #0550ae;
  }

  .export-content .hljs-regexp,
  .export-content .hljs-string,
  .export-content .hljs-meta .hljs-string {
    color: #0a3069;
  }

  .export-content .hljs-built_in,
  .export-content .hljs-symbol {
    color: #953800;
  }

  .export-content .hljs-comment,
  .export-content .hljs-code,
  .export-content .hljs-formula {
    color: #6e7781;
  }

  .export-content .hljs-name,
  .export-content .hljs-quote,
  .export-content .hljs-selector-tag,
  .export-content .hljs-selector-pseudo {
    color: #116329;
  }

  .export-content .hljs-subst {
    color: #24292f;
  }

  .export-content .hljs-section {
    color: #0550ae;
    font-weight: 700;
  }

  .export-content table {
    border-collapse: collapse;
    width: 100%;
  }

  .export-content table th,
  .export-content table td {
    border: 1px solid #ddd;
    padding: 6px 8px;
  }

  .export-content img {
    max-width: 100%;
    height: auto;
  }

  @media print {
    body {
      -webkit-print-color-adjust: exact;
      print-color-adjust: exact;
    }

    /* 设计说明：
       PDF（浏览器打印）里 overflow-x: auto 的横向滚动条无法操作，超宽代码行会被直接截断。
       因此打印时让代码块强制软换行、取消横向滚动，保证每一行都完整出现在 PDF 中。 */
    .export-content pre,
    .export-content pre.hljs,
    .export-content pre code,
    .export-content pre.hljs code {
      white-space: pre-wrap;
      word-break: break-word;
      overflow-wrap: anywhere;
      overflow: visible;
    }

    /* 图片 / 图表整块不跨页：剩余空间放不下时整体移到下一页，
       而不是拦腰截断。 */
    .export-content img,
    .export-content .mermaid,
    .export-content .mermaid svg,
    .export-content p.guaxi-media-group,
    .export-content .guaxi-media-group__item,
    .export-content .guaxi-chart-block,
    .export-content .guaxi-excalidraw-embed {
      break-inside: avoid;
      page-break-inside: avoid;
    }

    /* 高过一页的图与图表按内容区高度（A4 正文约 265mm）等比缩小：
       宁可小一号，也不要被分页线切断。 */
    .export-content img,
    .export-content .mermaid svg,
    .export-content .guaxi-chart-block svg,
    .export-content .guaxi-excalidraw-embed svg {
      max-height: 240mm;
    }

    /* 宽度被列宽（或 100%）固定时，max-height 可能把图片压扁；
       contain 保证内容按原比例缩放，宁可留白也不变形。 */
    .export-content img {
      object-fit: contain;
    }

    /* mermaid 的内联样式已经把宽度定成 100%，这里只需要把高度交回内容
       自带的宽高比，再交给 max-height 限高；viewBox 的 preserveAspectRatio
       会保证图形按比例缩放，不会被压扁。 */
    .export-content .mermaid svg {
      height: auto;
      max-width: 100%;
    }

    /* 单图在纸面上不设宽度下限：超高窄图被 max-height 压缩时，
       min-width 会反过来把它拉宽，比例会失真。 */
    .export-content .guaxi-media-group.is-single .guaxi-media-group__image {
      min-width: 0;
    }
  }
`;

// Python 代码单元 / 内联 DataFrame 卡片样式：
// 导出 HTML 里仍可能出现这些 guaxi-pycell-* / guaxi-inline-dataframe 结构，
// 复用编辑器内同源样式保证导出后排版与软件内预览一致。
export const EXPORT_PYCELL_CSS = `
.guaxi-editor .guaxi-pycell-overlay-root {
  position: absolute;
  inset: 0;
  pointer-events: none;
  z-index: 18;
}

.guaxi-editor .guaxi-pycell-overlay-card {
  position: absolute;
  pointer-events: none;
}

.guaxi-editor .guaxi-pycell-overlay-card > .guaxi-pycell-card {
  pointer-events: auto;
}

.guaxi-editor .guaxi-pycell-spacer {
  pointer-events: none;
}

.guaxi-editor .guaxi-pycell-card {
  margin: 4px 0 8px;
  padding: 0;
  border: 1px solid var(--pycell-card-border);
  border-radius: 16px;
  background: var(--pycell-card-bg);
  box-shadow: var(--pycell-card-shadow);
  overflow: hidden;
  pointer-events: auto;
}

.guaxi-editor .guaxi-pycell-card__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 11px 14px 9px;
  border-bottom: 1px solid var(--pycell-header-border);
  background: var(--pycell-header-bg);
}

.guaxi-editor .guaxi-pycell-card__prompt {
  flex: 0 0 auto;
  min-width: 56px;
  font-size: 12px;
  color: var(--pycell-prompt);
  font-weight: 700;
  letter-spacing: 0.02em;
}

.guaxi-editor .guaxi-pycell-card.is-running .guaxi-pycell-card__prompt {
  color: #2563eb;
  animation: guaxi-pycell-prompt-pulse 1.1s ease-in-out infinite;
}

.guaxi-editor .guaxi-pycell-card__title {
  font-size: 14px;
  line-height: 1.3;
  font-weight: 700;
  color: var(--pycell-title);
}

.guaxi-editor .guaxi-pycell-card__meta {
  margin-top: 4px;
  font-size: 11px;
  color: var(--pycell-meta);
}

.guaxi-editor .guaxi-pycell-card__actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 6px;
}

.guaxi-editor .guaxi-pycell-card__btn,
.guaxi-editor .guaxi-inline-dataframe__btn {
  border: 1px solid var(--pycell-btn-border);
  border-radius: 999px;
  background: var(--pycell-btn-bg);
  color: var(--pycell-btn-text);
  font-size: 11px;
  font-weight: 600;
  padding: 5px 10px;
  cursor: pointer;
  pointer-events: auto;
  transition: background 0.16s ease, border-color 0.16s ease, box-shadow 0.16s ease, transform 0.16s ease;
}

.guaxi-editor .guaxi-inline-dataframe__btn {
  font-size: 12px;
}

.guaxi-editor .guaxi-pycell-card__btn:hover,
.guaxi-editor .guaxi-inline-dataframe__btn:hover {
  border-color: var(--pycell-btn-hover-border);
  background: var(--pycell-btn-hover-bg);
  box-shadow: var(--pycell-btn-hover-shadow);
  transform: translateY(-1px);
}

.guaxi-editor .guaxi-pycell-card__btn:focus-visible,
.guaxi-editor .guaxi-inline-dataframe__btn:focus-visible {
  outline: none;
  border-color: var(--pycell-btn-hover-border);
  box-shadow: var(--pycell-btn-focus-ring);
}

.guaxi-editor .guaxi-pycell-card__body {
  display: flex;
  flex-direction: column;
  gap: 0;
}

.guaxi-editor .guaxi-pycell-card__code-shell,
.guaxi-editor .guaxi-pycell-card__panel,
.guaxi-editor .guaxi-pycell-card__rich,
.guaxi-editor .guaxi-inline-dataframe {
  border: none;
  border-radius: 0;
  background: transparent;
}

.guaxi-editor .guaxi-pycell-card__panel.is-collapsed {
  display: none;
}

.guaxi-editor .guaxi-pycell-card__editor {
  width: 100%;
  min-height: 96px;
  resize: none;
  border: none;
  outline: none;
  padding: 12px 14px 14px;
  background: var(--pycell-editor-bg);
  color: var(--pycell-editor-text);
  font-size: 13px;
  line-height: 1.65;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  box-sizing: border-box;
  pointer-events: auto;
  caret-color: #2563eb;
}

.guaxi-editor .guaxi-pycell-card__editor:focus {
  background: var(--pycell-editor-focus-bg);
}

.guaxi-editor .guaxi-pycell-card__panel {
  padding: 10px 14px 14px;
  border-top: 1px solid rgba(15, 23, 42, 0.08);
  color: var(--pycell-panel-text);
  background: var(--pycell-panel-bg);
}

.guaxi-editor .guaxi-pycell-card__panel--output::before,
.guaxi-editor .guaxi-pycell-card__panel--session::before {
  display: block;
  margin-bottom: 8px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.guaxi-editor .guaxi-pycell-card__panel--output::before {
  content: 'Out [ ]';
  color: var(--pycell-output-label);
}

.guaxi-editor .guaxi-pycell-card__panel--session::before {
  content: 'Session';
  color: var(--pycell-session-label);
}

.guaxi-editor .guaxi-pycell-card__stdout,
.guaxi-editor .guaxi-pycell-card__rich--markdown {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.65;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  color: var(--pycell-output-text);
  background: var(--pycell-output-bg);
  border: 1px solid var(--pycell-output-border);
  border-radius: 12px;
  padding: 12px 13px;
}

.guaxi-editor .guaxi-pycell-card__image {
  display: block;
  max-width: 100%;
  height: auto;
  border-radius: 12px;
}

.guaxi-editor .guaxi-pycell-card__rich--svg svg {
  max-width: 100%;
  height: auto;
}

.guaxi-editor .guaxi-pycell-card__empty {
  font-size: 12px;
  color: var(--pycell-empty);
}

.guaxi-editor .guaxi-pycell-card__var + .guaxi-pycell-card__var {
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px solid rgba(15, 23, 42, 0.08);
}

.guaxi-editor .guaxi-pycell-card__var-main {
  display: flex;
  align-items: center;
  gap: 8px;
}

.guaxi-editor .guaxi-pycell-card__var-name {
  font-weight: 700;
  color: var(--pycell-var-name);
}

.guaxi-editor .guaxi-pycell-card__var-type {
  color: var(--pycell-var-type);
  font-size: 12px;
}

.guaxi-editor .guaxi-pycell-card__var-preview,
.guaxi-editor .guaxi-inline-dataframe__meta {
  margin-top: 4px;
  color: var(--pycell-var-preview);
  font-size: 12px;
  line-height: 1.5;
  word-break: break-word;
}

.guaxi-editor .guaxi-inline-dataframe {
  overflow: hidden;
}

.guaxi-editor .guaxi-inline-dataframe__toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 14px;
  border-bottom: 1px solid rgba(15, 23, 42, 0.08);
}

.guaxi-editor .guaxi-inline-dataframe__actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}

.guaxi-editor .guaxi-inline-dataframe__viewport {
  overflow: auto;
}

.guaxi-editor .guaxi-inline-dataframe__table {
  width: 100%;
  border-collapse: collapse;
}

.guaxi-editor .guaxi-inline-dataframe__table th,
.guaxi-editor .guaxi-inline-dataframe__table td {
  padding: 8px 10px;
  border-bottom: 1px solid rgba(15, 23, 42, 0.08);
  text-align: left;
  white-space: nowrap;
}

.guaxi-editor .guaxi-inline-dataframe__table th {
  background: rgba(226, 232, 240, 0.48);
  font-weight: 700;
}

:root[data-guaxi-theme='dark'] .guaxi-editor .guaxi-inline-dataframe__table th {
  background: rgba(51, 65, 85, 0.72);
}
`;

// 导出文件必须自足（发出去、换台机器打开都要有颜色），所以把当前代码主题**亮色**那套
// highlight.js 样式内联进去。选亮色是因为导出多用于打印/分享，深底文档在这些场景不合适。
//
// 上游样式表是全局选择器，必须限定在 .export-content 下，否则会漏到导出外壳之外的元素上。
// 只处理普通规则：@media / @keyframes 这类 at-rule 直接跳过——导出外壳自己有打印适配，
// 而给 at-rule 加前缀会产出非法 CSS。
const scopeExportHljsSelectors = (css) => {
  const out = []
  for (const chunk of String(css || '').replace(/\/\*[\s\S]*?\*\//g, '').split('}')) {
    const brace = chunk.indexOf('{')
    if (brace < 0) continue
    const selectors = chunk.slice(0, brace).split(',').map((item) => item.trim()).filter(Boolean)
    if (!selectors.length || selectors.some((item) => item.startsWith('@'))) continue
    out.push(`${selectors.map((item) => `.export-content ${item}`).join(',')}${chunk.slice(brace)}}`)
  }
  return out.join('\n')
}

/**
 * 当前代码主题亮色版的内联样式。追加在 EXPORT_SHELL_CSS 之后：
 * 同特异性时靠顺序取胜，覆盖外壳自带的那份亮色兜底。
 *
 * 取的是色板（而不是上游样式表原样）：导出 HTML 里仍是 `.hljs-*` 类名，
 * 但颜色必须和软件内预览、编辑器一致，否则导出的 PDF 会是另一套配色。
 */
export const buildExportCodeThemeCss = () =>
  scopeExportHljsSelectors(
    buildSyntaxTokenCssForPalette(getCodeThemePalette(getActiveCodeThemeId(), 'light')),
  )

/**
 * pandoc/skylighting 的代码配色（`--syntax-highlighting=<file>.theme`）。
 *
 * Word/ODT/EPUB 走 pandoc，默认用 pygments 配色，和软件内预览是两套颜色。
 * 这里把同一份语义色板（当前代码主题的亮色版）映射到 skylighting 的 token 类型，
 * 生成的 .theme 让成品代码块与预览保持一致。
 */
export const buildPandocSyntaxTheme = () => {
  const palette = getCodeThemePalette(getActiveCodeThemeId(), 'light') || {}
  const pick = (...buckets) => {
    for (const bucket of buckets) {
      if (palette[bucket]) return palette[bucket]
    }
    return palette.base || '#383a42'
  }
  const style = (buckets, extra = {}) => ({
    'text-color': pick(...buckets),
    'background-color': null,
    bold: false,
    italic: false,
    underline: false,
    ...extra,
  })

  // 键名必须是 skylighting 认识的 token 类型（见 pandoc --print-highlight-style）。
  const textStyles = {
    Alert: style(['tag', 'keyword']),
    Annotation: style(['decorator', 'macro']),
    Attribute: style(['attribute', 'property']),
    BaseN: style(['number']),
    BuiltIn: style(['builtin']),
    Char: style(['string']),
    Comment: style(['comment'], { italic: true }),
    CommentVar: style(['comment']),
    Constant: style(['constant', 'boolean']),
    ControlFlow: style(['keyword']),
    DataType: style(['type', 'class']),
    DecVal: style(['number']),
    Documentation: style(['comment'], { italic: true }),
    Error: style(['tag', 'keyword']),
    Extension: style(['decorator', 'keyword']),
    Float: style(['number']),
    Function: style(['function', 'method']),
    Import: style(['macro', 'keyword']),
    Information: style(['base']),
    Keyword: style(['keyword']),
    Normal: style(['base']),
    Operator: style(['operator']),
    Other: style(['base']),
    Preprocessor: style(['macro', 'keyword']),
    RegionMarker: style(['label', 'keyword']),
    SpecialChar: style(['escape', 'string']),
    SpecialString: style(['regexp', 'string']),
    String: style(['string']),
    Variable: style(['variable', 'parameter']),
    VerbatimString: style(['string']),
    Warning: style(['keyword']),
  }

  return {
    'text-color': pick('base'),
    'background-color': null,
    'line-number-color': '#aaaaaa',
    'line-number-background-color': null,
    'text-styles': textStyles,
  }
}

/**
 * 导出 HTML/PDF 里的图片排版样式。
 *
 * 为什么不直接复用 guaxiMarkdownExtras.css：
 * 1. 那份是应用内的全局样式，导出文件是独立 HTML，拿不到应用的 CSS 变量与主题类；
 * 2. 导出只有一套浅色打印稿，宽度上限/圆角需要按用户设置**当时的值**写死，
 *    而不是留 var() —— 导出文件被单独打开时没有变量可解析。
 *
 * 结构与应用内保持一致：p.guaxi-media-group（单图 is-single / 多图 is-gallery）+
 * .guaxi-media-group__item + img.guaxi-media-group__image，
 * 由调用方在导出前跑一次 enhancePreviewHtmlWithMediaGroups 得到。
 */
export const buildExportFigureCss = (options = {}) => {
  const maxWidth = Number(options.maxWidth)
  const radius = Number(options.radius)
  const safeMaxWidth = Number.isFinite(maxWidth) && maxWidth > 0 ? Math.round(maxWidth) : 1400
  const safeRadius = Number.isFinite(radius) && radius >= 0 ? Math.round(radius) : 12
  const showCaption = !!options.showCaption

  // 图注：导出没有根节点类可挂，直接由函数决定是否输出这段规则。
  const captionCss = showCaption
    ? `
  .export-content .guaxi-media-group__item[data-gx-caption]:not([data-gx-caption=""]) {
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  .export-content .guaxi-media-group__item[data-gx-caption]:not([data-gx-caption=""])::after {
    content: attr(data-gx-caption);
    display: block;
    width: 100%;
    font-size: 13px;
    line-height: 1.55;
    text-align: center;
    color: #6b7280;
    overflow-wrap: anywhere;
  }
`
    : ''

  return `
  /* ---- Markdown 图片 ---- */
  .export-content p.guaxi-media-group {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    align-items: stretch;
    gap: 16px;
    margin: 24px 0;
    /* 覆盖上方 .export-content img 的 max-width，改为由媒体项控制宽度。 */
    text-align: center;
  }

  .export-content .guaxi-media-group__item {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 0;
    margin: 0;
    padding: 0;
    border: none;
    background: transparent;
  }

  .export-content .guaxi-media-group.is-single > .guaxi-media-group__item {
    flex: 0 1 auto;
    width: 100%;
    /* 不放大：原图比上限窄就保持原始尺寸。 */
    max-width: min(100%, ${safeMaxWidth}px);
    margin-left: auto;
    margin-right: auto;
  }

  /* 两张及以上平分一行：列宽由容器决定，两张不同尺寸的截图才会左右边缘对齐。
     不再写 420px 上限——那会让并排的图缩成窄条，和页面上「图比字小一号」是同一个问题。
     用 grid 而不是 flex：flex 的 1 1 0 会让 3 张以上排成一行窄条，而应用内是
     「3 张一行 / 4 张 2×2」，导出稿要与屏幕上看到的一致。 */
  .export-content .guaxi-media-group.is-gallery {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(240px, 100%), 1fr));
    align-items: stretch;
  }

  .export-content .guaxi-media-group.is-gallery[data-guaxi-media-count='2'] {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .export-content .guaxi-media-group.is-gallery[data-guaxi-media-count='3'] {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .export-content .guaxi-media-group.is-gallery[data-guaxi-media-count='4'] {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .export-content .guaxi-media-group.is-gallery > .guaxi-media-group__item {
    width: 100%;
    max-width: 100%;
  }

  .export-content .guaxi-media-group__image,
  .export-content .guaxi-media-group__item img {
    display: block;
    width: 100%;
    max-width: 100%;
    height: auto;
    margin: 0;
    /* 打印稿里 1px 描边能把浅色截图从白纸上收出轮廓。 */
    border: 1px solid #e5e7eb;
    border-radius: ${safeRadius}px;
    object-fit: contain;
  }

  /* 单图按原始宽度显示，只给一个 460px 下限：截图小图在打印稿里同样需要看得清。
     导出稿没有工具条包裹层，图片的包含块宽度是确定的，所以这里可以直接用百分比夹取。 */
  .export-content .guaxi-media-group.is-single .guaxi-media-group__image {
    width: auto;
    min-width: min(100%, 460px);
  }
${captionCss}
  @media print {
    /* 打印时不让一组图跨页撕开。 */
    .export-content p.guaxi-media-group {
      break-inside: avoid;
      page-break-inside: avoid;
    }
  }
`
}
