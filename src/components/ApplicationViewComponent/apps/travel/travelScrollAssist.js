// 横向卡片条的操作手感。
//
// 原生 `overflow-x: auto` 只吃「横向手势」：鼠标滚轮竖着滚时浏览器只会滚页面，
// 于是这些卡片条只能去拖那条细细的滚动条。这里统一补三件事：
//   1. 竖向滚轮在能滚的时候转成横向滚动；
//   2. 滚到头就把滚轮还给页面，不把页面卡死在一条卡片上；
//   3. 聚焦后方向键按「一张卡」翻页，Home / End 直达两端。
//
// 用法：<div class="tr-weather-strip" v-hscroll>…</div>

const BOUND_FLAG = '__travelHscrollBound'
const MIN_STEP_PX = 96

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value))
}

/**
 * 把一次 wheel 事件换算成这条横向容器该怎么滚。
 * 抽成纯函数：不碰 DOM，方便单测覆盖「到头交还页面」这类边界。
 *
 * 返回 { handled, next }：handled=false 表示这次滚轮不该被接管（交回浏览器/页面）。
 */
export function resolveWheelScroll({
  deltaX = 0,
  deltaY = 0,
  deltaMode = 0,
  ctrlKey = false,
  scrollLeft = 0,
  scrollWidth = 0,
  clientWidth = 0,
} = {}) {
  const max = Math.max(0, scrollWidth - clientWidth)
  const current = clamp(Number(scrollLeft) || 0, 0, max)
  // 不能横向滚、或者按着 Ctrl（缩放）：都不该拦。
  if (!(max > 1) || ctrlKey) return { handled: false, next: current }
  // 触控板横向手势本来就由浏览器处理，再插一手会顿。
  if (Math.abs(deltaX) > Math.abs(deltaY)) return { handled: false, next: current }
  // deltaMode: 0=像素 1=行 2=页
  const unit = deltaMode === 1 ? 18 : deltaMode === 2 ? Math.max(1, clientWidth) : 1
  const step = (Number(deltaY) || 0) * unit
  if (!step) return { handled: false, next: current }
  const next = clamp(current + step, 0, max)
  // 已经到头：交给页面继续上下滚，别让用户觉得滚轮失灵。
  if (Math.round(next) === Math.round(current)) return { handled: false, next: current }
  return { handled: true, next }
}

function canScrollX(el) {
  return Boolean(el) && el.scrollWidth - el.clientWidth > 1
}

// 方向键一次翻一张卡：取第一个子元素的实际宽度 + 卡片间距。
export function cardStep(el) {
  if (!el) return MIN_STEP_PX
  const first = el.firstElementChild
  const width = first?.getBoundingClientRect?.().width || 0
  let gap = 0
  try {
    const style = getComputedStyle(el)
    gap = parseFloat(style.columnGap || style.gap || '0') || 0
  } catch {
    gap = 0
  }
  const step = width + gap
  if (step > 40) return step
  return Math.max(MIN_STEP_PX, Math.round((el.clientWidth || 0) * 0.6))
}

function isTypingTarget(target) {
  if (!target) return false
  if (target.isContentEditable) return true
  return /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName || '')
}

function handleWheel(event) {
  const el = event.currentTarget
  const { handled, next } = resolveWheelScroll({
    deltaX: event.deltaX,
    deltaY: event.deltaY,
    deltaMode: event.deltaMode,
    ctrlKey: event.ctrlKey,
    scrollLeft: el.scrollLeft,
    scrollWidth: el.scrollWidth,
    clientWidth: el.clientWidth,
  })
  if (!handled) return
  event.preventDefault()
  el.scrollLeft = next
}

function handleKeydown(event) {
  if (event.defaultPrevented) return
  if (event.altKey || event.ctrlKey || event.metaKey) return
  if (isTypingTarget(event.target)) return
  const el = event.currentTarget
  if (!canScrollX(el)) return
  const max = el.scrollWidth - el.clientWidth
  const step = cardStep(el)
  let next = null
  if (event.key === 'ArrowLeft') next = el.scrollLeft - step
  else if (event.key === 'ArrowRight') next = el.scrollLeft + step
  else if (event.key === 'Home') next = 0
  else if (event.key === 'End') next = max
  if (next == null) return
  next = clamp(next, 0, max)
  if (Math.round(next) === Math.round(el.scrollLeft)) return
  // 方向键在这里就是「翻卡片」，别让它再带动页面上下滚。
  event.preventDefault()
  el.scrollTo({ left: next, behavior: 'smooth' })
}

function bind(el) {
  if (!el || el[BOUND_FLAG]) return
  el[BOUND_FLAG] = true
  el.dataset.hscroll = '1'
  // 方向键要能落在卡片条上，所以它得是个可聚焦区域；模板里已经给了 tabindex 的就不动。
  let addedTabindex = false
  if (!el.hasAttribute('tabindex')) {
    el.setAttribute('tabindex', '0')
    addedTabindex = true
  }
  el.addEventListener('wheel', handleWheel, { passive: false })
  el.addEventListener('keydown', handleKeydown)
  el.__travelHscrollCleanup = () => {
    el.removeEventListener('wheel', handleWheel)
    el.removeEventListener('keydown', handleKeydown)
    if (addedTabindex) el.removeAttribute('tabindex')
    delete el.dataset.hscroll
    delete el.__travelHscrollCleanup
    el[BOUND_FLAG] = false
  }
}

function unbind(el) {
  el?.__travelHscrollCleanup?.()
}

export const vHscroll = {
  mounted: bind,
  unmounted: unbind,
}

export default vHscroll
