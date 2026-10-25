<template>
  <section class="mail-pane mail-reader">
    <div v-if="loading" class="mail-reader-placeholder">
      <span class="mail-spin"></span>
    </div>

    <template v-else-if="detail">
      <header class="mail-reader-head">
        <button class="mail-btn" @click="$emit('close')" :title="closeLabel">
          ‹ {{ closeLabel }}
        </button>
        <div class="mail-reader-toolbar">
          <button class="mail-btn is-icon" title="回复" aria-label="回复" @click="$emit('reply', detail)">
            <MailIcon name="reply" />
          </button>
          <button
            class="mail-btn is-icon"
            title="全部回复"
            aria-label="全部回复"
            @click="$emit('reply-all', detail)"
          >
            <MailIcon name="replyAll" />
          </button>
          <button class="mail-btn is-icon" title="转发" aria-label="转发" @click="$emit('forward', detail)">
            <MailIcon name="forward" />
          </button>
          <span class="mail-toolbar-sep"></span>
          <button
            class="mail-btn is-icon"
            :class="{ 'is-on': detail.flagged }"
            :title="detail.flagged ? '取消星标' : '加星标'"
            :aria-label="detail.flagged ? '取消星标' : '加星标'"
            @click="$emit('toggle-flag', detail)"
          >
            <MailIcon name="star" :filled="detail.flagged" />
          </button>
          <button
            class="mail-btn is-icon"
            :title="detail.seen ? '标为未读' : '标为已读'"
            :aria-label="detail.seen ? '标为未读' : '标为已读'"
            @click="$emit('toggle-unread', detail)"
          >
            <MailIcon name="unread" :filled="!detail.seen" />
          </button>
          <button class="mail-btn is-icon" title="归档" aria-label="归档" @click="$emit('archive', detail)">
            <MailIcon name="archive" />
          </button>
          <button
            class="mail-btn is-icon is-danger"
            title="删除"
            aria-label="删除"
            @click="$emit('delete', detail)"
          >
            <MailIcon name="trash" />
          </button>
          <span class="mail-toolbar-sep"></span>
          <button
            class="mail-btn is-icon"
            :disabled="tools.translating.value"
            :title="'把正文整封翻译成 ' + tools.targetLang.value"
            aria-label="翻译全文"
            @click="tools.translateAll()"
          >
            <MailIcon name="translate" />
          </button>
          <button
            class="mail-btn is-icon"
            :class="{ 'is-on': tools.readAloud.isPlaying.value }"
            :title="tools.readAloud.isPlaying.value ? '暂停朗读' : '朗读正文'"
            :aria-label="tools.readAloud.isPlaying.value ? '暂停朗读' : '朗读正文'"
            @click="tools.toggleReadAloud()"
          >
            <MailIcon name="speak" />
          </button>
          <select class="mail-select" title="导出为…" aria-label="导出为" @change="onExportChange($event)">
            <option value="">导出…</option>
            <option value="eml">原始邮件 (.eml)</option>
            <option value="md">Markdown</option>
            <option value="html">HTML</option>
            <option value="pdf">PDF</option>
            <option value="docx">Word</option>
            <option value="txt">纯文本</option>
          </select>
          <button class="mail-btn is-icon" title="保存到笔记库" aria-label="保存到笔记库" @click="tools.saveToNotes()">
            <MailIcon name="note" />
          </button>
          <button class="mail-btn is-icon" title="转成一条任务" aria-label="转成一条任务" @click="tools.toTask()">
            <MailIcon name="task" />
          </button>
        </div>

        <h1 class="mail-reader-subject">{{ detail.subject || '（无主题）' }}</h1>
      </header>

      <div v-if="!isThread" class="mail-reader-meta">
        <span
          class="mail-avatar mail-reader-avatar"
          :style="{ background: `hsl(${avatarHue} 42% 48%)` }"
        >
          {{ avatarLetter }}
        </span>
        <div class="mail-reader-from">
          <span class="mail-reader-from-name">{{ fromLabel }}</span>
          <span class="mail-reader-from-addr">
            {{ detail.fromAddr }}
            <template v-if="detail.to?.length"> · 发给 {{ toSummary }}</template>
            <template v-if="ccSummary"> · 抄送 {{ ccSummary }}</template>
          </span>
        </div>
        <span class="mail-reader-when">{{ dateLabel }}</span>
      </div>

      <div ref="scrollRef" class="mail-reader-scroll">
        <!-- 远端图片默认不加载：先问一句，避免打开即给发件人回一个「已读+IP」的信标 -->
        <div v-if="hasRemoteImages && imagesBlocked" class="mail-remote-banner">
          <span>这封邮件包含外部图片，默认没有加载。</span>
          <button class="mail-btn" @click="showImages">显示图片</button>
        </div>

        <div v-if="tools.translateError.value" class="mail-search-error">
          {{ tools.translateError.value }}
        </div>

        <div v-if="tools.showTranslation.value" class="mail-translation">
          <div class="mail-translation-head">
            <span>全文翻译（{{ tools.targetLang.value }}）</span>
            <button class="mail-btn" @click="tools.showTranslation.value = false">收起</button>
          </div>
          <div class="mail-body" v-html="tools.translatedHtml.value"></div>
        </div>

        <div v-if="!isThread && showHeaders" class="mail-headers">
          <pre>{{ headerText }}</pre>
        </div>

        <!-- 会话视图：同一 thread 的每封邮件一张独立卡片，不再把历史塞进当前正文。 -->
        <div v-if="isThread" class="mail-thread">
          <article
            v-for="msg in threadMessages"
            :key="threadKeyOf(msg)"
            class="mail-thread-card"
          >
            <header class="mail-thread-head">
              <span
                class="mail-thread-avatar"
                :style="{ background: `hsl(${threadSeed(msg).hue} 42% 48%)` }"
              >
                {{ threadSeed(msg).letter }}
              </span>
              <div class="mail-thread-who">
                <span class="mail-thread-name">{{ threadName(msg) }}</span>
                <span class="mail-thread-subject">{{ msg.subject || '（无主题）' }}</span>
                <span v-if="threadRecipients(msg)" class="mail-thread-to">
                  {{ threadRecipients(msg) }}
                </span>
              </div>
              <span class="mail-thread-date">{{ threadDate(msg) }}</span>
            </header>
            <div
              class="mail-body mail-thread-body"
              :style="readingStyle"
              v-html="threadBody(msg)"
              @click="onBodyClick"
            ></div>
            <div v-if="msg.attachments?.length" class="mail-attachments">
              <div
                v-for="att in msg.attachments"
                :key="att.part + att.filename"
                class="mail-attachment-item"
              >
                <button
                  class="mail-attachment"
                  :title="`用系统默认程序打开 ${att.filename}`"
                  @click="$emit('open-attachment', { attachment: att, detail: msg })"
                >
                  <span aria-hidden="true">{{ iconFor(att.contentType) }}</span>
                  <span class="mail-attachment-name">{{ att.filename }}</span>
                  <span class="mail-attachment-size">{{ sizeLabel(att.size) }}</span>
                </button>
                <button
                  class="mail-attachment-save"
                  title="保存到本地"
                  aria-label="保存到本地"
                  @click="$emit('save-attachment', { attachment: att, detail: msg })"
                >
                  <MailIcon name="download" />
                </button>
              </div>
            </div>
          </article>
        </div>

        <template v-else>
          <div
            v-if="renderedHtml"
            ref="bodyRef"
            class="mail-body"
            :style="readingStyle"
            v-html="renderedHtml"
            @click="onBodyClick"
          ></div>
          <pre v-else class="mail-body mail-body--plain">{{ detail.text }}</pre>

          <div v-if="detail.attachments?.length" class="mail-attachments">
            <div
              v-for="att in detail.attachments"
              :key="att.part + att.filename"
              class="mail-attachment-item"
            >
              <button
                class="mail-attachment"
                :title="`用系统默认程序打开 ${att.filename}`"
                @click="$emit('open-attachment', { attachment: att, detail })"
              >
                <span aria-hidden="true">{{ iconFor(att.contentType) }}</span>
                <span class="mail-attachment-name">{{ att.filename }}</span>
                <span class="mail-attachment-size">{{ sizeLabel(att.size) }}</span>
              </button>
              <button
                class="mail-attachment-save"
                title="保存到本地"
                aria-label="保存到本地"
                @click="$emit('save-attachment', { attachment: att, detail })"
              >
                <MailIcon name="download" />
              </button>
            </div>
          </div>
        </template>
      </div>
    </template>
  </section>
</template>

<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import { avatarSeed } from './theme'
import { useMailReaderTools } from './useMailReaderTools'
import MailIcon from './MailIcon.vue'

const props = defineProps({
  detail: { type: Object, default: null },
  /** 会话内每封邮件的详情（按时间升序）；为空表示普通单封阅读。 */
  thread: { type: Array, default: () => [] },
  loading: { type: Boolean, default: false },
  settings: { type: Object, default: () => ({}) },
  themeMode: { type: String, default: 'dark' },
  monoFont: { type: String, default: '' },
  /** 列表收起时「返回列表」没有目标，换成「收起邮件」。 */
  closeLabel: { type: String, default: '返回列表' },
})
defineEmits([
  'reply',
  'reply-all',
  'forward',
  'delete',
  'archive',
  'toggle-flag',
  'toggle-unread',
  'save-attachment',
  'open-attachment',
  'close',
])

const showHeaders = computed(() => Boolean(props.settings?.showHeaders))

// 增强阅读：翻译 / 朗读 / 导出 / 笔记 / 任务
const tools = useMailReaderTools({
  getDetail: () => props.detail,
  notify: (text, isError) => console.warn('[mail-reader]', isError ? 'error:' : '', text),
  settings: props.settings,
})

/** 阅读区字号 / 字体由「外观」控制；正文根节点单独再落一次，避免邮件自带样式覆盖。 */
const readingStyle = computed(() => ({
  fontSize: `${props.settings?.readingSize || 15}px`,
}))
const imagesAllowed = ref(false)
const bodyRef = ref(null)

// 换邮件时重置「显示图片」的选择：一封邮件一次授权，不要跨邮件延续。
watch(
  () => `${props.detail?.accountId}:${props.detail?.folder}:${props.detail?.uid}`,
  () => {
    imagesAllowed.value = false
  },
)

const isThread = computed(() => (props.thread?.length || 0) > 1)
const threadMessages = computed(() => (isThread.value ? props.thread : []))

const hasRemoteImages = computed(() => {
  const html = isThread.value
    ? threadMessages.value.map((message) => message.html || '').join('')
    : props.detail?.html || ''
  return /data-guaxi-remote-src=/.test(html)
})

const imagesBlocked = computed(() => hasRemoteImages.value && !imagesAllowed.value)

/**
 * 把暂存的远端图片地址放回 `src`。
 *
 * 不能假设 `src=""` 一定还在：二次清洗（老缓存带 `<style>` 时会重新过一遍
 * sanitize）会把空的 src 当成非法图片地址丢掉，只剩 `data-guaxi-remote-src`。
 * 两种形态都要能恢复，否则点了「显示图片」只会留下碎图图标。
 */
function restoreRemoteImages(html) {
  return String(html || '')
    .replace(/data-guaxi-remote-src="([^"]*)"\s+src=""/g, 'src="$1"')
    .replace(/data-guaxi-remote-src="([^"]*)"/g, 'src="$1"')
}

const scrollRef = ref(null)

/** 解析 rgb()/rgba()，返回相对亮度（0-1）与透明度。 */
function colorInfo(value) {
  const match = String(value || '').match(
    /rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)(?:[,/\s]+([\d.]+))?\s*\)/i,
  )
  if (!match) return null
  const [r, g, b] = [Number(match[1]), Number(match[2]), Number(match[3])]
  const alpha = match[4] === undefined ? 1 : Number(match[4])
  return { luminance: (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255, alpha }
}

/**
 * 颜色是否接近白色。
 * 阈值取 0.95 而不是「比较亮」：只有近白背景反转后才会变成纯黑块；
 * 暖色/品牌色背景（米色 #F3EFE6、浅蓝）反转成深色系是原设计的暗色版本，应当保留。
 */
function isLightColor(value, threshold = 0.95) {
  const info = colorInfo(value)
  return Boolean(info && info.alpha >= 0.15 && info.luminance > threshold)
}

/** 浅色小字（次要说明、页脚）：反色后会变成深字，需要按背景重新决定。 */
function isLightText(value) {
  const info = colorInfo(value)
  return Boolean(info && info.alpha >= 0.5 && info.luminance > 0.6)
}

/** 渐变的所有可见色标都接近白色（QQ 邮箱页脚那种 #F6F6F6 → #FFFFFF）。 */
function isLightGradient(image) {
  const text = String(image || '')
  if (!/gradient/i.test(text)) return false
  const colors = text.match(/rgba?\([^)]*\)/gi) || []
  const visible = colors.filter(
    (color) => !/rgba\(\s*\d+\s*,\s*\d+\s*,\s*\d+\s*,\s*0(?:\.0+)?\s*\)/i.test(color),
  )
  return visible.length > 0 && visible.every((color) => isLightColor(color))
}

/**
 * 元素背后的底色（含祖先）是不是浅色。
 * 用来判断「浅色文字」在原设计里是不是写在浅底上的：是的话反转后会变成深字深底，
 * 需要由 guaxi-soft-fg 把颜色换回来。
 */
function hasLightBackdrop(el) {
  const body = el.closest('.mail-body')
  let node = el
  while (node) {
    const style = getComputedStyle(node)
    const image = style.backgroundImage
    if (image && image !== 'none') {
      // 浅色渐变算浅底；深色渐变/背景图（可能是深色照片）不猜，交给原来的反色。
      if (isLightGradient(image)) return true
      return false
    }
    const bg = style.backgroundColor
    if (bg && !/rgba\([^)]*,\s*0(?:\.0+)?\s*\)/i.test(bg) && bg !== 'transparent') {
      // 判断「文字原本写在什么底色上」用宽松一点的阈值：
      // 米色、浅蓝这类品牌底色反转后是深色系，浅色文字同样需要换回浅色。
      return isLightColor(bg, 0.7)
    }
    if (node === body) break
    node = node.parentElement
  }
  // 没有任何显式底色时落到的就是阅读纸面（浅色）。
  return true
}

/**
 * 暗色正文靠 `invert(1)` 自动压暗，邮件里写死的浅色背景会反转成纯黑块，
 * 写在浅底上的浅色文字也会变成「深字深底」。
 * 这里标记「浅底」元素，由 mail.css 在暗色主题下把底色抹掉、把浅色文字换回浅色。
 * 用 class 而不是内联样式：切回亮色主题规则自动失效。
 */
function softenLightBackgrounds() {
  if (props.themeMode === 'light' || !scrollRef.value) return
  scrollRef.value.querySelectorAll('.mail-body [style]').forEach((el) => {
    const style = getComputedStyle(el)
    if (isLightColor(style.backgroundColor)) el.classList.add('guaxi-soft-bg')
    if (isLightGradient(style.backgroundImage)) el.classList.add('guaxi-soft-bg-image')
    if (isLightText(style.color) && hasLightBackdrop(el)) el.classList.add('guaxi-soft-fg')
  })
}

const renderedHtml = computed(() => {
  const html = props.detail?.html
  if (!html) return ''
  // 用户点了「显示图片」：把暂存地址放回 src。
  const restored = imagesBlocked.value ? html : restoreRemoteImages(html)
  return upgradeQuoteCards(restored)
})

// 正文 DOM 更新后（换信、显示图片、切暗色主题）重新标记浅色背景。
watch(
  [renderedHtml, () => props.themeMode, () => props.thread, imagesAllowed],
  () => softenLightBackgrounds(),
  { flush: 'post' },
)
onMounted(() => softenLightBackgrounds())

// ---------------- 会话卡片 ----------------

function threadKeyOf(message) {
  return `${message.accountId}:${message.folder}:${message.uid}`
}

function threadSeed(message) {
  return avatarSeed(message.fromName, message.fromAddr)
}

function threadName(message) {
  return (message.fromName || '').trim() || message.fromAddr || '（未知发件人）'
}

function threadDate(message) {
  if (!message.date) return ''
  return new Date(message.date * 1000).toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
}

function threadRecipients(message) {
  const names = (list) =>
    (list || [])
      .map((item) => item?.name || item?.address)
      .filter(Boolean)
      .slice(0, 4)
  const to = names(message.to)
  const cc = names(message.cc)
  const parts = []
  if (to.length) parts.push(`发给 ${to.join('、')}`)
  if (cc.length) parts.push(`抄送 ${cc.join('、')}`)
  return parts.join(' · ')
}

/** 会话里每封邮件的正文：历史和嵌套引用已经由上方卡片呈现，这里直接去掉。 */
function threadBody(message) {
  if (message.html) {
    const restored = imagesBlocked.value ? message.html : restoreRemoteImages(message.html)
    return upgradeQuoteCards(restored, { dropQuotes: true })
  }
  return `<pre class="mail-body--plain" style="white-space:pre-wrap;margin:0">${escapeHtml(
    message.text || '',
  )}</pre>`
}

function escapeHtml(text) {
  return String(text)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}

/**
 * 把引用历史重排成 Apple Mail 式的消息卡片。
 *
 * 收件时的清洗层会剥掉 class，但保留 `data-*` 与内联样式：
 * - 新邮件靠 `data-guaxi-quote-*` 找结构；
 * - 旧邮件没有标记，按「10/14px 圆角外框 + 内含 blockquote」识别。
 * 处理方式：去掉旧的内联样式、补头像与主题占位，再交给 mail.css 的卡片样式。
 * DOMParser 解析出来的是惰性文档，不会执行脚本；正文本身也已在清洗层过滤过。
 */
function upgradeQuoteCards(html, { dropQuotes = false } = {}) {
  if (!html || !html.includes('blockquote')) return html
  try {
    const doc = new DOMParser().parseFromString(html, 'text/html')
    const cards = [...doc.querySelectorAll('div')].filter((div) => {
      if (div.dataset.guaxiQuoteCard) return true
      const style = div.getAttribute('style') || ''
      if (!/border-radius\s*:\s*(10|14)px/i.test(style)) return false
      const head = div.firstElementChild
      if (!head || head.tagName !== 'DIV' || !div.querySelector('blockquote')) return false
      // 只升级「像历史邮件头」的圆角块（含邮箱地址或时间），避免误伤普通内容卡片。
      const text = head.textContent || ''
      return /@|写道|wrote/i.test(text) || /\d{1,2}:\d{2}/.test(text)
    })
    // 会话视图：历史邮件已经是上方的独立卡片，正文里的引用块直接去掉，避免重复。
    if (dropQuotes) {
      for (const card of cards) card.remove()
      return doc.body.innerHTML
    }
    for (const card of cards) {
      card.classList.add('mail-quote--card')
      delete card.dataset.guaxiQuoteCard
      card.removeAttribute('style')
      const head = card.querySelector('[data-guaxi-quote-head]') || card.firstElementChild
      const body =
        card.querySelector('[data-guaxi-quote-body]') || card.querySelector('blockquote')
      if (head instanceof HTMLElement && head.tagName === 'DIV') {
        head.classList.add('mail-quote-head--card')
        head.removeAttribute('style')
      }
      if (body instanceof HTMLElement) {
        body.classList.add('mail-quote-body--card')
        body.removeAttribute('style')
      }
      if (head instanceof HTMLElement) {
        for (const [selector, className] of QUOTE_PARTS) {
          const part = card.querySelector(selector)
          if (part) part.classList.add(className)
        }
        ensureQuoteHeader(doc, head)
      }
    }
    return doc.body.innerHTML
  } catch {
    return html
  }
}

const QUOTE_PARTS = [
  ['[data-guaxi-quote-avatar]', 'mail-quote-avatar'],
  ['[data-guaxi-quote-meta]', 'mail-quote-meta'],
  ['[data-guaxi-quote-from]', 'mail-quote-from'],
  ['[data-guaxi-quote-subject]', 'mail-quote-subject'],
  ['[data-guaxi-quote-date]', 'mail-quote-date'],
]

/** 旧卡片没有头像/主题行：用发件人名字首字补一个头像，主题给个占位。 */
function ensureQuoteHeader(doc, head) {
  if (head.querySelector('.mail-quote-avatar')) return
  const spans = [...head.children].filter((el) => el.tagName === 'SPAN')
  if (!spans.length) return
  const fromSpan = spans[0]
  const dateSpan = spans.length > 1 ? spans[spans.length - 1] : null
  const text = (fromSpan.textContent || '').trim()
  const avatar = doc.createElement('span')
  avatar.className = 'mail-quote-avatar'
  avatar.textContent = text ? [...text][0].toUpperCase() : '✉'
  head.insertBefore(avatar, head.firstChild)
  if (dateSpan && dateSpan !== fromSpan) {
    fromSpan.classList.add('mail-quote-from')
    dateSpan.classList.add('mail-quote-date')
    const meta = doc.createElement('span')
    meta.className = 'mail-quote-meta'
    head.insertBefore(meta, fromSpan)
    meta.appendChild(fromSpan)
    const subject = doc.createElement('span')
    subject.className = 'mail-quote-subject'
    subject.textContent = '历史消息'
    meta.appendChild(subject)
  }
}

const avatar = computed(() => avatarSeed(props.detail?.fromName, props.detail?.fromAddr))
const avatarLetter = computed(() => avatar.value.letter)
const avatarHue = computed(() => avatar.value.hue)

const fromLabel = computed(() => {
  const d = props.detail
  if (!d) return ''
  return d.fromName?.trim() || d.fromAddr || '（未知发件人）'
})

const toSummary = computed(() => {
  const list = props.detail?.to || []
  if (!list.length) return ''
  const first = list[0].name || list[0].address
  return list.length > 1 ? `${first} 等 ${list.length} 人` : first
})

const ccSummary = computed(() => {
  const list = props.detail?.cc || []
  if (!list.length) return ''
  const first = list[0].name || list[0].address
  return list.length > 1 ? `${first} 等 ${list.length} 人` : first
})

const dateLabel = computed(() => {
  const unix = props.detail?.date
  if (!unix) return ''
  return new Date(unix * 1000).toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
})

const headerText = computed(() =>
  (props.detail?.headers || []).map(([name, value]) => `${name}: ${value}`).join('\n'),
)

function showImages() {
  imagesAllowed.value = true
}

/** 正文里的链接：交给系统浏览器打开，不在应用内导航。 */
async function onBodyClick(event) {
  const anchor = event.target?.closest?.('a')
  if (!anchor) return
  const href = anchor.getAttribute('href')
  if (!href) return
  event.preventDefault()
  if (!/^https?:/i.test(href)) return
  try {
    const { openUrl } = await import('@tauri-apps/plugin-opener')
    await openUrl(href)
  } catch {
    window.open(href, '_blank')
  }
}

/** 导出下拉：选完就复位，避免下次打开还停在上次那一项。 */
function onExportChange(event) {
  const kind = event.target.value
  event.target.value = ''
  if (kind) tools.exportAs(kind).catch((error) => console.warn('[mail-reader] 导出失败', error))
}

function sizeLabel(bytes) {
  const n = Number(bytes) || 0
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`
  return `${(n / 1024 / 1024).toFixed(1)} MB`
}

function iconFor(contentType) {
  const type = String(contentType || '').toLowerCase()
  if (type.startsWith('image/')) return '🖼'
  if (type.startsWith('audio/')) return '♪'
  if (type.startsWith('video/')) return '▶'
  if (type.includes('pdf')) return 'PDF'
  if (type.includes('zip') || type.includes('compressed')) return '⌸'
  if (type.includes('word') || type.includes('document')) return 'W'
  if (type.includes('sheet') || type.includes('excel')) return 'X'
  return '📄'
}
</script>
