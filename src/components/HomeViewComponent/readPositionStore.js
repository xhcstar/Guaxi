// 阅读位置的落盘层。
//
// 为什么单独成模块
//
// 位置缓存原本只在组件内存里（Map），进程一退就没了——「重新打开软件记不住阅读位置」
// 就是这么来的。落盘本身不难，难的是不能把这条路径做成第二个容易写坏的地方：
//
// 1. 写盘必须防抖。滚动事件每帧都来，直接 setItem 会把主线程塞满。
// 2. 写盘必须可失败。隐私模式/配额满/脏数据都不能抛给调用方，最坏只是一次记不住。
// 3. 结构必须带版本号。将来改字段时，旧数据整体丢弃比「猜着迁移」安全。
//
// 这里只做「序列化 + 定时 + 兜底」，具体存哪些字段由 readPositionCore 的
// buildPersistPayload / parsePersistPayload 决定，因此可以脱离浏览器在 Node 里测。

import { buildPersistPayload, parsePersistPayload } from './readPositionCore.js'

/** localStorage 里存一个对象，位置与「每文件模式」都在里面，避免两把 key 各自过期。 */
export const READ_POSITION_STORAGE_KEY = 'guaxi_editor_read_position_v1'

/** 滚动停止后再写盘；180ms 与组件里各条滚动防抖一致，不会多出一档延迟。 */
export const READ_POSITION_PERSIST_DELAY_MS = 400

/**
 * 创建一个落盘控制器。
 *
 * @param {object} opts
 * @param {Storage|null} [opts.storage] window.localStorage 或其等价物（可注入假实现用于测试）
 * @param {string} [opts.key]
 * @param {number} [opts.delayMs]
 * @param {() => { positions: Map, modes: Map }} [opts.readSources]
 *        取当前内存态的回调。不在构造时传入是因为缓存对象在组件里是常量，
 *        而回调可以延后绑定，避免模块初始化顺序问题。
 * @param {(payload:object)=>void} [opts.onRestore] 读回数据时的回调
 */
export const createReadPositionPersistence = ({
  storage = null,
  key = READ_POSITION_STORAGE_KEY,
  delayMs = READ_POSITION_PERSIST_DELAY_MS,
  readSources = null,
  onRestore = null,
} = {}) => {
  let timer = null
  let pendingReason = ''

  const resolveStorage = () => {
    if (storage) return storage
    try {
      return typeof window !== 'undefined' ? window.localStorage : null
    } catch {
      // 隐私模式下访问 localStorage 本身就可能抛错。
      return null
    }
  }

  const cancel = () => {
    if (!timer) return
    clearTimeout(timer)
    timer = null
    pendingReason = ''
  }

  /** 立即写盘。失败静默——记不住位置比让编辑器崩掉好得多。 */
  const flush = (reason = 'flush') => {
    cancel()
    const store = resolveStorage()
    if (!store || typeof readSources !== 'function') return false
    try {
      let sources = null
      try {
        sources = readSources()
      } catch {
        sources = null
      }
      if (!sources) return false
      const payload = buildPersistPayload({
        positions: sources.positions || new Map(),
        modes: sources.modes || new Map(),
        extra: sources.lastPath ? { lastPath: String(sources.lastPath) } : {},
      })
      if (!payload.positions.length && !Object.keys(payload.modes).length && !payload.lastPath) return false
      store.setItem(key, JSON.stringify(payload))
      return true
    } catch {
      return false
    }
  }

  /** 防抖写盘。滚动过程中反复调用只会保留最后一次。 */
  const schedule = (reason = 'scheduled') => {
    pendingReason = reason
    if (timer) return
    timer = setTimeout(() => {
      timer = null
      const why = pendingReason
      pendingReason = ''
      flush(why)
    }, Math.max(0, Number(delayMs) || 0))
    // Node 测试里没有浏览器定时器语义，不 unref，其余环境无副作用。
    if (timer && typeof timer.unref === 'function') timer.unref()
  }

  /** 启动时读回。返回 { positions, modes, lastPath }，任何异常都返回空结果。 */
  const load = () => {
    const store = resolveStorage()
    if (!store) return { positions: [], modes: {}, lastPath: '' }
    let raw = null
    try {
      const text = store.getItem(key)
      raw = text ? JSON.parse(text) : null
    } catch {
      return { positions: [], modes: {}, lastPath: '' }
    }
    const parsed = parsePersistPayload(raw)
    const result = {
      positions: parsed.positions,
      modes: parsed.modes,
      lastPath: typeof raw?.lastPath === 'string' ? raw.lastPath : '',
    }
    if (typeof onRestore === 'function' && (result.positions.length || Object.keys(result.modes).length || result.lastPath)) {
      try {
        onRestore(result)
      } catch {
        /* 恢复失败不影响启动 */
      }
    }
    return result
  }

  /** 清空落盘（设置页「清除阅读位置记录」这类入口用）。 */
  const clear = () => {
    cancel()
    const store = resolveStorage()
    if (!store) return
    try {
      store.removeItem(key)
    } catch {
      /* noop */
    }
  }

  return {
    load,
    flush,
    schedule,
    cancel,
    clear,
    get hasPendingWrite() {
      return Boolean(timer)
    },
  }
}
