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

const waitForDirectLspDocumentSync = () =>
  new Promise((resolve) => {
    window.setTimeout(resolve, DIRECT_LSP_SYNC_SETTLE_MS)
  })

const convertMonacoCompletionTriggerKindToLsp = (triggerKind: unknown) => {
  const value = Number(triggerKind)
  if (value === monaco.languages.CompletionTriggerKind.TriggerCharacter) return 2
  if (value === monaco.languages.CompletionTriggerKind.TriggerForIncompleteCompletions) return 3
  return 1
}

const convertLspRangeToMonaco = (range: unknown) => {
  const typedRange = range as {
    start?: { line?: number; character?: number }
    end?: { line?: number; character?: number }
  } | null
  if (
    typeof typedRange?.start?.line !== 'number'
    || typeof typedRange?.start?.character !== 'number'
    || typeof typedRange?.end?.line !== 'number'
    || typeof typedRange?.end?.character !== 'number'
  ) {
    return null
  }

  return new monaco.Range(
    typedRange.start.line + 1,
    typedRange.start.character + 1,
    typedRange.end.line + 1,
    typedRange.end.character + 1
  )
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
  position: monaco.Position
) => {
  const textEdit = item.textEdit as {
    range?: unknown
    insert?: unknown
    replace?: unknown
  } | undefined
  const editRange = convertLspRangeToMonaco(textEdit?.range)
    || convertLspRangeToMonaco(textEdit?.insert)
    || convertLspRangeToMonaco(textEdit?.replace)
  if (editRange) return editRange

  const wordInfo = model.getWordUntilPosition(position)
  return new monaco.Range(
    position.lineNumber,
    wordInfo?.startColumn || position.column,
    position.lineNumber,
    wordInfo?.endColumn || position.column
  )
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

  return {
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
    range: resolveLspCompletionRange(item, model, position),
    commitCharacters: Array.isArray(item.commitCharacters)
      ? item.commitCharacters.map((value) => String(value)).filter(Boolean)
      : undefined,
    preselect: !!item.preselect,
  }
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
  if (!uri || !entry.client) return

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
      return
    }

    if (previous.content === content && previous.languageId === normalizedLanguageId) return

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
  } catch (error) {
    logger.warn('[LSP] direct text document sync failed before completion', {
      languageId: normalizedLanguageId,
      clientKey,
      documentUri: uri,
      error: String((error as Error)?.message || error),
    })
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
  if (!entry?.client || entry.disposed || getLspConnectionState(normalizedLanguageId).state !== 'connected') {
    return []
  }

  const params = {
    textDocument: {
      uri: String(model.uri?.toString?.() || entry.documentUri || ''),
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
    await synchronizeDirectLspTextDocument(entry, clientKey, normalizedLanguageId, model)

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
    return []
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
  const relatedLanguages = getRelatedLspLanguages(cur.languageId || languageId)
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
  const relatedLanguages = getRelatedLspLanguages(languageId)
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
    getRelatedLspLanguages(normalizedLanguageId).forEach((lang) => {
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
        getRelatedLspLanguages(normalizedLanguageId).forEach((lang) => {
          ensureDirectCompletionProviderRegistered(lang, 'reuse-connected-client')
        })
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
      getRelatedLspLanguages(normalizedLanguageId).forEach((lang) => {
        ensureDirectCompletionProviderRegistered(lang, 'reuse-existing-entry')
      })
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
  getRelatedLspLanguages(normalizedLanguageId).forEach((lang) => {
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
        getRelatedLspLanguages(normalizedLanguageId).forEach((lang) => {
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
