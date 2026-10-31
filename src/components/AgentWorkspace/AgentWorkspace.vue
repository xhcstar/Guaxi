<template>
  <div class="workspace-root">
    <AgentSessionRail v-if="!railCollapsed" class="workspace-rail" />

    <main class="workspace-main">
      <header class="workspace-bar">
        <div class="bar-left">
          <button
            type="button"
            class="bar-icon"
            :title="railCollapsed ? '显示会话栏' : '隐藏会话栏'"
            @click="toggleRail"
          >
            <Icon :icon="panelLeftIcon" width="15" height="15" />
          </button>
          <h1 class="bar-title" :title="title">{{ title }}</h1>
          <span v-if="statusText" class="bar-state" :class="`bar-state--${stateKind}`">
            <span class="state-dot" />
            {{ statusText }}
          </span>
        </div>

        <div class="bar-right">
          <button
            type="button"
            class="cwd-chip"
            :title="cwd || '选择工作目录'"
            @click="pickWorkspace"
          >
            <Icon :icon="folderIcon" width="12" height="12" />
            <span>{{ cwdLabel }}</span>
          </button>

          <button
            v-if="diffStats.count"
            type="button"
            class="diff-chip"
            title="查看本次改动"
            @click="openDiff"
          >
            <Icon :icon="fileDiffIcon" width="13" height="13" />
            <span>{{ diffStats.count }}</span>
            <span v-if="diffStats.added" class="num-add">+{{ diffStats.added }}</span>
            <span v-if="diffStats.removed" class="num-del">−{{ diffStats.removed }}</span>
          </button>

          <n-dropdown
            :options="threadActions"
            trigger="click"
            placement="bottom-end"
            @select="onThreadAction"
          >
            <button type="button" class="bar-icon" title="会话操作">
              <Icon :icon="moreIcon" width="15" height="15" />
            </button>
          </n-dropdown>

          <button
            type="button"
            class="bar-icon"
            :title="instrumentCollapsed ? '显示仪表' : '隐藏仪表'"
            @click="toggleInstrument"
          >
            <Icon :icon="panelRightIcon" width="15" height="15" />
          </button>
        </div>
      </header>

      <div v-if="engineBlocked" class="workspace-alert">
        <span class="alert-text">{{ engineBlocked }}</span>
        <button type="button" class="alert-btn" :disabled="retrying" @click="onRetryEngine">
          {{ retrying ? '重连中…' : '重试连接' }}
        </button>
      </div>

      <div ref="scrollRef" class="workspace-transcript" @scroll="onScroll">
        <div class="transcript-inner">
          <AgentTimeline
            :turns="turns"
            :running="running"
            @suggest="onSuggest"
            @open-diff="onOpenFileDiff"
            @open-image="previewImage"
            @fork="onForkAt"
          />
        </div>
      </div>

      <button
        v-if="showJump"
        type="button"
        class="workspace-jump"
        title="跳到最新"
        @click="jumpToBottom"
      >
        <Icon :icon="arrowDownIcon" width="14" height="14" />
      </button>

      <div class="composer-zone">
        <AgentComposer
          :pending="running"
          @send="onSend"
          @steer="onSteer"
          @stop="onStop"
          @new="onNew"
          @open-diff="openDiff"
        />
      </div>
    </main>

    <AgentInstrument
      v-if="!instrumentCollapsed"
      class="workspace-instrument"
      @review-approval="showApproval = true"
      @open-diff="openDiff"
    />

    <AgentApprovalSheet
      v-if="showApproval && approvals.length"
      :queue="approvals"
      :index="approvalIndex"
      @decide="onDecide"
    />

    <div v-if="preview" class="image-layer" title="点击关闭" @click="preview = ''">
      <img :src="preview" alt="预览" />
    </div>
  </div>
</template>

<script setup>
/**
 * Agent 工作台外壳。
 *
 * 三段式：会话栏 | 转录 | 仪表。中间栏自己拥有顶栏——会话标题、工作目录、
 * 改动摘要和面板开关都在这里，和 Codex 的「对话头部」而不是应用头部对齐。
 *
 * 这一层只做编排：所有状态都在 agentWorkspaceStore，所有交互都落到 codex
 * 的 thread/turn 上。外壳不缓存消息，也不替 store 判断运行态。
 */
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useMessage } from 'naive-ui'
import { Icon } from '@iconify/vue'
import panelLeftIcon from '@iconify-icons/lucide/panel-left'
import panelRightIcon from '@iconify-icons/lucide/panel-right'
import moreIcon from '@iconify-icons/lucide/more-horizontal'
import folderIcon from '@iconify-icons/lucide/folder'
import fileDiffIcon from '@iconify-icons/lucide/file-diff'
import arrowDownIcon from '@iconify-icons/lucide/arrow-down'
import AgentSessionRail from './AgentSessionRail.vue'
import AgentTimeline from './AgentTimeline.vue'
import AgentComposer from './AgentComposer.vue'
import AgentInstrument from './AgentInstrument.vue'
import AgentApprovalSheet from './AgentApprovalSheet.vue'
import { useAgentWorkspaceStore } from '../../stores/agentWorkspaceStore'
import { useDiffCompareStore } from '../../stores/diffCompareStore'
import { useSiderStore } from '../../stores/useSiderStore'
import { createLogger } from '../../utils/logger'

const logger = createLogger('AgentWorkspace')
const message = useMessage()
const store = useAgentWorkspaceStore()
const diffStore = useDiffCompareStore()
const siderStore = useSiderStore()

const railCollapsed = computed(() => siderStore.isLeftSiderCollapsed.chat === true)
const instrumentCollapsed = computed(() => siderStore.isRightSiderCollapsed.chat === true)

const toggleRail = () => siderStore.setLeftSiderCollapsed('chat', !railCollapsed.value)
const toggleInstrument = () => siderStore.setRightSiderCollapsed('chat', !instrumentCollapsed.value)

const scrollRef = ref(null)
const preview = ref('')
const showApproval = ref(false)
const showJump = ref(false)
const approvalIndex = ref(0)
const retrying = ref(false)

const turns = computed(() => store.currentState?.turns || [])
const running = computed(() => store.currentRunning)
const approvals = computed(() => store.approvalsForCurrent)

const title = computed(() => {
  if (store.currentIsDraft) return '新任务'
  const state = store.currentState
  return String(state?.name || state?.preview || '').trim() || '未命名会话'
})

const stateKind = computed(() => {
  if (approvals.value.length) return 'waiting'
  if (running.value) return 'running'
  const status = String(store.currentState?.status || 'idle')
  if (status === 'failed' || status === 'systemError') return 'failed'
  return 'idle'
})

const statusText = computed(() => ({
  waiting: '等待确认',
  running: '运行中',
  failed: '上次失败',
  idle: '',
}[stateKind.value]))

const cwd = computed(() => store.currentState?.cwd || store.settings.cwd || '')
const cwdLabel = computed(() => {
  const parts = String(cwd.value || '').split(/[\\/]/).filter(Boolean)
  if (!parts.length) return '选择目录'
  return parts.length > 2 ? `…/${parts.slice(-2).join('/')}` : parts.join('/')
})

const diffStats = computed(() => {
  const files = store.currentDiffFiles
  return {
    count: files.length,
    added: files.reduce((n, f) => n + (Number(f.added) || 0), 0),
    removed: files.reduce((n, f) => n + (Number(f.removed) || 0), 0),
  }
})

const engineBlocked = computed(() => {
  if (store.engineError) return store.engineError
  const engine = store.engine
  if (engine && !engine.binary_exists) return '没有找到 codex 可执行文件，Agent 无法运行'
  return ''
})

const threadActions = computed(() => [
  { label: '分叉出新会话', key: 'fork', disabled: store.currentIsDraft },
  { label: '压缩上下文', key: 'compact', disabled: store.currentIsDraft },
  { label: '审查未提交改动', key: 'review', disabled: store.currentIsDraft },
  { type: 'divider', key: 'd1' },
  { label: '归档会话', key: 'archive', disabled: store.currentIsDraft },
])

// ───────────── 滚动跟随

const BOTTOM_THRESHOLD = 60
const stickToBottom = ref(true)

const isNearBottom = () => {
  const el = scrollRef.value
  if (!el) return true
  return el.scrollHeight - el.scrollTop - el.clientHeight < BOTTOM_THRESHOLD
}

const scrollToBottom = (smooth = false) => {
  const el = scrollRef.value
  if (!el) return
  el.scrollTo({ top: el.scrollHeight, behavior: smooth ? 'smooth' : 'auto' })
}

const jumpToBottom = () => {
  stickToBottom.value = true
  showJump.value = false
  scrollToBottom(true)
}

const onScroll = () => {
  stickToBottom.value = isNearBottom()
  showJump.value = !stickToBottom.value
}

/**
 * 流式内容的高度是异步长出来的：只盯「新增了几个 item」会漏掉正在增补的
 * 同一段文本。这里把最后一项的文本、输出、思考摘要长度也纳入信号。
 */
const streamSignal = computed(() => {
  const list = turns.value
  const lastTurn = list[list.length - 1]
  const lastItem = lastTurn?.items?.[lastTurn.items.length - 1]
  const lengthOf = (value) => String(value || '').length
  return [
    list.length,
    lastTurn?.status || '',
    lastItem?.id || '',
    lastItem?.status || '',
    lengthOf(lastItem?.text),
    lengthOf(lastItem?.output),
    lengthOf(lastItem?.progress),
    lengthOf(lastItem?.summary?.[0]),
  ].join(':')
})

watch(streamSignal, () => {
  if (stickToBottom.value) nextTick(() => scrollToBottom(false))
})

// 切换会话时直接落到最新，不沿用上一段会话的浏览位置。
watch(
  () => store.currentThreadId,
  () => {
    stickToBottom.value = true
    showJump.value = false
    approvalIndex.value = 0
    showApproval.value = approvals.value.length > 0
    nextTick(() => scrollToBottom(false))
  },
)

// ───────────── 审批

watch(
  () => approvals.value.length,
  (count, prev) => {
    if (count && !prev) {
      approvalIndex.value = 0
      showApproval.value = true
    }
    if (!count) {
      showApproval.value = false
      approvalIndex.value = 0
    } else if (approvalIndex.value >= count) {
      approvalIndex.value = Math.max(0, count - 1)
    }
  },
)

const onDecide = async ({ requestId, action, options }) => {
  await store.resolveApproval(requestId, action, options)
  const left = approvals.value.length
  if (approvalIndex.value >= left) approvalIndex.value = Math.max(0, left - 1)
  if (!left) showApproval.value = false
}

// ───────────── 发送

const onSend = async (text, attachments = []) => {
  try {
    await store.sendTurn(text, { attachments })
    nextTick(() => scrollToBottom(true))
  } catch (err) {
    message.error(String(err?.message || err))
  }
}

const onSteer = async (text, attachments = []) => {
  await store.steer(text, { attachments })
  nextTick(() => scrollToBottom(true))
}

const onStop = async () => {
  await store.interrupt()
}

const onSuggest = (prompt) => {
  if (prompt) onSend(prompt)
}

const onNew = () => {
  store.newDraft()
}

const onForkAt = async (turnId) => {
  if (!store.currentThreadId) return
  try {
    await store.forkThread(store.currentThreadId, turnId)
    message.success('已分叉出新会话')
  } catch (err) {
    message.error(String(err?.message || err))
  }
}

// ───────────── 顶栏动作

const onThreadAction = async (key) => {
  if (key === 'archive') {
    if (!store.currentThreadId) return
    try {
      await store.archiveThread(store.currentThreadId)
    } catch (err) {
      message.error(String(err?.message || err))
    }
    return
  }
  if (!store.currentThreadId) {
    message.info('先发一条消息建立会话')
    return
  }
  try {
    if (key === 'fork') {
      await store.forkThread(store.currentThreadId)
      message.success('已分叉出新会话')
    } else if (key === 'compact') {
      await store.compactThread()
      message.success('已开始压缩上下文')
    } else if (key === 'review') {
      await store.startReview()
      message.success('已开始审查未提交改动')
    }
  } catch (err) {
    message.error(String(err?.message || err))
  }
}

const pickWorkspace = async () => {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const picked = await open({ directory: true, multiple: false, title: '选择工作目录' })
    if (typeof picked === 'string' && picked) {
      store.setCwd(picked)
      if (store.currentThreadId) message.info('工作目录从下一轮开始生效')
    }
  } catch (err) {
    logger.warn('[workspace] pick workspace failed', { error: String(err?.message || err) })
  }
}

// ───────────── diff / 图片

const currentDiffText = () => {
  const state = store.currentState
  if (state?.diff) return String(state.diff)
  return store.currentDiffFiles
    .map((file) => file.patch)
    .filter(Boolean)
    .join('\n\n')
}

const openDiff = () => {
  const text = currentDiffText()
  if (!text) {
    message.info('本次运行还没有产生 diff')
    return
  }
  diffStore.compareTexts('', text, '原始', '本次运行')
}

const onOpenFileDiff = (file) => {
  const patch = String(file?.patch || '')
  if (patch) {
    diffStore.compareTexts('', patch, '原始', String(file?.path || '改动'))
    return
  }
  openDiff()
}

const previewImage = (src) => {
  preview.value = String(src || '')
}

// ───────────── 引擎 / 生命周期

const onRetryEngine = async () => {
  retrying.value = true
  try {
    await store.retryEngine()
    message.success('引擎已连接')
  } catch (err) {
    message.error(String(err?.message || err))
  } finally {
    retrying.value = false
  }
}

let observer = null

const onKeydown = (event) => {
  if (event.key === 'Escape' && preview.value) preview.value = ''
}

onMounted(async () => {
  try {
    await store.bootstrap()
  } catch (err) {
    // 启动失败要留在面板里，而不是中断挂载：下面的滚动跟随与快捷键仍需生效。
    store.engineError = store.engineError || String(err?.message || err)
    logger.error('[workspace] bootstrap failed', err)
  }
  nextTick(() => scrollToBottom(false))
  const inner = scrollRef.value?.querySelector('.transcript-inner')
  if (inner && typeof ResizeObserver !== 'undefined') {
    observer = new ResizeObserver(() => {
      if (stickToBottom.value) scrollToBottom(false)
    })
    observer.observe(inner)
  }
  window.addEventListener('keydown', onKeydown)
})

onBeforeUnmount(() => {
  if (observer) {
    observer.disconnect()
    observer = null
  }
  window.removeEventListener('keydown', onKeydown)
})
</script>

<style scoped>
.workspace-root {
  --rail-width: 236px;
  --instrument-width: 320px;
  display: flex;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
}

.workspace-rail {
  flex: 0 0 var(--rail-width);
  width: var(--rail-width);
  min-width: 0;
}

.workspace-main {
  position: relative;
  display: flex;
  flex: 1 1 auto;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.workspace-instrument {
  flex: 0 0 var(--instrument-width);
  width: var(--instrument-width);
  min-width: 0;
}

/* ── 顶栏 ───────────────────────────── */

.workspace-bar {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 8px;
  height: 40px;
  padding: 0 10px 0 8px;
  border-bottom: 1px solid var(--guaxi-border);
  background: var(--guaxi-content-bg);
}

.bar-left {
  display: flex;
  flex: 1 1 auto;
  align-items: center;
  gap: 7px;
  min-width: 0;
}

.bar-right {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 3px;
}

.bar-icon {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  width: 27px;
  height: 27px;
  padding: 0;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  cursor: pointer;
  transition: background 120ms ease, color 120ms ease;
}

.bar-icon:hover {
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-primary);
}

.bar-title {
  margin: 0;
  max-width: 320px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12.5px;
  font-weight: 500;
  line-height: 1.4;
}

.bar-state {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 5px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 9.5px;
  letter-spacing: 0.04em;
  color: var(--guaxi-text-secondary);
}

.state-dot {
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: currentColor;
  opacity: 0.65;
}

.bar-state--waiting,
.bar-state--running { color: #b47d1a; }

.bar-state--running .state-dot {
  animation: bar-pulse 1.6s ease-in-out infinite;
}

.bar-state--failed { color: #b42318; }

@keyframes bar-pulse {
  0%, 100% { opacity: 0.3; }
  50% { opacity: 1; }
}

.cwd-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  max-width: 210px;
  height: 26px;
  padding: 0 8px;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-family: inherit;
  font-size: 11px;
  cursor: pointer;
  transition: background 120ms ease, color 120ms ease;
}

.cwd-chip:hover {
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-primary);
}

.cwd-chip span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cwd-chip :deep(svg) {
  flex: 0 0 auto;
  opacity: 0.6;
}

.diff-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 25px;
  padding: 0 8px;
  border: 1px solid var(--guaxi-border);
  border-radius: 6px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-family: var(--guaxi-editor-font-family);
  font-size: 10px;
  cursor: pointer;
  transition: background 120ms ease;
}

.diff-chip:hover {
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-primary);
}

.num-add { color: #4f8a5b; }
.num-del { color: #b42318; }

/* ── 引擎告警 ───────────────────────── */

.workspace-alert {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 10px;
  margin: 8px 16px 0;
  padding: 7px 11px;
  border: 1px solid rgba(180, 125, 26, 0.42);
  border-radius: 6px;
  background: rgba(180, 125, 26, 0.07);
}

.alert-text {
  flex: 1 1 auto;
  font-size: 12px;
  line-height: 1.5;
  color: #b47d1a;
  word-break: break-word;
}

.alert-btn {
  flex: 0 0 auto;
  height: 23px;
  padding: 0 10px;
  border-radius: 5px;
  border: 1px solid rgba(180, 125, 26, 0.5);
  background: transparent;
  color: #b47d1a;
  font-size: 11.5px;
  font-family: inherit;
  cursor: pointer;
}

.alert-btn:disabled {
  opacity: 0.55;
  cursor: default;
}

/* ── 转录 ───────────────────────────── */

.workspace-transcript {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  padding: 0 24px;
  scrollbar-width: thin;
}

.transcript-inner {
  width: 100%;
  max-width: 760px;
  margin: 0 auto;
  padding: 20px 0 8px;
}

.transcript-inner :deep(.timeline-empty) {
  margin: 6vh auto 0;
}

.workspace-jump {
  position: absolute;
  right: 24px;
  bottom: 132px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-secondary);
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.12);
  cursor: pointer;
}

.workspace-jump:hover {
  color: var(--guaxi-text-primary);
}

/* ── 输入区 ─────────────────────────── */

.composer-zone {
  flex: 0 0 auto;
  border-top: 1px solid var(--guaxi-border);
  background: var(--guaxi-content-bg);
}

.composer-zone :deep(.composer-root) {
  width: 100%;
  /* 808px 是输入区自身的可见宽度，再加上组件左右各 28px 内边距（border-box）。 */
  max-width: calc(808px + 28px * 2);
  margin: 0 auto;
  border-top: none;
}

/* ── 图片预览 ───────────────────────── */

.image-layer {
  position: fixed;
  inset: 0;
  z-index: 1300;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 40px;
  background: rgba(8, 10, 12, 0.7);
  backdrop-filter: blur(3px);
  cursor: zoom-out;
}

.image-layer img {
  max-width: 88vw;
  max-height: 84vh;
  border-radius: 8px;
  box-shadow: 0 24px 60px rgba(0, 0, 0, 0.45);
}

@media (max-width: 1320px) {
  .workspace-root { --instrument-width: 288px; }
}

@media (max-width: 1080px) {
  .workspace-instrument { display: none; }
}

@media (max-width: 860px) {
  .workspace-rail { display: none; }
  .transcript-inner { max-width: none; }
}

@media (prefers-reduced-motion: reduce) {
  .bar-state--running .state-dot { animation: none; }
}
</style>
