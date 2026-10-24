<template>
  <div
    class="workspace-root"
    :class="{
      'is-rail-collapsed': railCollapsed,
      'is-instrument-collapsed': instrumentCollapsed,
    }"
  >
    <AgentSessionRail
      v-if="!railCollapsed"
      class="workspace-rail"
      :sessions="store.sessions"
      :current-key="store.currentKey"
      :running-key="runningKey"
      :engine="store.engine"
      @new="onNew"
      @select="onSelect"
      @remove="onRemove"
    />

    <main class="workspace-main">
      <header class="workspace-head">
        <h1 class="workspace-title">{{ session?.title || '新任务' }}</h1>
        <span class="workspace-sub">{{ modeLabel }}</span>
        <button
          v-if="session?.diff"
          type="button"
          class="workspace-link"
          @click="openDiff"
        >
          查看 diff
        </button>
      </header>

      <!-- 引擎连不上时把原因摆在最上面，并给一个重试入口。
           不给重试的话，用户只会看到「未运行」，既不知道能不能修，也不知道该等还是该动。 -->
      <div v-if="engineBlocked" class="workspace-alert">
        <span class="alert-text">{{ engineBlocked }}</span>
        <button type="button" class="alert-btn" :disabled="retrying" @click="onRetryEngine">
          {{ retrying ? '重连中…' : '重试连接' }}
        </button>
      </div>

      <div ref="scrollRef" class="workspace-transcript" @scroll="onScroll">
        <div v-if="!messages.length" class="workspace-empty">
          <p class="empty-title">给这台机器一个任务</p>
          <p class="empty-body">
            说清楚要达成什么即可。它会自己读文件、跑命令、查资料；每一步都在右侧账本里。
            需要你决定的时候，会在输入框上方问你。
          </p>
          <div class="empty-mode-list">
            <button
              v-for="m in AGENT_MODES"
              :key="m.id"
              type="button"
              class="empty-mode"
              @click="onPickMode(m.id)"
            >
              <span class="empty-mode-label">{{ m.label }}</span>
              <span class="empty-mode-hint">{{ m.hint }}</span>
            </button>
          </div>
        </div>

        <div class="transcript-inner">
          <article
            v-for="msg in messages"
            :key="msg.id"
            class="msg"
            :class="`msg--${msg.role}`"
          >
            <div v-if="msg.role === 'user'" class="msg-user">
              <div class="msg-user-text">{{ msg.text }}</div>
              <span v-if="msg.steer" class="msg-steer">转向</span>
              <div v-if="msg.images?.length" class="msg-images">
                <span v-for="(p, i) in msg.images" :key="i" class="msg-image">{{ baseName(p) }}</span>
              </div>
            </div>
            <div v-else-if="msg.role === 'system'" class="msg-system">{{ msg.text }}</div>
            <AgentMarkdown v-else :text="msg.text" :streaming="Boolean(msg.streaming)" />
          </article>
        </div>

        <AgentRunLedger
          v-if="session?.ledger?.length"
          class="transcript-ledger"
          :ledger="session.ledger"
          :running="store.running"
        />
      </div>

      <button
        v-if="showJump"
        type="button"
        class="workspace-jump"
        title="跳到最新"
        @click="jumpToBottom"
      >
        ↓
      </button>

      <AgentComposer
        :mode="store.currentMode"
        :cwd="session?.cwd || ''"
        :model="session?.model || ''"
        :models="store.models"
        :pending="store.running"
        :gui-enabled="guiEnabled"
        @send="onSend"
        @steer="onSteer"
        @stop="onStop"
        @update:mode="onPickMode"
        @update:model="onPickModel"
        @pick-workspace="onPickWorkspace"
      />
    </main>

    <AgentInstrument
      v-if="!instrumentCollapsed"
      class="workspace-instrument"
      :running="store.running"
      :approvals="store.approvals"
      :plan="session?.plan || []"
      :ledger="session?.ledger || []"
      :usage="session?.usage || null"
      :stats="store.runStats"
      :diff="session?.diff || ''"
      :mode-label="modeLabel"
      :sandbox="session?.sandbox || 'workspace-write'"
      :engine="store.engine"
      :session="session"
      @review-approval="showApproval = true"
      @open-diff="openDiff"
    />

    <AgentApprovalSheet
      v-if="showApproval && store.approvals.length"
      :queue="store.approvals"
      :index="approvalIndex"
      @decide="onDecide"
    />
  </div>
</template>

<script setup>
/**
 * Agent 工作台。
 *
 * 布局：会话栏 | 转录（≤68ch 正文） | 仪表。
 * - 转录列是左对齐的阅读栏，仪表再好看也不抢它的宽度；
 * - 仪表只读，不参与操作——操作全在输入区与审批浮层；
 * - 审批浮层在需要时盖住一切，处理完自动让位。
 */
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useMessage } from 'naive-ui'
import AgentSessionRail from './AgentSessionRail.vue'
import AgentComposer from './AgentComposer.vue'
import AgentRunLedger from './AgentRunLedger.vue'
import AgentInstrument from './AgentInstrument.vue'
import AgentApprovalSheet from './AgentApprovalSheet.vue'
import AgentMarkdown from './AgentMarkdown.vue'
import { AGENT_MODES, useAgentWorkspaceStore } from '../../stores/agentWorkspaceStore'
import { useCognitionStore } from '../../stores/cognitionStore'
import { useDiffCompareStore } from '../../stores/diffCompareStore'
import { useSiderStore } from '../../stores/useSiderStore'
import { createLogger } from '../../utils/logger'

const logger = createLogger('AgentWorkspace')
const message = useMessage()
const store = useAgentWorkspaceStore()
const cognition = useCognitionStore()
const diffStore = useDiffCompareStore()
const siderStore = useSiderStore()

const railCollapsed = computed(() => siderStore.isLeftSiderCollapsed.chat === true)
const instrumentCollapsed = computed(() => siderStore.isRightSiderCollapsed.chat === true)

const scrollRef = ref(null)
const showApproval = ref(false)
const showJump = ref(false)
const approvalIndex = ref(0)

const session = computed(() => store.currentSession)
const messages = computed(() => session.value?.messages || [])
const runningKey = computed(() => (store.running ? store.currentKey : ''))

const modeLabel = computed(() => store.currentMode?.label || '编码')

/** GUI 是否已授权：设置里的总开关 + 认知层的 Tier::Gui 两者都开才可用。 */
/** 引擎不可用的可读原因；为空表示一切正常。 */
const engineBlocked = computed(() => {
  const e = store.engine
  if (store.engineError) return store.engineError
  if (e && !e.binary_exists) return '没有找到 codex 可执行文件，Agent 无法运行'
  return ''
})

const retrying = ref(false)

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

const guiEnabled = computed(() => {
  const tier = cognition.status?.tiers?.find?.((t) => t?.[0] === 'gui')
  return Boolean(tier?.[1])
})

const baseName = (p) => String(p || '').split(/[\\/]/).pop()

// ───────────── 滚动

const BOTTOM_THRESHOLD = 60

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

const jumpToBottom = () => scrollToBottom(true)

const onScroll = () => {
  showJump.value = !isNearBottom()
}

// 只有本来就在底部时才自动跟随；用户往回翻的时候就不要再把他拽下去。
watch(
  () => [messages.value.length, messages.value.at(-1)?.text?.length, session.value?.ledger?.length],
  () => {
    if (isNearBottom()) nextTick(() => scrollToBottom(false))
  },
)

// ───────────── 生命周期

let offNotification = null

onMounted(async () => {
  await store.ensureListening()
  await store.loadEngine()
  await store.loadModels()
  // 启动时就查一次 GUI 授权，避免「操作电脑」看起来能用其实会被拒。
  try {
    await cognition.refreshStatus?.()
  } catch (err) {
    logger.warn('[agent] cognition status failed', { error: String(err?.message || err) })
  }
  nextTick(() => scrollToBottom(false))
})

onBeforeUnmount(() => {
  if (typeof offNotification === 'function') offNotification()
})

// ───────────── 审批

// 有待批时自动弹出来。这是整块界面唯一会主动打断你的地方——
// 因为它的全部意义就是"现在必须你决定"。
watch(
  () => store.approvals.length,
  (n, prev) => {
    if (n && !prev) {
      approvalIndex.value = 0
      showApproval.value = true
    }
    if (!n) showApproval.value = false
  },
)

const onDecide = async ({ requestId, decision }) => {
  await store.resolveApproval(requestId, decision)
  if (approvalIndex.value >= store.approvals.length) {
    approvalIndex.value = Math.max(0, store.approvals.length - 1)
  }
  if (!store.approvals.length) showApproval.value = false
}

// ───────────── 发送

const onSend = async (text) => {
  try {
    await store.sendTurn(text)
    store.suggestTitle()
    nextTick(() => scrollToBottom(true))
  } catch (err) {
    message.error(String(err?.message || err))
  }
}

const onSteer = async (text) => {
  await store.steer(text)
  nextTick(() => scrollToBottom(true))
}

const onStop = async () => {
  await store.interrupt()
  store.finishStreamingMessages()
}

// ───────────── 模式 / 模型 / 工作区

const onPickMode = (id) => {
  const mode = AGENT_MODES.find((m) => m.id === id)
  if (!mode) return
  if (mode.requiresGui && !guiEnabled.value) {
    message.warning('「操作电脑」需要先在 设置 → 认知层 → 工具授权 里开启 gui 级')
  }
  store.patchSession({
    mode: id,
    sandbox: mode.sandbox,
  })
}

const onPickModel = (model) => {
  store.patchSession({ model })
}

const onPickWorkspace = async () => {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const picked = await open({ directory: true, multiple: false, title: '选择工作目录' })
    if (typeof picked === 'string' && picked) {
      // 换目录意味着换线程：codex 的工作目录在建线程时固定，不能中途改。
      store.patchSession({ cwd: picked, threadId: '', messages: [], ledger: [], plan: [], diff: '' })
    }
  } catch (err) {
    logger.warn('[agent] pick workspace failed', { error: String(err?.message || err) })
  }
}

// ───────────── 会话

const onNew = () => {
  store.newSession({ mode: store.currentMode?.id || 'code' })
}

const onSelect = (key) => {
  if (key === store.currentKey) return
  store.selectSession(key)
  nextTick(() => scrollToBottom(false))
}

const onRemove = (key) => {
  const target = store.sessions.find((s) => s.key === key)
  if (!target) return
  if (store.running && key === store.currentKey) {
    message.warning('运行中的任务不能删除，先停止它')
    return
  }
  store.deleteSession(key)
}

// ───────────── diff

const openDiff = () => {
  const diff = session.value?.diff
  if (!diff) {
    message.info('本次运行还没有产生 diff')
    return
  }
  // 复用既有的独立 diff 窗口：它已经处理了虚拟滚动、分栏、目录树这些。
  // 这里给的是「空 → 当前 diff」的文本对比，读起来就是这次 run 改了什么。
  diffStore.compareTexts('', diff, '原始', '本次运行')
}
</script>

<style scoped>
.workspace-root {
  display: grid;
  /* 会话栏固定，转录弹性，仪表固定。仪表再宽也不挤占正文。
     两侧宽度放在变量里：收起时只改变量，窄屏断点仍能整体换一套宽度。 */
  --rail-width: 196px;
  --instrument-width: 268px;
  grid-template-columns: var(--rail-width) minmax(0, 1fr) var(--instrument-width);
  height: 100%;
  min-height: 0;
  overflow: hidden;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
}

.workspace-root.is-rail-collapsed {
  --rail-width: 0px;
}

.workspace-root.is-instrument-collapsed {
  --instrument-width: 0px;
}

.workspace-main {
  position: relative;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  /* 收起左右栏时对应轨道会被压到 0，这里按列固定位置：
     否则只剩中间一栏时，它会被自动排进第一列（宽度 0）。 */
  grid-column: 2;
}

.workspace-rail {
  grid-column: 1;
}

.workspace-instrument {
  grid-column: 3;
}

.workspace-head {
  display: flex;
  align-items: baseline;
  gap: 10px;
  padding: 12px 24px 10px;
  border-bottom: 1px solid var(--guaxi-border);
}

.workspace-title {
  margin: 0;
  font-size: 14px;
  font-weight: 500;
  line-height: 1.4;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.workspace-sub {
  font-size: 11.5px;
  color: var(--guaxi-text-secondary);
}

.workspace-link {
  margin-left: auto;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 12px;
  text-decoration: underline;
  text-underline-offset: 3px;
  cursor: pointer;
}

.workspace-link:hover { color: var(--guaxi-text-primary); }

.workspace-alert {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 0 24px 8px;
  padding: 8px 12px;
  border: 1px solid rgba(180, 125, 26, 0.45);
  border-radius: 6px;
  background: rgba(180, 125, 26, 0.08);
}

.alert-text {
  flex: 1 1 auto;
  font-size: 12.5px;
  line-height: 1.5;
  color: #b47d1a;
  word-break: break-word;
}

.alert-btn {
  flex: 0 0 auto;
  height: 24px;
  padding: 0 10px;
  border-radius: 5px;
  border: 1px solid rgba(180, 125, 26, 0.55);
  background: transparent;
  color: #b47d1a;
  font-size: 12px;
  cursor: pointer;
}

.alert-btn:disabled {
  opacity: 0.55;
  cursor: default;
}

.workspace-transcript {
  flex: 1 1 auto;
  overflow-y: auto;
  padding: 18px 24px 8px;
  scroll-behavior: auto;
}

.transcript-inner {
  /* 正文限宽：超过 68ch 之后眼睛要来回扫，读起来很累。 */
  max-width: 68ch;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.transcript-ledger {
  max-width: 68ch;
  margin-top: 18px;
}

.workspace-empty {
  max-width: 58ch;
  padding: 26px 0 18px;
}

.empty-title {
  margin: 0 0 8px;
  font-size: 17px;
  font-weight: 500;
  line-height: 1.5;
}

.empty-body {
  margin: 0 0 18px;
  font-size: 13.5px;
  line-height: 1.75;
  color: var(--guaxi-text-secondary);
}

.empty-mode-list {
  display: grid;
  gap: 1px;
  border: 1px solid var(--guaxi-border);
  border-radius: 7px;
  overflow: hidden;
}

.empty-mode {
  display: flex;
  align-items: baseline;
  gap: 10px;
  padding: 9px 12px;
  border: none;
  background: var(--guaxi-panel-bg);
  text-align: left;
  cursor: pointer;
  color: var(--guaxi-text-primary);
}

.empty-mode + .empty-mode {
  border-top: 1px solid var(--guaxi-border);
}

.empty-mode:hover {
  background: var(--guaxi-hover-bg);
}

.empty-mode-label {
  flex: 0 0 56px;
  font-size: 12.5px;
}

.empty-mode-hint {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.msg {
  min-width: 0;
}

.msg--user {
  display: flex;
  justify-content: flex-end;
}

.msg-user {
  max-width: 82%;
  padding: 8px 12px;
  border-radius: 8px;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
}

.msg-user-text {
  font-size: 14px;
  line-height: 1.65;
  white-space: pre-wrap;
  word-break: break-word;
}

.msg-steer {
  display: inline-block;
  margin-top: 4px;
  font-size: 10.5px;
  color: #b47d1a;
}

.msg-images {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 6px;
}

.msg-image {
  padding: 1px 7px;
  border-radius: 4px;
  background: var(--guaxi-hover-bg);
  font-family: var(--guaxi-editor-font-family);
  font-size: 11px;
  color: var(--guaxi-text-secondary);
}

.msg-system {
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--guaxi-text-secondary);
  padding-left: 9px;
  border-left: 2px solid var(--guaxi-border);
}

.workspace-jump {
  position: absolute;
  right: 24px;
  bottom: 120px;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-secondary);
  cursor: pointer;
}

@media (max-width: 1200px) {
  .workspace-root {
    --rail-width: 170px;
    --instrument-width: 232px;
  }
}

@media (max-width: 980px) {
  /* 窄屏先收起仪表：正文比仪表重要。 */
  .workspace-root {
    grid-template-columns: var(--rail-width) minmax(0, 1fr);
  }

  .workspace-instrument {
    display: none;
  }
}
</style>
