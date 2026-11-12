// 渲染缓存的「图片授权代次」。
//
// 后端 `/markdown/api/images/<绝对路径>` 的放行依据之一是内存里的图片登记表：渲染预处理
// 阶段每解析出一张图就登记一次。笔记放在图床/库/知识库三个根之外时，图片能不能显示**全靠
// 这张表**。而表会在两种情况下丢掉已有登记：条目达到上限整体清空、进程重启（进程内 static）。
//
// 前端有几层渲染缓存（编辑器预览的分段/全文缓存、Notebook 的 cell 缓存）把图片 URL 直接
// 存进了 HTML。表一清空，缓存仍然命中 —— 不会再走预处理、图片不会被重新登记 —— 结果是
// 一排 403，而且只有改一下文档或逐张点「重新加载」才会恢复。
//
// 办法：把代次并进渲染缓存的 key。代次由后端给（`render_cache_epoch` = 进程启动毫秒 +
// 登记表清空次数），这里只在内存里缓存它，并提供一个「取当前代次」的同步读取口。

import { invoke } from '@tauri-apps/api/core'
import { createLogger } from './logger'

const logger = createLogger('RenderCacheEpoch')

/** 已拿到的代次。空串表示还没问到过（此时调用方应退化成「不带代次」，行为与改造前一致）。 */
let currentEpoch = ''

/** 正在进行的查询，避免并发调用打出一串重复 IPC。 */
let inflight = null

/**
 * 主动刷新代次。渲染前后各调一次即可 —— 它很便宜（纯内存读取），但没必要每次渲染都问。
 * @returns {Promise<string>} 最新的代次串；查不到时返回上一次的值（可能为空）。
 */
export const refreshRenderCacheEpoch = async () => {
  if (inflight) return inflight
  inflight = (async () => {
    try {
      const value = await invoke('render_cache_epoch')
      currentEpoch = String(value || '')
      return currentEpoch
    } catch (error) {
      // 命令不可用（旧版后端/测试环境）不该影响渲染，静默降级。
      logger.warn('[RenderCacheEpoch] 查询失败，本次不影响渲染', {
        error: String(error?.message || error),
      })
      return currentEpoch
    } finally {
      inflight = null
    }
  })()
  return inflight
}

/**
 * 把代次拼成 key 的前缀段。代次为空时返回空串，保证 key 与改造前完全一致。
 */
export const renderCacheEpochPrefix = () => (currentEpoch ? `${currentEpoch}::` : '')
