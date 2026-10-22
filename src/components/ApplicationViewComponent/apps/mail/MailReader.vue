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
          <button class="mail-btn" @click="$emit('reply', detail)">回复</button>
          <button class="mail-btn" @click="$emit('reply-all', detail)">全部回复</button>
          <button class="mail-btn" @click="$emit('forward', detail)">转发</button>
          <span class="mail-toolbar-sep"></span>
          <button
            class="mail-btn"
            :class="{ 'is-primary': !detail.flagged }"
            @click="$emit('toggle-flag', detail)"
          >
            {{ detail.flagged ? '取消星标' : '加星标' }}
          </button>
          <button class="mail-btn" @click="$emit('toggle-unread', detail)">
            {{ detail.seen ? '标为未读' : '标为已读' }}
          </button>
          <button class="mail-btn" @click="$emit('archive', detail)">归档</button>
          <button class="mail-btn is-danger" @click="$emit('delete', detail)">删除</button>
        </div>

        <h1 class="mail-reader-subject">{{ detail.subject || '（无主题）' }}</h1>
      </header>

      <div class="mail-reader-meta">
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
          </span>
        </div>
        <span class="mail-reader-when">{{ dateLabel }}</span>
      </div>

      <div class="mail-reader-scroll">
        <!-- 远端图片默认不加载：先问一句，避免打开即给发件人回一个「已读+IP」的信标 -->
        <div v-if="hasRemoteImages && imagesBlocked" class="mail-remote-banner">
          <span>这封邮件包含外部图片，默认没有加载。</span>
          <button class="mail-btn" @click="showImages">显示图片</button>
        </div>

        <div v-if="showHeaders" class="mail-headers">
          <pre>{{ headerText }}</pre>
        </div>

        <div
          v-if="renderedHtml"
          ref="bodyRef"
          class="mail-body"
          :style="readingStyle"
          v-html="renderedHtml"
          @click="onBodyClick"
        ></div>
        <pre v-else class="mail-body" style="white-space: pre-wrap">{{ detail.text }}</pre>

        <div v-if="detail.attachments?.length" class="mail-attachments">
          <button
            v-for="att in detail.attachments"
            :key="att.part + att.filename"
            class="mail-attachment"
            :title="att.filename"
            @click="$emit('save-attachment', { attachment: att, detail })"
          >
            <span aria-hidden="true">{{ iconFor(att.contentType) }}</span>
            <span class="mail-attachment-name">{{ att.filename }}</span>
            <span class="mail-attachment-size">{{ sizeLabel(att.size) }}</span>
          </button>
        </div>
      </div>
    </template>
  </section>
</template>

<script setup>
import { computed, ref, watch } from 'vue'
import { avatarSeed } from './theme'

const props = defineProps({
  detail: { type: Object, default: null },
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
  'close',
])

const showHeaders = computed(() => Boolean(props.settings?.showHeaders))

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

const hasRemoteImages = computed(() => /data-guaxi-remote-src=/.test(props.detail?.html || ''))

const imagesBlocked = computed(() => hasRemoteImages.value && !imagesAllowed.value)

const renderedHtml = computed(() => {
  const html = props.detail?.html
  if (!html) return ''
  if (!imagesBlocked.value) {
    // 用户点了「显示图片」：把暂存地址放回 src。
    return html.replace(/data-guaxi-remote-src="([^"]*)"\s+src=""/g, 'src="$1"')
  }
  return html
})

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
