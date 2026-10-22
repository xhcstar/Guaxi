<template>
  <aside class="rail-root">
    <header class="rail-head">
      <span class="rail-title">任务</span>
      <button type="button" class="rail-new" title="新建任务" @click="$emit('new')">＋</button>
    </header>

    <div class="rail-list">
      <button
        v-for="s in sessions"
        :key="s.key"
        type="button"
        class="rail-item"
        :class="{ 'rail-item--active': s.key === currentKey }"
        @click="$emit('select', s.key)"
        @contextmenu.prevent="$emit('remove', s.key)"
      >
        <span class="rail-dot" :class="`rail-dot--${dotKey(s)}`" />
        <span class="rail-item-main">
          <span class="rail-item-title">{{ s.title || '新任务' }}</span>
          <span class="rail-item-meta">
            {{ modeLabel(s.mode) }}
            <template v-if="s.cwd"> · {{ baseName(s.cwd) }}</template>
          </span>
        </span>
      </button>
    </div>

    <footer class="rail-foot">
      <span class="rail-engine" :class="{ 'rail-engine--bad': engineBad }" :title="engineTitle">
        {{ engineText }}
      </span>
    </footer>
  </aside>
</template>

<script setup>
/**
 * 会话栏。
 *
 * 一条会话 = 一个 codex thread = 一个工作目录 + 一份沙箱策略。
 * 所以这里不再按「模型 → topic」分组：模型是轮次参数，不是会话身份。
 *
 * 右键删除，不做悬停才出现的删除按钮——悬停按钮在窄栏里会盖住标题。
 */
import { computed } from 'vue'
import { AGENT_MODES } from '../../stores/agentWorkspaceStore'

const props = defineProps({
  sessions: { type: Array, default: () => [] },
  currentKey: { type: String, default: '' },
  runningKey: { type: String, default: '' },
  engine: { type: Object, default: null },
})

defineEmits(['new', 'select', 'remove'])

const modeMap = new Map(AGENT_MODES.map((m) => [m.id, m]))
const modeLabel = (id) => modeMap.get(id)?.label || '编码'
const baseName = (p) => String(p || '').split(/[\\/]/).filter(Boolean).pop() || ''

const dotKey = (s) => {
  if (s.key === props.runningKey) return 'running'
  if (s.status === 'failed') return 'failed'
  return 'idle'
}

const engineBad = computed(() => props.engine && !props.engine.binary_exists)
const engineText = computed(() => {
  const e = props.engine
  if (!e) return '引擎：未检测'
  if (!e.binary_exists) return '引擎缺失'
  return e.running ? `引擎 ${e.binary_version || '已连接'}` : '引擎待启动'
})
const engineTitle = computed(() => props.engine?.hint || props.engine?.binary_path || '')
</script>

<style scoped>
.rail-root {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  border-right: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
}

.rail-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--guaxi-border);
}

.rail-title {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.rail-new {
  margin-left: auto;
  width: 20px;
  height: 20px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 15px;
  line-height: 1;
  cursor: pointer;
}

.rail-new:hover {
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-primary);
}

.rail-list {
  flex: 1 1 auto;
  overflow-y: auto;
  padding: 4px 0;
}

.rail-item {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  width: 100%;
  padding: 7px 12px;
  border: none;
  background: transparent;
  text-align: left;
  cursor: pointer;
  color: var(--guaxi-text-secondary);
}

.rail-item:hover {
  background: var(--guaxi-hover-bg);
}

.rail-item--active {
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-primary);
}

.rail-dot {
  flex: 0 0 auto;
  width: 5px;
  height: 5px;
  margin-top: 6px;
  border-radius: 50%;
  background: var(--guaxi-text-secondary);
  opacity: 0.45;
}

.rail-dot--running {
  background: #b47d1a;
  opacity: 1;
  animation: rail-pulse 1.5s ease-in-out infinite;
}

.rail-dot--failed {
  background: #b42318;
  opacity: 0.9;
}

@keyframes rail-pulse {
  0%, 100% { opacity: 0.4; }
  50% { opacity: 1; }
}

.rail-item-main {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.rail-item-title {
  font-size: 12.5px;
  line-height: 1.45;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rail-item-meta {
  font-size: 11px;
  opacity: 0.62;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rail-foot {
  flex: 0 0 auto;
  padding: 8px 12px;
  border-top: 1px solid var(--guaxi-border);
}

.rail-engine {
  font-size: 11px;
  color: var(--guaxi-text-secondary);
  opacity: 0.72;
  font-family: var(--guaxi-editor-font-family);
}

.rail-engine--bad {
  color: #b42318;
  opacity: 1;
}

@media (prefers-reduced-motion: reduce) {
  .rail-dot--running { animation: none; opacity: 1; }
}
</style>
