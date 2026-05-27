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

const mapTreeSitterSemanticTokenType = (
  model: monaco.editor.ITextModel,
  span: { start_byte: number; end_byte: number; token_type: string },
  start: { line: number; column: number }
) => {
  // 设计说明：
  // 1. semantic token、Markdown fence 和 Notebook light decoration 共用 `mapTreeSitterCaptureToGuaxiToken`。
  // 2. 这里额外传入源码与起始偏移，让共享 mapper 能把 `obj.method()` 这类点号成员调用归一为 method。
  // 3. 这样所有已支持语言都先进入同一套 Guaxi token 类型，再交给各自渲染通道。
  const startOffset = model.getOffsetAt({
    lineNumber: start.line + 1,
    column: start.column + 1,
  })
  return mapTreeSitterCaptureToGuaxiToken(span.token_type, {
    sourceText: model.getValue(),
    startOffset,
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
  spans: Array<{ start_byte: number; end_byte: number; token_type: string }>
) => {
  const text = model.getValue()
  const index = buildUtf8LineIndex(text)
  const rowsByRange = new Map<string, { row: number[]; priority: number }>()

  for (const span of spans || []) {
    if (span.end_byte <= span.start_byte) continue
    const start = positionFromByteOffset(text, index, span.start_byte)
    const end = positionFromByteOffset(text, index, span.end_byte)
    const tokenType = mapTreeSitterSemanticTokenType(model, span, start)
    const tokenIndex = treeSitterSemanticTokenTypes.indexOf(tokenType)
    if (tokenIndex < 0) continue

    if (start.line === end.line) {
      const length = Math.max(0, end.column - start.column)
      upsertSemanticTokenRow(rowsByRange, start.line, start.column, length, tokenIndex, tokenType)
      continue
    }

    const firstLength = Math.max(0, lineLengthAt(model, start.line) - start.column)
    upsertSemanticTokenRow(rowsByRange, start.line, start.column, firstLength, tokenIndex, tokenType)
    for (let line = start.line + 1; line < end.line; line += 1) {
      const length = lineLengthAt(model, line)
      upsertSemanticTokenRow(rowsByRange, line, 0, length, tokenIndex, tokenType)
    }
    upsertSemanticTokenRow(rowsByRange, end.line, 0, end.column, tokenIndex, tokenType)
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
        const mappedTokenTypeCounts = (response.spans || []).reduce<Record<string, number>>((acc, span) => {
          const mappedType = mapTreeSitterCaptureToGuaxiToken(span.token_type)
          acc[mappedType] = (acc[mappedType] || 0) + 1
          return acc
        }, {})
        logTreeSitterHighlight('info', 'semantic request success', {
          frontendRequestId,
          backendDebugId: response.debug_id,
          languageId: lang,
          normalized,
          versionId: model.getVersionId(),
          spanCount: response.spans?.length || 0,
          mappedTokenTypeCounts,
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
