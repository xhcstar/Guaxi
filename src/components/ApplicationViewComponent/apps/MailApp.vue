<template>
  <div
    class="mail-root"
    :class="[
      `mail-density-${settings.density || 'comfortable'}`,
      { 'mail-doc-dark': store.theme?.mode !== 'light' },
    ]"
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
        :offline-queued="store.offlineQueued"
        :pending-send="store.pendingSend"
        :idle-active="store.idleActive"
        @toggle-sidebar="toggleSidebar"
        @sync="onSync"
        @refresh="onRefreshFolders"
        @compose="onCompose"
        @accounts="openAccounts"
        @contacts="showContacts = true"
        @rules="showRules = true"
        @settings="openSettings"
        @replay-offline="onReplayOffline"
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
          ref="listPaneRef"
          :messages="store.messages"
          :selected="store.selected"
          :active-key="activeKey"
          :title="listTitle"
          :loading="store.loading"
          :search-query="store.searchQuery"
          :search-field="store.searchField"
          :filters="store.searchFilters"
          :virtual="settings.virtualList !== false"
          :row-height="rowHeightPx"
          :has-more="store.hasMore"
          :loading-more="store.loadingMore"
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
          @load-more="store.loadMore"
          @delete="onDeleteRow"
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
          :thread="store.thread"
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
          @open-attachment="onOpenAttachment"
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
      :templates="store.templates"
      :attachment-checks="attachmentChecks"
      @update="(patch) => Object.assign(store.composing, patch)"
      @send="onSend"
      @save-draft="onSaveDraft"
      @apply-template="(tpl) => store.applyTemplate(tpl)"
      @check-attachments="onCheckAttachments"
      @close="store.closeCompose"
    />

    <!-- 账户管理 -->
    <MailAccountDialog
      v-if="showAccounts"
      :accounts="store.accounts"
      :settings="settings"
      :active-account-id="store.activeAccountId"
      :test-fn="onTestAccount"
      :has-secret-fn="store.hasSecret"
      :oauth-start-fn="store.startOAuth"
      :oauth-poll-fn="onOAuthPoll"
      :oauth-status-fn="store.oauthStatusFor"
      :oauth-complete-fn="onOAuthComplete"
      :save-fn="onSaveAccount"
      @close="showAccounts = false"
      @deleted="onAccountDeleted"
      @select="onAccountSelected"
    />

    <!-- 通讯录 -->
    <ContactsDialog
      v-if="showContacts"
      @close="showContacts = false"
      @compose-to="onComposeTo"
      @compose-group="onComposeGroup"
      @open-message="onOpenContactMessage"
    />

    <!-- 规则管理 -->
    <MailRulesDialog
      v-if="showRules"
      :rules="store.rules"
      :accounts="store.accounts"
      :folders-by-account="store.foldersByAccount"
      :sender-rules="store.senderRules"
      @close="showRules = false"
      @save="store.saveRule"
      @delete="store.deleteRule"
      @save-sender-rule="store.saveSenderRule"
      @delete-sender-rule="store.deleteSenderRule"
    />

    <!-- 外观与设置 -->
    <MailSettingsDrawer
      v-if="showSettings"
      :settings="settings"
      :themes="themes"
      :templates="store.templates"
      :accounts="store.accounts"
      :active-account-id="store.activeAccountId"
      :oauth-status-fn="store.oauthStatusFor"
      :sieve-preview-fn="previewSieve"
      :sieve-sync-fn="syncSieve"
      :smime-status-fn="smimeStatus"
      :pgp-keys-fn="pgpKeys"
      @close="showSettings = false"
      @update="onSettingsUpdate"
      @save-template="store.saveTemplate"
      @delete-template="store.deleteTemplate"
      @test-notification="onTestNotification"
      @export-diagnostics="onExportDiagnostics"
      @reset-connections="(id) => store.resetConnections(id)"
    />

    <!-- 快捷键帮助 -->
    <div v-if="showShortcuts" class="mail-modal-backdrop" @click.self="showShortcuts = false">
      <div class="mail-modal" role="dialog" aria-label="键盘快捷键">
        <div class="mail-modal-head">
          <h2 class="mail-modal-title">键盘快捷键</h2>
          <button class="mail-modal-close" @click="showShortcuts = false">×</button>
        </div>
        <div class="mail-modal-body">
          <div v-for="item in shortcutList" :key="item.keys" class="mail-shortcut-row">
            <kbd class="mail-kbd">{{ item.keys }}</kbd>
            <span>{{ item.label }}</span>
          </div>
        </div>
      </div>
    </div>

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
      <!-- 删除是高频危险动作：放在菜单上半段，不能因为条数多被挤到屏幕外。 -->
      <button class="mail-context-item is-danger" @click="contextAction('delete')">删除</button>
      <div class="mail-context-sep"></div>
      <button class="mail-context-item is-danger" @click="contextAction('blockSender')">
        把发件人加入黑名单
      </button>
      <button class="mail-context-item" @click="contextAction('allowSender')">
        把发件人加入白名单
      </button>
      <div v-if="contextFolders.length" class="mail-context-sep"></div>
      <button
        v-for="folder in contextFolders"
        :key="folder.name"
        class="mail-context-item"
        :title="folder.name"
        @click="contextAction('move', folder.name)"
      >
        移到「{{ folder.displayName }}」
      </button>
      <div class="mail-context-sep"></div>
      <button class="mail-context-item" @click="contextAction('saveEml')">另存为 .eml（含附件）</button>
      <button class="mail-context-item" @click="contextAction('saveHtml')">另存为 .html（正文）</button>
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
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useMailStore } from '../../../stores/mailStore'
import { groupRecipientToken } from '../../../stores/contactsStore'
import { avatarSeed, densityMetrics, monoFontStack, readingFontStack, themeToStyle } from './mail/theme'
import { SHORTCUT_LIST, useMailShortcuts } from './mail/useMailShortcuts'
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
const showShortcuts = ref(false)
const toast = ref(null)
const contextMenu = ref(null)
const pendingSend = ref(null)
const listPaneRef = ref(null)
const activeKey = ref('')
const attachmentChecks = ref([])
let toastTimer = null
let pendingTimer = null

const shortcutList = SHORTCUT_LIST

const settings = computed(() => store.settings)
const themes = computed(() => [...store.builtinThemes, ...(settings.value.customThemes || [])])

const rootStyle = computed(() => {
  const metrics = densityMetrics(settings.value.density, settings.value.readingSize)
  return {
    ...themeToStyle(store.theme, settings.value.accentOverride),
    // 显式声明配色方案：系统同时在深色模式下，原生复选框 / 滚动条 / 日期控件
    // 会跟着系统走而变成深灰色（亮色主题里出现「黑框」就是这么来的）。
    colorScheme: store.theme?.mode === 'light' ? 'light' : 'dark',
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

const ROLE_TITLES = {
  inbox: '收件箱',
  drafts: '草稿',
  sent: '已发送',
  archive: '存档',
  junk: '垃圾邮件',
  trash: '已删除',
}

const listTitle = computed(() => {
  if (store.activeLabel) return `分类：${store.activeLabel}`
  if (store.searchQuery.trim()) return '搜索结果'
  if (store.scope === 'unread') return '未读'
  if (store.scope === 'flagged') return '已加星标'
  if (store.scope === 'attachments') return '带附件的邮件'
  if (store.scope === 'all') return '全部邮件'
  // 按角色聚合的视图（垃圾邮件 / 已删除 / 已发送…）以前没有分支，
  // 会错误地沿用上一次打开的文件夹名（例如选了垃圾邮件却显示 INBOX）。
  if (ROLE_TITLES[store.scope]) return ROLE_TITLES[store.scope]
  return store.currentFolder?.displayName || '收件箱'
})

/** 右键菜单里的「移到」只列关键去向；全部文件夹在批量操作栏的下拉里。 */
const MOVE_ROLES = ['inbox', 'archive', 'junk', 'trash']
const contextFolders = computed(() => {
  const menu = contextMenu.value
  if (!menu) return []
  const accountId = menu.row?.accountId || store.activeAccountId
  const current = menu.row?.folder
  const seen = new Set()
  const out = []
  for (const folder of store.foldersByAccount[accountId] || []) {
    if (!MOVE_ROLES.includes(folder.role)) continue
    if (folder.name === current || seen.has(folder.role)) continue
    seen.add(folder.role)
    out.push(folder)
  }
  return out.sort((a, b) => MOVE_ROLES.indexOf(a.role) - MOVE_ROLES.indexOf(b.role))
})

function notify(text, error = false) {
  toast.value = { text, error }
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (toast.value = null), error ? 5200 : 2600)
}

// ---------------- 虚拟滚动行高 ----------------
//
// 与 mail.css 的 --mail-row-height 同源：densityMetrics 已经把它算好了，
// 这里取数值直接喂给列表组件，避免两边各写一份而慢慢走样。
const rowHeightPx = computed(() => {
  const value = densityMetrics(settings.value.density, settings.value.readingSize).rowHeight
  return Number(String(value).replace('px', '')) || 74
})

/** 键盘导航的「当前行」：打开邮件时同步一次。 */
watch(
  () => store.detail,
  (detail) => {
    if (detail) activeKey.value = `${detail.accountId}:${detail.folder}:${detail.uid}`
  },
)

// ---------------- 键盘快捷键 ----------------

function step(delta) {
  if (!store.messages.length) return
  const index = store.messages.findIndex((m) => store.keyOf(m) === activeKey.value)
  const next = Math.max(0, Math.min(store.messages.length - 1, index < 0 ? 0 : index + delta))
  const row = store.messages[next]
  if (!row) return
  activeKey.value = store.keyOf(row)
  listPaneRef.value?.ensureVisible(activeKey.value)
  store.openMessage(row).catch(() => {})
}

/** 当前上下文：优先「当前行」，其次「已打开邮件」，最后「批量选中的第一封」。 */
function currentRow() {
  const byKey = store.rowByKey(activeKey.value)
  if (byKey) return byKey
  if (store.selected) return store.selected
  const selected = store.selectedRows()
  return selected[0] || null
}

function currentDetail() {
  if (store.detail) return store.detail
  return currentRow()
}

function replyCurrent(all = false) {
  const detail = currentDetail()
  if (!detail) return
  onReply(detail, all)
}

useMailShortcuts(
  {
    next: () => step(1),
    prev: () => step(-1),
    open: () => {
      const row = currentRow()
      if (row) onOpenMessage(row)
    },
    reply: () => replyCurrent(false),
    replyAll: () => replyCurrent(true),
    forward: () => {
      const detail = currentDetail()
      if (detail) onForward(detail)
    },
    compose: () => onCompose(),
    star: () => {
      const row = currentRow()
      if (row) store.setFlagged(row, !row.flagged)
    },
    unread: () => {
      const row = currentRow()
      if (row) {
        store.setSeen([row], false).then(() => {
          if (store.detail && store.detail.uid === row.uid) store.detail.seen = false
        })
      }
    },
    archive: () => {
      const detail = currentDetail()
      if (detail) onArchiveCurrent(detail)
    },
    delete: () => {
      const detail = currentDetail()
      if (detail) onDeleteCurrent(detail)
    },
    escape: () => {
      if (showShortcuts.value) {
        showShortcuts.value = false
        return
      }
      if (contextMenu.value) {
        closeContextMenu()
        return
      }
      if (store.detail) closeReader()
    },
    help: () => (showShortcuts.value = true),
    send: () => {
      // Ctrl+Enter：只在写信时有效，交给编辑器自己的处理（避免与「打开」冲突）。
    },
  },
  {
    focusSearch: () => {
      document.querySelector('.mail-search input')?.focus()
    },
  },
)

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

async function onDeleteRow(row) {
  try {
    await store.runBatch('delete', null, [row])
    notify('已删除')
  } catch (error) {
    notify(String(error?.message || error), true)
  }
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
  // 聚合行：整个会话按封展开成独立卡片，而不是只打开最新一封再靠正文里的引用块。
  if (row?.threadUids?.length > 1) {
    await store.openThread(row)
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
  // 引用历史按「Apple Mail 会话卡片」排版：头像 + 姓名 + 主题 + 收件人 + 右上角时间，
  // 正文跟在卡片里，而不是一个套一个的灰色引用框。颜色用半透明灰和继承色，
  // 深色编辑器里也不会出现刺眼的白块；内联样式保证收件端没有我们的 CSS 也能看。
  const quotedBody =
    detail.html || escapeHtml(detail.text || '').replace(/\n/g, '<br>')
  const seed = avatarSeed(detail.fromName, detail.fromAddr)
  const recipients = []
  const toLine = (detail.to || [])
    .map((item) => item?.name || item?.address)
    .filter(Boolean)
    .join('、')
  const ccLine = (detail.cc || [])
    .map((item) => item?.name || item?.address)
    .filter(Boolean)
    .join('、')
  if (toLine) recipients.push(`发给 ${toLine}`)
  if (ccLine) recipients.push(`抄送 ${ccLine}`)
  const recipientLine = recipients.length
    ? `<span style="font-size:11.5px;opacity:.62;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${escapeHtml(
        recipients.join(' · '),
      )}</span>`
    : ''
  const htmlBody =
    '<div class="mail-quote" data-guaxi-quote-card="1" style="margin:18px 0 0;border-radius:14px;background:rgba(127,127,127,0.07);overflow:hidden">' +
    '<div class="mail-quote-head" data-guaxi-quote-head="1" style="display:flex;align-items:flex-start;gap:10px;padding:12px 16px">' +
    `<span class="mail-quote-avatar" data-guaxi-quote-avatar="1" style="flex:0 0 auto;width:30px;height:30px;border-radius:50%;background:hsl(${seed.hue} 42% 48%);color:#fff;display:flex;align-items:center;justify-content:center;font-size:13px;font-weight:600">${escapeHtml(
      seed.letter,
    )}</span>` +
    '<span class="mail-quote-meta" data-guaxi-quote-meta="1" style="display:flex;flex-direction:column;gap:2px;min-width:0">' +
    `<span class="mail-quote-from" data-guaxi-quote-from="1" style="font-size:13px;font-weight:600;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${escapeHtml(
      who,
    )}</span>` +
    `<span class="mail-quote-subject" data-guaxi-quote-subject="1" style="font-size:12px;opacity:.72;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${escapeHtml(
      detail.subject || '（无主题）',
    )}</span>` +
    recipientLine +
    '</span>' +
    `<span class="mail-quote-date" data-guaxi-quote-date="1" style="margin-left:auto;font-size:11.5px;opacity:.65;white-space:nowrap">${escapeHtml(
      when,
    )}</span>` +
    '</div>' +
    `<blockquote class="mail-quote-body" data-guaxi-quote-body="1" style="margin:0;padding:0 16px 14px;border:0;background:transparent;color:inherit">${quotedBody}</blockquote>` +
    '</div>'
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

async function onSaveAttachment({ attachment, detail }) {
  // 会话视图里附件可能属于任意一封邮件，不能固定用 store.detail（最新一封）。
  const message = detail || store.detail
  if (!message) return
  try {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const dir = await save({ defaultPath: attachment.filename })
    if (!dir) return
    // 用户可能选了一个新名字：拆出目录与文件名。
    const parts = dir.replace(/\\/g, '/').split('/')
    const filename = parts.pop()
    const path = await store.saveAttachment(
      message,
      { ...attachment, filename },
      parts.join('/'),
    )
    notify(`已保存到 ${path}`)
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

/** 点附件：下载到本地缓存后交给系统默认程序打开；脚本/可执行文件先确认。 */
async function onOpenAttachment({ attachment, detail }) {
  const message = detail || store.detail
  if (!message) return
  try {
    const dangerous = await invoke('mail_is_dangerous_attachment', {
      name: attachment.filename,
    }).catch(() => false)
    if (
      dangerous &&
      !window.confirm(
        `「${attachment.filename}」是脚本或可执行文件，打开它可能运行代码。\n确定要用系统默认程序打开吗？`,
      )
    ) {
      return
    }
    await invoke('mail_open_attachment', {
      accountId: message.accountId,
      folder: message.folder,
      uid: message.uid,
      part: attachment.part,
      filename: attachment.filename,
    })
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

function closeReader() {
  store.selected = null
  store.detail = null
  store.thread = []
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
  const width = 220
  // 菜单本身有 max-height + 滚动；这里把 top 限制在「一屏能放下菜单最大高度」的范围内，
  // 避免在窗口下半部分右键时菜单底部（删除等）被屏幕裁掉。
  const menuMaxHeight = Math.min(window.innerHeight * 0.7, 560)
  const x = Math.max(8, Math.min(event.clientX, window.innerWidth - width - 8))
  const y = Math.max(8, Math.min(event.clientY, window.innerHeight - menuMaxHeight - 8))
  // 右键不改变「勾选」状态：点中已勾选的行 → 整批；点在其他行 → 只作用于这一封，
  // 同时清掉之前的勾选，避免批量操作落到用户看不见的邮件上。
  const key = store.keyOf(row)
  const inSelection = store.selectedKeys.includes(key)
  if (!inSelection && store.selectedKeys.length) store.clearSelection()
  const rows = inSelection ? store.selectedRows() : [row]
  contextMenu.value = { row, rows, x, y }
  setTimeout(() => document.addEventListener('click', closeContextMenu, { once: true }), 0)
}

function closeContextMenu() {
  contextMenu.value = null
}

async function contextAction(action, target) {
  const menu = contextMenu.value
  closeContextMenu()
  if (!menu?.row) return
  const row = menu.row

  // 右键菜单作用于「当前选择集」：点中已勾选的行处理整批，点在其他行只处理这一封。
  const rows = menu.rows?.length ? menu.rows : [row]

  try {
    if (action === 'saveEml' || action === 'saveHtml') {
      await saveMessageAs(row, action === 'saveEml' ? 'eml' : 'html')
      return
    }
    if (action === 'markRead') {
      await store.runBatch(row.seen ? 'unread' : 'read', null, rows)
      return
    }
    if (action === 'flag') {
      await store.runBatch(row.flagged ? 'unflag' : 'flag', null, rows)
      return
    }
    if (action === 'junk') {
      await store.runBatch('junk', null, rows)
      notify(`已移到垃圾邮件（${rows.length} 封）`)
      return
    }
    if (action === 'unmarkJunk') {
      await store.runBatch('unmarkJunk', null, rows)
      notify(`已移回收件箱（${rows.length} 封）`)
      return
    }
    if (action === 'move' && target) {
      await store.runBatch('move', target, rows)
      notify(`已移动 ${rows.length} 封`)
      return
    }
    if (action === 'delete') {
      await store.runBatch('delete', null, rows)
      notify(`已删除 ${rows.length} 封`)
      return
    }
    if (action === 'blockSender' || action === 'allowSender') {
      const targets = [...new Set(rows.map((r) => String(r.fromAddr || '').trim().toLowerCase()))]
        .filter(Boolean)
      if (!targets.length) {
        notify('这封邮件没有发件人地址', true)
        return
      }
      const kind = action === 'blockSender' ? 'block' : 'allow'
      for (const address of targets) {
        await store.blockSender(address, kind)
      }
      notify(kind === 'block' ? `已加入黑名单：${targets.join('、')}` : `已加入白名单：${targets.join('、')}`)
    }
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}
const contextCount = computed(() => contextMenu.value?.rows?.length || 1)
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
    await store.runBatch('move', folderName, rows)
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
    // 发信会往「已发送」追加一封：刷新服务器计数，侧栏与列表才不会停在旧数字。
    store
      .loadFolders(store.activeAccountId, { refresh: true })
      .then(() => Promise.all([store.reloadUnread(), store.loadMessages({ silent: true })]))
      .catch(() => {})
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

async function onSaveAccount(account, secret, { closeAfter = true, oauthPending = false } = {}) {
  try {
    // 设备码授权还没完成时先不拉文件夹：没有令牌，连上去只会报「未授权」。
    const saved = await store.saveAccount(account, secret, { loadFolders: !oauthPending })
    notify(oauthPending ? '账户已保存，完成授权后会自动同步' : '账户已保存')
    if (closeAfter && !oauthPending) {
      // 同步可能持续几秒：先让它走后台，别把「保存中…」的弹窗一直挂着。
      showAccounts.value = false
      store
        .syncNow()
        .then((result) => {
          if (result.errors?.length) notify(result.errors[0], true)
        })
        .catch(() => {})
    }
    return saved
  } catch (error) {
    notify(String(error?.message || error), true)
    throw error
  }
}

async function onAccountDeleted(id) {
  await store.deleteAccount(id)
  notify('账户已删除')
}

async function onTestAccount(account, secret) {
  return store.testAccount(account, secret)
}

/** OAuth 轮询：成功后把账户标记刷新一下，界面才能显示「已授权」。 */
async function onOAuthPoll(accountId, provider, deviceCode) {
  const email = await store.pollOAuth(
    accountId,
    provider,
    deviceCode.deviceCode,
    deviceCode.interval,
  )
  const accounts = await invoke('mail_list_accounts').catch(() => null)
  if (accounts) store.accounts = accounts
  await store.resetConnections(accountId).catch(() => {})
  // 授权成功后第一次连得上服务器：补一次文件夹与未读，侧栏不用等下一轮轮询。
  await store.loadFolders(accountId, { refresh: true })
  if (!store.activeAccountId || store.activeAccountId === accountId) {
    store.selectAccount(accountId)
  }
  return email
}

/** 浏览器没能自动回到本机时，用粘贴进来的回调地址完成 Outlook 授权。 */
async function onOAuthComplete(accountId, flowId, provider, redirectUri, code) {
  const email = await store.completeOAuth(
    accountId,
    flowId,
    provider,
    redirectUri,
    code,
  )
  const accounts = await invoke('mail_list_accounts').catch(() => null)
  if (accounts) store.accounts = accounts
  await store.resetConnections(accountId).catch(() => {})
  await store.loadFolders(accountId, { refresh: true })
  return email
}

/** 通讯录里点邮箱地址：直接带着收件人打开写信窗口。 */
function onComposeTo(email) {
  const address = String(email || '').trim()
  showContacts.value = false
  store.startCompose('new')
  if (store.composing && address) store.composing.to = [address]
}

/** 「写信给全组」：地址栏只放一个组芯片，发送时才展开成员。 */
function onComposeGroup(group) {
  if (!group?.id) return
  showContacts.value = false
  store.startCompose('new')
  if (store.composing) store.composing.to = [groupRecipientToken(group)]
}

/** 通讯录里点历史邮件：关掉通讯录，在阅读窗格打开那一封。 */
function onOpenContactMessage(message) {
  showContacts.value = false
  if (message) onOpenMessage(message).catch(() => {})
}

async function onAccountSelected(id) {
  store.selectAccount(id)
  showAccounts.value = false
}

async function onSettingsUpdate(patch) {
  try {
    await store.saveSettings(patch)
    // 改完设置后有些东西要重算：IDLE 开关、连接参数。
    if ('fetchMode' in patch) {
      if (patch.fetchMode === 'idle') store.startIdle().catch(() => {})
      else store.stopIdle().catch(() => {})
    }
    if ('virtualList' in patch) {
      // 虚拟滚动开关影响列表渲染方式，强制刷新一次。
      await store.loadMessages({ silent: true })
    }
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

async function onCheckAttachments(paths) {
  attachmentChecks.value = await store.checkAttachments(paths)
}

async function onReplayOffline() {
  const result = await store.replayOffline()
  if (result?.failed) notify(`${result.failed} 条操作重放失败，网络恢复后会再试`, true)
  else if (result?.replayed) notify(`已补传 ${result.replayed} 条离线操作`)
  else notify('没有需要补传的操作')
}

async function onTestNotification() {
  try {
    await store.testNotification()
    notify('已发送测试通知')
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

async function onExportDiagnostics() {
  try {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const path = await save({
      title: '导出诊断报告',
      defaultPath: `guaxi-mail-diagnostics-${new Date().toISOString().slice(0, 10)}.json`,
      filters: [{ name: 'JSON', extensions: ['json'] }],
    })
    if (!path) return
    await store.exportDiagnostics(path)
    notify(`诊断报告已保存到 ${path}`)
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

async function previewSieve(accountId) {
  return invoke('mail_sieve_preview', { accountId })
}

async function syncSieve(accountId) {
  const warnings = await invoke('mail_sieve_sync', { accountId })
  return warnings || []
}

async function smimeStatus(accountId) {
  return invoke('mail_smime_status', { accountId })
}

async function pgpKeys() {
  return invoke('mail_pgp_keys')
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
  max-width: 280px;
  /* 条目多时菜单必须能滚动，否则底部（删除 / 另存为）会被屏幕裁掉。 */
  max-height: min(70vh, 560px);
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-width: thin;
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
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
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
