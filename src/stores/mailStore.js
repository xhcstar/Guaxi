// 邮箱客户端的状态层。
//
// 所有后端调用集中在这里，组件只读状态、调 action。这样做的原因是邮箱页的交互
// 分散在侧栏、列表、阅读器、编辑器四处，如果每处各自 invoke，错误处理与刷新时机
// 会很快失控（比如「已读」要在三个地方同步刷新）。
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { createLogger } from '../utils/logger'
import { resolveTheme } from '../components/ApplicationViewComponent/apps/mail/theme'

const logger = createLogger('MailStore')

/** 删除 keychain 里的账户口令（后端没有单独的删除命令时用「写入空串」不通用，
 * 这里需要真正的删除：调用 mail_delete_account 会连账户一起删，不适用。
 * 所以后端补一个 mail_delete_secret 命令，见 commands/mail/mod.rs。 */
async function deleteSecret(accountId) {
  try {
    await invoke('mail_delete_secret', { accountId })
  } catch (error) {
    logger.debug('清理临时凭据失败', error)
  }
}

const emptySettings = () => ({
  theme: 'ink',
  density: 'comfortable',
  readingFont: 'system',
  monoFont: 'Maple Mono NF CN',
  readingSize: 15,
  accentOverride: '',
  showHeaders: false,
  fetchMode: 'poll',
  pollSeconds: 90,
  snippetLen: 140,
  undoSendSeconds: 10,
  proxy: { mode: 'system', url: null, bypass: [] },
  groupThreads: true,
  rulesEnabled: true,
  retentionDays: 0,
  composeHtml: true,
  inlineImages: false,
  customThemes: [],
})

export const useMailStore = defineStore('mail', {
  state: () => ({
    ready: false,
    loading: false,
    settings: emptySettings(),
    builtinThemes: [],
    accounts: [],
    unread: {},
    foldersByAccount: {},
    /** 当前选中的账户；`null` 表示「全部账户」聚合视图。 */
    activeAccountId: null,
    activeFolder: 'INBOX',
    scope: 'all',
    messages: [],
    searchQuery: '',
    searchField: 'all',
    searchFilters: { unreadOnly: false, starredOnly: false, hasAttachment: false },
    labels: [],
    activeLabel: '',
    selected: null,
    detail: null,
    detailLoading: false,
    composing: null,
    outbox: [],
    rules: [],
    syncing: false,
    lastSyncAt: 0,
    lastError: '',
    pollTimer: null,
    unlisten: [],
  }),

  getters: {
    theme(state) {
      return resolveTheme(state.settings, state.builtinThemes)
    },
    isDark() {
      return this.theme?.mode === 'dark'
    },
    accountsById(state) {
      return Object.fromEntries(state.accounts.map((a) => [a.id, a]))
    },
    activeAccount(state) {
      return state.accounts.find((a) => a.id === state.activeAccountId) || null
    },
    folders(state) {
      if (state.activeAccountId) return state.foldersByAccount[state.activeAccountId] || []
      // 聚合视图：按账户顺序合并各自文件夹，用 accountId 区分。
      return state.accounts.flatMap((a) => state.foldersByAccount[a.id] || [])
    },
    currentFolder(state) {
      const list = state.activeAccountId
        ? state.foldersByAccount[state.activeAccountId] || []
        : []
      return list.find((f) => f.name === state.activeFolder) || null
    },
    totalUnread(state) {
      return Object.values(state.unread).reduce((sum, n) => sum + (n || 0), 0)
    },
    accountUnread(state) {
      return (id) => state.unread[id] || 0
    },
    selectedAccountLabel(state) {
      if (!state.activeAccountId) return '全部账户'
      return state.accounts.find((a) => a.id === state.activeAccountId)?.email || ''
    },
    hasAccounts(state) {
      return state.accounts.length > 0
    },
  },

  actions: {
    // ---------------- 初始化 ----------------

    async init() {
      if (this.ready) return
      this.loading = true
      try {
        const [settings, themes, accounts, unread, rules] = await Promise.all([
          invoke('mail_get_settings').catch(() => emptySettings()),
          invoke('mail_builtin_themes').catch(() => []),
          invoke('mail_list_accounts').catch(() => []),
          invoke('mail_unread_counts').catch(() => []),
          invoke('mail_list_rules').catch(() => []),
        ])
        this.settings = { ...emptySettings(), ...(settings || {}) }
        this.builtinThemes = themes || []
        this.accounts = accounts || []
        this.unread = Object.fromEntries((unread || []).map(([id, n]) => [id, n]))
        this.rules = rules || []

        if (this.accounts.length && !this.activeAccountId) {
          this.activeAccountId = this.accounts[0].id
        }
        await Promise.all(this.accounts.map((a) => this.loadFolders(a.id)))
        this.ready = true
        this.lastSyncAt = Date.now()
        await this.loadMessages({ silent: true })
        this.startPolling()
        await this.bindEvents()
        this.refreshLabels().catch(() => {})
        await this.flushOutbox()
      } catch (error) {
        logger.error('邮箱初始化失败', error)
        this.lastError = String(error?.message || error)
      } finally {
        this.loading = false
      }
    },

    async bindEvents() {
      if (this.unlisten.length) return
      try {
        const offSync = await listen('mail-sync-progress', () => {
          this.lastSyncAt = Date.now()
        })
        const offNew = await listen('mail-new-messages', async () => {
          await this.reloadUnread()
          await this.loadMessages({ silent: true })
        })
        this.unlisten.push(offSync, offNew)
      } catch (error) {
        logger.warn('邮箱事件监听失败', error)
      }
    },

    dispose() {
      this.stopPolling()
      for (const off of this.unlisten) {
        try {
          off?.()
        } catch {
          /* 解绑失败不影响退出 */
        }
      }
      this.unlisten = []
    },

    startPolling() {
      this.stopPolling()
      const seconds = Math.max(30, Number(this.settings.pollSeconds) || 90)
      this.pollTimer = setInterval(async () => {
        if (document.hidden || this.syncing) return
        await this.syncAll({ quiet: true })
      }, seconds * 1000)
    },

    stopPolling() {
      if (this.pollTimer) {
        clearInterval(this.pollTimer)
        this.pollTimer = null
      }
    },

    // ---------------- 账户 ----------------

    async saveAccount(account, secret) {
      const saved = await invoke('mail_save_account', { account, secret: secret || null })
      const index = this.accounts.findIndex((a) => a.id === saved.id)
      if (index >= 0) this.accounts.splice(index, 1, saved)
      else this.accounts.push(saved)
      if (!this.activeAccountId) this.activeAccountId = saved.id
      await this.loadFolders(saved.id)
      return saved
    },

    async deleteAccount(id) {
      await invoke('mail_delete_account', { accountId: id })
      this.accounts = this.accounts.filter((a) => a.id !== id)
      delete this.foldersByAccount[id]
      delete this.unread[id]
      if (this.activeAccountId === id) {
        this.activeAccountId = this.accounts[0]?.id || null
        this.messages = []
        this.detail = null
        this.selected = null
      }
    },

    async hasSecret(accountId) {
      try {
        return await invoke('mail_has_secret', { accountId })
      } catch {
        return false
      }
    },

    /**
     * 连接诊断。新建账户时表单里的口令还没进 keychain，这里先临时写一份
     * （用真实 id 或临时 id），诊断完按需删除，避免留下孤儿凭据。
     */
    async testAccount(account, secret) {
      let tempId = null
      let target = account
      if (!account.id && secret && secret.trim()) {
        // 没有 id 的账户没法写 keychain；给一个临时 id，诊断完删掉。
        tempId = `probe-${Date.now()}`
        target = { ...account, id: tempId }
      }
      try {
        return await invoke('mail_test_account', { account: target, secret: secret?.trim() || null })
      } finally {
        if (tempId) await deleteSecret(tempId)
      }
    },

    selectAccount(id) {
      this.activeAccountId = id
      const folders = this.foldersByAccount[id] || []
      const inbox = folders.find((f) => f.role === 'inbox') || folders[0]
      this.activeFolder = inbox?.name || 'INBOX'
      this.scope = 'all'
      this.activeLabel = ''
      this.detail = null
      this.selected = null
      this.loadMessages()
    },

    // ---------------- 文件夹 ----------------

    async loadFolders(accountId, { refresh = false } = {}) {
      try {
        const folders = refresh
          ? await invoke('mail_refresh_folders', { accountId })
          : await invoke('mail_list_folders', { accountId })
        this.foldersByAccount = { ...this.foldersByAccount, [accountId]: folders || [] }
        const inbox = (folders || []).find((f) => f.role === 'inbox')
        if (inbox && accountId === this.activeAccountId && !this.activeFolder) {
          this.activeFolder = inbox.name
        }
      } catch (error) {
        logger.warn('加载文件夹失败', accountId, error)
      }
    },

    async refreshFolders() {
      await Promise.all(this.accounts.map((a) => this.loadFolders(a.id, { refresh: true })))
      await this.reloadUnread()
    },

    selectFolder(name) {
      this.activeFolder = name
      // `folder` 是独立的 scope：`all` 表示「跨文件夹全部邮件」，两者不能混用。
      this.scope = 'folder'
      this.activeLabel = ''
      this.detail = null
      this.selected = null
      this.loadMessages()
    },

    selectScope(scope, label = '') {
      this.scope = scope
      this.activeLabel = label
      this.detail = null
      this.selected = null
      this.loadMessages()
    },

    async createFolder(name) {
      if (!this.activeAccountId) throw new Error('请先选择账户')
      await invoke('mail_create_folder', { accountId: this.activeAccountId, name })
      await this.loadFolders(this.activeAccountId, { refresh: true })
    },

    // ---------------- 邮件列表 ----------------

    async reloadUnread() {
      try {
        const unread = await invoke('mail_unread_counts')
        this.unread = Object.fromEntries((unread || []).map(([id, n]) => [id, n]))
      } catch (error) {
        logger.warn('刷新未读数失败', error)
      }
    },

    async loadMessages({ silent = false } = {}) {
      if (!this.ready && !silent) return
      const searching =
        Boolean(this.searchQuery.trim()) ||
        this.searchFilters.unreadOnly ||
        this.searchFilters.starredOnly ||
        this.searchFilters.hasAttachment ||
        Boolean(this.activeLabel) ||
        this.scope === 'all'

      const query = {
        accountIds: this.activeAccountId ? [this.activeAccountId] : [],
        folder: this.scope === 'folder' ? this.activeFolder : null,
        scope: this.scope,
        text: this.searchQuery.trim(),
        field: this.searchField,
        labels: this.activeLabel ? [this.activeLabel] : [],
        unreadOnly: this.searchFilters.unreadOnly ? true : null,
        starredOnly: this.searchFilters.starredOnly ? true : null,
        hasAttachment: this.searchFilters.hasAttachment ? true : null,
        limit: 300,
      }

      try {
        const rows = await invoke('mail_search', { query })
        this.messages = await this.groupThreadsIfNeeded(rows || [])
      } catch (error) {
        // 搜索条件本身可能让 SQL 失效（极端输入）；退回列表接口，避免整页空白。
        logger.warn('检索失败，回退到列表', error)
        if (this.activeAccountId && this.scope === 'all') {
          try {
            const rows = await invoke('mail_list_messages', {
              accountId: this.activeAccountId,
              folder: this.activeFolder,
              limit: 200,
              offset: 0,
            })
            this.messages = rows || []
          } catch (fallbackError) {
            logger.error('加载邮件失败', fallbackError)
            this.messages = []
          }
        } else {
          this.messages = []
        }
        if (searching) this.lastError = String(error?.message || error)
      }
    },

    async groupThreadsIfNeeded(rows) {
      if (!this.settings.groupThreads) return rows
      // 只在「全部邮件」这类混合视图里聚合；搜索与按文件夹查看时用户要的是逐封结果。
      if (this.searchQuery.trim() || this.scope !== 'all') return rows

      const buckets = new Map()
      for (const row of rows) {
        const key = row.threadKey || `${row.accountId}:${row.folder}:${row.uid}`
        const bucket = buckets.get(key)
        if (bucket) bucket.push(row)
        else buckets.set(key, [row])
      }

      const out = []
      for (const bucket of buckets.values()) {
        if (bucket.length === 1) {
          out.push({ ...bucket[0], threadCount: 1, threadUids: bucket })
          continue
        }
        // 代表行取最新一封；聚合状态取并集（有未读就算未读，有星标就算星标）。
        const newest = bucket.reduce((a, b) => (b.date > a.date ? b : a))
        out.push({
          ...newest,
          seen: bucket.every((m) => m.seen),
          flagged: bucket.some((m) => m.flagged),
          hasAttachments: bucket.some((m) => m.hasAttachments),
          threadCount: bucket.length,
          threadUids: bucket,
        })
      }
      out.sort((a, b) => b.date - a.date)
      return out
    },

    async loadThread(row) {
      if (!row?.threadUids || row.threadUids.length < 2) return
      try {
        const rows = await invoke('mail_thread', { threadKey: row.threadKey })
        if (rows?.length) {
          this.messages = rows
        }
      } catch (error) {
        logger.warn('加载会话失败', error)
      }
    },

    async syncNow() {
      if (!this.activeAccountId) return { added: 0, errors: [] }
      this.syncing = true
      try {
        const rows = await invoke('mail_sync_folder', {
          accountId: this.activeAccountId,
          folder: this.activeFolder,
        })
        await this.loadFolders(this.activeAccountId)
        await this.reloadUnread()
        await this.loadMessages({ silent: true })
        this.lastSyncAt = Date.now()
        return { added: rows?.length || 0, errors: [] }
      } catch (error) {
        logger.error('同步失败', error)
        this.lastError = String(error?.message || error)
        return { added: 0, errors: [this.lastError] }
      } finally {
        this.syncing = false
      }
    },

    async syncAll({ quiet = false } = {}) {
      if (this.syncing) return
      this.syncing = true
      try {
        const result = await invoke('mail_sync_all')
        await this.refreshFolders()
        await this.loadMessages({ silent: true })
        await this.flushOutbox()
        this.lastSyncAt = Date.now()
        if (!quiet && result?.errors?.length) {
          this.lastError = result.errors.join('\n')
        }
        return result
      } catch (error) {
        logger.warn('全量同步失败', error)
        if (!quiet) this.lastError = String(error?.message || error)
        return { added: 0, errors: [String(error)] }
      } finally {
        this.syncing = false
      }
    },

    // ---------------- 阅读 ----------------

    async openMessage(row) {
      this.selected = row
      this.detailLoading = true
      try {
        const detail = await invoke('mail_get_message', {
          accountId: row.accountId,
          folder: row.folder,
          uid: row.uid,
        })
        this.detail = detail
        // 本地列表同步标记已读，不再等下一次同步。
        const target = this.messages.find(
          (m) => m.accountId === row.accountId && m.folder === row.folder && m.uid === row.uid,
        )
        if (target) target.seen = true
        if (row.threadUids) {
          for (const item of row.threadUids) {
            const match = this.messages.find((m) => m === item)
            if (match) match.seen = true
          }
        }
        await this.reloadUnread()
      } catch (error) {
        logger.error('打开邮件失败', error)
        this.lastError = String(error?.message || error)
        this.detail = null
      } finally {
        this.detailLoading = false
      }
    },

    async setSeen(rows, seen) {
      for (const row of rows) {
        try {
          await invoke('mail_set_seen', {
            accountId: row.accountId,
            folder: row.folder,
            uids: [row.uid],
            seen,
          })
        } catch (error) {
          logger.warn('标记已读失败', error)
        }
      }
      for (const row of rows) {
        const match = this.messages.find(
          (m) => m.accountId === row.accountId && m.folder === row.folder && m.uid === row.uid,
        )
        if (match) match.seen = seen
      }
      await this.reloadUnread()
    },

    async setFlagged(row, flagged) {
      try {
        await invoke('mail_set_flagged', {
          accountId: row.accountId,
          folder: row.folder,
          uids: [row.uid],
          flagged,
        })
        row.flagged = flagged
        if (this.detail?.uid === row.uid && this.detail?.folder === row.folder) {
          this.detail.flagged = flagged
        }
      } catch (error) {
        logger.warn('星标失败', error)
        this.lastError = String(error?.message || error)
      }
    },

    async moveMessages(rows, target) {
      for (const row of rows) {
        await invoke('mail_move_messages', {
          accountId: row.accountId,
          folder: row.folder,
          uids: [row.uid],
          target,
        })
      }
      this.afterRemoval(rows)
    },

    async deleteMessages(rows) {
      for (const row of rows) {
        await invoke('mail_delete_messages', {
          accountId: row.accountId,
          folder: row.folder,
          uids: [row.uid],
        })
      }
      this.afterRemoval(rows)
    },

    afterRemoval(rows) {
      const keys = new Set(rows.map((r) => `${r.accountId}:${r.folder}:${r.uid}`))
      this.messages = this.messages.filter(
        (m) => !keys.has(`${m.accountId}:${m.folder}:${m.uid}`),
      )
      if (this.selected) {
        this.selected = null
        this.detail = null
      }
      this.loadFolders(this.activeAccountId).catch(() => {})
      this.reloadUnread().catch(() => {})
    },

    async saveAttachment(detail, attachment, targetDir) {
      return invoke('mail_save_attachment', {
        accountId: detail.accountId,
        folder: detail.folder,
        uid: detail.uid,
        part: attachment.part,
        targetDir,
        filename: attachment.filename,
      })
    },

    // ---------------- 搜索 ----------------

    setSearch(text) {
      this.searchQuery = text
    },

    async runSearch() {
      await this.loadMessages()
    },

    clearSearch() {
      this.searchQuery = ''
      this.searchFilters = { unreadOnly: false, starredOnly: false, hasAttachment: false }
      this.activeLabel = ''
      this.loadMessages()
    },

    async suggest(prefix) {
      if (!prefix || prefix.length < 2) return []
      try {
        return await invoke('mail_suggest', { prefix })
      } catch {
        return []
      }
    },

    async refreshLabels() {
      try {
        const labels = await invoke('mail_labels')
        this.labels = labels || []
      } catch (error) {
        logger.warn('加载分类失败', error)
      }
    },

    // ---------------- 写信 ----------------

    startCompose(mode = 'new', source = null) {
      const account = this.activeAccount || this.accounts[0]
      if (!account) return
      this.composing = buildCompose(mode, source, account, this.settings)
    },

    closeCompose() {
      this.composing = null
    },

    async sendCompose({ useOutbox = false } = {}) {
      if (!this.composing) return
      const request = {
        accountId: this.composing.accountId,
        to: this.composing.to,
        cc: this.composing.cc,
        bcc: this.composing.bcc,
        subject: this.composing.subject,
        text: this.composing.text,
        html: this.composing.htmlMode ? this.composing.html : null,
        inReplyTo: this.composing.inReplyTo,
        references: this.composing.references,
        attachments: this.composing.attachments.map((a) => a.path),
        saveToSent: true,
      }
      if (useOutbox && this.settings.undoSendSeconds > 0) {
        const item = await invoke('mail_queue_send', { request })
        this.outbox.push(item)
        this.composing = null
        return { queued: item }
      }
      await invoke('mail_send', { request })
      this.composing = null
      return { sent: true }
    },

    async saveDraft() {
      if (!this.composing) return
      const request = {
        accountId: this.composing.accountId,
        to: this.composing.to,
        cc: this.composing.cc,
        bcc: this.composing.bcc,
        subject: this.composing.subject,
        text: this.composing.text,
        html: this.composing.htmlMode ? this.composing.html : null,
        inReplyTo: this.composing.inReplyTo,
        references: this.composing.references,
        attachments: this.composing.attachments.map((a) => a.path),
        saveToSent: true,
      }
      await invoke('mail_save_draft', { request })
      this.composing = null
    },

    async loadOutbox() {
      try {
        this.outbox = (await invoke('mail_list_outbox')) || []
      } catch (error) {
        logger.warn('加载待发箱失败', error)
      }
    },

    async cancelOutbox(id) {
      await invoke('mail_cancel_outbox', { id })
      this.outbox = this.outbox.filter((item) => item.id !== id)
    },

    /** 启动与轮询时把到期的待发邮件投出去（撤回窗口结束后真正发送）。 */
    async flushOutbox() {
      try {
        const result = await invoke('mail_flush_outbox')
        await this.loadOutbox()
        return result
      } catch (error) {
        logger.warn('投递待发邮件失败', error)
        return { sent: 0, errors: [String(error)] }
      }
    },

    // ---------------- 规则 ----------------

    async loadRules() {
      try {
        this.rules = (await invoke('mail_list_rules')) || []
      } catch (error) {
        logger.warn('加载规则失败', error)
      }
    },

    async saveRule(rule) {
      const saved = await invoke('mail_save_rule', { rule })
      const index = this.rules.findIndex((r) => r.id === saved.id)
      if (index >= 0) this.rules.splice(index, 1, saved)
      else this.rules.push(saved)
      this.rules.sort((a, b) => a.priority - b.priority)
      return saved
    },

    async deleteRule(id) {
      await invoke('mail_delete_rule', { id })
      this.rules = this.rules.filter((r) => r.id !== id)
    },

    // ---------------- 设置 ----------------

    async saveSettings(patch) {
      this.settings = { ...this.settings, ...patch }
      await invoke('mail_set_settings', { settings: this.settings })
      if ('pollSeconds' in patch) this.startPolling()
    },
  },
})

/** 根据模式生成初始草稿。 */
function buildCompose(mode, source, account, settings) {
  const base = {
    mode,
    accountId: account.id,
    to: [],
    cc: [],
    bcc: [],
    subject: '',
    text: '',
    html: '',
    htmlMode: settings.composeHtml !== false,
    attachments: [],
    inReplyTo: null,
    references: null,
    showCc: false,
    showBcc: false,
  }
  if (!source) return base

  const from = source.fromAddr || ''
  const isMine = from === account.email
  if (mode === 'reply' || mode === 'replyAll') {
    // 自己发的邮件点「回复」应当回给原收件人，而不是自己。
    const primary = isMine ? source.toJson : from
    base.to = parseAddressList(primary)
    if (mode === 'replyAll') {
      base.cc = parseAddressList(source.ccJson).filter(
        (addr) => !addr.includes(account.email) && !base.to.includes(addr),
      )
      base.showCc = base.cc.length > 0
    }
    base.subject = replySubject(source.subject)
    base.inReplyTo = source.messageId || null
    // 引用链由调用方给出完整串（含原链）；没有时退回本条 Message-Id。
    base.references = source.references || source.messageId || null
    return base
  }
  if (mode === 'forward') {
    base.subject = forwardSubject(source.subject)
    return base
  }
  return base
}

/** 把 `[{name,address}]` 的 JSON 字符串转成 "Name <addr>" 列表。 */
export function parseAddressList(json) {
  let list = json
  if (typeof json === 'string') {
    try {
      list = JSON.parse(json || '[]')
    } catch {
      return []
    }
  }
  if (!Array.isArray(list)) return []
  return list
    .map((item) => {
      if (typeof item === 'string') return item
      const { name, address } = item || {}
      if (!address) return ''
      return name ? `${name} <${address}>` : address
    })
    .filter(Boolean)
}

export function replySubject(subject) {
  const text = String(subject || '').trim()
  const lower = text.toLowerCase()
  if (lower.startsWith('re:') || lower.startsWith('re：') || lower.startsWith('回复')) return text
  return `Re: ${text}`
}

export function forwardSubject(subject) {
  const text = String(subject || '').trim()
  const lower = text.toLowerCase()
  if (lower.startsWith('fw:') || lower.startsWith('fwd:') || lower.startsWith('转发')) return text
  return `Fwd: ${text}`
}
