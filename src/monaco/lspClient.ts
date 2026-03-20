import * as monaco from 'monaco-editor'
import { MonacoLanguageClient } from 'monaco-languageclient'
import { CloseAction, ErrorAction } from 'vscode-languageclient'
import type { MessageTransports } from 'vscode-languageclient/browser.js'
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

const normalizeLanguageId = (languageId: string) =>
  String(languageId || '').trim().toLowerCase()

const getLspClientKey = (languageId: string) =>
  String(getLspServerForLanguage(languageId) || normalizeLanguageId(languageId)).trim().toLowerCase()

const getRelatedLspLanguages = (languageId: string) => {
  const selectors = getLspDocumentSelectorForLanguage(languageId)
    .map((item) => normalizeLanguageId(item?.language || ''))
    .filter(Boolean)
  return selectors.length ? selectors : [normalizeLanguageId(languageId)].filter(Boolean)
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
  filePath: string
) {
  if (!editor || !languageId) return

  if (!isLspSupportedLanguage(languageId)) {
    return
  }

  const normalizedLanguageId = normalizeLanguageId(languageId)
  const clientKey = getLspClientKey(normalizedLanguageId)
  const documentSelector = getLspDocumentSelectorForLanguage(normalizedLanguageId)
  const startupLanguageId = normalizeLanguageId(documentSelector[0]?.language || normalizedLanguageId)

  const normalizedFilePath = String(filePath || '').trim()
  if (!isLikelyFilePath(normalizedFilePath)) {
    getRelatedLspLanguages(normalizedLanguageId).forEach((lang) => {
      setLspConnectionState(lang, 'disconnected', 'invalid-file-path')
    })
    logger.warn('[LSP] skipped invalid file path', { languageId: normalizedLanguageId, filePath: normalizedFilePath })
    lspRetryBlockedUntil.set(clientKey, Date.now() + LSP_RETRY_BACKOFF_MS)
    return
  }

  if (lspDisabled.has(normalizedLanguageId)) {
    return
  }
  if (lspClients.has(clientKey)) {
    return
  }
  const blockedUntil = lspRetryBlockedUntil.get(clientKey) || 0
  if (Date.now() < blockedUntil) {
    return
  }

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
  }
  lspClients.set(clientKey, entry)
  getRelatedLspLanguages(normalizedLanguageId).forEach((lang) => {
    setLspConnectionState(lang, 'connecting')
  })

  const cleanup = (reason: string) => {
    const cur = lspClients.get(clientKey)
    if (!cur || cur.disposed) return
    if (reason === 'manual-dispose') {
      cur.manualDisposing = true
    }
    cur.disposed = true

    // Prevent close/error callbacks from re-entering cleanup during manual shutdown.
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
    const relatedLanguages = getRelatedLspLanguages(normalizedLanguageId)
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
    if (reason !== 'manual-dispose') {
      lspRetryBlockedUntil.set(clientKey, Date.now() + LSP_RETRY_BACKOFF_MS)
    }
  }

  webSocket.onopen = () => {
    const socket = toSocket(webSocket)
    const reader = new WebSocketMessageReader(socket)
    const writer = new WebSocketMessageWriter(socket)
    entry.reader = reader
    entry.writer = writer

    const messageTransports: MessageTransports = { reader, writer }

    let client: MonacoLanguageClient
    try {
        client = new MonacoLanguageClient({
          name: `${clientKey} Language Client`,
          clientOptions: {
            documentSelector,
            outputChannel: LSP_NOOP_OUTPUT_CHANNEL as never,
            traceOutputChannel: LSP_NOOP_OUTPUT_CHANNEL as never,
            initializationOptions: {
              workspaceFolders: [],
              filePath,
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
        getRelatedLspLanguages(normalizedLanguageId).forEach((lang) => {
          setLspConnectionState(lang, 'connected')
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

  webSocket.onclose = () => {
    cleanup('socket-close')
  }

  webSocket.onerror = () => {
    recordLspFailure(languageId, 'socket-error')
    cleanup('socket-error')
  }
}

export function disposeAllLspClients() {
  lspClients.forEach((entry, clientKey) => {
    if (entry.disposed) return
    entry.manualDisposing = true
    // Reuse the same shutdown path to keep ordering consistent.
    safeDisposeTransport(entry.reader)
    entry.reader = null
    safeDisposeTransport(entry.writer)
    entry.writer = null

    entry.webSocket.onopen = null
    entry.webSocket.onclose = null
    entry.webSocket.onerror = null

    entry.disposed = true
    const client = entry.client
    entry.client = null
    void stopClientSafely(client).finally(() => {
      closeSocketSafely(entry.webSocket)
    })
    entry.resolveReady(false)
    lspClients.delete(clientKey)
  })
  lspConnectionState.forEach((_value, languageId) => {
    setLspConnectionState(languageId, 'disconnected', 'manual-dispose')
  })
}
