// 邮件阅读区的「增强阅读」：全文翻译、朗读、导出、存入笔记、转任务。
//
// 这些都调项目里已有的能力（translate_text_free / useReadAloud / typst），
// 不重复实现翻译与 TTS——那两块本身已经很重，邮件只是又一个调用方。
import { computed, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../../../../utils/logger'
import { useReadAloud } from '../../../../composables/useReadAloud'

const logger = createLogger('useMailReaderTools')

/** 目标语言：跟随设置的「翻译目标语言」，缺省中文。 */
function defaultTargetLang(settings) {
  const lang = String(settings?.translateTo || settings?.translateTarget || 'zh').trim()
  return lang || 'zh'
}

export function useMailReaderTools({ getDetail, notify, settings }) {
  // ── 全文翻译 ──
  const translating = ref(false)
  const showTranslation = ref(false)
  const translatedHtml = ref('')
  const translateError = ref('')
  const targetLang = ref(defaultTargetLang(settings?.value ?? settings))

  /** 把正文拆成段落翻译。整封一次性送过去会超出接口长度上限，且失败就是全失败。 */
  const splitParagraphs = (text) =>
    String(text || '')
      .split(/\n{2,}/)
      .map((p) => p.trim())
      .filter(Boolean)

  async function translateAll() {
    const detail = getDetail()
    if (!detail) return
    translating.value = true
    translateError.value = ''
    try {
      // 优先用纯文本分段：HTML 直接送去翻译会把标签也翻掉，反而更难读。
      const source = String(detail.text || '').trim()
      const paragraphs = splitParagraphs(source)
      if (!paragraphs.length) {
        translateError.value = '这封邮件没有可翻译的正文'
        return
      }
      const response = await invoke('translate_text_free', {
        texts: paragraphs,
        from: 'auto',
        to: targetLang.value,
      })
      const outs = response?.translated_texts || []
      if (!outs.length) {
        translateError.value = String(response?.error || '翻译结果为空')
        return
      }
      translatedHtml.value = outs
        .map((p) => `<p>${escapeHtml(p)}</p>`)
        .join('')
      showTranslation.value = true
    } catch (error) {
      logger.error('邮件全文翻译失败', error)
      translateError.value = String(error?.message || error)
    } finally {
      translating.value = false
    }
  }

  function escapeHtml(text) {
    return String(text || '')
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
  }

  // ── 朗读 ──
  const readAloud = useReadAloud({
    onSegmentChange: (index) => {
      currentSentenceIndex.value = index
    },
  })
  const currentSentenceIndex = ref(-1)
  const readingAvailable = computed(() => Boolean(readAloud?.isPlaying?.value !== undefined))

  async function toggleReadAloud() {
    const detail = getDetail()
    if (!detail) return
    if (readAloud.isPlaying.value) {
      readAloud.pause?.()
      return
    }
    if (readAloud.segments.value.length) {
      readAloud.play?.()
      return
    }
    // 朗读用纯文本：HTML 里的标签会被念出来。
    const text = String(detail.text || '').trim()
    if (!text) {
      notify?.('这封邮件没有可朗读的正文', true)
      return
    }
    try {
      const ok = await readAloud.prepare(text, {
        docKey: `mail:${detail.accountId}:${detail.folder}:${detail.uid}`,
        fileName: detail.subject || '邮件',
      })
      if (ok) readAloud.play?.()
    } catch (error) {
      logger.error('邮件朗读准备失败', error)
      notify?.(String(error?.message || error), true)
    }
  }

  // ── 导出 ──
  async function exportAs(kind) {
    const detail = getDetail()
    if (!detail) return
    const { save } = await import('@tauri-apps/plugin-dialog')
    const ILLEGAL = new Set([
      String.fromCharCode(92), '/', ':', '*', '?', '"', '<', '>', '|',
      String.fromCharCode(13), String.fromCharCode(10),
    ])
    const stem = [...(detail.subject || '邮件')]
      .map((ch) => (ILLEGAL.has(ch) ? '_' : ch))
      .join('')
      .slice(0, 80)
    const path = await save({
      title: `导出为 ${kind}`,
      defaultPath: `${stem}.${kind}`,
      filters: [{ name: kind.toUpperCase(), extensions: [kind] }],
    })
    if (!path) return
    await invoke('mail_export_message', {
      accountId: detail.accountId,
      folder: detail.folder,
      uid: detail.uid,
      format: kind,
      targetPath: path,
    })
    notify?.(`已导出到 ${path}`)
  }

  // ── 存入笔记 ──
  async function saveToNotes() {
    const detail = getDetail()
    if (!detail) return null
    const path = await invoke('mail_save_to_notes', {
      accountId: detail.accountId,
      folder: detail.folder,
      uid: detail.uid,
      subdir: '邮件归档',
    })
    notify?.(`已存入笔记库：${path}`)
    return path
  }

  // ── 转任务 ──
  async function toTask(options = {}) {
    const detail = getDetail()
    if (!detail) return 0
    const count = await invoke('mail_to_task', {
      accountId: detail.accountId,
      folder: detail.folder,
      uid: detail.uid,
      category: options.category || 'work',
      status: options.status || 'TODO',
      deadline: options.deadline || '',
    })
    notify?.(`已添加 ${count} 条任务（任务管理器 → 收件箱）`)
    return count
  }

  // 换邮件时清掉上一条的翻译与朗读状态，避免张冠李戴。
  watch(
    () => {
      const d = getDetail()
      return d ? `${d.accountId}:${d.folder}:${d.uid}` : ''
    },
    () => {
      showTranslation.value = false
      translatedHtml.value = ''
      translateError.value = ''
      try {
        readAloud.stop?.()
      } catch {
        /* 没在播时 stop 可能抛，无所谓 */
      }
    },
  )

  return {
    // 翻译
    translating,
    showTranslation,
    translatedHtml,
    translateError,
    targetLang,
    translateAll,
    // 朗读
    readAloud,
    readingAvailable,
    currentSentenceIndex,
    toggleReadAloud,
    // 导出 / 笔记 / 任务
    exportAs,
    saveToNotes,
    toTask,
  }
}
