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
        this.percepts = await invoke('cognition_percept_list', {
          query: {
            state: f.state || null,
            sourceKind: f.sourceKind || null,
            minSalience: f.minSalience ?? null,
            limit: 60,
          },
        })
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
      await invoke('cognition_tool_set_tier', { tier, granted })
      await Promise.all([this.refreshTools(), this.refreshStatus()])
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

    /** 首屏一次性拉齐看板需要的东西。 */
    async bootstrap() {
      await this.refreshStatus()
      if (!this.enabled) return
      await Promise.all([
        this.refreshNudges(),
        this.refreshPercepts(),
        this.loadBrief(),
      ])
    },
  },
})
