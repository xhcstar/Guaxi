<template>
  <div class="ledger-root">
    <div class="ledger-head">
      <span class="ledger-title">本次运行</span>
      <span class="ledger-summary">{{ summaryText }}</span>
    </div>

    <div class="ledger-body">
      <template v-for="entry in entries" :key="entry.id">
        <!-- 文件改动行：不同文件合并成一条，点开看逐文件增删 -->
        <div v-if="entry.kind === 'file'" class="ledger-row ledger-row--file">
          <span class="ledger-rail ledger-rail--file" />
          <span class="ledger-label">{{ baseName(entry.path) }}</span>
          <span class="ledger-nums">
            <span v-if="entry.added" class="num-add">+{{ entry.added }}</span>
            <span v-if="entry.removed" class="num-del">−{{ entry.removed }}</span>
          </span>
        </div>

        <div v-else class="ledger-row" :class="`ledger-row--${statusClass(entry.status)}`">
          <span class="ledger-rail" />
          <button
            type="button"
            class="ledger-main"
            :disabled="!hasDetail(entry)"
            @click="toggle(entry.id)"
          >
            <span class="ledger-label">{{ entry.title }}</span>
            <span v-if="entry.detail?.exitCode !== null && entry.detail?.exitCode !== undefined" class="ledger-exit" :class="{ 'ledger-exit--bad': entry.detail.exitCode !== 0 }">
              exit {{ entry.detail.exitCode }}
            </span>
            <span class="ledger-status">{{ statusText(entry) }}</span>
            <span v-if="hasDetail(entry)" class="ledger-caret" :class="{ 'ledger-caret--open': expanded.has(entry.id) }">›</span>
          </button>
        </div>

        <div v-if="expanded.has(entry.id)" class="ledger-detail">
          <pre v-if="entry.detail?.command" class="ledger-pre">{{ entry.detail.command }}</pre>
          <pre v-if="entry.detail?.output" class="ledger-pre ledger-pre--out">{{ entry.detail.output }}</pre>
          <pre v-if="entry.detail?.error" class="ledger-pre ledger-pre--err">{{ entry.detail.error }}</pre>
          <pre v-if="entry.detail?.args" class="ledger-pre">{{ pretty(entry.detail.args) }}</pre>
          <pre v-if="entry.detail?.result" class="ledger-pre ledger-pre--out">{{ pretty(entry.detail.result) }}</pre>
        </div>
      </template>

      <div v-if="!entries.length" class="ledger-empty">还没有动作</div>
    </div>
  </div>
</template>

<script setup>
/**
 * 运行账本：一次 run 里做了什么。
 *
 * 刻意不用卡片。一次执行里的 12 条命令、3 个文件不是 15 张独立的卡片，
 * 它们是一条线上的事——所以用左侧一条竖引导线 + 紧排的行，读起来是"过程"而不是"列表"。
 *
 * 行默认单行，点开才展开参数/输出。理由：运行时最该看见的是"它在干什么"，
 * 命令输出只在出问题时才有价值。
 */
import { computed, ref, watch } from 'vue'

const props = defineProps({
  ledger: { type: Array, default: () => [] },
  running: { type: Boolean, default: false },
})

const expanded = ref(new Set())

const entries = computed(() => (Array.isArray(props.ledger) ? props.ledger : []))

const summaryText = computed(() => {
  const items = entries.value.filter((e) => e.kind === 'item')
  const files = entries.value.filter((e) => e.kind === 'file')
  if (!items.length && !files.length) return props.running ? '正在开始…' : '—'
  const parts = []
  if (items.length) parts.push(`${items.length} 个动作`)
  if (files.length) parts.push(`${files.length} 个文件`)
  return parts.join(' · ')
})

const baseName = (p) => String(p || '').split(/[\\/]/).pop()

const statusClass = (status) => {
  const s = String(status || '')
  if (s === 'failed' || s === 'declined' || s === 'aborted' || s === 'error') return 'bad'
  if (s === 'completed' || s === 'succeeded') return 'ok'
  return 'run'
}

const statusText = (entry) => {
  const s = String(entry?.status || '')
  if (s === 'inProgress' || s === 'running' || s === 'in_progress') return '进行中'
  if (s === 'failed' || s === 'error') return '失败'
  if (s === 'declined') return '已拒绝'
  if (s === 'aborted') return '已中断'
  if (s === 'completed' || s === 'succeeded') {
    const ms = Number(entry?.detail?.durationMs || 0)
    return ms ? formatMs(ms) : '完成'
  }
  return s
}

const formatMs = (ms) => (ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(ms < 10000 ? 1 : 0)}s`)

const hasDetail = (entry) => {
  const d = entry?.detail
  if (!d) return false
  return Boolean(d.command || d.output || d.error || d.args || d.result)
}

const pretty = (v) => {
  try {
    return typeof v === 'string' ? v : JSON.stringify(v, null, 2)
  } catch {
    return String(v)
  }
}

const toggle = (id) => {
  const next = new Set(expanded.value)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  expanded.value = next
}

// 新一轮开始时收起所有展开项：上一次的输出留着展开只会挡住这一次。
watch(
  () => props.ledger,
  (list) => {
    if (!list?.length) expanded.value = new Set()
  },
)
</script>

<style scoped>
.ledger-root {
  border: 1px solid var(--guaxi-border);
  border-radius: 7px;
  overflow: hidden;
  background: var(--guaxi-panel-bg);
}

.ledger-head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 7px 12px;
  border-bottom: 1px solid var(--guaxi-border);
}

.ledger-title {
  font-size: 12px;
  color: var(--guaxi-text-primary);
  font-weight: 500;
}

.ledger-summary {
  margin-left: auto;
  font-size: 11.5px;
  color: var(--guaxi-text-secondary);
  font-family: var(--guaxi-editor-font-family);
}

.ledger-body {
  max-height: 260px;
  overflow-y: auto;
  padding: 4px 0;
}

.ledger-row {
  display: flex;
  align-items: stretch;
  gap: 0;
  padding-left: 12px;
  font-size: 12.5px;
  line-height: 1.55;
}

.ledger-rail {
  width: 2px;
  margin: 3px 0;
  border-radius: 1px;
  background: var(--guaxi-border);
  flex: 0 0 auto;
}

.ledger-row--ok .ledger-rail { background: rgba(120, 150, 120, 0.55); }
.ledger-row--bad .ledger-rail { background: rgba(180, 90, 90, 0.7); }
.ledger-row--run .ledger-rail {
  background: rgba(180, 125, 26, 0.75);
  animation: ledger-rail-pulse 1.4s ease-in-out infinite;
}

@keyframes ledger-rail-pulse {
  0%, 100% { opacity: 0.45; }
  50% { opacity: 1; }
}

.ledger-main {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1 1 auto;
  min-width: 0;
  padding: 3px 10px 3px 9px;
  border: none;
  background: transparent;
  color: var(--guaxi-text-primary);
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.ledger-main:disabled {
  cursor: default;
}

.ledger-main:not(:disabled):hover {
  background: var(--guaxi-hover-bg);
}

.ledger-label {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--guaxi-editor-font-family);
  font-size: 12px;
}

.ledger-exit {
  flex: 0 0 auto;
  font-size: 11px;
  color: var(--guaxi-text-secondary);
  font-family: var(--guaxi-editor-font-family);
}

.ledger-exit--bad {
  color: #b42318;
}

.ledger-status {
  flex: 0 0 auto;
  font-size: 11px;
  color: var(--guaxi-text-secondary);
}

.ledger-row--bad .ledger-status { color: #b42318; }

.ledger-caret {
  flex: 0 0 auto;
  color: var(--guaxi-text-secondary);
  font-size: 13px;
  transition: transform 0.15s ease;
}

.ledger-caret--open {
  transform: rotate(90deg);
}

.ledger-row--file {
  padding-top: 2px;
  padding-bottom: 2px;
}

.ledger-row--file .ledger-label {
  padding-left: 9px;
  color: var(--guaxi-text-secondary);
}

.ledger-nums {
  display: flex;
  gap: 6px;
  padding-right: 10px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 11px;
}

.num-add { color: #4f8a5b; }
.num-del { color: #b42318; }

.ledger-detail {
  padding: 2px 12px 6px 23px;
}

.ledger-pre {
  margin: 3px 0;
  padding: 6px 8px;
  max-height: 220px;
  overflow: auto;
  border-radius: 4px;
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-secondary);
  font-family: var(--guaxi-editor-font-family);
  font-size: 11.5px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-all;
}

.ledger-pre--err {
  color: #b42318;
}

.ledger-empty {
  padding: 8px 12px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  opacity: 0.7;
}
</style>
