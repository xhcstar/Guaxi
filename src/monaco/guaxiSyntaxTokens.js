// 设计说明：
// 1. tree-sitter-highlight 返回的是各语言 query 的 capture 名称，例如 `function.call`、`variable.member`。
// 2. Monaco semantic token、Markdown fence decoration、Notebook cell decoration、预览 `.hljs-*` 必须落到同一套桶。
// 3. 桶的色值只在 codeThemes/syntaxPalette 里维护；这里只负责「capture → 桶」和「桶 → Monaco 规则」的映射。

import { GUAXI_SYNTAX_BUCKETS } from './syntaxPalette.js'

// `base` 是正文兜底色，不会作为 token 类型出现，所以不进 legend。
export const GUAXI_SYNTAX_TOKEN_TYPES = GUAXI_SYNTAX_BUCKETS.filter((bucket) => bucket !== 'base')

const GUAXI_SYNTAX_CLASS_SUFFIX_BY_TYPE = {
  punctuation: 'delimiter',
}

export const getGuaxiSyntaxTokenPriority = (tokenType) => {
  switch (tokenType) {
    case 'property':
      return 80
    case 'method':
      return 78
    case 'class':
    case 'type':
      return 70
    case 'function':
      return 60
    case 'variable':
    case 'parameter':
      return 40
    default:
      return 50
  }
}

export const isDottedMemberAtOffset = (sourceText = '', startOffset = 0) => {
  let cursor = Number(startOffset || 0) - 1
  const text = String(sourceText || '')
  while (cursor >= 0 && /\s/.test(text[cursor] || '')) cursor -= 1
  return text[cursor] === '.'
}

// Markdown 正文的 capture 名。
//
// **当前在用的 grammar 是 tree-sitter-md，它产出的是 `text.*`**：
//   标题 text.title、斜体 text.emphasis、加粗 text.strong、行内代码 text.literal、
//   链接文字 text.reference、链接地址 text.uri。
// `markup.*` 是另一套命名习惯（nvim-treesitter 的写法），这里一并保留，换 grammar 时不至于再漏一次。
//
// 之前这张表两种名字都写成了 markup.*：后端白名单里没有 text.*，capture 在
// `HighlightConfiguration::configure` 阶段就被整条丢弃，于是编辑器里 Markdown 正文
// **一条高亮都没有**——不是"颜色选得不对"，是"根本没命中"。
const MARKDOWN_CAPTURE_TO_TOKEN = {
  // tree-sitter-md（当前）
  'text.title': 'class',
  'text.emphasis': 'decorator',
  'text.strong': 'keyword',
  // text.literal 同时覆盖"行内代码"与"围栏代码块的外层节点"，
  // 围栏内容本身由注入的语言着色，这里影响的是标记行与行内代码，取 string 与既有语义一致。
  'text.literal': 'string',
  'text.reference': 'function',
  'text.uri': 'function',
  // 兼容另一套命名
  'markup.heading': 'class',
  'markup.bold': 'keyword',
  'markup.italic': 'decorator',
  'markup.raw': 'string',
  'markup.raw.block': 'string',
  'markup.raw.inline': 'string',
  'markup.quote': 'comment',
  'markup.list': 'punctuation',
  'markup.link': 'function',
  'markup.link.url': 'function',
}

/**
 * tree-sitter capture → 语义桶。
 *
 * 这里的细分程度直接决定「参数 / 变量 / 成员 / 常量 / 类型」能不能分开：
 * 之前 `variable.parameter` 和 `variable` 都落到同一个桶，参数自然和变量同色。
 */
export const mapTreeSitterCaptureToGuaxiToken = (tokenType, options = {}) => {
  const type = String(tokenType || '').toLowerCase()
  if (!type) return ''

  const markdownToken = MARKDOWN_CAPTURE_TO_TOKEN[type]
  if (markdownToken) return markdownToken

  let mapped = ''
  if (type.includes('comment')) mapped = 'comment'
  else if (type.includes('string.escape') || type.includes('escape') || type.includes('interpolation')) mapped = 'escape'
  else if (type.includes('regexp')) mapped = 'regexp'
  else if (type.includes('string')) mapped = 'string'
  else if (type.includes('keyword.operator')) mapped = 'operator'
  else if (type.includes('keyword.directive') || type.includes('preproc') || type.includes('preprocessor')) mapped = 'macro'
  // function.macro 必须排在 function 之前，否则 Rust 的 `println!` 会被当成普通函数
  else if (type.includes('macro')) mapped = 'macro'
  else if (type.includes('keyword') || type.includes('storage') || type.includes('modifier')) mapped = 'keyword'
  else if (type.includes('number') || type.includes('constant.numeric')) mapped = 'number'
  else if (type.includes('boolean')) mapped = 'boolean'
  else if (type.includes('operator')) mapped = 'operator'
  else if (type.includes('punctuation') || type.includes('delimiter') || type.includes('bracket')) mapped = 'punctuation'
  else if (type.includes('constructor')) mapped = 'class'
  else if (type.includes('class')) mapped = 'class'
  else if (type.includes('enummember') || type.includes('enum.member')) mapped = 'constant'
  else if (type.includes('interface') || type.includes('enum') || type.includes('struct') || type.includes('type')) mapped = 'type'
  else if (type.includes('parameter') || type.includes('argument')) mapped = 'parameter'
  else if (type.includes('variable.member') || type.includes('property') || type.includes('member') || type.includes('field')) mapped = 'property'
  else if (type.includes('function.method') || type.includes('method')) mapped = 'method'
  else if (type.includes('function')) mapped = 'function'
  else if (type.includes('variable.builtin') || type === 'self' || type === 'this') mapped = 'variable-language'
  else if (type.includes('builtin') || type.includes('support')) mapped = 'builtin'
  else if (type.includes('attribute')) mapped = 'attribute'
  else if (type.includes('module') || type.includes('namespace') || type.includes('package')) mapped = 'namespace'
  else if (type.includes('tag')) mapped = 'tag'
  else if (type.includes('event') || type.includes('constant')) mapped = 'constant'
  else if (type.includes('decorator') || type.includes('annotation')) mapped = 'decorator'
  else if (type.includes('label')) mapped = 'label'
  else if (type.includes('variable') || type.includes('identifier')) mapped = 'variable'
  else mapped = 'variable'

  // `obj.method()` 里的 method 只有在带点号时才成立；不带点号的 function 保持原样。
  if (
    (mapped === 'function' || mapped === 'method') &&
    options?.sourceText != null &&
    Number.isFinite(Number(options?.startOffset)) &&
    isDottedMemberAtOffset(options.sourceText, Number(options.startOffset))
  ) {
    return 'method'
  }

  return mapped
}

export const getGuaxiSyntaxClassForToken = (tokenType, prefix = 'md-fence-token') => {
  const normalized = String(tokenType || '').trim()
  if (!normalized) return ''
  const suffix = GUAXI_SYNTAX_CLASS_SUFFIX_BY_TYPE[normalized] || normalized
  return `${prefix}-${suffix}`
}

export const getGuaxiSyntaxClassForCapture = (tokenType, options = {}) => {
  const mapped = mapTreeSitterCaptureToGuaxiToken(tokenType, options)
  return getGuaxiSyntaxClassForToken(mapped, options?.prefix || 'md-fence-token')
}

// 允许传 'light'/'dark'（历史调用方式）或直接传调色板对象（代码主题功能需要按主题取色）。
const resolveSyntaxPalette = (themeOrPalette) => {
  if (themeOrPalette && typeof themeOrPalette === 'object') return themeOrPalette
  return {}
}

/**
 * Monaco 词法 token 名 → 语义桶。
 *
 * 这张表对应的是各语言 Monaco Monarch 分词器产出的 token 名，
 * 覆盖不到的类型会掉回基础主题色（表现为「这个文件不跟代码主题走」），所以宁可写全。
 */
const MONACO_TOKEN_TO_BUCKET = {
  comment: 'comment',
  'comment.doc': 'comment',
  quote: 'comment',

  keyword: 'keyword',
  'keyword.control': 'keyword',
  'keyword.flow': 'keyword',
  'keyword.directive': 'macro',
  'keyword.operator': 'operator',
  literal: 'boolean',
  constant: 'constant',
  'constant.numeric': 'number',

  string: 'string',
  'string.escape': 'escape',
  'string.regexp': 'regexp',
  regexp: 'regexp',
  number: 'number',
  'number.float': 'number',
  'number.hex': 'number',
  boolean: 'boolean',

  type: 'type',
  'type.identifier': 'type',
  typeParameter: 'type',
  struct: 'type',
  interface: 'type',
  enum: 'type',
  'namespace.identifier': 'namespace',
  namespace: 'namespace',
  module: 'namespace',

  class: 'class',
  'class.identifier': 'class',
  constructor: 'class',
  enumMember: 'constant',

  function: 'function',
  'function.identifier': 'function',
  'function.declaration': 'function',
  'function.call': 'function',
  'function.defaultLibrary': 'builtin',
  method: 'method',
  'method.call': 'method',
  property: 'property',
  'property.name': 'property',
  field: 'property',

  parameter: 'parameter',
  'parameter.declaration': 'parameter',
  variable: 'variable',
  // Monarch 的 identifier 是「没被更具体规则命中的标识符」，给正文色；
  // 真正按变量上色的是语义 token（tree-sitter），否则 JSON/YAML 这类文件会整片变红。
  identifier: 'base',
  'variable.predefined': 'variable-language',
  'variable.readonly': 'constant',
  'variable.defaultLibrary': 'builtin',
  'variable.language': 'variable-language',

  attribute: 'attribute',
  'attribute.name': 'attribute',
  'attribute.value': 'string',
  tag: 'tag',
  decorator: 'decorator',
  annotation: 'decorator',
  label: 'label',
  meta: 'macro',
  metatag: 'macro',
  macro: 'macro',

  delimiter: 'punctuation',
  'delimiter.bracket': 'punctuation',
  'delimiter.parenthesis': 'punctuation',
  'delimiter.curly': 'punctuation',
  punctuation: 'punctuation',
  'punctuation.definition': 'punctuation',
  'punctuation.bracket': 'punctuation',
  operator: 'operator',
  'string.escape.character': 'escape',

  emphasis: 'decorator',
  strong: 'keyword',
  'meta.embedded': 'comment',
}

export const buildGuaxiMonacoThemeRules = (themeOrPalette) => {
  const palette = resolveSyntaxPalette(themeOrPalette)
  return Object.entries(MONACO_TOKEN_TO_BUCKET)
    .filter(([, bucket]) => !!palette[bucket])
    .map(([token, bucket]) => ({
      token,
      foreground: String(palette[bucket]).replace(/^#/, ''),
    }))
}

export const buildGuaxiSemanticTokenColors = (themeOrPalette) => {
  const palette = resolveSyntaxPalette(themeOrPalette)
  const colors = {}
  for (const bucket of GUAXI_SYNTAX_TOKEN_TYPES) {
    if (palette[bucket]) colors[bucket] = palette[bucket]
  }

  // Monaco / VSCode 语义 token 的固定名字，必须映射到我们的桶，
  // 否则大模型补全、LSP 语义高亮会退回基础主题色。
  const aliases = {
    'function.declaration': 'function',
    'function.defaultLibrary': 'builtin',
    'parameter.declaration': 'parameter',
    variable: 'variable',
    'variable.readonly': 'constant',
    'variable.defaultLibrary': 'builtin',
    'variable.language': 'variable-language',
    enumMember: 'constant',
    struct: 'type',
    interface: 'type',
    typeParameter: 'type',
    'punctuation.definition': 'punctuation',
    'punctuation.bracket': 'punctuation',
    'string.escape': 'escape',
    property: 'property',
    method: 'method',
  }
  for (const [alias, bucket] of Object.entries(aliases)) {
    if (palette[bucket]) colors[alias] = palette[bucket]
  }
  return colors
}
