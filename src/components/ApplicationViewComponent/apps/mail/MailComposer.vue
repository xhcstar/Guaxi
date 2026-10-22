<template>
  <div class="mail-modal-backdrop" @click.self="$emit('close')">
    <div class="mail-modal mail-composer" role="dialog" aria-label="写邮件">
      <div class="mail-modal-head">
        <h2 class="mail-modal-title">{{ title }}</h2>
        <select
          class="mail-select"
          :value="draft.accountId"
          title="发件账户"
          @change="$emit('update', { accountId: $event.target.value })"
        >
          <option v-for="account in accounts" :key="account.id" :value="account.id">
            {{ account.email }}
          </option>
        </select>
        <button class="mail-modal-close" title="关闭" @click="$emit('close')">×</button>
      </div>

      <div class="mail-compose-fields">
        <AddressInput
          :model-value="draft.to"
          label="收件人"
          placeholder="输入姓名或邮箱，回车确认"
          :suggest="suggestAddress"
          :expand-group="expandGroup"
          @update:model-value="$emit('update', { to: $event })"
        >
          <template #actions>
            <button class="mail-fmt-btn" title="抄送" @click="$emit('update', { showCc: !draft.showCc })">
              Cc
            </button>
            <button class="mail-fmt-btn" title="密送" @click="$emit('update', { showBcc: !draft.showBcc })">
              Bcc
            </button>
            <button
              class="mail-fmt-btn"
              title="从通讯录选择"
              :class="{ 'is-on': pickerOpen }"
              @click="pickerOpen = !pickerOpen"
            >
              通讯录
            </button>
          </template>
        </AddressInput>

        <AddressInput
          v-if="draft.showCc"
          :model-value="draft.cc"
          label="抄送"
          placeholder="抄送地址"
          :suggest="suggestAddress"
          :expand-group="expandGroup"
          @update:model-value="$emit('update', { cc: $event })"
        />
        <AddressInput
          v-if="draft.showBcc"
          :model-value="draft.bcc"
          label="密送"
          placeholder="密送地址"
          :suggest="suggestAddress"
          :expand-group="expandGroup"
          @update:model-value="$emit('update', { bcc: $event })"
        />
        <div class="mail-compose-line">
          <span class="key">主题</span>
          <input
            :value="draft.subject"
            placeholder="邮件主题"
            @input="$emit('update', { subject: $event.target.value })"
          />
        </div>
      </div>

      <AddressPicker
        v-if="pickerOpen"
        :exclude="[...draft.to, ...draft.cc, ...draft.bcc]"
        @pick="onPickAddress"
        @pick-group="onPickGroup"
        @close="pickerOpen = false"
      />

      <div class="mail-compose-toolbar">
        <template v-if="!sourceMode">
          <button class="mail-fmt-btn" title="加粗 (Ctrl+B)" @mousedown.prevent="format('bold')">
            <b>B</b>
          </button>
          <button class="mail-fmt-btn" title="斜体 (Ctrl+I)" @mousedown.prevent="format('italic')">
            <i>I</i>
          </button>
          <button class="mail-fmt-btn" title="下划线 (Ctrl+U)" @mousedown.prevent="format('underline')">
            <u>U</u>
          </button>
          <span class="mail-fmt-sep"></span>
          <button class="mail-fmt-btn" title="无序列表" @mousedown.prevent="format('insertUnorderedList')">
            •≡
          </button>
          <button class="mail-fmt-btn" title="有序列表" @mousedown.prevent="format('insertOrderedList')">
            1≡
          </button>
          <button class="mail-fmt-btn" title="引用" @mousedown.prevent="format('formatBlock', 'blockquote')">
            ❝
          </button>
          <span class="mail-fmt-sep"></span>
          <button class="mail-fmt-btn" title="插入链接" @mousedown.prevent="insertLink">
            ↗
          </button>
          <button class="mail-fmt-btn" title="插入代码块" @mousedown.prevent="insertCode">
            { }
          </button>
          <span class="mail-fmt-sep"></span>
        </template>

        <button
          class="mail-fmt-btn"
          :class="{ 'is-on': sourceMode }"
          title="切换 HTML 源码"
          @click="toggleSource"
        >
          源码
        </button>
        <button class="mail-fmt-btn" title="添加附件" @click="pickAttachments">📎 附件</button>

        <span style="margin-left: auto"></span>
        <button class="mail-btn" :disabled="sending" @click="$emit('save-draft')">保存草稿</button>
        <button class="mail-btn" :disabled="sending || !canSend" @click="send(false)">
          {{ sending ? '发送中…' : '发送' }}
        </button>
        <button
          v-if="undoSeconds > 0"
          class="mail-btn is-primary"
          :disabled="sending || !canSend"
          :title="`${undoSeconds} 秒内可以撤回`"
          @click="send(true)"
        >
          延迟 {{ undoSeconds }} 秒发送
        </button>
      </div>

      <div class="mail-compose-body">
        <div v-if="sourceMode" class="mail-compose-body">
          <textarea
            class="mail-compose-source"
            :value="htmlSource"
            spellcheck="false"
            @input="htmlSource = $event.target.value"
          ></textarea>
        </div>
        <div
          v-else
          ref="editorRef"
          class="mail-compose-editor"
          contenteditable="true"
          data-placeholder="写点什么…"
          @input="onEditorInput"
          @paste="onPaste"
          @keydown.ctrl.b.prevent="format('bold')"
          @keydown.ctrl.i.prevent="format('italic')"
          @keydown.ctrl.u.prevent="format('underline')"
          @keydown.ctrl.enter.prevent="send(false)"
        ></div>
      </div>

      <div v-if="draft.attachments.length" class="mail-attach-list">
        <button
          v-for="att in draft.attachments"
          :key="att.path"
          class="mail-attachment"
          :title="`移除 ${att.path}`"
          @click="removeAttachment(att.path)"
        >
          <span>📄</span>
          <span class="mail-attachment-name">{{ att.name }}</span>
          <span class="mail-attachment-size">×</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { useContactsStore } from '../../../../stores/contactsStore'
import AddressInput from './AddressInput.vue'
import AddressPicker from './AddressPicker.vue'
import { createLogger } from '../../../../utils/logger'

const logger = createLogger('MailComposer')

const props = defineProps({
  draft: { type: Object, required: true },
  accounts: { type: Array, default: () => [] },
  settings: { type: Object, default: () => ({}) },
  themeMode: { type: String, default: 'dark' },
  undoSeconds: { type: Number, default: 0 },
})
const emit = defineEmits(['update', 'send', 'save-draft', 'close'])

const editorRef = ref(null)
const pickerOpen = ref(false)
const contacts = useContactsStore()
const sourceMode = ref(false)
const htmlSource = ref(props.draft.html || '')
const sending = ref(false)
let lastHtml = props.draft.html || ''

const title = computed(
  () =>
    ({
      new: '新邮件',
      reply: '回复',
      replyAll: '全部回复',
      forward: '转发',
    })[props.draft.mode] || '新邮件',
)

const canSend = computed(
  () => props.draft.to.length || props.draft.cc.length || props.draft.bcc.length,
)

onMounted(async () => {
  if (!editorRef.value) return
  // 直接把已有 HTML 灌进 contenteditable：用 innerHTML 而不是 v-html，
  // 避免 Vue 与浏览器的编辑结果互相覆盖（contenteditable 里的 DOM 是浏览器在管）。
  editorRef.value.innerHTML = props.draft.html || ''
  lastHtml = props.draft.html || ''
  await nextTick()
  editorRef.value.focus()
})

// 从「源码模式」切回所见即所得时，把编辑区的 DOM 同步成最新的源码。
watch(sourceMode, async (on) => {
  if (!on) {
    await nextTick()
    if (editorRef.value) {
      editorRef.value.innerHTML = htmlSource.value
      lastHtml = htmlSource.value
    }
  } else {
    htmlSource.value = editorRef.value?.innerHTML || props.draft.html || ''
  }
})

function onEditorInput() {
  const html = editorRef.value?.innerHTML || ''
  if (html === lastHtml) return
  lastHtml = html
  emit('update', { html, text: htmlToText(html) })
}

function format(command, value = null) {
  if (sourceMode.value) return
  document.execCommand(command, false, value)
  onEditorInput()
  editorRef.value?.focus()
}

function insertLink() {
  const url = window.prompt('链接地址', 'https://')
  if (!url) return
  format('createLink', url)
}

function insertCode() {
  const selection = window.getSelection()
  if (!selection || !selection.rangeCount) return
  const code = window.prompt('代码内容')
  if (code == null) return
  const pre = document.createElement('pre')
  pre.textContent = code
  const range = selection.getRangeAt(0)
  range.deleteContents()
  range.insertNode(pre)
  onEditorInput()
}

function toggleSource() {
  sourceMode.value = !sourceMode.value
}

/** 粘贴时只保留纯文本与基本行内格式：外部网页的样式会毁掉整封邮件的排版。 */
function onPaste(event) {
  const html = event.clipboardData?.getData('text/html')
  if (html) {
    event.preventDefault()
    const cleaned = sanitizePastedHtml(html)
    document.execCommand('insertHTML', false, cleaned)
    onEditorInput()
    return
  }
  const text = event.clipboardData?.getData('text/plain')
  if (text != null) {
    event.preventDefault()
    document.execCommand('insertText', false, text)
    onEditorInput()
  }
}

/** 只保留白色名单里的标签；class/id/style 全部丢弃。 */
function sanitizePastedHtml(html) {
  const doc = new DOMParser().parseFromString(html, 'text/html')
  const allowed = new Set([
    'B', 'STRONG', 'I', 'EM', 'U', 'S', 'P', 'BR', 'DIV', 'SPAN', 'A', 'UL', 'OL', 'LI',
    'BLOCKQUOTE', 'PRE', 'CODE', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'TABLE', 'THEAD',
    'TBODY', 'TR', 'TD', 'TH', 'HR', 'IMG',
  ])
  const walk = (node) => {
    for (const child of [...node.childNodes]) {
      if (child.nodeType === 1) {
        if (!allowed.has(child.tagName)) {
          // 不认识的容器：把子节点提上来，保留文字。
          while (child.firstChild) node.insertBefore(child.firstChild, child)
          node.removeChild(child)
          continue
        }
        for (const attr of [...child.attributes]) {
          const name = attr.name.toLowerCase()
          if (name === 'href' && /^(https?:|mailto:|cid:)/i.test(attr.value)) continue
          if (child.tagName === 'IMG' && name === 'src' && /^(https?:|data:image\/|cid:)/i.test(attr.value)) {
            continue
          }
          child.removeAttribute(attr.name)
        }
        walk(child)
      }
    }
  }
  walk(doc.body)
  return doc.body.innerHTML
}

/** 地址栏补全：联系人与历史往来地址统一走 contacts store。 */
async function suggestAddress(prefix) {
  return contacts.suggest(prefix)
}

/** 点选通讯组：展开成成员地址追加到当前字段。 */
async function expandGroup(id) {
  return contacts.expandGroup(id)
}

function onPickAddress(address) {
  const value = String(address || '').trim()
  if (!value) return
  if (!draft.to.some((item) => sameAddress(item, value))) {
    emit('update', { to: [...draft.to, value] })
  }
}

async function onPickGroup(groupId) {
  const addresses = await contacts.expandGroup(groupId)
  const next = [...draft.to]
  for (const address of addresses || []) {
    if (!next.some((item) => sameAddress(item, address))) next.push(address)
  }
  emit('update', { to: next })
  pickerOpen.value = false
}

function sameAddress(a, b) {
  const norm = (value) => {
    const match = String(value || '').match(/<([^>]+)>/)
    return (match ? match[1] : value).trim().toLowerCase()
  }
  return norm(a) === norm(b)
}

async function pickAttachments() {
  try {
    const files = await open({ multiple: true, title: '选择附件' })
    if (!files) return
    const list = Array.isArray(files) ? files : [files]
    const next = [...props.draft.attachments]
    for (const path of list) {
      if (!path) continue
      if (next.some((a) => a.path === path)) continue
      const name = path.replace(/\\/g, '/').split('/').pop()
      next.push({ path, name })
    }
    emit('update', { attachments: next })
  } catch (error) {
    logger.warn('选择附件失败', error)
  }
}

function removeAttachment(path) {
  emit('update', { attachments: props.draft.attachments.filter((a) => a.path !== path) })
}

async function send(useOutbox) {
  if (sending.value) return
  // 把编辑区最新内容并入草稿（用户可能还没触发 input 事件）。
  if (!sourceMode.value && editorRef.value) {
    const html = editorRef.value.innerHTML
    emit('update', { html, text: htmlToText(html) })
  } else {
    emit('update', { html: htmlSource.value, text: htmlToText(htmlSource.value) })
  }
  sending.value = true
  try {
    await nextTick()
    emit('send', { useOutbox })
  } finally {
    sending.value = false
  }
}

function setList(key, value) {
  emit('update', { [key]: splitAddresses(value) })
}

function joinList(list) {
  return (list || []).join(', ')
}

/** 按逗号/分号切分，保留 `Name <addr>` 形态。 */
function splitAddresses(value) {
  return String(value || '')
    .split(/[,;，；]/)
    .map((s) => s.trim())
    .filter(Boolean)
}

/** 富文本转纯文本（发给不支持 HTML 的客户端 / 保存搜索索引用的那一份）。 */
function htmlToText(html) {
  const doc = new DOMParser().parseFromString(String(html || ''), 'text/html')
  doc.querySelectorAll('br').forEach((br) => br.replaceWith('\n'))
  doc.querySelectorAll('p,div,li,tr,h1,h2,h3,h4,h5,h6,blockquote,pre').forEach((el) => {
    el.append('\n')
  })
  return (doc.body.textContent || '').replace(/\n{3,}/g, '\n\n').trim()
}
</script>
