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
        :sidebar-collapsed="Boolean(settings.sidebarCollapsed)"
        @toggle-sidebar="toggleSidebar"
        @sync="onSync"
        @refresh="onRefreshFolders"
        @compose="onCompose"
        @accounts="openAccounts"
        @contacts="showContacts = true"
        @rules="showRules = true"
        @settings="openSettings"
      />

      <div
        class="mail-shell"
        :class="{
          'is-sidebar-collapsed': Boolean(settings.sidebarCollapsed),
          'is-list-collapsed': Boolean(settings.listCollapsed),
        }"
        :style="shellStyle"
      >
        <MailSidebar
          v-if="!settings.sidebarCollapsed"
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
          @create-folder="onCreateFolder"
          @delete-folder="onDeleteFolder"
          @settings="showSettings = true"
          @drop-on-folder="onDropOnFolder"
        />

        <!-- 栏边界：拖拽调宽；中间的小手柄点一下收起/展开。
             控制某一栏的开关就该长在那一栏的边上，而不是顶栏的图标。 -->
        <div
          v-if="!settings.sidebarCollapsed"
          class="mail-splitter"
          :class="{ 'is-dragging': dragging === 'sidebar' }"
          title="拖动调整宽度，双击收起"
          @mousedown.prevent="startDrag('sidebar', $event)"
          @dblclick="toggleSidebar"
        >
          <button
            class="mail-collapse-handle"
            title="收起账户与文件夹栏"
            aria-label="收起账户与文件夹栏"
            @mousedown.stop
            @click.stop="toggleSidebar"
          >
            ‹
          </button>
        </div>

        <!-- 列表收起后：左缘留一条常驻手柄，否则没有任何入口把它拉回来。 -->
        <div v-if="settings.listCollapsed" class="mail-expand-rail is-list">
          <button
            class="mail-collapse-handle is-reversed"
            title="展开邮件列表"
            aria-label="展开邮件列表"
            @click="toggleList"
          >
            ›
          </button>
        </div>

        <MailListPane
          v-if="!settings.listCollapsed"
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
          :selected-keys="store.selectedKeys"
          :search-error="store.searchError"
          :move-targets="moveTargets"
          @toggle-select="store.toggleSelect"
          @select-all="store.selectAll"
          @clear-selection="store.clearSelection"
          @batch="onBatch"
          @drag-start="onDragStart"
          @drag-end="onDragEnd"
        />

        <div
          v-if="!settings.listCollapsed"
          class="mail-splitter"
          :class="{ 'is-dragging': dragging === 'list' }"
          title="拖动调整宽度，双击收起"
          @mousedown.prevent="startDrag('list', $event)"
          @dblclick="toggleList"
        >
          <button
            class="mail-collapse-handle"
            title="收起邮件列表"
            aria-label="收起邮件列表"
            :disabled="!canCollapseList"
            @mousedown.stop
            @click.stop="toggleList"
          >
            ‹
          </button>
        </div>

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
          :close-label="settings.listCollapsed ? '收起邮件' : '返回列表'"
          @close="closeReader"
        />
        <div v-else-if="!settings.listCollapsed" class="mail-pane mail-reader">
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
      <div v-if="contextCount > 1" class="mail-context-head">已选 {{ contextCount }} 封</div>
      <button class="mail-context-item" @click="contextAction('markRead')">
        {{ contextMenu.row.seen ? '标为未读' : '标为已读' }}
      </button>
      <button class="mail-context-item" @click="contextAction('flag')">
        {{ contextMenu.row.flagged ? '取消星标' : '加星标' }}
      </button>
      <button class="mail-context-item" @click="contextAction('junk')">标为垃圾邮件</button>
      <button v-if="contextInJunk" class="mail-context-item" @click="contextAction('unmarkJunk')">
        不是垃圾邮件
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
      <button class="mail-context-item" @click="contextAction('saveEml')">另存为 .eml（含附件）</button>
      <button class="mail-context-item" @click="contextAction('saveHtml')">另存为 .html（正文）</button>
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
import { invoke } from '@tauri-apps/api/core'
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

/** 没有打开邮件时不允许收起列表，否则会得到一片空白。 */
const canCollapseList = computed(() => Boolean(store.detail || store.detailLoading))

const dragging = ref(null)
let dragStartX = 0
let dragStartWidth = 0

const SIDEBAR_MIN = 180
const SIDEBAR_MAX = 360
const LIST_MIN = 260
const LIST_MAX = 620

const sidebarWidth = computed(() => {
  const value = Number(settings.value.sidebarWidth) || 232
  return Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, value))
})

const listWidth = computed(() => {
  const value = Number(settings.value.listWidth) || 340
  return Math.min(LIST_MAX, Math.max(LIST_MIN, value))
})

const shellStyle = computed(() => ({
  '--mail-sidebar-w': `${sidebarWidth.value}px`,
  '--mail-list-w': `${listWidth.value}px`,
}))

function startDrag(which, event) {
  dragging.value = which
  dragStartX = event.clientX
  dragStartWidth = which === 'sidebar' ? sidebarWidth.value : listWidth.value
  document.addEventListener('mousemove', onDragMove)
  document.addEventListener('mouseup', endDrag)
  document.body.style.cursor = 'col-resize'
  // 拖拽期间禁止选中文本，否则整页会被刷蓝。
  document.body.style.userSelect = 'none'
}

function onDragMove(event) {
  if (!dragging.value) return
  const delta = event.clientX - dragStartX
  if (dragging.value === 'sidebar') {
    store.settings.sidebarWidth = clamp(dragStartWidth + delta, SIDEBAR_MIN, SIDEBAR_MAX)
  } else {
    store.settings.listWidth = clamp(dragStartWidth + delta, LIST_MIN, LIST_MAX)
  }
}

function endDrag() {
  if (!dragging.value) return
  const which = dragging.value
  dragging.value = null
  document.removeEventListener('mousemove', onDragMove)
  document.removeEventListener('mouseup', endDrag)
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
  // 拖完才落库：拖动过程中每帧写设置会把 sqlite 打满。
  const patch =
    which === 'sidebar'
      ? { sidebarWidth: store.settings.sidebarWidth }
      : { listWidth: store.settings.listWidth }
  store.saveSettings(patch).catch(() => {})
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, Math.round(value)))
}

/** 收起左侧账户栏：只影响布局，不改变当前选中项。 */
function toggleSidebar() {
  store.saveSettings({ sidebarCollapsed: !settings.value.sidebarCollapsed }).catch(() => {})
}

/** 收起邮件列表：只有已经打开一封邮件时才允许，否则会得到一片空白。 */
function toggleList() {
  const next = !settings.value.listCollapsed
  if (next && !store.detail) {
    notify('先打开一封邮件，再收起列表', true)
    return
  }
  store.saveSettings({ listCollapsed: next }).catch(() => {})
}

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
  document.removeEventListener('mousemove', onDragMove)
  document.removeEventListener('mouseup', endDrag)
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

async function onCreateFolder(name) {
  try {
    await store.createFolder(name)
    notify(`已创建「${name}」`)
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

async function onDeleteFolder(folder) {
  if (folder.role !== 'custom') {
    notify('系统文件夹不能删除', true)
    return
  }
  if (
    !window.confirm(
      `删除文件夹「${folder.displayName}」？

服务器上该文件夹及其中的邮件会一并删除，无法撤销。`,
    )
  ) {
    return
  }
  try {
    await store.deleteFolder(folder.name)
    notify(`已删除「${folder.displayName}」`)
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

/** 另存为：.eml（原始报文，含附件）或 .html（正文）。 */
async function saveMessageAs(row, kind) {
  const { save } = await import('@tauri-apps/plugin-dialog')
  // 文件名里不能出现路径分隔符与 Windows 保留字符，否则保存对话框会直接失败。
  // 用字符码构造，避免多层字符串转义把反斜杠吃掉（这里踩过一次）。
  const ILLEGAL = new Set([
    String.fromCharCode(92), '/', ':', '*', '?', '"', '<', '>', '|',
    String.fromCharCode(13), String.fromCharCode(10),
  ])
  const subject = [...(row.subject || '邮件')]
    .map((ch) => (ILLEGAL.has(ch) ? '_' : ch))
    .join('')
    .slice(0, 80)
  const path = await save({
    title: kind === 'eml' ? '另存为 .eml' : '另存为 .html',
    defaultPath: `${subject}.${kind}`,
    filters: [
      {
        name: kind === 'eml' ? '邮件报文' : '网页',
        extensions: [kind],
      },
    ],
  })
  if (!path) return
  if (kind === 'eml') {
    await invoke('mail_save_as_eml', {
      accountId: row.accountId,
      folder: row.folder,
      uid: row.uid,
      targetPath: path,
    })
  } else {
    await invoke('mail_save_as_html', {
      accountId: row.accountId,
      folder: row.folder,
      uid: row.uid,
      targetPath: path,
    })
  }
  notify(`已保存到 ${path}`)
}

async function onRefreshFolders() {
  await store.refreshFolders()
  await store.loadMessages({ silent: true })
  notify('文件夹已刷新')
}

async function onOpenMessage(row) {
  closeContextMenu()
  if (settings.value.listCollapsed) {
    store.saveSettings({ listCollapsed: false }).catch(() => {})
  }
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

  // 右键菜单作用于「当前选择集」：单选时就是那一封，多选时整批。
  // Outlook 也是这个行为——否则右键一堆选中的邮件却只处理一封，很容易误操作。
  const rows = store.selectedRows().length ? store.selectedRows() : [row]

  try {
    if (action === 'saveEml' || action === 'saveHtml') {
      await saveMessageAs(row, action === 'saveEml' ? 'eml' : 'html')
      return
    }
    if (action === 'markRead') {
      await store.runBatch(row.seen ? 'unread' : 'read')
      return
    }
    if (action === 'flag') {
      await store.runBatch(row.flagged ? 'unflag' : 'flag')
      return
    }
    if (action === 'junk') {
      await store.runBatch('junk')
      notify(`已移到垃圾邮件（${rows.length} 封）`)
      return
    }
    if (action === 'unmarkJunk') {
      await store.runBatch('unmarkJunk')
      notify(`已移回收件箱（${rows.length} 封）`)
      return
    }
    if (action === 'move' && target) {
      await store.runBatch('move', target)
      notify(`已移动 ${rows.length} 封`)
      return
    }
    if (action === 'delete') {
      await store.runBatch('delete')
      notify(`已删除 ${rows.length} 封`)
    }
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

const contextCount = computed(() => store.selectedKeys.length || 1)
const contextInJunk = computed(() => {
  const row = contextMenu.value?.row
  if (!row) return false
  const list = store.foldersByAccount[row.accountId] || []
  return list.some((f) => f.name === row.folder && f.role === 'junk')
})

/** 可拖放到 / 批量移动到哪些文件夹。 */
const moveTargets = computed(() => {
  const accountId = store.activeAccountId
  return (store.foldersByAccount[accountId] || []).filter((f) => f.selectable)
})

let draggingRows = []

function onDragStart(row) {
  draggingRows = store.selectedRows().length ? store.selectedRows() : [row]
}

function onDragEnd() {
  draggingRows = []
}

/** 列表动作条发出的批量命令。 */
async function onBatch(action, target) {
  try {
    await store.runBatch(action, target)
    if (action === 'junk') notify('已移到垃圾邮件')
    if (action === 'delete') notify('已删除')
    if (action === 'move') notify('已移动')
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

/** 拖到侧栏文件夹上：把正在拖的那批邮件移过去。 */
async function onDropOnFolder(folderName) {
  const source = store.selectedRows()
  const rows = draggingRows.length ? draggingRows : source
  draggingRows = []
  if (!rows.length) return
  // 拖到源文件夹本身没有意义。
  if (rows.every((r) => r.folder === folderName)) return
  try {
    await store.runBatch('move', folderName)
    notify(`已移动 ${rows.length} 封到该文件夹`)
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
