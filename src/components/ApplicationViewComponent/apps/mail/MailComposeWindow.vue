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
      embedded
      @update="patchDraft"
      @send="onSend"
      @save-draft="onSaveDraft"
      @schedule="onSchedule"
      @apply-template="applyTemplate"
      @check-attachments="onCheckAttachments"
    />
  </div>
</template>

<script setup>
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
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

let unlistenClose = null
let closing = false

onMounted(() => {
  loadComposeWindow(props.label)
  // 独立窗口没有内部 × 了，系统标题栏的关闭是唯一出口：拦下来先存草稿再关。
  // 存完由 closeWindow 走 `mail_close_compose_window`，那一次 close 会再次进入
  // 本回调——`closing` 让它放行，否则窗口永远关不掉。
  getCurrentWindow()
    .onCloseRequested(async (event) => {
      if (closing || !windowDraft.value) return
      event.preventDefault()
      closing = true
      try {
        await closeWindow({ save: true })
      } catch (err) {
        closing = false
        notify(String(err?.message || err))
      }
    })
    .then((unlisten) => {
      unlistenClose = unlisten
    })
    .catch((err) => {
      console.warn('[mail-compose] 关闭拦截不可用', err)
    })
})

onBeforeUnmount(() => {
  unlistenClose?.()
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
