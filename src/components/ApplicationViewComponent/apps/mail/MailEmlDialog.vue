<template>
  <div class="mail-modal-backdrop" @click.self="$emit('close')">
    <div class="mail-modal is-wide mail-eml-modal" role="dialog" aria-label="查看 .eml 邮件">
      <div class="mail-modal-head">
        <h2 class="mail-modal-title">本地邮件文件</h2>
        <span class="mail-eml-path" :title="path">{{ fileName }}</span>
        <button class="mail-modal-close" title="关闭" @click="$emit('close')">×</button>
      </div>

      <div class="mail-modal-body">
        <div class="mail-eml-head">
          <div class="mail-eml-row">
            <span class="mail-eml-key">主题</span>
            <span class="mail-eml-value">{{ detail.subject || '（无主题）' }}</span>
          </div>
          <div class="mail-eml-row">
            <span class="mail-eml-key">发件人</span>
            <span class="mail-eml-value">{{ formatAddress(detail.fromName, detail.fromAddr) }}</span>
          </div>
          <div v-if="detail.to?.length" class="mail-eml-row">
            <span class="mail-eml-key">收件人</span>
            <span class="mail-eml-value">{{ joinAddresses(detail.to) }}</span>
          </div>
          <div v-if="detail.cc?.length" class="mail-eml-row">
            <span class="mail-eml-key">抄送</span>
            <span class="mail-eml-value">{{ joinAddresses(detail.cc) }}</span>
          </div>
          <div class="mail-eml-row">
            <span class="mail-eml-key">时间</span>
            <span class="mail-eml-value">{{ dateLabel }}</span>
          </div>
        </div>

        <div v-if="hasRemoteImages && !imagesAllowed" class="mail-remote-banner">
          <span>这封邮件包含外部图片，默认没有加载。</span>
          <button class="mail-btn" @click="imagesAllowed = true">显示图片</button>
        </div>

        <div v-if="detail.html" ref="bodyRef" class="mail-body" v-html="renderedHtml"></div>
        <pre v-else class="mail-body mail-body--plain">{{ detail.text }}</pre>

        <div v-if="detail.attachments?.length" class="mail-eml-attachments">
          <div class="mail-flabel">附件（{{ detail.attachments.length }}）</div>
          <div v-for="att in detail.attachments" :key="att.part + att.filename" class="mail-attachment">
            <span aria-hidden="true">📄</span>
            <span class="mail-attachment-name">{{ att.filename }}</span>
            <span class="mail-attachment-size">{{ sizeLabel(att.size) }}</span>
          </div>
          <p class="mail-hint" style="margin: 6px 0 0">
            附件仍在文件里，没有单独解出来；需要的话用邮件客户端打开这个 .eml，或把它拖回邮箱。
          </p>
        </div>
      </div>

      <div class="mail-modal-foot">
        <span class="mail-hint">这是本机保存的 .eml 文件，不属于任何账户，也不会被同步。</span>
        <span class="spacer"></span>
        <button class="mail-btn" @click="$emit('close')">关闭</button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onMounted, ref } from 'vue'
import { applyBodyTheme } from './colors'

const props = defineProps({
  /** 后端 `mail_read_eml` 解析出的详情（HTML 已清洗）。 */
  detail: { type: Object, required: true },
  path: { type: String, default: '' },
  themeMode: { type: String, default: 'dark' },
})
defineEmits(['close'])

const imagesAllowed = ref(false)
const bodyRef = ref(null)

const fileName = computed(() => String(props.path || '').split(/[\\/]/).pop() || '邮件')

const dateLabel = computed(() => {
  const unix = props.detail?.date
  if (!unix) return '（没有时间戳）'
  return new Date(unix * 1000).toLocaleString('zh-CN', { hour12: false })
})

function formatAddress(name, address) {
  if (!name) return address || '（未知）'
  return `${name} <${address}>`
}

function joinAddresses(list) {
  return (list || []).map((item) => formatAddress(item?.name, item?.address)).join('、')
}

function sizeLabel(bytes) {
  const n = Number(bytes) || 0
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`
  return `${(n / 1024 / 1024).toFixed(1)} MB`
}

const hasRemoteImages = computed(() => /data-guaxi-remote-src=/.test(props.detail?.html || ''))

/** 远端图片默认不加载（避免打开即回一个「已读+IP」信标），点一下再还原地址。 */
const renderedHtml = computed(() => {
  const html = props.detail?.html || ''
  if (imagesAllowed.value) {
    return html
      .replace(/data-guaxi-remote-src="([^"]*)"\s+src=""/g, 'src="$1"')
      .replace(/data-guaxi-remote-src="([^"]*)"/g, 'src="$1"')
  }
  return html
})

// 深色主题下邮件里写死的颜色要逐元素映射，和阅读区同一套规则。
onMounted(() => {
  nextTick(() => applyBodyTheme(bodyRef.value?.parentElement, props.themeMode === 'light' ? 'light' : 'dark', { selector: '.mail-body' }))
})
</script>

<style scoped>
.mail-eml-modal {
  height: min(820px, 92vh);
  width: min(1040px, 94vw);
}

.mail-eml-path {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--mail-text-muted);
  font-size: 11.5px;
}
.mail-eml-modal .mail-modal-close {
  margin-left: 0;
}

.mail-eml-head {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 12px;
  padding-bottom: 10px;
  border-bottom: 1px solid var(--mail-border);
}
.mail-eml-row {
  display: flex;
  gap: 8px;
  font-size: 12.5px;
}
.mail-eml-key {
  flex: 0 0 46px;
  color: var(--mail-text-muted);
}
.mail-eml-value {
  flex: 1 1 auto;
  min-width: 0;
  color: var(--mail-text);
  word-break: break-word;
}

.mail-eml-attachments {
  margin-top: 14px;
  padding-top: 10px;
  border-top: 1px solid var(--mail-border);
}
.mail-eml-attachments .mail-attachment {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin: 6px 6px 0 0;
  cursor: default;
}
</style>
