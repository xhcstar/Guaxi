<template>
  <aside class="instrument-root">
    <!-- 雷荷波：右侧的主角。它表示「这台机器正在运转」，不是壁纸。 -->
    <div class="instrument-hero">
      <GuaxiRehoboamBackdrop :active="running" :telemetry="telemetry" />
      <div class="hero-meta">
        <span class="hero-state" :class="`hero-state--${stateKey}`">
          <span class="hero-dot" />
          {{ stateText }}
        </span>
        <span class="hero-caption">{{ caption }}</span>
      </div>
    </div>
    <div class="instrument-tabs" role="tablist">
      <button
        type="button"
        class="inst-tab"
        :class="{ 'inst-tab--on': tab === 'overview' }"
        role="tab"
        :aria-selected="tab === 'overview'"
        @click="tab = 'overview'"
      >
        概览
      </button>
      <button
        type="button"
        class="inst-tab"
        :class="{ 'inst-tab--on': tab === 'changes' }"
        role="tab"
        :aria-selected="tab === 'changes'"
        @click="tab = 'changes'"
      >
        改动
        <span v-if="files.length" class="inst-tab-count">{{ files.length }}</span>
      </button>
      <button
        v-if="files.length"
        type="button"
        class="inst-tab-open"
        title="在独立窗口打开完整 diff"
        @click="$emit('open-diff')"
      >
        <Icon :icon="externalIcon" width="12" height="12" />
      </button>
    </div>

    <div class="instrument-body">
      <div v-show="tab === 'overview'" class="inst-pane">
        <!-- 待批：整块界面里唯一会着色、并且置顶的东西。 -->
        <section v-if="approvals.length" class="inst-section inst-section--alert">
          <header class="inst-head">
            <span class="inst-title">需要你决定</span>
            <span class="inst-badge">{{ approvals.length }}</span>
          </header>
          <p class="inst-alert-line">模型停下来等一个明确答复。</p>
          <button type="button" class="inst-alert-btn" @click="$emit('review-approval')">
            查看并处理
          </button>
        </section>

        <section class="inst-section">
          <header class="inst-head">
            <span class="inst-title">计划</span>
            <span class="inst-meta">{{ planDone }}/{{ plan.length }}</span>
          </header>
          <div v-if="plan.length" class="inst-plan-bar">
            <span class="inst-plan-fill" :style="{ width: `${planPercent}%` }" />
          </div>
          <ol v-if="plan.length" class="inst-plan">
            <li
              v-for="(step, i) in plan"
              :key="step.id || i"
              class="inst-plan-row"
              :class="`inst-plan-row--${step.status}`"
            >
              <span class="inst-plan-mark" />
              <span class="inst-plan-text">{{ step.text }}</span>
            </li>
          </ol>
          <div v-else class="inst-empty">本次运行还没有计划</div>
        </section>

        <section class="inst-section">
          <header class="inst-head">
            <span class="inst-title">环境</span>
            <button type="button" class="inst-refresh" title="刷新连接器" @click="refreshEnv">
              <Icon :icon="refreshIcon" width="12" height="12" :class="{ spinning: refreshing }" />
            </button>
          </header>
          <dl class="inst-kv">
            <div class="inst-kv-row">
              <dt>目录</dt>
              <dd class="mono" :title="currentCwd">{{ cwdLabel }}</dd>
            </div>
            <div class="inst-kv-row">
              <dt>分支</dt>
              <dd class="mono">
                <template v-if="isRepo">
                  <Icon :icon="gitBranchIcon" width="11" height="11" class="kv-icon" />
                  {{ git.branch || 'detached' }}
                  <span v-if="git.files" class="git-dirty">{{ git.files }} 个未提交</span>
                </template>
                <template v-else>不在 git 仓库</template>
              </dd>
            </div>
            <div class="inst-kv-row">
              <dt>模型</dt>
              <dd class="mono">{{ modelName }}</dd>
            </div>
            <div class="inst-kv-row">
              <dt>权限</dt>
              <dd>{{ permissionLabel }}</dd>
            </div>
            <div class="inst-kv-row">
              <dt>引擎</dt>
              <dd class="mono">{{ engineText }}</dd>
            </div>
          </dl>

          <div v-if="connectors.length" class="inst-connectors">
            <div class="inst-sub-head">连接器</div>
            <div v-for="server in connectors.slice(0, 6)" :key="server.name || server.id" class="inst-connector" :title="connectorHint(server)">
              <span class="connector-dot" :class="{ 'connector-dot--bad': connectorBad(server) }" />
              <span class="connector-name">{{ server.name || server.id || server.serverName }}</span>
              <span v-if="toolCount(server)" class="connector-tools">{{ toolCount(server) }} 工具</span>
            </div>
          </div>
          <div v-else class="inst-empty inst-empty--tight">没有挂载 MCP 连接器</div>

          <div v-if="store.skills.length" class="inst-skills">
            <Icon :icon="sparklesIcon" width="11" height="11" />
            {{ store.skills.length }} 个技能可用
          </div>
        </section>

        <section class="inst-section">
          <header class="inst-head">
            <span class="inst-title">上下文</span>
            <span class="inst-meta">{{ context.percent }}%</span>
          </header>
          <div class="inst-ctx">
            <span class="inst-ctx-num">{{ formatTokens(context.used) }}</span>
            <span class="inst-ctx-total">/ {{ context.window ? formatTokens(context.window) : '—' }}</span>
          </div>
          <div class="inst-bar">
            <span class="inst-bar-fill" :style="{ width: `${context.percent}%` }" />
          </div>
          <dl class="inst-kv inst-kv--stats">
            <div class="inst-kv-row">
              <dt>命令</dt>
              <dd>{{ stats.commands }}</dd>
            </div>
            <div class="inst-kv-row">
              <dt>工具</dt>
              <dd>{{ stats.tools + stats.searches }}</dd>
            </div>
            <div class="inst-kv-row">
              <dt>文件</dt>
              <dd>{{ stats.files }}</dd>
            </div>
            <div v-if="stats.failed" class="inst-kv-row">
              <dt>失败</dt>
              <dd class="num-del">{{ stats.failed }}</dd>
            </div>
          </dl>
        </section>
      </div>

      <div v-show="tab === 'changes'" class="inst-pane">
        <section class="inst-section inst-section--first">
          <header class="inst-head">
            <span class="inst-title">改动</span>
            <span class="inst-meta">
              <span v-if="diffStats.added" class="num-add">+{{ diffStats.added }}</span>
              <span v-if="diffStats.removed" class="num-del">−{{ diffStats.removed }}</span>
            </span>
          </header>

          <ul v-if="files.length" class="inst-files">
            <li v-for="file in files" :key="file.path">
              <button
                type="button"
                class="inst-file"
                :class="{ 'inst-file--on': selectedFile?.path === file.path }"
                :title="file.path"
                @click="selectFile(file)"
              >
                <span class="inst-file-kind" :class="`inst-file-kind--${file.kind || 'update'}`">
                  {{ fileKind(file.kind) }}
                </span>
                <span class="inst-file-name">{{ baseName(file.path) }}</span>
                <span class="inst-file-nums">
                  <span v-if="file.added" class="num-add">+{{ file.added }}</span>
                  <span v-if="file.removed" class="num-del">−{{ file.removed }}</span>
                </span>
              </button>
            </li>
          </ul>
          <div v-else class="inst-empty">还没有文件改动</div>

          <form v-if="isRepo && git.dirty" class="inst-commit" @submit.prevent="commit">
            <Icon :icon="gitCommitIcon" width="12" height="12" class="commit-icon" />
            <input
              v-model="commitMessage"
              class="commit-input"
              type="text"
              placeholder="提交信息"
              spellcheck="false"
            />
            <button type="submit" class="commit-btn" :disabled="committing || !commitMessage.trim()">
              {{ committing ? '提交中…' : '提交' }}
            </button>
          </form>
        </section>

        <section v-if="selectedFile" class="inst-section inst-section--diff">
          <header class="inst-head">
            <span class="inst-title diff-head-title" :title="selectedFile.path">
              {{ baseName(selectedFile.path) }}
            </span>
            <span class="inst-meta">
              <span v-if="selectedFile.added" class="num-add">+{{ selectedFile.added }}</span>
              <span v-if="selectedFile.removed" class="num-del">−{{ selectedFile.removed }}</span>
            </span>
          </header>
          <pre v-if="visibleLines.length" class="diff-pre"><code><span
            v-for="(line, i) in visibleLines"
            :key="i"
            class="diff-line"
            :class="lineClass(line)"
          >{{ line || ' ' }}</span></code></pre>
          <div v-else class="inst-empty">
            这个文件只有计数，没有可展开的补丁。用右上角按钮看完整 diff。
          </div>
          <div v-if="hiddenLines" class="diff-more">
            还有 {{ hiddenLines }} 行未展开，请在独立窗口查看。
          </div>
        </section>
        <section v-else-if="files.length" class="inst-section">
          <div class="inst-empty">从上面选一个文件查看补丁。</div>
        </section>
      </div>
    </div>
  </aside>
</template>

<script setup>
/**
 * 右侧仪表。
 *
 * 三条约束：
 * - 一个主角：雷荷波。其余全部安静——细线、无卡片堆叠、无阴影。
 * - 结构即信息：每一段都对应真实数据；没有数据就不画。
 * - 只有「需要你」用色（琥珀）。成功就是安静地变成已完成。
 */
import { computed, onMounted, ref, watch } from 'vue'
import { useMessage } from 'naive-ui'
import { Icon } from '@iconify/vue'
import gitBranchIcon from '@iconify-icons/lucide/git-branch'
import gitCommitIcon from '@iconify-icons/lucide/git-commit'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import sparklesIcon from '@iconify-icons/lucide/sparkles'
import externalIcon from '@iconify-icons/lucide/external-link'
import GuaxiRehoboamBackdrop from '../ChatViewComponent/GuaxiRehoboamBackdrop.vue'
import { useAgentWorkspaceStore } from '../../stores/agentWorkspaceStore'
import { createLogger } from '../../utils/logger'

defineEmits(['review-approval', 'open-diff'])

const logger = createLogger('AgentInstrument')
const message = useMessage()
const store = useAgentWorkspaceStore()

const committing = ref(false)
const refreshing = ref(false)
const commitMessage = ref('')
const tab = ref('overview')
const selectedPath = ref('')

/** 单个文件的补丁最多画这么多行；再长就交给独立 diff 窗口。 */
const MAX_PATCH_LINES = 800

const state = computed(() => store.currentState || {})
const running = computed(() => store.currentRunning)
const approvals = computed(() => store.approvalsForCurrent)
const plan = computed(() => state.value.plan || [])
const files = computed(() => store.currentDiffFiles)
const stats = computed(() => store.runStats)
const context = computed(() => store.contextUsage)
const connectors = computed(() => store.mcpServers || [])
const git = computed(() => state.value.git || {})
const isRepo = computed(() => Boolean(git.value.isRepo))

const currentCwd = computed(() => state.value.cwd || store.settings.cwd || '')
const cwdLabel = computed(() => {
  const parts = String(currentCwd.value || '').split(/[\\/]/).filter(Boolean)
  if (!parts.length) return '未选择'
  return parts.length > 2 ? `…/${parts.slice(-2).join('/')}` : parts.join('/')
})

const permissionLabel = computed(() => store.permissionPreset.label)
const modelName = computed(() => {
  const m = store.currentModel
  return m?.displayName || m?.model || store.settings.model || '默认'
})

const engineText = computed(() => {
  const e = store.engine
  if (!e) return '未连接'
  if (!e.binary_exists) return '缺失'
  return e.running ? `app-server ${e.binary_version || ''}`.trim() : '待启动'
})

const planDone = computed(() => plan.value.filter((s) => s.status === 'completed' || s.status === 'done').length)
const planPercent = computed(() => (plan.value.length ? Math.round((planDone.value / plan.value.length) * 100) : 0))

const diffStats = computed(() => ({
  added: files.value.reduce((n, f) => n + (Number(f.added) || 0), 0),
  removed: files.value.reduce((n, f) => n + (Number(f.removed) || 0), 0),
}))

const selectedFile = computed(() =>
  files.value.find((f) => f.path === selectedPath.value) || files.value[0] || null,
)

const selectedLines = computed(() => {
  const patch = String(selectedFile.value?.patch || '')
  return patch ? patch.split('\n') : []
})

const visibleLines = computed(() => selectedLines.value.slice(0, MAX_PATCH_LINES))
const hiddenLines = computed(() => Math.max(0, selectedLines.value.length - MAX_PATCH_LINES))

const selectFile = (file) => {
  selectedPath.value = String(file?.path || '')
}

const fileKind = (kind) => ({
  add: 'A',
  delete: 'D',
  update: 'M',
  move: 'R',
}[String(kind || 'update')] || 'M')

const lineClass = (line) => {
  if (line.startsWith('+++') || line.startsWith('---')) return 'diff-line--meta'
  if (line.startsWith('@@')) return 'diff-line--hunk'
  if (line.startsWith('+')) return 'diff-line--add'
  if (line.startsWith('-')) return 'diff-line--del'
  return ''
}

// 改动清单换了（新文件、线程切换）时把选中项落在仍然存在的文件上。
watch(
  files,
  (list) => {
    if (!list.length) {
      selectedPath.value = ''
      return
    }
    if (!list.some((f) => f.path === selectedPath.value)) {
      selectedPath.value = list[0].path
    }
  },
  { immediate: true },
)

const stateKey = computed(() => {
  if (approvals.value.length) return 'waiting'
  if (running.value) return 'running'
  const status = String(state.value.status || 'idle')
  if (status === 'failed' || status === 'systemError') return 'failed'
  return 'idle'
})

const stateText = computed(() => ({
  waiting: '等待确认',
  running: '运行中',
  failed: '上次失败',
  idle: '空闲',
}[stateKey.value]))

const baseName = (p) => String(p || '').split(/[\\/]/).filter(Boolean).pop() || ''
const cwdName = computed(() => baseName(currentCwd.value) || '未选择目录')

const caption = computed(() => {
  const mode = store.settings.mode === 'code' ? '编码' : store.settings.mode
  return `${mode} · ${cwdName.value}`
})

const itemCount = computed(() =>
  (state.value.turns || []).reduce((n, t) => n + (t.items?.length || 0), 0),
)

/** 给雷荷波的数据。全部来自真实事件，缺项就不画。 */
const telemetry = computed(() => {
  const s = stats.value
  const usage = state.value.usage
  const total = Number(usage?.total?.totalTokens || 0)
  const out = Number(usage?.total?.outputTokens || 0)
  return {
    tokenRate: running.value && total ? Math.min(1, out / Math.max(1, total)) : 0,
    toolCount: (s.commands || 0) + (s.tools || 0) + (s.searches || 0) + (s.files || 0),
    pending: approvals.value.length,
    itemIndex: itemCount.value,
    caption: `${store.settings.mode}\n${cwdName.value}`,
  }
})

const formatTokens = (n) => {
  const value = Number(n || 0)
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (value >= 1000) return `${(value / 1000).toFixed(1)}k`
  return String(value)
}

const toolCount = (server) => {
  if (Array.isArray(server?.tools)) return server.tools.length
  return Number(server?.toolCount || 0)
}

const connectorBad = (server) => {
  const status = String(server?.status || server?.authStatus || '')
  return ['failed', 'error', 'unauthorized', 'notLoggedIn', 'not_logged_in'].includes(status)
}

const connectorHint = (server) => {
  const parts = []
  if (server?.status) parts.push(String(server.status))
  if (server?.authStatus) parts.push(String(server.authStatus))
  return parts.join(' · ')
}

const refreshEnv = async () => {
  refreshing.value = true
  try {
    await Promise.all([
      store.refreshConnectors(store.currentThreadId),
      store.loadSkills(),
      store.refreshGit(state.value),
    ])
  } catch (err) {
    logger.warn('[instrument] refresh failed', { error: String(err?.message || err) })
  } finally {
    refreshing.value = false
  }
}

const commit = async () => {
  const messageText = commitMessage.value.trim()
  if (!messageText) return
  committing.value = true
  try {
    const res = await store.commitChanges(messageText)
    commitMessage.value = ''
    message.success(`已提交 ${res?.commit || ''}`.trim())
  } catch (err) {
    logger.warn('[instrument] commit failed', { error: String(err?.message || err) })
    message.error(String(err?.message || err))
  } finally {
    committing.value = false
  }
}

watch(
  () => store.currentThreadId,
  () => {
    commitMessage.value = ''
    tab.value = 'overview'
    selectedPath.value = ''
  },
)

onMounted(() => {
  store.loadSkills().catch(() => {})
})
</script>

<style scoped>
.instrument-root {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  overflow: hidden;
  border-left: 1px solid var(--guaxi-border);
  background: var(--guaxi-content-bg);
}

.instrument-hero {
  position: relative;
  flex: 0 0 auto;
  height: 214px;
  overflow: hidden;
  background:
    radial-gradient(circle at 56% 42%, rgba(180, 125, 26, 0.07), transparent 64%),
    radial-gradient(circle at 20% 90%, rgba(87, 96, 106, 0.06), transparent 60%);
}

.hero-meta {
  position: absolute;
  left: 14px;
  right: 14px;
  bottom: 10px;
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.hero-state {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 10px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--guaxi-text-secondary);
}

.hero-dot {
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: currentColor;
  opacity: 0.6;
}

.hero-state--running { color: #b47d1a; }
.hero-state--running .hero-dot { animation: hero-pulse 1.6s ease-in-out infinite; }
.hero-state--waiting { color: #b47d1a; }
.hero-state--failed { color: #b42318; }

@keyframes hero-pulse {
  0%, 100% { opacity: 0.35; }
  50% { opacity: 1; }
}

.hero-caption {
  margin-left: auto;
  max-width: 62%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--guaxi-editor-font-family);
  font-size: 9.5px;
  color: var(--guaxi-text-secondary);
  opacity: 0.55;
}

.instrument-tabs {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 2px;
  padding: 0 10px;
  border-top: 1px solid var(--guaxi-border);
  border-bottom: 1px solid var(--guaxi-border);
}

.inst-tab {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 32px;
  padding: 0 8px;
  border: none;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-family: inherit;
  font-size: 11.5px;
  cursor: pointer;
  transition: color 120ms ease;
}

.inst-tab:hover {
  color: var(--guaxi-text-primary);
}

.inst-tab--on {
  color: var(--guaxi-text-primary);
}

.inst-tab--on::after {
  content: '';
  position: absolute;
  right: 8px;
  bottom: -1px;
  left: 8px;
  height: 1.5px;
  border-radius: 1px;
  background: #b47d1a;
}

.inst-tab-count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 14px;
  height: 14px;
  padding: 0 4px;
  border-radius: 7px;
  background: var(--guaxi-hover-bg);
  font-family: var(--guaxi-editor-font-family);
  font-size: 9px;
}

.inst-tab-open {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  margin-left: auto;
  padding: 0;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  opacity: 0.6;
  cursor: pointer;
}

.inst-tab-open:hover {
  background: var(--guaxi-hover-bg);
  opacity: 1;
}

.instrument-body {
  flex: 1 1 auto;
  overflow-y: auto;
  padding: 0 14px 22px;
  scrollbar-width: thin;
}

.inst-pane {
  min-height: 0;
}

.inst-section--first {
  border-top: none;
  padding-top: 10px;
}

.inst-section {
  padding: 12px 0 11px;
  border-top: 1px solid var(--guaxi-border);
}

.inst-section--alert {
  border-top: none;
  padding-top: 10px;
}

.inst-head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin-bottom: 7px;
}

.inst-title {
  font-size: 12px;
  font-weight: 500;
  color: var(--guaxi-text-primary);
}

.inst-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 15px;
  height: 15px;
  padding: 0 4px;
  border-radius: 7px;
  background: rgba(180, 125, 26, 0.14);
  color: #b47d1a;
  font-family: var(--guaxi-editor-font-family);
  font-size: 9.5px;
}

.inst-meta {
  margin-left: auto;
  display: flex;
  align-items: baseline;
  gap: 5px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 10px;
  color: var(--guaxi-text-secondary);
  opacity: 0.8;
}

.inst-alert-line {
  margin: 0 0 8px;
  font-size: 11.5px;
  line-height: 1.55;
  color: var(--guaxi-text-secondary);
}

.inst-alert-btn {
  width: 100%;
  height: 27px;
  border-radius: 6px;
  border: 1px solid rgba(180, 125, 26, 0.55);
  background: rgba(180, 125, 26, 0.08);
  color: #b47d1a;
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
  transition: background 120ms ease;
}

.inst-alert-btn:hover {
  background: rgba(180, 125, 26, 0.16);
}

.inst-plan-bar {
  height: 2px;
  margin-bottom: 8px;
  border-radius: 1px;
  background: var(--guaxi-border);
  overflow: hidden;
}

.inst-plan-fill {
  display: block;
  height: 100%;
  background: #b47d1a;
  opacity: 0.8;
  transition: width 240ms ease;
}

.inst-plan {
  list-style: none;
  margin: 0;
  padding: 0;
}

.inst-plan-row {
  display: flex;
  align-items: flex-start;
  gap: 7px;
  padding: 2px 0;
  font-size: 11.5px;
  line-height: 1.55;
  color: var(--guaxi-text-secondary);
}

.inst-plan-mark {
  flex: 0 0 auto;
  width: 5px;
  height: 5px;
  margin-top: 6px;
  border-radius: 50%;
  border: 1px solid var(--guaxi-text-secondary);
  opacity: 0.5;
}

.inst-plan-row--inProgress {
  color: var(--guaxi-text-primary);
}

.inst-plan-row--inProgress .inst-plan-mark {
  background: #b47d1a;
  border-color: #b47d1a;
  opacity: 1;
  box-shadow: 0 0 0 3px rgba(180, 125, 26, 0.12);
}

.inst-plan-row--completed {
  color: var(--guaxi-text-primary);
  opacity: 0.72;
}

.inst-plan-row--completed .inst-plan-mark {
  background: var(--guaxi-text-secondary);
  border-color: var(--guaxi-text-secondary);
  opacity: 0.8;
}

.inst-empty {
  font-size: 11.5px;
  line-height: 1.6;
  color: var(--guaxi-text-secondary);
  opacity: 0.58;
}

.inst-empty--tight {
  margin-top: 6px;
  font-size: 11px;
}

.inst-files {
  list-style: none;
  margin: 0;
  padding: 0;
}

.inst-file {
  display: flex;
  align-items: baseline;
  gap: 8px;
  width: 100%;
  padding: 3px 2px;
  border: none;
  border-radius: 4px;
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.inst-file:hover {
  background: var(--guaxi-hover-bg);
}

.inst-file-name {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--guaxi-editor-font-family);
  font-size: 10.5px;
  color: var(--guaxi-text-secondary);
}

.inst-file-nums {
  flex: 0 0 auto;
  display: flex;
  gap: 5px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 9.5px;
}

.num-add { color: #4f8a5b; }
.num-del { color: #b42318; }

.inst-file--on {
  background: var(--guaxi-hover-bg);
}

.inst-file--on .inst-file-name {
  color: var(--guaxi-text-primary);
}

.inst-file-kind {
  flex: 0 0 auto;
  width: 10px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 9px;
  color: var(--guaxi-text-secondary);
  opacity: 0.55;
}

.inst-file-kind--add { color: #4f8a5b; opacity: 1; }
.inst-file-kind--delete { color: #b42318; opacity: 1; }
.inst-file-kind--move { color: #b47d1a; opacity: 1; }

.inst-section--diff {
  padding-bottom: 6px;
}

.diff-head-title {
  max-width: 72%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--guaxi-editor-font-family);
  font-size: 10.5px;
}

.diff-pre {
  margin: 2px -4px 6px;
  padding: 5px 4px;
  max-height: 60vh;
  overflow: auto;
  border: 1px solid var(--guaxi-border);
  border-radius: 5px;
  background: var(--guaxi-panel-bg);
  font-family: var(--guaxi-editor-font-family);
  font-size: 9.5px;
  line-height: 1.62;
  scrollbar-width: thin;
}

.diff-pre code {
  display: block;
  width: max-content;
  min-width: 100%;
}

.diff-line {
  display: block;
  padding: 0 4px;
  white-space: pre;
  color: var(--guaxi-text-secondary);
}

.diff-line--add {
  color: #4f8a5b;
  background: rgba(79, 138, 91, 0.08);
}

.diff-line--del {
  color: #b42318;
  background: rgba(180, 35, 24, 0.07);
}

.diff-line--hunk {
  color: #b47d1a;
  opacity: 0.9;
}

.diff-line--meta {
  opacity: 0.5;
}

.diff-more {
  padding-top: 4px;
  font-size: 10px;
  color: var(--guaxi-text-secondary);
  opacity: 0.6;
}

.inst-link {
  margin-top: 5px;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 11px;
  text-decoration: underline;
  text-underline-offset: 3px;
  cursor: pointer;
}

.inst-link:hover {
  color: var(--guaxi-text-primary);
}

.inst-commit {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 9px;
  padding: 3px 4px 3px 8px;
  border: 1px solid var(--guaxi-border);
  border-radius: 6px;
  background: var(--guaxi-panel-bg);
}

.inst-commit:focus-within {
  border-color: rgba(180, 125, 26, 0.5);
}

.commit-icon {
  flex: 0 0 auto;
  opacity: 0.55;
}

.commit-input {
  flex: 1 1 auto;
  min-width: 0;
  height: 24px;
  border: none;
  outline: none;
  background: transparent;
  color: var(--guaxi-text-primary);
  font-family: inherit;
  font-size: 11.5px;
}

.commit-input::placeholder {
  color: var(--guaxi-text-secondary);
  opacity: 0.6;
}

.commit-btn {
  flex: 0 0 auto;
  height: 22px;
  padding: 0 8px;
  border-radius: 4px;
  border: 1px solid transparent;
  background: rgba(180, 125, 26, 0.12);
  color: #b47d1a;
  font-size: 10.5px;
  font-family: inherit;
  cursor: pointer;
}

.commit-btn:disabled {
  opacity: 0.4;
  cursor: default;
}

.inst-refresh {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  padding: 0;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  opacity: 0.6;
  cursor: pointer;
}

.inst-refresh:hover {
  background: var(--guaxi-hover-bg);
  opacity: 1;
}

.spinning {
  animation: spin 900ms linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.inst-kv {
  margin: 0;
}

.inst-kv-row {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 1.5px 0;
  font-size: 11.5px;
  line-height: 1.5;
}

.inst-kv-row dt {
  flex: 0 0 34px;
  color: var(--guaxi-text-secondary);
  opacity: 0.68;
}

.inst-kv-row dd {
  margin: 0;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--guaxi-text-primary);
}

.inst-kv-row dd.mono {
  font-family: var(--guaxi-editor-font-family);
  font-size: 10.5px;
}

.kv-icon {
  vertical-align: -1px;
  margin-right: 2px;
  opacity: 0.6;
}

.git-dirty {
  margin-left: 5px;
  color: #b47d1a;
  font-size: 9.5px;
}

.inst-connectors {
  margin-top: 9px;
}

.inst-sub-head {
  margin-bottom: 4px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 9.5px;
  letter-spacing: 0.07em;
  text-transform: uppercase;
  color: var(--guaxi-text-secondary);
  opacity: 0.55;
}

.inst-connector {
  display: flex;
  align-items: baseline;
  gap: 6px;
  padding: 1.5px 0;
  font-size: 11px;
}

.connector-dot {
  flex: 0 0 auto;
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: #4f8a5b;
  opacity: 0.85;
}

.connector-dot--bad {
  background: #b42318;
}

.connector-name {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--guaxi-editor-font-family);
  font-size: 10.5px;
  color: var(--guaxi-text-secondary);
}

.connector-tools {
  flex: 0 0 auto;
  font-family: var(--guaxi-editor-font-family);
  font-size: 9px;
  color: var(--guaxi-text-secondary);
  opacity: 0.5;
}

.inst-skills {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 8px;
  font-size: 10.5px;
  color: var(--guaxi-text-secondary);
  opacity: 0.62;
}

.inst-ctx {
  display: flex;
  align-items: baseline;
  gap: 5px;
  margin-bottom: 5px;
}

.inst-ctx-num {
  font-family: var(--guaxi-editor-font-family);
  font-size: 17px;
  font-weight: 500;
  letter-spacing: -0.02em;
  color: var(--guaxi-text-primary);
}

.inst-ctx-total {
  font-family: var(--guaxi-editor-font-family);
  font-size: 10px;
  color: var(--guaxi-text-secondary);
  opacity: 0.55;
}

.inst-bar {
  height: 2px;
  border-radius: 1px;
  background: var(--guaxi-border);
  overflow: hidden;
}

.inst-bar-fill {
  display: block;
  height: 100%;
  background: #b47d1a;
  opacity: 0.7;
  transition: width 300ms ease;
}

.inst-kv--stats {
  margin-top: 7px;
  display: flex;
  flex-wrap: wrap;
  gap: 0 12px;
}

.inst-kv--stats .inst-kv-row {
  gap: 4px;
}

.inst-kv--stats .inst-kv-row dt {
  flex: 0 0 auto;
}

@media (prefers-reduced-motion: reduce) {
  .hero-state--running .hero-dot,
  .spinning {
    animation: none;
  }
}
</style>
