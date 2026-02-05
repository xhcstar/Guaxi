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