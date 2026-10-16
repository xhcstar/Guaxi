// 设计说明：
// 1. tree-sitter-highlight 返回的是各语言 query 的 capture 名称，例如 `function.call`、`variable.member`。
// 2. Monaco semantic token、Markdown fence decoration、Notebook cell decoration 必须先归一化到同一套 Guaxi token 类型。
// 3. 这里同时集中维护明暗主题色值和 CSS class 映射，避免不同入口各自解释 capture 后出现颜色漂移。

export const GUAXI_SYNTAX_TOKEN_TYPES = [
  'comment',
  'string',
  'keyword',
  'number',
  'regexp',
  'operator',
  'punctuation',
  'type',
  'function',
  'method',
  'property',
  'variable',
  'parameter',
  'namespace',
  'class',
  'boolean',
  'constant',
  'attribute',
  'builtin',
  'macro',
  'decorator',
  'label',
]

export const GUAXI_SYNTAX_TOKEN_COLORS = {
  light: {
    base: '#383a42',
    comment: '#6a737d',
    string: '#22863a',
    keyword: '#a626a4',
    number: '#986801',
    boolean: '#986801',
    regexp: '#007c89',
    operator: '#007c89',
    punctuation: '#383a42',
    type: '#6f42c1',
    class: '#6f42c1',
    function: '#0969da',
    method: '#007c89',
    property: '#007c89',
    parameter: '#9a6700',
    variable: '#0550ae',
    constant: '#986801',
    namespace: '#6f42c1',
    macro: '#a626a4',
    tag: '#e45649',
    attribute: '#9a6700',
    builtin: '#0969da',
    decorator: '#a626a4',
    label: '#a626a4',
  },
  dark: {
    base: '#abb2bf',
    comment: '#5c6370',
    string: '#98c379',
    keyword: '#c678dd',
    number: '#d19a66',
    boolean: '#d19a66',
    regexp: '#56b6c2',
    operator: '#56b6c2',
    punctuation: '#abb2bf',
    type: '#e5c07b',
    class: '#e5c07b',
    function: '#61afef',
    method: '#56b6c2',
    property: '#56b6c2',
    parameter: '#d19a66',
    variable: '#d7deea',
    constant: '#d19a66',
    namespace: '#e5c07b',
    macro: '#c678dd',
    tag: '#e06c75',
    attribute: '#d19a66',
    builtin: '#56b6c2',
    decorator: '#c678dd',
    label: '#c678dd',
  },
}

const GUAXI_SYNTAX_CLASS_SUFFIX_BY_TYPE = {
  punctuation: 'delimiter',
  label: 'keyword',
}

export const getGuaxiSyntaxTokenPriority = (tokenType) => {
  switch (tokenType) {
    case 'method':
    case 'property':
      return 80
    case 'function':
      return 60
    case 'variable':
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

export const mapTreeSitterCaptureToGuaxiToken = (tokenType, options = {}) => {
  const type = String(tokenType || '').toLowerCase()
  if (!type) return ''

  let mapped = ''
  if (type.includes('comment')) mapped = 'comment'
  else if (type.includes('string.escape') || type.includes('escape')) mapped = 'string'
  else if (type.includes('string')) mapped = 'string'
  else if (type.includes('keyword.operator')) mapped = 'operator'
  else if (type.includes('keyword') || type.includes('storage') || type.includes('modifier')) mapped = 'keyword'
  else if (type.includes('number') || type.includes('constant.numeric')) mapped = 'number'
  else if (type.includes('boolean')) mapped = 'boolean'
  else if (type.includes('regexp')) mapped = 'regexp'
  else if (type.includes('operator')) mapped = 'operator'
  else if (type.includes('punctuation') || type.includes('delimiter') || type.includes('bracket')) mapped = 'punctuation'
  else if (type.includes('constructor')) mapped = 'class'
  else if (type.includes('class')) mapped = 'class'
  else if (type.includes('type') || type.includes('interface') || type.includes('enum') || type.includes('struct')) mapped = 'type'
  else if (type.includes('variable.parameter') || type.includes('parameter') || type.includes('argument')) mapped = 'parameter'
  else if (type.includes('variable.member') || type.includes('property') || type.includes('member') || type.includes('field')) mapped = 'property'
  else if (type.includes('function.method') || type.includes('method')) mapped = 'method'
  else if (type.includes('function')) mapped = 'function'
  else if (type.includes('variable.builtin') || type.includes('builtin') || type.includes('support')) mapped = 'builtin'
  else if (type.includes('macro')) mapped = 'macro'
  else if (type.includes('module') || type.includes('namespace') || type.includes('package')) mapped = 'namespace'
  else if (type.includes('tag')) mapped = 'class'
  else if (type.includes('attribute')) mapped = 'attribute'
  else if (type.includes('enummember') || type.includes('event') || type.includes('constant')) mapped = 'constant'
  else if (type.includes('decorator') || type.includes('annotation')) mapped = 'decorator'
  else if (type.includes('label')) mapped = 'label'
  else if (type.includes('variable') || type.includes('identifier')) mapped = 'variable'
  else mapped = 'variable'

  if (
    mapped === 'function' &&
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
// 兼容两种入参是为了不让 7 处既有调用点一起改签名。
const resolveSyntaxPalette = (themeOrPalette) => {
  if (themeOrPalette && typeof themeOrPalette === 'object') return themeOrPalette
  return GUAXI_SYNTAX_TOKEN_COLORS[themeOrPalette] || GUAXI_SYNTAX_TOKEN_COLORS.light
}

export const buildGuaxiMonacoThemeRules = (themeOrPalette) => {
  const palette = resolveSyntaxPalette(themeOrPalette)
  const tokenMap = {
    comment: 'comment',
    keyword: 'keyword',
    string: 'string',
    number: 'number',
    regexp: 'regexp',
    type: 'type',
    delimiter: 'punctuation',
    variable: 'variable',
    parameter: 'parameter',
    function: 'function',
    constant: 'constant',
    class: 'class',
    identifier: 'base',
    tag: 'tag',
    'attribute.name': 'attribute',
    'meta.embedded': 'comment',
    operator: 'operator',
    punctuation: 'punctuation',
    'punctuation.definition': 'punctuation',
    'punctuation.bracket': 'punctuation',
    property: 'property',
    method: 'method',
    'function.declaration': 'function',
    'function.defaultLibrary': 'function',
    'parameter.declaration': 'parameter',
    'variable.readonly': 'constant',
    'variable.defaultLibrary': 'builtin',
    'type.defaultLibrary': 'type',
    'class.defaultLibrary': 'class',
    interface: 'type',
    enum: 'type',
    enumMember: 'constant',
    namespace: 'namespace',
    typeParameter: 'type',
    struct: 'type',
    macro: 'macro',
    decorator: 'decorator',
    label: 'label',
    'keyword.control': 'keyword',
    'keyword.defaultLibrary': 'keyword',
    'string.escape': 'operator',
    'number.float': 'number',
    boolean: 'boolean',
  }

  return Object.entries(tokenMap)
    .filter(([, colorKey]) => !!palette[colorKey])
    .map(([token, colorKey]) => ({
      token,
      foreground: String(palette[colorKey]).replace(/^#/, ''),
    }))
}

export const buildGuaxiSemanticTokenColors = (themeOrPalette) => {
  const palette = resolveSyntaxPalette(themeOrPalette)
  return {
    comment: palette.comment,
    string: palette.string,
    keyword: palette.keyword,
    number: palette.number,
    regexp: palette.regexp,
    operator: palette.operator,
    punctuation: palette.punctuation,
    'punctuation.definition': palette.punctuation,
    'punctuation.bracket': palette.punctuation,
    class: palette.class,
    enum: palette.type,
    interface: palette.type,
    type: palette.type,
    typeParameter: palette.type,
    namespace: palette.namespace,
    struct: palette.type,
    function: palette.function,
    method: palette.method,
    property: palette.property,
    parameter: palette.parameter,
    'function.declaration': palette.function,
    'function.defaultLibrary': palette.function,
    'parameter.declaration': palette.parameter,
    variable: palette.variable,
    constant: palette.constant,
    attribute: palette.attribute,
    builtin: palette.builtin,
    'variable.readonly': palette.constant,
    'variable.defaultLibrary': palette.builtin,
    enumMember: palette.constant,
    boolean: palette.boolean,
    macro: palette.macro,
    decorator: palette.decorator,
    label: palette.label,
    'string.escape': palette.operator,
  }
}
