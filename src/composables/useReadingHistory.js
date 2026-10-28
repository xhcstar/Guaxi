import { onBeforeUnmount, ref } from 'vue'
import { createLogger } from '../utils/logger'

const logger = createLogger('ReadingHistory')

/*
 * 阅读位置历史（上一处 / 下一处）与 Alt+← / Alt+→ 往返。
 *
 * 以前只有 PDF 的宿主自己维护一份栈，而且只在「翻页式大跳转」时入栈 ——
 * 读者一路滚下来，栈始终是空的，点顶部箭头、按 Alt+← 都没有反应，
 * 表现就是「阅读位置没被记录」。
 *
 * 现在的位置记录由「滚动停稳后提交」驱动：读者停在新的一屏，上一处就被记下来了，
 * 于是 Alt+← 相当于退回约一屏（与浏览器前进/后退的心智一致）；
 * 大纲/搜索/翻页这类跳转则用 markJump() 明确把「离开的位置」记下来。
 *
 * 位置的具体表示由调用方决定（PDF 用「页码 + 页内 pt」，电子书用「章节 + 比例」，
 * 双栏阅读器用滚动像素），本模块只关心栈、阈值与快捷键。
 *
 * Alt+← / Alt+→ 在主界面本来是 Monaco 的定位键，所以这里有两道保险：
 *   1. 只有「最近一次在阅读器里按下鼠标」的那个实例才响应（多阅读器同屏时不会互相抢）；
 *   2. 焦点在输入框 / 可编辑区里时一律放行。
 */

const owners = new Set()
let lastOwner = null
let keydownBound = false

const isEditableTarget = (target) => {
  const el = target instanceof Element ? target : null
  if (!el) return false
  return Boolean(el.closest('input, textarea, select, [contenteditable="true"], [contenteditable=""]'))
}

const pickOwner = () => {
  if (lastOwner && owners.has(lastOwner) && lastOwner.isActive()) return lastOwner
  const active = [...owners].filter((owner) => owner.isActive())
  return active.length === 1 ? active[0] : null
}

const onGlobalKeydown = (event) => {
  if (event.defaultPrevented) return
  if (!event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return
  if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
  if (isEditableTarget(event.target)) return
  const owner = pickOwner()
  if (!owner) return
  event.preventDefault()
  event.stopPropagation()
  if (event.key === 'ArrowLeft') owner.back()
  else owner.forward()
}

const bindGlobalKeydown = () => {
  if (keydownBound || typeof window === 'undefined') return
  keydownBound = true
  window.addEventListener('keydown', onGlobalKeydown, true)
}

/**
 * @param {object} options
 * @param {() => any} options.getPosition 取当前位置；量不到时返回 null
 * @param {(position: any) => void} options.applyPosition 落到某个历史位置
 * @param {(a: any, b: any) => boolean} options.isFar 两个位置是否「差一段」（决定要不要记一条）
 * @param {() => boolean} options.isActive 当前实例是否真实可见（隐藏的标签页不响应快捷键）
 * @param {() => Element | null} [options.getContainer] 用于判断「最近一次操作的是谁」
 */
export function useReadingHistory({
  getPosition,
  applyPosition,
  isFar,
  isActive,
  getContainer,
}) {
  const canBack = ref(false)
  const canForward = ref(false)

  let backStack = []
  let forwardStack = []
  // 已提交的当前位置。栈里存的是「上一步在哪」，anchor 是「现在在哪」。
  let anchor = null
  // 正在执行历史跳转：这段时间里的滚动事件不算新位置。
  let applying = false

  const MAX_ENTRIES = 100

  const syncFlags = () => {
    canBack.value = backStack.length > 0
    canForward.value = forwardStack.length > 0
  }

  const snapshot = () => {
    try {
      return getPosition() || null
    } catch {
      return null
    }
  }

  const pushBack = (position) => {
    if (!position) return
    backStack.push(position)
    if (backStack.length > MAX_ENTRIES) backStack.shift()
    forwardStack = []
    syncFlags()
  }

  /** 滚动停稳后调用：离上一个记录点足够远，就把上一个点记进历史。 */
  const commit = () => {
    if (applying) return
    const current = snapshot()
    if (!current) return
    if (!anchor) {
      anchor = current
      return
    }
    let far = false
    try {
      far = isFar(anchor, current)
    } catch {
      far = false
    }
    if (!far) return
    pushBack(anchor)
    anchor = current
  }

  /** 跳转（翻页 / 大纲 / 搜索 / 书签 / 链接）之前调用：把离开的位置记下来。 */
  const markJump = () => {
    if (applying) return
    const from = anchor || snapshot()
    pushBack(from)
    // 落点由跳转之后的滚动停稳来提交，避免把中间过程记成历史。
    anchor = null
  }

  const apply = (target) => {
    applying = true
    try {
      applyPosition(target)
    } catch (error) {
      logger.warn('应用阅读位置失败', { error: String(error?.message || error) })
    }
    window.setTimeout(() => {
      applying = false
    }, 80)
  }

  const goBack = () => {
    if (!backStack.length) return false
    const target = backStack.pop()
    const current = anchor || snapshot()
    if (current) forwardStack.push(current)
    anchor = target
    apply(target)
    syncFlags()
    return true
  }

  const goForward = () => {
    if (!forwardStack.length) return false
    const target = forwardStack.pop()
    const current = anchor || snapshot()
    if (current) backStack.push(current)
    anchor = target
    apply(target)
    syncFlags()
    return true
  }

  /** 换文档时清空：上一本文档的位置拿到当前文档里没有任何意义。 */
  const reset = () => {
    backStack = []
    forwardStack = []
    anchor = null
    applying = false
    syncFlags()
  }

  const setLastOwner = () => {
    lastOwner = api
  }

  const onPointerDown = (event) => {
    const container = getContainer?.()
    if (!container) return
    const target = event.target
    if (target instanceof Node && container.contains(target)) setLastOwner()
  }

  const api = {
    canBack,
    canForward,
    commit,
    markJump,
    back: goBack,
    forward: goForward,
    reset,
    isActive,
    activate: setLastOwner,
  }

  owners.add(api)
  bindGlobalKeydown()
  if (getContainer && typeof window !== 'undefined') {
    window.addEventListener('pointerdown', onPointerDown, true)
  }

  onBeforeUnmount(() => {
    owners.delete(api)
    if (lastOwner === api) lastOwner = null
    if (getContainer && typeof window !== 'undefined') {
      window.removeEventListener('pointerdown', onPointerDown, true)
    }
  })

  return api
}
