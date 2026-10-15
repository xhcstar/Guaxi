import { listen } from '@tauri-apps/api/event'
import { createLogger } from './logger'

const logger = createLogger('readaloudEvents')

/** 后端合成好（或失败）一段朗读音频时广播。payload: { sessionId, index, ok, ready?, error? } */
export const READALOUD_SEGMENT_EVENT = 'readaloud_segment'

/**
 * 订阅朗读段就绪事件。
 *
 * 只透传属于本会话的事件：一个窗口先后开过多个阅读器是常态，
 * 旧会话迟到的段不该影响新会话的播放（后端合成是异步的，切文档时会有在途事件）。
 *
 * @param {string} sessionId 会话 id；空串表示不过滤
 * @param {(evt: { index: number, ok: boolean, path: string, durationMs: number, pauseAfterMs: number, error: string }) => void} handler
 * @returns {Promise<() => void>} unlisten
 */
export async function onReadaloudSegment(sessionId, handler) {
  const unlisten = await listen(READALOUD_SEGMENT_EVENT, (event) => {
    const payload = event?.payload || {}
    if (sessionId && String(payload.sessionId || '') !== String(sessionId)) return
    const ready = payload.ready || {}
    try {
      handler({
        index: Number(payload.index ?? -1),
        ok: Boolean(payload.ok),
        path: String(ready.path || ''),
        durationMs: Number(ready.durationMs || 0),
        pauseAfterMs: Number(ready.pauseAfterMs || 0),
        error: String(payload.error || ''),
      })
    } catch (e) {
      logger.warn('readaloud_segment 处理失败', e)
    }
  })

  return () => {
    try {
      unlisten()
    } catch {
      /* 已卸载 */
    }
  }
}
