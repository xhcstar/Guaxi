import * as monaco from 'monaco-editor'
import { MonacoLanguageClient } from 'monaco-languageclient'
import type { MessageTransports } from 'vscode-languageclient/browser.js'
import {
  toSocket,
  WebSocketMessageReader,
  WebSocketMessageWriter,
} from 'vscode-ws-jsonrpc'
import { MonacoVscodeApiWrapper } from 'monaco-languageclient/vscodeApiWrapper'
import type { MonacoVscodeApiConfig } from 'monaco-languageclient/vscode'

let currentClient: MonacoLanguageClient | null = null
let apiWrapper: MonacoVscodeApiWrapper | null = null
let apiStarted: Promise<void> | null = null

function ensureVscodeApiStarted() {
  if (!apiWrapper) {
    const config: MonacoVscodeApiConfig = {
      $type: 'classic',
      viewsConfig: { $type: 'EditorService' },
      logLevel: 0,
    }
    apiWrapper = new MonacoVscodeApiWrapper(config)
  }
  if (!apiStarted) {
    apiStarted = apiWrapper.start({ caller: 'GuaxiLsp' })
  }
  return apiStarted
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

  const protocol =
    window.location.protocol === 'https:' ? 'wss' : 'ws'
  const host = '127.0.0.1:8888'
  const url = `${protocol}://${host}/lsp/${languageId}`

  ensureVscodeApiStarted().then(() => {
    const webSocket = new WebSocket(url)

    webSocket.onopen = () => {
      const socket = toSocket(webSocket)
      const reader = new WebSocketMessageReader(socket)
      const writer = new WebSocketMessageWriter(socket)

      const messageTransports: MessageTransports = { reader, writer }

      const client = new MonacoLanguageClient({
        name: `${languageId} Language Client`,
        clientOptions: {
          documentSelector: [languageId],
          initializationOptions: {
            workspaceFolders: [],
            filePath,
          },
        },
        messageTransports,
      })

      currentClient = client
      client.start()
    }

    webSocket.onclose = () => {
      if (currentClient) {
        currentClient.stop()
        currentClient = null
      }
    }
  })
}