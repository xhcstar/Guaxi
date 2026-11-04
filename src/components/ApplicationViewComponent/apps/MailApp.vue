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
          支持 IMAP 或 POP3 收信、SMTP 发信的邮箱都可以添加，包括 QQ、163、Gmail、
          Outlook 与企业邮箱。口令只存在系统凭据库里，不会随数据库备份外流。
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
        @compose="onCompose"
        @accounts="openAccounts"
        @contacts="showContacts = true"
        @rules="showRules = true"
        @open-eml="onOpenEml"
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
          @add-account="openAccounts"
          @create-folder="onCreateFolder"
          @delete-folder="onDeleteFolder"
          @pin-folder="onPinFolder"
          @settings="showSettings = true"
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
          :can-backfill="store.canBackfill"
          :loading-more="store.loadingMore"
          :scheduled-items="folderScheduledItems"
          :staged-items="stageOutboxItems"
          :scheduled-label="folderScheduledItems.length ? scheduleLabel(folderScheduledItems[0]) : ''"
          :retrying="retryingOutbox"
          @retry-outbox="onRetryOutbox"
          @manage-scheduled="scheduledOpen = true"
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
          @select-all="onSelectAll"
          :selection-all="store.selectionAll"
          @select-range="(keys) => store.selectRange(keys, true)"
          @clear-selection="store.clearSelection"
          @batch="onBatch"
          @drop-on-folder="onDropOnFolder"
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
          :thread-total="store.threadTotal"
          :loading="store.detailLoading"
          :settings="settings"
          :theme-mode="store.theme?.mode || 'dark'"
          :theme-style="rootStyle"
          :mono-font="settings.monoFont"
          :search-query="store.searchQuery"
          :can-recall="canRecallCurrent"
          @reply="(m, opts) => onReply(m, false, opts || {})"
          @reply-all="(m) => onReply(m, true)"
          @forward="onForward"
          @delete="onDeleteCurrent"
          @archive="onArchiveCurrent"
          @recall="onRecallMessage"
          @toggle-flag="onToggleFlagCurrent"
          @toggle-unread="onToggleUnreadCurrent"
          @save-attachment="onSaveAttachment"
          @open-attachment="onOpenAttachment"
          :close-label="settings.listCollapsed ? '收起邮件' : '返回列表'"
          @close="closeReader"
        >
          <!-- 回复 / 转发直接长在阅读区顶部：列表和原文都还能看，不用先关掉浮层。 -->
          <template #inline-composer>
            <div
              v-if="store.composing && !composeOpensInWindow && store.composing.mode !== 'new'"
              class="mail-inline-composer"
            >
              <MailComposer
                inline
                :key="
                  store.composing.mode +
                  ':' +
                  (store.composing.inReplyTo || store.composing.draftUid || '')
                "
                :draft="store.composing"
                :accounts="store.accounts"
                :settings="settings"
                :theme-mode="store.theme?.mode || 'dark'"
                :undo-seconds="settings.undoSendSeconds"
                :templates="store.templates"
                :attachment-checks="attachmentChecks"
                :save-mailbox-draft="onAutoSaveToMailbox"
                @update="(patch) => Object.assign(store.composing, patch)"
                @send="onSend"
                @schedule="onScheduleSend"
                @save-draft="onSaveDraft"
                @discard="onDiscardCompose"
                @apply-template="(tpl) => store.applyTemplate(tpl)"
                @check-attachments="onCheckAttachments"
                @close="onCloseCompose"
              />
            </div>
          </template>
        </MailReader>
        <div v-else-if="!settings.listCollapsed" class="mail-pane mail-reader">
          <div class="mail-reader-placeholder">
            选中左侧的一封邮件开始阅读。<br />
            未读邮件带一条细色条，点开即标记为已读。
          </div>
        </div>
      </div>
    </template>

    <!-- 兜底：没有打开的阅读区（例如从列表用快捷键回复），或从通讯录新写一封时，
         仍用弹层，避免「回复了却看不到编辑器」。 -->
    <MailComposer
      v-if="
        store.composing &&
        !composeOpensInWindow &&
        (store.composing.mode === 'new' || !store.detail)
      "
      :draft="store.composing"
      :accounts="store.accounts"
      :settings="settings"
      :theme-mode="store.theme?.mode || 'dark'"
      :undo-seconds="settings.undoSendSeconds"
      :templates="store.templates"
      :attachment-checks="attachmentChecks"
      :save-mailbox-draft="onAutoSaveToMailbox"
      @update="(patch) => Object.assign(store.composing, patch)"
      @send="onSend"
      @schedule="onScheduleSend"
      @save-draft="onSaveDraft"
                @discard="onDiscardCompose"
      @apply-template="(tpl) => store.applyTemplate(tpl)"
      @check-attachments="onCheckAttachments"
      @close="onCloseCompose"
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
      :delete-fn="onAccountDeleted"
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

    <!-- 查看本机保存的 .eml -->
    <MailEmlDialog
      v-if="emlDetail"
      :detail="emlDetail"
      :path="emlPath"
      :theme-mode="store.theme?.mode || 'dark'"
      @close="emlDetail = null"
    />

    <!-- 应用内确认框：所有不可逆操作共用（系统 confirm 在本 webview 里可能不弹）。 -->
    <MailConfirmHost />

    <!-- 规则管理 -->
    <MailRulesDialog      v-if="showRules"
      :rules="store.rules"
      :accounts="store.accounts"
      :folders-by-account="store.foldersByAccount"
      :active-account-id="store.activeAccountId"
      :sender-rules="store.senderRules"
      :create-folder-fn="onCreateFolderFromRules"
      :refresh-folders-fn="onRefreshFoldersFromRules"
      :save-rule-fn="onSaveRuleFromDialog"
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
      :save-template-fn="store.saveTemplate"
      @close="showSettings = false"
      @update="onSettingsUpdate"
      @save-template="store.saveTemplate"
      @delete-template="store.deleteTemplate"
      @test-notification="onTestNotification"
      @export-diagnostics="onExportDiagnostics"
      @reset-connections="(id) => store.resetConnections(id)"
    />

    <!-- 快捷键帮助 -->
    <div v-if="showShortcuts" class="mail-modal-backdrop" @mousedown.self="showShortcuts = false">
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
      <!-- 「移动」与 Outlook 一致：一个主条目，常用去向（存档 / 收件箱）直接列出来，
           其余全部文件夹收进子菜单。子菜单是悬停打开的，列表来自当前账户实时的
           文件夹表，新同步到 / 新建的文件夹不需要重启就能出现在这里。 -->
      <button class="mail-context-item" @click="contextAction('archive')">
        存档
        <span class="mail-context-hint">移动到的默认文件夹</span>
      </button>
      <div
        class="mail-context-item is-parent"
        :class="{ 'is-open': contextSubmenu === 'move' }"
        @mouseenter="openSubmenu('move', $event)"
        @mouseleave="scheduleSubmenuClose"
      >
        <span>移动</span>
        <span class="mail-context-chevron" aria-hidden="true">›</span>
      </div>
      <div class="mail-context-sep"></div>
      <button class="mail-context-item" @click="contextAction('saveEml')">另存为 .eml（含附件）</button>
      <button class="mail-context-item" @click="contextAction('saveHtml')">另存为 .html（正文）</button>
    </div>

    <!-- 「移动」的子菜单挂在主菜单外面、用 fixed 定位：
         主菜单自己有 max-height + overflow-y:auto，写在里面的绝对定位子菜单
         会被这个滚动容器裁掉——表现就是右键「移动」看不到任何文件夹。 -->
    <div
      v-if="contextMenu && contextSubmenu === 'move'"
      class="mail-context mail-context-submenu"
      :style="{ left: `${submenuPos.x}px`, top: `${submenuPos.y}px` }"
      @click.stop
      @mouseenter="keepSubmenu"
      @mouseleave="scheduleSubmenuClose"
    >
      <div v-if="!contextFolders.length" class="mail-context-empty">没有可移动到的文件夹</div>
      <button
        v-for="folder in contextFolders"
        :key="folder.name"
        class="mail-context-item"
        :class="{ 'is-current': folder.name === contextMenu.row?.folder }"
        :title="folder.name"
        @click="contextAction('move', folder.name)"
      >
        {{ folder.displayName }}
      </button>
    </div>

    <!-- 底部提示 -->
    <div v-if="toast" class="mail-toast" :class="{ 'is-error': toast.error }">
      <span>{{ toast.text }}</span>
      <button v-if="toast.action" class="mail-toast-action" @click="runToastAction">
        {{ toast.action.label }}
      </button>
    </div>

    <!-- 待发倒计时（撤回窗口） -->
    <div v-if="pendingSend" class="mail-pending">
      <span>邮件将在 {{ pendingSend.remaining }} 秒后发出</span>
      <button class="mail-btn" @click="undoSend">撤回</button>
      <button class="mail-btn is-primary" @click="sendNow">立即发送</button>
    </div>

    <!-- 定时发送 / 待发队列 -->
    <div v-if="scheduledSends.length" class="mail-scheduled">
      <span>待发送 {{ scheduledSends.length }} 封，最近一封：{{ scheduleLabel(scheduledSends[0]) }}</span>
      <button class="mail-btn" @click="scheduledOpen = true">管理</button>
    </div>

    <div v-if="scheduledOpen" class="mail-modal-backdrop" @mousedown.self="scheduledOpen = false">
      <div class="mail-modal mail-schedule-modal" role="dialog" aria-label="待发送邮件">
        <div class="mail-modal-head">
          <h2 class="mail-modal-title">待发送邮件</h2>
          <button class="mail-modal-close" title="关闭" @click="scheduledOpen = false">×</button>
        </div>
        <div class="mail-schedule-list">
          <div v-for="item in scheduledSends" :key="item.id" class="mail-schedule-item">
            <div class="mail-schedule-info">
              <div class="mail-schedule-subject">{{ item.subject || '（无主题）' }}</div>
              <div class="mail-schedule-meta">{{ item.to }} · {{ scheduleLabel(item) }}</div>
            </div>
            <button class="mail-btn" @click="onSendScheduledNow(item.id)">立即发送</button>
            <button class="mail-btn" @click="onCancelScheduled(item.id)">撤回</button>
          </div>
          <div v-if="!scheduledSends.length" class="mail-hint">没有待发送的邮件。</div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useMailStore } from '../../../stores/mailStore'
import { groupRecipientToken } from '../../../stores/contactsStore'
import { createLogger } from '../../../utils/logger'
import { avatarSeed, densityMetrics, monoFontStack, readingFontStack, themeToStyle } from './mail/theme'
import { flattenQuoteHtml } from './mail/quoteCards'
import { selectableMoveTargets } from './mail/folderTree'
import { SHORTCUT_LIST, useMailShortcuts } from './mail/useMailShortcuts'
import MailToolbar from './mail/MailToolbar.vue'
import MailSidebar from './mail/MailSidebar.vue'
import MailListPane from './mail/MailListPane.vue'
import MailReader from './mail/MailReader.vue'
import MailComposer from './mail/MailComposer.vue'
import MailAccountDialog from './mail/MailAccountDialog.vue'
import MailRulesDialog from './mail/MailRulesDialog.vue'
import MailEmlDialog from './mail/MailEmlDialog.vue'
import MailConfirmHost from './mail/MailConfirmHost.vue'
import { askMailConfirm } from './mail/mailConfirm'
import ContactsDialog from './mail/ContactsDialog.vue'
import MailSettingsDrawer from './mail/MailSettingsDrawer.vue'

const store = useMailStore()
const logger = createLogger('MailApp')

const showAccounts = ref(false)
const showRules = ref(false)
/** 顶栏「打开 .eml」：解析出来的本机邮件文件。 */
const emlDetail = ref(null)
const emlPath = ref('')
const showContacts = ref(false)
const showSettings = ref(false)
const showShortcuts = ref(false)
const toast = ref(null)
const contextMenu = ref(null)
const pendingSend = ref(null)
const scheduledOpen = ref(false)
/** 待发队列：排除正在倒计时撤回的那一条，其余（含定时邮件）都可以管理。 */
const scheduledSends = computed(() =>
  store.outbox
    .filter((item) => !item.staged && (!pendingSend.value || item.id !== pendingSend.value.id))
    .slice()
    .sort((a, b) => Number(a.dueAt) - Number(b.dueAt)),
)
const listPaneRef = ref(null)
const activeKey = ref('')
const attachmentChecks = ref([])
/** 「在新窗口里写」的调用过程中先隐藏内联卡片，避免开窗瞬间闪一下编辑区。 */
const composeOpensInWindow = ref(false)
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

/**
 * 右键菜单「移动」子菜单里的全部去向。
 *
 * 不再只列几个角色：Outlook 用户习惯了「移动 → 随便哪个文件夹」，
 * 漏掉任何一个都会让人以为那封信移动不了。这里取当前账户实时的文件夹表，
 * 排除源文件夹与纯本地的发件箱，标准角色排前面、自建文件夹按名字排后面。
 */
const FOLDER_ROLE_ORDER = ['inbox', 'archive', 'sent', 'drafts', 'junk', 'trash', 'outbox', 'custom']
const contextFolders = computed(() => {
  const menu = contextMenu.value
  if (!menu) return []
  const accountId = menu.row?.accountId || store.activeAccountId
  const current = menu.row?.folder
  // 服务商模板壳（QQ 的「其他文件夹」那类）不进目标列表：它们的邮件本来就不同步，
  // 移进去等于凭空消失。
  return selectableMoveTargets(store.foldersByAccount[accountId] || [])
    .filter((folder) => folder.name !== current)
    .sort((a, b) => {
      const ai = FOLDER_ROLE_ORDER.indexOf(a.role)
      const bi = FOLDER_ROLE_ORDER.indexOf(b.role)
      if (ai !== bi) return (ai < 0 ? 99 : ai) - (bi < 0 ? 99 : bi)
      return String(a.displayName).localeCompare(String(b.displayName), 'zh-CN')
    })
})

function notify(text, error = false, action = null) {
  toast.value = { text, error, action }
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (toast.value = null), error ? 5200 : action ? 5200 : 2600)
}

/** 提示条上的动作（目前用于「撤销删除」）。 */
function runToastAction() {
  const action = toast.value?.action
  toast.value = null
  clearTimeout(toastTimer)
  action?.run?.()
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
let detailKey = ''
watch(
  () => store.detail,
  (detail) => {
    const key = detail ? `${detail.accountId}:${detail.folder}:${detail.uid}` : ''
    if (key) activeKey.value = key
    // 内联回复跟着当前邮件走：换一封/关掉阅读区时把它存进草稿箱，
    // 否则回复卡片会挂在另一封邮件头上。
    if (detailKey && detailKey !== key && store.composing) onCloseCompose()
    detailKey = key
  },
)

// 延迟删除真正提交失败：邮件会被放回列表，同时把原因说出来。
watch(
  () => store.deferredDeleteNotice,
  (notice) => {
    if (notice?.text) notify(notice.text, Boolean(notice.error))
  },
)

// ---------------- 键盘快捷键 ----------------

/** Shift+方向键扩展选择的锚点与「扩展前已有的选择」，普通移动时清空。 */
let keyboardAnchor = ''
let keyboardBase = []

function step(delta, extend = false) {
  if (!store.messages.length) return
  const index = store.messages.findIndex((m) => store.keyOf(m) === activeKey.value)
  const current = index < 0 ? 0 : index
  const next = Math.max(0, Math.min(store.messages.length - 1, current + delta))
  const row = store.messages[next]
  if (!row) return

  if (extend) {
    if (!keyboardAnchor) {
      keyboardAnchor = activeKey.value || store.keyOf(store.messages[current])
      keyboardBase = [...store.selectedKeys]
    }
    const anchorIndex = store.messages.findIndex(
      (item) => store.keyOf(item) === keyboardAnchor,
    )
    const from = Math.min(anchorIndex < 0 ? current : anchorIndex, next)
    const to = Math.max(anchorIndex < 0 ? current : anchorIndex, next)
    const range = store.messages.slice(from, to + 1).map((item) => store.keyOf(item))
    store.setSelection([...keyboardBase, ...range])
  } else {
    keyboardAnchor = ''
    keyboardBase = []
  }

  activeKey.value = store.keyOf(row)
  listPaneRef.value?.ensureVisible(activeKey.value)
  // Shift+方向键只改选择，不打开邮件——否则每按一下都换一次正文。
  if (!extend) store.openMessage(row).catch(() => {})
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

/** 当前阅读的是「已发送」里的邮件：给它一个明确的撤回入口。 */
const canRecallCurrent = computed(() => {
  const detail = store.detail
  if (!detail) return false
  const folders = store.foldersByAccount[detail.accountId] || []
  return folders.some((folder) => folder.name === detail.folder && folder.role === 'sent')
})

/**
 * 已投递邮件的「撤回」：IMAP / POP3 / SMTP 没有 Exchange 那种从对方邮箱
 * 物理删除的能力，所以这里不做一个看起来成功、实际对方仍能看到的假撤回。
 * 只有还停在待发队列里的延迟邮件可以真正撤回，入口在「待发送邮件」里。
 */
async function onRecallMessage() {
  await askMailConfirm(
    '这封邮件已经投递，IMAP / POP3 / SMTP 无法从对方邮箱收回。\n' +
      '只有仍在待发队列里的延迟邮件可以撤回；已延迟的邮件可以在「待发送邮件」里撤回。',
    {
      title: '无法撤回已投递的邮件',
      confirmText: '知道了',
      danger: false,
    },
  )
}

function replyCurrent(all = false) {
  const detail = currentDetail()
  if (!detail) return
  onReply(detail, all)
}

useMailShortcuts(
  {
    next: (event) => step(1, Boolean(event?.shiftKey)),
    prev: (event) => step(-1, Boolean(event?.shiftKey)),
    open: () => {
      keyboardAnchor = ''
      keyboardBase = []
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
      // 选择模式下按删除：删整批；没有勾选时删当前打开 / 光标所在的那封。
      if (store.selectedKeys.length) {
        deleteRowsWithUndo()
        return
      }
      const detail = currentDetail()
      if (detail) onDeleteCurrent(detail)
    },
    selectAll: () => {
      onSelectAll(true)
    },
    undo: async () => {
      try {
        const result = await store.undoLastAction()
        if (!result) notify('没有可撤销的操作')
        else if (result.type === 'delete') notify('已撤销删除')
        else notify('已撤销上一步')
      } catch (error) {
        notify(String(error?.message || error), true)
      }
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
      if (store.selectedKeys.length) {
        store.clearSelection()
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

/**
 * 写信窗口关掉后会广播一次：那里已经把草稿写进库了，主窗口刷新一下草稿箱，
 * 用户回到主窗口就能看到「刚才写了一半的那封」。
 */
let stopComposeClosed = null

onMounted(async () => {
  await store.init()
  await store.loadOutbox()
  if (window.location.search.includes('setup=1')) showAccounts.value = true
  try {
    const { listen } = await import('@tauri-apps/api/event')
    stopComposeClosed = await listen('mail-compose:closed', (event) => {
      store.loadOutbox().catch(() => {})
      store.loadFolders(store.activeAccountId, { refresh: true }).catch(() => {})
      store.loadMessages({ silent: true }).catch(() => {})
      // 独立窗口里存上的草稿，回到主窗口要能看见这条消息——否则「我刚才写的呢」。
      if (event?.payload?.saved) notify('未发送，已存入草稿箱')
    })
  } catch {
    /* 非 Tauri 环境（浏览器预览）没有事件系统，忽略即可 */
  }
})

onBeforeUnmount(() => {
  stopComposeClosed?.()
  document.removeEventListener('mousemove', onDragMove)
  document.removeEventListener('mouseup', endDrag)
  store.dispose()
  clearTimeout(toastTimer)
  clearInterval(pendingTimer)
  document.removeEventListener('click', closeContextMenu)
})

// ---------------- 操作 ----------------

/**
 * 关闭编辑器：把「有没有真的存进草稿箱」说清楚。
 *
 * 存失败只在控制台里留一行日志是不够的——用户判断「有没有存」的唯一依据是
 * 草稿箱里能不能看到那封，看不见就当没存（真实反馈）。
 */
/** 编辑器里点了「直接放弃」：不写草稿箱，也不留在本地暂存。 */
async function onDiscardCompose() {
  // 编辑器每 30 秒会自动往草稿箱写一份；放弃时 store 会把那一份一起删掉。
  // 删不掉要如实说，否则用户会看到「明明点了放弃，草稿箱里还有」。
  const cleaned = await store.discardCompose()
  if (cleaned) notify('已放弃这次编辑，没有写入草稿箱')
  else notify('已放弃这次编辑，但草稿箱里那份没能删掉，请稍后重试', true)
}

async function onCloseCompose() {
  const result = await store.closeCompose()
  if (!result || result.skipped || !result.saved) {
    if (result?.error) notify(`草稿没有保存：${result.error}`, true)
    return
  }
  notify(result.localOnly ? '未发送，已存到本机草稿箱（服务器未连接）' : '未发送，已存入草稿箱')
}

async function onSync() {
  // 手动同步顺带重新发现文件夹：别处（手机 / 网页）增删的目录也能立刻出现在这里，
  // 所以顶栏不再需要一个单独的「重新发现文件夹」按钮。
  const result = await store.syncNow({ refreshFolders: true })
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

/**
 * 规则弹层里「移到文件夹」旁边的「＋」：建完立刻能选中，不必关掉弹层去侧栏建。
 * 返回给弹层用于回填选中项；失败时抛错让弹层提示。
 */
async function onCreateFolderFromRules(name) {
  const created = await store.createFolder(name)
  notify(`已创建「${name}」`)
  return created
}

/** 规则弹层打开 / 下拉聚焦时刷新一次文件夹表：其它客户端新建的目录要能立刻看到。 */
function onRefreshFoldersFromRules() {
  if (!store.activeAccountId) return
  store.loadFolders(store.activeAccountId, { refresh: true }).catch(() => {})
}

/**
 * 保存规则，并按需立刻对库里已有的邮件执行一遍。
 *
 * 规则平时只在同步到新邮件时跑，所以「建了规则 + 建了文件夹」之后点进新文件夹
 * 还是空的（真实反馈）。勾选立即执行时扫一遍已有邮件，并把命中条数说清楚——
 * 命中 0 封也要说，否则用户会以为按钮没生效。
 */
async function onSaveRuleFromDialog(rule, applyExisting = false) {
  const saved = await store.saveRule(rule)
  // 条件改完后，旧目标文件夹里不再匹配的邮件已经在后端退回收件箱。
  const restored = Number(store.lastRuleRestore) || 0
  if (restored) {
    await Promise.all([
      store.loadFolders(store.activeAccountId).catch(() => {}),
      store.loadMessages({ silent: true }).catch(() => {}),
      store.reloadUnread().catch(() => {}),
    ])
  }
  const restoredNote = restored ? `，并把 ${restored} 封不再匹配的邮件退回收件箱` : ''
  if (!applyExisting) {
    notify(`规则「${saved.name}」已保存${restoredNote}，将作用于之后收到的新邮件`)
    return saved
  }
  const accountId = store.activeAccountId || store.accounts[0]?.id
  if (!accountId) {
    notify('没有可用账户，无法对已有邮件执行规则', true)
    return saved
  }
  const result = await invoke('mail_apply_rules_to_existing', { accountId, ruleId: saved.id })
  const matched = Number(result?.matched) || 0
  const scanned = Number(result?.scanned) || 0
  // moved 是后端按「源夹里少了几行」数出来的真数：离线时 IMAP MOVE 会整批失败，
  // 只报命中数就会变成「提示成功、文件夹里还是空的」。
  const moved = Number(result?.moved) || 0
  const partial = matched > 0 && moved < matched
  const restoredHint = restored ? `；另有 ${restored} 封不再匹配的邮件已退回收件箱` : ''
  notify(
    (!matched
      ? `已检查 ${scanned} 封已有邮件，没有命中这条规则的`
      : partial
        ? `命中 ${matched} 封，实际处理 ${moved} 封——其余没能连上服务器，等下次同步再补`
        : `已对已有邮件执行：${matched} 封已按规则处理（共检查 ${scanned} 封）`) + restoredHint,
    partial,
  )
  await Promise.all([
    store.loadFolders(accountId).catch(() => {}),
    store.loadMessages({ silent: true }).catch(() => {}),
    store.reloadUnread().catch(() => {}),
  ])
  return saved
}

async function onDeleteFolder(folder) {
  if (folder.role !== 'custom') {
    notify('系统文件夹不能删除', true)
    return
  }
  if (
    !(await askMailConfirm(
      `服务器上该文件夹及其中的邮件会一并删除，无法撤销。`,
      { title: `删除文件夹「${folder.displayName}」？`, confirmText: '删除文件夹' },
    ))
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

// ---------------- 发件箱 ----------------

/** 当前列表就是发件箱时，把「多次失败后暂存」的邮件摆到列表上方。 */
const stageOutboxItems = computed(() => {
  const folder = store.currentFolder
  if (!folder || folder.role !== 'outbox') return []
  return store.outbox.filter((item) => item.staged)
})

/**
 * 发件箱里「定时待发」的邮件：还没到发送时间的那些。
 *
 * 定时邮件以前只活在一个悬浮小条里，进了发件箱反而是空的——用户会以为邮件丢了。
 * 现在它们按时间顺序出现在发件箱列表上方，到点自动发出后消失。
 */
const folderScheduledItems = computed(() => {
  const folder = store.currentFolder
  if (!folder || folder.role !== 'outbox') return []
  const now = Date.now() / 1000
  return store.outbox
    .filter((item) => !item.staged && Number(item.dueAt) > 0)
    .filter((item) => Number(item.dueAt) > now)
    .slice()
    .sort((a, b) => Number(a.dueAt) - Number(b.dueAt))
})
const retryingOutbox = ref(false)

async function onRetryOutbox() {
  if (retryingOutbox.value) return
  retryingOutbox.value = true
  let ok = 0
  let failed = 0
  try {
    for (const item of [...stageOutboxItems.value]) {
      try {
        await store.retryOutbox(item.id)
        ok += 1
      } catch {
        failed += 1
      }
    }
    if (ok && failed) notify(`已发出 ${ok} 封，仍有 ${failed} 封未发出`, true)
    else if (ok) notify(`已发出 ${ok} 封`)
    else notify('仍然发送失败，请检查网络或账户设置', true)
  } finally {
    retryingOutbox.value = false
  }
}

/** 侧栏文件夹右键的「保留在侧栏 / 从侧栏隐藏」。 */
async function onPinFolder(name, pinned) {
  try {
    await store.setFolderPinned(name, pinned)
    notify(pinned ? '已保留在侧栏' : '已从侧栏隐藏')
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

/** 顶部全选框 / Ctrl+A：全选当前文件夹（不只是已加载的一页）。 */
async function onSelectAll(checked) {
  if (!checked) {
    store.clearSelection()
    return
  }
  try {
    const count = await store.selectAll(true)
    if (!count) notify('当前视图没有可选择的邮件')
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

/**
 * 删除并给一个「撤销」入口。
 *
 * 真正的服务器删除由 store 延后 5 秒提交（见 deleteRowsDeferred）。这里只负责
 * 把结果说清楚：删了几封、5 秒内可以点撤销；窗口过了再按 Ctrl+Z 会如实回答
 * 「已经提交，撤不回来了」。
 */
async function deleteRowsWithUndo(rows = null, note = '') {
  try {
    const batch = await store.deleteRowsDeferred(rows)
    if (!batch) return null
    notify(`已删除 ${batch.keys.length} 封${note ? `，${note}` : ''}`, Boolean(note), {
      label: '撤销',
      run: async () => {
        const undone = await store.undoDeleteDeferred(batch.id)
        if (undone) notify('已恢复')
        else notify('这些邮件已经提交删除，无法撤销', true)
      },
    })
    return batch
  } catch (error) {
    notify(String(error?.message || error), true)
    return null
  }
}

async function onDeleteRow(row) {
  await deleteRowsWithUndo([row])
}

/**
 * 顶栏「打开 .eml」：选一个本机保存的邮件文件来看。
 *
 * 「另存为 .eml」一直只有出口，存下来的文件想看还得换程序；这里补上入口。
 * 解析与清洗都在后端做（与收信完全同一条路径），前端只负责渲染。
 */
async function onOpenEml() {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const path = await open({
      title: '打开邮件文件',
      multiple: false,
      filters: [{ name: '邮件文件', extensions: ['eml', 'EML', 'msg'] }],
    })
    if (!path) return
    const detail = await invoke('mail_read_eml', { path })
    emlPath.value = path
    emlDetail.value = detail
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

async function onOpenMessage(row) {
  closeContextMenu()
  if (settings.value.listCollapsed) {
    store.saveSettings({ listCollapsed: false }).catch(() => {})
  }
  // 草稿：点开就是继续写。以前点开的是只读预览，用户没法改完再发，
  // 只能自己再新建一封把内容抄一遍。
  if (isDraftRow(row)) {
    await resumeDraft(row)
    return
  }
  // 聚合行：整个会话按封展开成独立卡片，而不是只打开最新一封再靠正文里的引用块。
  if (row?.threadUids?.length > 1) {
    await store.openThread(row)
    return
  }
  await store.openMessage(row)
}

/** 这一行是不是草稿箱里的草稿（按账户的文件夹角色判断，不依赖报文里的 Draft 标记）。 */
function isDraftRow(row) {
  if (!row?.accountId) return false
  const folders = store.foldersByAccount[row.accountId] || []
  return folders.some((folder) => folder.name === row.folder && folder.role === 'drafts')
}

/** 续写草稿：取回正文与收件人，交给独立写信窗口（与「写邮件」一致）。 */
async function resumeDraft(row) {
  await store.resumeDraft(row)
  if (!store.composing) {
    notify(store.lastError || '没能读取这封草稿，请检查账户连接', true)
    return
  }
  composeOpensInWindow.value = true
  openComposeInWindow(composeIdFor('resume', row))
    .catch(() => {})
    .finally(() => {
      composeOpensInWindow.value = false
    })
}

/** 点开会话聚合行时，先展开整个会话。 */
async function expandThread(row) {
  if (row?.threadUids?.length > 1) await store.loadThread(row)
}

/**
 * 「写邮件」默认在新窗口里编辑。
 *
 * 应用页的写信是一个覆盖层：打开后整个邮件页（列表、阅读区）都用不了，
 * 想对照原邮件、翻通讯录都得先关掉。独立窗口可以和主窗口并排，
 * 这也是 Outlook / Thunderbird 的默认行为。
 */
async function onCompose() {
  store.startCompose('new')
  await openComposeInWindow(composeIdFor('new'))
}

function onReply(detail, all = false, { newWindow = false } = {}) {
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
  // 默认在阅读区顶部就地回复；用户点了「在新窗口里写」才开窗。
  if (newWindow) {
    composeOpensInWindow.value = true
    openComposeInWindow(composeIdFor(all ? 'replyAll' : 'reply', detail))
      .catch(() => {})
      .finally(() => {
        composeOpensInWindow.value = false
      })
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

function onForward(detail, { newWindow = false } = {}) {
  store.startCompose('forward', {
    fromAddr: detail.fromAddr,
    fromName: detail.fromName,
    subject: detail.subject,
    toJson: '[]',
    ccJson: '[]',
    messageId: null,
  })
  // 转发要带上原文：以前只改了主题，编辑器里是空的，用户得自己复制粘贴。
  if (store.composing) {
    const quote = buildQuote(detail)
    store.composing.text = `\n\n${quote.text}`
    store.composing.html = `<br><br>${quote.html}`
  }
  if (newWindow) {
    composeOpensInWindow.value = true
    openComposeInWindow(composeIdFor('forward', detail))
      .catch(() => {})
      .finally(() => {
        composeOpensInWindow.value = false
      })
  }
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
  // 拍平后返回：被引用的这封邮件自己也可能带着历史卡片（回信套回信），
  // 直接包进新卡片就会叠成两三层、越往里缩得越深。拍平 = 新卡片里只留
  // 「这一封自己的正文」，更早的历史作为兄弟卡片依次排在下面。
  return { text: textBody, html: flattenQuoteHtml(htmlBody) }
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
  // 走 deleteRowsWithUndo：会话聚合行会按 threadUids 展开，删的是整条会话，
  // 与行上的删除按钮行为一致；5 秒内还能在提示条上撤销。
  await deleteRowsWithUndo([row])
}

async function onArchiveCurrent(detail) {
  const accountId = detail.accountId
  // 存档是账户的固定入口（后端同步时总会补齐）；万一还没有，先建一个再归档，
  // 而不是让用户为了「归档」按钮去侧栏手动建文件夹。
  let archive = store.folderForRole(accountId, 'archive')
  if (!archive) {
    try {
      // createFolder 内部已经刷新过文件夹表，这里只取结果。
      await store.createFolder('Archive')
      archive = store.folderForRole(accountId, 'archive')
    } catch (error) {
      notify(String(error?.message || error), true)
      return
    }
  }
  if (!archive) {
    notify('没法创建存档文件夹，请检查账户连接', true)
    return
  }
  try {
    await store.moveMessages([store.selected || detail], archive.name)
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
  // 打开后要下载 + 落盘 + 拉起系统程序，旧代码在这里没有任何反馈，
  // 服务器慢的时候用户会以为点击没生效而反复点。先给一条提示。
  notify(`正在打开「${attachment.filename}」…`)
  try {
    const dangerous = await invoke('mail_is_dangerous_attachment', {
      name: attachment.filename,
    }).catch(() => false)
    if (
      dangerous &&
      !(await askMailConfirm('打开它可能运行代码，请确认来源可信。', {
        title: `「${attachment.filename}」是脚本或可执行文件`,
        confirmText: '仍然打开',
      }))
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
  contextSubmenu.value = ''
  // 右键即刷新一次该账户的文件夹表：在别处（网页端 / 本机其它入口）新建的文件夹
  // 要能马上出现在「移动」子菜单里，而不是等下一轮同步。
  if (row.accountId) store.loadFolders(row.accountId, { refresh: true }).catch(() => {})
  setTimeout(() => document.addEventListener('click', closeContextMenu, { once: true }), 0)
}

function closeContextMenu() {
  contextMenu.value = null
  contextSubmenu.value = ''
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
    if (action === 'archive') {
      const archive = store.folderForRole(row.accountId, 'archive')
      if (!archive) {
        notify('该账户没有存档文件夹，可以先用侧栏的「＋」新建一个', true)
        return
      }
      await store.runBatch('move', archive.name, rows)
      notify(`已存档 ${rows.length} 封`)
      return
    }
    if (action === 'move' && target) {
      await store.runBatch('move', target, rows)
      notify(`已移动 ${rows.length} 封`)
      return
    }
    if (action === 'delete') {
      await deleteRowsWithUndo(rows)
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
/** 右键菜单里正在展开的子菜单（目前只有「移动」）。 */
const contextSubmenu = ref('')
/** 子菜单的视口坐标（fixed 定位，见模板里的说明）。 */
const submenuPos = ref({ x: 0, y: 0 })
let submenuTimer = null

function openSubmenu(name, event) {
  clearTimeout(submenuTimer)
  contextSubmenu.value = name
  const rect = event?.currentTarget?.getBoundingClientRect?.()
  if (!rect) return
  const height = Math.min(window.innerHeight * 0.6, 460)
  submenuPos.value = {
    x: Math.min(rect.right + 2, Math.max(8, window.innerWidth - 260)),
    y: Math.min(rect.top, Math.max(8, window.innerHeight - height - 8)),
  }
}

/** 鼠标从父项移进子菜单时会短暂离开父项：收到子菜单上要重新计时，否则点不到。 */
function keepSubmenu() {
  clearTimeout(submenuTimer)
  contextSubmenu.value = 'move'
}

/**
 * 收起子菜单前留一点延迟：鼠标斜着移过去会短暂离开两侧元素，
 * 立刻收起的话子菜单根本点不到。
 */
function scheduleSubmenuClose() {
  clearTimeout(submenuTimer)
  submenuTimer = setTimeout(() => {
    contextSubmenu.value = ''
  }, 220)
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

/** 列表行拖拽的完成动作（拖拽本身在 MailListPane / mailDragState 里）。 */

/** 列表动作条发出的批量命令。 */
async function onBatch(action, target) {
  if (action === 'delete') {
    await deleteRowsWithUndo()
    return
  }
  try {
    await store.runBatch(action, target)
    if (action === 'junk') notify('已移到垃圾邮件')
    if (action === 'move') notify('已移动')
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

/**
 * 拖到侧栏文件夹上：把正在拖的那批邮件移过去。
 *
 * 目标是「已删除」时按删除处理（走 delete 而不是普通 move，服务端才会打上
 * 删除标记、会话也按删除规则处理）；「垃圾邮件」同理。
 * 侧栏只显示当前账户的文件夹，所以跨账户拖拽（只在「全部邮件」聚合视图里可能出现）
 * 要把不属于这个账户的行剔除，并如实告诉用户。
 */
async function onDropOnFolder(folderName, dropped = null) {
  // 列表把「这一拖带的是哪几封」直接给过来；没有 rows 时退回当前勾选的集合。
  const rows = dropped?.length ? dropped : store.selectedRows()
  if (!rows.length) return
  const accountId = rows[0].accountId
  const folder = (store.foldersByAccount[accountId] || []).find((f) => f.name === folderName)
  const targets = rows.filter((row) => row.accountId === accountId)
  const skipped = rows.length - targets.length
  if (!targets.length) return
  // 拖到源文件夹本身没有意义。
  if (targets.every((row) => row.folder === folderName)) return
  const skippedNote = skipped ? `另有 ${skipped} 封属于其它账户，没有移动` : ''
  try {
    if (folder?.role === 'trash') {
      await deleteRowsWithUndo(targets, skippedNote)
      return
    }
    if (folder?.role === 'junk') {
      await store.runBatch('junk', null, targets)
      notify(`已移到垃圾邮件（${targets.length} 封）`)
    } else {
      await store.runBatch('move', folderName, targets)
      notify(`已移动 ${targets.length} 封到「${folder?.displayName || folderName}」`)
    }
    if (skippedNote) notify(skippedNote, true)
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

// ---------------- 发送 ----------------

/**
 * 草稿 id：同一份草稿重复打开会复用同一扇窗口。
 *
 * 新邮件用时间戳；回复 / 转发用「账户 + 源邮件 Message-Id」，
 * 这样同一封邮件反复点「回复」不会堆出一排窗口。
 */
function composeIdFor(mode, detail) {
  const accountId = store.activeAccountId || 'acct'
  const base = detail?.messageId || `${detail?.folder || ''}:${detail?.uid ?? ''}`
  const suffix = base ? `${mode}-${base}` : `${mode}-${Date.now()}`
  return `${accountId}:${suffix}`
}

/** 把主窗口的 composing 转成后端交接用的请求体。 */
function composeRequestOf(id, composing) {
  return {
    id,
    mode: composing.mode,
    accountId: composing.accountId,
    to: composing.to || [],
    cc: composing.cc || [],
    bcc: composing.bcc || [],
    subject: composing.subject || '',
    text: composing.text || '',
    html: composing.html || '',
    htmlMode: composing.htmlMode !== false,
    inReplyTo: composing.inReplyTo || null,
    references: composing.references || null,
    draftUid: composing.draftUid ?? null,
    draftFolder: composing.draftFolder || '',
    inlineImages: (composing.inlineImages || []).map((img) => ({
      contentId: img.contentId,
      path: img.path,
      name: img.name || '',
    })),
    attachments: (composing.attachments || []).map((a) => a.path),
    sourceMode: Boolean(composing.sourceMode),
  }
}

/**
 * 在新窗口里编辑这封草稿。
 *
 * 写信是持续几分钟的事，而应用页的编辑器是模态的——打开后在应用里做不了任何事，
 * 连回头看一眼原文都不行。所以默认开独立窗口。
 */
async function openComposeInWindow(id) {
  const composing = store.composing
  if (!composing) return
  try {
    await invoke('mail_open_compose_window', {
      request: composeRequestOf(id, composing),
      theme: store.theme?.mode || 'dark',
    })
    // 内容已经交给窗口，主窗口这份要收起来（否则关窗后 store 里还挂着一份旧的）。
    store.composing = null
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

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

/** 定时发送：进 outbox，由 store 的定时器在到期后投递。 */
async function onScheduleSend(at) {
  try {
    const result = await store.sendCompose({ scheduledAt: at })
    if (result?.queued) notify(`已安排在 ${scheduleLabel(result.queued)} 发送`)
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

function scheduleLabel(item) {
  const unix = Number(item?.dueAt)
  if (!unix) return '—'
  return new Date(unix * 1000).toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
}

async function onCancelScheduled(id) {
  try {
    await store.cancelOutbox(id)
    notify('已撤回定时邮件')
    if (!store.outbox.length) scheduledOpen.value = false
  } catch (error) {
    notify(String(error?.message || error), true)
  }
}

async function onSendScheduledNow(id) {
  try {
    const result = await store.retryOutbox(id)
    if (result?.errors?.length) notify(result.errors[0], true)
    else notify('已发送')
    if (!store.outbox.length) scheduledOpen.value = false
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

/**
 * 延迟发送（撤回窗口）可能发生在独立写信窗口里：主窗口从 outbox 看到
 * 「刚排队、还在窗口内」的那一条，接上倒计时条，撤回按钮才不会只对主窗口
 * 内联写信生效。普通「发送」仍然立即投递，不走这里。
 */
watch(
  () => store.outbox,
  (items) => {
    if (pendingSend.value) {
      if (!items.some((item) => item.id === pendingSend.value.id)) {
        clearInterval(pendingTimer)
        pendingSend.value = null
      }
      return
    }
    const windowSeconds = Number(settings.value.undoSendSeconds) || 0
    if (windowSeconds <= 0) return
    const now = Date.now() / 1000
    const candidate = items
      .filter(
        (item) =>
          !item.staged &&
          Number(item.dueAt) > now &&
          Number(item.dueAt) - now <= windowSeconds + 2,
      )
      .sort((a, b) => Number(a.dueAt) - Number(b.dueAt))[0]
    if (!candidate) return
    startPendingCountdown(candidate)
    notify(`邮件已延迟发送，${pendingSend.value?.remaining ?? windowSeconds} 秒内可撤回`)
  },
  { deep: true },
)

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

/**
 * 编辑器的定时自动保存：写进草稿箱。
 *
 * 与「保存草稿」按钮的区别是**不弹提示**——它每 15 秒跑一次，弹提示会刷屏。
 * 失败也只记日志：下一轮还会重试，断了提示反而打扰正在写信的人。
 *
 * 返回值是后端给出的 `{ folder, uid }`：编辑器记下它，下一次保存才能把上一份
 * 替换掉而不是再追加一封。这里不能依赖 `store.composing`——关闭编辑器的瞬间
 * 它已经是 null 了，而最后那次保存仍然要能落库。
 */
let autoSaveInflight = false

async function onAutoSaveToMailbox(request) {
  if (!request || autoSaveInflight) return null
  autoSaveInflight = true
  try {
    return await store.autoSaveDraft(request)
  } catch (error) {
    logger.warn('自动保存草稿失败', error)
    return null
  } finally {
    autoSaveInflight = false
  }
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

async function onAccountDeleted(id, deleteLocalMail = true) {
  await store.deleteAccount(id, { deleteLocalMail })
  notify(deleteLocalMail ? '账户已删除，本机邮件已一并清空' : '账户已删除，本机邮件仍保留')
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

/* 「移动」这类带子菜单的条目：右缘一个箭头，悬停时在右侧展开子菜单。
   子菜单不参与父菜单的滚动，所以父菜单要允许溢出（详见 .is-parent）。 */
.mail-context-item.is-parent {
  display: flex;
  align-items: center;
  gap: 6px;
  position: relative;
}
.mail-context-item.is-parent.is-open {
  background: var(--mail-panel-hover);
  color: var(--mail-text);
}
.mail-context-chevron {
  margin-left: auto;
  color: var(--mail-text-muted);
  font-size: 13px;
  line-height: 1;
}
.mail-context-hint {
  margin-left: auto;
  color: var(--mail-text-muted);
  font-size: 10.5px;
}
.mail-context-submenu {
  /* 与主菜单同级、fixed 定位：主菜单是 overflow-y:auto 的滚动容器，
     画在里面的绝对定位子菜单会被它裁掉——右键「移动」看不到文件夹就是这个原因。 */
  position: fixed;
  z-index: 61;
  min-width: 180px;
  max-width: 300px;
  max-height: min(60vh, 460px);
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-width: thin;
  padding: 4px;
  border: 1px solid var(--mail-border-strong);
  border-radius: 8px;
  background: var(--mail-panel);
  box-shadow: var(--mail-shadow);
  /* 子菜单用块级布局而不是从父项的 flex 里继承：条目仍是整行可点。 */
  display: block;
}
.mail-context-submenu .mail-context-item.is-current {
  color: var(--mail-accent);
}
.mail-context-empty {
  padding: 6px 10px;
  font-size: 11.5px;
  color: var(--mail-text-muted);
}

.mail-toast {
  position: fixed;
  left: 50%;
  bottom: 24px;
  transform: translateX(-50%);
  z-index: 70;
  display: flex;
  align-items: center;
  gap: 10px;
  max-width: 70vw;
  padding: 8px 16px;
  border-radius: 999px;
  background: var(--mail-text);
  color: var(--mail-bg);
  font-size: 12.5px;
  box-shadow: var(--mail-shadow);
}
.mail-toast-action {
  flex: 0 0 auto;
  padding: 2px 10px;
  border: 1px solid currentColor;
  border-radius: 999px;
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}
.mail-toast-action:hover {
  background: color-mix(in srgb, currentColor 16%, transparent);
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

.mail-scheduled {
  position: fixed;
  left: 50%;
  bottom: 122px;
  transform: translateX(-50%);
  z-index: 69;
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

.mail-schedule-modal {
  max-width: 620px;
  width: min(620px, 92vw);
}
.mail-schedule-list {
  display: flex;
  flex-direction: column;
  max-height: 60vh;
  overflow-y: auto;
  padding: 10px 16px 16px;
}
.mail-schedule-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 0;
  border-bottom: 1px solid var(--mail-border);
}
.mail-schedule-item:last-child {
  border-bottom: 0;
}
.mail-schedule-info {
  flex: 1 1 auto;
  min-width: 0;
}
.mail-schedule-subject {
  font-size: 13px;
  color: var(--mail-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mail-schedule-meta {
  margin-top: 2px;
  font-size: 11px;
  color: var(--mail-text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
