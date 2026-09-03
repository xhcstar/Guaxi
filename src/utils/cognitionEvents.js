import { listen } from '@tauri-apps/api/event'
import { createLogger } from './logger'

const logger = createLogger('cognitionEvents')

/** 后端投递一条提醒时广播。payload: { id, kind, channel, push } */
export const COGNITION_NUDGE_EVENT = 'cognition_nudge'
/** 简报生成完成。payload: { day, kind, path } */
export const COGNITION_BRIEF_EVENT = 'cognition_brief'
/** 对话接地完成，带本轮引证。payload: { episodeId, citations, channels, elapsedMs, mode? } */
export const COGNITION_GROUNDED_EVENT = 'cognition_grounded'

/**
 * 订阅提醒投递。
 *
 * 设计说明：一次审议可能连发多条（看板类的居多），这里做一次合并防抖，
 * 让看板只重拉一次；但 push=true（系统通知/打断卡片）必须立刻透传，
 * 否则用户会觉得提醒"迟到"。
 *
 * @param {(evt: { id: string, kind: string, channel: string, push: boolean }) => void} handler
 * @param {{ debounceMs?: number }} [options]
 * @returns {Promise<() => void>} unlisten
 */
export async function onCognitionNudge(handler, { debounceMs = 600 } = {}) {
  let timer = null
  let disposed = false
  let lastQuiet = null

  const unlisten = await listen(COGNITION_NUDGE_EVENT, (event) => {
    if (disposed) return
    const p = event?.payload || {}
    const evt = {
      id: String(p.id || ''),
      kind: String(p.kind || ''),
      channel: String(p.channel || 'board'),
      push: Boolean(p.push),
    }
    if (evt.push) {
      try {
        handler(evt)
      } catch (e) {
        logger.warn('cognition_nudge handler failed', e)
      }
      return
    }
    lastQuiet = evt
    if (timer) clearTimeout(timer)
    timer = setTimeout(() => {
      timer = null
      const pending = lastQuiet
      lastQuiet = null
      if (!pending) return
      try {
        handler(pending)
      } catch (e) {
        logger.warn('cognition_nudge handler failed', e)
      }
    }, debounceMs)
  })

  return () => {
    disposed = true
    if (timer) clearTimeout(timer)
    try {
      unlisten()
    } catch (e) {
      /* noop */
    }
  }
}

/**
 * 订阅简报生成。
 * @param {(evt: { day: string, kind: string, path: string }) => void} handler
 */
export async function onCognitionBrief(handler) {
  const unlisten = await listen(COGNITION_BRIEF_EVENT, (event) => {
    const p = event?.payload || {}
    try {
      handler({ day: String(p.day || ''), kind: String(p.kind || ''), path: String(p.path || '') })
    } catch (e) {
      logger.warn('cognition_brief handler failed', e)
    }
  })
  return () => {
    try {
      unlisten()
    } catch (e) {
      /* noop */
    }
  }
}

/**
 * 订阅对话接地事件（用于在回答上方渲染引证条）。
 * @param {(evt: { episodeId: string, citations: any[], channels: object, elapsedMs: number, mode: string }) => void} handler
 */
export async function onCognitionGrounded(handler) {
  const unlisten = await listen(COGNITION_GROUNDED_EVENT, (event) => {
    const p = event?.payload || {}
    try {
      handler({
        episodeId: String(p.episodeId || ''),
        citations: Array.isArray(p.citations) ? p.citations : [],
        channels: p.channels || {},
        elapsedMs: Number(p.elapsedMs || 0),
        mode: String(p.mode || 'chat'),
      })
    } catch (e) {
      logger.warn('cognition_grounded handler failed', e)
    }
  })
  return () => {
    try {
      unlisten()
    } catch (e) {
      /* noop */
    }
  }
}
