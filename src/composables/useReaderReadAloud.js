import { computed, ref, shallowRef } from 'vue'
import { createLogger } from '../utils/logger'
import { onReadaloudSegment } from '../utils/readaloudEvents'
import { buildTextIndex, globalOffsetOf, offsetAtPoint, rangeFromOffsets, scrollRangeIntoView, segmentIndexAtOffset } from '../utils/textOffsets'
import { useReadAloud } from './useReadAloud'

const logger = createLogger('useReaderReadAloud')

/** 当前朗读句的高亮名（全局样式与 iframe 内注入的样式都用它）。 */
export const READ_ALOUD_HIGHLIGHT = 'guaxi-readaloud'

/**
 * 把"逐句朗读"接到一个阅读器上：建文本索引、画当前句高亮、跟随滚动、点哪读哪。
 *
 * 五个阅读器（markdown/pdf/epub/text/word）的这部分完全同构，所以只此一份；差异只有两处，
 * 用 `getContext()` 吸收：
 * - **内容在哪个 DOM 里**：普通阅读器就是页面内的容器；EPUB 的正文在 iframe 里，
 *   所以连 `win`（CSS Custom Highlight 是挂在每个 window 上的）和 `scrollRoot` 一起给出来。
 * - **什么时候能拿到**：EPUB 要等渲染出章节，所以每处都按"用时现取"处理，而不是启动时缓存。
 *
 * @param {{
 *   getContext: () => ({ root: Node|null, scrollRoot: HTMLElement|null, win: (Window|null), doc?: Document|null }),
 *   getIdentity: () => ({ library: string, fileName: string }),
 *   speechIndex: import('vue').Ref<number>,
 *   onIndexChange?: (index: number) => void,
 *   message: { warning: Function, error: Function },
 *   ownerLabel?: string,
 * }} options
 */
export function useReaderReadAloud(options) {
  const message = options.message
  const ownerLabel = options.ownerLabel || '阅读器'
  const textIndex = shallowRef({ text: '', nodes: [] })
  /** 建索引时用的那一批节点是否还在文档里——阅读器重渲染后索引会失效，要重建。 */
  let indexRoot = null

  const buildIndex = (root) => {
    const index = buildTextIndex(root)
    textIndex.value = index
    indexRoot = root
    return index
  }

  const indexIsStale = () => {
    const first = textIndex.value?.nodes?.[0]?.node
    if (!first) return true
    if (!first.isConnected) return true
    return indexRoot !== options.getContext().root
  }

  const highlightRegistry = () => {
    const win = options.getContext().win
    return win && win.CSS && win.CSS.highlights ? win.CSS.highlights : null
  }

  const clearHighlight = () => {
    try {
      highlightRegistry()?.delete(READ_ALOUD_HIGHLIGHT)
    } catch {
      /* 文档已销毁 */
    }
  }

  /** 标注当前朗读句。用 CSS Custom Highlight，不动渲染出的 DOM（插节点会破坏链接与锚点）。 */
  const paintHighlight = (index) => {
    const registry = highlightRegistry()
    if (!registry) return
    const context = options.getContext()
    if (!context.root) return
    if (indexIsStale()) {
      buildIndex(context.root)
    }
    const segment = segments.value[index]
    const range = segment
      ? rangeFromOffsets(textIndex.value, segment.startOffset, segment.endOffset)
      : null
    if (!range) {
      clearHighlight()
      return
    }
    try {
      registry.set(READ_ALOUD_HIGHLIGHT, new Highlight(range))
    } catch (e) {
      logger.warn('设置朗读高亮失败', e)
      return
    }
    scrollRangeIntoView(range, context.scrollRoot)
  }

  const handleSegmentChange = (index) => {
    options.speechIndex.value = index
    paintHighlight(index)
    options.onIndexChange?.(index)
  }

  const {
    segments,
    currentIndex,
    isPlaying,
    isBuffering,
    mode,
    cachedMs,
    prepare: prepareSession,
    play,
    pause,
    resume,
    seek,
  } = useReadAloud({ onSegmentChange: handleSegmentChange })

  const title = computed(() => {
    if (isBuffering.value) return '正在缓冲朗读音频…'
    if (isPlaying.value) return '暂停朗读'
    const stored = mode.value === 'complete' ? '（整本语音已存好）' : ''
    const index = options.speechIndex.value
    return index >= 0 ? `从第 ${index + 1} 句继续朗读${stored}` : `逐句朗读本文${stored}`
  })

  /** 首次朗读才建索引与后端会话：不朗读的阅读过程不该付这份成本。 */
  const prepare = async () => {
    const context = options.getContext()
    if (!context.root) {
      message.warning(`正文还没就绪，无法朗读`)
      return false
    }
    const index = buildIndex(context.root)
    if (!index.text.trim()) {
      message.warning('正文为空，无法朗读')
      return false
    }
    const identity = options.getIdentity()
    try {
      const ready = await prepareSession(index.text, {
        library: identity.library,
        fileName: identity.fileName,
        // 不传语速：后端会用设置页「朗读声音」里的全局语速（并夹在 0.5–2.0）。
      })
      if (!ready) {
        message.warning('这篇文档没有可朗读的内容')
        return false
      }
      return true
    } catch (e) {
      logger.error(`[${ownerLabel}] 朗读准备失败:`, e)
      message.error(`朗读准备失败：${String(e?.message || e || '')}`)
      return false
    }
  }

  const toggle = async () => {
    if (isPlaying.value) {
      pause()
      return
    }
    if (segments.value.length) {
      resume()
      return
    }
    const ready = await prepare()
    if (!ready) return
    const from = options.speechIndex.value >= 0 ? options.speechIndex.value : 0
    await play({ fromIndex: from })
  }

  /**
   * 朗读中点击正文 = 从点到的那一句开始读。
   * 用 caret 命中点换算偏移，不必给每句包节点。返回 true 表示这次点击已被消费。
   */
  const seekAtPoint = (event) => {
    if (!segments.value.length) return false
    const context = options.getContext()
    const point = offsetAtPoint(event.clientX, event.clientY, context.doc || null)
    if (!point) return false
    if (indexIsStale()) {
      if (!context.root) return false
      buildIndex(context.root)
    }
    const offset = globalOffsetOf(textIndex.value, point.node, point.offset)
    if (offset === null) return false
    const index = segmentIndexAtOffset(segments.value, offset)
    if (index < 0 || index === currentIndex.value) return false
    seek(index)
    return true
  }

  /** 阅读器内容重渲染后（EPUB 翻章、Word 重排）标记索引失效，下次用时重建。 */
  const invalidate = () => {
    indexRoot = null
  }

  /**
   * 正文整体换掉之后（EPUB 翻到下一章）重建会话并从头读下去。
   * 旧的段表对应的是上一章的文本，偏移与音频都作废，只能重新准备。
   */
  const restart = async () => {
    const ready = await prepare()
    if (!ready) return
    await play({ fromIndex: 0 })
  }

  return {
    segments,
    index: currentIndex,
    isPlaying,
    isBuffering,
    mode,
    cachedMs,
    title,
    toggle,
    seekAtPoint,
    paintHighlight,
    clearHighlight,
    invalidate,
    restart,
  }
}
