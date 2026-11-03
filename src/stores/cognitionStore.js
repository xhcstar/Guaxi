import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../utils/logger'

const logger = createLogger('cognitionStore')

/** 提醒种类的中文标签与配色，看板与简报共用。 */
export const NUDGE_KINDS = {
  contradiction: { label: '矛盾', color: '#e04f5f', hint: '新材料反驳了你既有的结论' },
  gap: { label: '结构缺口', color: '#d98a2b', hint: '读了很多，但没有一处把它串起来' },
  due: { label: '到期', color: '#3b82c4', hint: '任务将到期，附上它需要的材料' },
  review: { label: '复习', color: '#5b9e6a', hint: '按遗忘曲线该回看的笔记' },
  emerging: { label: '新主题', color: '#8b6bc9', hint: '一批新材料聚成了知识库里还没有的主题' },
  stale: { label: '停滞', color: '#7a7a7a', hint: '项目很久没动，还留着悬空的问题' },
}

export const PERCEPT_SOURCE_LABELS = {
  crawl: '整站抓取',
  rss: 'RSS 订阅',
  capture: '采集（插件/手机）',
  file: '本地文件',
  clipboard: '剪贴板',
  screen: '屏幕',
  manual: '手动录入',
  chat: '对话',
  // 用户自有材料（M2）：不占收件箱，但参与打分、锚定与召回。
  note: '摘录笔记',
  library: '资料库文档',
}

export const useCognitionStore = defineStore('cognitionStore', {
  state: () => ({
    /**
     * 大脑页当前视图：'cognition'（收件箱/提醒/简报）或 'graph'（3D 图谱）。
     * 放在 store 里而不是组件内，是因为顶栏的切换按钮、中间内容区、左侧边栏
     * 三处都要读它——左栏在两个模式下展示的东西完全不同。
     */
    viewMode: 'cognition',

    /** cognition_status 的完整返回，null 表示还没拉过。 */
    status: null,
    statusLoading: false,
    statusError: '',

    /** 看板：未处理的提醒。 */
    nudges: [],
    nudgesLoading: false,
    nudgeFilterKind: '',

    /** 感知收件箱。 */
    percepts: [],
    perceptsLoading: false,
    perceptFilter: { state: 'new', sourceKind: '', minSalience: null },

    /** 今日简报。 */
    brief: null,
    briefLoading: false,
    briefHistory: [],

    /** 画像。 */
    profile: null,
    profileLoading: false,

    /** 信念。 */
    beliefs: [],
    beliefsLoading: false,

    /** 工具面与授权。 */
    tools: [],
    sources: [],

    /** 进化闩。 */
    evalRuns: [],
    evalBusy: false,

    /** 活动总线：最近事件与概览（设置页调试视图）。 */
    activity: [],
    activityStats: null,
    activityLoading: false,

    /** 长期记忆：活跃条目、memory.md 路径与最近一次巩固结果。 */
    memories: [],
    memoriesLoading: false,
    memoryBusy: false,
    memoryFilePath: '',
    memoryReport: null,

    /** 学习看板：主题掌握度、复习队列与阅读统计。 */
    learningOverview: null,
    learningLoading: false,
    learningRefreshing: false,
    /** 上次成功拉取学习概览的时刻（毫秒），用于切页签时的去抖刷新。 */
    learningLoadedAt: 0,

    /** 引证排序器状态（训练/回滚用）。 */
    rankerStatus: null,
    rankerBusy: false,

    /** 当前会话的接地引证：episodeId -> citations[]。 */
    groundedByEpisode: {},
    /** 最近一次接地（供聊天视图直接渲染）。 */
    lastGrounded: null,

    /** 未读的推送提醒数（托盘/角标用）。 */
    unreadPush: 0,
  }),

  getters: {
    enabled: (s) => Boolean(s.status?.enabled),
    embeddingReady: (s) => Boolean(s.status?.embeddingReady),
    openNudgeCount: (s) => s.nudges.length,
    contradictionCount: (s) => s.nudges.filter((n) => n.kind === 'contradiction').length,
    pendingScoreCount: (s) => Number(s.status?.percept?.pendingScore || 0),
    filteredNudges: (s) =>
      s.nudgeFilterKind ? s.nudges.filter((n) => n.kind === s.nudgeFilterKind) : s.nudges,
    /** 今日看板:值得读的感知(显著优先,未打分的新内容兜底)。 */
    topPercepts: (s) => {
      const scored = s.percepts.filter((p) => Number(p.salience) > 0.25)
      return (scored.length ? scored : s.percepts).slice(0, 3)
    },
  },

  actions: {
    setViewMode(mode) {
      this.viewMode = mode === 'graph' ? 'graph' : 'cognition'
    },

    toggleViewMode() {
      this.setViewMode(this.viewMode === 'graph' ? 'cognition' : 'graph')
    },

    async refreshStatus() {
      this.statusLoading = true
      this.statusError = ''
      try {
        this.status = await invoke('cognition_status')
      } catch (e) {
        this.statusError = String(e)
        logger.warn('cognition_status failed', e)
      } finally {
        this.statusLoading = false
      }
    },

    async setEnabled(enabled) {
      await invoke('cognition_set_enabled', { enabled })
      await this.refreshStatus()
    },

    /**
     * 一键让语义服务就绪：启动本地嵌入服务（首次会下载模型，走既有下载链路）。
     * 就绪后打分、新颖度、召回、矛盾检测才可用。返回是否成功。
     */
    async ensureEmbeddingReady() {
      try {
        await invoke('kg_ensure_embedding_service')
        await this.refreshStatus()
        return this.embeddingReady
      } catch (e) {
        logger.warn('kg_ensure_embedding_service failed', e)
        throw e
      }
    },

    async setSetting(key, value) {
      await invoke('cognition_set_setting', { key, value: String(value) })
      await this.refreshStatus()
    },

    // ── 看板

    async refreshNudges({ state = 'open', limit = 60 } = {}) {
      this.nudgesLoading = true
      try {
        this.nudges = await invoke('cognition_nudge_list', { state, limit })
      } catch (e) {
        logger.warn('cognition_nudge_list failed', e)
        this.nudges = []
      } finally {
        this.nudgesLoading = false
      }
    },

    /** 采纳/忽略一条提醒。两者都是策略模型的训练标签，所以忽略也要如实上报。 */
    async decideNudge(id, accepted) {
      await invoke('cognition_nudge_decide', { id, accepted })
      this.nudges = this.nudges.filter((n) => n.id !== id)
      this.refreshStatus()
    },

    async deliberateNow(modelid) {
      const report = await invoke('cognition_deliberate_now', { modelid: modelid ?? null })
      await Promise.all([this.refreshNudges(), this.refreshStatus()])
      return report
    },

    async reviewFeedback(nodeId, remembered) {
      await invoke('cognition_review_feedback', { nodeId, remembered })
    },

    // ── 感知收件箱

    async refreshPercepts(overrides = {}) {
      this.perceptsLoading = true
      const f = { ...this.perceptFilter, ...overrides }
      this.perceptFilter = f
      try {
        const list = await invoke('cognition_percept_list', {
          query: {
            state: f.state || null,
            sourceKind: f.sourceKind || null,
            minSalience: f.minSalience ?? null,
            limit: 60,
          },
        })
        // 显著性优先、时间兜底:未打分的条目沉到后面,不再把"最新抓的"顶在最上面。
        this.percepts = [...(list || [])].sort(
          (a, b) =>
            (Number(b.salience) || 0) - (Number(a.salience) || 0) ||
            String(b.capturedAt || '').localeCompare(String(a.capturedAt || '')),
        )
      } catch (e) {
        logger.warn('cognition_percept_list failed', e)
        this.percepts = []
      } finally {
        this.perceptsLoading = false
      }
    },

    async setPerceptState(id, state, reason) {
      await invoke('cognition_percept_set_state', { id, state, reason: reason ?? null })
      this.percepts = this.percepts.filter((p) => p.id !== id)
      this.refreshStatus()
    },

    async getPerceptText(id) {
      return invoke('cognition_percept_get', { id })
    },

    /** 把一条感知提炼成笔记写进知识库。成功后它从「未处理」里消失。 */
    async perceptToNote(id, kbName, folder) {
      const r = await invoke('cognition_percept_to_note', {
        id,
        kbName,
        folder: folder || null,
      })
      this.percepts = this.percepts.filter((p) => p.id !== id)
      await Promise.all([this.refreshStatus(), this.refreshPercepts()])
      return r
    },

    /** 提醒 → 3 道自测题。 */
    async nudgeQuiz(id) {
      return invoke('cognition_nudge_quiz', { id })
    },

    /** 提醒 → 任务管理里的一条任务。 */
    async nudgeToTask(id, category = 'work') {
      return invoke('cognition_nudge_to_task', { id, category })
    },

    /** 手动触发一批打分（"立即处理"按钮）。 */
    async scorePercepts(limit = 24) {
      const n = await invoke('cognition_score_percepts', { limit })
      await Promise.all([this.refreshPercepts(), this.refreshStatus()])
      return n
    },

    async capture(sourceKind, text, { title = '', uri = '' } = {}) {
      const r = await invoke('cognition_capture', {
        sourceKind,
        text,
        title: title || null,
        uri: uri || null,
      })
      this.refreshStatus()
      return r
    },

    async purgePercepts(keepDays) {
      const n = await invoke('cognition_purge_percepts', {
        keepDays: keepDays ?? null,
      })
      await Promise.all([this.refreshPercepts(), this.refreshStatus()])
      return n
    },

    async refreshSources() {
      try {
        this.sources = await invoke('cognition_sources')
      } catch (e) {
        logger.warn('cognition_sources failed', e)
        this.sources = []
      }
    },

    async setSourceEnabled(sourceKind, enabled) {
      await invoke('cognition_set_source_enabled', { sourceKind, enabled })
      await this.refreshSources()
    },

    // ── 简报

    async loadBrief(day, kind = 'morning') {
      this.briefLoading = true
      try {
        this.brief = await invoke('cognition_brief_get', { day: day ?? null, kind })
        if (this.brief) {
          await invoke('cognition_brief_mark_opened', { day: this.brief.day, kind })
          // 左栏的「新」角标与未读数读的是 briefHistory；看完简报要就地翻成已读，
          // 否则内容区已经读过了，左栏还挂着一个「新」。
          this.briefHistory = this.briefHistory.map((b) =>
            b.day === this.brief.day && (b.kind ?? 'morning') === (this.brief.kind ?? kind)
              ? { ...b, opened: true }
              : b,
          )
        }
      } catch (e) {
        logger.warn('cognition_brief_get failed', e)
        this.brief = null
      } finally {
        this.briefLoading = false
      }
      return this.brief
    },

    async generateBrief(kind = 'morning', modelid) {
      this.briefLoading = true
      try {
        this.brief = await invoke('cognition_brief_generate', {
          kind,
          modelid: modelid ?? null,
        })
        await this.refreshBriefHistory()
        return this.brief
      } finally {
        this.briefLoading = false
      }
    },

    async refreshBriefHistory(limit = 30) {
      try {
        this.briefHistory = await invoke('cognition_brief_list', { limit })
      } catch (e) {
        logger.warn('cognition_brief_list failed', e)
        this.briefHistory = []
      }
    },

    // ── 画像

    async loadProfile() {
      this.profileLoading = true
      try {
        this.profile = await invoke('cognition_profile_get')
      } catch (e) {
        logger.warn('cognition_profile_get failed', e)
      } finally {
        this.profileLoading = false
      }
      return this.profile
    },

    async saveProfile(markdown) {
      this.profile = await invoke('cognition_profile_save', { markdown })
      this.refreshStatus()
      return this.profile
    },

    async upsertInterest(topic, weight) {
      await invoke('cognition_interest_upsert', { topic, weight })
      await this.loadProfile()
    },

    async removeInterest(topic) {
      await invoke('cognition_interest_remove', { topic })
      await this.loadProfile()
    },

    // ── 信念

    async refreshBeliefs({ kbName = '', status = 'active', limit = 80 } = {}) {
      this.beliefsLoading = true
      try {
        this.beliefs = await invoke('cognition_belief_list', {
          kbName: kbName || null,
          status: status || null,
          limit,
        })
      } catch (e) {
        logger.warn('cognition_belief_list failed', e)
        this.beliefs = []
      } finally {
        this.beliefsLoading = false
      }
    },

    async extractBeliefs(kbName, { modelid, maxSections = 20 } = {}) {
      const r = await invoke('cognition_belief_extract', {
        kbName,
        modelid: modelid ?? null,
        maxSections,
      })
      await Promise.all([this.refreshBeliefs({ kbName }), this.refreshStatus()])
      return r
    },

    async setBeliefStatus(id, status) {
      await invoke('cognition_belief_set_status', { id, status })
      const hit = this.beliefs.find((b) => b.id === id)
      if (hit) hit.status = status
      this.refreshStatus()
    },

    async removeBelief(id) {
      await invoke('cognition_belief_remove', { id })
      this.beliefs = this.beliefs.filter((b) => b.id !== id)
      this.refreshStatus()
    },

    async beliefEvidence(beliefId) {
      return invoke('cognition_belief_evidence', { beliefId })
    },

    async detectContradictions({ modelid, maxJudge = 4 } = {}) {
      const found = await invoke('cognition_detect_contradictions', {
        modelid: modelid ?? null,
        maxJudge,
      })
      await this.refreshStatus()
      return found
    },

    // ── 召回 / 引证

    async recall(query, options = {}) {
      return invoke('cognition_recall', { query, options })
    },

    /** 接地事件回调：存下本轮引证，供聊天视图渲染引证条。 */
    applyGrounded(evt) {
      if (!evt?.episodeId) return
      this.groundedByEpisode = {
        ...this.groundedByEpisode,
        [evt.episodeId]: evt.citations || [],
      }
      this.lastGrounded = evt
    },

    /**
     * 用户点开了某条引证。
     *
     * 这是检索权重学习里最强的正标签——比"答案里出现了编号"更能说明这段真的有用，
     * 所以前端每次跳转都必须上报，哪怕失败也只记日志、不打扰用户。
     */
    async reportCitationClick(episodeId, refId) {
      if (!episodeId || !refId) return
      try {
        await invoke('cognition_citation_clicked', { episodeId, refId })
      } catch (e) {
        logger.debug('cognition_citation_clicked failed', e)
      }
    },

    /** 对话流结束后上报完整答案，用于回填"哪条引证真被用上"。 */
    async finishChat(episodeId, answer, { outcome = 'ok', tokensOut = 0, latencyMs = 0 } = {}) {
      if (!episodeId) return
      try {
        await invoke('cognition_chat_finish', {
          episodeId,
          answer: answer || '',
          outcome,
          tokensOut,
          latencyMs,
        })
      } catch (e) {
        logger.debug('cognition_chat_finish failed', e)
      }
    },

    // ── 工具面与 MCP

    async refreshTools() {
      try {
        this.tools = await invoke('cognition_tools_list')
      } catch (e) {
        logger.warn('cognition_tools_list failed', e)
        this.tools = []
      }
    },

    async setTier(tier, granted) {
      // 乐观更新：开关必须立刻动。写库是一次 SQLite upsert（毫秒级），
      // 慢的是它后面那串状态重读——以前 await 完整体刷新，用户看到的是
      // 「点了没反应」。这里先就地翻转，再写库，失败时回滚。
      const prevTiers = Array.isArray(this.status?.tiers)
        ? this.status.tiers.map(([k, v]) => [k, v])
        : null
      const prevTools = this.tools.map((t) => ({ ...t }))

      this.applyTierLocal(tier, granted)
      try {
        await invoke('cognition_tool_set_tier', { tier, granted })
      } catch (e) {
        if (prevTiers) this.status = { ...this.status, tiers: prevTiers }
        this.tools = prevTools
        throw e
      }
      // 状态里的统计数字与服务无关，后台补一下即可，不阻塞开关。
      this.refreshStatus().catch(() => {})
    },

    /** 只改本地状态：等级行与工具行的 enabled 字段。 */
    applyTierLocal(tier, granted) {
      if (this.status && Array.isArray(this.status.tiers)) {
        this.status = {
          ...this.status,
          tiers: this.status.tiers.map(([k, v]) => [k, k === tier ? granted : v]),
        }
      }
      this.tools = this.tools.map((t) => (t.tier === tier ? { ...t, enabled: granted } : t))
    },

    async callTool(name, args = {}) {
      return invoke('cognition_tool_call', { name, args })
    },

    async toolAudit(limit = 100) {
      return invoke('cognition_tool_audit', { limit })
    },

    async mcpInfo() {
      return invoke('cognition_mcp_info')
    },

    // ── 进化闩

    async refreshEvalRuns(target = '', limit = 50) {
      try {
        this.evalRuns = await invoke('cognition_eval_runs', {
          target: target || null,
          limit,
        })
      } catch (e) {
        logger.warn('cognition_eval_runs failed', e)
        this.evalRuns = []
      }
    },

    async mineEvalSet(kbName, { modelid, targetN = 40 } = {}) {
      this.evalBusy = true
      try {
        const r = await invoke('cognition_eval_mine', {
          kbName,
          modelid: modelid ?? null,
          targetN,
        })
        await this.refreshStatus()
        return r
      } finally {
        this.evalBusy = false
      }
    },

    async runEval(limit = 200) {
      this.evalBusy = true
      try {
        const r = await invoke('cognition_eval_run', { limit })
        await this.refreshEvalRuns()
        return r
      } finally {
        this.evalBusy = false
      }
    },

    /** 调参：只有在评测集上不退化才会落库，返回的 note 说明是否采纳。 */
    async tuneRecall(limit = 200) {
      this.evalBusy = true
      try {
        const r = await invoke('cognition_eval_tune', { limit })
        await Promise.all([this.refreshEvalRuns(), this.refreshStatus()])
        return r
      } finally {
        this.evalBusy = false
      }
    },

    async rollbackRecallWeights() {
      await invoke('cognition_recall_weights_rollback')
      await this.refreshStatus()
    },

    async trainPolicy() {
      const r = await invoke('cognition_policy_train')
      await this.refreshStatus()
      return r
    },

    async resetPolicy() {
      await invoke('cognition_policy_reset')
      await Promise.all([this.refreshNudges(), this.refreshStatus()])
    },

    async maintenance(request) {
      const r = await invoke('cognition_maintenance', { request })
      await this.refreshStatus()
      return r
    },

    // ── 活动总线

    async loadActivity({ limit = 60, app = '', kind = '' } = {}) {
      this.activityLoading = true
      try {
        this.activity = await invoke('cognition_activity_recent', {
          limit,
          app: app || null,
          kind: kind || null,
        })
      } catch (e) {
        logger.warn('cognition_activity_recent failed', e)
        this.activity = []
      } finally {
        this.activityLoading = false
      }
    },

    async refreshActivityStats(days = 7) {
      try {
        this.activityStats = await invoke('cognition_activity_stats', { days })
      } catch (e) {
        logger.warn('cognition_activity_stats failed', e)
        this.activityStats = null
      }
    },

    // ── 长期记忆

    async loadMemories({ kind = '', status = 'active', limit = 200 } = {}) {
      this.memoriesLoading = true
      try {
        this.memories = await invoke('cognition_memory_list', {
          kind: kind || null,
          status: status || 'active',
          limit,
        })
      } catch (e) {
        logger.warn('cognition_memory_list failed', e)
        this.memories = []
      } finally {
        this.memoriesLoading = false
      }
    },

    async loadMemoryFilePath() {
      try {
        this.memoryFilePath = await invoke('cognition_memory_file_path')
      } catch (e) {
        logger.warn('cognition_memory_file_path failed', e)
        this.memoryFilePath = ''
      }
    },

    async addMemory(kind, text) {
      const memory = await invoke('cognition_memory_add', { kind, text })
      await this.loadMemories()
      return memory
    },

    async forgetMemory(id) {
      await invoke('cognition_memory_forget', { id })
      await this.loadMemories()
    },

    async consolidateMemory() {
      this.memoryBusy = true
      try {
        this.memoryReport = await invoke('cognition_memory_consolidate')
        await this.loadMemories()
        return this.memoryReport
      } finally {
        this.memoryBusy = false
      }
    },

    // ── 学习看板

    async loadLearning(days = 30) {
      this.learningLoading = true
      try {
        this.learningOverview = await invoke('cognition_learning_overview', { days })
        this.learningLoadedAt = Date.now()
      } catch (e) {
        logger.warn('cognition_learning_overview failed', e)
        this.learningOverview = null
      } finally {
        this.learningLoading = false
      }
    },

    /**
     * 切到学习页签 / 回到大脑页时调用：超过 minIntervalMs 就重算一次掌握度。
     * 直接调 loadLearning 会吃到 overview 的 10 分钟节流，用户会觉得「刚读完的书没算进去」。
     */
    async refreshLearningIfStale(days = 30, minIntervalMs = 30_000) {
      if (this.learningOverview && Date.now() - this.learningLoadedAt < minIntervalMs) return
      await this.refreshLearning(days)
    },

    async refreshLearning(days = 30) {
      this.learningRefreshing = true
      try {
        const n = await invoke('cognition_learning_refresh')
        await this.loadLearning(days)
        return n
      } finally {
        this.learningRefreshing = false
      }
    },

    // ── 引证排序器

    async loadRankerStatus() {
      try {
        this.rankerStatus = await invoke('cognition_ranker_status')
      } catch (e) {
        logger.warn('cognition_ranker_status failed', e)
        this.rankerStatus = null
      }
    },

    async trainRanker() {
      this.rankerBusy = true
      try {
        const report = await invoke('cognition_ranker_train')
        await this.loadRankerStatus()
        return report
      } finally {
        this.rankerBusy = false
      }
    },

    async resetRanker() {
      await invoke('cognition_ranker_reset')
      await this.loadRankerStatus()
    },

    /** 首屏一次性拉齐看板需要的东西。 */
    async bootstrap() {
      await this.refreshStatus()
      if (!this.enabled) return
      await Promise.all([
        this.refreshNudges(),
        this.refreshPercepts(),
        this.loadBrief(),
        this.loadLearning(),
      ])
    },
  },
})
