<template>
  <div class="mail-compose-window">
    <div v-if="loading" class="mail-compose-window-state">
      <span class="mail-spin"></span>
    </div>
    <div v-else-if="error" class="mail-compose-window-state is-error">
      <p>{{ error }}</p>
      <button class="mail-btn" @click="closeWindow({ save: false })">关闭</button>
    </div>
    <MailComposer
      v-else-if="draft"
      :draft="draft"
      :accounts="accounts"
      :settings="settings"
      :theme-mode="themeMode"
      :undo-seconds="Number(settings.undoSendSeconds) || 0"
      :templates="templates"
      :attachment-checks="attachmentChecks"
      @update="patchDraft"
      @send="onSend"
      @save-draft="onSaveDraft"
      @schedule="onSchedule"
      @apply-template="applyTemplate"
      @check-attachments="onCheckAttachments"
      @close="onClose"
    />
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import MailComposer from './MailComposer.vue'
import {
  accounts as windowAccounts,
  checkAttachments,
  closeWindow,
  composeDraft as windowDraft,
  error,
  loadComposeWindow,
  loading,
  patchDraft,
  saveDraft,
  sendDraft,
  settings as windowSettings,
  templates as windowTemplates,
} from './composeWindowBridge'

// 独立写信窗口的壳：数据从 composeWindowBridge 读，写完关窗。
const props = defineProps({
  label: { type: String, default: '' },
  themeMode: { type: String, default: 'dark' },
})

const accounts = windowAccounts
const settings = windowSettings
const templates = windowTemplates
const draft = windowDraft
const attachmentChecks = ref([])

const notify = (text) => {
  // 窗口很小，用一个轻量提示条即可（不引入主窗口的 toast 机制）。
  console.warn('[mail-compose]', text)
  window.dispatchEvent(new CustomEvent('mail-compose-notice', { detail: text }))
}

async function onSend({ useOutbox }) {
  try {
    const result = await sendDraft({ useOutbox })
    if (result?.queued || result?.sent) {
      await closeWindow({ save: false })
    }
  } catch (err) {
    notify(String(err?.message || err))
  }
}

async function onSchedule(at) {
  try {
    await sendDraft({ scheduledAt: at })
    await closeWindow({ save: false })
  } catch (err) {
    notify(String(err?.message || err))
  }
}

async function onSaveDraft() {
  try {
    const saved = await saveDraft()
    notify(saved ? '草稿已保存' : '没有内容可保存')
  } catch (err) {
    notify(String(err?.message || err))
  }
}

async function onCheckAttachments(paths) {
  attachmentChecks.value = await checkAttachments(paths)
}

function applyTemplate(template) {
  if (!draft.value || !template) return
  if (template.subject) draft.value.subject = template.subject
  if (template.html) {
    draft.value.html = template.html
    draft.value.htmlMode = true
  } else if (template.text) {
    draft.value.html = ''
    draft.value.htmlMode = false
    draft.value.text = template.text
  }
}

/** 关闭：先存草稿再关窗。 */
function onClose() {
  closeWindow({ save: true })
}

onMounted(() => {
  loadComposeWindow(props.label)
  // 窗口被系统关掉（Alt+F4 / 点 ×）时同样要存草稿：
  // beforeunload 里发 IPC 不保证送达，所以这里用「关闭前先存」的主动路径——
  // 用户在窗口内点关闭走 onClose；系统级关闭由主窗口的 closed 事件兜底。
  window.addEventListener('beforeunload', onBeforeUnload)
})

function onBeforeUnload(event) {
  if (!windowDraft.value) return
  // 有未保存内容时阻止直接丢弃：让 closeWindow 先写库。
  event.preventDefault()
  event.returnValue = ''
}

onBeforeUnmount(() => {
  window.removeEventListener('beforeunload', onBeforeUnload)
})
</script>

<style scoped>
.mail-compose-window {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--mail-bg);
}
.mail-compose-window-state {
  margin: auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  color: var(--mail-text-muted);
  font-size: 12.5px;
  text-align: center;
  max-width: 320px;
}
.mail-compose-window-state.is-error p {
  margin: 0;
  line-height: 1.7;
}
</style>
