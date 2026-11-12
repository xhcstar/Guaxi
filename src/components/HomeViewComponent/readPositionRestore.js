// 滚动落位引擎：把「该滚到哪里」变成「每帧写一次 scrollTop 直到稳定」。
//
// 为什么单独成模块
//
// 这段逻辑原来长在 GuaxiEditor.vue 里，和 Monaco、预览渲染、模式切换缠在一起，
// 唯一能验证它的方式是端起整个应用点界面。可它恰恰是最容易出问题的部分：
//   - 预览是渐进渲染，图片/公式/图表会在之后若干帧继续把文档撑高；
//   - 只写一次 scrollTop 会被当时还偏小的 scrollHeight 夹断，「只恢复一点点」；
//   - 目标算不出来时若当成 0，就变成「切回来回到开头」；
//   - 停太晚又会和用户抢滚动条。
//
// 这里只依赖「一个能读 scrollHeight/clientHeight/scrollTop 的元素」和两个纯函数，
// 因此既能在 Node 里用假元素跑，也能在浏览器里对真实滚动容器跑（见 .readpos-preview）。
import {
  resolveScrollTarget,
  resolveAnchorTarget,
  isPixelTrustworthy,
  isRestoreSettled,
} from './readPositionCore.js'

/** 逐帧重试的默认预算。约 14 × (一帧 + 40ms)，覆盖图片与图表的加载时间。 */
export const RESTORE_SCROLL_MAX_ATTEMPTS = 14
export const RESTORE_SCROLL_RETRY_DELAY_MS = 40
export const RESTORE_SCROLL_STABLE_FRAMES = 2

/** 记录里是否真的写了「要滚到某处」——用于区分「目标是顶部」与「这一帧还算不出来」。 */
export const entryHasScrollTarget = (entry) => Number(entry?.scrollTop) > 0
  || Number(entry?.scrollRatio ?? entry?.ratio) > 0
  || Number(entry?.anchorLine) > 0

/**
 * 算出这一帧该滚到哪里。
 *
 * 优先级：内容锚点 > 像素（仅当文档高度仍可信）> 比例 > 不动。
 * 返回 `awaiting: true` 表示「记录里有目标，但当前帧还算不出来」，调用方应保持原视口继续等。
 */
export const planScrollTarget = (entry, metrics, resolveBlockTop) => {
  const { scrollHeight, clientHeight } = metrics
  const maxTop = Math.max(0, Number(scrollHeight || 0) - Number(clientHeight || 0))

  const anchorTarget = typeof resolveBlockTop === 'function'
    ? resolveAnchorTarget(entry, resolveBlockTop)
    : null
  const pixelTarget = resolveScrollTarget(entry, metrics)
  const pixelTrusted = isPixelTrustworthy(entry, metrics)

  let resolved = anchorTarget
  if (!resolved && pixelTrusted) resolved = pixelTarget
  if (!resolved && pixelTarget.source === 'ratio') resolved = pixelTarget

  if (!resolved) {
    // 有目标却算不出来：不能当成「滚到顶部」。
    if (entryHasScrollTarget(entry)) {
      return { top: 0, source: 'none', awaiting: true, nextTop: null, reachable: false, maxTop }
    }
    return { top: 0, source: 'none', awaiting: false, nextTop: 0, reachable: false, maxTop }
  }

  const reachable = resolved.top <= maxTop
  return {
    top: resolved.top,
    source: resolved.source,
    awaiting: false,
    nextTop: Math.min(Math.max(0, resolved.top), maxTop),
    reachable,
    maxTop,
  }
}

/**
 * 逐帧把滚动位置落到 entry 记录的位置。
 *
 * @param {object} opts
 * @param {() => (HTMLElement|null)} opts.getElement   每帧重新取元素（元素可能被重新挂载）
 * @param {object} opts.entry                         位置记录
 * @param {(line:number)=>number|null} [opts.resolveBlockTop]  内容锚点 → 当前像素
 * @param {(ms:number)=>Promise<void>} [opts.wait]     重试间隔（测试可注入）
 * @param {()=>Promise<void>} [opts.nextFrame]         每帧等待（测试可注入）
 * @param {number} [opts.maxAttempts]
 * @param {{debug?:Function}} [opts.logger]
 * @returns {Promise<{applied:number, reached:boolean, source:string, attempts:number}>}
 */
export const restoreScrollTopWithGrowthRetry = async (
  getElement,
  entry,
  {
    reason = 'unknown',
    label = 'scroll',
    resolveBlockTop = null,
    maxAttempts = RESTORE_SCROLL_MAX_ATTEMPTS,
    retryDelayMs = RESTORE_SCROLL_RETRY_DELAY_MS,
    stableFrames = RESTORE_SCROLL_STABLE_FRAMES,
    logger = null,
    nextFrame = () => new Promise((resolve) => window.requestAnimationFrame(resolve)),
    wait = (ms) => new Promise((resolve) => window.setTimeout(resolve, ms)),
  } = {},
) => {
  let lastMaxTop = -1
  let stableCount = 0
  let applied = 0
  let reached = false
  let source = 'pixels'
  let targetTop = Math.max(0, Number(entry?.scrollTop) || 0)
  let attempts = 0

  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    attempts = attempt
    await nextFrame()
    const el = getElement()
    if (!el || !el.isConnected) break

    const maxTop = Math.max(0, el.scrollHeight - el.clientHeight)
    stableCount = maxTop === lastMaxTop ? stableCount + 1 : 0
    lastMaxTop = maxTop

    const plan = planScrollTarget(
      entry,
      { scrollHeight: el.scrollHeight, clientHeight: el.clientHeight },
      resolveBlockTop,
    )
    targetTop = plan.top
    source = plan.source
    // 「有目标但算不出来」在预算内继续等；预算用尽后不再等待，保住已有的视口。
    const awaitingTarget = plan.awaiting && attempt < maxAttempts

    if (!awaitingTarget) {
      el.scrollTop = plan.nextTop
      applied = el.scrollTop
    }

    if (isRestoreSettled({
      reachable: plan.reachable,
      targetTop,
      heightStableFrames: stableCount,
      stableFrames,
      awaitingTarget,
    })) {
      reached = plan.reachable && applied >= Math.min(targetTop, plan.maxTop) - 1
      break
    }
    await wait(retryDelayMs)
  }

  logger?.debug?.('[ReadPosition] scroll restore settled', {
    reason,
    label,
    attempts,
    source,
    targetTop: Math.round(targetTop),
    applied: Math.round(applied),
    reached,
  })
  return { applied, reached, source, attempts }
}

/**
 * 落位之后的短暂看护。
 *
 * 为什么需要：文档「看起来稳定了」不代表加载结束。图片解码、Mermaid 出图、字体替换
 * 常常发生在落位收工之后；只要它们出现在视口上方，内容就会整体下移，用户原本看到的那一段
 * 被推出屏幕。用户说的「位置只是大概对」有一半来自这里。
 *
 * 做法：在观看窗口内每帧核对一次「视口顶部还是不是记录的那一块」，漂了就纠回去；
 * 用户一旦自己滚动/按键/点击，立刻放手，绝不跟用户抢滚动条。
 *
 * @returns {() => void} 停止看护
 */
export const keepAnchorStableAfterRestore = (
  getElement,
  entry,
  {
    resolveBlockTop = null,
    durationMs = 1500,
    driftTolerancePx = 2,
    nextFrame = () => new Promise((resolve) => window.requestAnimationFrame(resolve)),
    now = () => Date.now(),
  } = {},
) => {
  if (typeof resolveBlockTop !== 'function') return () => {}
  if (!entryHasScrollTarget(entry)) return () => {}
  const el = getElement()
  if (!el) return () => {}

  let stopped = false
  const stop = () => {
    if (stopped) return
    stopped = true
    el.removeEventListener?.('wheel', stop)
    el.removeEventListener?.('touchstart', stop)
    el.removeEventListener?.('pointerdown', stop)
    el.removeEventListener?.('keydown', stop)
  }
  // 用户的任何主动操作都意味着「他要去别处了」，看护必须让位。
  for (const type of ['wheel', 'touchstart', 'pointerdown', 'keydown']) {
    el.addEventListener?.(type, stop, { passive: true })
  }

  const deadline = now() + Math.max(0, Number(durationMs) || 0)
  void (async () => {
    while (!stopped && now() < deadline) {
      await nextFrame()
      if (stopped) break
      const node = getElement()
      if (!node || !node.isConnected) break
      const target = resolveAnchorTarget(entry, resolveBlockTop)
      if (!target) continue
      const maxTop = Math.max(0, node.scrollHeight - node.clientHeight)
      const nextTop = Math.min(Math.max(0, target.top), maxTop)
      if (Math.abs(node.scrollTop - nextTop) > driftTolerancePx) {
        node.scrollTop = nextTop
      }
    }
    stop()
  })()

  return stop
}
