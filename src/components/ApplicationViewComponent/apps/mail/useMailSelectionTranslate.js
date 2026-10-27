/**
 * 邮件页的划词翻译面板：阅读区与写信编辑器共用。
 *
 * 复用项目里现成的 `useSelectionTranslate` + `GuaxiSelectionTranslatePanel`
 * （电子书、PDF、认知收件箱用的都是同一套），保证交互与译文格式在所有页面一致。
 *
 * 这里只做两件邮件特有的适配：
 * 1. **定位**：面板相对「阅读区 / 编辑区」定位，而不是相对视口，
 *    否则窗口滚动后浮动面板会飘到与选区无关的位置；
 * 2. **触发时机**：只对「非空选区 + 鼠标松开」响应，并且避免与右键菜单、
 *    表格拖拽冲突。
 */
import { computed, ref } from 'vue'
import { useSelectionTranslate } from '../../../../composables/useSelectionTranslate'

/** 面板宽度（与 GuaxiSelectionTranslatePanel 的 PW 一致）。 */
const PANEL_WIDTH = 460
const PANEL_HEIGHT_ESTIMATE = 300

export function useMailSelectionTranslate(getContainer) {
  const translate = useSelectionTranslate({
    // 取词范围限定在容器内：邮件正文里拖选时不会把界面文字也带进去。
    getContainer: () => (typeof getContainer === 'function' ? getContainer() : getContainer) || null,
  })

  const visible = translate.visible
  const pos = translate.pos
  const raw = translate.raw
  const result = translate.result
  const error = translate.error
  const busy = translate.busy
  const pinned = translate.pinned
  const items = translate.items
  const wordMeta = translate.wordMeta
  const fromLang = translate.fromLang
  const toLang = translate.toLang
  const langOptions = translate.langOptions

  /** 上一块被翻译的选区（用于「重译」按钮）。 */
  const lastText = ref('')

  /**
   * 打开面板并翻译。
   *
   * @param {string} text 选中的原文
   * @param {DOMRect} rect 选区的包围盒（视口坐标）
   */
  function openAt(text, rect) {
    const container = typeof getContainer === 'function' ? getContainer() : getContainer
    const content = String(text || '').trim()
    if (!content || !container || !rect) return
    const box = container.getBoundingClientRect()
    // 选区坐标 → 容器坐标
    let left = rect.left - box.left
    let top = rect.bottom - box.top + 8
    left = Math.max(8, Math.min(left, Math.max(8, box.width - PANEL_WIDTH - 8)))
    if (top + PANEL_HEIGHT_ESTIMATE > box.height) {
      top = Math.max(8, rect.top - box.top - PANEL_HEIGHT_ESTIMATE)
    }
    pos.value = { left: Math.round(left), top: Math.round(top) }
    visible.value = true
    lastText.value = content
    translate.translate(content)
  }

  /** 重译当前选区。 */
  function retranslate() {
    if (lastText.value) translate.translate(lastText.value)
  }

  /** 关闭面板。 */
  function close() {
    lastText.value = ''
    translate.close()
  }

  /**
   * 内容区的 mouseup 处理器：有选区就翻译，没有就收起。
   *
   * 只在「鼠标松开」时触发，避免拖动选择的过程中反复弹面板。
   */
  function handleMouseUp(event) {
    // 右键 / 中键不触发
    if (event?.button !== 0) return
    const selection = window.getSelection()
    const text = selection?.toString?.() || ''
    if (!text.trim() || text.trim().length < 1) {
      return
    }
    const range = selection.rangeCount ? selection.getRangeAt(0) : null
    if (!range) return
    const rect = range.getBoundingClientRect()
    if (!rect || (!rect.width && !rect.height)) return
    openAt(text, rect)
  }

  /** 光标在选区外点击时收起（面板自己处理固定状态）。 */
  function handleDocumentMouseDown(event) {
    if (!visible.value || pinned.value) return
    if (event.target?.closest?.('.guaxi-selection-translate-panel')) return
    close()
  }

  return {
    visible,
    pos,
    raw,
    result,
    error,
    busy,
    pinned,
    items,
    wordMeta,
    fromLang,
    toLang,
    langOptions: langOptions || [],
    lastText: computed(() => lastText.value),
    openAt,
    retranslate,
    close,
    handleMouseUp,
    handleDocumentMouseDown,
    // 直接透传面板需要的动作
    onTranslate: () => retranslate(),
    onCopy: () => translate.copyResult(),
    onSwapLangs: () => translate.swapLangs(),
    onTogglePin: () => translate.togglePin(),
  }
}
