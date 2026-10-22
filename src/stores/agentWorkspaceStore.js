import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { createLogger } from '../utils/logger'

const logger = createLogger('agentWorkspaceStore')

/**
 * Agent 工作台的引擎客户端与运行态。
 *
 * 设计说明（为什么不复用 rightSiderStore）
 *
 * 右侧助手是「一条消息 → 一次回答」的模型，它的 store 里 everything 都是消息数组；
 * 工作台是「一次 run 里有一串 item、一个计划、若干审批、一份 diff」。硬塞进同一份结构，
 * 两边都会被对方的字段污染。这里只抽出**引擎通信**这一层共用（sendTurn/stop/approve），
 * 视图状态各管各的。
 *
 * 事件契约由后端 `codex_app_server.rs` 定义，前端不认 codex 原始字段名：
 * - `agent://notification` → { kind, payload, at }
 * - `agent://approval`     → { requestId, kind, method, params, at }
 */

/** 工作模式预设。与后端 `mode_instructions` 一一对应。 */
export const AGENT_MODES = Object.freeze([
  {
    id: 'explore',
    label: '探索',
    sandbox: 'read-only',
    hint: '只读检索，产出结论与出处',
  },
  {
    id: 'code',
    label: '编码',
    sandbox: 'workspace-write',
    hint: '改文件、跑测试、看 diff',
  },
  {
    id: 'research',
    label: '研究',
    sandbox: 'read-only',
    hint: '先查知识库再联网，区分出处',
  },
  {
    id: 'computer',
    label: '操作电脑',
    sandbox: 'workspace-write',
    hint: '截屏、点按、输入（需逐次确认）',
    requiresGui: true,
  },
])

const MODE_BY_ID = new Map(AGENT_MODES.map((m) => [m.id, m]))

const SESSIONS_KEY = 'guaxi:agent:sessions:v1'
const SETTINGS_KEY = 'guaxi:agent:settings:v1'

/** 一次 run 的账本条目上限。转录本身不限，账本只是"这次跑了什么"的速览。 */
const LEDGER_LIMIT = 200

const now = () => Date.now()

function makeSession(seed = {}) {
  const t = now()
  return {
    key: String(seed.key || `agent-${t}-${Math.random().toString(36).slice(2, 8)}`),
    threadId: String(seed.threadId || ''),
    title: String(seed.title || '新任务'),
    mode: MODE_BY_ID.has(seed.mode) ? seed.mode : 'code',
    cwd: String(seed.cwd || ''),
    model: String(seed.model || ''),
    effort: String(seed.effort || ''),
    // 会话级沙箱与审批策略：线程建立时下发，之后随 thread 走。
    sandbox: String(seed.sandbox || 'workspace-write'),
    approvalPolicy: String(seed.approvalPolicy || 'on-request'),
    messages: Array.isArray(seed.messages) ? seed.messages : [],
    ledger: Array.isArray(seed.ledger) ? seed.ledger : [],
    plan: Array.isArray(seed.plan) ? seed.plan : [],
    usage: seed.usage && typeof seed.usage === 'object' ? seed.usage : null,
    diff: String(seed.diff || ''),
    status: String(seed.status || 'idle'),
    createdAt: Number(seed.createdAt || t),
    updatedAt: Number(seed.updatedAt || t),
  }
}

function readJson(key, fallback) {
  if (typeof localStorage === 'undefined') return fallback
  try {
    const raw = localStorage.getItem(key)
    if (!raw) return fallback
    return JSON.parse(raw) ?? fallback
  } catch (err) {
    logger.warn('[agent] read local state failed', { key, error: String(err) })
    return fallback
  }
}

function writeJson(key, value) {
  if (typeof localStorage === 'undefined') return
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch (err) {
    // 配额满时不能打断运行：丢掉持久化，内存里的状态仍是对的。
    logger.warn('[agent] persist local state failed', { key, error: String(err) })
  }
}

export const useAgentWorkspaceStore = defineStore('agentWorkspace', {
  state: () => {
    const savedSessions = readJson(SESSIONS_KEY, null)
    const sessions = Array.isArray(savedSessions?.sessions) && savedSessions.sessions.length
      ? savedSessions.sessions.map((s) => makeSession(s))
      : [makeSession({ title: '新任务' })]
    const currentKey = sessions.some((s) => s.key === savedSessions?.currentKey)
      ? savedSessions.currentKey
      : sessions[0].key

    const settings = readJson(SETTINGS_KEY, {}) || {}

    return {
      sessions,
      currentKey,
      /** 引擎是否空闲；运行中不允许再发。 */
      running: false,
      runningSince: 0,
      /** 当前回合的 id，steer / interrupt 都要它。 */
      currentTurnId: '',
      /** 待裁决的审批。同一时刻可能有多条（并行工具调用）。 */
      approvals: [],
      /** 引擎状态：null 表示还没查。 */
      engine: null,
      /** 引擎报错（无法启动、供应商未配），用于在界面上给可读原因。 */
      engineError: '',
      models: [],
      settings: {
        showGridOverlay: true,
        // 操作电脑模式的网格密度。模型用「第 N 格」表达位置，密度决定精度与可读性的平衡。
        gridCols: 12,
        gridRows: 8,
        ...settings,
      },
      listening: false,
      /** 已展示过的引擎告警（按原文去重）。 */
      seenWarnings: [],
    }
  },

  getters: {
    currentSession(state) {
      return state.sessions.find((s) => s.key === state.currentKey) || state.sessions[0] || null
    },
    currentMode(state) {
      const s = state.sessions.find((x) => x.key === state.currentKey)
      return MODE_BY_ID.get(s?.mode) || AGENT_MODES[1]
    },
    pendingApprovalCount(state) {
      return state.approvals.length
    },
    /** 账本里最近一次 run 的统计，给仪表用。 */
    runStats(state) {
      const s = state.sessions.find((x) => x.key === state.currentKey)
      const ledger = Array.isArray(s?.ledger) ? s.ledger : []
      const counts = { command: 0, file: 0, tool: 0, search: 0, failed: 0 }
      for (const item of ledger) {
        if (!item || item.kind !== 'item') continue
        const t = String(item.itemType || '')
        if (t === 'commandExecution') counts.command += 1
        else if (t === 'fileChange') counts.file += 1
        else if (t === 'mcpToolCall' || t === 'dynamicToolCall') counts.tool += 1
        else if (t === 'webSearch') counts.search += 1
        if (item.status === 'failed') counts.failed += 1
      }
      return counts
    },
  },

  actions: {
    // ───────────── 会话

    newSession(seed = {}) {
      const s = makeSession({ mode: this.currentMode?.id || 'code', ...seed })
      this.sessions.unshift(s)
      this.currentKey = s.key
      this.persist()
      return s
    },

    selectSession(key) {
      if (!this.sessions.some((s) => s.key === key)) return
      this.currentKey = key
      this.persist()
    },

    deleteSession(key) {
      const i = this.sessions.findIndex((s) => s.key === key)
      if (i < 0) return
      this.sessions.splice(i, 1)
      if (!this.sessions.length) this.sessions.push(makeSession())
      if (this.currentKey === key) this.currentKey = this.sessions[0].key
      this.persist()
    },

    patchSession(patch) {
      const s = this.currentSession
      if (!s) return
      Object.assign(s, patch, { updatedAt: now() })
      this.persist()
    },

    touch() {
      const s = this.currentSession
      if (s) s.updatedAt = now()
    },

    persist() {
      writeJson(SESSIONS_KEY, { sessions: this.sessions, currentKey: this.currentKey })
    },

    setSetting(key, value) {
      this.settings[key] = value
      writeJson(SETTINGS_KEY, this.settings)
    },

    // ───────────── 引擎

    async loadEngine() {
      try {
        this.engine = await invoke('agent_engine_status')
        this.engineError = this.engine?.binary_exists ? '' : (this.engine?.hint || '未找到 codex 可执行文件')
      } catch (err) {
        this.engineError = String(err?.message || err)
        logger.error('[agent] engine status failed', err)
      }
      return this.engine
    },

    async loadModels() {
      try {
        const res = await invoke('agent_list_models')
        this.models = Array.isArray(res?.data) ? res.data : []
      } catch (err) {
        // 模型清单拿不到不影响使用：用户仍可手填模型名。
        logger.warn('[agent] model list failed', { error: String(err?.message || err) })
        this.models = []
      }
      return this.models
    },

    async restartEngine() {
      this.engineError = ''
      try {
        this.engine = await invoke('agent_restart_engine')
        return this.engine
      } catch (err) {
        this.engineError = String(err?.message || err)
        throw err
      }
    },

    // ───────────── 监听

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

    onNotification(payload) {
      const kind = String(payload?.kind || '')
      const p = payload?.payload || {}
      const session = this.currentSession
      if (!session) return

      switch (kind) {
        case 'thread/started':
          if (p?.thread?.id && !session.threadId) {
            session.threadId = String(p.thread.id)
            this.persist()
          }
          break

        case 'turn/started':
          this.running = true
          this.runningSince = now()
          this.currentTurnId = String(p?.turn?.id || '')
          session.status = 'running'
          session.plan = []
          session.ledger = []
          session.diff = ''
          break

        case 'turn/completed':
          this.running = false
          this.currentTurnId = ''
          session.status = String(p?.turn?.status || 'idle')
          this.persist()
          break

        case 'item/started':
          this.upsertLedgerItem(p?.item, { started: true })
          break

        case 'item/completed':
          this.upsertLedgerItem(p?.item, { completed: true })
          break

        case 'item/agentMessage/delta':
          this.appendAssistantDelta(String(p?.delta || ''), String(p?.itemId || ''))
          break

        case 'item/plan/delta':
        case 'turn/plan/updated':
          this.applyPlan(p)
          break

        case 'turn/diff/updated':
          session.diff = String(p?.diff || '')
          break

        case 'thread/tokenUsage/updated':
          session.usage = this.normalizeUsage(p?.tokenUsage)
          break

        case 'thread/status/changed':
          if (p?.status?.type) session.status = String(p.status.type)
          break

        case 'error': {
          const msg = String(p?.error?.message || p?.error || '未知错误')
          this.appendSystemNote(`运行出错：${msg}`)
          logger.warn('[agent] engine error', { kind, message: msg })
          break
        }

        case 'warning': {
          // 引擎的降级告警（例如某个可选组件缺失）。它不影响主流程，但藏起来会让
          // 「为什么某个能力不生效」变成谜——所以照原样给用户看，只是标注为提示。
          const msg = String(p?.message || '')
          // 同一条告警每次建线程都会再来一次；内存里记一份，避免刷屏。
          if (msg && !this.seenWarnings.includes(msg)) {
            this.seenWarnings.push(msg)
            if (this.seenWarnings.length > 40) this.seenWarnings.shift()
            this.appendSystemNote(`提示：${msg}`)
            logger.warn('[agent] engine warning', { message: msg })
          }
          break
        }

        default:
          // 未识别的通知不丢弃也不报错：上游是实验性协议，多出来的通知是常态。
          break
      }
    },

    onApproval(payload) {
      const requestId = Number(payload?.requestId)
      if (!Number.isFinite(requestId)) return
      // 同一个请求重复到达（重连）时不重复入队。
      if (this.approvals.some((a) => a.requestId === requestId)) return
      this.approvals.push({
        requestId,
        kind: String(payload?.kind || 'other'),
        method: String(payload?.method || ''),
        params: payload?.params || {},
        at: Number(payload?.at || now()),
      })
      logger.info('[agent] approval requested', {
        requestId,
        kind: payload?.kind,
        method: payload?.method,
      })
    },

    /** 回一条裁决。decision 的结构随审批类型变化，原样透传给后端。 */
    async resolveApproval(requestId, decision) {
      this.approvals = this.approvals.filter((a) => a.requestId !== requestId)
      try {
        await invoke('agent_resolve_approval', { requestId, decision })
      } catch (err) {
        logger.warn('[agent] resolve approval failed', { requestId, error: String(err?.message || err) })
      }
    },

    // ───────────── 账本 / 计划 / 用量

    upsertLedgerItem(item, { started = false, completed = false } = {}) {
      if (!item) return
      const session = this.currentSession
      if (!session) return
      const id = String(item.id || '')
      if (!id) return
      // 用户消息、hook 提示这类 item 也会走 item/started|completed（实测），
      // 但它们已经在转录里了，再记进账本就重复了。账本只收「机器做的事」。
      const type = String(item.type || '')
      if (type === 'userMessage' || type === 'hookPrompt') return
      const idx = session.ledger.findIndex((x) => x.id === id)
      const status = String(item.status || (completed ? 'completed' : 'inProgress'))
      const entry = {
        kind: 'item',
        id,
        itemType: String(item.type || (idx >= 0 ? session.ledger[idx].itemType : '')),
        status,
        title: this.describeItem(item),
        detail: this.itemDetail(item),
        at: now(),
      }
      if (idx >= 0) {
        // 保留首次出现时的类型：某些 item 在 completed 时不再带 type。
        entry.itemType = entry.itemType || session.ledger[idx].itemType
        Object.assign(session.ledger[idx], entry)
      } else {
        session.ledger.push(entry)
      }
      if (session.ledger.length > LEDGER_LIMIT) {
        session.ledger.splice(0, session.ledger.length - LEDGER_LIMIT)
      }
      // 文件改动同时喂给 diff 摘要：即使 turn/diff/updated 没到，账本也能显示改了哪些文件。
      if (entry.itemType === 'fileChange' && Array.isArray(item.changes)) {
        this.applyFileChanges(item.changes)
      }
      if (started) this.touch()
    },

    describeItem(item) {
      const type = String(item?.type || '')
      switch (type) {
        case 'commandExecution':
          return String(item?.command || '执行命令').slice(0, 160)
        case 'fileChange': {
          const n = Array.isArray(item?.changes) ? item.changes.length : 0
          return n ? `修改 ${n} 个文件` : '修改文件'
        }
        case 'mcpToolCall':
          return `${item?.server || 'MCP'} · ${item?.tool || '工具'}`
        case 'dynamicToolCall':
          return String(item?.tool || '工具')
        case 'webSearch':
          return `搜索：${item?.query || ''}`
        case 'reasoning':
          return '思考'
        case 'plan':
          return '更新计划'
        case 'imageView':
          return '查看图片'
        case 'agentMessage': {
          const text = String(item?.text || '').trim()
          return text ? text.slice(0, 80) : '回复'
        }
        default:
          return type || '事件'
      }
    },

    itemDetail(item) {
      const type = String(item?.type || '')
      if (type === 'commandExecution') {
        const out = String(item?.aggregatedOutput || '')
        const code = item?.exitCode
        return {
          command: String(item?.command || ''),
          cwd: String(item?.cwd || ''),
          output: out.slice(-4000),
          exitCode: typeof code === 'number' ? code : null,
          durationMs: Number(item?.durationMs || 0),
        }
      }
      if (type === 'fileChange') {
        return { changes: Array.isArray(item?.changes) ? item.changes : [] }
      }
      if (type === 'mcpToolCall' || type === 'dynamicToolCall') {
        return { args: item?.arguments ?? null, result: item?.result ?? null, error: item?.error ?? null }
      }
      if (type === 'webSearch') {
        return { query: item?.query || '', results: item?.results ?? null }
      }
      return null
    },

    applyFileChanges(changes) {
      const session = this.currentSession
      if (!session) return
      for (const c of changes) {
        const path = String(c?.path || '')
        if (!path) continue
        const existing = session.ledger.find((x) => x.kind === 'file' && x.path === path)
        const added = Number(c?.addedLines || 0)
        const removed = Number(c?.removedLines || 0)
        if (existing) {
          existing.added = added
          existing.removed = removed
        } else {
          session.ledger.push({
            kind: 'file',
            id: `file:${path}`,
            path,
            added,
            removed,
            at: now(),
          })
        }
      }
    },

    applyPlan(payload) {
      const session = this.currentSession
      if (!session) return
      const plan = payload?.plan
      if (Array.isArray(plan)) {
        session.plan = plan.map((step, i) => ({
          id: String(step?.id || i),
          text: String(step?.text || step?.step || ''),
          status: String(step?.status || 'pending'),
        }))
      } else if (typeof payload?.delta === 'string') {
        session.plan.push({ id: `p${session.plan.length}`, text: payload.delta, status: 'pending' })
      }
    },

    normalizeUsage(raw) {
      if (!raw || typeof raw !== 'object') return null
      const num = (v) => (typeof v === 'number' && Number.isFinite(v) ? v : 0)
      return {
        inputTokens: num(raw.inputTokens ?? raw.input_tokens),
        outputTokens: num(raw.outputTokens ?? raw.output_tokens),
        totalTokens: num(raw.totalTokens ?? raw.total_tokens),
        contextWindow: num(raw.contextWindow ?? raw.context_window),
      }
    },

    // ───────────── 消息

    appendUserMessage(text, extra = {}) {
      const s = this.currentSession
      if (!s) return
      s.messages.push({
        id: `u-${now()}-${Math.random().toString(36).slice(2, 6)}`,
        role: 'user',
        text: String(text || ''),
        at: now(),
        ...extra,
      })
      this.touch()
    },

    ensureStreamingMessage(itemId) {
      const s = this.currentSession
      if (!s) return null
      const id = `a-${itemId || 'stream'}`
      let msg = s.messages.find((m) => m.id === id)
      if (!msg) {
        msg = { id, role: 'assistant', text: '', streaming: true, at: now() }
        s.messages.push(msg)
      }
      return msg
    },

    appendAssistantDelta(delta, itemId) {
      if (!delta) return
      const msg = this.ensureStreamingMessage(itemId)
      if (!msg) return
      msg.text += delta
      this.touch()
    },

    finishStreamingMessages() {
      const s = this.currentSession
      if (!s) return
      for (const m of s.messages) {
        if (m.streaming) m.streaming = false
      }
      this.persist()
    },

    appendSystemNote(text) {
      const s = this.currentSession
      if (!s) return
      s.messages.push({
        id: `n-${now()}-${Math.random().toString(36).slice(2, 6)}`,
        role: 'system',
        text: String(text || ''),
        at: now(),
      })
      this.touch()
    },

    // ───────────── 发送 / 停止

    async sendTurn(text, { images = [] } = {}) {
      const s = this.currentSession
      if (!s || this.running) return
      const trimmed = String(text || '').trim()
      if (!trimmed && !images.length) return

      await this.ensureListening()

      // 首次发送时建线程。工作目录取当前会话设置；模式决定沙箱与说明。
      if (!s.threadId) {
        const mode = MODE_BY_ID.get(s.mode) || AGENT_MODES[1]
        try {
          const summary = await invoke('agent_start_thread', {
            request: {
              cwd: s.cwd,
              sandbox: s.sandbox || mode.sandbox,
              approval_policy: s.approvalPolicy || 'on-request',
              model: s.model || '',
              mode: s.mode,
            },
          })
          s.threadId = String(summary?.thread_id || '')
          if (summary?.cwd) s.cwd = String(summary.cwd)
          if (summary?.model) s.model = String(summary.model)
          if (summary?.sandbox) s.sandbox = String(summary.sandbox)
          this.persist()
        } catch (err) {
          const msg = String(err?.message || err)
          this.engineError = msg
          this.appendSystemNote(`无法启动会话：${msg}`)
          throw err
        }
      }

      this.appendUserMessage(trimmed, { images })
      this.running = true
      this.runningSince = now()
      s.status = 'running'
      s.ledger = []
      s.plan = []
      s.diff = ''

      try {
        const res = await invoke('agent_start_turn', {
          request: {
            thread_id: s.threadId,
            text: trimmed,
            model: s.model || '',
            effort: s.effort || '',
            images,
          },
        })
        this.currentTurnId = String(res?.turn?.id || '')
      } catch (err) {
        this.running = false
        s.status = 'failed'
        this.appendSystemNote(`发送失败：${String(err?.message || err)}`)
        throw err
      }
    },

    /** 中途改指令：不打断当前回合，把新要求插进去。 */
    async steer(text) {
      const s = this.currentSession
      if (!s?.threadId || !this.currentTurnId) return
      const trimmed = String(text || '').trim()
      if (!trimmed) return
      this.appendUserMessage(trimmed, { steer: true })
      try {
        await invoke('agent_steer_turn', {
          threadId: s.threadId,
          turnId: this.currentTurnId,
          text: trimmed,
        })
      } catch (err) {
        this.appendSystemNote(`转向失败：${String(err?.message || err)}`)
      }
    },

    async interrupt() {
      const s = this.currentSession
      if (!s?.threadId) return
      try {
        await invoke('agent_interrupt_turn', { threadId: s.threadId })
        this.appendSystemNote('已中断本次运行。')
      } catch (err) {
        this.appendSystemNote(`中断失败：${String(err?.message || err)}`)
      } finally {
        this.running = false
        this.currentTurnId = ''
        this.finishStreamingMessages()
      }
    },

    /** 会话标题：取首条用户消息的前若干字，比「新任务」好认。 */
    suggestTitle() {
      const s = this.currentSession
      if (!s) return
      if (s.title && s.title !== '新任务') return
      const first = s.messages.find((m) => m.role === 'user' && m.text)
      if (first) {
        s.title = first.text.slice(0, 24)
        this.persist()
      }
    },
  },
})
