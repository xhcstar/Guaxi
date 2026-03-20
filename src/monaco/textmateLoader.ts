import * as monaco from 'monaco-editor'
import { Registry, IGrammarDefinition } from 'monaco-textmate'
import { loadWASM, OnigScanner, OnigString } from 'onigasm'
// Vite: 把 wasm 当资源 url 引入
// 如果路径报错，可以改为 'onigasm/lib/onigasm.wasm?url'
import onigWasmUrl from 'onigasm/lib/onigasm.wasm?url'

// 1. scopeName -> 动态导入对应 grammar（使用 Vite 的动态 import）
//   注意：这些路径要和你实际保存的文件一致
const grammarDefinitions: Record<string, () => Promise<IGrammarDefinition>> = {
  'source.cpp': async () => ({
    format: 'json',
    content: JSON.stringify((await import('../grammar/cpp.tmLanguage.json')).default)
  }),
  'source.c': async () => ({
    format: 'json',
    content: JSON.stringify((await import('../grammar/c.tmLanguage.json')).default)
  }),
  'source.rust': async () => ({
    format: 'json',
    content: JSON.stringify((await import('../grammar/rust.tmLanguage.json')).default)
  }),
  'source.python': async () => ({
    format: 'json',
    content: JSON.stringify((await import('../grammar/MagicPython.tmLanguage.json')).default)
  }),
  'source.shell': async () => ({
    format: 'json',
    content: JSON.stringify((await import('../grammar/shell-unix-bash.tmLanguage.json')).default)
  }),
  'source.lua': async () => ({
    format: 'json',
    content: JSON.stringify((await import('../grammar/lua.tmLanguage.json')).default)
  }),
  'source.java': async () => ({
    format: 'json',
    content: JSON.stringify((await import('../grammar/java.tmLanguage.json')).default)
  }),
  'source.js': async () => ({
    format: 'json',
    content: JSON.stringify((await import('../grammar/javascript.tmLanguage.json')).default)
  }),
  'text.html.basic': async () => ({
    format: 'json',
    content: JSON.stringify((await import('../grammar/html.tmLanguage.json')).default)
  }),
  'text.html.markdown': async () => ({
    format: 'json',
    content: JSON.stringify((await import('../grammar/markdown.tmLanguage.json')).default)
  }),
  'text.tex.markdown_latex_combined': async () => ({
    format: 'json',
    content: JSON.stringify((await import('../grammar/markdown-latex-combined.tmLanguage.json')).default)
  }),
  'text.tex.latex': async () => ({
    format: 'json',
    content: JSON.stringify((await import('../grammar/latex.tmLanguage.json')).default)
  })
}

// 2. monaco languageId -> TextMate scopeName
const languageScopeMap = new Map<string, string>([
  ['cpp', 'source.cpp'],
  ['c', 'source.c'],
  ['rust', 'source.rust'],
  ['python', 'source.python'],
  ['shell', 'source.shell'],
  ['lua', 'source.lua'],
  ['java', 'source.java'],
  ['javascript', 'source.js'],
  ['html', 'text.html.basic'],
  ['markdown', 'text.html.markdown'],
  ['latex', 'text.tex.latex']
])

function createEmptyGrammarDefinition(scopeName: string): IGrammarDefinition {
  return {
    format: 'json',
    content: JSON.stringify({
      scopeName,
      patterns: [],
    }),
  }
}

let registry: Registry | null = null
let onigReady: Promise<void> | null = null
let onigSupportChecked = false
let textmateDisabled = false
let textmateDisableLogged = false
const missingGrammarLogged = new Set<string>()
const invalidPatternLogged = new Set<string>()
const textmateConfiguredDisabledLanguages = new Set<string>([
  'markdown',
  'latex',
  'c',
  'cpp',
  'rust'
])
const textmateRuntimeDisabledLanguages = new Set<string>()
const textmateLanguageSkipLogged = new Set<string>()
const textmateRuntimeDisableLogged = new Set<string>()
const textmateReadyLanguages = new Set<string>()
const textmateReadyPromises = new Map<string, Promise<void>>()
const textmateRuntimeFailureLogged = new Set<string>()
const textmateFallbackApplied = new Set<string>()
const textmateFallbackLogged = new Set<string>()
const suppressedMissingGrammars = new Set<string>([
  'source.asm',
  'source.arm',
  'source.x86',
  'source.x86_64',
  'source.cpp.embedded.macro',
  'source.regexp.python',
  'source.glsl',
  'source.sql',
  'source.css',
  'source.ini',
  'source.makefile',
  'source.perl',
  'source.r',
  'source.ruby',
  'source.php',
  'source.asp.vb.net',
  'text.xml',
  'text.xml.xsl',
  'source.yaml',
  'source.batchfile',
  'source.clojure',
  'source.coffee',
  'source.diff',
  'source.dockerfile',
  'source.abap',
  'source.rst',
  'text.html.derivative',
  'text.tex',
  'source.asy',
  'source.asymptote',
  'source.cpp.embedded.latex',
  'source.gnuplot',
  'source.haskell',
  'source.dot',
  'text.tex.markdown_latex_combined',
  'text.git-commit',
  'text.git-rebase',
  'source.go',
  'source.groovy',
  'text.pug',
  'source.ignore',
  'source.js.regexp',
  'source.json',
  'source.json.comments',
  'source.json.lines',
  'source.css.less',
  'source.objc',
  'source.swift',
  'source.css.scss',
  'source.perl.6',
  'source.powershell',
  'source.julia',
  'source.scala',
  'source.ts',
  'source.tsx',
  'source.cs',
  'source.fsharp',
  'source.dart',
  'text.html.handlebars',
  'text.log',
  'source.erlang',
  'source.elixir',
  'text.bibtex',
  'source.twig',
  'source.yang'
])

function checkOnigSupport() {
  if (onigSupportChecked) return
  onigSupportChecked = true
  try {
    const scanner = new OnigScanner(['(?<=a)b'])
    scanner.findNextMatchSync('ab', 0)
  } catch (error) {
    textmateDisabled = true
    console.warn('[TextMate] disabled due to onigasm lookbehind unsupported', error)
  }
}

function createSafeOnigScanner(patterns: string[]) {
  try {
    return new OnigScanner(patterns)
  } catch (error) {
    const reason = String((error as Error)?.message || error || 'invalid-pattern')
    const key = `${reason}:${patterns.join('|')}`
    if (!invalidPatternLogged.has(key)) {
      invalidPatternLogged.add(key)
      console.warn('[TextMate] invalid onig pattern, fallback to empty scanner', {
        reason,
        sample: patterns.slice(0, 6)
      })
    }
    return {
      findNextMatchSync() {
        return null
      }
    } as unknown as OnigScanner
  }
}

class TextmateTokenizerState implements monaco.languages.IState {
  private readonly ruleStack: unknown

  constructor(ruleStack: unknown) {
    this.ruleStack = ruleStack
  }

  clone(): monaco.languages.IState {
    return new TextmateTokenizerState(this.ruleStack)
  }

  equals(other: monaco.languages.IState): boolean {
    return other instanceof TextmateTokenizerState && other.ruleStack === this.ruleStack
  }

  getRuleStack() {
    return this.ruleStack
  }
}

class StatelessTokenizerState implements monaco.languages.IState {
  clone(): monaco.languages.IState {
    return this
  }

  equals(other: monaco.languages.IState): boolean {
    return other === this
  }
}

const STATELESS_TOKENIZER_STATE = new StatelessTokenizerState()
const LATEX_ENV_COMMANDS = new Set(['begin', 'end'])
const LATEX_MATH_COMMANDS = new Set([
  'frac', 'dfrac', 'tfrac', 'sqrt', 'sum', 'prod', 'int', 'iint', 'iiint', 'oint',
  'lim', 'limsup', 'liminf', 'sup', 'inf', 'max', 'min', 'argmax', 'argmin',
  'sin', 'cos', 'tan', 'csc', 'sec', 'cot', 'arcsin', 'arccos', 'arctan',
  'sinh', 'cosh', 'tanh', 'log', 'ln', 'exp', 'det', 'dim', 'gcd', 'hom', 'ker',
  'Pr', 'mod', 'bmod', 'pmod', 'left', 'right', 'middle', 'bigl', 'bigr', 'Bigl',
  'Bigr', 'biggl', 'biggr', 'Biggl', 'Biggr', 'overline', 'underline', 'hat',
  'widehat', 'tilde', 'widetilde', 'bar', 'vec', 'dot', 'ddot', 'mathbb', 'mathbf',
  'mathcal', 'mathrm', 'mathsf', 'mathtt', 'mathit', 'operatorname', 'text', 'label',
  'ref', 'eqref', 'cite', 'includegraphics'
])
const LATEX_STYLE_COMMANDS = new Set([
  'documentclass', 'usepackage', 'title', 'author', 'date', 'chapter', 'section',
  'subsection', 'subsubsection', 'paragraph', 'subparagraph', 'part', 'caption',
  'centering', 'raggedright', 'raggedleft', 'emph', 'textbf', 'textit', 'texttt',
  'underline', 'url', 'href', 'item'
])
const LATEX_SYMBOL_COMMANDS = new Set([
  'alpha', 'beta', 'gamma', 'delta', 'epsilon', 'varepsilon', 'zeta', 'eta', 'theta',
  'vartheta', 'iota', 'kappa', 'lambda', 'mu', 'nu', 'xi', 'pi', 'varpi', 'rho',
  'varrho', 'sigma', 'varsigma', 'tau', 'upsilon', 'phi', 'varphi', 'chi', 'psi',
  'omega', 'Gamma', 'Delta', 'Theta', 'Lambda', 'Xi', 'Pi', 'Sigma', 'Upsilon',
  'Phi', 'Psi', 'Omega', 'leq', 'geq', 'neq', 'approx', 'equiv', 'subset', 'subseteq',
  'supset', 'supseteq', 'in', 'notin', 'to', 'mapsto', 'times', 'cdot', 'pm',
  'mp', 'oplus', 'otimes', 'cup', 'cap', 'forall', 'exists', 'infty', 'partial',
  'nabla', 'ldots', 'cdots', 'vdots', 'ddots'
])
const LATEX_ENV_NAME_RE = /^[A-Za-z*@][\w*.-]*/

class LatexFallbackTokenizerState implements monaco.languages.IState {
  readonly expectEnvironmentBrace: boolean
  readonly insideEnvironmentName: boolean

  constructor(expectEnvironmentBrace = false, insideEnvironmentName = false) {
    this.expectEnvironmentBrace = expectEnvironmentBrace
    this.insideEnvironmentName = insideEnvironmentName
  }

  clone(): monaco.languages.IState {
    return new LatexFallbackTokenizerState(this.expectEnvironmentBrace, this.insideEnvironmentName)
  }

  equals(other: monaco.languages.IState): boolean {
    return other instanceof LatexFallbackTokenizerState
      && other.expectEnvironmentBrace === this.expectEnvironmentBrace
      && other.insideEnvironmentName === this.insideEnvironmentName
  }
}

const mapTextmateScopesToMonacoToken = (scopes: string[]) => {
  const joined = scopes.join(' ').toLowerCase()
  if (!joined) return ''
  if (joined.includes('comment')) return 'comment'
  if (joined.includes('string')) return 'string'
  if (joined.includes('regexp')) return 'regexp'
  if (joined.includes('keyword.operator')) return 'operator'
  if (joined.includes('keyword')) return 'keyword'
  if (joined.includes('constant.numeric') || joined.includes('number')) return 'number'
  if (joined.includes('constant.language.boolean') || joined.includes('boolean')) return 'boolean'
  if (joined.includes('entity.name.type') || joined.includes('support.type') || joined.includes('storage.type')) return 'type'
  if (joined.includes('entity.name.class')) return 'class'
  if (joined.includes('entity.name.function') || joined.includes('support.function')) return 'function'
  if (joined.includes('variable.parameter')) return 'parameter'
  if (joined.includes('variable.other.property')) return 'property'
  if (joined.includes('entity.name.namespace') || joined.includes('entity.name.module')) return 'namespace'
  if (joined.includes('entity.name.tag')) return 'tag'
  if (joined.includes('entity.other.attribute-name')) return 'attribute.name'
  if (joined.includes('support.constant')) return 'constant'
  if (joined.includes('support.function.builtin') || joined.includes('support.type.builtin')) return 'builtin'
  if (joined.includes('variable')) return 'variable'
  if (joined.includes('meta.embedded')) return 'meta.embedded'
  if (joined.includes('punctuation') || joined.includes('delimiter')) return 'delimiter'
  const tail = scopes[scopes.length - 1] || ''
  return tail.split('.').slice(0, 3).join('.')
}

const createLatexFallbackTokensProvider = (): monaco.languages.TokensProvider => ({
  getInitialState: () => new LatexFallbackTokenizerState(),
  tokenize(line: string, state: monaco.languages.IState) {
    const tokens: monaco.languages.IToken[] = []
    let index = 0
    let expectEnvironmentBrace = state instanceof LatexFallbackTokenizerState
      ? state.expectEnvironmentBrace
      : false
    let insideEnvironmentName = state instanceof LatexFallbackTokenizerState
      ? state.insideEnvironmentName
      : false

    const push = (startIndex: number, scopes: string) => {
      tokens.push({ startIndex, scopes })
    }

    const isEscaped = (source: string, position: number) => {
      let slashCount = 0
      for (let i = position - 1; i >= 0 && source[i] === '\\'; i -= 1) {
        slashCount += 1
      }
      return (slashCount % 2) === 1
    }

    while (index < line.length) {
      const rest = line.slice(index)

      if (insideEnvironmentName) {
        if (rest[0] === '}') {
          push(index, 'punctuation.definition.group.latex')
          index += 1
          insideEnvironmentName = false
          continue
        }

        const envName = LATEX_ENV_NAME_RE.exec(rest)
        if (envName) {
          push(index, 'entity.name.type.environment.latex')
          index += envName[0].length
          continue
        }

        push(index, 'entity.name.type.environment.latex')
        index += 1
        continue
      }

      if (expectEnvironmentBrace) {
        if (/^\s+/.test(rest)) {
          index += /^\s+/.exec(rest)?.[0].length || 1
          continue
        }
        if (rest[0] === '{') {
          push(index, 'punctuation.definition.group.latex')
          index += 1
          expectEnvironmentBrace = false
          insideEnvironmentName = true
          continue
        }
        expectEnvironmentBrace = false
      }

      if (rest[0] === '%' && !isEscaped(line, index)) {
        push(index, 'comment')
        break
      }

      const command = /^(\\[a-zA-Z@]+(\*+)?|\\.)/.exec(rest)
      if (command) {
        const name = command[0].slice(1).replace(/\*+$/, '')
        let scope = 'keyword.control.latex'
        if (LATEX_ENV_COMMANDS.has(name)) {
          scope = 'keyword.control.environment.latex'
          expectEnvironmentBrace = true
        } else if (LATEX_MATH_COMMANDS.has(name)) {
          scope = 'support.function.math.latex'
        } else if (LATEX_STYLE_COMMANDS.has(name)) {
          scope = 'storage.type.command.latex'
        } else if (LATEX_SYMBOL_COMMANDS.has(name)) {
          scope = 'support.constant.symbol.latex'
        } else if (/^[A-Z]+$/.test(name)) {
          scope = 'support.constant.symbol.latex'
        }
        push(index, scope)
        index += command[0].length
        continue
      }

      const parameter = /^#[1-9]/.exec(rest)
      if (parameter) {
        push(index, 'variable.parameter.latex')
        index += parameter[0].length
        continue
      }

      const displayMath = /^\$\$/.exec(rest)
      if (displayMath) {
        push(index, 'string.math.block.latex')
        index += displayMath[0].length
        continue
      }

      const inlineMath = /^\$/.exec(rest)
      if (inlineMath && !isEscaped(line, index)) {
        push(index, 'string.math.inline.latex')
        index += inlineMath[0].length
        continue
      }

      const bracketContent = /^\[[^\]]*\]/.exec(rest)
      if (bracketContent) {
        push(index, 'entity.other.attribute-name.optional.latex')
        index += bracketContent[0].length
        continue
      }

      const number = /^\d+(\.\d+)?/.exec(rest)
      if (number) {
        push(index, 'constant.numeric.latex')
        index += number[0].length
        continue
      }

      if (/^[{}[\]()]/.test(rest)) {
        push(index, 'punctuation.definition.group.latex')
        index += 1
        continue
      }

      if (/^[&_^=+\-*/<>!|:,.;~]/.test(rest)) {
        push(index, 'keyword.operator.latex')
        index += 1
        continue
      }

      index += 1
    }

    return {
      endState: new LatexFallbackTokenizerState(expectEnvironmentBrace, insideEnvironmentName),
      tokens,
    }
  }
})

const getFallbackTokensProvider = (languageId: string) => {
  if (languageId === 'latex') return createLatexFallbackTokensProvider()
  return null
}

const getTextmateDisableReason = (languageId: string) => {
  if (textmateConfiguredDisabledLanguages.has(languageId)) {
    return 'configured-disabled'
  }
  if (textmateRuntimeDisabledLanguages.has(languageId)) {
    return 'runtime-disabled'
  }
  return ''
}

const isTextmateLanguageDisabled = (languageId: string) =>
  !!getTextmateDisableReason(languageId)

const applyFallbackTokensProvider = (languageId: string, reason: string) => {
  const fallbackProvider = getFallbackTokensProvider(languageId)
  if (!fallbackProvider) return false
  monaco.languages.setTokensProvider(languageId, fallbackProvider)
  if (!textmateFallbackApplied.has(languageId)) {
    textmateFallbackApplied.add(languageId)
  }
  const logKey = `${languageId}:${reason}`
  if (!textmateFallbackLogged.has(logKey)) {
    textmateFallbackLogged.add(logKey)
    console.debug('[TextMate] fallback tokenizer applied', { languageId, reason })
  }
  return true
}

async function ensureOnigReady() {
  if (!onigReady) {
    onigReady = (async () => {
      const wasmResponse = await fetch(onigWasmUrl)
      const wasmArrayBuffer = await wasmResponse.arrayBuffer()
      try {
        await loadWASM(wasmArrayBuffer)
      } catch (error) {
        const message = String((error as Error)?.message || error || '')
        if (!message.includes('subsequent calls are not allowed once initialized')) {
          throw error
        }
      }
      checkOnigSupport()
    })()
  }
  return onigReady
}

async function ensureRegistry() {
  await ensureOnigReady()
  if (!registry) {
    registry = new Registry({
      onigLib: Promise.resolve({
        createOnigScanner: (patterns) => createSafeOnigScanner(patterns),
        createOnigString: (s) => new OnigString(s)
      }),
      getGrammarDefinition: async (scopeName: string) => {
        const loader = grammarDefinitions[scopeName]
        if (!loader) {
          if (!suppressedMissingGrammars.has(scopeName) && !missingGrammarLogged.has(scopeName)) {
            missingGrammarLogged.add(scopeName)
            console.warn('[TextMate] 未找到 scopeName 对应的 grammar:', scopeName)
          }
          return createEmptyGrammarDefinition(scopeName)
        }
        return loader()
      }
    })
  }
  return registry!
}

// 对外暴露：在 Monaco editor 创建之后调用
export async function initTextmate(
  editor: monaco.editor.IStandaloneCodeEditor,
  languageId: string
) {
  const lang = String(languageId || '').trim().toLowerCase()

  const scopeName = languageScopeMap.get(lang)
  if (!scopeName) {
    return
  }

  const disableReason = getTextmateDisableReason(lang)
  if (disableReason) {
    if (
      disableReason === 'configured-disabled' &&
      !textmateLanguageSkipLogged.has(lang)
    ) {
      textmateLanguageSkipLogged.add(lang)
      console.debug('[TextMate] language intentionally bypassed', {
        languageId: lang,
        scopeName,
        reason: disableReason
      })
    }
    applyFallbackTokensProvider(lang, disableReason)
    return
  }

  if (textmateReadyLanguages.has(lang)) {
    return
  }

  const existing = textmateReadyPromises.get(lang)
  if (existing) {
    await existing
    return
  }

  const readyPromise = (async () => {
    const registry = await ensureRegistry()

    if (textmateDisabled) {
      if (!textmateDisableLogged) {
        textmateDisableLogged = true
        console.warn('[TextMate] disabled, fallback to Monaco default')
      }
      return
    }

    const grammar = await registry.loadGrammar(scopeName)
    if (!grammar) {
      console.warn('[TextMate] grammar load failed', { languageId: lang, scopeName })
      applyFallbackTokensProvider(lang, 'grammar-load-failed')
      return
    }

    monaco.languages.setTokensProvider(lang, {
      getInitialState: () => new TextmateTokenizerState(null),
      tokenize: (line: string, state: monaco.languages.IState) => {
        const stack = state instanceof TextmateTokenizerState ? state.getRuleStack() : null
        try {
          const result = grammar.tokenizeLine(line, stack as never)
          return {
            endState: new TextmateTokenizerState(result.ruleStack),
            tokens: result.tokens.map((token) => ({
              startIndex: token.startIndex,
              scopes: mapTextmateScopesToMonacoToken(token.scopes),
            })),
          }
        } catch (error) {
          const message = String((error as Error)?.message || error || '')
          if (!textmateRuntimeFailureLogged.has(lang)) {
            textmateRuntimeFailureLogged.add(lang)
            console.warn('[TextMate] tokenize failed, disabling language', { languageId: lang, scopeName, message }, error)
          }
          if (message.includes('endless loop') || message.includes('invalid pattern')) {
            textmateRuntimeDisabledLanguages.add(lang)
            if (!textmateRuntimeDisableLogged.has(lang)) {
              textmateRuntimeDisableLogged.add(lang)
              console.warn('[TextMate] runtime disabled for language', {
                languageId: lang,
                scopeName,
                reason: message
              })
            }
            applyFallbackTokensProvider(lang, `runtime:${message}`)
          }
          return {
            endState: STATELESS_TOKENIZER_STATE,
            tokens: [],
          }
        }
      }
    })

    textmateReadyLanguages.add(lang)
    console.debug('[TextMate] wired', { languageId: lang, scopeName })
  })()

  textmateReadyPromises.set(lang, readyPromise)
  try {
    await readyPromise
  } finally {
    textmateReadyPromises.delete(lang)
  }
}
