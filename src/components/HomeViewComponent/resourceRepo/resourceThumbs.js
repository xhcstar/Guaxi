// 资源库缩略图的共享缓存。
//
// 网格视图一屏可能同时出现几十张图。每个瓦片自己去 invoke 会在打开目录的瞬间
// 同时发起几十个 IPC，而且滚动回去时会重复请求。这里做两件事：
//   1. 同一 (路径, 边长) 只请求一次，失败结果也缓存，避免坏图反复重试；
//   2. 并发上限固定，超出排队的按到达顺序执行。
// 缓存键里带 mtime，源文件被替换后自动失效 —— 由调用方传入 node.mtime。

import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { createLogger } from '../../../utils/logger'

const logger = createLogger('ResourceRepoThumbs')

const CONCURRENCY = 3
const cache = new Map() // key -> { url, width, height }
const failed = new Set() // key
const inflight = new Map() // key -> Promise
const queue = []
let running = 0

const keyOf = (path, mtime, maxEdge) => `${path}\u0000${mtime || 0}\u0000${maxEdge || 0}`

const pump = () => {
  while (running < CONCURRENCY && queue.length) {
    const job = queue.shift()
    running += 1
    job
      .run()
      .then((value) => job.resolve(value))
      .catch((error) => job.reject(error))
      .finally(() => {
        running -= 1
        pump()
      })
  }
}

const enqueue = (run) =>
  new Promise((resolve, reject) => {
    queue.push({ run, resolve, reject })
    pump()
  })

/** 同步读缓存；没有则返回 null。瓦片首次渲染就能拿到已下载过的图，不闪。 */
export const peekThumbnail = (relativePath, mtime = 0, maxEdge = 320) => {
  const hit = cache.get(keyOf(relativePath, mtime, maxEdge))
  return hit || null
}

/** 拉取缩略图，返回 { url, width, height }；失败抛错。 */
export const loadThumbnail = (relativePath, mtime = 0, maxEdge = 320) => {
  const key = keyOf(relativePath, mtime, maxEdge)
  const hit = cache.get(key)
  if (hit) return Promise.resolve(hit)
  if (failed.has(key)) return Promise.reject(new Error('缩略图不可用'))
  if (inflight.has(key)) return inflight.get(key)

  const task = enqueue(async () => {
    try {
      const res = await invoke('resource_repo_thumbnail', { relativePath, maxEdge })
      const value = {
        url: convertFileSrc(res.path),
        width: Number(res.width) || 0,
        height: Number(res.height) || 0,
      }
      cache.set(key, value)
      return value
    } catch (error) {
      failed.add(key)
      logger.warn('[Thumbs] thumbnail failed', {
        path: relativePath,
        error: String(error?.message || error),
      })
      throw error
    } finally {
      inflight.delete(key)
    }
  })
  inflight.set(key, task)
  return task
}

/** 删除/重命名后让旧缓存失效，避免显示已不存在的图。 */
export const dropThumbnails = (predicate) => {
  for (const key of [...cache.keys()]) {
    const path = key.split('\u0000')[0]
    if (predicate(path)) cache.delete(key)
  }
  for (const key of [...failed]) {
    const path = key.split('\u0000')[0]
    if (predicate(path)) failed.delete(key)
  }
}
