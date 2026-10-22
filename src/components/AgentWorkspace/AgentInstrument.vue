<template>
  <aside class="instrument-root">
    <!-- 雷荷波：仪表主体。它表示"这台机器正在运转"，不是背景装饰。 -->
    <div class="instrument-hero">
      <GuaxiRehoboamBackdrop :active="running" :telemetry="telemetry" />
      <div class="instrument-hero-label">
        <span class="hero-state" :class="`hero-state--${stateKey}`">{{ stateText }}</span>
      </div>
    </div>

    <div class="instrument-body">
      <!-- 待批：整块界面里唯一会着色、并且置顶的东西。 -->
      <section v-if="approvals.length" class="inst-section inst-section--alert">
        <header class="inst-head">
          <span class="inst-title">需要你决定</span>
          <span class="inst-badge">{{ approvals.length }}</span>
        </header>
        <button type="button" class="inst-alert-btn" @click="$emit('review-approval')">
          查看并处理
        </button>
      </section>

      <section class="inst-section">
        <header class="inst-head">
          <span class="inst-title">计划</span>
          <span class="inst-meta">{{ planDone }}/{{ plan.length }}</span>
        </header>
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
          <span class="inst-title">改动</span>
          <span class="inst-meta">
            <span v-if="diffStats.added" class="num-add">+{{ diffStats.added }}</span>
            <span v-if="diffStats.removed" class="num-del">−{{ diffStats.removed }}</span>
            <template v-if="!diffStats.added && !diffStats.removed">{{ files.length }} 文件</template>
          </span>
        </header>
        <ul v-if="files.length" class="inst-files">
          <li v-for="f in files.slice(0, 6)" :key="f.path" class="inst-file" :title="f.path">
            <span class="inst-file-name">{{ baseName(f.path) }}</span>
            <span class="inst-file-nums">
              <span v-if="f.added" class="num-add">+{{ f.added }}</span>
              <span v-if="f.removed" class="num-del">−{{ f.removed }}</span>
            </span>
          </li>
        </ul>
        <div v-else class="inst-empty">还没有文件改动</div>
        <button v-if="diff" type="button" class="inst-link" @click="$emit('open-diff')">查看完整 diff</button>
      </section>

      <section class="inst-section">
        <header class="inst-head">
          <span class="inst-title">用量</span>
        </header>
        <dl class="inst-kv">
          <div class="inst-kv-row">
            <dt>上下文</dt>
            <dd>{{ contextText }}</dd>
          </div>
          <div class="inst-kv-row">
            <dt>动作</dt>
            <dd>{{ stats.command + stats.file + stats.tool + stats.search }} 次</dd>
          </div>
          <div class="inst-kv-row">
            <dt>耗时</dt>
            <dd>{{ elapsedText }}</dd>
          </div>
        </dl>
        <div v-if="usageRatio !== null" class="inst-bar" :title="contextText">
          <span class="inst-bar-fill" :style="{ width: `${Math.min(100, usageRatio * 100)}%` }" />
        </div>
      </section>

      <section class="inst-section inst-section--quiet">
        <dl class="inst-kv">
          <div class="inst-kv-row">
            <dt>模式</dt>
            <dd>{{ modeLabel }}</dd>
          </div>
          <div class="inst-kv-row">
            <dt>沙箱</dt>
            <dd class="mono">{{ sandbox }}</dd>
          </div>
          <div class="inst-kv-row">
            <dt>引擎</dt>
            <dd class="mono">{{ engineText }}</dd>
          </div>
        </dl>
      </section>
    </div>
  </aside>
</template>

<script setup>
/**
 * 右侧仪表。
 *
 * 设计约束（来自方案 §5）：
 * - 一个主角：雷荷波。其余全部安静——细线、无卡片堆叠、无阴影。
 * - 结构即信息：每一段都对应真实数据；没有数据就不画。
 * - 只有「需要你」用色（琥珀）。成功就是安静地变成已完成。
 */
import { computed } from 'vue'
import GuaxiRehoboamBackdrop from '../ChatViewComponent/GuaxiRehoboamBackdrop.vue'

const props = defineProps({
  running: { type: Boolean, default: false },
  approvals: { type: Array, default: () => [] },
  plan: { type: Array, default: () => [] },
  ledger: { type: Array, default: () => [] },
  usage: { type: Object, default: null },
  stats: { type: Object, default: () => ({}) },
  diff: { type: String, default: '' },
  modeLabel: { type: String, default: '' },
  sandbox: { type: String, default: '' },
  engine: { type: Object, default: null },
  session: { type: Object, default: null },
})

defineEmits(['review-approval', 'open-diff'])

const baseName = (p) => String(p || '').split(/[\\/]/).pop()

const planDone = computed(() => props.plan.filter((s) => s.status === 'completed' || s.status === 'done').length)

const files = computed(() => props.ledger.filter((e) => e.kind === 'file'))

const diffStats = computed(() => ({
  added: files.value.reduce((n, f) => n + (Number(f.added) || 0), 0),
  removed: files.value.reduce((n, f) => n + (Number(f.removed) || 0), 0),
}))

const usageRatio = computed(() => {
  const u = props.usage
  if (!u || !u.contextWindow) return null
  return Math.min(1, (u.inputTokens + u.outputTokens) / u.contextWindow)
})

const contextText = computed(() => {
  const u = props.usage
  if (!u) return '—'
  const fmt = (n) => (n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n))
  if (u.contextWindow) return `${fmt(u.inputTokens + u.outputTokens)} / ${fmt(u.contextWindow)}`
  return fmt(u.totalTokens || 0)
})

const elapsedText = computed(() => {
  const s = props.session
  if (!props.running || !s?.updatedAt) {
    const ms = Number(props.session?.lastDurationMs || 0)
    return ms ? `${(ms / 1000).toFixed(0)}s` : '—'
  }
  const secs = Math.max(0, Math.round((Date.now() - (s.updatedAt || Date.now())) / 1000))
  return `${secs}s`
})

const engineText = computed(() => {
  const e = props.engine
  if (!e) return '未连接'
  if (!e.binary_exists) return '缺失'
  return e.running ? `app-server · ${e.binary_version || ''}`.trim() : '待启动'
})

const stateKey = computed(() => {
  if (props.approvals.length) return 'waiting'
  if (props.running) return 'running'
  const status = String(props.session?.status || 'idle')
  if (status === 'failed') return 'failed'
  return 'idle'
})

const stateText = computed(() => ({
  waiting: '等待确认',
  running: '运行中',
  failed: '上次失败',
  idle: '空闲',
}[stateKey.value]))

/** 给雷荷波的数据。全部来自真实事件，缺项就不画。 */
const telemetry = computed(() => {
  const s = props.stats || {}
  const toolCount = (s.command || 0) + (s.file || 0) + (s.tool || 0) + (s.search || 0)
  // token 速率的归一：out/总 的比例作为"这次跑得多用力"的近似。
  // 没有真实速率时用 0（基线），绝不随机。
  const u = props.usage
  const rate = u && u.totalTokens ? Math.min(1, u.outputTokens / Math.max(1, u.totalTokens)) : 0
  const name = props.session?.cwd ? baseName(props.session.cwd) : ''
  return {
    tokenRate: props.running ? Math.max(0.15, rate) : 0,
    toolCount,
    pending: props.approvals.length,
    itemIndex: props.ledger.length,
    caption: name ? `${props.modeLabel}\n${name}` : props.modeLabel,
  }
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
  height: 250px;
  overflow: hidden;
}

.instrument-hero-label {
  position: absolute;
  left: 14px;
  bottom: 10px;
}

.hero-state {
  font-size: 11.5px;
  letter-spacing: 0.02em;
  color: var(--guaxi-text-secondary);
}

.hero-state--running { color: var(--guaxi-text-primary); }
.hero-state--waiting { color: #b47d1a; }
.hero-state--failed { color: #b42318; }

.instrument-body {
  flex: 1 1 auto;
  overflow-y: auto;
  padding: 4px 14px 20px;
}

.inst-section {
  padding: 12px 0;
  border-top: 1px solid var(--guaxi-border);
}

.inst-section:first-child {
  border-top: none;
}

.inst-section--alert {
  padding: 10px 0;
}

.inst-section--quiet {
  opacity: 0.82;
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
  min-width: 16px;
  height: 16px;
  padding: 0 5px;
  border-radius: 8px;
  background: rgba(180, 125, 26, 0.16);
  color: #b47d1a;
  font-size: 10.5px;
  font-family: var(--guaxi-editor-font-family);
}

.inst-meta {
  margin-left: auto;
  font-size: 11px;
  color: var(--guaxi-text-secondary);
  font-family: var(--guaxi-editor-font-family);
}

.inst-alert-btn {
  width: 100%;
  height: 28px;
  border-radius: 5px;
  border: 1px solid rgba(180, 125, 26, 0.55);
  background: rgba(180, 125, 26, 0.08);
  color: #b47d1a;
  font-size: 12.5px;
  cursor: pointer;
}

.inst-alert-btn:hover {
  background: rgba(180, 125, 26, 0.15);
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
  font-size: 12.5px;
  line-height: 1.5;
  color: var(--guaxi-text-secondary);
}

.inst-plan-mark {
  flex: 0 0 auto;
  width: 5px;
  height: 5px;
  margin-top: 7px;
  border-radius: 50%;
  border: 1px solid var(--guaxi-text-secondary);
}

.inst-plan-row--inProgress { color: var(--guaxi-text-primary); }
.inst-plan-row--inProgress .inst-plan-mark {
  background: #b47d1a;
  border-color: #b47d1a;
}

.inst-plan-row--completed { color: var(--guaxi-text-primary); }
.inst-plan-row--completed .inst-plan-mark {
  background: var(--guaxi-text-secondary);
  border-color: var(--guaxi-text-secondary);
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
  padding: 2px 0;
  font-size: 12px;
}

.inst-file-name {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--guaxi-editor-font-family);
  font-size: 11.5px;
  color: var(--guaxi-text-secondary);
}

.inst-file-nums {
  flex: 0 0 auto;
  display: flex;
  gap: 5px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 11px;
}

.num-add { color: #4f8a5b; }
.num-del { color: #b42318; }

.inst-link {
  margin-top: 6px;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 12px;
  text-decoration: underline;
  text-underline-offset: 3px;
  cursor: pointer;
}

.inst-link:hover { color: var(--guaxi-text-primary); }

.inst-kv {
  margin: 0;
}

.inst-kv-row {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 2px 0;
  font-size: 12px;
}

.inst-kv-row dt {
  flex: 0 0 46px;
  color: var(--guaxi-text-secondary);
  opacity: 0.78;
}

.inst-kv-row dd {
  margin: 0;
  color: var(--guaxi-text-primary);
}

.inst-kv-row dd.mono {
  font-family: var(--guaxi-editor-font-family);
  font-size: 11.5px;
}

.inst-bar {
  margin-top: 7px;
  height: 2px;
  border-radius: 1px;
  background: var(--guaxi-border);
  overflow: hidden;
}

.inst-bar-fill {
  display: block;
  height: 100%;
  background: var(--guaxi-text-secondary);
  opacity: 0.65;
}

.inst-empty {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  opacity: 0.62;
}
</style>
