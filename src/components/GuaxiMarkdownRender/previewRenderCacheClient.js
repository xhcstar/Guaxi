// 渲染缓存的持久层访问（**仅主线程**）。
//
// Worker 里没有 Tauri 的 invoke，所以持久层只在这一侧接；Worker 侧由主线程把预热好的
// 条目随渲染请求传进去（见 GuaxiEditor 的 worker 请求），再把新算出来的回传回来一起写。
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../../utils/logger'
import {
  MATH_ENGINE_VERSION,
  collectInlineMathCandidates,
  drainMathBackfill,
  primeMathMemo,
} from './mathRenderMemo'

const logger = createLogger('PreviewRenderCache')

/** 连续失败几次就不再尝试，避免后端命令不可用时每次渲染都白跑一次 IPC。 */
const MAX_CONSECUTIVE_FAILURES = 3
let consecutiveFailures = 0
let disabled = false

const noteFailure = (where, error) => {
  consecutiveFailures += 1
  logger.warn(`[PreviewRenderCache] ${where} 失败`, { error: String(error?.message || error) })
  if (consecutiveFailures >= MAX_CONSECUTIVE_FAILURES && !disabled) {
    disabled = true
    logger.warn('[PreviewRenderCache] 连续失败，本次会话停用持久缓存（渲染不受影响）')
  }
}

/**
 * 批量查公式缓存，返回原始 `{payload: svg}`（未命中不出现）。
 * 主线程要用它把结果**转交给 Worker**（Worker 里没有 invoke，拿不到持久缓存）。
 * @returns {Promise<Object|null>}
 */
export const fetchMathCache = async (payloads) => {
  if (disabled || !payloads?.length) return null
  try {
    const hits = await invoke('preview_render_cache_get', {
      kind: 'math',
      engine: MATH_ENGINE_VERSION,
      payloads,
    })
    consecutiveFailures = 0
    return hits && typeof hits === 'object' ? hits : null
  } catch (error) {
    noteFailure('查询公式缓存', error)
    return null
  }
}

/**
 * 批量查公式缓存并灌进本侧记忆层。
 * @returns {Promise<number>} 命中条数
 */
export const preloadMathCache = async (payloads) => {
  const hits = await fetchMathCache(payloads)
  if (!hits) return 0
  const primed = primeMathMemo(hits)
  if (primed) logger.debug('[PreviewRenderCache] 公式缓存命中', { requested: payloads.length, primed })
  return primed
}

/** 从 Markdown 源码粗略挑出候选公式，查一次后端并预热。用于 Worker 路径（主线程不 tokenize）。 */
export const preloadMathCacheFromMarkdown = async (content) => {
  const payloads = collectInlineMathCandidates(content)
  if (!payloads.length) return 0
  return preloadMathCache(payloads)
}

// ───────── 回填 ─────────

const BACKFILL_DEBOUNCE_MS = 1500
const BACKFILL_MAX_BATCH = 400
const pending = new Map()
let flushTimer = null

const flush = async () => {
  flushTimer = null
  if (disabled || !pending.size) return
  const entries = [...pending.entries()].slice(0, BACKFILL_MAX_BATCH).map(([payload, value]) => ({ payload, value }))
  for (const entry of entries) pending.delete(entry.payload)
  try {
    await invoke('preview_render_cache_put', {
      kind: 'math',
      engine: MATH_ENGINE_VERSION,
      entries,
    })
    consecutiveFailures = 0
  } catch (error) {
    noteFailure('回填公式缓存', error)
  }
  // 还剩就继续排下一批。
  if (pending.size && !flushTimer) flushTimer = setTimeout(flush, BACKFILL_DEBOUNCE_MS)
}

const enqueue = (pairs) => {
  for (const [payload, value] of pairs) pending.set(payload, value)
  if (!pending.size || flushTimer) return
  flushTimer = setTimeout(flush, BACKFILL_DEBOUNCE_MS)
}

/**
 * 把记忆层里「本次新算出来」的条目排进回填队列（防抖 + 分批）。
 * 渲染链路只调这个，不等磁盘、不等 IPC。
 */
export const scheduleMathBackfill = () => {
  if (disabled) return
  enqueue(drainMathBackfill())
}

/**
 * 把 Worker 报回来的新条目排进回填队列。
 * Worker 里没有 invoke，所以它只负责算、由主线程统一写。
 * @param {Array<[string, string]>} pairs `[payload, svg]`
 */
export const queueMathEntries = (pairs) => {
  if (disabled || !Array.isArray(pairs) || !pairs.length) return
  enqueue(pairs)
}
