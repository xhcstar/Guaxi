import * as monaco from 'monaco-editor'
import { invoke } from '@tauri-apps/api/core'
import {
  GUAXI_SYNTAX_TOKEN_TYPES,
  getGuaxiSyntaxTokenPriority,
  mapTreeSitterCaptureToGuaxiToken,
} from './guaxiSyntaxTokens'

const treeSitterSemanticReadyLanguages = new Set<string>()
const treeSitterSemanticLogged = new Set<string>()
const treeSitterSemanticUnsupportedLanguages = new Set<string>()
let treeSitterSemanticRequestSeq = 1

const treeSitterSemanticTokenTypes = GUAXI_SYNTAX_TOKEN_TYPES

const treeSitterSemanticLegend: monaco.languages.SemanticTokensLegend = {
  tokenTypes: treeSitterSemanticTokenTypes,
  tokenModifiers: [],
}

const treeSitterSemanticLanguages = new Set<string>([
  'markdown',
  'javascript',
  'typescript',
  'tsx',
  'python',
  'rust',
  'shell',
  'bash',
  'lua',
  'java',
  'html',
  'c',
  'cpp',
  'latex',
])

const isTreeSitterHighlightDebugEnabled = () => {
  try {
    return localStorage.getItem('guaxi.debug.treeSitterHighlight') === '1'
  } catch {
    return false
  }
}

const logTreeSitterHighlight = (
  level: 'debug' | 'info' | 'warn',
  message: string,
  payload: Record<string, unknown> = {},
  force = false
) => {
  if (!force && !isTreeSitterHighlightDebugEnabled()) return
  const logger = level === 'warn' ? console.warn : level === 'info' ? console.info : console.debug
  logger(`[TreeSitterHighlight] ${message}`, payload)
}

const normalizeTreeSitterLanguageId = (languageId: string) => {
  const id = String(languageId || '').trim().toLowerCase()
  if (id === 'shell' || id === 'sh' || id === 'zsh' || id === 'powershell' || id === 'ps1') return 'bash'
  if (id === 'tex') return 'latex'
  if (id === 'javascriptreact') return 'javascript'
  if (id === 'typescriptreact') return 'tsx'
  return id
}

const lineLengthAt = (model: monaco.editor.ITextModel, zeroBasedLine: number) => {
  const lineNumber = zeroBasedLine + 1
  if (lineNumber < 1 || lineNumber > model.getLineCount()) return 0
  return model.getLineMaxColumn(lineNumber) - 1
}

const mapTreeSitterSemanticTokenType = (
  span: { start_byte: number; end_byte: number; token_type: string },
  // 局部源码段（发给后端的那段 code）与该 span 在其中的字符偏移；
  // 避免每个 span 都 model.getValue()（O(spans×n)），只用局部段做 obj.method() 归一。
  localText: string,
  localStartOffset: number,
) => {
  return mapTreeSitterCaptureToGuaxiToken(span.token_type, {
    sourceText: localText,
    startOffset: localStartOffset,
  })
}

const upsertSemanticTokenRow = (
  rowsByRange: Map<string, { row: number[]; priority: number }>,
  line: number,
  start: number,
  length: number,
  tokenIndex: number,
  tokenType: string
) => {
  if (length <= 0) return
  const key = `${line}:${start}:${length}`
  const priority = getGuaxiSyntaxTokenPriority(tokenType)
  const existing = rowsByRange.get(key)
  if (existing && existing.priority > priority) return
  rowsByRange.set(key, {
    row: [line, start, length, tokenIndex, 0],
    priority,
  })
}

const encodeTreeSitterSemanticTokens = (
  model: monaco.editor.ITextModel,
  spans: Array<{ start_byte: number; end_byte: number; token_type: string }>,
  // 0 基行偏移：spans 的字节偏移相对于传入后端的 code 段；range 高亮时该 code 段从 model 的第 lineOffset 行开始。
  lineOffset = 0,
  // 与发给后端完全一致的 code 段（document=全文，range=可视区段），保证字节偏移→位置换算不错位。
  sourceCode?: string,
) => {
  const text = typeof sourceCode === 'string' ? sourceCode : model.getValue()
  const rowsByRange = new Map<string, { row: number[]; priority: number }>()

  for (const span of spans || []) {
    if (span.end_byte <= span.start_byte) continue
    // 行列与字符偏移都由后端一次算好（syntax_highlight.rs 的 resolve_token_span_positions），
    // 这里原本要为整段 code 逐字符建一份 UTF-8 索引（每个字符一次 TextEncoder.encode）。
    // 后端给 1 基列，这个文件用 0 基，所以减 1。
    const startLine = Number(span.line ?? 0)
    const endLine = Number(span.end_line ?? 0)
    const startColumn = Number(span.start_column ?? 0) - 1
    const endColumn = Number(span.end_column ?? 0) - 1
    if (startColumn < 0 || endColumn < 0) {
      // 后端没给解析结果时不按 0 列去猜，否则整行都会染错。
      continue
    }

    const tokenType = mapTreeSitterSemanticTokenType(span, text, Number(span.start_utf16 ?? 0))
    const tokenIndex = treeSitterSemanticTokenTypes.indexOf(tokenType)
    if (tokenIndex < 0) continue

    if (startLine === endLine) {
      const length = Math.max(0, endColumn - startColumn)
      upsertSemanticTokenRow(rowsByRange, startLine + lineOffset, startColumn, length, tokenIndex, tokenType)
      continue
    }

    const firstLength = Math.max(0, lineLengthAt(model, startLine + lineOffset) - startColumn)
    upsertSemanticTokenRow(rowsByRange, startLine + lineOffset, startColumn, firstLength, tokenIndex, tokenType)
    for (let line = startLine + 1; line < endLine; line += 1) {
      const length = lineLengthAt(model, line + lineOffset)
      upsertSemanticTokenRow(rowsByRange, line + lineOffset, 0, length, tokenIndex, tokenType)
    }
    upsertSemanticTokenRow(rowsByRange, endLine + lineOffset, 0, endColumn, tokenIndex, tokenType)
  }

  const rows = Array.from(rowsByRange.values()).map((item) => item.row)
  rows.sort((a, b) => (a[0] - b[0]) || (a[1] - b[1]))
  const data: number[] = []
  let previousLine = 0
  let previousStart = 0
  for (const row of rows) {
    const [line, start, length, tokenType, tokenModifiers] = row
    const deltaLine = line - previousLine
    const deltaStart = deltaLine === 0 ? start - previousStart : start
    if (deltaLine < 0 || deltaStart < 0 || length <= 0) continue
    data.push(deltaLine, deltaStart, length, tokenType, tokenModifiers)
    previousLine = line
    previousStart = start
  }
  return new Uint32Array(data)
}

function ensureTreeSitterSemanticProvider(languageId: string) {
  const lang = String(languageId || '').trim().toLowerCase()
  const normalized = normalizeTreeSitterLanguageId(lang)

  if (!treeSitterSemanticLanguages.has(normalized)) {
    const unsupportedKey = normalized || lang
    if (!treeSitterSemanticUnsupportedLanguages.has(unsupportedKey)) {
      treeSitterSemanticUnsupportedLanguages.add(unsupportedKey)
      logTreeSitterHighlight('debug', 'tree-sitter backend does not support Monaco language', {
        languageId: lang,
        normalized,
      })
    }
    return
  }

  if (treeSitterSemanticReadyLanguages.has(lang)) {
    logTreeSitterHighlight('debug', 'semantic provider already registered', { languageId: lang, normalized })
    return
  }
  treeSitterSemanticReadyLanguages.add(lang)

  // 向后端请求一段 code 的高亮 span，并按 model 的绝对偏移编码成 Monaco semantic tokens。
  // lineOffset：该段 code 在 model 中的起始行（0 基）。
  const fetchAndEncode = async (
    model: monaco.editor.ITextModel,
    code: string,
    lineOffset: number,
    cancellationToken?: monaco.CancellationToken,
  ): Promise<Uint32Array> => {
    if (cancellationToken?.isCancellationRequested) return new Uint32Array()
    try {
      const response = await invoke<{
        debug_id?: number
        elapsed_ms?: number
        supported: boolean
        spans: Array<{ start_byte: number; end_byte: number; token_type: string }>
      }>('syntax_highlight_tokens', {
        request: { code, language: normalized },
      })
      if (cancellationToken?.isCancellationRequested || !response?.supported) {
        return new Uint32Array()
      }
      return encodeTreeSitterSemanticTokens(model, response.spans || [], lineOffset, code)
    } catch (error) {
      logTreeSitterHighlight('warn', 'semantic request failed', { languageId: lang, normalized, error }, true)
      return new Uint32Array()
    }
  }

  // 只注册 Range(可视区) provider：
  // 1. Monaco 仅当「无 document provider 产出完整 token」时才用 viewport(range) 高亮可视区（含上下缓冲区）。
  //    若同时注册 document provider，它返回任何结果（含空）都会标记 hasCompleteSemanticTokens=true，
  //    从而让 range provider 被跳过——大文档就会完全无高亮。故这里彻底改用 range provider。
  // 2. 小文件几乎整篇都在视口内，效果与整篇高亮一致；大文件只对可视区请求后端 → O(全文) 降为 O(视口)。
  //    滚动时 Monaco 自动对新可视区再请求。
  monaco.languages.registerDocumentRangeSemanticTokensProvider(lang, {
    getLegend: () => treeSitterSemanticLegend,
    async provideDocumentRangeSemanticTokens(model, range, cancellationToken) {
      const startLine = Math.max(1, range.startLineNumber)
      const endLine = Math.min(model.getLineCount(), range.endLineNumber)
      const code = model.getValueInRange({
        startLineNumber: startLine,
        startColumn: 1,
        endLineNumber: endLine,
        endColumn: model.getLineMaxColumn(endLine),
      })
      if (cancellationToken?.isCancellationRequested) return { data: new Uint32Array() }
      const encoded = await fetchAndEncode(model, code, startLine - 1, cancellationToken)
      return { data: encoded }
    },
  })

  if (!treeSitterSemanticLogged.has(lang)) {
    treeSitterSemanticLogged.add(lang)
    logTreeSitterHighlight('info', 'semantic provider registered (range only)', {
      languageId: lang,
      normalized,
      legend: treeSitterSemanticTokenTypes,
    }, true)
  }
}

// 兼容旧调用名：这里不再初始化 TextMate，只注册 tree-sitter 高亮并禁用旧词法 tokenizer。
export async function initTextmate(
  _editor: monaco.editor.IStandaloneCodeEditor,
  languageId: string
) {
  ensureTreeSitterSemanticProvider(languageId)
}

export async function initTreeSitterHighlight(
  editor: monaco.editor.IStandaloneCodeEditor,
  languageId: string
) {
  return initTextmate(editor, languageId)
}
