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

/** 正在同步的文件夹（`accountId:folder`），避免切来切去时并发拉同一个文件夹。 */
const inflightFolderSyncs = new Set()

/**
 * 每个账户最近一次文件夹请求的序号。
 *
 * 刷新要走一次服务器 LIST，比读本地慢；新建文件夹后再触发刷新时，**先发**的那次
 * 可能后到，把刚建好的目录又盖回旧列表——侧栏就会「出现 → 消失 → 又出现」。
 * 只让最后一次请求写 state。
 */
const foldersRequestSeq = new Map()

/** 操作后的计数校正：合并短时间内的多次操作，避免每次都打一轮完整 STATUS。 */
const folderRefreshTimers = new Map()

function scheduleFolderRefresh(store, accountId) {
  if (!accountId) return
  clearTimeout(folderRefreshTimers.get(accountId))
  const timer = setTimeout(() => {
    folderRefreshTimers.delete(accountId)
    store
      .loadFolders(accountId, { refresh: true })
      .then(() => store.reloadUnread())
      .catch(() => {})
  }, 4000)
  folderRefreshTimers.set(accountId, timer)
}

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
  sidebarCollapsed: false,
  listCollapsed: false,
  sidebarWidth: 232,
  listWidth: 340,
  rulesEnabled: true,
  retentionDays: 0,
  composeHtml: true,
  inlineImages: false,
  customThemes: [],
  // 可靠性相关的新设置
  notifyNewMail: true,
  attachmentLimitMb: 25,
  flagSyncMinutes: 10,
  offlineQueue: true,
  pageSize: 100,
  virtualList: true,
  warnDangerousAttachments: true,
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
    /** 会话视图：同一 thread 内每封邮件的详情（按时间升序）。 */
    thread: [],
    detailLoading: false,
    composing: null,
    outbox: [],
    rules: [],
    syncing: false,
    searching: false,
    /** 批量选择：{ accountId, folder, uid } 的唯一键集合。 */
    selectedKeys: [],
    searchError: '',
    lastSyncAt: 0,
    lastError: '',
    pollTimer: null,
    outboxTimer: null,
    unlisten: [],
    /** 已加载的页码（「加载更多」按页累加，不再硬编码 300 条）。 */
    pageCount: 1,
    hasMore: false,
    loadingMore: false,
    /** 搜索序号：快速输入时丢弃过期结果。 */
    searchSeq: 0,
    /** 离线队列条数 / 待发邮件数（顶栏状态条）。 */
    offlineQueued: 0,
    pendingSend: 0,
    idleActive: false,
    /** 模板与收发件人名单（设置页 / 写信页共用）。 */
    templates: [],
    senderRules: [],
    /** 当前账户的 MFA / OAuth 状态。 */
    oauthStatus: null,
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
        const [settings, themes, accounts, unread, rules, templates, senderRules] = await Promise.all([
          invoke('mail_get_settings').catch(() => emptySettings()),
          invoke('mail_builtin_themes').catch(() => []),
          invoke('mail_list_accounts').catch(() => []),
          invoke('mail_unread_counts').catch(() => []),
          invoke('mail_list_rules').catch(() => []),
          invoke('mail_list_templates').catch(() => []),
          invoke('mail_list_sender_rules').catch(() => []),
        ])
        this.settings = { ...emptySettings(), ...(settings || {}) }
        this.builtinThemes = themes || []
        this.accounts = accounts || []
        this.unread = Object.fromEntries((unread || []).map(([id, n]) => [id, n]))
        this.rules = rules || []
        this.templates = templates || []
        this.senderRules = senderRules || []

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
        // 启动时先把上一次断网期间排队的操作重放掉，再去看 IDLE。
        this.replayOffline().catch(() => {})
        this.startIdle().catch(() => {})
        // 共享邮箱：启动时把已配置的委托邮箱重新展开一次（服务器上可能新增了文件夹）。
        for (const account of this.accounts) {
          if (account.delegate?.length) this.expandDelegates(account.id).catch(() => {})
        }
        this.refreshConnection().catch(() => {})
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
        // 从任务页 / 其它模块跳回原邮件：切到对应账户与文件夹，再打开它。
        const offOrigin = await listen('mail-open-origin', async (event) => {
          const payload = event?.payload || {}
          await this.revealMessage(payload.accountId, payload.folder, payload.uid)
        })
        // IMAP IDLE：服务器说「有变化」就做一次增量同步。
        // 这里不直接刷新列表，而是走 syncNow —— 列表数据来自本地库，
        // 必须先落库再刷新，否则会读到旧数据。
        const offIdle = await listen('mail-idle-change', async (event) => {
          const payload = event?.payload || {}
          if (payload.accountId && payload.accountId !== this.activeAccountId) {
            // 其它账户的变化：只更新未读徽标，不打断当前阅读。
            this.reloadUnread().catch(() => {})
            return
          }
          await this.syncNow().catch(() => {})
        })
        this.unlisten.push(offSync, offNew, offOrigin, offIdle)
      } catch (error) {
        logger.warn('邮箱事件监听失败', error)
      }
    },

    dispose() {
      this.stopPolling()
      if (this.outboxTimer) {
        clearTimeout(this.outboxTimer)
        this.outboxTimer = null
      }
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

    async saveAccount(account, secret, { loadFolders = true } = {}) {
      const saved = await invoke('mail_save_account', { account, secret: secret || null })
      const index = this.accounts.findIndex((a) => a.id === saved.id)
      if (index >= 0) this.accounts.splice(index, 1, saved)
      else this.accounts.push(saved)
      if (!this.activeAccountId) this.activeAccountId = saved.id
      // 设备码授权还没完成时先不连服务器：此时既没有令牌，拉文件夹只会报
      // 「还没有完成 OAuth 授权」，把控制台刷满。授权成功后由调用方补一次。
      if (loadFolders) {
        await this.loadFolders(saved.id)
        // 共享邮箱需要在保存后展开一次，否则侧栏里看不到它们的文件夹。
        this.expandDelegates(saved.id).catch(() => {})
      }
      return saved
    },

    /**
     * 删除账户。
     *
     * `deleteLocalMail` 由用户在确认框里选：勾上就连本机缓存的邮件 / 正文 /
     * 附件缓存 / 草稿一起清掉（「不留任何痕迹」），否则只移除账户本身。
     */
    async deleteAccount(id, { deleteLocalMail = true } = {}) {
      await invoke('mail_delete_account', { accountId: id, deleteLocalMail })
      // 以库里的实际结果为准：后端若没真的删掉，界面不能自己把它抹了，
      // 否则用户看到「删了」、下次启动它又回来了。
      const remaining = await invoke('mail_list_accounts').catch(() => null)
      if (remaining) {
        if (remaining.some((account) => account.id === id)) {
          throw new Error('后端没有删除这个账户，请稍后重试')
        }
        this.accounts = remaining
      } else {
        this.accounts = this.accounts.filter((a) => a.id !== id)
      }
      const wasActive = this.activeAccountId === id
      delete this.foldersByAccount[id]
      delete this.unread[id]
      // 这一页的列表里可能混着该账户的行：先摘掉，再把当前账户的信重新拉一次。
      this.messages = this.messages.filter((m) => m.accountId !== id)
      if (wasActive) {
        this.activeAccountId = this.accounts[0]?.id || null
        this.detail = null
        this.thread = []
        this.selected = null
        this.messages = []
        await this.loadMessages({ silent: true }).catch(() => {})
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
      this.thread = []
      this.selected = null
      this.loadMessages()
      // 切账户时顺手把收件箱拉到最新：列表和侧栏计数不该停留在上一次同步。
      this.syncFolders([this.activeFolder])
    },

    // ---------------- 文件夹 ----------------

    async loadFolders(accountId, { refresh = false } = {}) {
      const seq = (foldersRequestSeq.get(accountId) || 0) + 1
      foldersRequestSeq.set(accountId, seq)
      try {
        const folders = refresh
          ? await invoke('mail_refresh_folders', { accountId })
          : await invoke('mail_list_folders', { accountId })
        // 更晚发出的请求已经回来了：这一份是旧数据，丢掉，别把新列表盖回去。
        if (foldersRequestSeq.get(accountId) !== seq) return
        this.foldersByAccount = { ...this.foldersByAccount, [accountId]: folders || [] }
        const inbox = (folders || []).find((f) => f.role === 'inbox')
        if (inbox && accountId === this.activeAccountId && !this.activeFolder) {
          this.activeFolder = inbox.name
        }
      } catch (error) {
        logger.warn('加载文件夹失败', accountId, error)
      }
    },

    /** 展开某个共享邮箱的文件夹（账户设置里配了 delegate 之后调用）。 */
    async loadDelegateFolders(accountId, mailbox) {
      const folders = await invoke('mail_list_delegate_folders', { accountId, mailbox })
      await this.loadFolders(accountId)
      return folders || []
    },

    /** 把账户里配置的所有共享邮箱都展开（保存账户 / 启动时）。 */
    async expandDelegates(accountId) {
      const account = this.accounts.find((a) => a.id === accountId)
      if (!account?.delegate?.length) return []
      const out = []
      for (const item of account.delegate) {
        if (!item?.email) continue
        try {
          out.push(...(await this.loadDelegateFolders(accountId, item.email)))
        } catch (error) {
          logger.warn('展开共享邮箱失败', item.email, error)
        }
      }
      return out
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
      this.thread = []
      this.selected = null
      this.loadMessages()
      // 本地库里可能还没同步过这个文件夹（列表会是空的，计数也不准）。
      this.syncFolders([name])
    },

    selectScope(scope, label = '') {
      this.scope = scope
      this.activeLabel = label
      this.detail = null
      this.thread = []
      this.selected = null
      this.loadMessages()
      // 按角色聚合的视图同样要先同步：垃圾箱 / 已删除往往从没拉过邮件。
      if (['inbox', 'sent', 'drafts', 'trash', 'junk', 'archive'].includes(scope)) {
        const names = (this.foldersByAccount[this.activeAccountId] || [])
          .filter((f) => f.role === scope)
          .map((f) => f.name)
        this.syncFolders(names)
      }
    },

    /** 后台增量同步若干文件夹，完成后刷新本地目录计数与列表。 */
    async syncFolders(names) {
      const accountId = this.activeAccountId
      if (!accountId) return
      const targets = [...new Set(names || [])].filter(Boolean).slice(0, 4)
      let changed = false
      for (const name of targets) {
        const key = `${accountId}:${name}`
        if (inflightFolderSyncs.has(key)) continue
        inflightFolderSyncs.add(key)
        try {
          await invoke('mail_sync_folder', { accountId, folder: name })
          changed = true
        } catch (error) {
          logger.warn('同步文件夹失败', name, error)
        } finally {
          inflightFolderSyncs.delete(key)
        }
      }
      if (!changed) return
      // 同步只补邮件正文/列表；计数仍以服务端 STATUS（folders 表）为准，不动它。
      await this.loadFolders(accountId)
      await this.reloadUnread()
      if (this.activeAccountId === accountId) {
        this.loadMessages({ silent: true })
      }
    },

    /** 本地乐观调整文件夹计数：操作后角标先动，后台再用服务端结果校正。 */
    adjustFolderCount(accountId, folderName, { total = 0, unread = 0 } = {}) {
      const list = this.foldersByAccount[accountId]
      if (!list?.length || !folderName) return
      const folder = list.find((f) => f.name === folderName)
      if (!folder) return
      folder.total = Math.max(0, Number(folder.total || 0) + total)
      folder.unread = Math.max(0, Number(folder.unread || 0) + unread)
      this.foldersByAccount = { ...this.foldersByAccount, [accountId]: [...list] }
    },

    /** 删除自建文件夹（服务器 + 本地缓存）。 */
    /** 文件夹「保留在侧栏」：服务器带下来的模板目录默认不显示，用户可以手动留下。 */
    async setFolderPinned(name, pinned) {
      if (!this.activeAccountId) return
      await invoke('mail_set_folder_pinned', {
        accountId: this.activeAccountId,
        name,
        pinned,
      })
      const list = this.foldersByAccount[this.activeAccountId] || []
      const folder = list.find((f) => f.name === name)
      if (folder) folder.userCreated = pinned
    },

    async deleteFolder(name) {
      if (!this.activeAccountId) throw new Error('请先选择账户')
      const accountId = this.activeAccountId
      const before = this.foldersByAccount[accountId] || []
      const wasActive = this.activeFolder === name
      const previousFolder = this.activeFolder
      const previousScope = this.scope
      // 先移除本地行：IMAP 删除可能要等服务器几百毫秒到几秒，界面不能一直没有任何反应。
      const after = before.filter((f) => f.name !== name)
      this.foldersByAccount = { ...this.foldersByAccount, [accountId]: after }
      if (wasActive) {
        const inbox = after.find((f) => f.role === 'inbox')
        this.activeFolder = inbox?.name || 'INBOX'
        this.scope = 'all'
        this.loadMessages()
      }
      try {
        await invoke('mail_delete_folder', { accountId, name })
        // 后端已删掉本地行；读一次本地表，避免期间的后台刷新把旧文件夹又带回来。
        await this.loadFolders(accountId)
        await this.reloadUnread()
      } catch (error) {
        // 服务器拒绝（离线、系统文件夹、权限不足）时把侧栏恢复原样。
        this.foldersByAccount = { ...this.foldersByAccount, [accountId]: before }
        if (wasActive) {
          this.activeFolder = previousFolder
          this.scope = previousScope
          this.loadMessages()
        }
        throw error
      }
    },

    async createFolder(name) {
      if (!this.activeAccountId) throw new Error('请先选择账户')
      const accountId = this.activeAccountId
      const clean = String(name || '').trim()
      if (!clean) throw new Error('文件夹名不能为空')
      const before = this.foldersByAccount[accountId] || []
      const existing = before.find((f) => f.name === clean)
      if (!existing) {
        // 乐观插入：服务器 CREATE 返回前侧栏就能看到新文件夹。
        const optimistic = {
          accountId,
          name: clean,
          displayName: clean,
          delimiter: null,
          role: 'custom',
          unread: 0,
          total: 0,
          uidValidity: 0,
          uidNext: 0,
          selectable: true,
          localOnly: false,
          userCreated: true,
        }
        this.foldersByAccount = { ...this.foldersByAccount, [accountId]: [...before, optimistic] }
      }
      try {
        await invoke('mail_create_folder', { accountId, name: clean })
        // 只读本地库：服务器创建成功后后端已经补了本地行，不必再 LIST 一次。
        await this.loadFolders(accountId)
        await this.reloadUnread()
        // 返回落库后的那条：调用方（规则弹层的「移到文件夹」）要立刻选中它。
        return (this.foldersByAccount[accountId] || []).find((f) => f.name === clean) || null
      } catch (error) {
        if (!existing) {
          this.foldersByAccount = {
            ...this.foldersByAccount,
            [accountId]: (this.foldersByAccount[accountId] || []).filter(
              (f) => f.name !== clean,
            ),
          }
        }
        throw error
      }
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

    /**
     * 搜索词的「同义地址」：通讯录里叫这个名字的人，邮箱地址是什么。
     *
     * 用户搜「张三」时，很多邮件的信头只有 `zhangsan@x.com`、没写姓名——
     * 只按文本匹配就会漏。这里把通讯录命中的地址一起交给后端做 OR。
     * 正文搜索不需要（地址出现在正文里没有意义）。
     */
    async searchAlternativesFor(text) {
      const term = String(text || '').trim()
      if (!term || this.searchField === 'body') return []
      try {
        const hits = (await invoke('mail_contact_suggest', { prefix: term })) || []
        const emails = hits
          .map((hit) => String(hit?.email || '').trim().toLowerCase())
          .filter(Boolean)
        return [...new Set(emails)].slice(0, 8)
      } catch {
        // 通讯录查询失败不该挡住搜索本身。
        return []
      }
    },

    async loadMessages({ silent = false, seq = null, append = false } = {}) {
      if (!this.ready && !silent) return
      const searching =
        Boolean(this.searchQuery.trim()) ||
        this.searchFilters.unreadOnly ||
        this.searchFilters.starredOnly ||
        this.searchFilters.hasAttachment ||
        Boolean(this.activeLabel) ||
        this.scope === 'all'

      const pageSize = Math.max(20, Number(this.settings.pageSize) || 100)
      const offset = append ? this.messages.length : 0
      // 增量请求多要一条：用来判断「还有没有下一页」，
      // 否则用户要一直点「加载更多」直到点出一个空列表才知道到底了。
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
        limit: pageSize + 1,
        offset,
        textAlternatives: await this.searchAlternativesFor(this.searchQuery.trim()),
      }

      try {
        const rows = await invoke('mail_search', { query })
        // 过期请求：整批结果丢掉，不能覆盖更新的一次搜索。
        if (seq !== null && seq !== this.searchSeq) return
        const list = rows || []
        const hasMore = list.length > pageSize
        const page = hasMore ? list.slice(0, pageSize) : list
        const grouped = await this.groupThreadsIfNeeded(append ? [...this.messages, ...page] : page)
        this.messages = grouped
        this.hasMore = hasMore
        this.pageCount = append ? this.pageCount + 1 : 1
        this.searchError = ''
      } catch (error) {
        // 以前这里会「静默回退成普通列表」——于是 SQL 写错也看起来像正常页面，
        // 搜索 bug 藏了很久（MATCH 用别名就是被这样掩盖的）。
        // 现在如实把错误暴露到列表头部，并且**不再**用全量列表冒充搜索结果。
        logger.error('检索失败', error)
        this.searchError = String(error?.message || error)
        if (!append) this.messages = []
      }
    },

    /** 上一页之后继续加载。 */
    async loadMore() {
      if (this.loadingMore || !this.hasMore) return
      this.loadingMore = true
      try {
        await this.loadMessages({ silent: true, append: true })
      } finally {
        this.loadingMore = false
      }
    },

    async groupThreadsIfNeeded(rows) {
      if (!this.settings.groupThreads) return rows
      // 搜索时用户要的是逐封结果；收件箱 / 全部邮件这类列表都按会话聚合，
      // 这样点开任意一封都会进入「每封一张卡片」的会话视图。
      if (this.searchQuery.trim()) return rows

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

    /** 从草稿箱续写：取回正文与收件人，装进编辑器。 */
    async resumeDraft(row) {
      const account = this.accountsById[row.accountId] || this.activeAccount || this.accounts[0]
      if (!account) return
      try {
        let detail
        try {
          detail = await invoke('mail_get_message', {
            accountId: row.accountId,
            folder: row.folder,
            uid: row.uid,
            // 草稿的自定义头只在原始报文里有；缓存路径拿不到组芯片信息。
            force: true,
          })
        } catch {
          // 离线时退回缓存正文：至少能继续写，只是组芯片会显示成成员地址。
          detail = await invoke('mail_get_message', {
            accountId: row.accountId,
            folder: row.folder,
            uid: row.uid,
            force: false,
          })
        }
        const draft = buildCompose('new', null, account, this.settings)
        // 本应用保存草稿时会把「组芯片」原样留在自定义头里；有它就用回原清单，
        // 否则通讯组会被展开成上百个地址，正好破坏写信页的显示方式。
        const restored = decodeDraftRecipients(detail)
        draft.to = restored
          ? restored.to
          : (detail.to || []).map((a) => (a.name ? `${a.name} <${a.address}>` : a.address))
        draft.cc = restored
          ? restored.cc
          : (detail.cc || []).map((a) => (a.name ? `${a.name} <${a.address}>` : a.address))
        if (restored) {
          draft.bcc = restored.bcc
          draft.showBcc = draft.bcc.length > 0
        }
        draft.showCc = draft.cc.length > 0
        draft.subject = detail.subject || ''
        draft.text = detail.text || ''
        draft.html = detail.html || ''
        draft.inReplyTo = detail.inReplyTo || null
        draft.references = detail.references || null
        // 附件只在本地有副本时才能续写；草稿里服务器上的附件不自动回填，
        // 免得用户以为附件还在、实际发送时却缺失。
        draft.attachments = []
        // 记住来源：保存或发送后要删掉这条旧草稿，否则草稿箱会越堆越多。
        draft.draftUid = row.uid
        draft.draftFolder = row.folder
        this.composing = draft
      } catch (error) {
        logger.error('打开草稿失败', error)
        this.lastError = String(error?.message || error)
      }
    },

    /** 定位并打开一封指定邮件（供外部跳转使用）。 */
    async revealMessage(accountId, folder, uid) {
      if (!accountId || !folder || !uid) return
      if (this.activeAccountId !== accountId) {
        this.activeAccountId = accountId
        await this.loadFolders(accountId)
      }
      this.activeFolder = folder
      this.scope = 'folder'
      this.activeLabel = ''
      await this.loadMessages({ silent: true })
      const row =
        this.messages.find(
          (m) => m.accountId === accountId && m.folder === folder && m.uid === Number(uid),
        ) || { accountId, folder, uid: Number(uid) }
      await this.openMessage(row)
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
        // 每次同步顺带补一次离线队列与连接状态：断网期间排队的操作
        // 只要能连上就在这一轮被重放掉，不需要用户手动点。
        this.replayOffline({ quiet: true }).catch(() => {})
        this.refreshConnection().catch(() => {})
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

    // ---------------- 离线队列 / IDLE / 连接状态 ----------------

    /** 重放断网期间排队的写操作。 */
    async replayOffline({ quiet = false } = {}) {
      if (!this.settings.offlineQueue) return { replayed: 0, failed: 0 }
      try {
        const result = await invoke('mail_offline_replay', {
          accountId: this.activeAccountId || null,
        })
        if (!quiet && result?.replayed) {
          // 重放成功后列表里的标记可能与本地缓存不一致，刷一次。
          await this.loadMessages({ silent: true })
          await this.reloadUnread()
        }
        await this.refreshConnection()
        return result || { replayed: 0, failed: 0 }
      } catch (error) {
        logger.warn('重放离线操作失败', error)
        return { replayed: 0, failed: 0, errors: [String(error)] }
      }
    },

    /** 拉取「离线队列 / 待发 / 同步中 / IDLE」状态。 */
    async refreshConnection() {
      try {
        const status = await invoke('mail_connection_status', {
          accountId: this.activeAccountId || null,
        })
        this.offlineQueued = Number(status?.offlineQueued || 0)
        this.pendingSend = Number(status?.pendingSend || 0)
        this.idleActive = Boolean(status?.idleActive)
      } catch (error) {
        logger.debug('读取连接状态失败', error)
      }
    },

    /** 开启 IMAP IDLE（fetchMode = idle 时才真正启动）。 */
    async startIdle() {
      if (!this.activeAccountId) return false
      if (this.settings.fetchMode !== 'idle') return false
      try {
        const started = await invoke('mail_start_idle', {
          accountId: this.activeAccountId,
          folder: this.activeFolder || 'INBOX',
        })
        this.idleActive = Boolean(started)
        return Boolean(started)
      } catch (error) {
        logger.warn('启动 IDLE 失败，继续使用轮询', error)
        this.idleActive = false
        return false
      }
    },

    async stopIdle(accountId = this.activeAccountId) {
      if (!accountId) return
      await invoke('mail_stop_idle', { accountId }).catch(() => {})
      this.idleActive = false
    },

    // ---------------- OAuth2 ----------------

    /** 发起授权，返回给界面展示的授权地址（设备码流程还会带短码）。 */
    async startOAuth(provider, accountId = null, { clientId = null, scope = null } = {}) {
      return invoke('mail_oauth_start', { provider, clientId, scope, accountId })
    },

    /** 轮询等待用户在浏览器里完成授权，返回授权后的账户邮箱（服务商提供时）。 */
    async pollOAuth(accountId, provider, deviceCode, interval = 5) {
      const email = await invoke('mail_oauth_poll', {
        accountId,
        provider,
        deviceCode,
        interval,
        clientId: null,
      })
      this.oauthStatus = await this.oauthStatusFor(accountId).catch(() => null)
      return email || ''
    },

    async oauthStatusFor(accountId) {
      if (!accountId) return null
      return invoke('mail_oauth_status', { accountId })
    },

    /** 手动完成授权：把浏览器回跳地址里的 code 交给后端换取令牌。 */
    async completeOAuth(accountId, flowId, provider, redirectUri, code) {
      return (
        (await invoke('mail_oauth_complete', {
          accountId,
          flowId,
          provider,
          redirectUri,
          code,
        })) || ''
      )
    },

    async signOutOAuth(accountId) {
      await invoke('mail_oauth_sign_out', { accountId })
      this.oauthStatus = null
      const account = this.accounts.find((a) => a.id === accountId)
      if (account) {
        account.authKind = 'password'
        account.oauthProvider = ''
      }
    },

    // ---------------- 模板 ----------------

    async loadTemplates() {
      try {
        this.templates = (await invoke('mail_list_templates')) || []
      } catch (error) {
        logger.warn('加载模板失败', error)
      }
    },

    async saveTemplate(template) {
      const saved = await invoke('mail_save_template', { template })
      const index = this.templates.findIndex((t) => t.id === saved.id)
      if (index >= 0) this.templates.splice(index, 1, saved)
      else this.templates.push(saved)
      return saved
    },

    async deleteTemplate(id) {
      await invoke('mail_delete_template', { id })
      this.templates = this.templates.filter((t) => t.id !== id)
    },

    /** 把模板套进当前草稿（保留收件人，只覆盖主题与正文）。 */
    applyTemplate(template) {
      if (!this.composing || !template) return
      if (template.subject) this.composing.subject = template.subject
      if (template.html) {
        this.composing.html = template.html
        this.composing.htmlMode = true
      } else if (template.text) {
        this.composing.html = ''
        this.composing.htmlMode = false
      }
      if (template.text) this.composing.text = template.text
    },

    // ---------------- 收发件人名单 ----------------

    async loadSenderRules() {
      try {
        this.senderRules = (await invoke('mail_list_sender_rules')) || []
      } catch (error) {
        logger.warn('加载名单失败', error)
      }
    },

    async saveSenderRule(rule) {
      const saved = await invoke('mail_save_sender_rule', { rule })
      const index = this.senderRules.findIndex((r) => r.id === saved.id)
      if (index >= 0) this.senderRules.splice(index, 1, saved)
      else this.senderRules.push(saved)
      return saved
    },

    async deleteSenderRule(id) {
      await invoke('mail_delete_sender_rule', { id })
      this.senderRules = this.senderRules.filter((r) => r.id !== id)
    },

    /** 把某个发件人一键拉黑 / 放行（邮件右键菜单用）。 */
    async blockSender(address, kind = 'block') {
      const rule = {
        id: '',
        kind,
        pattern: String(address || '').trim().toLowerCase(),
        note: '',
        createdAt: 0,
      }
      if (!rule.pattern) return null
      return this.saveSenderRule(rule)
    },

    // ---------------- 附件检查 / 诊断 ----------------

    async checkAttachments(paths) {
      try {
        return (await invoke('mail_check_attachments', { paths })) || []
      } catch {
        return []
      }
    },

    async exportDiagnostics(path) {
      return invoke('mail_export_diagnostics', { targetPath: path })
    },

    async testNotification() {
      return invoke('mail_test_notification')
    },

    async resetConnections(accountId = null) {
      return invoke('mail_reset_connections', { accountId })
    },

    /** S/MIME：本账户身份状态（是否已导入证书与私钥）。 */
    async smimeStatus(accountId = this.activeAccountId) {
      if (!accountId) return null
      try {
        return await invoke('mail_smime_status', { accountId })
      } catch {
        return null
      }
    },

    /** S/MIME：收件人证书是否已导入（加密发送的前提）。 */
    async smimePeerStatus(address) {
      try {
        return await invoke('mail_smime_peer_status', { address })
      } catch {
        return false
      }
    },

    /** PGP 钥匙串。 */
    async pgpKeys() {
      try {
        return (await invoke('mail_pgp_keys')) || []
      } catch {
        return []
      }
    },

    // ---------------- 阅读 ----------------

    async openMessage(row) {
      this.selected = row
      this.thread = []
      this.detailLoading = true
      const wasUnseen = !row.seen
      try {
        const detail = await invoke('mail_get_message', {
          accountId: row.accountId,
          folder: row.folder,
          uid: row.uid,
          force: false,
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
        // 打开即已读：侧栏的未读角标先减一，稍后由服务端 STATUS 校正。
        if (wasUnseen) {
          this.adjustFolderCount(row.accountId, row.folder, { unread: -1 })
          scheduleFolderRefresh(this, row.accountId)
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

    /**
     * 打开一个会话：把同一 thread 的每封邮件都取出来，按时间升序展示成独立卡片。
     * 以前只打开最新一封、把历史留在正文的引用块里；现在阅读区本身就是会话。
     */
    async openThread(row) {
      let rows = (row.threadUids || []).filter(Boolean)
      // 列表里的聚合行只包含当前文件夹的邮件；到库里按 thread_key 再查一次，
      // 才能把「已发送」里的回复也带进会话。只取同一个账户，避免不同账户
      // 恰好同主题的邮件被误并进同一个会话。
      if (row.threadKey) {
        try {
          const threadRows = await invoke('mail_thread', { threadKey: row.threadKey })
          const sameAccount = (threadRows || []).filter(
            (item) => item.accountId === row.accountId,
          )
          if (sameAccount.length > rows.length) rows = sameAccount
        } catch (error) {
          logger.warn('加载会话列表失败', error)
        }
      }
      rows = rows.slice().sort((a, b) => (Number(a.date) || 0) - (Number(b.date) || 0)).slice(-30)
      if (rows.length < 2) return this.openMessage(row)
      this.selected = row
      this.thread = []
      this.detailLoading = true
      const deltas = new Map()
      for (const item of rows) {
        if (item.seen) continue
        const key = `${item.accountId}\u0000${item.folder}`
        const entry = deltas.get(key) || {
          accountId: item.accountId,
          folder: item.folder,
          count: 0,
        }
        entry.count += 1
        deltas.set(key, entry)
      }
      try {
        const details = await Promise.all(
          rows.map((item) =>
            invoke('mail_get_message', {
              accountId: item.accountId,
              folder: item.folder,
              uid: item.uid,
              force: false,
            }).catch(() => null),
          ),
        )
        const list = details
          .filter(Boolean)
          .sort((a, b) => (Number(a.date) || 0) - (Number(b.date) || 0))
        if (!list.length) throw new Error('会话内容加载失败')
        this.thread = list
        this.detail = list[list.length - 1]
        const seenKeys = new Set(
          rows.map((item) => `${item.accountId}:${item.folder}:${item.uid}`),
        )
        for (const item of rows) item.seen = true
        // 列表里同一批邮件的行也要同步成已读，否则会话卡片已是已读、列表还是粗体。
        for (const message of this.messages) {
          if (seenKeys.has(`${message.accountId}:${message.folder}:${message.uid}`)) {
            message.seen = true
          }
        }
        for (const entry of deltas.values()) {
          this.adjustFolderCount(entry.accountId, entry.folder, { unread: -entry.count })
          scheduleFolderRefresh(this, entry.accountId)
        }
        await this.reloadUnread()
      } catch (error) {
        logger.error('打开会话失败', error)
        this.lastError = String(error?.message || error)
        this.thread = []
        this.detail = null
      } finally {
        this.detailLoading = false
      }
    },

    async setSeen(rows, seen) {
      const deltas = new Map()
      for (const row of rows) {
        if (Boolean(row.seen) === seen) continue
        try {
          await invoke('mail_set_seen', {
            accountId: row.accountId,
            folder: row.folder,
            uids: [row.uid],
            seen,
          })
        } catch (error) {
          logger.warn('标记已读失败', error)
          continue
        }
        row.seen = seen
        const match = this.messages.find(
          (m) => m.accountId === row.accountId && m.folder === row.folder && m.uid === row.uid,
        )
        if (match) match.seen = seen
        const key = `${row.accountId}\u0000${row.folder}`
        const entry = deltas.get(key) || { accountId: row.accountId, folder: row.folder, count: 0 }
        entry.count += 1
        deltas.set(key, entry)
      }
      for (const entry of deltas.values()) {
        this.adjustFolderCount(entry.accountId, entry.folder, {
          unread: seen ? -entry.count : entry.count,
        })
        scheduleFolderRefresh(this, entry.accountId)
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
      // 阅读区跟着删：以前只在 `selected` 非空时才清，用键盘/通讯录打开的邮件
      // 删掉后正文会留在右边，形成一个已经不存在的「幽灵邮件」。
      if (
        this.detail &&
        keys.has(`${this.detail.accountId}:${this.detail.folder}:${this.detail.uid}`)
      ) {
        this.detail = null
        this.selected = null
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
      // 递增序号：快速输入会连发多个请求，慢的后到会把新结果覆盖掉，
      // 表现就是「搜索框里是新词，列表还是旧词的结果」。
      const seq = (this.searchSeq || 0) + 1
      this.searchSeq = seq
      this.searching = true
      try {
        await this.loadMessages({ seq })
      } finally {
        if (this.searchSeq === seq) this.searching = false
      }
    },

    clearSearch() {
      this.searchQuery = ''
      this.searchFilters = { unreadOnly: false, starredOnly: false, hasAttachment: false }
      this.activeLabel = ''
      this.loadMessages()
    },

    // ---------------- 批量操作 ----------------

    keyOf(row) {
      return `${row.accountId}:${row.folder}:${row.uid}`
    },

    rowByKey(key) {
      return this.messages.find((m) => this.keyOf(m) === key) || null
    },

    toggleSelect(row, checked) {
      const key = this.keyOf(row)
      if (checked) {
        if (!this.selectedKeys.includes(key)) this.selectedKeys.push(key)
      } else {
        this.selectedKeys = this.selectedKeys.filter((k) => k !== key)
      }
    },

    selectAll(checked) {
      this.selectedKeys = checked ? this.messages.map((m) => this.keyOf(m)) : []
    },

    clearSelection() {
      this.selectedKeys = []
    },

    /** 选中的行，按账户分组（IMAP 命令是 per-account 的）。 */
    selectedRows() {
      return this.selectedKeys.map((k) => this.rowByKey(k)).filter(Boolean)
    },

    async runBatch(action, target, explicitRows = null) {
      // 显式传入的行优先：右键菜单可能只针对一封未勾选的邮件，
      // 那时选择集是空的，不能退回 selectedRows()。
      const rows = explicitRows?.length ? explicitRows : this.selectedRows()
      if (!rows.length) return
      const groups = new Map()
      for (const row of rows) {
        const key = `${row.accountId}::${row.folder}`
        if (!groups.has(key)) groups.set(key, [])
        groups.get(key).push(row)
      }
      const accountsTouched = new Set()
      const affectsList = ['delete', 'move', 'junk', 'unmarkJunk'].includes(action)
      const affectedKeys = new Set(
        rows.map((row) => `${row.accountId}:${row.folder}:${row.uid}`),
      )
      const folderByRole = (accountId, role) =>
        (this.foldersByAccount[accountId] || []).find((f) => f.role === role)
      for (const list of groups.values()) {
        const { accountId, folder } = list[0]
        const uids = list.map((m) => m.uid)
        const total = list.length
        const unseen = list.filter((m) => !m.seen).length
        const seenCount = total - unseen
        switch (action) {
          case 'read':
            await invoke('mail_batch_flags', { accountId, folder, uids, seen: true, flagged: null })
            list.forEach((m) => (m.seen = true))
            this.adjustFolderCount(accountId, folder, { unread: -unseen })
            break
          case 'unread':
            await invoke('mail_batch_flags', { accountId, folder, uids, seen: false, flagged: null })
            list.forEach((m) => (m.seen = false))
            this.adjustFolderCount(accountId, folder, { unread: seenCount })
            break
          case 'flag':
            await invoke('mail_batch_flags', { accountId, folder, uids, seen: null, flagged: true })
            list.forEach((m) => (m.flagged = true))
            break
          case 'unflag':
            await invoke('mail_batch_flags', { accountId, folder, uids, seen: null, flagged: false })
            list.forEach((m) => (m.flagged = false))
            break
          case 'junk': {
            await invoke('mail_junk_messages', { accountId, folder, uids })
            const junk = folderByRole(accountId, 'junk')
            this.adjustFolderCount(accountId, folder, { total: -total, unread: -unseen })
            if (junk) this.adjustFolderCount(accountId, junk.name, { total, unread: unseen })
            break
          }
          case 'move':
            await invoke('mail_batch_flags', { accountId, folder, uids, seen: null, flagged: null }).catch(() => {})
            await invoke('mail_move_messages', { accountId, folder, uids, target })
            this.adjustFolderCount(accountId, folder, { total: -total, unread: -unseen })
            this.adjustFolderCount(accountId, target, { total, unread: unseen })
            break
          case 'delete': {
            await invoke('mail_batch_delete', { accountId, folder, uids })
            this.adjustFolderCount(accountId, folder, { total: -total, unread: -unseen })
            const isTrash = (this.foldersByAccount[accountId] || []).find(
              (f) => f.name === folder,
            )?.role === 'trash'
            const trash = folderByRole(accountId, 'trash')
            if (!isTrash && trash) {
              this.adjustFolderCount(accountId, trash.name, { total, unread: unseen })
            }
            break
          }
          case 'unmarkJunk': {
            await invoke('mail_unmark_junk', { accountId, folder, uids })
            this.adjustFolderCount(accountId, folder, { total: -total, unread: -unseen })
            const inbox = folderByRole(accountId, 'inbox')
            if (inbox) this.adjustFolderCount(accountId, inbox.name, { total, unread: unseen })
            break
          }
        }
        accountsTouched.add(accountId)
      }
      this.clearSelection()
      // 删除 / 移走的是当前正在读的邮件：列表里已经没有了，阅读区也要一并收起，
      // 否则会留下「正文还在、列表里却找不到这封邮件」的断链状态。
      if (
        affectsList &&
        this.detail &&
        affectedKeys.has(`${this.detail.accountId}:${this.detail.folder}:${this.detail.uid}`)
      ) {
        this.detail = null
        this.selected = null
      }
      await Promise.all([this.loadMessages({ silent: true }), this.reloadUnread()])
      // 本地增量只是先让角标动起来；服务端计数（尤其目标文件夹）稍后用一次
      // 完整的 STATUS 校正，并合并连续操作，避免每封邮件都触发一轮刷新。
      for (const accountId of accountsTouched) scheduleFolderRefresh(this, accountId)
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

    /**
     * 关闭编辑器。有内容时先存进草稿箱——「写了半封关掉就没了」是最伤人的体验。
     *
     * 返回 `{ saved, localOnly, error }`：调用方要能把结果如实说给用户。
     * 以前失败只写进日志，用户看到的就是「我明明关了，草稿箱里什么都没有」，
     * 只能自己猜（真实反馈：收件人为空的草稿被后端拒绝，界面上一点动静都没有）。
     */
    /** 用户明确选择「直接放弃」：不写草稿箱，直接关掉（本地暂存由编辑器删）。 */
    discardCompose() {
      const draft = this.composing
      this.composing = null
      return draft
    },

    async closeCompose({ saveDraft = true } = {}) {
      const draft = this.composing
      this.composing = null
      if (!saveDraft || !draft) return { saved: false, skipped: true }
      // 不再判断「有没有内容」：用户一旦开始写信，哪怕收件人、主题、正文全空，
      // 也该在草稿箱里留一条可以继续编辑的草稿。以前这里把空草稿直接跳过，
      // 用户看到的就是「关了编辑器，草稿箱什么也没有」。
      try {
        const saved = await invoke('mail_save_draft', {
          request: {
            accountId: draft.accountId,
            to: draft.to,
            cc: draft.cc,
            bcc: draft.bcc,
            subject: draft.subject,
            text: draft.text,
            html: draft.htmlMode ? draft.html : null,
            inReplyTo: draft.inReplyTo,
            references: draft.references,
            attachments: (draft.attachments || []).map((a) => a.path),
            inlineImages: (draft.inlineImages || []).map((img) => ({
              contentId: img.contentId,
              path: img.path,
              filename: img.name || '',
            })),
            cryptoMode: draft.cryptoMode || '',
            saveToSent: true,
          },
        })
        // 最终草稿已经写进草稿箱：编辑器自动保存的那份就没用了。
        await this._discardMailboxDraft(draft)
        await this.loadFolders(draft.accountId).catch(() => {})
        await this.reloadUnread()
        // 列表也要跟上：用户往往就停在草稿箱里看这一封，不刷新会以为没存上。
        await this.loadMessages({ silent: true }).catch(() => {})
        return { saved: true, localOnly: Boolean(saved?.localOnly) }
      } catch (error) {
        logger.warn('关闭时保存草稿失败', error)
        this.lastError = `草稿保存失败：${error?.message || error}`
        return { saved: false, error: String(error?.message || error) }
      }
    },

    async sendCompose({ useOutbox = false, scheduledAt = 0 } = {}) {
      if (!this.composing) return
      // 附件体积与危险类型检查：超限直接拦下，危险类型由界面确认过后再走到这里。
      const paths = (this.composing.attachments || []).map((a) => a.path)
      const checks = await this.checkAttachments(paths)
      const tooLarge = checks.filter((c) => c.level === 'large')
      if (tooLarge.length) {
        const limit = this.settings.attachmentLimitMb
        throw new Error(
          `附件过大：${tooLarge.map((c) => c.name).join('、')}。总大小上限 ${limit} MB，请改用网盘链接。`,
        )
      }
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
        attachments: paths,
        inlineImages: (this.composing.inlineImages || []).map((img) => ({
          contentId: img.contentId,
          path: img.path,
          filename: img.name || '',
        })),
        cryptoMode: this.composing.cryptoMode || '',
        saveToSent: true,
      }
      if (scheduledAt || (useOutbox && this.settings.undoSendSeconds > 0)) {
        // 定时发送用绝对时间戳；撤回窗口用后端设置里的秒数。
        const sendAt = scheduledAt ? Math.floor(Number(scheduledAt) / 1000) : null
        const item = await invoke('mail_queue_send', { request, sendAt })
        this.outbox.push(item)
        await this._discardSourceDraft(this.composing)
        await this._discardMailboxDraft(this.composing)
        this.composing = null
        await this.refreshConnection().catch(() => {})
        await this.loadOutbox().catch(() => {})
        this.scheduleOutboxFlush()
        return { queued: item, scheduled: Boolean(scheduledAt) }
      }
      const sourceDraft = this.composing
      await invoke('mail_send', { request })
      this.composing = null
      // 续写自草稿 / 自动保存过草稿：发出去之后那两份就没用了，删掉，
      // 否则草稿箱里会留着一封「已经发过的信」。
      await this._discardSourceDraft(sourceDraft)
      await this._discardMailboxDraft(sourceDraft)
      return { sent: true }
    },


    /** 续写自草稿时，保存 / 发送成功后删掉服务器上的旧草稿。 */
    async _discardSourceDraft(draft) {
      if (!draft?.draftUid || !draft?.draftFolder) return
      try {
        await invoke('mail_discard_draft', {
          accountId: draft.accountId,
          folder: draft.draftFolder,
          uid: draft.draftUid,
        })
      } catch (error) {
        logger.warn('删除旧草稿失败', error)
      }
    },

    /**
     * 删掉编辑器自动保存的那份草稿。
     *
     * 编辑器每 15 秒把当前内容写进草稿箱（`draft.mailboxDraft`）；关闭 / 发送时
     * 又会写一份最终版本，所以这份「自动草稿」要一起清掉，否则每次写信都会在
     * 草稿箱里留下两封一模一样的信。
     */
    async _discardMailboxDraft(draft) {
      const saved = draft?.mailboxDraft
      if (!saved?.uid || !saved?.folder) return
      draft.mailboxDraft = null
      try {
        await invoke('mail_discard_draft', {
          accountId: draft.accountId,
          folder: saved.folder,
          uid: saved.uid,
        })
      } catch (error) {
        logger.warn('删除自动保存的草稿失败', error)
      }
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
        inlineImages: (this.composing.inlineImages || []).map((img) => ({
          contentId: img.contentId,
          path: img.path,
          filename: img.name || '',
        })),
        cryptoMode: this.composing.cryptoMode || '',
        saveToSent: true,
      }
      const sourceDraft = this.composing
      await invoke('mail_save_draft', { request })
      // 先存新的再删旧的：顺序反过来，一旦新草稿保存失败就会把原稿也弄丢。
      await this._discardSourceDraft(sourceDraft)
      await this._discardMailboxDraft(sourceDraft)
      this.composing = null
      await this.loadFolders(request.accountId).catch(() => {})
      await this.reloadUnread()
    },

    /**
     * 编辑器定时自动保存：只写草稿箱，不关闭编辑器、不弹提示。
     *
     * 与 `saveDraft()` 的区别：后者是「保存草稿并关窗」的用户动作，
     * 会清空 composing；这里每 15 秒跑一次，必须保持编辑器原样。
     *
     * 返回 `{ folder, uid }`（后端给出）：编辑器记着它，下一次自动保存先把上一份
     * 删掉再写，草稿箱里始终只有一封「当前进度」。
     */
    async autoSaveDraft(request) {
      if (!request) return null
      const saved = await invoke('mail_save_draft', { request })
      const accountId = request.accountId
      if (accountId) {
        // 静默刷新草稿箱计数，让侧栏立刻能看到「草稿 +1」。
        this.loadFolders(accountId).catch(() => {})
      }
      return saved
    },

    async loadOutbox() {
      try {
        this.outbox = (await invoke('mail_list_outbox')) || []
        this.scheduleOutboxFlush()
      } catch (error) {
        logger.warn('加载待发箱失败', error)
      }
    },

    /**
     * 按最早的到期时间安排一次投递。
     *
     * 长定时拆成最多 1 小时一段：浏览器定时器有上限，应用也可能休眠；
     * 每次醒来重新看 outbox 里最早的一条，既不会漏发，也不会提前发。
     */
    scheduleOutboxFlush() {
      if (this.outboxTimer) {
        clearTimeout(this.outboxTimer)
        this.outboxTimer = null
      }
      const due = this.outbox
        .filter((item) => !item.staged)
        // 失败过的邮件有退避时间：到期后要等 nextAttemptAt 才重试，否则会每秒空转。
        .map((item) =>
          Math.max(Number(item.dueAt) || 0, Number(item.nextAttemptAt) || 0) * 1000,
        )
        .filter((value) => Number.isFinite(value) && value > 0)
        .sort((a, b) => a - b)[0]
      if (!due) return
      const wait = Math.max(0, due - Date.now())
      // 最短 5 秒：flush 失败时给 loadOutbox / 退避写入留出时间，避免定时器空转。
      const delay = Math.min(Math.max(wait + 300, 5_000), 60 * 60 * 1000)
      this.outboxTimer = setTimeout(async () => {
        this.outboxTimer = null
        if (wait <= 1_500) await this.flushOutbox()
        this.scheduleOutboxFlush()
      }, delay)
    },

    async cancelOutbox(id) {
      await invoke('mail_cancel_outbox', { id })
      this.outbox = this.outbox.filter((item) => item.id !== id)
      this.scheduleOutboxFlush()
    },

    /** 发件箱里点「重试」：立刻再投递一次（不等待退避）。 */
    async retryOutbox(id) {
      const result = await invoke('mail_retry_outbox', { id })
      await this.loadOutbox()
      await this.loadMessages({ silent: true })
      return result
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
    inlineImages: [],
    cryptoMode: '',
    inReplyTo: null,
    references: null,
    showCc: false,
    showBcc: false,
    // 编辑器自动保存进草稿箱的那一份（uid + folder）：关闭 / 发送时把它替换掉。
    mailboxDraft: null,
  }
  if (!source) return base

  const from = source.fromAddr || ''
  const self = String(account.email || '').trim().toLowerCase()
  const isMine = from.trim().toLowerCase() === self
  if (mode === 'reply' || mode === 'replyAll') {
    // 自己发的邮件点「回复」应当回给原收件人，而不是自己。
    const primary = parseAddressList(isMine ? source.toJson : from)
    if (mode === 'replyAll') {
      // 全部回复 = 原发件人 + 原收件人 + 原抄送，去掉自己、按地址去重。
      // 以前只带「发件人 + 原抄送」，原 To 里的其他收件人会被丢掉。
      const addressKey = (token) => {
        const text = String(token || '')
        const match = text.match(/<([^>]+)>/)
        return (match ? match[1] : text).trim().toLowerCase()
      }
      const seen = new Set()
      const add = (tokens, target) => {
        for (const token of tokens) {
          const key = addressKey(token)
          if (!key || key === self || seen.has(key)) continue
          seen.add(key)
          target.push(token)
        }
      }
      base.to = []
      add(primary, base.to)
      add(parseAddressList(source.toJson), base.to)
      base.cc = []
      add(parseAddressList(source.ccJson), base.cc)
      base.showCc = base.cc.length > 0
    } else {
      base.to = primary
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

/**
 * 从草稿的自定义头里取回写信页的原始收件人清单（含通讯组芯片）。
 * 值由 Rust 侧写入：base64(JSON)，按 RFC 折行；解析时先去掉行间空白。
 */
function decodeDraftRecipients(detail) {
  const header = (detail?.headers || []).find(
    ([name]) => String(name).toLowerCase() === 'x-guaxi-draft-recipients',
  )
  if (!header) return null
  try {
    const raw = String(header[1] || '').replace(/\s+/g, '')
    // atob 给的是字节串，JSON 里的中文要先按 UTF-8 解回来。
    const binary = atob(raw)
    const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0))
    const data = JSON.parse(new TextDecoder().decode(bytes))
    if (!data || !Array.isArray(data.to)) return null
    return {
      to: data.to.map(String),
      cc: Array.isArray(data.cc) ? data.cc.map(String) : [],
      bcc: Array.isArray(data.bcc) ? data.bcc.map(String) : [],
    }
  } catch {
    return null
  }
}
