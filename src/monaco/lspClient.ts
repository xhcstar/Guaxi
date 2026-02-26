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
type LspClientEntry = {
  client: MonacoLanguageClient | null
  webSocket: WebSocket
  disposed: boolean
  manualDisposing: boolean
  reader: WebSocketMessageReader | null
  writer: WebSocketMessageWriter | null
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
const lspFailureTracker = new Map<string, { count: number; firstAt: number }>()

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

const lspConnectionState = new Map<string, LspConnectionState>()
const lspConnectionListeners = new Set<(payload: LspConnectionStatePayload) => void>()

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
  const key = String(languageId || '').trim().toLowerCase()
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

const safeDisposeTransport = (value: unknown) => {
  try {
    const candidate = value as { dispose?: () => void } | null | undefined
    candidate?.dispose?.()
  } catch {
  }
}

const stopClientSafely = (client: MonacoLanguageClient | null) => {
  if (!client) return Promise.resolve()
  try {
    const result = client.stop()
    if (result && typeof (result as Promise<void>).then === 'function') {
      return (result as Promise<void>).catch(() => { })
    }
  } catch {
  }
  return Promise.resolve()
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
  if (!lspDisabled.has(languageId)) {
    lspDisabled.add(languageId)
  }
  if (!lspDisabledReason.has(languageId)) {
    lspDisabledReason.set(languageId, reason)
    logger.warn('[LSP] disabled', { languageId, reason, error })
  }
  setLspConnectionState(languageId, 'disabled', reason)
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
  const lang = String(languageId || '').trim().toLowerCase()
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

  const supported = [
    'cpp',
    'c',
    'rust',
    'python',
    'shell',
    'lua',
    'java',
    'javascript',
    'typescript',
    'html',
    'markdown',
    'latex',
  ]

  if (!supported.includes(languageId)) {
    return
  }

  const normalizedFilePath = String(filePath || '').trim()
  if (!isLikelyFilePath(normalizedFilePath)) {
    setLspConnectionState(languageId, 'disconnected', 'invalid-file-path')
    logger.warn('[LSP] skipped invalid file path', { languageId, filePath: normalizedFilePath })
    lspRetryBlockedUntil.set(languageId, Date.now() + LSP_RETRY_BACKOFF_MS)
    return
  }

  if (lspDisabled.has(languageId)) {
    return
  }
  if (lspClients.has(languageId)) {
    return
  }
  const blockedUntil = lspRetryBlockedUntil.get(languageId) || 0
  if (Date.now() < blockedUntil) {
    return
  }

  const protocol =
    window.location.protocol === 'https:' ? 'wss' : 'ws'
  const host = '127.0.0.1:8888'
  const url = `${protocol}://${host}/lsp/${languageId}`

  const webSocket = new WebSocket(url)
  const entry: LspClientEntry = {
    client: null,
    webSocket,
    disposed: false,
    manualDisposing: false,
    reader: null,
    writer: null,
  }
  lspClients.set(languageId, entry)
  setLspConnectionState(languageId, 'connecting')

  const cleanup = (reason: string) => {
    const cur = lspClients.get(languageId)
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

    void stopClientSafely(cur.client).finally(() => {
      closeSocketSafely(cur.webSocket)
    })

    lspClients.delete(languageId)
    if (lspDisabled.has(languageId)) {
      const disabledReason = lspDisabledReason.get(languageId) || reason
      setLspConnectionState(languageId, 'disabled', disabledReason)
    } else {
      setLspConnectionState(languageId, 'disconnected', reason)
    }
    if (reason !== 'manual-dispose') {
      lspRetryBlockedUntil.set(languageId, Date.now() + LSP_RETRY_BACKOFF_MS)
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
        name: `${languageId} Language Client`,
        clientOptions: {
          documentSelector: [{ language: languageId }],
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
        disableLsp(languageId, 'vscode-api-not-ready', error)
      } else {
        recordLspFailure(languageId, 'client-init-failed', error)
        logger.warn('[LSP] client init failed', { languageId, error })
      }
      cleanup('client-init-failed')
      return
    }

    const cur = lspClients.get(languageId)
    if (!cur || cur.disposed) {
      try {
        webSocket.close()
      } catch {
      }
      return
    }
    cur.client = client
    setLspConnectionState(languageId, 'connected')
    lspFailureTracker.delete(languageId)
    lspRetryBlockedUntil.delete(languageId)

    const startPromise = client.start()
    if (startPromise && typeof (startPromise as Promise<void>).catch === 'function') {
      ; (startPromise as Promise<void>).catch((error) => {
        if (shouldDisableLsp(error)) {
          disableLsp(languageId, 'vscode-api-not-ready', error)
        } else {
          recordLspFailure(languageId, 'start-failed', error)
          logger.warn('[LSP] client start failed', { languageId, error })
        }
        cleanup('start-failed')
      })
    }
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
  lspClients.forEach((entry, languageId) => {
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
    void stopClientSafely(entry.client).finally(() => {
      closeSocketSafely(entry.webSocket)
    })
    lspClients.delete(languageId)
    setLspConnectionState(languageId, 'disconnected', 'manual-dispose')
  })
}
