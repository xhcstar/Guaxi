import * as monaco from 'monaco-editor'
import { MonacoLanguageClient } from 'monaco-languageclient'
import {
  CloseAction,
  ErrorAction,
  type MessageTransports,
} from 'vscode-languageclient/browser.js'
import {
  CompletionRequest,
  DidChangeTextDocumentNotification,
  DidOpenTextDocumentNotification,
  SemanticTokensRequest,
} from 'vscode-languageserver-protocol'
import {
  toSocket,
  WebSocketMessageReader,
  WebSocketMessageWriter,
} from 'vscode-ws-jsonrpc'
import { createLogger } from '../utils/logger'
import {
  getLspDocumentSelectorForLanguage,
  getLspServerForLanguage,
  isLspSupportedLanguage
} from './lspCapabilities'
type LspClientEntry = {
  client: MonacoLanguageClient | null
  webSocket: WebSocket
  disposed: boolean
  manualDisposing: boolean
  reader: WebSocketMessageReader | null
  writer: WebSocketMessageWriter | null
  readyPromise: Promise<boolean>
  resolveReady: (value: boolean) => void
  languageId: string
  documentUri: string
  workspacePath: string | null
  workspaceUri: string | null
}
const lspClients = new Map<string, LspClientEntry>()
const lspRetryBlockedUntil = new Map<string, number>()
const LSP_RETRY_BACKOFF_MS = 15000
const logger = createLogger('lspClient')
const lspDisabled = new Set<string>()
const lspDisabledReason = new Map<string, string>()
const LSP_VSCODE_API_NOT_READY = 'Default api is not ready yet'
const LSP_FAILURE_WINDOW_MS = 60000
const LSP_FAILURE_THRESHOLD = 3
const LSP_STOP_TIMEOUT_MS = 350
const lspFailureTracker = new Map<string, { count: number; firstAt: number }>()
const directCompletionProviderDisposables = new Map<string, monaco.IDisposable>()
const directSyncedDocuments = new Map<string, { version: number; content: string; languageId: string }>()
const DIRECT_LSP_SYNC_SETTLE_MS = 35
const DIRECT_LSP_CLIENT_READY_TIMEOUT_MS = 1500

const LSP_TO_MONACO_COMPLETION_KIND: Record<number, monaco.languages.CompletionItemKind> = {
  1: monaco.languages.CompletionItemKind.Text,
  2: monaco.languages.CompletionItemKind.Method,
  3: monaco.languages.CompletionItemKind.Function,
  4: monaco.languages.CompletionItemKind.Constructor,
  5: monaco.languages.CompletionItemKind.Field,
  6: monaco.languages.CompletionItemKind.Variable,
  7: monaco.languages.CompletionItemKind.Class,
  8: monaco.languages.CompletionItemKind.Interface,
  9: monaco.languages.CompletionItemKind.Module,
  10: monaco.languages.CompletionItemKind.Property,
  11: monaco.languages.CompletionItemKind.Unit,
  12: monaco.languages.CompletionItemKind.Value,
  13: monaco.languages.CompletionItemKind.Enum,
  14: monaco.languages.CompletionItemKind.Keyword,
  15: monaco.languages.CompletionItemKind.Snippet,
  16: monaco.languages.CompletionItemKind.Color,
  17: monaco.languages.CompletionItemKind.File,
  18: monaco.languages.CompletionItemKind.Reference,
  19: monaco.languages.CompletionItemKind.Folder,
  20: monaco.languages.CompletionItemKind.EnumMember,
  21: monaco.languages.CompletionItemKind.Constant,
  22: monaco.languages.CompletionItemKind.Struct,
  23: monaco.languages.CompletionItemKind.Event,
  24: monaco.languages.CompletionItemKind.Operator,
  25: monaco.languages.CompletionItemKind.TypeParameter,
}

const DIRECT_COMPLETION_TRIGGER_CHARACTERS = [
  // 设计说明：
  // 1. 与 VS Code 的 LSP 补全模型保持一致：普通输入由 quickSuggestions 触发，特殊符号由 triggerCharacters 立即触发。
  // 2. 这里不响应鼠标/光标移动；真正导致探索者模式误弹窗的是上层主动 triggerSuggest，而不是 provider 注册本身。
  // 3. 触发符保留常见成员访问、标签、注解、路径和字符串场景，避免编辑模式补全变钝。
  '.',
  ':',
  '>',
  '/',
  '\\',
  '"',
  '\'',
  '`',
  '@',
  '#',
  '<',
]

const LSP_LANGUAGE_ALIASES: Record<string, string> = {
  ra_syntax_tree: 'rust',
  javascriptreact: 'javascript',
  typescriptreact: 'typescript',
  shellscript: 'shell',
  sh: 'shell',
  tex: 'latex',
}

const normalizeLanguageId = (languageId: string) =>
  LSP_LANGUAGE_ALIASES[String(languageId || '').trim().toLowerCase()]
  || String(languageId || '').trim().toLowerCase()

const getLspClientKey = (languageId: string) =>
  String(getLspServerForLanguage(languageId) || normalizeLanguageId(languageId)).trim().toLowerCase()

const getRelatedLspLanguages = (languageId: string) => {
  const selectors = getLspDocumentSelectorForLanguage(languageId)
    .map((item) => normalizeLanguageId(item?.language || ''))
    .filter(Boolean)
  return selectors.length ? selectors : [normalizeLanguageId(languageId)].filter(Boolean)
}

const getLspClientLanguages = (languageId: string) => {
  const normalizedLanguageId = normalizeLanguageId(languageId)
  // 设计说明：
  // 1. clangd 这类服务端会同时服务 c/cpp，但运行时 extension snapshot 有时只把启动语言放进 selector。
  // 2. 复用同一个 clientKey 时，当前 languageId 必须显式纳入状态和补全 provider 注册范围。
  // 3. 否则会出现日志里 client 已复用、readyPromise 已完成，但 c/cpp 其中一个语言仍停在 idle 的情况。
  return Array.from(new Set([
    normalizedLanguageId,
    ...getRelatedLspLanguages(normalizedLanguageId),
  ].filter(Boolean)))
}

const waitForDirectLspDocumentSync = () =>
  new Promise((resolve) => {
    window.setTimeout(resolve, DIRECT_LSP_SYNC_SETTLE_MS)
  })

const getLanguageClientRuntimeState = (client: MonacoLanguageClient | null) => {
  const candidate = client as (MonacoLanguageClient & {
    isRunning?: () => boolean
    state?: unknown
    _state?: unknown
  }) | null
  if (!candidate) {
    return { isRunning: false, publicState: '', privateState: '' }
  }

  let isRunning = false
  try {
    isRunning = candidate.isRunning?.() === true
  } catch {
    isRunning = false
  }

  return {
    isRunning,
    publicState: String(candidate.state ?? ''),
    privateState: String(candidate._state ?? ''),
  }
}

const isLspClientNotRunningError = (error: unknown) =>
  resolveErrorMessage(error).toLowerCase().includes('client is not running')

const waitForLanguageClientRunning = (
  client: MonacoLanguageClient | null,
  timeoutMs = DIRECT_LSP_CLIENT_READY_TIMEOUT_MS
) =>
  new Promise<boolean>((resolve) => {
    const initialState = getLanguageClientRuntimeState(client)
    if (initialState.isRunning) {
      resolve(true)
      return
    }

    const candidate = client as (MonacoLanguageClient & {
      onDidChangeState?: (listener: (event: { newState?: unknown }) => void) => monaco.IDisposable
    }) | null
    if (!candidate?.onDidChangeState) {
      resolve(false)
      return
    }

    let settled = false
    let disposable: monaco.IDisposable | null = null
    const done = (value: boolean) => {
      if (settled) return
      settled = true
      disposable?.dispose?.()
      resolve(value)
    }

    const timer = window.setTimeout(() => {
      done(getLanguageClientRuntimeState(client).isRunning)
    }, Math.max(50, Number(timeoutMs) || DIRECT_LSP_CLIENT_READY_TIMEOUT_MS))

    disposable = candidate.onDidChangeState((event) => {
      const runtimeState = getLanguageClientRuntimeState(client)
      if (runtimeState.isRunning || Number(event?.newState) === 2) {
        window.clearTimeout(timer)
        done(true)
      }
    })
  })

const ensureLspClientReadyForDirectRequest = async (
  entry: LspClientEntry,
  clientKey: string,
  languageId: string,
  documentUri: string
) => {
  const ready = await waitForSharedLspClientReady(entry)
  if (!ready || entry.disposed || !entry.client) return false

  const running = await waitForLanguageClientRunning(entry.client)
  if (running && !entry.disposed && lspClients.get(clientKey) === entry) return true

  const runtimeState = getLanguageClientRuntimeState(entry.client)
  logger.warn('[LSP] direct completion found non-running shared client; recycling stale entry', {
    languageId,
    clientKey,
    documentUri,
    publicState: runtimeState.publicState,
    privateState: runtimeState.privateState,
    ready,
    disposed: entry.disposed,
  })
  cleanupLspClientEntry(clientKey, languageId, 'direct-completion-client-not-running', {
    suppressRetryBackoff: true,
  })
  return false
}

const convertMonacoCompletionTriggerKindToLsp = (triggerKind: unknown) => {
  const value = Number(triggerKind)
  if (value === monaco.languages.CompletionTriggerKind.TriggerCharacter) return 2
  if (value === monaco.languages.CompletionTriggerKind.TriggerForIncompleteCompletions) return 3
  return 1
}

type LspTextRange = {
  start?: { line?: number; character?: number }
  end?: { line?: number; character?: number }
}

const convertLspRangeToMonaco = (range: unknown): monaco.IRange | null => {
  const typedRange = range as LspTextRange | null
  if (
    typeof typedRange?.start?.line !== 'number'
    || typeof typedRange?.start?.character !== 'number'
    || typeof typedRange?.end?.line !== 'number'
    || typeof typedRange?.end?.character !== 'number'
  ) {
    return null
  }

  return {
    startLineNumber: typedRange.start.line + 1,
    startColumn: typedRange.start.character + 1,
    endLineNumber: typedRange.end.line + 1,
    endColumn: typedRange.end.character + 1,
  }
}

const getLspRangeSummary = (range: unknown) => {
  const typedRange = range as LspTextRange | null
  if (
    typeof typedRange?.start?.line !== 'number'
    || typeof typedRange?.start?.character !== 'number'
    || typeof typedRange?.end?.line !== 'number'
    || typeof typedRange?.end?.character !== 'number'
  ) {
    return null
  }

  return {
    start: {
      line: typedRange.start.line,
      character: typedRange.start.character,
    },
    end: {
      line: typedRange.end.line,
      character: typedRange.end.character,
    },
  }
}

const getLspTextEditShape = (item: Record<string, unknown>) => {
  const textEdit = item.textEdit as {
    range?: unknown
    insert?: unknown
    replace?: unknown
    newText?: unknown
  } | undefined

  if (!textEdit) return 'none'
  if (textEdit.insert && textEdit.replace) return 'insert-replace'
  if (textEdit.range) return 'range'
  return 'unknown'
}

const getLspTextEditLogPayload = (item: Record<string, unknown> | undefined) => {
  if (!item) return null

  const textEdit = item.textEdit as {
    range?: unknown
    insert?: unknown
    replace?: unknown
    newText?: unknown
  } | undefined

  return {
    label: String(item.label || ''),
    textEditShape: getLspTextEditShape(item),
    hasInsertText: typeof item.insertText === 'string',
    hasTextEditNewText: typeof textEdit?.newText === 'string',
    range: getLspRangeSummary(textEdit?.range),
    insert: getLspRangeSummary(textEdit?.insert),
    replace: getLspRangeSummary(textEdit?.replace),
  }
}

const getIncludePathFallbackRange = (
  model: monaco.editor.ITextModel,
  position: monaco.Position,
  insertText: string
): monaco.IRange | null => {
  const lineContent = model.getLineContent(position.lineNumber)
  const beforeCursor = lineContent.slice(0, position.column - 1)
  const includeMatch = /^\s*#\s*include\s*[<"]([^>"]*)$/.exec(beforeCursor)
  if (!includeMatch) return null

  const typedPath = includeMatch[1] || ''
  if (!typedPath || !insertText.startsWith(typedPath.replace(/[./\\]+$/, ''))) return null

  const startColumn = position.column - typedPath.length
  // 设计说明：
  // 1. clangd 的 include 补全有时不带 textEdit，光标位于 `stdio.` 之后时 Monaco 默认 word range 为空。
  // 2. 此时如果只在光标处插入 `stdio.h`，用户正常输入会得到 `stdio.stdio.h`。
  // 3. 回退范围按 include 分隔符后的整段路径替换，既保留子目录补全，也避免把 `<` / `"` 吃掉。
  logger.debug('[LSP] include completion fallback range resolved', {
    lineNumber: position.lineNumber,
    column: position.column,
    typedPath,
    insertTextPreview: insertText.slice(0, 80),
    startColumn,
  })

  return {
    startLineNumber: position.lineNumber,
    startColumn,
    endLineNumber: position.lineNumber,
    endColumn: position.column,
  }
}

const resolveLspCompletionRangeFromTextEdit = (textEdit: {
  range?: unknown
  insert?: unknown
  replace?: unknown
} | undefined): monaco.IRange | monaco.languages.CompletionItemRanges | null => {
  const range = convertLspRangeToMonaco(textEdit?.range)
  if (range) return range

  const insert = convertLspRangeToMonaco(textEdit?.insert)
  const replace = convertLspRangeToMonaco(textEdit?.replace)
  if (insert && replace) {
    // 设计说明：
    // 1. LSP 3.16 的 InsertReplaceEdit 需要完整传给 Monaco，不能压平成单个 range。
    // 2. clangd 在 include 路径补全里会依赖 replace 覆盖已输入文本，insert 只描述继续输入时的插入区。
    // 3. 保留双范围后，Monaco 可以根据当前接受方式选择正确替换范围，避免 `stdio.stdio.h` 这类重复文本。
    return { insert, replace }
  }
  return replace || insert
}

const getCompletionRangeLogPayload = (
  range: monaco.IRange | monaco.languages.CompletionItemRanges
) => {
  if ('insert' in range && 'replace' in range) {
    return {
      shape: 'insert-replace',
      insert: range.insert,
      replace: range.replace,
    }
  }

  return {
    shape: 'range',
    range,
  }
}

const getFallbackWordRange = (
  model: monaco.editor.ITextModel,
  position: monaco.Position
) => {
  const wordInfo = model.getWordUntilPosition(position)
  return {
    startLineNumber: position.lineNumber,
    startColumn: wordInfo?.startColumn || position.column,
    endLineNumber: position.lineNumber,
    endColumn: wordInfo?.endColumn || position.column,
  }
}

const resolveLspCompletionInsertText = (item: Record<string, unknown>) => {
  const textEdit = item.textEdit as Record<string, unknown> | undefined
  const insertReplaceEdit = textEdit as {
    newText?: unknown
  } | undefined
  return String(
    insertReplaceEdit?.newText
    ?? item.insertText
    ?? item.label
    ?? ''
  )
}

const resolveLspCompletionRange = (
  item: Record<string, unknown>,
  model: monaco.editor.ITextModel,
  position: monaco.Position,
  insertText: string
) => {
  const textEdit = item.textEdit as {
    range?: unknown
    insert?: unknown
    replace?: unknown
  } | undefined
  const editRange = resolveLspCompletionRangeFromTextEdit(textEdit)
  if (editRange) return editRange

  return getIncludePathFallbackRange(model, position, insertText)
    || getFallbackWordRange(model, position)
}

const convertLspCompletionItemToMonaco = (
  item: Record<string, unknown>,
  model: monaco.editor.ITextModel,
  position: monaco.Position,
  index: number
): monaco.languages.CompletionItem | null => {
  const label = String(item.label || '').trim()
  if (!label) return null

  const insertText = resolveLspCompletionInsertText(item)
  if (!insertText) return null

  const sortText = String(item.sortText || '').trim() || `${String(index).padStart(5, '0')}_${label}`
  const filterText = String(item.filterText || '').trim() || label
  const documentation = item.documentation as
    | string
    | { kind?: string; value?: string }
    | undefined

  const range = resolveLspCompletionRange(item, model, position, insertText)
  const completionItem: monaco.languages.CompletionItem = {
    label,
    kind: LSP_TO_MONACO_COMPLETION_KIND[Number(item.kind || 1)] || monaco.languages.CompletionItemKind.Text,
    insertText,
    insertTextRules: Number(item.insertTextFormat || 1) === 2
      ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet
      : undefined,
    detail: typeof item.detail === 'string' ? item.detail : undefined,
    documentation: typeof documentation === 'string'
      ? documentation
      : documentation?.value
        ? { value: documentation.value }
        : undefined,
    sortText,
    filterText,
    range,
    preselect: !!item.preselect,
  }

  if (getLspTextEditShape(item) !== 'none' || /^(?:c|cpp)$/.test(model.getLanguageId())) {
    logger.debug('[LSP] converted completion item', {
      languageId: model.getLanguageId(),
      label,
      textEditShape: getLspTextEditShape(item),
      range: getCompletionRangeLogPayload(range),
      commitCharacterCount: Array.isArray(item.commitCharacters) ? item.commitCharacters.length : 0,
    })
  }

  return completionItem
}

const synchronizeDirectLspTextDocument = async (
  entry: LspClientEntry,
  clientKey: string,
  languageId: string,
  model: monaco.editor.ITextModel
) => {
  const uri = String(model.uri?.toString?.() || entry.documentUri || '')
  const normalizedLanguageId = normalizeLanguageId(languageId)
  const content = String(model.getValue?.() || '')
  if (!uri || !entry.client) return false

  const syncKey = `${clientKey}:${uri}`
  const previous = directSyncedDocuments.get(syncKey)
  const clientWithNotifications = entry.client as unknown as {
    sendNotification: (type: typeof DidOpenTextDocumentNotification.type | typeof DidChangeTextDocumentNotification.type, params: unknown) => Promise<void>
  }

  try {
    // 设计说明：
    // 1. Markdown fence 和 Notebook cell 使用的是虚拟 file:// 文档，Monaco/overlay 内容同步并不总会触发
    //    vscode-languageclient 的 didOpen/didChange 事件。
    // 2. pyright 在未看到最新 textDocument 内容时仍会响应 completion，但会返回 0 个候选，
    //    日志表现为 direct completion request resolved 之后仍然 no suggestions。
    // 3. 因此 direct completion 前主动发送一次全量文档同步，确保服务端用当前模型内容计算补全。
    if (!previous) {
      const version = 1
      await clientWithNotifications.sendNotification(DidOpenTextDocumentNotification.type, {
        textDocument: {
          uri,
          languageId: normalizedLanguageId,
          version,
          text: content,
        },
      })
      directSyncedDocuments.set(syncKey, { version, content, languageId: normalizedLanguageId })
      logger.info('[LSP] direct text document opened before completion', {
        languageId: normalizedLanguageId,
        clientKey,
        documentUri: uri,
        version,
        length: content.length,
      })
      await waitForDirectLspDocumentSync()
      return true
    }

    if (previous.content === content && previous.languageId === normalizedLanguageId) return true

    const version = previous.version + 1
    await clientWithNotifications.sendNotification(DidChangeTextDocumentNotification.type, {
      textDocument: {
        uri,
        version,
      },
      contentChanges: [{ text: content }],
    })
    directSyncedDocuments.set(syncKey, { version, content, languageId: normalizedLanguageId })
    logger.info('[LSP] direct text document changed before completion', {
      languageId: normalizedLanguageId,
      clientKey,
      documentUri: uri,
      version,
      length: content.length,
    })
    await waitForDirectLspDocumentSync()
    return true
  } catch (error) {
    logger.warn('[LSP] direct text document sync failed before completion', {
      languageId: normalizedLanguageId,
      clientKey,
      documentUri: uri,
      error: String((error as Error)?.message || error),
    })
    if (isLspClientNotRunningError(error)) {
      cleanupLspClientEntry(clientKey, normalizedLanguageId, 'direct-sync-client-not-running', {
        suppressRetryBackoff: true,
      })
      return false
    }
    return true
  }
}

export const provideDirectLspCompletionSuggestions = async (
  languageId: string,
  model: monaco.editor.ITextModel,
  position: monaco.Position,
  context: monaco.languages.CompletionContext | undefined,
  token: monaco.CancellationToken | undefined
) => {
  const normalizedLanguageId = normalizeLanguageId(languageId)
  const clientKey = getLspClientKey(normalizedLanguageId)
  const entry = lspClients.get(clientKey)
  if (!entry?.client || entry.disposed) {
    return []
  }
  const documentUri = String(model.uri?.toString?.() || entry.documentUri || '')
  const directClientReady = await ensureLspClientReadyForDirectRequest(
    entry,
    clientKey,
    normalizedLanguageId,
    documentUri
  )
  if (!directClientReady || token?.isCancellationRequested) {
    logger.debug('[LSP] direct completion skipped because language client is not running', {
      languageId: normalizedLanguageId,
      clientKey,
      documentUri,
      state: getLspConnectionState(normalizedLanguageId).state,
    })
    return []
  }
  const connectionStateBeforeDirectCompletion = getLspConnectionState(normalizedLanguageId).state
  if (connectionStateBeforeDirectCompletion !== 'connected') {
    if (token?.isCancellationRequested) {
      logger.debug('[LSP] direct completion skipped because shared client is not ready', {
        languageId: normalizedLanguageId,
        clientKey,
        state: getLspConnectionState(normalizedLanguageId).state,
        documentUri,
      })
      return []
    }

    getLspClientLanguages(normalizedLanguageId).forEach((lang) => {
      setLspConnectionState(lang, 'connected', 'shared-client-direct-completion')
      ensureDirectCompletionProviderRegistered(lang, 'shared-client-direct-completion')
    })
    logger.info('[LSP] direct completion using ready shared client', {
      languageId: normalizedLanguageId,
      clientKey,
      previousState: connectionStateBeforeDirectCompletion,
      documentUri,
    })
  }
  ensureDirectCompletionProviderRegistered(normalizedLanguageId, 'direct-completion-request')

  const params = {
    textDocument: {
      uri: documentUri,
    },
    position: {
      line: Math.max(0, Number(position.lineNumber || 1) - 1),
      character: Math.max(0, Number(position.column || 1) - 1),
    },
    context: {
      triggerKind: convertMonacoCompletionTriggerKindToLsp(context?.triggerKind),
      triggerCharacter: context?.triggerCharacter || undefined,
    },
  }

  try {
    const synced = await synchronizeDirectLspTextDocument(entry, clientKey, normalizedLanguageId, model)
    if (!synced || token?.isCancellationRequested) return []

    // 设计说明：
    // 1. monaco-languageclient 正常会通过 VSCode API 注册 completion provider。
    // 2. 当前应用里存在 VSCode registry 已连接但 Standalone Monaco registry 为空的场景，
    //    表现为 LSP connected、suggest widget 打开，但候选数始终为 0。
    // 3. 这里保留标准 client 生命周期，只在 provider 层补一条直接 textDocument/completion
    //    请求，让编辑模式、探索者模式和 Markdown fence fallback 都走同一个 LSP client。
    const result = await (entry.client as unknown as {
      sendRequest: (type: typeof CompletionRequest.type, params: unknown, token?: unknown) => Promise<unknown>
    }).sendRequest(CompletionRequest.type, params, token)

    if (token?.isCancellationRequested) return []
    const rawItems = Array.isArray(result)
      ? result
      : Array.isArray((result as { items?: unknown[] } | null)?.items)
        ? ((result as { items: unknown[] }).items)
        : []

    logger.info('[LSP] direct completion request resolved', {
      languageId: normalizedLanguageId,
      clientKey,
      documentUri: params.textDocument.uri,
      lineNumber: position.lineNumber,
      column: position.column,
      rawItemCount: rawItems.length,
      isIncomplete: !!(result as { isIncomplete?: boolean } | null)?.isIncomplete,
    })

    return rawItems
      .map((item, index) => convertLspCompletionItemToMonaco(
        item as Record<string, unknown>,
        model,
        position,
        index
      ))
      .filter((item): item is monaco.languages.CompletionItem => !!item)
  } catch (error) {
    logger.warn('[LSP] direct completion request failed', {
      languageId: normalizedLanguageId,
      clientKey,
      documentUri: params.textDocument.uri,
      lineNumber: position.lineNumber,
      column: position.column,
      error: String((error as Error)?.message || error),
    })
    if (isLspClientNotRunningError(error)) {
      cleanupLspClientEntry(clientKey, normalizedLanguageId, 'direct-request-client-not-running', {
        suppressRetryBackoff: true,
      })
    }
    return []
  }
}

// ───── LSP 语义着色（semantic tokens）─────
// 设计说明：
// 1. 本项目编辑器着色默认走后端 tree-sitter，但 typst 没有 tree-sitter grammar；
//    tinymist 已通过 LSP 提供高质量 semantic tokens，这里把它接到 Monaco 的 DocumentSemanticTokensProvider。
// 2. 复用已建立的共享 LSP client（与补全同一条），按需取 server 在 initialize 时声明的 legend 解码 token 类型。
// 3. LSP 的 semantic tokens data 是 5 元组 delta 编码（deltaLine, deltaStartChar, length, tokenType, tokenModifiers），
//    Monaco 的 provider 期望同样的 5 元组 delta 编码，因此 data 可直接透传，只需把 legend 透传给 getLegend()。

type LspSemanticLegend = {
  tokenTypes: string[]
  tokenModifiers: string[]
}

const getServerSemanticLegend = (client: MonacoLanguageClient | null): LspSemanticLegend | null => {
  const initResult = (client as unknown as {
    initializeResult?: {
      capabilities?: {
        semanticTokensProvider?: {
          legend?: { tokenTypes?: string[]; tokenModifiers?: string[] }
          full?: unknown
        }
      }
    }
  } | null)?.initializeResult
  const provider = initResult?.capabilities?.semanticTokensProvider
  const legend = provider?.legend
  if (!legend || !Array.isArray(legend.tokenTypes) || !legend.tokenTypes.length) return null
  return {
    tokenTypes: legend.tokenTypes,
    tokenModifiers: Array.isArray(legend.tokenModifiers) ? legend.tokenModifiers : [],
  }
}

export const getLspSemanticTokensLegend = (languageId: string): monaco.languages.SemanticTokensLegend | null => {
  const normalizedLanguageId = normalizeLanguageId(languageId)
  const clientKey = getLspClientKey(normalizedLanguageId)
  const entry = lspClients.get(clientKey)
  const legend = getServerSemanticLegend(entry?.client || null)
  if (!legend) return null
  return { tokenTypes: legend.tokenTypes, tokenModifiers: legend.tokenModifiers }
}

export const provideLspSemanticTokens = async (
  languageId: string,
  model: monaco.editor.ITextModel,
  token: monaco.CancellationToken | undefined
): Promise<monaco.languages.SemanticTokens | null> => {
  const normalizedLanguageId = normalizeLanguageId(languageId)
  const clientKey = getLspClientKey(normalizedLanguageId)
  const entry = lspClients.get(clientKey)
  if (!entry?.client || entry.disposed) return null

  const documentUri = String(model.uri?.toString?.() || entry.documentUri || '')
  const ready = await ensureLspClientReadyForDirectRequest(
    entry,
    clientKey,
    normalizedLanguageId,
    documentUri
  )
  if (!ready || token?.isCancellationRequested) return null

  const legend = getServerSemanticLegend(entry.client)
  if (!legend) {
    logger.debug('[LSP] semantic tokens unavailable: server has no legend', {
      languageId: normalizedLanguageId,
      clientKey,
    })
    return null
  }

  try {
    const synced = await synchronizeDirectLspTextDocument(entry, clientKey, normalizedLanguageId, model)
    if (!synced || token?.isCancellationRequested) return null

    const result = await (entry.client as unknown as {
      sendRequest: (type: typeof SemanticTokensRequest.type, params: unknown, token?: unknown) => Promise<unknown>
    }).sendRequest(
      SemanticTokensRequest.type,
      { textDocument: { uri: documentUri } },
      token
    )
    if (token?.isCancellationRequested) return null

    const data = (result as { data?: number[] } | null)?.data
    if (!Array.isArray(data) || !data.length) {
      return { data: new Uint32Array() }
    }
    logger.debug('[LSP] semantic tokens resolved', {
      languageId: normalizedLanguageId,
      clientKey,
      tokenCount: Math.floor(data.length / 5),
    })
    return { data: Uint32Array.from(data) }
  } catch (error) {
    logger.warn('[LSP] semantic tokens request failed', {
      languageId: normalizedLanguageId,
      clientKey,
      error: String((error as Error)?.message || error),
    })
    if (isLspClientNotRunningError(error)) {
      cleanupLspClientEntry(clientKey, normalizedLanguageId, 'semantic-tokens-client-not-running', {
        suppressRetryBackoff: true,
      })
    }
    return null
  }
}

const ensureDirectCompletionProviderRegistered = (languageId: string, reason = 'client-start') => {
  const normalizedLanguageId = normalizeLanguageId(languageId)
  if (!normalizedLanguageId || directCompletionProviderDisposables.has(normalizedLanguageId)) return

  const disposable = monaco.languages.registerCompletionItemProvider(normalizedLanguageId, {
    triggerCharacters: DIRECT_COMPLETION_TRIGGER_CHARACTERS,
    async provideCompletionItems(model, position, context, token) {
      const suggestions = await provideDirectLspCompletionSuggestions(
        normalizedLanguageId,
        model,
        position,
        context,
        token
      )
      return { suggestions }
    },
  })
  directCompletionProviderDisposables.set(normalizedLanguageId, disposable)
  logger.info('[LSP] registered direct Monaco completion provider', {
    languageId: normalizedLanguageId,
    reason,
    triggerCharacters: DIRECT_COMPLETION_TRIGGER_CHARACTERS,
  })
}

const waitForSharedLspClientReady = (
  entry: LspClientEntry,
  timeoutMs = DIRECT_LSP_CLIENT_READY_TIMEOUT_MS
) =>
  new Promise<boolean>((resolve) => {
    if (!entry || entry.disposed) {
      resolve(false)
      return
    }

    let settled = false
    let timer: ReturnType<typeof window.setTimeout> | null = null
    const done = (value: boolean) => {
      if (settled) return
      settled = true
      if (timer) window.clearTimeout(timer)
      resolve(value)
    }
    timer = window.setTimeout(() => done(false), Math.max(50, Number(timeoutMs) || DIRECT_LSP_CLIENT_READY_TIMEOUT_MS))

    Promise.resolve(entry.readyPromise)
      .then((ready) => done(!!ready && !entry.disposed && !!entry.client))
      .catch(() => done(false))
  })

const markSharedLspClientReadyForLanguage = (
  entry: LspClientEntry,
  clientKey: string,
  languageId: string,
  reason: string
) => {
  const normalizedLanguageId = normalizeLanguageId(languageId)
  if (!entry || !normalizedLanguageId) return

  void waitForSharedLspClientReady(entry)
    .then((ready) => {
      if (!ready || entry.disposed || lspClients.get(clientKey) !== entry) return

      const languages = getLspClientLanguages(normalizedLanguageId)
      languages.forEach((lang) => {
        setLspConnectionState(lang, 'connected', reason)
        ensureDirectCompletionProviderRegistered(lang, reason)
      })
      logger.info('[LSP] shared client marked ready for language', {
        languageId: normalizedLanguageId,
        clientKey,
        reason,
        languages,
        activeDocumentUri: entry.documentUri,
      })
    })
}

type LspConnectionStateValue = 'idle' | 'connecting' | 'connected' | 'disconnected' | 'disabled'
type LspConnectionState = {
  state: LspConnectionStateValue
  reason?: string
  updatedAt: number
}
type LspConnectionStatePayload = {
  languageId: string
  state: LspConnectionStateValue
  reason?: string
  updatedAt: number
}

type ResolvedLspDocumentContext = {
  documentUri: string
  workspacePath: string | null
  workspaceUri: string | null
}

type InitLspClientOptions = {
  workspacePath?: string
  documentUri?: string
  forceRetry?: boolean
  requestSource?: string
}

type OutputChannelLike = {
  append(value: string): void
  appendLine(value: string): void
  clear(): void
  replace(value: string): void
  show(preserveFocus?: boolean): void
  hide(): void
  dispose(): void
}

const lspConnectionState = new Map<string, LspConnectionState>()
const lspConnectionListeners = new Set<(payload: LspConnectionStatePayload) => void>()
const LSP_NOOP_OUTPUT_CHANNEL: OutputChannelLike = {
  append() { },
  appendLine() { },
  clear() { },
  replace() { },
  show() { },
  hide() { },
  dispose() { },
}

const notifyLspConnectionState = (payload: LspConnectionStatePayload) => {
  lspConnectionListeners.forEach((listener) => {
    try {
      listener(payload)
    } catch (error) {
      logger.warn('[LSP] state listener failed', { error })
    }
  })
}

const setLspConnectionState = (languageId: string, state: LspConnectionStateValue, reason?: string) => {
  const prev = lspConnectionState.get(languageId)
  if (prev && prev.state === state && prev.reason === reason) return
  const next = { state, reason, updatedAt: Date.now() }
  lspConnectionState.set(languageId, next)
  logger.info('[LSP] state updated', { languageId, state, reason })
  notifyLspConnectionState({ languageId, ...next })
}

export function getLspConnectionState(languageId: string): LspConnectionState {
  const key = normalizeLanguageId(languageId)
  if (!key) {
    return { state: 'idle', reason: 'empty-language', updatedAt: Date.now() }
  }
  return lspConnectionState.get(key) || { state: 'idle', updatedAt: Date.now() }
}

export function onLspConnectionStateChange(
  listener: (payload: LspConnectionStatePayload) => void
) {
  lspConnectionListeners.add(listener)
  return () => {
    lspConnectionListeners.delete(listener)
  }
}

const resolveErrorMessage = (error: unknown) => {
  if (error instanceof Error) return error.message
  return String(error || '')
}

const shouldDisableLsp = (error: unknown) => {
  const message = resolveErrorMessage(error)
  return message.includes(LSP_VSCODE_API_NOT_READY)
}

const isFatalLspError = (error: unknown) => {
  const message = resolveErrorMessage(error).toLowerCase()
  if (!message) return false
  return (
    message.includes('already exists') ||
    message.includes('server initialization failed') ||
    message.includes('couldn\'t create connection to server') ||
    message.includes('stopping the server timed out')
  )
}

const safeDisposeTransport = (value: unknown) => {
  try {
    const candidate = value as { dispose?: () => void } | null | undefined
    candidate?.dispose?.()
  } catch {
  }
}

export function waitForLspConnection(languageId: string, timeoutMs: number = 1500): Promise<boolean> {
  const lang = normalizeLanguageId(languageId)
  if (!lang) return Promise.resolve(false)

  const current = getLspConnectionState(lang)
  if (current.state === 'connected') return Promise.resolve(true)
  if (current.state === 'disabled') return Promise.resolve(false)

  const existingEntry = lspClients.get(getLspClientKey(lang))
  if (existingEntry?.readyPromise) {
    return new Promise((resolve) => {
      const timer = window.setTimeout(() => {
        resolve(getLspConnectionState(lang).state === 'connected')
      }, Math.max(50, Number(timeoutMs) || 1500))

      Promise.resolve(existingEntry.readyPromise)
        .then((value) => {
          window.clearTimeout(timer)
          if (value && getLspConnectionState(lang).state !== 'connected') {
            getLspClientLanguages(lang).forEach((item) => {
              setLspConnectionState(item, 'connected', 'wait-shared-client-ready')
              ensureDirectCompletionProviderRegistered(item, 'wait-shared-client-ready')
            })
          }
          resolve(!!value)
        })
        .catch(() => {
          window.clearTimeout(timer)
          resolve(false)
        })
    })
  }

  return new Promise((resolve) => {
    let settled = false
    let timer: ReturnType<typeof window.setTimeout> | null = null

    const done = (value: boolean) => {
      if (settled) return
      settled = true
      if (timer) window.clearTimeout(timer)
      off()
      resolve(value)
    }

    const off = onLspConnectionStateChange((payload) => {
      if (String(payload.languageId || '').trim().toLowerCase() !== lang) return
      if (payload.state === 'connected') {
        done(true)
      } else if (payload.state === 'disabled') {
        done(false)
      }
    })

    timer = window.setTimeout(() => {
      done(getLspConnectionState(lang).state === 'connected')
    }, Math.max(50, Number(timeoutMs) || 1500))
  })
}

const stopClientSafely = async (client: MonacoLanguageClient | null) => {
  if (!client) return

  try {
    const disposableClient = client as MonacoLanguageClient & {
      stop?: (timeout?: number) => Promise<void> | void
      state?: unknown
      _state?: unknown
    }
    const currentState = String(disposableClient.state || disposableClient._state || '').toLowerCase()
    if (currentState.includes('starting')) {
      logger.warn('[LSP] skip stop while client is still starting', { currentState })
      return
    }

    if (typeof disposableClient.stop === 'function') {
      await Promise.resolve(disposableClient.stop(LSP_STOP_TIMEOUT_MS)).catch(() => { })
    }
  } catch {
  }
}

const closeSocketSafely = (webSocket: WebSocket) => {
  try {
    if (webSocket.readyState === WebSocket.OPEN || webSocket.readyState === WebSocket.CONNECTING) {
      webSocket.close()
    }
  } catch {
  }
}

const normalizeComparableLspPath = (value: string | null | undefined) =>
  String(value || '')
    .trim()
    .replace(/\\/g, '/')
    .replace(/\/+$/, '')
    .toLowerCase()

const hasLspClientContextChanged = (
  entry: LspClientEntry,
  context: ResolvedLspDocumentContext
) => {
  const currentWorkspace = normalizeComparableLspPath(entry.workspacePath)
  const nextWorkspace = normalizeComparableLspPath(context.workspacePath)
  if (currentWorkspace || nextWorkspace) {
    return currentWorkspace !== nextWorkspace
  }

  const currentDocument = normalizeComparableLspPath(entry.documentUri)
  const nextDocument = normalizeComparableLspPath(context.documentUri)
  return !!currentDocument && !!nextDocument && currentDocument !== nextDocument
}

const cleanupLspClientEntry = (
  clientKey: string,
  languageId: string,
  reason: string,
  options: {
    manualDisposing?: boolean
    suppressRetryBackoff?: boolean
  } = {}
) => {
  const cur = lspClients.get(clientKey)
  if (!cur || cur.disposed) return
  if (options.manualDisposing || reason === 'manual-dispose') {
    cur.manualDisposing = true
  }
  cur.disposed = true

  cur.webSocket.onopen = null
  cur.webSocket.onclose = null
  cur.webSocket.onerror = null

  safeDisposeTransport(cur.reader)
  cur.reader = null
  safeDisposeTransport(cur.writer)
  cur.writer = null
  const client = cur.client
  cur.client = null

  void stopClientSafely(client).finally(() => {
    closeSocketSafely(cur.webSocket)
  })
  cur.resolveReady(false)

  lspClients.delete(clientKey)
  Array.from(directSyncedDocuments.keys()).forEach((syncKey) => {
    if (syncKey.startsWith(`${clientKey}:`)) {
      directSyncedDocuments.delete(syncKey)
    }
  })
  const relatedLanguages = getLspClientLanguages(cur.languageId || languageId)
  if (relatedLanguages.some((lang) => lspDisabled.has(lang))) {
    relatedLanguages.forEach((lang) => {
      const disabledReason = lspDisabledReason.get(lang) || reason
      setLspConnectionState(lang, 'disabled', disabledReason)
    })
  } else {
    relatedLanguages.forEach((lang) => {
      setLspConnectionState(lang, 'disconnected', reason)
    })
  }
  if (!options.suppressRetryBackoff && reason !== 'manual-dispose') {
    lspRetryBlockedUntil.set(clientKey, Date.now() + LSP_RETRY_BACKOFF_MS)
  }
}

const disableLsp = (languageId: string, reason: string, error?: unknown) => {
  const relatedLanguages = getLspClientLanguages(languageId)
  relatedLanguages.forEach((lang) => {
    if (!lspDisabled.has(lang)) {
      lspDisabled.add(lang)
    }
    if (!lspDisabledReason.has(lang)) {
      lspDisabledReason.set(lang, reason)
    }
    setLspConnectionState(lang, 'disabled', reason)
  })
  logger.warn('[LSP] disabled', { languageId, relatedLanguages, reason, error })
}

const isLikelyFilePath = (filePath: string) => {
  const path = String(filePath || '').trim()
  if (!path) return false
  if (path.startsWith('file://')) return true
  if (/^[a-zA-Z]:[\\/]/.test(path)) return true
  if (path.startsWith('\\')) return true
  return path.startsWith('/')
}

const getParentDirectoryPath = (filePath: string) => {
  const normalized = String(filePath || '').trim()
  if (!normalized) return null
  const sanitized = normalized.replace(/[\\/]+$/, '')
  const slashIndex = Math.max(sanitized.lastIndexOf('/'), sanitized.lastIndexOf('\\'))
  if (slashIndex <= 0) return null
  return sanitized.slice(0, slashIndex)
}

const resolveWorkspacePath = (rawPath: string) => {
  const normalized = String(rawPath || '').trim()
  if (!normalized) return null

  if (normalized.startsWith('file://')) {
    try {
      const uri = monaco.Uri.parse(normalized)
      const fsPath = String(uri.fsPath || '').trim()
      if (!isLikelyFilePath(fsPath)) return null
      return getParentDirectoryPath(fsPath)
    } catch (error) {
      logger.warn('[LSP] failed to resolve workspace path from file URI', { rawPath: normalized, error })
      return null
    }
  }

  if (!isLikelyFilePath(normalized)) return null
  return getParentDirectoryPath(normalized)
}

const resolveDocumentUri = (rawPath: string) => {
  const normalized = String(rawPath || '').trim()
  if (!normalized) return ''

  if (normalized.startsWith('file://')) {
    try {
      return monaco.Uri.parse(normalized).toString()
    } catch (error) {
      logger.warn('[LSP] failed to normalize existing file URI', { rawPath: normalized, error })
      return normalized
    }
  }

  if (!isLikelyFilePath(normalized)) return normalized

  try {
    return monaco.Uri.file(normalized).toString()
  } catch (error) {
    logger.warn('[LSP] failed to convert file path into Monaco URI', { rawPath: normalized, error })
    return normalized
  }
}

const resolveLspDocumentContext = (
  documentTarget: string,
  workspaceTarget: string
): ResolvedLspDocumentContext => {
  const documentUri = resolveDocumentUri(documentTarget)
  const workspacePath = resolveWorkspacePath(workspaceTarget || documentTarget)
  const workspaceUri = workspacePath ? resolveDocumentUri(workspacePath) : null
  return {
    documentUri,
    workspacePath,
    workspaceUri,
  }
}

const recordLspFailure = (languageId: string, reason: string, error?: unknown) => {
  const lang = getLspClientKey(languageId)
  if (!lang) return false
  const now = Date.now()
  const prev = lspFailureTracker.get(lang)
  if (!prev || now - prev.firstAt > LSP_FAILURE_WINDOW_MS) {
    lspFailureTracker.set(lang, { count: 1, firstAt: now })
    logger.warn('[LSP] failure recorded', { languageId: lang, reason, count: 1, error })
    return false
  }
  const next = { count: prev.count + 1, firstAt: prev.firstAt }
  lspFailureTracker.set(lang, next)
  logger.warn('[LSP] failure recorded', { languageId: lang, reason, count: next.count, error })
  if (next.count >= LSP_FAILURE_THRESHOLD) {
    disableLsp(lang, `frequent-failures:${reason}`, error)
    lspFailureTracker.delete(lang)
    return true
  }
  return false
}

export function resetLspDisabled(languageId?: string) {
  if (languageId) {
    const lang = String(languageId || '').trim().toLowerCase()
    if (!lang) return
    lspDisabled.delete(lang)
    lspDisabledReason.delete(lang)
    lspFailureTracker.delete(lang)
    setLspConnectionState(lang, 'idle', 'manual-reset')
    return
  }
  lspDisabled.clear()
  lspDisabledReason.clear()
  lspFailureTracker.clear()
}

export function initLspClient(
  editor: monaco.editor.IStandaloneCodeEditor,
  languageId: string,
  filePath: string,
  options: InitLspClientOptions = {}
) {
  if (!editor || !languageId) return

  if (!isLspSupportedLanguage(languageId)) {
    const normalizedLanguageId = normalizeLanguageId(languageId)
    if (normalizedLanguageId) {
      setLspConnectionState(normalizedLanguageId, 'disconnected', 'runtime-not-ready')
      logger.warn('[LSP] init skipped because runtime extension is not ready', {
        languageId: normalizedLanguageId,
        filePath,
        requestSource: String(options.requestSource || 'unknown'),
      })
    }
    return
  }

  const normalizedLanguageId = normalizeLanguageId(languageId)
  const clientKey = getLspClientKey(normalizedLanguageId)
  const documentSelector = getLspDocumentSelectorForLanguage(normalizedLanguageId)
  const startupLanguageId = normalizeLanguageId(documentSelector[0]?.language || normalizedLanguageId)
  const modelDocumentUri = String(editor.getModel?.()?.uri?.toString?.() || '').trim()
  const documentTarget = String(options.documentUri || modelDocumentUri || filePath || '').trim()
  const workspaceTarget = String(options.workspacePath || filePath || '').trim()
  const resolvedDocumentContext = resolveLspDocumentContext(documentTarget, workspaceTarget)
  const requestSource = String(options.requestSource || 'unknown')
  const forceRetry = !!options.forceRetry

  if (!resolvedDocumentContext.documentUri) {
    getLspClientLanguages(normalizedLanguageId).forEach((lang) => {
      setLspConnectionState(lang, 'disconnected', 'invalid-file-path')
    })
    logger.warn('[LSP] skipped invalid file path', {
      languageId: normalizedLanguageId,
      filePath,
      modelDocumentUri,
      documentTarget,
      workspaceTarget,
    })
    lspRetryBlockedUntil.set(clientKey, Date.now() + LSP_RETRY_BACKOFF_MS)
    return
  }

  logger.info('[LSP] init request observed', {
    languageId: normalizedLanguageId,
    clientKey,
    documentUri: resolvedDocumentContext.documentUri,
    workspacePath: resolvedDocumentContext.workspacePath,
    workspaceUri: resolvedDocumentContext.workspaceUri,
    requestSource,
    forceRetry,
  })

  if (lspDisabled.has(normalizedLanguageId)) {
    logger.warn('[LSP] init skipped because language is disabled', {
      languageId: normalizedLanguageId,
      clientKey,
      requestSource,
      disabledReason: lspDisabledReason.get(normalizedLanguageId) || '',
    })
    return
  }
  const existingClient = lspClients.get(clientKey)
  if (existingClient) {
    if (hasLspClientContextChanged(existingClient, resolvedDocumentContext)) {
      logger.info('[LSP] init will recycle existing client because document context changed', {
        languageId: normalizedLanguageId,
        clientKey,
        requestSource,
        prevDocumentUri: existingClient.documentUri,
        prevWorkspacePath: existingClient.workspacePath,
        nextDocumentUri: resolvedDocumentContext.documentUri,
        nextWorkspacePath: resolvedDocumentContext.workspacePath,
      })
      cleanupLspClientEntry(clientKey, normalizedLanguageId, 'context-changed', {
        suppressRetryBackoff: true,
      })
    } else {
      logger.debug('[LSP] init reused existing client', {
        languageId: normalizedLanguageId,
        clientKey,
        requestSource,
        state: getLspConnectionState(normalizedLanguageId).state,
        documentUri: existingClient.documentUri,
        workspacePath: existingClient.workspacePath,
      })
      if (getLspConnectionState(normalizedLanguageId).state === 'connected') {
        getLspClientLanguages(normalizedLanguageId).forEach((lang) => {
          ensureDirectCompletionProviderRegistered(lang, 'reuse-connected-client')
        })
      } else {
        markSharedLspClientReadyForLanguage(existingClient, clientKey, normalizedLanguageId, 'reuse-shared-client-ready')
      }
      return
    }
  }

  if (lspClients.has(clientKey)) {
    logger.debug('[LSP] init reused existing client', {
      languageId: normalizedLanguageId,
      clientKey,
      requestSource,
      state: getLspConnectionState(normalizedLanguageId).state,
    })
    if (getLspConnectionState(normalizedLanguageId).state === 'connected') {
      getLspClientLanguages(normalizedLanguageId).forEach((lang) => {
        ensureDirectCompletionProviderRegistered(lang, 'reuse-existing-entry')
      })
    } else {
      const existingEntry = lspClients.get(clientKey)
      if (existingEntry) {
        markSharedLspClientReadyForLanguage(existingEntry, clientKey, normalizedLanguageId, 'reuse-existing-entry-ready')
      }
    }
    return
  }
  const blockedUntil = lspRetryBlockedUntil.get(clientKey) || 0
  if (!forceRetry && Date.now() < blockedUntil) {
    logger.warn('[LSP] init skipped because retry backoff is active', {
      languageId: normalizedLanguageId,
      clientKey,
      requestSource,
      remainingMs: Math.max(0, blockedUntil - Date.now()),
    })
    return
  }
  if (forceRetry && Date.now() < blockedUntil) {
    logger.info('[LSP] init is overriding retry backoff because request is interactive', {
      languageId: normalizedLanguageId,
      clientKey,
      requestSource,
      remainingMs: Math.max(0, blockedUntil - Date.now()),
    })
    lspRetryBlockedUntil.delete(clientKey)
  }

  logger.info('[LSP] init request accepted', {
    languageId: normalizedLanguageId,
    clientKey,
    documentUri: resolvedDocumentContext.documentUri,
    workspacePath: resolvedDocumentContext.workspacePath,
    workspaceUri: resolvedDocumentContext.workspaceUri,
    requestSource,
    forceRetry,
  })

  const protocol =
    window.location.protocol === 'https:' ? 'wss' : 'ws'
  const host = '127.0.0.1:8888'
  const url = `${protocol}://${host}/lsp/${startupLanguageId}`

  const webSocket = new WebSocket(url)
  let resolveReady = (_value: boolean) => { }
  const readyPromise = new Promise<boolean>((resolve) => {
    resolveReady = resolve
  })
  const entry: LspClientEntry = {
    client: null,
    webSocket,
    disposed: false,
    manualDisposing: false,
    reader: null,
    writer: null,
    readyPromise,
    resolveReady,
    languageId: normalizedLanguageId,
    documentUri: resolvedDocumentContext.documentUri,
    workspacePath: resolvedDocumentContext.workspacePath,
    workspaceUri: resolvedDocumentContext.workspaceUri,
  }
  lspClients.set(clientKey, entry)
  getLspClientLanguages(normalizedLanguageId).forEach((lang) => {
    setLspConnectionState(lang, 'connecting')
  })

  const cleanup = (reason: string) => {
    cleanupLspClientEntry(clientKey, normalizedLanguageId, reason)
  }

  webSocket.onopen = () => {
    logger.info('[LSP] websocket opened', {
      languageId: normalizedLanguageId,
      clientKey,
      url,
      requestSource,
    })
    const socket = toSocket(webSocket)
    const reader = new WebSocketMessageReader(socket)
    const writer = new WebSocketMessageWriter(socket)
    entry.reader = reader
    entry.writer = writer

    const messageTransports: MessageTransports = { reader, writer }
    // Design note:
    // 1. monaco-languageclient v10 expects transports inside the constructor options.
    // 2. Without this binding the editor falls back to empty Monaco providers or word completion.
    // 3. Completion, diagnostics, hover, and semantic tokens share one standard LSP client path.

    let client: MonacoLanguageClient
    try {
      client = new MonacoLanguageClient({
        id: `${clientKey}-language-client`,
        name: `${clientKey} Language Client`,
        clientOptions: {
          documentSelector,
          outputChannel: LSP_NOOP_OUTPUT_CHANNEL as never,
          traceOutputChannel: LSP_NOOP_OUTPUT_CHANNEL as never,
          initializationOptions: {
            workspaceFolders: resolvedDocumentContext.workspaceUri && resolvedDocumentContext.workspacePath
              ? [{
                  uri: resolvedDocumentContext.workspaceUri,
                  name: resolvedDocumentContext.workspacePath.split(/[\\/]/).pop() || resolvedDocumentContext.workspacePath,
                }]
              : [],
            filePath: resolvedDocumentContext.workspacePath || filePath,
            documentUri: resolvedDocumentContext.documentUri,
            workspacePath: resolvedDocumentContext.workspacePath,
            workspaceUri: resolvedDocumentContext.workspaceUri,
            rootUri: resolvedDocumentContext.workspaceUri,
            rootPath: resolvedDocumentContext.workspacePath,
          },
          // Avoid restart loop/noisy reconnect logs when socket is intentionally closed.
          errorHandler: {
            error: () => ({ action: ErrorAction.Continue }),
            closed: () => ({ action: CloseAction.DoNotRestart }),
          },
        },
        messageTransports,
      })
    } catch (error) {
      if (shouldDisableLsp(error)) {
        disableLsp(normalizedLanguageId, 'vscode-api-not-ready', error)
      } else if (isFatalLspError(error)) {
        disableLsp(normalizedLanguageId, 'fatal-client-init-error', error)
      } else {
        recordLspFailure(normalizedLanguageId, 'client-init-failed', error)
        logger.warn('[LSP] client init failed', { languageId: normalizedLanguageId, clientKey, error })
      }
      cleanup('client-init-failed')
      return
    }

    const cur = lspClients.get(clientKey)
    if (!cur || cur.disposed) {
      try {
        webSocket.close()
      } catch {
      }
      return
    }
    cur.client = client
    lspFailureTracker.delete(clientKey)
    lspRetryBlockedUntil.delete(clientKey)

    Promise.resolve(client.start())
      .then(() => {
        const active = lspClients.get(clientKey)
        if (!active || active.disposed) return
        logger.info('[LSP] client start resolved', {
          languageId: normalizedLanguageId,
          clientKey,
          requestSource,
          documentUri: resolvedDocumentContext.documentUri,
        })
        getLspClientLanguages(normalizedLanguageId).forEach((lang) => {
          setLspConnectionState(lang, 'connected')
          ensureDirectCompletionProviderRegistered(lang, 'client-start-resolved')
        })
        active.resolveReady(true)
      })
      .catch((error) => {
        if (shouldDisableLsp(error)) {
          disableLsp(normalizedLanguageId, 'vscode-api-not-ready', error)
        } else if (isFatalLspError(error)) {
          disableLsp(normalizedLanguageId, 'fatal-start-error', error)
        } else {
          recordLspFailure(normalizedLanguageId, 'start-failed', error)
          logger.warn('[LSP] client start failed', { languageId: normalizedLanguageId, clientKey, error })
        }
        cleanup('start-failed')
      })
  }

  webSocket.onclose = (event) => {
    logger.warn('[LSP] websocket closed', {
      languageId: normalizedLanguageId,
      clientKey,
      code: Number(event?.code || 0),
      reason: String(event?.reason || ''),
      wasClean: !!event?.wasClean,
    })
    cleanup('socket-close')
  }

  webSocket.onerror = (event) => {
    logger.warn('[LSP] websocket errored', {
      languageId: normalizedLanguageId,
      clientKey,
      eventType: String(event?.type || 'error'),
    })
    recordLspFailure(languageId, 'socket-error')
    cleanup('socket-error')
  }
}

export function disposeAllLspClients() {
  lspClients.forEach((entry, clientKey) => {
    if (entry.disposed) return
    cleanupLspClientEntry(clientKey, entry.languageId, 'manual-dispose', {
      manualDisposing: true,
      suppressRetryBackoff: true,
    })
  })
  lspConnectionState.forEach((_value, languageId) => {
    setLspConnectionState(languageId, 'disconnected', 'manual-dispose')
  })
}
