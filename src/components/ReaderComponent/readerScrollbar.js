const READER_SCROLLBAR_ACTIVE_CLASS = 'is-reader-scrolling'
const readerScrollbarTimers = new WeakMap()

// 设计说明：阅读器滚动条默认保持安静，只在真实滚动发生后短暂显色。
// 这里用 WeakMap 记录每个滚动容器的隐藏计时器，避免多个阅读器互相清理状态。
export const markReaderScrollbarActive = (target, timeout = 900) => {
  if (!(target instanceof HTMLElement)) return

  target.classList.add(READER_SCROLLBAR_ACTIVE_CLASS)

  const previousTimer = readerScrollbarTimers.get(target)
  if (previousTimer) window.clearTimeout(previousTimer)

  const nextTimer = window.setTimeout(() => {
    target.classList.remove(READER_SCROLLBAR_ACTIVE_CLASS)
    readerScrollbarTimers.delete(target)
  }, timeout)

  readerScrollbarTimers.set(target, nextTimer)
}

export const clearReaderScrollbarActive = () => {
  // WeakMap 不可枚举，卸载时由 DOM 节点释放计时器引用即可；此函数保留给未来需要集中清理的调用点。
}
