import { invoke } from '@tauri-apps/api/core'

const DEFAULT_LEVEL = import.meta.env.DEV ? 'debug' : 'info'

const LEVEL_ORDER = {
  debug: 10,
  info: 20,
  warn: 30,
  error: 40
}

const state = {
  installed: false,
  scope: 'Frontend',
  level: DEFAULT_LEVEL,
  forwardConsole: true,
  forwardErrors: true,
  queue: [],
  flushing: false,
  flushTimer: null,
  maxBatchSize: 50,
  flushIntervalMs: 300,
  maxQueueSize: 2000,
  originalConsole: null
}

const MERMAID_ARCHITECTURE_WARNING_PREFIX =
  'Do not assign mappings to elements without corresponding data'
const MERMAID_ARCHITECTURE_WARNING_HINT =
  'property `label` with data field `label`'
const TAURI_IPC_FALLBACK_WARNING =
  'IPC custom protocol failed, Tauri will now use the postMessage interface instead'
const VUE_SLOT_OUTSIDE_RENDER_WARNING =
  'Slot "default" invoked outside of the render function'
const VUE_SLOT_WARNING_REQUIRED_STACK_MARKERS = ['<Popover', '<Tooltip']
const VUE_SLOT_WARNING_TARGET_STACK_MARKERS = ['<Binder', '<Target']
let mermaidArchitectureWarningSuppressCount = 0
let tauriIpcFallbackWarningSuppressCount = 0
let vueSlotOutsideRenderWarningSuppressCount = 0

function normalizeLevel(level) {
  const v = String(level || '').toLowerCase()
  if (v === 'log') return 'info'
  if (v === 'warning') return 'warn'
  if (v === 'fatal') return 'error'
  if (v === 'trace') return 'debug'
  if (v in LEVEL_ORDER) return v
  return 'info'
}

function shouldLog(level) {
  const cur = LEVEL_ORDER[normalizeLevel(state.level)] ?? LEVEL_ORDER.info
  const lv = LEVEL_ORDER[normalizeLevel(level)] ?? LEVEL_ORDER.info
  return lv >= cur
}

function stringifyArg(arg) {
  if (arg == null) return String(arg)
  if (typeof arg === 'string') return arg
  if (arg instanceof Error) {
    return JSON.stringify({
      name: arg.name,
      message: arg.message,
      stack: arg.stack
    })
  }

  const seen = new WeakSet()
  try {
    return JSON.stringify(arg, (k, v) => {
      if (v instanceof Error) {
        return { name: v.name, message: v.message, stack: v.stack }
      }
      if (typeof v === 'object' && v !== null) {
        if (seen.has(v)) return '[Circular]'
        seen.add(v)
      }
      return v
    })
  } catch {
    try {
      return String(arg)
    } catch {
      return '[Unserializable]'
    }
  }
}

function formatMessage(args) {
  if (!Array.isArray(args) || args.length === 0) return ''
  return args.map(stringifyArg).join(' ')
}

function isVueTooltipSlotWarning(message) {
  if (!message.includes('[Vue warn]')) return false
  if (!message.includes(VUE_SLOT_OUTSIDE_RENDER_WARNING)) return false

  // 设计思路：
  // 1. 只识别 Naive UI Tooltip/Popover 经由 vueuc Binder/Target 打出的开发态 slot 告警。
  // 2. 业务组件如果真的在 setup、watch、异步回调里错误调用 slot，组件栈通常不会同时包含这些标记，
  //    因此仍会按普通 warn 透传，避免把真实响应式依赖问题静默掉。
  // 3. 该过滤只影响 console.warn 转发和前端日志入队，不改变 Tooltip/Popover 的渲染、事件和状态。
  return (
    VUE_SLOT_WARNING_REQUIRED_STACK_MARKERS.every((marker) => message.includes(marker)) &&
    VUE_SLOT_WARNING_TARGET_STACK_MARKERS.some((marker) => message.includes(marker))
  )
}

function getKnownWarnSuppressionKind(args) {
  const message = formatMessage(args)
  if (!message) return ''

  // 设计思路：
  // 1. Tauri 在部分 WebView2 环境下会先尝试 ipc.localhost 自定义协议；
  //    如果该通道不可用，会自动降级到 postMessage，这条 warn 本身不代表业务命令失败。
  // 2. 前端日志接管 console.warn 后，如果继续转发这条 warn，会在用户点击 LSP 安装按钮时
  //    把一次正常降级误放大成“下载插件失败”的线索，因此这里将它作为已知运行时噪声过滤。
  // 3. 真正的 invoke 失败仍会进入各业务 catch，并由业务代码记录 error 日志。
  if (message.includes(TAURI_IPC_FALLBACK_WARNING)) {
    return 'tauri-ipc-fallback'
  }

  if (isVueTooltipSlotWarning(message)) {
    return 'vue-tooltip-slot'
  }

  if (
    message.includes(MERMAID_ARCHITECTURE_WARNING_PREFIX) &&
    message.includes(MERMAID_ARCHITECTURE_WARNING_HINT)
  ) {
    return 'mermaid-architecture'
  }

  return ''
}

function logKnownWarnSuppression(kind) {
  if (kind === 'tauri-ipc-fallback') {
    tauriIpcFallbackWarningSuppressCount += 1
    if (
      tauriIpcFallbackWarningSuppressCount === 1 ||
      tauriIpcFallbackWarningSuppressCount % 20 === 0
    ) {
      enqueue({
        level: 'debug',
        scope: state.scope,
        message: '[FrontendLogger] suppressed known Tauri IPC fallback warning',
        fields: {
          suppressCount: tauriIpcFallbackWarningSuppressCount
        }
      })
    }
    return
  }

  if (kind === 'vue-tooltip-slot') {
    vueSlotOutsideRenderWarningSuppressCount += 1
    if (
      vueSlotOutsideRenderWarningSuppressCount === 1 ||
      vueSlotOutsideRenderWarningSuppressCount % 20 === 0
    ) {
      enqueue({
        level: 'debug',
        scope: state.scope,
        message: '[FrontendLogger] suppressed known Vue tooltip slot warning',
        fields: {
          suppressCount: vueSlotOutsideRenderWarningSuppressCount
        }
      })
    }
    return
  }

  if (kind === 'mermaid-architecture') {
    mermaidArchitectureWarningSuppressCount += 1

    // 只过滤 Mermaid architecture 图内部由 cytoscape 打出的这条已知上游告警，
    // 其它 warn 仍然原样透传，避免误伤真正的业务问题。
    if (mermaidArchitectureWarningSuppressCount === 1 || mermaidArchitectureWarningSuppressCount % 20 === 0) {
      enqueue({
        level: 'debug',
        scope: state.scope,
        message: '[FrontendLogger] suppressed known Mermaid architecture warning',
        fields: {
          suppressCount: mermaidArchitectureWarningSuppressCount
        }
      })
    }
  }
}

function enqueue(entry) {
  if (!entry) return
  if (!shouldLog(entry.level)) return

  if (state.queue.length >= state.maxQueueSize) {
    state.queue.splice(0, state.queue.length - Math.floor(state.maxQueueSize / 2))
  }

  state.queue.push({
    level: normalizeLevel(entry.level),
    scope: String(entry.scope || state.scope || 'Frontend'),
    message: String(entry.message || ''),
    fields: entry.fields ?? null,
    ts: new Date().toISOString()
  })

  scheduleFlush()
}

function scheduleFlush() {
  if (state.flushTimer) return
  state.flushTimer = setTimeout(() => {
    state.flushTimer = null
    flush().catch(() => { })
  }, state.flushIntervalMs)
}

async function flush() {
  if (state.flushing) return
  if (state.queue.length === 0) return

  state.flushing = true
  try {
    while (state.queue.length > 0) {
      const batch = state.queue.splice(0, state.maxBatchSize)
      try {
        await invoke('frontend_log_batch', { entries: batch })
      } catch (e) {
        state.queue.unshift(...batch)
        if (state.originalConsole) {
          state.originalConsole.warn('[FrontendLogger] flush failed:', e)
        }
        break
      }
    }
  } finally {
    state.flushing = false
  }
}

function patchConsole() {
  if (!state.originalConsole) {
    state.originalConsole = {
      debug: console.debug?.bind(console),
      info: console.info?.bind(console),
      log: console.log?.bind(console),
      warn: console.warn?.bind(console),
      error: console.error?.bind(console)
    }
  }

  const orig = state.originalConsole

  console.debug = (...args) => {
    orig.debug?.(...args)
    enqueue({ level: 'debug', scope: state.scope, message: formatMessage(args) })
  }

  console.info = (...args) => {
    orig.info?.(...args)
    enqueue({ level: 'info', scope: state.scope, message: formatMessage(args) })
  }

  console.log = (...args) => {
    orig.log?.(...args)
    enqueue({ level: 'info', scope: state.scope, message: formatMessage(args) })
  }

  console.warn = (...args) => {
    const knownWarnKind = getKnownWarnSuppressionKind(args)
    if (knownWarnKind) {
      logKnownWarnSuppression(knownWarnKind)
      return
    }

    orig.warn?.(...args)
    enqueue({ level: 'warn', scope: state.scope, message: formatMessage(args) })
  }

  console.error = (...args) => {
    orig.error?.(...args)
    enqueue({ level: 'error', scope: state.scope, message: formatMessage(args) })
  }
}

function attachGlobalErrorHandlers() {
  window.addEventListener('error', (event) => {
    const err = event?.error
    enqueue({
      level: 'error',
      scope: state.scope,
      message: err ? stringifyArg(err) : String(event?.message || 'window.error'),
      fields: {
        filename: event?.filename,
        lineno: event?.lineno,
        colno: event?.colno
      }
    })
  })

  window.addEventListener('unhandledrejection', (event) => {
    const reason = event?.reason
    enqueue({
      level: 'error',
      scope: state.scope,
      message: reason instanceof Error ? stringifyArg(reason) : stringifyArg(reason ?? 'unhandledrejection')
    })
  })
}

export function initFrontendLogging(options = {}) {
  if (state.installed) return

  state.scope = String(options.scope || state.scope || 'Frontend')
  state.level = normalizeLevel(options.level || state.level || DEFAULT_LEVEL)
  state.forwardConsole = options.forwardConsole !== false
  state.forwardErrors = options.forwardErrors !== false

  state.installed = true

  if (state.forwardConsole) patchConsole()
  if (state.forwardErrors) attachGlobalErrorHandlers()

  enqueue({
    level: 'info',
    scope: state.scope,
    message: 'frontend logging initialized',
    fields: {
      level: state.level,
      forwardConsole: state.forwardConsole,
      forwardErrors: state.forwardErrors
    }
  })
}

export function createLogger(scope) {
  const scoped = String(scope || 'Frontend')

  return {
    debug: (...args) => console.debug(`[${scoped}]`, ...args),
    info: (...args) => console.info(`[${scoped}]`, ...args),
    warn: (...args) => console.warn(`[${scoped}]`, ...args),
    error: (...args) => console.error(`[${scoped}]`, ...args)
  }
}

export async function flushFrontendLogs() {
  await flush()
}
