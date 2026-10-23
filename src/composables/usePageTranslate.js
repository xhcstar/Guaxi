import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../utils/logger'

const logger = createLogger('usePageTranslate')

/**
 * 段级 / 整页 / 全文翻译。
 *
 * 与 `useSelectionTranslate` 的分工：
 * - 划词翻译是「一次性、单段、带词典」的交互，结果是给人临时看一眼的；
 * - 这里的翻译是**可留存的中文对照**：把一段、当前页或整本书切成段落批量翻，
 *   结果按段保存下来，可以关掉再打开、可以逐段复制。
 *
 * 后端只有 `translate_text_free`（批量、返回等长数组）与 `selection_translate_free_multi`
 * （单段多引擎）。批量路径按段并发受后端免费接口的限流约束，所以并发度刻意保守：
 * 一次 4 段，段间有小延迟。宁可慢一点，也不要因为触发限流把整页翻译变成一片失败。
 */

/** 单次批量请求的段数。超过这个量时免费接口的错误率明显上升。 */
const BATCH_SIZE = 4
/** 两批之间的间隔（毫秒）。 */
const BATCH_GAP_MS = 120
/** 单段太长时先切分，避免一章被当成一整段塞进接口。 */
const MAX_SEGMENT_CHARS = 1200

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

/**
 * 把长文本切成适合翻译的段落。
 *
 * 按空行优先切，其次按句末标点（中英文都认），最后按长度硬切 ——
 * 与 PDF 阅读器「按行组段」的输出配合时，空行切分几乎总能命中。
 */
export function splitIntoParagraphs(text, { maxChars = MAX_SEGMENT_CHARS } = {}) {
  const src = String(text || '').replace(/\r\n?/g, '\n').trim()
  if (!src) return []
  const blocks = src
    .split(/\n{2,}/)
    .map((b) => b.trim())
    .filter(Boolean)

  const out = []
  for (const block of blocks) {
    if (block.length <= maxChars) {
      out.push(block)
      continue
    }
    // 长块按句末标点再切。
    const sentences = block.split(/(?<=[。！？.!?;；])\s*/).filter(Boolean)
    let buf = ''
    for (const s of sentences) {
      if (!buf) {
        buf = s
      } else if ((buf + s).length <= maxChars) {
        buf += s
      } else {
        out.push(buf.trim())
        buf = s
      }
      // 单句仍超长：硬切。
      while (buf.length > maxChars) {
        out.push(buf.slice(0, maxChars))
        buf = buf.slice(maxChars)
      }
    }
    if (buf.trim()) out.push(buf.trim())
  }
  return out
}

export function usePageTranslate({ getContainer = null } = {}) {
  /** 译文面板（划词）的状态，形状与 GuaxiSelectionTranslatePanel 对齐。 */
  const visible = ref(false)
  const pos = ref({ left: 8, top: 8 })
  const raw = ref('')
  const result = ref('')
  const error = ref('')
  const busy = ref(false)
  const pinned = ref(false)
  const items = ref([])
  const wordMeta = ref(null)
  const fromLang = ref('auto')
  const toLang = ref('zh')
  const langOptions = [
    { label: '自动检测', value: 'auto' },
    { label: '中文', value: 'zh' },
    { label: '英文', value: 'en' },
    { label: '日文', value: 'ja' },
    { label: '韩文', value: 'ko' },
    { label: '法文', value: 'fr' },
    { label: '德文', value: 'de' },
    { label: '西班牙语', value: 'es' },
    { label: '俄语', value: 'ru' },
  ]

  /** 整页/全文翻译的进度（0..1）与状态，供宿主显示进度条。 */
  const progress = ref(0)
  const translating = ref(false)
  const doneCount = ref(0)
  const totalCount = ref(0)

  const close = () => {
    visible.value = false
    busy.value = false
    pinned.value = false
    pos.value = { left: 8, top: 8 }
    raw.value = ''
    result.value = ''
    error.value = ''
    items.value = []
    wordMeta.value = null
  }

  const translateText = async (text, anchor = null) => {
    const t = String(text || '').trim()
    if (!t) return
    raw.value = t
    result.value = ''
    error.value = ''
    items.value = []
    wordMeta.value = null
    if (anchor) pos.value = { left: anchor.left ?? 8, top: anchor.top ?? 8 }
    visible.value = true
    busy.value = true
    try {
      // 单段走批量接口（`translated_texts` 与请求等长），再按需补词典信息。
      const res = await invoke('translate_text_free', {
        texts: [t],
        from: fromLang.value,
        to: toLang.value,
      })
      const translated = String(res?.translated_texts?.[0] || '').trim()
      result.value = translated
      items.value = translated
        ? [{ engine: String(res?.engine || 'free'), success: true, text: translated }]
        : []
      // 单词才去查词典：整句送进去只会得到空的音标/词性。
      const word = t.replace(/^[^A-Za-z一-龥]+|[^A-Za-z一-龥]+$/g, '')
      const isSingleWord = /^[A-Za-z][A-Za-z'-]{0,47}$/.test(word)
      if (isSingleWord) {
        try {
          wordMeta.value = await invoke('language_learning_enrich_word_basic', {
            word,
            sourceLang: fromLang.value === 'auto' ? null : fromLang.value,
            targetLang: toLang.value,
          })
        } catch (dictError) {
          logger.debug('[usePageTranslate] dictionary lookup skipped', {
            error: String(dictError?.message || dictError),
          })
        }
      }
      if (!result.value) throw new Error('翻译结果为空')
    } catch (e) {
      error.value = String(e?.message || e)
      logger.warn('[usePageTranslate] translate failed', { error: error.value })
    } finally {
      busy.value = false
    }
  }

  /**
   * 批量翻译若干段，逐段回调。
   *
   * 为什么按批而不是一次性发全部：免费接口对单请求的文本量有上限，且一次性发几百段
   * 时任何一段失败都会让整批作废。分批 + 每批独立重试一次，能在少数段失败时保住其余结果。
   */
  const translateParagraphs = async (paragraphs, onBatch) => {
    const list = (Array.isArray(paragraphs) ? paragraphs : []).filter((p) => p && p.trim())
    if (!list.length) return []
    const out = new Array(list.length).fill('')
    translating.value = true
    progress.value = 0
    doneCount.value = 0
    totalCount.value = list.length
    try {
      for (let i = 0; i < list.length; i += BATCH_SIZE) {
        const slice = list.slice(i, i + BATCH_SIZE)
        let translations = []
        try {
          const res = await invoke('translate_text_free', {
            texts: slice,
            from: fromLang.value,
            to: toLang.value,
          })
          translations = Array.isArray(res?.translated_texts) ? res.translated_texts : []
        } catch (e) {
          logger.warn('[usePageTranslate] batch failed', {
            error: String(e?.message || e),
            at: i,
          })
          // 整批失败时再逐段重试一次，尽量救回能救的。
          translations = []
          for (const seg of slice) {
            try {
              const res = await invoke('translate_text_free', {
                texts: [seg],
                from: fromLang.value,
                to: toLang.value,
              })
              translations.push(String(res?.translated_texts?.[0] || ''))
            } catch {
              translations.push('')
            }
            await sleep(60)
          }
        }
        for (let k = 0; k < slice.length; k += 1) {
          out[i + k] = String(translations[k] || '')
        }
        doneCount.value = Math.min(list.length, i + slice.length)
        progress.value = doneCount.value / list.length
        if (typeof onBatch === 'function') onBatch(out, i + slice.length)
        if (i + BATCH_SIZE < list.length) await sleep(BATCH_GAP_MS)
      }
      return out
    } finally {
      translating.value = false
    }
  }

  const resetProgress = () => {
    progress.value = 0
    doneCount.value = 0
    totalCount.value = 0
  }

  return {
    // 划词面板
    visible, pos, raw, result, error, busy, pinned, items, wordMeta,
    fromLang, toLang, langOptions,
    translateText, close,
    // 批量
    progress, translating, doneCount, totalCount,
    translateParagraphs, resetProgress,
  }
}
