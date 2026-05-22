import * as monaco from 'monaco-editor'
import { invoke } from '@tauri-apps/api/core'

const treeSitterSemanticReadyLanguages = new Set<string>()
const treeSitterSemanticLogged = new Set<string>()
let treeSitterSemanticRequestSeq = 1

const treeSitterSemanticTokenTypes = [
  'comment',
  'string',
  'keyword',
  'number',
  'regexp',
  'operator',
  'punctuation',
  'type',
  'function',
  'property',
  'variable',
  'parameter',
  'namespace',
  'class',
  'boolean',
  'macro',
  'decorator',
  'label',
]

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

const mapTreeSitterTokenType = (tokenType: string) => {
  const type = String(tokenType || '').toLowerCase()
  if (type.includes('comment')) return 'comment'
  if (type.includes('string.escape') || type.includes('escape')) return 'string'
  if (type.includes('string')) return 'string'
  if (type.includes('keyword')) return 'keyword'
  if (type.includes('number')) return 'number'
  if (type.includes('regexp')) return 'regexp'
  if (type.includes('operator')) return 'operator'
  if (type.includes('punctuation')) return 'punctuation'
  if (type.includes('type') || type.includes('constructor')) return 'type'
  if (type.includes('function')) return 'function'
  if (type.includes('property') || type.includes('member')) return 'property'
  if (type.includes('parameter')) return 'parameter'
  if (type.includes('module')) return 'namespace'
  if (type.includes('tag')) return 'class'
  if (type.includes('boolean') || type.includes('constant')) return 'boolean'
  if (type.includes('macro')) return 'macro'
  if (type.includes('decorator')) return 'decorator'
  if (type.includes('label')) return 'label'
  return 'variable'
}

const buildUtf8LineIndex = (text: string) => {
  const encoder = new TextEncoder()
  const lineStartBytes = [0]
  const lineStartOffsets = [0]
  let byteOffset = 0
  for (let offset = 0; offset < text.length;) {
    const codePoint = text.codePointAt(offset) || 0
    const char = String.fromCodePoint(codePoint)
    byteOffset += encoder.encode(char).length
    offset += char.length
    if (char === '\n') {
      lineStartBytes.push(byteOffset)
      lineStartOffsets.push(offset)
    }
  }
  return { encoder, lineStartBytes, lineStartOffsets }
}

const findLineForByte = (lineStartBytes: number[], byteOffset: number) => {
  let low = 0
  let high = lineStartBytes.length - 1
  while (low <= high) {
    const mid = Math.floor((low + high) / 2)
    if (lineStartBytes[mid] <= byteOffset) low = mid + 1
    else high = mid - 1
  }
  return Math.max(0, high)
}

const positionFromByteOffset = (
  text: string,
  index: ReturnType<typeof buildUtf8LineIndex>,
  byteOffset: number
) => {
  const line = findLineForByte(index.lineStartBytes, byteOffset)
  const lineStartByte = index.lineStartBytes[line]
  const lineStartOffset = index.lineStartOffsets[line]
  const targetBytes = Math.max(0, byteOffset - lineStartByte)
  let consumedBytes = 0
  let column = 0
  for (let offset = lineStartOffset; offset < text.length && consumedBytes < targetBytes;) {
    const codePoint = text.codePointAt(offset) || 0
    const char = String.fromCodePoint(codePoint)
    if (char === '\n') break
    const charBytes = index.encoder.encode(char).length
    if (consumedBytes + charBytes > targetBytes) break
    consumedBytes += charBytes
    column += char.length
    offset += char.length
  }
  return { line, column }
}

const lineLengthAt = (model: monaco.editor.ITextModel, zeroBasedLine: number) => {
  const lineNumber = zeroBasedLine + 1
  if (lineNumber < 1 || lineNumber > model.getLineCount()) return 0
  return model.getLineMaxColumn(lineNumber) - 1
}

const encodeTreeSitterSemanticTokens = (
  model: monaco.editor.ITextModel,
  spans: Array<{ start_byte: number; end_byte: number; token_type: string }>
) => {
  const text = model.getValue()
  const index = buildUtf8LineIndex(text)
  const rows: number[][] = []

  for (const span of spans || []) {
    const tokenType = mapTreeSitterTokenType(span.token_type)
    const tokenIndex = treeSitterSemanticTokenTypes.indexOf(tokenType)
    if (tokenIndex < 0 || span.end_byte <= span.start_byte) continue

    const start = positionFromByteOffset(text, index, span.start_byte)
    const end = positionFromByteOffset(text, index, span.end_byte)
    if (start.line === end.line) {
      const length = Math.max(0, end.column - start.column)
      if (length > 0) rows.push([start.line, start.column, length, tokenIndex, 0])
      continue
    }

    const firstLength = Math.max(0, lineLengthAt(model, start.line) - start.column)
    if (firstLength > 0) rows.push([start.line, start.column, firstLength, tokenIndex, 0])
    for (let line = start.line + 1; line < end.line; line += 1) {
      const length = lineLengthAt(model, line)
      if (length > 0) rows.push([line, 0, length, tokenIndex, 0])
    }
    if (end.column > 0) rows.push([end.line, 0, end.column, tokenIndex, 0])
  }

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
    logTreeSitterHighlight('warn', 'tree-sitter backend does not support Monaco language', {
      languageId: lang,
      normalized,
    }, true)
    return
  }

  if (treeSitterSemanticReadyLanguages.has(lang)) {
    logTreeSitterHighlight('debug', 'semantic provider already registered', { languageId: lang, normalized })
    return
  }
  treeSitterSemanticReadyLanguages.add(lang)

  monaco.languages.registerDocumentSemanticTokensProvider(lang, {
    getLegend: () => treeSitterSemanticLegend,
    async provideDocumentSemanticTokens(model, _lastResultId, cancellationToken) {
      const frontendRequestId = treeSitterSemanticRequestSeq++
      const startedAt = performance.now()
      const code = model.getValue()
      logTreeSitterHighlight('info', 'semantic request start', {
        frontendRequestId,
        languageId: lang,
        normalized,
        uri: String(model.uri || ''),
        versionId: model.getVersionId(),
        lineCount: model.getLineCount(),
        codeLength: code.length,
      }, true)

      if (cancellationToken?.isCancellationRequested) {
        logTreeSitterHighlight('info', 'semantic request cancelled before backend invoke', {
          frontendRequestId,
          languageId: lang,
          normalized,
        }, true)
        return { data: new Uint32Array() }
      }

      try {
        const response = await invoke<{
          debug_id?: number
          elapsed_ms?: number
          supported: boolean
          spans: Array<{ start_byte: number; end_byte: number; token_type: string }>
        }>('syntax_highlight_tokens', {
          request: {
            code,
            language: normalized,
          },
        })

        if (cancellationToken?.isCancellationRequested || !response?.supported) {
          logTreeSitterHighlight('warn', 'semantic request returned no usable tokens', {
            frontendRequestId,
            backendDebugId: response?.debug_id,
            languageId: lang,
            normalized,
            supported: !!response?.supported,
            cancelled: !!cancellationToken?.isCancellationRequested,
            backendElapsedMs: response?.elapsed_ms,
            frontendElapsedMs: Math.round(performance.now() - startedAt),
          }, true)
          return { data: new Uint32Array() }
        }

        const encoded = encodeTreeSitterSemanticTokens(model, response.spans || [])
        logTreeSitterHighlight('info', 'semantic request success', {
          frontendRequestId,
          backendDebugId: response.debug_id,
          languageId: lang,
          normalized,
          versionId: model.getVersionId(),
          spanCount: response.spans?.length || 0,
          encodedTokenCount: encoded.length / 5,
          backendElapsedMs: response.elapsed_ms,
          frontendElapsedMs: Math.round(performance.now() - startedAt),
        }, true)
        return {
          data: encoded,
          resultId: `${model.getVersionId()}`,
        }
      } catch (error) {
        logTreeSitterHighlight('warn', 'semantic request failed', {
          frontendRequestId,
          languageId: lang,
          normalized,
          frontendElapsedMs: Math.round(performance.now() - startedAt),
          error,
        }, true)
        return { data: new Uint32Array() }
      }
    },
    releaseDocumentSemanticTokens() {},
  })

  if (!treeSitterSemanticLogged.has(lang)) {
    treeSitterSemanticLogged.add(lang)
    logTreeSitterHighlight('info', 'semantic provider registered', {
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
