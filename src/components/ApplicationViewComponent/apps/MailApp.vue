<template>
  <div
    class="mail-root"
    :class="[`mail-density-${settings.density || 'comfortable'}`]"
    :style="rootStyle"
  >
    <!-- 首次使用：没有任何账户时整页让位给引导，而不是显示一个空三栏 -->
    <div v-if="!store.loading && !store.hasAccounts" class="mail-onboard">
      <div class="mail-onboard-card">
        <div class="mail-onboard-mark">✉</div>
        <h1>把邮箱接进来</h1>
        <p>
          支持 IMAP / SMTP 的邮箱都可以添加，包括 QQ、163、Gmail、Outlook
          与企业邮箱。口令只存在系统凭据库里，不会随数据库备份外流。
        </p>
        <div class="mail-onboard-actions">
          <button class="mail-btn is-primary" @click="openAccounts">添加账户</button>
          <button class="mail-btn" @click="openSettings">外观与设置</button>
        </div>
        <p class="mail-onboard-hint">
          不确定服务器地址？添加账户的窗口里有常见邮箱的一键填充。
        </p>
      </div>
    </div>

    <template v-else>
      <MailToolbar
        :syncing="store.syncing"
        :last-sync-at="store.lastSyncAt"
        :unread="store.totalUnread"
        @sync="onSync"
        @refresh="onRefreshFolders"
        @compose="onCompose"
        @accounts="openAccounts"
        @contacts="showContacts = true"
        @rules="showRules = true"
        @settings="openSettings"
      />

      <div class="mail-shell">
        <MailSidebar
          :accounts="store.accounts"
          :folders-by-account="store.foldersByAccount"
          :active-account-id="store.activeAccountId"
          :active-folder="store.activeFolder"
          :active-label="store.activeLabel"
          :labels="store.labels"
          :scope="store.scope"
          :unread="store.unread"
          @select-account="store.selectAccount"
          @select-folder="store.selectFolder"
          @select-scope="store.selectScope"
          @compose="onCompose"
          @add-account="openAccounts"
        />

        <MailListPane
          :messages="store.messages"
          :selected="store.selected"
          :title="listTitle"
          :loading="store.loading"
          :search-query="store.searchQuery"
          :search-field="store.searchField"
          :filters="store.searchFilters"
          @update:search-query="onSearchInput"
          @update:search-field="(v) => (store.searchField = v)"
          @search="store.runSearch"
          @clear-search="store.clearSearch"
          @toggle-filter="onToggleFilter"
          @open="onOpenMessage"
          @toggle-star="store.setFlagged"
          @contextmenu="onRowContextMenu"
        />

        <MailReader
          v-if="store.detail || store.detailLoading"
          :detail="store.detail"
          :loading="store.detailLoading"
          :settings="settings"
          :theme-mode="store.theme?.mode || 'dark'"
          :mono-font="settings.monoFont"
          @reply="(m) => onReply(m)"
          @reply-all="(m) => onReply(m, true)"
          @forward="onForward"
          @delete="onDeleteCurrent"
          @archive="onArchiveCurrent"
          @toggle-flag="onToggleFlagCurrent"
          @toggle-unread="onToggleUnreadCurrent"
          @save-attachment="onSaveAttachment"
          @close="closeReader"
        />
        <div v-else class="mail-pane mail-reader">
          <div class="mail-reader-placeholder">
            选中左侧的一封邮件开始阅读。<br />
            未读邮件带一条细色条，点开即标记为已读。
          </div>
        </div>
      </div>
    </template>

    <!-- 写信窗口 -->
    <MailComposer
      v-if="store.composing"
      :draft="store.composing"
      :accounts="store.accounts"
      :settings="settings"
      :theme-mode="store.theme?.mode || 'dark'"
      :undo-seconds="settings.undoSendSeconds"
      @update="(patch) => Object.assign(store.composing, patch)"
      @send="onSend"
      @save-draft="onSaveDraft"
      @close="store.closeCompose"
    />

    <!-- 账户管理 -->
    <MailAccountDialog
      v-if="showAccounts"
      :accounts="store.accounts"
      :settings="settings"
      :active-account-id="store.activeAccountId"
      @close="showAccounts = false"
      :test-fn="onTestAccount"
      :has-secret-fn="store.hasSecret"
      @saved="onAccountSaved"
      @deleted="onAccountDeleted"
      @select="onAccountSelected"
    />

    <!-- 通讯录 -->
    <ContactsDialog
      v-if="showContacts"
      @close="showContacts = false"
      @compose-to="onComposeTo"
      @open-message="onOpenContactMessage"
    />

    <!-- 规则管理 -->
    <MailRulesDialog
      v-if="showRules"
      :rules="store.rules"
      :accounts="store.accounts"
      :folders-by-account="store.foldersByAccount"
      @close="showRules = false"
      @save="store.saveRule"
      @delete="store.deleteRule"
    />

    <!-- 外观与设置 -->
    <MailSettingsDrawer
      v-if="showSettings"
      :settings="settings"
      :themes="themes"
      @close="showSettings = false"
      @update="onSettingsUpdate"
    />

    <!-- 右键菜单 -->
    <div
      v-if="contextMenu"
      class="mail-context"
      :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }"
      @click.stop
    >
      <button class="mail-context-item" @click="contextAction('markRead')">
        {{ contextMenu.row.seen ? '标为未读' : '标为已读' }}
      </button>
      <button class="mail-context-item" @click="contextAction('flag')">
        {{ contextMenu.row.flagged ? '取消星标' : '加星标' }}
      </button>
      <div class="mail-context-sep"></div>
      <button
        v-for="folder in contextFolders"
        :key="folder.name"
        class="mail-context-item"
        @click="contextAction('move', folder.name)"
      >
        移到「{{ folder.displayName }}」
      </button>
      <div class="mail-context-sep"></div>
      <button class="mail-context-item is-danger" @click="contextAction('delete')">删除</button>
    </div>

    <!-- 底部提示 -->
    <div v-if="toast" class="mail-toast" :class="{ 'is-error': toast.error }">
      {{ toast.text }}
    </div>

    <!-- 待发倒计时（撤回窗口） -->
    <div v-if="pendingSend" class="mail-pending">
      <span>邮件将在 {{ pendingSend.remaining }} 秒后发出</span>
      <button class="mail-btn" @click="undoSend">撤回</button>
      <button class="mail-btn is-primary" @click="sendNow">立即发送</button>
    </div>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useMailStore } from '../../../stores/mailStore'
import { densityMetrics, monoFontStack, readingFontStack, themeToStyle } from './mail/theme'
import MailToolbar from './mail/MailToolbar.vue'
import MailSidebar from './mail/MailSidebar.vue'
import MailListPane from './mail/MailListPane.vue'
import MailReader from './mail/MailReader.vue'
import MailComposer from './mail/MailComposer.vue'
import MailAccountDialog from './mail/MailAccountDialog.vue'
import MailRulesDialog from './mail/MailRulesDialog.vue'
import ContactsDialog from './mail/ContactsDialog.vue'
import MailSettingsDrawer from './mail/MailSettingsDrawer.vue'

const store = useMailStore()

const showAccounts = ref(false)
const showRules = ref(false)
const showContacts = ref(false)
const showSettings = ref(false)
const toast = ref(null)
const contextMenu = ref(null)
const pendingSend = ref(null)
let toastTimer = null
let pendingTimer = null

const settings = computed(() => store.settings)
const themes = computed(() => [...store.builtinThemes, ...(settings.value.customThemes || [])])

const rootStyle = computed(() => {
  const metrics = densityMetrics(settings.value.density, settings.value.readingSize)
  return {
    ...themeToStyle(store.theme, settings.value.accentOverride),
    '--mail-font-reading': readingFontStack(settings.value.readingFont),
    '--mail-font-mono': monoFontStack(settings.value.monoFont),
    '--mail-reading-size': metrics.readingFontSize,
    '--mail-row-height': metrics.rowHeight,
    '--mail-gap': metrics.gap,
    '--mail-pad': metrics.pad,
  }
})

const listTitle = computed(() => {
  if (store.activeLabel) return `分类：${store.activeLabel}`
  if (store.searchQuery.trim()) return '搜索结果'
  if (store.scope === 'unread') return '未读'
  if (store.scope === 'flagged') return '已加星标'
  if (store.scope === 'attachments') return '带附件的邮件'
  if (store.scope === 'all') return '全部邮件'
  return store.currentFolder?.displayName || '收件箱'
})

const contextFolders = computed(() => {
  const accountId = contextMenu.value?.row?.accountId || store.activeAccountId
  return (store.foldersByAccount[accountId] || []).filter((f) => f.selectable)
})

function notify(text, error = false) {
  toast.value = { text, error }
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (toast.value = null), error ? 5200 : 2600)
}

// ---------------- 生命周期 ----------------

onMounted(async () => {
  await store.init()
  await store.loadOutbox()
  if (window.location.search.includes('setup=1')) showAccounts.value = true
})

onBeforeUnmount(() => {
  store.dispose()
  clearTimeout(toastTimer)
  clearInterval(pendingTimer)
  document.removeEventListener('click', closeContextMenu)
})

// ---------------- 操作 ----------------

async function onSync() {
  const result = await store.syncNow()
  if (result.errors?.length) notify(result.errors[0], true)
  else if (result.added) notify(`收到 ${result.added} 封新邮件`)
  else notify('已是最新')
}

async function onRefreshFolders() {
  await store.refreshFolders()
  await store.loadMessages({ silent: true })
  notify('文件夹已刷新')
}

async function onOpenMessage(row) {
  closeContextMenu()
  // 聚合行：先展开整个会话的列表，再打开最新那封，用户点一下就能看到来龙去脉。
  if (row?.threadUids?.length > 1) {
    await store.loadThread(row)
    const newest = row.threadUids.reduce((a, b) => (b.date > a.date ? b : a))
    await store.openMessage(newest)
    return
  }
  await store.openMessage(row)
}

/** 点开会话聚合行时，先展开整个会话。 */
async function expandThread(row) {
  if (row?.threadUids?.length > 1) await store.loadThread(row)
}

function onCompose() {
  store.startCompose('new')
}

function onReply(detail, all = false) {
  const source = {
    fromAddr: detail.fromAddr,
    fromName: detail.fromName,
    subject: detail.subject,
    toJson: JSON.stringify(detail.to || []),
    ccJson: JSON.stringify(detail.cc || []),
    messageId: detail.messageId,
    // References 必须是「原有引用链 + 本条 Message-Id」：只放本条会让多数客户端
    // 把回复挂到错误的分支上，会话在收信端就断了。
    references: buildReferences(detail),
  }
  store.startCompose(all ? 'replyAll' : 'reply', source)
  // 引用原文：回复里带上缩进的正文，收件端才有上下文。
  if (store.composing) {
    const quote = buildQuote(detail)
    store.composing.text = `\n\n${quote.text}`
    store.composing.html = `<br><br>${quote.html}`
  }
}

/** 拼 References：原链去重后追加本封的 Message-Id。 */
function buildReferences(detail) {
  const refs = String(detail.references || '')
    .split(/\s+/)
    .map((s) => s.trim())
    .filter(Boolean)
  const id = (detail.messageId || '').trim()
  if (id && !refs.includes(id)) refs.push(id)
  return refs.join(' ')
}

function onForward(detail) {
  store.startCompose('forward', {
    fromAddr: detail.fromAddr,
    fromName: detail.fromName,
    subject: detail.subject,
    toJson: '[]',
    ccJson: '[]',
    messageId: null,
  })
}

function buildQuote(detail) {
  const when = detail.date
    ? new Date(detail.date * 1000).toLocaleString('zh-CN', { hour12: false })
    : ''
  const who = detail.fromName ? `${detail.fromName} <${detail.fromAddr}>` : detail.fromAddr
  const textBody = `在 ${when}，${who} 写道：\n${detail.text || ''}`
  const htmlBody = `<blockquote>在 ${when}，${escapeHtml(who)} 写道：<br>${
    detail.html || escapeHtml(detail.text || '').replace(/\n/g, '<br>')
  }</blockquote>`
  return { text: textBody, html: htmlBody }
}

function escapeHtml(text) {
  return String(text).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
}

async function onDeleteCurrent(detail) {
  const row = store.selected || {
    accountId: detail.accountId,
    folder: detail.folder,
    uid: detail.uid,
  }
  try {
    await store.deleteMessages([row])
    notify('已删除')
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

async function onArchiveCurrent(detail) {
  const archive = (store.foldersByAccount[detail.accountId] || []).find((f) => f.role === 'archive')
  if (!archive) {
    notify('该账户没有归档文件夹，可以先在文件夹列表里新建', true)
    return
  }
  try {
    await store.moveMessages([store.selected], archive.name)
    notify(`已归档到「${archive.displayName}」`)
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

async function onToggleFlagCurrent(detail) {
  const row = store.selected || { accountId: detail.accountId, folder: detail.folder, uid: detail.uid }
  await store.setFlagged(row, !detail.flagged)
}

async function onToggleUnreadCurrent(detail) {
  const row = store.selected || {
    accountId: detail.accountId,
    folder: detail.folder,
    uid: detail.uid,
  }
  await store.setSeen([row], !detail.seen)
  detail.seen = !detail.seen
}

async function onSaveAttachment({ attachment }) {
  try {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const dir = await save({ defaultPath: attachment.filename })
    if (!dir) return
    // 用户可能选了一个新名字：拆出目录与文件名。
    const parts = dir.replace(/\\/g, '/').split('/')
    const filename = parts.pop()
    const path = await store.saveAttachment(
      store.detail,
      { ...attachment, filename },
      parts.join('/'),
    )
    notify(`已保存到 ${path}`)
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

function closeReader() {
  store.selected = null
  store.detail = null
}

// ---------------- 搜索 ----------------

function onSearchInput(value) {
  store.setSearch(value)
  // 输入即搜：本地库是 SQLite + FTS，几百毫秒内就能出结果，不需要「回车才搜」。
  clearTimeout(onSearchInput.timer)
  onSearchInput.timer = setTimeout(() => store.runSearch(), 180)
}

function onToggleFilter(key) {
  store.searchFilters = { ...store.searchFilters, [key]: !store.searchFilters[key] }
  store.runSearch()
}

// ---------------- 右键菜单 ----------------

function onRowContextMenu({ row, event }) {
  const width = 200
  const x = Math.min(event.clientX, window.innerWidth - width - 8)
  const y = Math.min(event.clientY, window.innerHeight - 220)
  contextMenu.value = { row, x, y }
  setTimeout(() => document.addEventListener('click', closeContextMenu, { once: true }), 0)
}

function closeContextMenu() {
  contextMenu.value = null
}

async function contextAction(action, target) {
  const row = contextMenu.value?.row
  closeContextMenu()
  if (!row) return
  try {
    if (action === 'markRead') await store.setSeen([row], !row.seen)
    if (action === 'flag') await store.setFlagged(row, !row.flagged)
    if (action === 'move') await store.moveMessages([row], target)
    if (action === 'delete') await store.deleteMessages([row])
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

// ---------------- 发送 ----------------

async function onSend({ useOutbox }) {
  try {
    if (useOutbox) {
      const result = await store.sendCompose({ useOutbox: true })
      if (result?.queued) {
        startPendingCountdown(result.queued)
      }
      return
    }
    await store.sendCompose({ useOutbox: false })
    notify('已发送')
    store.loadFolders(store.activeAccountId).catch(() => {})
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

function startPendingCountdown(item) {
  clearInterval(pendingTimer)
  const due = item.dueAt * 1000
  pendingSend.value = { ...item, remaining: Math.max(0, Math.round((due - Date.now()) / 1000)) }
  pendingTimer = setInterval(async () => {
    if (!pendingSend.value) {
      clearInterval(pendingTimer)
      return
    }
    const left = Math.max(0, Math.round((due - Date.now()) / 1000))
    pendingSend.value.remaining = left
    if (left <= 0) {
      clearInterval(pendingTimer)
      pendingSend.value = null
      const result = await store.flushOutbox()
      if (result?.errors?.length) notify(result.errors[0], true)
      else notify('已发送')
    }
  }, 1000)
}

async function undoSend() {
  const item = pendingSend.value
  clearInterval(pendingTimer)
  pendingSend.value = null
  if (!item) return
  try {
    await store.cancelOutbox(item.id)
    notify('已撤回，邮件没有发出')
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

async function sendNow() {
  clearInterval(pendingTimer)
  pendingSend.value = null
  const result = await store.flushOutbox()
  if (result?.errors?.length) notify(result.errors[0], true)
  else notify('已发送')
}

async function onSaveDraft() {
  try {
    await store.saveDraft()
    notify('草稿已保存')
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

// ---------------- 窗口内弹层 ----------------

function openAccounts() {
  showAccounts.value = true
}

function openSettings() {
  showSettings.value = true
}

async function onAccountSaved({ account, secret }) {
  try {
    await store.saveAccount(account, secret)
    showAccounts.value = false
    notify('账户已保存')
    const result = await store.syncNow()
    if (result.errors?.length) notify(result.errors[0], true)
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

async function onAccountDeleted(id) {
  await store.deleteAccount(id)
  notify('账户已删除')
}

async function onTestAccount(account, secret) {
  return store.testAccount(account, secret)
}

async function onAccountSelected(id) {
  store.selectAccount(id)
  showAccounts.value = false
}

async function onSettingsUpdate(patch) {
  try {
    await store.saveSettings(patch)
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

defineExpose({ expandThread })
</script>

<style>
/* 全局块：上下文菜单、toast、待发条都 teleport 到 body 之外的本组件里，
   但仍需要这些类名在全局生效（scoped 会加属性选择器，动态创建的节点也能匹配，
   但为与 mail.css 保持一致，这里只用全局类名，不加 scoped）。 */
@import './mail/mail.css';

.mail-context {
  position: fixed;
  z-index: 60;
  min-width: 190px;
  padding: 4px;
  border: 1px solid var(--mail-border-strong);
  border-radius: 8px;
  background: var(--mail-panel);
  box-shadow: var(--mail-shadow);
}

.mail-context-item {
  display: block;
  width: 100%;
  padding: 6px 10px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--mail-text-secondary);
  font: inherit;
  font-size: 12.5px;
  text-align: left;
  cursor: pointer;
}
.mail-context-item:hover {
  background: var(--mail-panel-hover);
  color: var(--mail-text);
}
.mail-context-item.is-danger:hover {
  color: var(--mail-danger);
}
.mail-context-sep {
  height: 1px;
  margin: 4px 6px;
  background: var(--mail-border);
}

.mail-toast {
  position: fixed;
  left: 50%;
  bottom: 24px;
  transform: translateX(-50%);
  z-index: 70;
  max-width: 70vw;
  padding: 8px 16px;
  border-radius: 999px;
  background: var(--mail-text);
  color: var(--mail-bg);
  font-size: 12.5px;
  box-shadow: var(--mail-shadow);
}
.mail-toast.is-error {
  background: var(--mail-danger);
  color: #fff;
}

.mail-pending {
  position: fixed;
  left: 50%;
  bottom: 72px;
  transform: translateX(-50%);
  z-index: 70;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px 8px 16px;
  border: 1px solid var(--mail-border-strong);
  border-radius: 999px;
  background: var(--mail-panel);
  color: var(--mail-text-secondary);
  font-size: 12.5px;
  box-shadow: var(--mail-shadow);
}

/* 首次使用引导 */
.mail-onboard {
  flex: 1 1 auto;
  display: grid;
  place-items: center;
  padding: 32px;
}

.mail-onboard-card {
  max-width: 460px;
  text-align: left;
  padding: 32px;
  border: 1px solid var(--mail-border);
  border-radius: 16px;
  background: var(--mail-panel);
  box-shadow: var(--mail-shadow);
}
.mail-onboard-mark {
  width: 40px;
  height: 40px;
  border-radius: 12px;
  display: grid;
  place-items: center;
  background: var(--mail-accent);
  color: var(--mail-accent-text);
  font-size: 20px;
  margin-bottom: 16px;
}
.mail-onboard-card h1 {
  font-size: 21px;
  font-weight: 650;
  margin: 0 0 10px;
  letter-spacing: -0.01em;
}
.mail-onboard-card p {
  margin: 0 0 16px;
  color: var(--mail-text-secondary);
  font-size: 13px;
  line-height: 1.75;
}
.mail-onboard-actions {
  display: flex;
  gap: 8px;
  margin-bottom: 14px;
}
.mail-onboard-hint {
  font-size: 11.5px !important;
  color: var(--mail-text-muted) !important;
  margin: 0 !important;
}
</style>
