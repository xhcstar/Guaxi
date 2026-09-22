import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../utils/logger'

const logger = createLogger('useSelectionTranslate')

/**
 * 划词翻译（选中文字 → 弹浮层显示译文）。
 *
 * 复用后端 `translate_text_free`（与编辑器预览、各阅读器同一条翻译链路）。
 * 不依赖具体页面结构：调用方在内容容器上挂 mouseup → handleMouseUp(event)，
 * 再把 state 传给 <GuaxiInlineTranslatePopover> 渲染即可。
 *
 * @param {object} options
 * @param {() => string} [options.getTargetLang] 目标语言，默认 zh
 * @param {() => string} [options.getSourceLang] 源语言，默认 auto
 * @param {number} [options.maxChars] 超过则不翻译（避免误选整段大文本），默认 800
 */
export function useSelectionTranslate({ getTargetLang, getSourceLang, maxChars = 800 } = {}) {
  const visible = ref(false)
  const loading = ref(false)
  const sourceText = ref('')
  const resultText = ref('')
  const errorText = ref('')
  // 浮层位置（相对视口的 fixed 定位）。
  const pos = ref({ left: 0, top: 0 })

  let reqToken = 0
  const cache = new Map()

  const cacheKey = (t) => `${getSourceLang?.() || 'auto'}|${getTargetLang?.() || 'zh'}|${t}`

  const close = () => {
    visible.value = false
    loading.value = false
    sourceText.value = ''
    resultText.value = ''
    errorText.value = ''
  }

  const translate = async (text) => {
    const from = getSourceLang?.() || 'auto'
    const to = getTargetLang?.() || 'zh'
    const hit = cache.get(cacheKey(text))
    if (hit) {
      resultText.value = hit
      loading.value = false
      return
    }
    const myToken = ++reqToken
    loading.value = true
    errorText.value = ''
    resultText.value = ''
    try {
      const resp = await invoke('translate_text_free', { texts: [text], from, to })
      if (myToken !== reqToken) return
      const translated = String(resp?.translated_texts?.[0] || '').trim()
      if (!translated || translated.includes('[Translation Failed]')) {
        errorText.value = String(resp?.error || '翻译结果为空')
      } else {
        resultText.value = translated
        cache.set(cacheKey(text), translated)
      }
    } catch (e) {
      if (myToken !== reqToken) return
      logger.warn('[SelectionTranslate] translate failed', e)
      errorText.value = String(e?.message || e || '翻译失败')
    } finally {
      if (myToken === reqToken) loading.value = false
    }
  }

  /**
   * 在内容容器的 mouseup 上调用。有选中文本则定位浮层并翻译；无选区则关闭。
   * 返回 true 表示已接管本次选择（调用方可据此避免与其它 mouseup 逻辑冲突）。
   */
  const handleMouseUp = (event) => {
    try {
      const sel = window.getSelection?.()
      const text = sel ? String(sel.toString() || '').trim() : ''
      if (!text) {
        // 点击（无选区）时关闭已有浮层。
        if (visible.value) close()
        return false
      }
      if (text.length > maxChars) {
        // 选得太长（整段/整篇）不自动翻译，避免打扰与开销。
        return false
      }
      if (!sel || sel.rangeCount === 0) return false
      const rect = sel.getRangeAt(0).getBoundingClientRect()
      if (!rect || (rect.width === 0 && rect.height === 0)) return false

      // 浮层定位：选区下方偏左，超出视口边缘时收回。
      const PW = 320
      let left = rect.left
      left = Math.max(8, Math.min(left, window.innerWidth - PW - 8))
      let top = rect.bottom + 8
      // 靠近底部则放到选区上方（浮层估高 160）。
      if (top + 160 > window.innerHeight) top = Math.max(8, rect.top - 168)

      pos.value = { left: Math.round(left), top: Math.round(top) }
      sourceText.value = text
      visible.value = true
      translate(text)
      return true
    } catch (e) {
      logger.warn('[SelectionTranslate] handleMouseUp error', e)
      return false
    }
  }

  return { visible, loading, sourceText, resultText, errorText, pos, handleMouseUp, translate, close }
}
