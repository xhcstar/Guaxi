import { ref, shallowRef } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../utils/logger'

const logger = createLogger('useBilingualPreview')

// 这些子树整体跳过：图片、代码块、数学公式、Mermaid/SVG 图表、已注入的译文、
// 以及探索者模式的可执行 Cell。用户明确要求这些内容不翻译。
const SKIP_SELECTOR = [
  'pre', 'code', 'kbd', 'samp', 'script', 'style',
  'img', 'picture', 'video', 'audio', 'canvas', 'svg',
  '.katex', '.katex-display', '.katex-inline', 'mjx-container', 'mjx-math', 'mjx-assistive-mml',
  // 本项目的数学公式渲染产物（notebookMarkdownRenderWorker 输出的类名）
  '.math-inline', '.math-block', '.math-inline-container', '.math-block-container',
  '.mermaid', '.guaxi-mermaid', '.hljs',
  '[data-guaxi-bilingual]', '.guaxi-bilingual-line',
  '.guaxi-pycell', '.guaxi-preview-file-card', '.guaxi-media-group',
].join(',')

// 块级翻译单元。只取「不再包含其它单元」的叶子块，保证译文插在最贴近原文的位置，
// 从而不打乱标题层级、列表编号和表格结构。
const UNIT_SELECTOR = [
  'p', 'li', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6',
  'td', 'th', 'blockquote', 'dd', 'dt', 'figcaption', 'summary',
].join(',')

// 这些单元的译文放进元素内部（插成兄弟会破坏列表编号 / 表格单元格结构）。
const INLINE_APPEND_TAGS = new Set(['LI', 'TD', 'TH', 'DD', 'DT', 'SUMMARY'])

const HAS_WORD_RE = /[\p{L}\p{N}]/u

const isSkipped = (el) => {
  try {
    return typeof el?.closest === 'function' && !!el.closest(SKIP_SELECTOR)
  } catch {
    return false
  }
}

/** 取单元的可翻译文本：排除被跳过的子树，压掉多余空白。 */
const extractUnitText = (unit) => {
  const parts = []
  const walk = (node) => {
    if (!node) return
    if (node.nodeType === Node.ELEMENT_NODE) {
      if (node.matches?.(SKIP_SELECTOR)) return
      for (const child of node.childNodes) walk(child)
      return
    }
    if (node.nodeType === Node.TEXT_NODE) {
      const t = node.textContent || ''
      if (t.trim()) parts.push(t)
    }
  }
  walk(unit)
  return parts.join(' ').replace(/\s+/g, ' ').trim()
}

/** 收集容器里所有待翻译单元（已翻译过的跳过）。 */
const collectUnits = (root) => {
  if (!root) return []
  const candidates = Array.from(root.querySelectorAll(UNIT_SELECTOR))
  const units = []
  for (const el of candidates) {
    if (el.dataset?.guaxiBilingualDone === '1') continue
    if (isSkipped(el)) continue
    // 只要叶子单元：内部还含有其它单元的容器交给更内层处理。
    if (el.querySelector(UNIT_SELECTOR)) continue
    const text = extractUnitText(el)
    if (!text || !HAS_WORD_RE.test(text)) continue
    units.push({ el, text })
  }
  return units
}

/** 把译文节点插到原文旁边。返回插入的元素。 */
const insertTranslation = (unit, translated) => {
  const doc = unit.ownerDocument || document
  const holder = doc.createElement('div')
  holder.className = 'guaxi-bilingual-line'
  holder.setAttribute('data-guaxi-bilingual', '1')
  holder.textContent = translated
  if (INLINE_APPEND_TAGS.has(unit.tagName)) {
    unit.appendChild(holder)
  } else {
    unit.insertAdjacentElement('afterend', holder)
  }
  unit.dataset.guaxiBilingualDone = '1'
  return holder
}

/**
 * 预览区双语全文翻译。
 *
 * 设计要点：
 * - 不改渲染管线，只在已渲染好的预览 DOM 上就地注入译文块，因此对 Markdown、
 *   纯文本、任何走同一预览容器的内容都通用，也不会打乱原文结构与格式。
 * - 逐块翻译 + 分批下发，块级切分保证标题还是标题、列表还是列表。
 * - 图片 / 代码块 / 公式 / 图表整棵子树跳过。
 * - 译文按「原文文本」做进程内缓存，重渲染（滚动重排、主题切换）不会重复请求。
 *
 * @param {object} options
 * @param {() => HTMLElement | null} options.getContainer 返回预览内容容器
 * @param {() => string} options.getSourceLang
 * @param {() => string} options.getTargetLang
 */
export function useBilingualPreview({ getContainer, getSourceLang, getTargetLang } = {}) {
  const enabled = ref(false)
  const busy = ref(false)
  const total = ref(0)
  const done = ref(0)
  const errorText = ref('')
  const cache = shallowRef(new Map())

  let runToken = 0
  const BATCH_SIZE = 12

  const cacheKey = (text) => `${getSourceLang?.() || 'auto'}|${getTargetLang?.() || 'zh'}|${text}`

  /** 移除全部已注入的译文，恢复纯原文。 */
  const clear = () => {
    runToken += 1
    busy.value = false
    total.value = 0
    done.value = 0
    errorText.value = ''
    const root = getContainer?.()
    if (!root) return
    root.querySelectorAll('[data-guaxi-bilingual="1"]').forEach((el) => el.remove())
    root.querySelectorAll('[data-guaxi-bilingual-done="1"]').forEach((el) => {
      delete el.dataset.guaxiBilingualDone
    })
  }

  /**
   * 翻译容器内尚未翻译的块。可重复调用（增量），预览重渲染后再调一次即可补齐。
   */
  const run = async () => {
    const root = getContainer?.()
    if (!root) return
    const units = collectUnits(root)
    if (!units.length) return

    const myToken = ++runToken
    busy.value = true
    errorText.value = ''
    total.value = units.length
    done.value = 0

    const from = getSourceLang?.() || 'auto'
    const to = getTargetLang?.() || 'zh'

    // 先用缓存直接填，剩下的按去重后的唯一文本请求，重复段落只算一次。
    const pending = []
    for (const unit of units) {
      const hit = cache.value.get(cacheKey(unit.text))
      if (hit) {
        insertTranslation(unit.el, hit)
        done.value += 1
      } else {
        pending.push(unit)
      }
    }

    const uniqueTexts = []
    const textToUnits = new Map()
    for (const unit of pending) {
      if (!textToUnits.has(unit.text)) {
        textToUnits.set(unit.text, [])
        uniqueTexts.push(unit.text)
      }
      textToUnits.get(unit.text).push(unit)
    }

    try {
      for (let i = 0; i < uniqueTexts.length; i += BATCH_SIZE) {
        if (myToken !== runToken) return
        const batch = uniqueTexts.slice(i, i + BATCH_SIZE)
        let response = null
        try {
          response = await invoke('translate_text_free', { texts: batch, from, to })
        } catch (err) {
          logger.warn('[Bilingual] batch failed', { err, size: batch.length })
          errorText.value = String(err?.message || err || '翻译失败')
          continue
        }
        if (myToken !== runToken) return
        const results = Array.isArray(response?.translated_texts) ? response.translated_texts : []
        if (response?.error) errorText.value = String(response.error)
        batch.forEach((text, idx) => {
          const translated = String(results[idx] || '').trim()
          const targets = textToUnits.get(text) || []
          if (!translated || translated.includes('[Translation Failed]')) {
            done.value += targets.length
            return
          }
          cache.value.set(cacheKey(text), translated)
          targets.forEach((unit) => {
            // 译文与原文完全一致（例如纯数字、纯符号行）就不重复展示。
            if (translated === text) {
              unit.el.dataset.guaxiBilingualDone = '1'
            } else {
              insertTranslation(unit.el, translated)
            }
            done.value += 1
          })
        })
      }
    } finally {
      if (myToken === runToken) {
        busy.value = false
      }
    }
  }

  /** 打开/关闭双语显示。打开时立即开始翻译，关闭时清掉译文。 */
  const toggle = async () => {
    enabled.value = !enabled.value
    if (enabled.value) {
      await run()
    } else {
      clear()
    }
  }

  /** 预览重渲染后重新注入（DOM 已被整体替换，需要重跑一遍）。 */
  const refresh = async () => {
    if (!enabled.value) return
    await run()
  }

  return { enabled, busy, total, done, errorText, toggle, refresh, clear, run }
}

