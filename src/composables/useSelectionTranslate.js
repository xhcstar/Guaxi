import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../utils/logger'

const logger = createLogger('useSelectionTranslate')

/**
 * 划词翻译（富面板版）。
 *
 * 与主界面编辑器的划词翻译面板（GuaxiSelectionTranslatePanel.vue）完全一致：
 * - 多引擎译文（selection_translate_free_multi，缺失回退 translate_text_free）
 * - 单词时附词典模块（音标/词性/释义/近反义/联想/短语/例句）
 * - 语言选择、交换、重译、复制、固定、拖拽（面板组件自带）
 *
 * 用法：调用方在内容容器 mouseup 上调 handleMouseUp(event)，把返回的 state 绑给
 * <GuaxiSelectionTranslatePanel>，并接它的事件（translate/copy/swap-langs/toggle-pin/
 * update:pos/update:fromLang/update:toLang/close）。
 */

const LANG_OPTIONS = [
  { label: '自动检测', value: 'auto' },
  { label: '中文', value: 'zh' },
  { label: '英文', value: 'en' },
  { label: '日文', value: 'ja' },
  { label: '韩文', value: 'ko' },
  { label: '法文', value: 'fr' },
  { label: '德文', value: 'de' },
  { label: '西班牙语', value: 'es' },
  { label: '俄语', value: 'ru' },
  { label: '葡萄牙语', value: 'pt' },
  { label: '意大利语', value: 'it' },
  { label: '荷兰语', value: 'nl' },
  { label: '越南语', value: 'vi' },
  { label: '泰语', value: 'th' },
]

const isWordSelection = (raw) => {
  const text = String(raw || '').trim()
  if (!text || /\s/.test(text)) return false
  return /^[A-Za-z][A-Za-z'-]{0,47}$/.test(text)
}

// 与编辑器一致的取词清洗：折叠软换行、压多余空白。
const normalizeSelectionText = (raw) => {
  const text = String(raw || '').replace(/\r\n?/g, '\n')
  if (!text.trim()) return ''
  return text
    .replace(/[ \t]+\n/g, '\n')
    .replace(/\n[ \t]+/g, '\n')
    .replace(/([^\n。！？.!?;；:：])\n(?!\n)/g, '$1 ')
    .replace(/[ \t]{2,}/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
    .trim()
}

export function useSelectionTranslate({ maxChars = 4000, getContainer = null } = {}) {
  const visible = ref(false)
  const pinned = ref(false)
  const busy = ref(false)
  const raw = ref('')
  const result = ref('')
  const error = ref('')
  const items = ref([])
  const wordMeta = ref(null)
  const fromLang = ref('auto')
  const toLang = ref('zh')
  const pos = ref({ left: 8, top: 8 })
  const langOptions = LANG_OPTIONS

  let reqToken = 0
  let multiAvailable = true

  const close = () => {
    visible.value = false
    if (!pinned.value) {
      raw.value = ''
      result.value = ''
      error.value = ''
      items.value = []
      wordMeta.value = null
    }
  }

  const translate = async (overrideText) => {
    const text = normalizeSelectionText(overrideText ?? raw.value)
    if (!text) return
    if (busy.value) return

    const myToken = ++reqToken
    raw.value = text
    result.value = ''
    error.value = ''
    items.value = []
    wordMeta.value = null
    busy.value = true

    try {
      const needWordMeta = isWordSelection(text)
      const wordBasicPromise = needWordMeta
        ? invoke('language_learning_enrich_word_basic', { word: text, sourceLang: fromLang.value, targetLang: toLang.value }).catch(() => null)
        : Promise.resolve(null)
      const wordDetailsPromise = needWordMeta
        ? invoke('language_learning_enrich_word_details', { word: text, sourceLang: fromLang.value, targetLang: toLang.value }).catch(() => null)
        : Promise.resolve(null)

      let hasMulti = false
      if (multiAvailable) {
        try {
          const response = await invoke('selection_translate_free_multi', { text, from: fromLang.value, to: toLang.value })
          if (myToken !== reqToken) return
          const list = Array.isArray(response?.items) ? response.items : []
          items.value = list.map((it) => ({
            engine: String(it?.engine || ''),
            success: Boolean(it?.success),
            text: String(it?.text || ''),
            error: it?.error ? String(it.error) : '',
          }))
          const primary = items.value.find((it) => it.success && String(it.text || '').trim())
          if (primary) {
            result.value = String(primary.text || '')
            hasMulti = true
          } else {
            throw new Error(String(response?.error || '翻译结果为空'))
          }
        } catch (err) {
          const msg = String(err?.message || err || '')
          if (msg.includes('selection_translate_free_multi') && msg.toLowerCase().includes('not found')) {
            multiAvailable = false
          } else {
            logger.warn('[SelectionTranslate] multi failed, fallback to single', err)
          }
        }
      }

      if (!hasMulti) {
        const response = await invoke('translate_text_free', { texts: [text], from: fromLang.value, to: toLang.value })
        if (myToken !== reqToken) return
        const translated = String(response?.translated_texts?.[0] || '').trim()
        if (!translated || translated.includes('[Translation Failed]')) {
          throw new Error(String(response?.error || '翻译结果为空'))
        }
        items.value = [{ engine: 'Free', success: true, text: translated, error: '' }]
        result.value = translated
      }

      if (needWordMeta) {
        try {
          const [basic, details] = await Promise.all([wordBasicPromise, wordDetailsPromise])
          if (myToken !== reqToken) return
          const base = {
            word: String(basic?.word || text).trim(),
            phonetic: String(basic?.phonetic || '').trim(),
            phonetic_uk: String(basic?.phonetic_uk || '').trim(),
            phonetic_us: String(basic?.phonetic_us || '').trim(),
            part_of_speech: String(basic?.part_of_speech || '').trim(),
            meanings: Array.isArray(basic?.meanings) ? basic.meanings : [],
            synonyms: [], antonyms: [], associates: [],
            synonyms_translations: [], antonyms_translations: [], associates_translations: [],
            phrases: [], phrase_translations: [],
            examples: Array.isArray(basic?.examples) ? basic.examples : [],
            example_translations: [],
          }
          wordMeta.value = details
            ? {
              ...base,
              synonyms: Array.isArray(details?.synonyms) ? details.synonyms : [],
              antonyms: Array.isArray(details?.antonyms) ? details.antonyms : [],
              associates: Array.isArray(details?.associates) ? details.associates : [],
              synonyms_translations: Array.isArray(details?.synonyms_translations) ? details.synonyms_translations : [],
              antonyms_translations: Array.isArray(details?.antonyms_translations) ? details.antonyms_translations : [],
              associates_translations: Array.isArray(details?.associates_translations) ? details.associates_translations : [],
              phrases: Array.isArray(details?.phrases) ? details.phrases : [],
              phrase_translations: Array.isArray(details?.phrase_translations) ? details.phrase_translations : [],
              examples: Array.isArray(details?.examples) && details.examples.length ? details.examples : base.examples,
              example_translations: Array.isArray(details?.example_translations) ? details.example_translations : [],
            }
            : base
        } catch (err) {
          logger.warn('[SelectionTranslate] enrich word failed', err)
        }
      }
    } catch (err) {
      if (myToken !== reqToken) return
      const msg = String(err?.message || err || '翻译失败')
      error.value = msg
      items.value = [{ engine: 'Free', success: false, text: '', error: msg }]
    } finally {
      if (myToken === reqToken) busy.value = false
    }
  }

  const copyResult = async (payload) => {
    const t = typeof payload === 'string' ? payload : payload?.text
    const text = String(t || result.value || '')
    if (!text) return
    try { await navigator.clipboard.writeText(text) } catch { /* ignore */ }
  }

  const swapLangs = () => {
    const f = fromLang.value
    fromLang.value = toLang.value === 'auto' ? 'auto' : toLang.value
    toLang.value = f === 'auto' ? 'zh' : f
    translate(raw.value)
  }

  const togglePin = () => { pinned.value = !pinned.value }

  /**
   * 在内容容器 mouseup 上调用：**仅当按住 Ctrl 划词**才翻译，避免与"摘录"同时触发。
   * 固定(pinned)时忽略新的空点击。返回 true 表示已接管本次选择。
   */
  const handleMouseUp = (event) => {
    try {
      // 只有 Ctrl+划词 才触发翻译。普通划词留给摘录逻辑。
      if (!event?.ctrlKey) {
        return false
      }
      const sel = window.getSelection?.()
      const text = sel ? String(sel.toString() || '').trim() : ''
      if (!text) {
        if (visible.value && !pinned.value) close()
        return false
      }
      if (text.length > maxChars) return false
      if (!sel || sel.rangeCount === 0) return false
      const rect = sel.getRangeAt(0).getBoundingClientRect()
      if (!rect || (rect.width === 0 && rect.height === 0)) return false

      const PW = 460
      let left = rect.left
      left = Math.max(8, Math.min(left, window.innerWidth - PW - 8))
      let top = rect.bottom + 8
      if (top + 280 > window.innerHeight) top = Math.max(8, rect.top - 288)

      // 面板是相对定位容器的 absolute 定位，需把视口坐标换算成容器局部坐标。
      const container = getContainer?.()
      const crect = container?.getBoundingClientRect?.()
      if (crect) {
        left -= crect.left
        top -= crect.top
      }

      pos.value = { left: Math.round(left), top: Math.round(top) }
      visible.value = true
      translate(text)
      return true
    } catch (e) {
      logger.warn('[SelectionTranslate] handleMouseUp error', e)
      return false
    }
  }

  return {
    visible, pinned, busy, raw, result, error, items, wordMeta,
    fromLang, toLang, pos, langOptions,
    handleMouseUp, translate, copyResult, swapLangs, togglePin, close,
  }
}
