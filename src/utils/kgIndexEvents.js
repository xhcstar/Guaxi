import { listen } from '@tauri-apps/api/event'
import { createLogger } from './logger'

const logger = createLogger('kgIndexEvents')

// 后端 kgraph 在 mark_kb_dirty / drop_kb_index 时广播的事件名。
// payload: { kb_name: string, reason: 'dirty' | 'removed' }
export const KG_INDEX_CHANGED_EVENT = 'kg_index_changed'

/**
 * 订阅知识图谱索引变更。
 *
 * 设计说明:文件保存(尤其自动保存)会连发多次 dirty,这里按知识库名各自防抖合并,
 * 保证同一知识库的连续变更只回调一次;'removed' 需要立刻生效,不参与合并。
 *
 * @param {(evt: { kbName: string, reason: string }) => void} handler
 * @param {{ debounceMs?: number }} [options]
 * @returns {Promise<() => void>} unlisten,同时清掉挂起的防抖定时器
 */
export async function onKgIndexChanged(handler, { debounceMs = 900 } = {}) {
  const timers = new Map()
  let disposed = false

  const clearTimers = () => {
    for (const t of timers.values()) clearTimeout(t)
    timers.clear()
  }

  const unlisten = await listen(KG_INDEX_CHANGED_EVENT, (event) => {
    if (disposed) return
    const payload = event?.payload || {}
    const kbName = String(payload.kb_name || '')
    const reason = String(payload.reason || 'dirty')
    if (!kbName) return

    const fire = () => {
      timers.delete(kbName)
      try {
        handler({ kbName, reason })
      } catch (e) {
        logger.warn('kg_index_changed handler failed', e)
      }
    }

    const pending = timers.get(kbName)
    if (pending) clearTimeout(pending)
    if (reason === 'removed') {
      fire()
      return
    }
    timers.set(kbName, setTimeout(fire, debounceMs))
  })

  return () => {
    disposed = true
    clearTimers()
    try {
      unlisten()
    } catch (e) {
      /* noop */
    }
  }
}
