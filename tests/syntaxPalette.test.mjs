import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { GUAXI_CODE_THEME_ROLES } from '../src/monaco/codeThemeRoles.js'
import { buildTokenHtml } from '../src/monaco/tokenHtml.js'
import { resolvePreviewLanguage } from '../src/monaco/previewLanguages.js'
import {
  GUAXI_SYNTAX_BUCKETS,
  buildSyntaxTokenCss,
  buildSyntaxTokenCssForPalette,
  resolveSyntaxPalette,
  stripThemeColors,
} from '../src/monaco/syntaxPalette.js'

// 与 codeThemes.GUAXI_CODE_THEMES 一一对应；roles 只给人工挑色的主题。
const THEME_FILES = [
  ['atom-one', 'node_modules/highlight.js/styles/atom-one-light', 'node_modules/highlight.js/styles/atom-one-dark'],
  ['github', 'node_modules/highlight.js/styles/github', 'node_modules/highlight.js/styles/github-dark'],
  ['panda', 'node_modules/highlight.js/styles/panda-syntax-light', 'node_modules/highlight.js/styles/panda-syntax-dark'],
  ['tokyo-night', 'node_modules/highlight.js/styles/tokyo-night-light', 'node_modules/highlight.js/styles/tokyo-night-dark'],
  ['stackoverflow', 'node_modules/highlight.js/styles/stackoverflow-light', 'node_modules/highlight.js/styles/stackoverflow-dark'],
  ['rose-pine', 'node_modules/highlight.js/styles/rose-pine-dawn', 'node_modules/highlight.js/styles/rose-pine'],
  ['monokai', 'src/monaco/codeStyles/monokai-light', 'src/monaco/codeStyles/monokai-dark'],
  ['gruvbox', 'src/monaco/codeStyles/gruvbox-light', 'src/monaco/codeStyles/gruvbox-dark'],
]

const resolveTheme = (id, file, appearance) => resolveSyntaxPalette(
  readThemeCss(file),
  appearance,
  GUAXI_CODE_THEME_ROLES[id]?.[appearance],
)

const readThemeCss = (file) => readFileSync(new URL(`../${file}.css`, import.meta.url), 'utf8')

test('每套主题的明暗色板都覆盖全部语义桶', () => {
  for (const [id, light, dark] of THEME_FILES) {
    for (const [appearance, file] of [['light', light], ['dark', dark]]) {
      const palette = resolveTheme(id, file, appearance)
      for (const bucket of GUAXI_SYNTAX_BUCKETS) {
        assert.match(palette[bucket] || '', /^#[0-9a-f]{6}$/, `${id}/${appearance} 缺少 ${bucket}`)
      }
    }
  }
})

test('细化的意义在于同色不撞车：除正文/标点外每个桶颜色唯一', () => {
  for (const [id, light, dark] of THEME_FILES) {
    for (const [appearance, file] of [['light', light], ['dark', dark]]) {
      const palette = resolveTheme(id, file, appearance)
      // 标点、裸变量故意与正文同色，正文本身不参与「桶」的区分。
      const seen = new Map()
      for (const bucket of GUAXI_SYNTAX_BUCKETS) {
        if (bucket === 'base' || bucket === 'punctuation' || bucket === 'variable') continue
        const color = palette[bucket]
        assert.ok(!seen.has(color), `${id}/${appearance}: ${bucket} 与 ${seen.get(color)} 同色 ${color}`)
        seen.set(color, bucket)
      }
    }
  }
})

test('参数/变量/成员/常量/类方法这些曾经撞色的桶必须分开', () => {
  const palette = resolveSyntaxPalette(readThemeCss('node_modules/highlight.js/styles/atom-one-dark'), 'dark')
  const pairs = [
    ['parameter', 'variable'],
    ['property', 'variable'],
    ['method', 'function'],
    ['constant', 'number'],
    ['type', 'class'],
    ['macro', 'keyword'],
  ]
  for (const [left, right] of pairs) {
    assert.notEqual(palette[left], palette[right], `${left} 与 ${right} 仍是同色`)
  }
})

test('预览样式表由色板生成，且上游样式表剥掉颜色后不再残留 color 声明', () => {
  const css = buildSyntaxTokenCss()
  for (const bucket of GUAXI_SYNTAX_BUCKETS) {
    assert.ok(css.includes(`var(--guaxi-syntax-${bucket})`), `预览样式表缺少 ${bucket}`)
  }

  const stripped = stripThemeColors(readThemeCss('node_modules/highlight.js/styles/atom-one-dark'))
  assert.ok(stripped.includes('background'), '背景声明必须保留')
  assert.ok(!/(^|[;{]\s*)color\s*:/.test(stripped), 'color 声明必须全部剥掉')
})

test('预览走 tree-sitter 后用的 guaxi-tok-* 类也必须有色', () => {
  const css = buildSyntaxTokenCss()
  for (const bucket of GUAXI_SYNTAX_BUCKETS) {
    assert.ok(css.includes(`.guaxi-tok-${bucket}{`), `缺少 .guaxi-tok-${bucket}`)
  }
})

test('导出样式把色板写成字面值，不再依赖页面变量', () => {
  const palette = resolveSyntaxPalette(readThemeCss('node_modules/highlight.js/styles/atom-one-light'), 'light')
  const css = buildSyntaxTokenCssForPalette(palette)
  assert.ok(css.includes(palette.keyword), '导出样式必须带具体色值')
  assert.ok(!css.includes('var(--guaxi-syntax-'), '导出样式不能依赖 CSS 变量')
})

test('token span 拼 HTML：多字节字符按 UTF-16 列换算，不偏色', () => {
  const html = buildTokenHtml('名 = "中"', [
    { line: 0, start_column: 1, end_line: 0, end_column: 2, token_type: 'variable' },
    { line: 0, start_column: 5, end_line: 0, end_column: 8, token_type: 'string' },
  ])
  assert.equal(
    html,
    '<span class="guaxi-tok-variable">名</span> = <span class="guaxi-tok-string">&quot;中&quot;</span>',
  )
})

test('token span 拼 HTML：重叠 / 越界 span 不能把后面的正文吞掉', () => {
  const code = 'x = 1'
  const html = buildTokenHtml(code, [
    { line: 0, start_column: 1, end_line: 0, end_column: 6, token_type: 'string' },
    { line: 0, start_column: 5, end_line: 0, end_column: 6, token_type: 'number' },
  ])
  assert.equal(html, '<span class="guaxi-tok-string">x = 1</span>')
})

test('预览语言解析：别名、大小写、不支持的都按预期落点', () => {
  assert.equal(resolvePreviewLanguage('language-python'), 'python')
  assert.equal(resolvePreviewLanguage('hljs language-PY'), 'python')
  assert.equal(resolvePreviewLanguage('language-c++'), 'cpp')
  assert.equal(resolvePreviewLanguage('language-js'), 'javascript')
  assert.equal(resolvePreviewLanguage('language-tex'), 'latex')
  assert.equal(resolvePreviewLanguage('language-json'), '')
  assert.equal(resolvePreviewLanguage('language-plaintext'), '')
  assert.equal(resolvePreviewLanguage(''), '')
  assert.equal(resolvePreviewLanguage(undefined), '')
})
