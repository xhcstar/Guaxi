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
      :save-mailbox-draft="saveDraftToMailbox"
      embedded
      @update="patchDraft"
      @send="onSend"
      @save-draft="onSaveDraft"
      @discard="onDiscard"
      @close="onSaveAndClose"
      @schedule="onSchedule"
      @apply-template="applyTemplate"
      @check-attachments="onCheckAttachments"
    />

    <!-- 窗口里没有主界面那套 toast：草稿有没有存上、存到了服务器还是只在本机，
         必须在这里说一句，否则用户只能靠猜（关掉窗口发现草稿箱是空的，就会以为丢了）。 -->
    <div v-if="notice" class="mail-compose-notice" :class="{ 'is-error': notice.error }">
      {{ notice.text }}
    </div>

    <!-- 独立窗口没有主窗口那套 MailConfirmHost：写信时的大组确认 / 定时时间提示
         如果继续用 window.confirm/alert，在这个 webview 里会被静默吞掉。 -->
    <MailConfirmHost />
  </div>
</template>

<script setup>
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import MailComposer from './MailComposer.vue'
import MailConfirmHost from './MailConfirmHost.vue'
import {
  accounts as windowAccounts,
  checkAttachments,
  closeWindow,
  composeDraft as windowDraft,
  discardWindow,
  error,
  loadComposeWindow,
  loading,
  patchDraft,
  saveDraft,
  saveDraftToMailbox,
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
/** 窗口内的轻量提示：草稿存上了 / 只存到本机 / 存失败，都要说一句。 */
const notice = ref(null)
let noticeTimer = null

function showNotice(text, error = false) {
  if (!text) return
  notice.value = { text, error }
  clearTimeout(noticeTimer)
  noticeTimer = setTimeout(() => (notice.value = null), error ? 5200 : 2600)
}

const notify = (text, error = false) => {
  console.warn('[mail-compose]', text)
  showNotice(text, error)
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
    await saveDraft()
    // 空草稿也会存：用户要的是「还能接着写」，不是「必须有内容」。
    notify('草稿已保存')
  } catch (err) {
    notify(String(err?.message || err), true)
  }
}

/** 编辑器里点了「直接放弃」：不存草稿，并把自动保存进草稿箱的那份删掉再关窗。 */
async function onDiscard() {
  await discardWindow()
}

/** 编辑器里点了「存为草稿并关闭」：走正常的关窗流程（先存再关）。 */
async function onSaveAndClose() {
  await closeWindow({ save: true })
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
  clearTimeout(noticeTimer)
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
.mail-compose-notice {
  position: fixed;
  left: 50%;
  bottom: 22px;
  transform: translateX(-50%);
  z-index: 30;
  max-width: 70vw;
  padding: 8px 16px;
  border-radius: 999px;
  background: var(--mail-text);
  color: var(--mail-bg);
  font-size: 12.5px;
  box-shadow: var(--mail-shadow);
}
.mail-compose-notice.is-error {
  background: var(--mail-danger);
  color: #fff;
}
</style>
