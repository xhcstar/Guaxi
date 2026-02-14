import * as monaco from 'monaco-editor'
import { Registry, IGrammarDefinition } from 'monaco-textmate'
import { wireTmGrammars } from 'monaco-editor-textmate'
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
const textmateDisabledLanguages = new Set<string>(['markdown'])
const textmateLanguageSkipLogged = new Set<string>()
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

async function ensureOnigReady() {
  if (!onigReady) {
    onigReady = (async () => {
      const wasmResponse = await fetch(onigWasmUrl)
      const wasmArrayBuffer = await wasmResponse.arrayBuffer()
      await loadWASM(wasmArrayBuffer)
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

  if (textmateDisabledLanguages.has(lang)) {
    if (!textmateLanguageSkipLogged.has(lang)) {
      textmateLanguageSkipLogged.add(lang)
      console.warn('[TextMate] skipped for language', { languageId: lang, scopeName })
    }
    return
  }

  const registry = await ensureRegistry()

  if (textmateDisabled) {
    if (!textmateDisableLogged) {
      textmateDisableLogged = true
      console.warn('[TextMate] disabled, fallback to Monaco default')
    }
    return
  }

  const grammars = new Map<string, string>()
  for (const [k, scope] of languageScopeMap.entries()) {
    grammars.set(k, scope)
  }

  try {
    await wireTmGrammars(monaco, registry, grammars, editor)
    console.debug('[TextMate] wired', { languageId: lang, scopeName })
  } catch (e) {
    const message = String((e as Error)?.message || e || '')
    if (message.includes('endless loop') || message.includes('invalid pattern')) {
      textmateDisabledLanguages.add(lang)
    }
    console.warn('[TextMate] wire failed, fallback to Monaco default', { languageId: lang, scopeName }, e)
  }
}
