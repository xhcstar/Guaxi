import { createLogger } from '../utils/logger'
import EditorWorker from '@codingame/monaco-vscode-api/workers/editor.worker?worker'
import ExtensionHostWorker from '@codingame/monaco-vscode-api/workers/extensionHost.worker?worker'
import TextMateWorker from '@codingame/monaco-vscode-textmate-service-override/worker?worker'
import JsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker'
import CssWorker from 'monaco-editor/esm/vs/language/css/css.worker?worker'
import HtmlWorker from 'monaco-editor/esm/vs/language/html/html.worker?worker'
import TypeScriptWorker from 'monaco-editor/esm/vs/language/typescript/ts.worker?worker'

const logger = createLogger('monacoWorkers')
const MONACO_WORKER_BOOTSTRAP_FLAG = '__GUAXI_MONACO_WORKERS_CONFIGURED__'

const normalizeWorkerLabel = (label) => {
  const normalized = String(label || '').trim()
  if (!normalized) return 'editorWorkerService'

  switch (normalized) {
    case 'javascript':
      return 'typescript'
    case 'scss':
    case 'less':
      return 'css'
    case 'handlebars':
    case 'razor':
      return 'html'
    case 'TextEditorWorker':
      return 'editorWorkerService'
    case 'ExtensionHostWorker':
      return 'extensionHostWorker'
    default:
      return normalized
  }
}

const attachWorkerDiagnostics = (worker, requestedLabel, resolvedLabel) => {
  if (!worker || typeof worker.addEventListener !== 'function') return worker

  worker.addEventListener('error', (event) => {
    logger.error('[MonacoWorkers] worker runtime error', {
      requestedLabel,
      resolvedLabel,
      type: String(event?.type || 'error'),
      message: String(event?.message || ''),
      filename: String(event?.filename || ''),
      lineno: Number(event?.lineno || 0),
      colno: Number(event?.colno || 0),
    })
  })

  worker.addEventListener('messageerror', (event) => {
    logger.error('[MonacoWorkers] worker messageerror', {
      requestedLabel,
      resolvedLabel,
      type: String(event?.type || 'messageerror'),
    })
  })

  return worker
}

const createWorkerByLabel = (requestedLabel) => {
  const resolvedLabel = normalizeWorkerLabel(requestedLabel)

  // 设计说明：
  // 1. 这里不再依赖 `vite-plugin-monaco-editor` 往 HTML 注入 `getWorkerUrl`，而是直接使用
  //    Vite 原生 `?worker` 入口生成模块 Worker。
  // 2. 这样 Worker 地址会交给 Vite 自己托管，避免在 Tauri/Vite 运行时里因为路径改写失败，
  //    最终请求落到 `index.html`，被浏览器报成 `text/html` MIME 错误。
  // 3. `@codingame/monaco-vscode-api` 自己的 editor worker、TextMate worker、extension host worker
  //    也统一走这里，保证 Monaco 与 VSCode service override 共用同一套解析规则。
  switch (resolvedLabel) {
    case 'json':
      return attachWorkerDiagnostics(new JsonWorker({ name: requestedLabel || resolvedLabel }), requestedLabel, resolvedLabel)
    case 'css':
      return attachWorkerDiagnostics(new CssWorker({ name: requestedLabel || resolvedLabel }), requestedLabel, resolvedLabel)
    case 'html':
      return attachWorkerDiagnostics(new HtmlWorker({ name: requestedLabel || resolvedLabel }), requestedLabel, resolvedLabel)
    case 'typescript':
      return attachWorkerDiagnostics(new TypeScriptWorker({ name: requestedLabel || resolvedLabel }), requestedLabel, resolvedLabel)
    case 'TextMateWorker':
      return attachWorkerDiagnostics(new TextMateWorker({ name: requestedLabel || resolvedLabel }), requestedLabel, resolvedLabel)
    case 'extensionHostWorker':
      return attachWorkerDiagnostics(new ExtensionHostWorker({ name: requestedLabel || resolvedLabel }), requestedLabel, resolvedLabel)
    case 'editorWorkerService':
    default:
      return attachWorkerDiagnostics(new EditorWorker({ name: requestedLabel || resolvedLabel }), requestedLabel, resolvedLabel)
  }
}

export function configureMonacoWorkers() {
  const globalObject = globalThis
  if (globalObject[MONACO_WORKER_BOOTSTRAP_FLAG]) {
    logger.debug('[MonacoWorkers] reuse existing MonacoEnvironment worker bootstrap')
    return globalObject.MonacoEnvironment
  }

  const previousEnvironment = globalObject.MonacoEnvironment || {}

  globalObject.MonacoEnvironment = {
    ...previousEnvironment,
    getWorker(workerId, label) {
      const requestedLabel = String(label || '').trim() || 'editorWorkerService'
      const worker = createWorkerByLabel(requestedLabel)
      logger.info('[MonacoWorkers] worker created', {
        workerId: String(workerId || ''),
        requestedLabel,
        resolvedLabel: normalizeWorkerLabel(requestedLabel),
      })
      return worker
    },
  }

  globalObject[MONACO_WORKER_BOOTSTRAP_FLAG] = true

  logger.info('[MonacoWorkers] MonacoEnvironment configured', {
    hadPreviousEnvironment: !!previousEnvironment,
    hasPreviousGetWorker: typeof previousEnvironment.getWorker === 'function',
    hasPreviousGetWorkerUrl: typeof previousEnvironment.getWorkerUrl === 'function',
  })

  return globalObject.MonacoEnvironment
}
