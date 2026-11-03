import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../utils/logger'

const logger = createLogger('ActivityBeacon')

/**
 * 活动总线的前端采集器。
 *
 * 记的是「人做过什么」：打开哪本书、读了多深、摘了什么、搜了什么、保存了哪个文件。
 * 它和 `episodes`（系统做过什么）是两张表，写入代价也必须低到可以随手调用：
 * - 进内存队列，约 1.2 秒或攒满 24 条才提交一次，单次 IPC 带一批；
 * - 窗口切到后台/关闭时立刻 flush，尽量不丢最后一段；
 * - 任何异常只写 logger.debug —— 记录行为绝不能影响被记录的操作。
 *
 * 读取侧的展示在设置页「认知层 → 最近活动」，分析用途（会话化、记忆沉淀、
 * 学习状态）留给后续里程碑。
 */

const FLUSH_DELAY_MS = 1200
const MAX_BATCH = 24
const MAX_QUEUE = 200
/** 同一目标（同一本书/同一文件）的进度事件最小间隔。 */
const PROGRESS_INTERVAL_MS = 45_000
/** 同一目标的打开/保存事件最小间隔（防止重复触发与自动保存刷屏）。 */
const REPEAT_INTERVAL_MS = 60_000

const queue = []
/** key → 上次提交时间，控制 trackActivityOnce 的节流。 */
const lastTrackedAt = new Map()
let timer = null

const sessionId = (() => {
  try {
    return globalThis.crypto?.randomUUID?.() || `session-${Date.now().toString(36)}`
  } catch {
    return `session-${Date.now().toString(36)}`
  }
})()

const hasTauri = () => {
  try {
    return Boolean(globalThis.__TAURI_INTERNALS__)
  } catch {
    return false
  }
}

const pruneThrottle = () => {
  if (lastTrackedAt.size <= 2000) return
  const cutoff = Date.now() - 24 * 3600 * 1000
  for (const [key, at] of lastTrackedAt) {
    if (at < cutoff) lastTrackedAt.delete(key)
  }
}

/** 立即把队列里的事件提交掉。 */
export function flushActivity() {
  if (timer) {
    clearTimeout(timer)
    timer = null
  }
  if (!queue.length) return
  const events = queue.splice(0, queue.length)
  if (!hasTauri()) return
  invoke('cognition_activity_log', { events }).catch((error) => {
    // 写不进去就算了：这是背景轨迹，不值得打断用户，也不值得重试把队列撑大。
    logger.debug('activity log failed', {
      error: String(error?.message || error),
      count: events.length,
    })
  })
}

const schedule = () => {
  if (timer) return
  timer = setTimeout(() => {
    timer = null
    flushActivity()
  }, FLUSH_DELAY_MS)
}

/**
 * 记录一条活动。
 * @param {{ app: string, kind: string, targetKind?: string, targetId?: string,
 *   kbName?: string, path?: string, line?: number, durationMs?: number,
 *   progress?: number, meta?: object, at?: string }} event
 */
export function trackActivity(event) {
  if (!event || !event.app || !event.kind) return
  if (queue.length >= MAX_QUEUE) queue.shift()
  queue.push({ ...event, sessionId: event.sessionId || sessionId })
  if (queue.length >= MAX_BATCH) flushActivity()
  else schedule()
}

/**
 * 节流版：同一 key 在 minIntervalMs 内只记一次。
 * 用于阅读进度、自动保存这类天然高频的事件。
 */
export function trackActivityOnce(key, event, minIntervalMs = REPEAT_INTERVAL_MS) {
  if (!key) {
    trackActivity(event)
    return
  }
  const now = Date.now()
  const last = lastTrackedAt.get(key) || 0
  if (now - last < minIntervalMs) return
  lastTrackedAt.set(key, now)
  pruneThrottle()
  trackActivity(event)
}

/**
 * 阅读进度：书/文档的「读到哪了」。
 * 阅读位置的持久化就在调用点上，这里只顺手把同一条事实送进活动总线。
 */
export function trackReaderProgress({
  filePath,
  library,
  fileName,
  progress,
  page,
  total,
  app = 'reader',
  extra = null,
} = {}) {
  const targetId = String(filePath || '').trim()
  if (!targetId) return
  const ratio = Number(progress)
  trackActivityOnce(
    `read:${targetId}`,
    {
      app,
      kind: 'read',
      targetKind: 'book',
      targetId,
      kbName: library ? String(library) : undefined,
      progress: Number.isFinite(ratio) ? Math.min(1, Math.max(0, ratio)) : undefined,
      meta: {
        fileName: fileName || undefined,
        page: Number.isFinite(Number(page)) ? Number(page) : undefined,
        total: Number.isFinite(Number(total)) ? Number(total) : undefined,
        ...(extra && typeof extra === 'object' ? extra : {}),
      },
    },
    PROGRESS_INTERVAL_MS,
  )
}

// 后台/关闭前把最后一段发出去。pagehide 里 IPC 不保证送达，但比什么都不做强。
if (typeof document !== 'undefined') {
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') flushActivity()
  })
}
if (typeof window !== 'undefined') {
  window.addEventListener('pagehide', flushActivity)
}

/** 组合式用法；纯副作用模块，没有响应式状态。 */
export function useActivityBeacon() {
  return { track: trackActivity, trackOnce: trackActivityOnce, trackReaderProgress, flush: flushActivity }
}
