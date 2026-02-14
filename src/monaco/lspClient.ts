import * as monaco from 'monaco-editor'
import { MonacoLanguageClient } from 'monaco-languageclient'
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
}
const lspClients = new Map<string, LspClientEntry>()
const lspRetryBlockedUntil = new Map<string, number>()
const LSP_RETRY_BACKOFF_MS = 15000
const logger = createLogger('lspClient')
const lspDisabled = new Set<string>()
const lspDisabledReason = new Map<string, string>()
const LSP_VSCODE_API_NOT_READY = 'Default api is not ready yet'

const resolveErrorMessage = (error: unknown) => {
  if (error instanceof Error) return error.message
  return String(error || '')
}

const shouldDisableLsp = (error: unknown) => {
  const message = resolveErrorMessage(error)
  return message.includes(LSP_VSCODE_API_NOT_READY)
}

const disableLsp = (languageId: string, reason: string, error?: unknown) => {
  if (!lspDisabled.has(languageId)) {
    lspDisabled.add(languageId)
  }
  if (!lspDisabledReason.has(languageId)) {
    lspDisabledReason.set(languageId, reason)
    logger.warn('[LSP] disabled', { languageId, reason, error })
  }
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
    'html',
    'markdown',
    'latex',
  ]

  if (!supported.includes(languageId)) {
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
  }
  lspClients.set(languageId, entry)

  const cleanup = (reason: string) => {
    const cur = lspClients.get(languageId)
    if (!cur || cur.disposed) return
    cur.disposed = true

    if (cur.client) {
      try {
        const r = cur.client.stop()
        if (r && typeof (r as Promise<void>).catch === 'function') {
          ; (r as Promise<void>).catch(() => { })
        }
      } catch {
      }
    }

    try {
      if (cur.webSocket.readyState === WebSocket.OPEN || cur.webSocket.readyState === WebSocket.CONNECTING) {
        cur.webSocket.close()
      }
    } catch {
    }

    lspClients.delete(languageId)
    if (reason !== 'manual-dispose') {
      lspRetryBlockedUntil.set(languageId, Date.now() + LSP_RETRY_BACKOFF_MS)
    }
  }

  webSocket.onopen = () => {
    const socket = toSocket(webSocket)
    const reader = new WebSocketMessageReader(socket)
    const writer = new WebSocketMessageWriter(socket)

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
        },
        messageTransports,
      })
    } catch (error) {
      if (shouldDisableLsp(error)) {
        disableLsp(languageId, 'vscode-api-not-ready', error)
      } else {
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

    const startPromise = client.start()
    if (startPromise && typeof (startPromise as Promise<void>).catch === 'function') {
      ; (startPromise as Promise<void>).catch((error) => {
        if (shouldDisableLsp(error)) {
          disableLsp(languageId, 'vscode-api-not-ready', error)
        } else {
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
    cleanup('socket-error')
  }
}

export function disposeAllLspClients() {
  lspClients.forEach((entry, languageId) => {
    entry.disposed = true
    try {
      if (entry.client) {
        const r = entry.client.stop()
        if (r && typeof (r as Promise<void>).catch === 'function') {
          ; (r as Promise<void>).catch(() => { })
        }
      }
    } catch {
    }
    try {
      if (entry.webSocket.readyState === WebSocket.OPEN || entry.webSocket.readyState === WebSocket.CONNECTING) {
        entry.webSocket.close()
      }
    } catch {
    }
    lspClients.delete(languageId)
  })
}
