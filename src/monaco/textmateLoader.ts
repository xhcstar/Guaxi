import * as monaco from 'monaco-editor'
import { Registry, IGrammarDefinition } from 'monaco-textmate'
import { wireTmGrammars } from 'monaco-editor-textmate'
import { loadWASM } from 'onigasm'
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

async function ensureOnigReady() {
  if (!onigReady) {
    onigReady = (async () => {
      const wasmResponse = await fetch(onigWasmUrl)
      const wasmArrayBuffer = await wasmResponse.arrayBuffer()
      await loadWASM(wasmArrayBuffer)
    })()
  }
  return onigReady
}

async function ensureRegistry() {
  await ensureOnigReady()
  if (!registry) {
    registry = new Registry({
      getGrammarDefinition: async (scopeName: string) => {
        const loader = grammarDefinitions[scopeName]
        if (!loader) {
          console.warn('[TextMate] 未找到 scopeName 对应的 grammar:', scopeName)
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

  if (lang === 'markdown' || lang === 'shell') {
    console.info('[TextMate] skipped', { languageId: lang })
    return
  }

  const scopeName = languageScopeMap.get(lang)
  if (!scopeName) {
    return
  }

  const registry = await ensureRegistry()

  // 所有开启 TextMate 的语言
  const grammars = new Map<string, string>()
  for (const [k, scope] of languageScopeMap.entries()) {
    grammars.set(k, scope)
  }

  try {
    await wireTmGrammars(monaco, registry, grammars, editor)
    console.debug('[TextMate] wired', { languageId: lang, scopeName })
  } catch (e) {
    console.warn('[TextMate] wire failed, fallback to Monaco default', { languageId: lang, scopeName }, e)
  }
}