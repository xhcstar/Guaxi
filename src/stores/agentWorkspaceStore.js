import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { createLogger } from '../utils/logger'

const logger = createLogger('agentWorkspaceStore')

/**
 * Agent 工作台状态。
 *
 * 这一版把「谁是会话本体」交还给 codex：
 *
 * - 线程列表、正文、工具调用、计划、diff、用量全部来自 app-server 的 `thread/*`，
 *   前端只保留**视图需要的**派生结构（把 ThreadItem 规整成渲染块）。
 * - localStorage 只存偏好与草稿（模型、权限、工作目录、输入历史），
 *   不再存消息正文——否则 codex 里删掉的会话在 Guaxi 里还阴魂不散。
 *
 * 事件契约（后端 codex_app_server.rs）：
 * - `agent://notification` → { kind, threadId, turnId, payload, at }
 * - `agent://approval`     → { requestId, kind, method, threadId, turnId, params, at }
 */

/** 后端 `mode_instructions` 对应的四种工作方式。 */
export const AGENT_MODES = Object.freeze([
  { id: 'code', label: '编码', hint: '读代码、改文件、跑测试' },
  { id: 'explore', label: '探索', hint: '只读检索，产出结论与出处' },
  { id: 'research', label: '研究', hint: '先查知识库再联网，区分出处' },
  { id: 'computer', label: '操作电脑', hint: '截屏、点按、输入，逐步确认' },
])

/**
 * 权限预设。四项对应 Codex 底部的权限选择器。
 * `bypass` 不是「更宽松的沙箱」，而是「不再询问」——两者要分开标。
 */
export const PERMISSION_PRESETS = Object.freeze([
  {
    id: 'read-only',
    label: '只读',
    hint: '只能读取文件，任何改动都会先问你',
    sandbox: 'read-only',
    approvalPolicy: 'on-request',
  },
  {
    id: 'workspace',
    label: '工作区写入',
    hint: '可以改当前工作目录，越界操作需要批准',
    sandbox: 'workspace-write',
    approvalPolicy: 'on-request',
  },
  {
    id: 'full-access',
    label: '完全访问',
    hint: '可以读写整台机器，危险操作仍会询问',
    sandbox: 'danger-full-access',
    approvalPolicy: 'on-request',
  },
  {
    id: 'bypass',
    label: '跳过审批',
    hint: '命令直接执行，不再弹审批',
    sandbox: 'danger-full-access',
    approvalPolicy: 'never',
  },
])

const PRESET_BY_ID = new Map(PERMISSION_PRESETS.map((p) => [p.id, p]))

const STORE_KEY = 'guaxi:agent:workspace:v2'
const MAX_HISTORY = 120
const THREAD_PAGE_SIZE = 80

const now = () => Date.now()
const uid = (prefix = 'x') => `${prefix}-${now()}-${Math.random().toString(36).slice(2, 7)}`

function readStore() {
  if (typeof localStorage === 'undefined') return {}
  try {
    return JSON.parse(localStorage.getItem(STORE_KEY) || '{}') || {}
  } catch (err) {
    logger.warn('[agent] read state failed', { error: String(err) })
    return {}
  }
}

function writeStore(value) {
  if (typeof localStorage === 'undefined') return
  try {
    localStorage.setItem(STORE_KEY, JSON.stringify(value))
  } catch (err) {
    // 配额满时不能影响运行：输入历史丢了可以再输，会话本身在 codex 里。
    logger.warn('[agent] persist state failed', { error: String(err) })
  }
}

function normalizeStatus(status) {
  if (!status) return 'idle'
  if (typeof status === 'string') return status
  return String(status.type || 'idle')
}

function emptyThreadState(seed = {}) {
  return {
    id: String(seed.id || ''),
    name: String(seed.name || ''),
    preview: String(seed.preview || ''),
    cwd: String(seed.cwd || ''),
    model: String(seed.model || ''),
    modelProvider: String(seed.modelProvider || ''),
    sandbox: String(seed.sandbox || ''),
    approvalPolicy: String(seed.approvalPolicy || ''),
    effort: String(seed.effort || ''),
    status: seed.status || 'idle',
    gitInfo: seed.gitInfo || null,
    createdAt: Number(seed.createdAt || 0),
    updatedAt: Number(seed.updatedAt || 0),
    turns: Array.isArray(seed.turns) ? seed.turns : [],
    plan: Array.isArray(seed.plan) ? seed.plan : [],
    diff: String(seed.diff || ''),
    usage: seed.usage || null,
    git: seed.git || null,
    loaded: Boolean(seed.loaded),
    loading: false,
    unread: false,
    error: String(seed.error || ''),
  }
}

/** ThreadItem → 渲染块。字段保留原始值，额外补几个方便模板直接用的派生字段。 */
export function normalizeItem(raw) {
  if (!raw || typeof raw !== 'object') return null
  const type = String(raw.type || '')
  const id = String(raw.id || uid('item'))
  const base = {
    id,
    type,
    status: String(raw.status || 'completed'),
    startedAt: Number(raw.startedAt || 0),
    completedAt: 0,
    streaming: false,
  }

  switch (type) {
    case 'userMessage':
      return {
        ...base,
        content: Array.isArray(raw.content) ? raw.content : [],
        text: (Array.isArray(raw.content) ? raw.content : [])
          .filter((c) => c?.type === 'text')
          .map((c) => String(c.text || ''))
          .join('\n'),
        clientId: raw.clientId || null,
      }
    case 'agentMessage':
      return {
        ...base,
        text: String(raw.text || ''),
        phase: raw.phase || null,
        delivery: raw.delivery || null,
      }
    case 'reasoning':
      return {
        ...base,
        summary: Array.isArray(raw.summary) ? raw.summary.map(String) : [],
        content: Array.isArray(raw.content) ? raw.content.map(String) : [],
      }
    case 'plan':
      return { ...base, text: String(raw.text || '') }
    case 'commandExecution':
      return {
        ...base,
        command: String(raw.command || ''),
        cwd: String(raw.cwd || ''),
        output: String(raw.aggregatedOutput || ''),
        exitCode: typeof raw.exitCode === 'number' ? raw.exitCode : null,
        durationMs: Number(raw.durationMs || 0),
        processId: raw.processId || null,
        source: raw.source || null,
        actions: Array.isArray(raw.commandActions) ? raw.commandActions : [],
      }
    case 'fileChange':
      return {
        ...base,
        changes: Array.isArray(raw.changes) ? raw.changes : [],
        patch: String(raw.patch || ''),
        output: '',
      }
    case 'mcpToolCall':
    case 'dynamicToolCall':
      return {
        ...base,
        server: String(raw.server || ''),
        tool: String(raw.tool || ''),
        namespace: String(raw.namespace || ''),
        arguments: raw.arguments ?? null,
        result: raw.result ?? null,
        error: raw.error ?? null,
        contentItems: Array.isArray(raw.contentItems) ? raw.contentItems : [],
        success: typeof raw.success === 'boolean' ? raw.success : null,
        durationMs: Number(raw.durationMs || 0),
        progress: '',
      }
    case 'webSearch':
      return {
        ...base,
        query: String(raw.query || ''),
        action: raw.action || null,
        results: Array.isArray(raw.results) ? raw.results : [],
      }
    case 'imageView':
      return { ...base, path: String(raw.path || '') }
    case 'imageGeneration':
      return { ...base, ...raw }
    case 'contextCompaction':
      return { ...base }
    case 'enteredReviewMode':
      return { ...base, review: String(raw.review || '') }
    case 'exitedReviewMode':
      return { ...base, review: String(raw.review || '') }
    case 'hookPrompt':
      return { ...base, fragments: Array.isArray(raw.fragments) ? raw.fragments : [] }
    case 'collabAgentToolCall':
      return {
        ...base,
        tool: String(raw.tool || ''),
        prompt: String(raw.prompt || ''),
        receiverThreadIds: Array.isArray(raw.receiverThreadIds) ? raw.receiverThreadIds : [],
      }
    default:
      return { ...base, ...raw }
  }
}

export function normalizeTurn(raw) {
  if (!raw || typeof raw !== 'object') return null
  return {
    id: String(raw.id || ''),
    status: String(raw.status || 'completed'),
    items: (Array.isArray(raw.items) ? raw.items : []).map(normalizeItem).filter(Boolean),
    error: raw.error || null,
    startedAt: Number(raw.startedAt || 0),
    completedAt: Number(raw.completedAt || 0),
    durationMs: Number(raw.durationMs || 0),
    local: false,
  }
}

function normalizeUsage(raw) {
  if (!raw || typeof raw !== 'object') return null
  const bucket = (v) => {
    if (!v || typeof v !== 'object') return null
    const num = (x) => (typeof x === 'number' && Number.isFinite(x) ? x : 0)
    return {
      totalTokens: num(v.totalTokens),
      inputTokens: num(v.inputTokens),
      cachedInputTokens: num(v.cachedInputTokens),
      cacheWriteInputTokens: num(v.cacheWriteInputTokens),
      outputTokens: num(v.outputTokens),
      reasoningOutputTokens: num(v.reasoningOutputTokens),
    }
  }
  return {
    total: bucket(raw.total),
    last: bucket(raw.last),
    contextWindow: typeof raw.modelContextWindow === 'number' ? raw.modelContextWindow : 0,
  }
}

/** 从 unified diff 里按文件拆出增删行数，给 Changes 面板用。 */
export function parseUnifiedDiff(diff) {
  const text = String(diff || '')
  if (!text.trim()) return []
  const files = []
  let current = null
  for (const line of text.split('\n')) {
    if (line.startsWith('diff --git ')) {
      if (current) files.push(current)
      const match = line.match(/ b\/(.+)$/)
      current = { path: match ? match[1] : line.slice(11), added: 0, removed: 0, patch: [line] }
      continue
    }
    if (!current) continue
    current.patch.push(line)
    if (line.startsWith('+++') || line.startsWith('---')) continue
    if (line.startsWith('+')) current.added += 1
    else if (line.startsWith('-')) current.removed += 1
  }
  if (current) files.push(current)
  return files.map((f) => ({ ...f, patch: f.patch.join('\n') }))
}

function contentPreview(content) {
  if (!Array.isArray(content)) return ''
  const text = content
    .filter((c) => c?.type === 'text')
    .map((c) => String(c.text || ''))
    .join(' ')
  const extras = content.filter((c) => c?.type !== 'text').length
  return extras ? `${text}${text ? ' ' : ''}[${extras} 个附件]`.trim() : text
}

export const useAgentWorkspaceStore = defineStore('agentWorkspace', {
  state: () => {
    const saved = readStore()
    const settings = {
      model: '',
      effort: '',
      permission: 'workspace',
      cwd: '',
      mode: 'code',
      ...(saved.settings || {}),
    }
    return {
      settings,
      /** codex 线程索引（thread/list）。 */
      threads: [],
      archivedThreads: [],
      nextCursor: null,
      archivedCursor: null,
      loadingMore: false,
      sections: [],
      view: 'active', // active | archived
      searchTerm: '',
      currentThreadId: String(saved.currentThreadId || ''),

      /** threadId → 已加载的线程正文与运行态。 */
      threadStates: {},
      /** 还没建线程时的空白会话。 */
      draftState: emptyThreadState({ id: '' }),

      models: [],
      permissionProfiles: [],
      skills: [],
      mcpServers: [],
      engine: null,
      engineError: '',
      listening: false,

      approvals: [],
      /** 按线程记录的输入历史（上下键切换）。 */
      historyByThread: saved.historyByThread || {},
      /** 未发送的草稿。 */
      drafts: saved.drafts || {},
      seenWarnings: [],
      /** 最近一次 git 概览的刷新时间，避免每个通知都去跑 git。 */
      gitCheckedAt: 0,
    }
  },

  getters: {
    currentThreadSummary(state) {
      return state.threads.find((t) => t.id === state.currentThreadId)
        || state.archivedThreads.find((t) => t.id === state.currentThreadId)
        || null
    },
    currentState(state) {
      if (!state.currentThreadId) return state.draftState
      return state.threadStates[state.currentThreadId] || state.draftState
    },
    currentIsDraft(state) {
      return !state.currentThreadId
    },
    currentRunning() {
      return String(this.currentState?.status || '') === 'active'
    },
    permissionPreset(state) {
      return PRESET_BY_ID.get(state.settings.permission) || PERMISSION_PRESETS[1]
    },
    currentModel(state) {
      const id = this.currentState?.model || state.settings.model
      return state.models.find((m) => m.model === id || m.id === id) || null
    },
    /** 左栏按分区聚合。没有分区时归到「最近」。 */
    threadGroups(state) {
      const list = state.view === 'archived' ? state.archivedThreads : state.threads
      const groups = []
      const bySection = new Map()
      for (const section of state.sections) {
        bySection.set(section.id, {
          id: section.id,
          name: section.name,
          color: section.appearance?.color || '',
          threads: [],
        })
      }
      const loose = { id: '', name: state.sections.length ? '未分组' : '最近', color: '', threads: [] }
      for (const thread of list) {
        const section = thread.section
        if (section?.id && bySection.has(section.id)) {
          bySection.get(section.id).threads.push(thread)
        } else {
          loose.threads.push(thread)
        }
      }
      for (const group of bySection.values()) {
        // 空分区也要出现：它是拖放的目标，也是「先建分区再归置」的入口。
        groups.push(group)
      }
      if (loose.threads.length || !state.sections.length) groups.push(loose)
      return groups
    },
    runningThreadIds(state) {
      return Object.values(state.threadStates)
        .filter((s) => normalizeStatus(s.status) === 'active')
        .map((s) => s.id)
    },
    pendingApprovalCount(state) {
      return state.approvals.length
    },
    /** 当前线程的 diff 文件清单；turn diff 没到时退回 fileChange items。 */
    currentDiffFiles() {
      const state = this.currentState
      if (!state) return []
      const fromDiff = parseUnifiedDiff(state.diff)
      if (fromDiff.length) return fromDiff
      const map = new Map()
      for (const turn of state.turns || []) {
        for (const item of turn.items || []) {
          if (item.type !== 'fileChange' || !Array.isArray(item.changes)) continue
          for (const change of item.changes) {
            const path = String(change?.path || '')
            if (!path) continue
            const counts = countPatch(change?.diff)
            const prev = map.get(path)
            map.set(path, {
              path,
              kind: change?.kind || 'update',
              added: (prev?.added || 0) + counts.added,
              removed: (prev?.removed || 0) + counts.removed,
              patch: change?.diff || '',
            })
          }
        }
      }
      return [...map.values()]
    },
    contextUsage() {
      const usage = this.currentState?.usage
      if (!usage?.total) return { used: 0, window: 0, percent: 0 }
      const used = Number(usage.total.totalTokens || 0)
      const window = Number(usage.contextWindow || 0)
      return {
        used,
        window,
        percent: window > 0 ? Math.min(100, Math.round((used / window) * 100)) : 0,
      }
    },
    currentInputHistory(state) {
      const key = state.currentThreadId || '__draft__'
      return state.historyByThread[key] || []
    },
    runStats() {
      const state = this.currentState
      const counts = { commands: 0, files: 0, tools: 0, searches: 0, failed: 0 }
      for (const turn of state?.turns || []) {
        for (const item of turn.items || []) {
          if (item.type === 'commandExecution') counts.commands += 1
          else if (item.type === 'fileChange') counts.files += 1
          else if (item.type === 'mcpToolCall' || item.type === 'dynamicToolCall') counts.tools += 1
          else if (item.type === 'webSearch') counts.searches += 1
          if (item.status === 'failed') counts.failed += 1
        }
      }
      return counts
    },
    approvalsForCurrent(state) {
      if (!state.currentThreadId) return state.approvals
      return state.approvals.filter(
        (a) => !a.threadId || a.threadId === state.currentThreadId,
      )
    },
  },

  actions: {
    // ───────────────────────── 启动 / 持久化

    async bootstrap() {
      this.settings.cwd = this.settings.cwd || ''
      await this.ensureListening()
      const ok = await this.loadEngine()
      if (ok) {
        await Promise.all([
          this.loadModels(),
          this.loadSections(),
          this.loadThreads(),
          this.loadPermissionProfiles(),
        ])
        // 恢复上次打开的会话；线程可能已被删除，失败就退回草稿。
        if (this.currentThreadId) {
          const found = this.threads.some((t) => t.id === this.currentThreadId)
          if (found) await this.openThread(this.currentThreadId).catch(() => this.newDraft())
          else this.newDraft()
        }
        this.refreshConnectors().catch(() => {})
      }
      this.persist()
      return ok
    },

    persist() {
      writeStore({
        settings: this.settings,
        currentThreadId: this.currentThreadId,
        historyByThread: this.historyByThread,
        drafts: this.drafts,
      })
    },

    setSetting(key, value) {
      this.settings[key] = value
      this.persist()
    },

    setModel(model) {
      this.settings.model = String(model || '')
      if (this.currentThreadId) {
        const state = this.threadStates[this.currentThreadId]
        if (state) state.model = this.settings.model
      }
      this.persist()
    },

    setEffort(effort) {
      this.settings.effort = String(effort || '')
      this.persist()
    },

    setMode(mode) {
      this.settings.mode = String(mode || 'code')
      this.persist()
    },

    setCwd(cwd) {
      const value = String(cwd || '')
      this.settings.cwd = value
      // turn/start 支持每轮覆盖 cwd；同步到当前线程，目录 chip 与实际执行位置才不会脱节。
      const state = this.currentThreadId ? this.threadStates[this.currentThreadId] : this.draftState
      if (state) state.cwd = value
      this.persist()
    },

    setPermission(id) {
      if (!PRESET_BY_ID.has(id)) return
      this.settings.permission = id
      this.persist()
    },

    setDraft(text) {
      const key = this.currentThreadId || '__draft__'
      this.drafts[key] = String(text || '')
      this.persist()
    },

    getDraft() {
      const key = this.currentThreadId || '__draft__'
      return this.drafts[key] || ''
    },

    pushHistory(text) {
      const value = String(text || '').trim()
      if (!value) return
      const key = this.currentThreadId || '__draft__'
      const list = this.historyByThread[key] || []
      if (list[list.length - 1] === value) return
      list.push(value)
      if (list.length > MAX_HISTORY) list.splice(0, list.length - MAX_HISTORY)
      this.historyByThread[key] = list
      this.persist()
    },

    // ───────────────────────── 引擎

    async loadEngine() {
      try {
        this.engine = await invoke('agent_engine_status')
        this.engineError = this.engine?.binary_exists
          ? ''
          : this.engine?.hint || '未找到 codex 可执行文件'
      } catch (err) {
        this.engineError = String(err?.message || err)
        logger.error('[agent] engine status failed', err)
        return false
      }
      if (!this.engine?.binary_exists) return false
      if (!this.engine?.running) {
        try {
          await this.warmUpEngine()
        } catch (err) {
          logger.warn('[agent] engine warm-up failed', { error: String(err?.message || err) })
        }
      }
      return true
    },

    async warmUpEngine() {
      try {
        await this.ensureListening()
        const res = await invoke('agent_list_models', { includeHidden: false })
        this.models = Array.isArray(res?.data) ? res.data : []
        this.engineError = ''
        await this.refreshEngineState()
        return true
      } catch (err) {
        this.engineError = String(err?.message || err)
        logger.warn('[agent] engine unavailable', { error: this.engineError })
        return false
      }
    },

    async refreshEngineState() {
      try {
        this.engine = await invoke('agent_engine_status')
      } catch {
        // 状态刷新失败不影响已建立的连接。
      }
      return this.engine
    },

    async loadModels() {
      if (this.models.length) return this.models
      try {
        const res = await invoke('agent_list_models', { includeHidden: false })
        this.models = Array.isArray(res?.data) ? res.data : []
      } catch (err) {
        logger.warn('[agent] model list failed', { error: String(err?.message || err) })
      }
      return this.models
    },

    async loadPermissionProfiles() {
      try {
        const res = await invoke('agent_list_permission_profiles')
        this.permissionProfiles = Array.isArray(res?.data) ? res.data : []
      } catch (err) {
        logger.warn('[agent] permission profiles failed', { error: String(err?.message || err) })
      }
      return this.permissionProfiles
    },

    async retryEngine() {
      this.engineError = ''
      this.models = []
      const ok = await this.warmUpEngine()
      if (!ok) throw new Error(this.engineError || '引擎仍不可用')
      await this.loadThreads()
      return this.engine
    },

    async restartEngine() {
      this.engineError = ''
      try {
        this.engine = await invoke('agent_restart_engine')
        this.threadStates = {}
        await this.loadThreads()
      } catch (err) {
        this.engineError = String(err?.message || err)
        throw err
      }
    },

    // ───────────────────────── 事件

    async ensureListening() {
      if (this.listening) return
      this.listening = true
      try {
        await listen('agent://notification', (event) => this.onNotification(event?.payload))
        await listen('agent://approval', (event) => this.onApproval(event?.payload))
        logger.info('[agent] event listeners attached')
      } catch (err) {
        this.listening = false
        logger.error('[agent] attach listeners failed', err)
      }
    },

    stateFor(threadId) {
      const id = String(threadId || '')
      if (!id) return this.draftState
      let state = this.threadStates[id]
      if (!state) {
        state = emptyThreadState({ id })
        this.threadStates[id] = state
      }
      return state
    },

    onNotification(envelope) {
      const kind = String(envelope?.kind || '')
      const p = envelope?.payload || {}
      const threadId = String(envelope?.threadId || '')
      const turnId = String(envelope?.turnId || '')
      if (!kind) return

      // 线程列表事件先处理：它们可能发生在正文还没加载时。
      if (kind === 'thread/started') {
        const thread = p?.thread
        if (thread?.id) this.mergeThreadSummary(thread)
      }
      if (kind === 'thread/deleted') {
        this.threads = this.threads.filter((t) => t.id !== threadId)
        this.archivedThreads = this.archivedThreads.filter((t) => t.id !== threadId)
        delete this.threadStates[threadId]
        if (this.currentThreadId === threadId) this.newDraft()
        return
      }
      if (kind === 'thread/archived') {
        this.threads = this.threads.filter((t) => t.id !== threadId)
        this.loadThreads({ archived: true }).catch(() => {})
        return
      }
      if (kind === 'thread/unarchived') {
        this.loadThreads({ archived: true }).catch(() => {})
        this.loadThreads().catch(() => {})
        return
      }

      const state = this.stateFor(threadId)
      if (threadId && threadId !== this.currentThreadId) state.unread = true

      switch (kind) {
        case 'thread/name/updated': {
          const name = String(p?.threadName || p?.name || '')
          if (name) {
            state.name = name
            const summary = this.threads.find((t) => t.id === threadId)
            if (summary) summary.name = name
          }
          break
        }
        case 'thread/status/changed':
          state.status = normalizeStatus(p?.status)
          break
        case 'thread/settings/updated': {
          const s = p?.threadSettings || {}
          if (s.model) state.model = String(s.model)
          if (s.cwd) state.cwd = String(s.cwd)
          if (s.sandboxPolicy?.type) {
            const type = String(s.sandboxPolicy.type)
            state.sandbox = type === 'dangerFullAccess'
              ? 'danger-full-access'
              : type === 'readOnly' ? 'read-only' : 'workspace-write'
          }
          if (s.approvalPolicy) {
            state.approvalPolicy = typeof s.approvalPolicy === 'string'
              ? s.approvalPolicy
              : 'on-request'
          }
          if (s.effort) state.effort = String(s.effort)
          break
        }
        case 'thread/tokenUsage/updated':
          state.usage = normalizeUsage(p?.tokenUsage)
          break
        case 'turn/started':
          this.startServerTurn(threadId, p?.turn)
          break
        case 'turn/completed':
          this.completeServerTurn(threadId, p?.turn)
          break
        case 'item/started':
          this.upsertItem(threadId, turnId, p?.item, { started: true })
          break
        case 'item/completed':
          this.upsertItem(threadId, turnId, p?.item, { completed: true })
          break
        case 'item/agentMessage/delta':
          this.appendAgentDelta(state, turnId, String(p?.itemId || ''), String(p?.delta || ''))
          break
        case 'item/reasoning/summaryPartAdded':
          this.appendReasoningPart(state, turnId, String(p?.itemId || ''), Number(p?.summaryIndex || 0))
          break
        case 'item/reasoning/summaryTextDelta':
          this.appendReasoningSummary(
            state,
            turnId,
            String(p?.itemId || ''),
            Number(p?.summaryIndex || 0),
            String(p?.delta || ''),
          )
          break
        case 'item/reasoning/textDelta':
          this.appendReasoningText(
            state,
            turnId,
            String(p?.itemId || ''),
            Number(p?.contentIndex || 0),
            String(p?.delta || ''),
          )
          break
        case 'item/commandExecution/outputDelta':
          this.appendToItem(state, turnId, String(p?.itemId || ''), 'output', String(p?.delta || ''))
          break
        case 'item/commandExecution/terminalInteraction':
          this.appendToItem(
            state,
            turnId,
            String(p?.itemId || ''),
            'output',
            `\n${String(p?.stdin || '')}\n`,
          )
          break
        case 'item/fileChange/outputDelta':
          this.appendToItem(state, turnId, String(p?.itemId || ''), 'output', String(p?.delta || ''))
          break
        case 'item/fileChange/patchUpdated':
          this.patchFileChange(state, turnId, String(p?.itemId || ''), p?.changes)
          break
        case 'item/mcpToolCall/progress':
          this.appendToItem(
            state,
            turnId,
            String(p?.itemId || ''),
            'progress',
            String(p?.message || ''),
          )
          break
        case 'item/plan/delta':
          this.appendToItem(state, turnId, String(p?.itemId || ''), 'text', String(p?.delta || ''), 'plan')
          break
        case 'turn/plan/updated':
          state.plan = (Array.isArray(p?.plan) ? p.plan : []).map((step, i) => ({
            id: String(step?.id || i),
            text: String(step?.step || step?.text || ''),
            status: String(step?.status || 'pending'),
          }))
          break
        case 'turn/diff/updated':
          state.diff = String(p?.diff || '')
          this.refreshGitSoon(state)
          break
        case 'thread/compacted':
          state.turns.push({
            id: uid('compact'),
            status: 'completed',
            items: [{ id: uid('compact-item'), type: 'contextCompaction', status: 'completed', startedAt: now() }],
            startedAt: now(),
            completedAt: now(),
            durationMs: 0,
            local: true,
          })
          break
        case 'model/rerouted':
          this.note(state, `模型由 ${p?.fromModel || '?'} 切换到 ${p?.toModel || '?'}。`)
          break
        case 'error': {
          const msg = String(p?.error?.message || p?.error || '未知错误')
          state.error = msg
          this.note(state, `运行出错：${msg}`)
          if (!p?.willRetry) {
            state.status = 'systemError'
          }
          break
        }
        case 'warning': {
          const msg = String(p?.message || '')
          if (msg && !this.seenWarnings.includes(msg)) {
            this.seenWarnings.push(msg)
            if (this.seenWarnings.length > 40) this.seenWarnings.shift()
            this.note(state, `提示：${msg}`)
          }
          break
        }
        case 'serverRequest/resolved':
          this.approvals = this.approvals.filter(
            (a) => Number(a.requestId) !== Number(p?.requestId),
          )
          break
        default:
          break
      }
    },

    onApproval(payload) {
      const requestId = Number(payload?.requestId)
      if (!Number.isFinite(requestId)) return
      if (this.approvals.some((a) => a.requestId === requestId)) return
      this.approvals.push({
        requestId,
        kind: String(payload?.kind || 'other'),
        method: String(payload?.method || ''),
        threadId: String(payload?.threadId || ''),
        turnId: String(payload?.turnId || ''),
        params: payload?.params || {},
        at: Number(payload?.at || now()),
      })
      logger.info('[agent] approval requested', { requestId, kind: payload?.kind })
    },

    async resolveApproval(requestId, action, options = {}) {
      const approval = this.approvals.find((a) => a.requestId === requestId)
      this.approvals = this.approvals.filter((a) => a.requestId !== requestId)
      if (!approval) return
      const decision = buildApprovalDecision(approval, action, options)
      try {
        await invoke('agent_resolve_approval', { requestId, decision })
      } catch (err) {
        logger.warn('[agent] resolve approval failed', {
          requestId,
          error: String(err?.message || err),
        })
      }
    },

    // ───────────────────────── 线程

    async loadThreads({ archived = false, searchTerm = '' } = {}) {
      const params = {
        limit: THREAD_PAGE_SIZE,
        sortKey: 'recency_at',
      }
      if (archived) params.archived = true
      if (searchTerm.trim()) params.searchTerm = searchTerm.trim()
      const res = await invoke('agent_list_threads', { params })
      const list = Array.isArray(res?.data) ? res.data : []
      if (archived) {
        this.archivedThreads = list
        this.archivedCursor = res?.nextCursor || null
      } else {
        this.threads = list
        this.nextCursor = res?.nextCursor || null
      }
      for (const thread of list) this.mergeThreadSummary(thread)
      return list
    },

    async loadMoreThreads() {
      const archived = this.view === 'archived'
      const cursor = archived ? this.archivedCursor : this.nextCursor
      if (!cursor) return
      const params = { limit: THREAD_PAGE_SIZE, sortKey: 'recency_at', cursor }
      if (archived) params.archived = true
      const res = await invoke('agent_list_threads', { params })
      const list = Array.isArray(res?.data) ? res.data : []
      if (archived) {
        this.archivedThreads.push(...list)
        this.archivedCursor = res?.nextCursor || null
      } else {
        this.threads.push(...list)
        this.nextCursor = res?.nextCursor || null
      }
      for (const thread of list) this.mergeThreadSummary(thread)
    },

    async searchThreads(term) {
      this.searchTerm = String(term || '')
      try {
        await this.loadThreads({ archived: this.view === 'archived', searchTerm: this.searchTerm })
      } catch (err) {
        logger.warn('[agent] thread search failed', { error: String(err?.message || err) })
      }
    },

    setView(view) {
      this.view = view === 'archived' ? 'archived' : 'active'
      this.loadThreads({ archived: this.view === 'archived', searchTerm: this.searchTerm }).catch(() => {})
    },

    async loadSections() {
      try {
        const res = await invoke('agent_section_list')
        this.sections = Array.isArray(res?.data) ? res.data : []
      } catch (err) {
        logger.warn('[agent] section list failed', { error: String(err?.message || err) })
      }
      return this.sections
    },

    async createSection(name) {
      const res = await invoke('agent_section_create', {
        name: String(name || '').trim() || '新分区',
      })
      const section = res?.section
      if (section) this.sections.push(section)
      return section
    },

    async renameSection(sectionId, name) {
      const existing = this.sections.find((s) => s.id === sectionId)
      await invoke('agent_section_update', {
        sectionId,
        name: String(name || '').trim() || '分区',
        icon: existing?.appearance?.icon || null,
        color: existing?.appearance?.color || null,
      })
      if (existing) existing.name = String(name || '').trim() || '分区'
    },

    async deleteSection(sectionId) {
      await invoke('agent_section_delete', { sectionId })
      this.sections = this.sections.filter((s) => s.id !== sectionId)
      await this.loadThreads({ archived: false })
    },

    async moveThreadToSection(threadId, sectionId) {
      await invoke('agent_thread_move_section', {
        threadId,
        sectionId: sectionId || null,
      })
      await this.loadThreads({ archived: this.view === 'archived' })
    },

    mergeThreadSummary(thread) {
      if (!thread?.id) return
      const summary = {
        ...thread,
        status: thread.status || 'idle',
        section: thread.section || null,
      }
      const list = thread.archived ? this.archivedThreads : this.threads
      const idx = list.findIndex((t) => t.id === thread.id)
      if (idx >= 0) list[idx] = { ...list[idx], ...summary }
      else if (!thread.archived) list.unshift(summary)

      const state = this.stateFor(thread.id)
      state.name = String(thread.name || state.name || '')
      state.preview = String(thread.preview || state.preview || '')
      state.cwd = String(thread.cwd || state.cwd || '')
      state.modelProvider = String(thread.modelProvider || state.modelProvider || '')
      state.status = normalizeStatus(thread.status)
      state.createdAt = Number(thread.createdAt || state.createdAt || 0)
      state.updatedAt = Number(thread.updatedAt || state.updatedAt || 0)
      state.gitInfo = thread.gitInfo || state.gitInfo
    },

    async openThread(threadId) {
      const id = String(threadId || '')
      if (!id) return
      if (this.currentThreadId && this.currentThreadId !== id) {
        const prev = this.threadStates[this.currentThreadId]
        if (prev) prev.unread = false
      }
      this.currentThreadId = id
      this.persist()
      const state = this.stateFor(id)
      if (state.loading) return
      state.loading = true
      state.error = ''
      try {
        // resume 才会让 app-server 订阅这个线程；read 只读历史，发下一轮会被拒。
        const res = await invoke('agent_resume_thread', { threadId: id })
        const thread = res?.thread
        if (thread) this.applyThread(thread, state)
        state.loaded = true
        state.unread = false
        if (!state.usage) {
          // resume 不带用量；历史会话要等下一次 turn 才有 token 统计。
        }
        this.refreshGit(state).catch(() => {})
        this.refreshConnectors(id).catch(() => {})
        if (state.status === 'active') state.unread = false
      } catch (err) {
        // 已归档或已删除的线程 resume 可能失败，退回只读。
        try {
          const res = await invoke('agent_read_thread', { threadId: id, includeTurns: true })
          if (res?.thread) this.applyThread(res.thread, state)
          state.loaded = true
        } catch (readErr) {
          state.error = String(readErr?.message || readErr)
          logger.warn('[agent] open thread failed', { id, error: state.error })
        }
      } finally {
        state.loading = false
      }
    },

    applyThread(thread, state) {
      const target = state || this.stateFor(thread?.id)
      if (!thread?.id || !target) return
      target.id = String(thread.id)
      target.name = String(thread.name || '')
      target.preview = String(thread.preview || '')
      target.cwd = String(thread.cwd || '')
      target.model = String(thread.model || '')
      target.modelProvider = String(thread.modelProvider || '')
      target.sandbox = String(thread.sandbox || thread.sandboxPolicy?.type || '')
      target.approvalPolicy = String(thread.approvalPolicy || '')
      target.effort = String(thread.reasoningEffort || thread.effort || '')
      target.status = normalizeStatus(thread.status)
      target.gitInfo = thread.gitInfo || null
      target.createdAt = Number(thread.createdAt || 0)
      target.updatedAt = Number(thread.updatedAt || 0)
      if (Array.isArray(thread.turns)) {
        target.turns = thread.turns.map(normalizeTurn).filter(Boolean)
      }
      this.mergeThreadSummary(thread)
    },

    async renameThread(threadId, name) {
      const value = String(name || '').trim()
      if (!value) return
      await invoke('agent_rename_thread', { threadId, name: value })
      const state = this.stateFor(threadId)
      state.name = value
      const summary = this.threads.find((t) => t.id === threadId)
      if (summary) summary.name = value
    },

    async archiveThread(threadId) {
      await invoke('agent_archive_thread', { threadId })
      this.threads = this.threads.filter((t) => t.id !== threadId)
      if (this.currentThreadId === threadId) this.newDraft()
    },

    async unarchiveThread(threadId) {
      await invoke('agent_unarchive_thread', { threadId })
      this.archivedThreads = this.archivedThreads.filter((t) => t.id !== threadId)
      await this.loadThreads()
    },

    async deleteThread(threadId) {
      await invoke('agent_delete_thread', { threadId })
      this.threads = this.threads.filter((t) => t.id !== threadId)
      this.archivedThreads = this.archivedThreads.filter((t) => t.id !== threadId)
      delete this.threadStates[threadId]
      if (this.currentThreadId === threadId) this.newDraft()
    },

    async forkThread(threadId, lastTurnId = '') {
      const res = await invoke('agent_fork_thread', {
        threadId,
        lastTurnId: lastTurnId || null,
      })
      const thread = res?.thread
      if (thread) {
        this.mergeThreadSummary(thread)
        await this.openThread(thread.id)
      }
      return thread
    },

    async compactThread() {
      const id = this.currentThreadId
      if (!id) return
      await invoke('agent_compact_thread', { threadId: id })
    },

    async startReview(target = { type: 'uncommittedChanges' }) {
      const id = this.currentThreadId
      if (!id) return
      await invoke('agent_review_start', {
        threadId: id,
        target,
        delivery: 'inline',
      })
    },

    newDraft() {
      if (this.currentThreadId) {
        const state = this.threadStates[this.currentThreadId]
        if (state) state.unread = false
      }
      this.currentThreadId = ''
      this.draftState = emptyThreadState({ id: '', status: 'idle' })
      this.persist()
    },

    // ───────────────────────── 正文增量

    startServerTurn(threadId, turn) {
      const state = this.stateFor(threadId)
      if (!state) return
      state.status = 'active'
      if (!turn?.id) return
      const existing = state.turns.find(
        (t) => t.id === turn.id || (t.local && t.status === 'inProgress'),
      )
      if (existing) {
        const localItems = existing.items
        Object.assign(existing, normalizeTurn(turn), { items: localItems, local: false })
        existing.status = 'inProgress'
      } else {
        const normalized = normalizeTurn(turn) || {
          id: turn.id,
          status: 'inProgress',
          items: [],
          startedAt: now(),
          completedAt: 0,
          durationMs: 0,
        }
        normalized.status = 'inProgress'
        state.turns.push(normalized)
      }
    },

    completeServerTurn(threadId, turn) {
      const state = this.stateFor(threadId)
      if (!state) return
      if (turn?.id) {
        const existing = state.turns.find((t) => t.id === turn.id)
        if (existing) {
          existing.status = String(turn.status || 'completed')
          existing.error = turn.error || null
          existing.completedAt = Number(turn.completedAt || 0)
          existing.durationMs = Number(turn.durationMs || 0)
          if (Array.isArray(turn.items) && turn.items.length) {
            for (const item of turn.items) {
              this.upsertItem(threadId, turn.id, item, { completed: true })
            }
          }
        } else {
          const normalized = normalizeTurn(turn)
          if (normalized) state.turns.push(normalized)
        }
      }
      const running = state.turns.some((t) => t.status === 'inProgress')
      if (!running) {
        state.status = 'idle'
        for (const t of state.turns) {
          for (const item of t.items) item.streaming = false
        }
        this.refreshGit(state).catch(() => {})
      }
      if (statesTurnFailed(turn)) {
        state.error = String(turn?.error?.message || '本次运行失败')
      }
      this.persist()
    },

    findTurn(state, turnId) {
      if (!state) return null
      if (turnId) {
        const exact = state.turns.find((t) => t.id === turnId)
        if (exact) return exact
      }
      for (let i = state.turns.length - 1; i >= 0; i -= 1) {
        if (state.turns[i].status === 'inProgress') return state.turns[i]
      }
      return state.turns[state.turns.length - 1] || null
    },

    ensureTurn(state, turnId) {
      const turn = this.findTurn(state, turnId)
      if (turn) return turn
      const created = {
        id: String(turnId || uid('turn')),
        status: 'inProgress',
        items: [],
        startedAt: now(),
        completedAt: 0,
        durationMs: 0,
        local: true,
      }
      state.turns.push(created)
      return created
    },

    ensureItem(state, turnId, itemId, type) {
      const turn = this.ensureTurn(state, turnId)
      let item = turn.items.find((x) => x.id === itemId)
      if (!item) {
        item = normalizeItem({ id: itemId, type }) || { id: itemId, type, status: 'inProgress' }
        item.status = 'inProgress'
        item.streaming = true
        turn.items.push(item)
      }
      return item
    },

    upsertItem(threadId, turnId, raw, { started = false, completed = false } = {}) {
      if (!raw) return
      const state = this.stateFor(threadId)
      if (!state) return
      const id = String(raw.id || '')
      if (!id) return
      const turn = this.ensureTurn(state, turnId)
      const idx = turn.items.findIndex((x) => x.id === id)
      const incoming = normalizeItem(raw)
      if (!incoming) return

      if (idx >= 0) {
        const existing = turn.items[idx]
        // completed 的 agentMessage 带最终全文，比逐字 delta 可信；
        // 但只在它不短于已累积内容时替换，避免某个协议版本给的是片段。
        if (incoming.type === 'agentMessage') {
          const finalText = String(incoming.text || '')
          if (finalText.length >= String(existing.text || '').length) existing.text = finalText
        } else if (incoming.type === 'reasoning') {
          if (incoming.summary?.some((s) => String(s).trim())) existing.summary = incoming.summary
          if (incoming.content?.some((s) => String(s).trim())) existing.content = incoming.content
        } else if (incoming.type === 'commandExecution') {
          if (incoming.command) existing.command = incoming.command
          if (incoming.output.length >= String(existing.output || '').length) existing.output = incoming.output
        } else if (incoming.type === 'plan') {
          if (incoming.text) existing.text = incoming.text
        } else {
          Object.assign(existing, incoming, { id })
        }
        existing.status = incoming.status || existing.status
        existing.streaming = false
        if (completed) existing.completedAt = now()
      } else {
        incoming.status = String(raw.status || (completed ? 'completed' : 'inProgress'))
        incoming.streaming = !completed && incoming.type === 'agentMessage'
        if (completed) incoming.completedAt = now()
        // 乐观插入的用户消息与服务器回流的同一条合并，避免正文出现两遍。
        if (incoming.type === 'userMessage') {
          const optimistic = turn.items.find(
            (x) => x.optimistic && x.text && x.text.trim() === incoming.text.trim(),
          )
          if (optimistic) {
            Object.assign(optimistic, incoming, { id: optimistic.id, optimistic: false })
            return
          }
        }
        turn.items.push(incoming)
      }

      if (incoming.type === 'fileChange' && Array.isArray(incoming.changes)) {
        this.refreshGitSoon(state)
      }
      if (started) state.unread = state.id !== this.currentThreadId
    },

    appendAgentDelta(state, turnId, itemId, delta) {
      if (!delta) return
      const item = this.ensureItem(state, turnId, itemId || uid('msg'), 'agentMessage')
      item.text = String(item.text || '') + delta
      item.streaming = true
    },

    appendReasoningPart(state, turnId, itemId, index) {
      const item = this.ensureItem(state, turnId, itemId || uid('reason'), 'reasoning')
      if (!Array.isArray(item.summary)) item.summary = []
      while (item.summary.length <= index) item.summary.push('')
      item.streaming = true
    },

    appendReasoningSummary(state, turnId, itemId, index, delta) {
      if (!delta) return
      const item = this.ensureItem(state, turnId, itemId || uid('reason'), 'reasoning')
      if (!Array.isArray(item.summary)) item.summary = []
      while (item.summary.length <= index) item.summary.push('')
      item.summary[index] = String(item.summary[index] || '') + delta
      item.streaming = true
    },

    appendReasoningText(state, turnId, itemId, index, delta) {
      if (!delta) return
      const item = this.ensureItem(state, turnId, itemId || uid('reason'), 'reasoning')
      if (!Array.isArray(item.content)) item.content = []
      while (item.content.length <= index) item.content.push('')
      item.content[index] = String(item.content[index] || '') + delta
      item.streaming = true
    },

    appendToItem(state, turnId, itemId, field, delta, type = 'commandExecution') {
      if (!itemId || !delta) return
      const item = this.ensureItem(state, turnId, itemId, type)
      item[field] = String(item[field] || '') + delta
    },

    patchFileChange(state, turnId, itemId, changes) {
      if (!itemId || !Array.isArray(changes)) return
      const item = this.ensureItem(state, turnId, itemId, 'fileChange')
      item.changes = changes
      this.refreshGitSoon(state)
    },

    note(state, text) {
      if (!state) return
      const turn = this.ensureTurn(state, '')
      turn.items.push({
        id: uid('note'),
        type: 'notice',
        status: 'completed',
        text: String(text || ''),
        startedAt: now(),
      })
    },

    // ───────────────────────── 发送 / 停止

    async sendTurn(text, { attachments = [] } = {}) {
      const trimmed = String(text || '').trim()
      if (!trimmed && !attachments.length) return
      const ok = await this.warmUpEngineIfNeeded()
      if (!ok) throw new Error(this.engineError || '引擎不可用')

      let state = this.currentState
      if (!this.currentThreadId) {
        const preset = this.permissionPreset
        const cwd = this.settings.cwd
        const res = await invoke('agent_start_thread', {
          request: {
            cwd,
            sandbox: preset.sandbox,
            approval_policy: preset.approvalPolicy,
            model: this.settings.model,
            mode: this.settings.mode,
          },
        })
        const thread = res?.thread
        if (!thread?.id) throw new Error('未能创建会话')
        this.currentThreadId = String(thread.id)
        state = this.stateFor(thread.id)
        this.applyThread(thread, state)
        if (res?.model) state.model = String(res.model)
        if (res?.cwd) state.cwd = String(res.cwd)
        this.mergeThreadSummary(thread)
      } else {
        state = this.stateFor(this.currentThreadId)
        if (!state.loaded) {
          await invoke('agent_resume_thread', { threadId: this.currentThreadId })
          state.loaded = true
        }
      }

      const preset = this.permissionPreset
      const content = []
      if (trimmed) content.push({ type: 'text', text: trimmed })
      for (const a of attachments) content.push(attachmentToInput(a))

      const turn = this.ensureTurn(state, '')
      turn.local = true
      turn.status = 'inProgress'
      turn.items.push({
        id: uid('user'),
        type: 'userMessage',
        status: 'completed',
        content,
        text: trimmed,
        optimistic: true,
        startedAt: now(),
      })
      state.status = 'active'
      this.pushHistory(trimmed)
      this.setDraft('')

      try {
        const res = await invoke('agent_start_turn', {
          request: {
            thread_id: this.currentThreadId,
            text: trimmed,
            model: this.settings.model,
            effort: this.settings.effort,
            summary: 'auto',
            cwd: state.cwd || this.settings.cwd,
            sandbox: preset.sandbox,
            approval_policy: preset.approvalPolicy,
            attachments: attachments.map(attachmentToRequest),
            client_user_message_id: uid('client'),
          },
        })
        const serverTurnId = String(res?.turn?.id || '')
        if (serverTurnId && turn.local) {
          turn.id = serverTurnId
          turn.local = false
        }
        if (!state.name && trimmed) {
          state.name = trimmed.slice(0, 32)
          invoke('agent_rename_thread', {
            threadId: this.currentThreadId,
            name: state.name,
          }).catch(() => {})
        }
      } catch (err) {
        turn.status = 'failed'
        state.status = 'idle'
        state.error = String(err?.message || err)
        this.note(state, `发送失败：${state.error}`)
        throw err
      }
    },

    async warmUpEngineIfNeeded() {
      if (this.engine?.running) return true
      return this.warmUpEngine()
    },

    async steer(text, { attachments = [] } = {}) {
      const state = this.currentState
      if (!this.currentThreadId) return
      const trimmed = String(text || '').trim()
      if (!trimmed && !attachments.length) return
      const running = (state.turns || []).find((t) => t.status === 'inProgress')
      const content = []
      if (trimmed) content.push({ type: 'text', text: trimmed })
      for (const a of attachments) content.push(attachmentToInput(a))
      const turn = this.ensureTurn(state, running?.id || '')
      turn.items.push({
        id: uid('user'),
        type: 'userMessage',
        status: 'completed',
        content,
        text: trimmed,
        steer: true,
        optimistic: true,
        startedAt: now(),
      })
      this.pushHistory(trimmed)
      try {
        await invoke('agent_steer_turn', {
          threadId: this.currentThreadId,
          turnId: running?.id || '',
          text: trimmed,
          attachments: attachments.map(attachmentToRequest),
        })
      } catch (err) {
        this.note(state, `转向失败：${String(err?.message || err)}`)
      }
    },

    async interrupt() {
      const state = this.currentState
      if (!this.currentThreadId) return
      try {
        await invoke('agent_interrupt_turn', { threadId: this.currentThreadId })
      } catch (err) {
        this.note(state, `中断失败：${String(err?.message || err)}`)
      } finally {
        state.status = 'idle'
        for (const turn of state.turns || []) {
          if (turn.status === 'inProgress') turn.status = 'interrupted'
          for (const item of turn.items || []) item.streaming = false
        }
      }
    },

    // ───────────────────────── 环境 / 连接器

    async refreshGit(state) {
      const cwd = state?.cwd || this.settings.cwd
      if (!cwd) return null
      try {
        const res = await invoke('agent_git_status', { cwd })
        state.git = res || null
      } catch (err) {
        logger.warn('[agent] git status failed', { error: String(err?.message || err) })
        state.git = null
      }
      this.gitCheckedAt = now()
      return state.git
    },

    refreshGitSoon(state) {
      if (now() - this.gitCheckedAt < 2500) return
      this.refreshGit(state).catch(() => {})
    },

    async refreshConnectors(threadId = '') {
      try {
        const res = await invoke('agent_list_mcp_servers', {
          threadId: threadId || null,
          detail: 'toolsAndAuthOnly',
        })
        this.mcpServers = Array.isArray(res?.data) ? res.data : []
      } catch (err) {
        logger.warn('[agent] mcp status failed', { error: String(err?.message || err) })
      }
      return this.mcpServers
    },

    async reloadConnectors() {
      await invoke('agent_reload_mcp_servers')
      await this.refreshConnectors(this.currentThreadId)
    },

    async loadSkills() {
      try {
        const res = await invoke('agent_list_skills', {
          cwds: this.currentState?.cwd ? [this.currentState.cwd] : [],
        })
        this.skills = Array.isArray(res?.data)
          ? res.data
          : Array.isArray(res?.skills) ? res.skills : []
      } catch (err) {
        logger.warn('[agent] skills list failed', { error: String(err?.message || err) })
      }
      return this.skills
    },

    async fuzzyFiles(query) {
      const cwd = this.currentState?.cwd || this.settings.cwd
      if (!cwd || !query) return []
      try {
        const res = await invoke('agent_fuzzy_file_search', {
          query,
          roots: [cwd],
          cancellationToken: null,
        })
        return Array.isArray(res?.files) ? res.files : []
      } catch (err) {
        logger.warn('[agent] file search failed', { error: String(err?.message || err) })
        return []
      }
    },

    async commitChanges(message) {
      const cwd = this.currentState?.cwd || this.settings.cwd
      if (!cwd) throw new Error('还没有工作目录')
      const res = await invoke('agent_commit_changes', {
        cwd,
        message: String(message || '').trim() || 'chore(agent): 提交本次改动',
      })
      const state = this.currentState
      await this.refreshGit(state)
      return res
    },
  },
})

function countPatch(patch) {
  let added = 0
  let removed = 0
  for (const line of String(patch || '').split('\n')) {
    if (line.startsWith('+') && !line.startsWith('+++')) added += 1
    else if (line.startsWith('-') && !line.startsWith('---')) removed += 1
  }
  return { added, removed }
}

function statesTurnFailed(turn) {
  return String(turn?.status || '') === 'failed'
}

/** 附件从 UI 结构转成后端 TurnStartRequest.attachments。 */
function attachmentToRequest(a) {
  return {
    type: String(a?.type || 'mention'),
    path: String(a?.path || ''),
    url: String(a?.url || ''),
    name: String(a?.name || ''),
    text: String(a?.text || ''),
    detail: String(a?.detail || ''),
  }
}

/** 乐观回显用：与 UserInput 同形，但不带服务端字段。 */
function attachmentToInput(a) {
  const type = String(a?.type || 'mention')
  if (type === 'localImage' || type === 'localAudio') {
    return { type, path: String(a?.path || ''), name: String(a?.name || '') }
  }
  if (type === 'image' || type === 'audio') {
    return { type, url: String(a?.url || ''), name: String(a?.name || '') }
  }
  return { type, path: String(a?.path || ''), name: String(a?.name || '') }
}

/**
 * 把审批 UI 的选择翻译成 codex 期望的回包。
 *
 * 四种审批的 schema 完全不同：命令/文件是 `{decision}`，权限是
 * `{permissions, scope}`，用户输入是 `{answers:{id:{answers:[]}}}`，
 * MCP elicitation 是 `{action, content, _meta}`。
 */
function buildApprovalDecision(approval, action, options) {
  const kind = String(approval?.kind || '')
  const method = String(approval?.method || '')
  const allow = action === 'accept' || action === 'acceptForSession'
  const isInput = kind === 'input' || method.includes('requestUserInput')

  if (method === 'mcpServer/elicitation/request') {
    return {
      action: allow ? 'accept' : 'decline',
      content: allow ? (options?.content ?? null) : null,
      _meta: null,
    }
  }

  if (kind === 'permission') {
    if (!allow) return { permissions: {}, scope: 'turn' }
    const permissions = options?.permissions ?? approval?.params?.permissions ?? {}
    return {
      permissions,
      scope: action === 'acceptForSession' ? 'session' : 'turn',
    }
  }

  if (isInput) {
    // 审批浮层已经给出 `{id: {answers: []}}`；这里兼容直接给字符串数组的调用方，
    // 但绝不能把对象再包一层，否则模型收到的是 "[object Object]"。
    const answers = {}
    for (const [key, value] of Object.entries(options?.answers || {})) {
      if (value && typeof value === 'object' && Array.isArray(value.answers)) {
        answers[key] = { answers: value.answers.map(String) }
      } else {
        answers[key] = { answers: Array.isArray(value) ? value.map(String) : [String(value)] }
      }
    }
    return { answers }
  }

  // command / file / legacy：直接给 decision 字符串。
  return { decision: allow ? (action === 'acceptForSession' ? 'acceptForSession' : 'accept') : 'decline' }
}
