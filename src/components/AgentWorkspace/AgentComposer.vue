<template>
  <div class="composer-root">
    <!-- 模式与工作区紧贴输入框上沿：它们是这次发送的参数，不是全局设置。 -->
    <div class="composer-meta">
      <n-dropdown :options="modeOptions" trigger="click" @select="onModeSelect">
        <button type="button" class="meta-chip meta-chip--mode" :title="mode.hint">
          <span class="meta-dot" :class="`meta-dot--${mode.id}`" />
          {{ mode.label }}
        </button>
      </n-dropdown>

      <button type="button" class="meta-chip" :title="cwd || '未选择工作目录'" @click="$emit('pick-workspace')">
        <span class="meta-key">工作区</span>
        <span class="meta-value">{{ workspaceLabel }}</span>
      </button>

      <n-dropdown :options="modelOptions" trigger="click" @select="onModelSelect">
        <button type="button" class="meta-chip" :title="modelHint">
          <span class="meta-key">模型</span>
          <span class="meta-value">{{ modelLabel }}</span>
        </button>
      </n-dropdown>

      <span v-if="mode.requiresGui && !guiEnabled" class="meta-warning" title="该模式需要「电脑操控」授权">
        未授权电脑操控
      </span>
      <span v-if="pending" class="composer-running">
        <span class="composer-running-dot" />
        运行中
      </span>
    </div>

    <div class="composer-box" :class="{ 'composer-box--running': pending }">
      <textarea
        ref="inputRef"
        v-model="text"
        class="composer-input"
        :placeholder="placeholder"
        rows="1"
        @keydown="onKeydown"
        @compositionstart="composing = true"
        @compositionend="composing = false"
        @input="autosize"
      />
      <div class="composer-actions">
        <span class="composer-hint">{{ hint }}</span>
        <button
          v-if="pending"
          type="button"
          class="composer-btn composer-btn--stop"
          title="停止（Esc）"
          @click="$emit('stop')"
        >
          停止
        </button>
        <button
          v-else
          type="button"
          class="composer-btn"
          :disabled="!canSend"
          title="发送（Enter）"
          @click="submit"
        >
          发送
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
/**
 * 输入区。
 *
 * 三件事按重要性排：能发（Enter）、能停（Esc/按钮）、能改模式。
 * 模式/工作区/模型放在上沿的 chip 里而不是塞进输入框——它们是这次发送的参数，
 * 放在框内会和"我正在说什么"混在一起。
 */
import { computed, nextTick, ref, watch } from 'vue'
import { AGENT_MODES } from '../../stores/agentWorkspaceStore'

const props = defineProps({
  mode: { type: Object, required: true },
  modes: { type: Array, default: () => AGENT_MODES },
  cwd: { type: String, default: '' },
  model: { type: String, default: '' },
  models: { type: Array, default: () => [] },
  pending: { type: Boolean, default: false },
  guiEnabled: { type: Boolean, default: false },
})

const emit = defineEmits(['send', 'stop', 'steer', 'update:mode', 'update:model', 'pick-workspace'])

const text = ref('')
const composing = ref(false)
const inputRef = ref(null)

const canSend = computed(() => !props.pending && text.value.trim().length > 0)

const placeholder = computed(() => {
  if (props.pending) return '运行中；输入新要求会作为「转向」插入当前任务'
  return props.mode.id === 'computer'
    ? '说明要在这台电脑上做什么…'
    : '描述任务，或直接说要改什么…'
})

const hint = computed(() => (props.pending ? 'Enter 转向 · Esc 停止' : 'Enter 发送 · Shift+Enter 换行'))

const workspaceLabel = computed(() => {
  const p = String(props.cwd || '').trim()
  if (!p) return '默认工作区'
  const parts = p.split(/[\\/]/).filter(Boolean)
  return parts.length > 1 ? `…/${parts.slice(-2).join('/')}` : p
})

const modelLabel = computed(() => props.model || '默认')
const modelHint = computed(() => (props.models.length ? `共 ${props.models.length} 个可用模型` : '尚未读取模型清单'))

const modeOptions = computed(() =>
  props.modes.map((m) => ({
    label: m.requiresGui && !props.guiEnabled ? `${m.label}（需授权）` : m.label,
    key: m.id,
  })),
)

const modelOptions = computed(() => {
  if (!props.models.length) return [{ label: '（先连接引擎）', key: '', disabled: true }]
  return props.models
    .filter((m) => !m.hidden)
    .map((m) => ({
      label: m.displayName || m.model || m.id,
      key: String(m.model || m.id),
    }))
})

const onModeSelect = (key) => emit('update:mode', key)
const onModelSelect = (key) => emit('update:model', key)

const autosize = () => {
  const el = inputRef.value
  if (!el) return
  el.style.height = 'auto'
  // 上限约 8 行，再多就在框内滚动。
  el.style.height = `${Math.min(el.scrollHeight, 190)}px`
}

const submit = () => {
  const value = text.value.trim()
  if (!value) return
  if (props.pending) {
    emit('steer', value)
  } else {
    emit('send', value)
  }
  text.value = ''
  nextTick(autosize)
}

const onKeydown = (event) => {
  // 中文输入法组合期间的 Enter 是选词，不是发送。
  if (composing.value || event.isComposing) return
  if (event.key === 'Escape' && props.pending) {
    event.preventDefault()
    emit('stop')
    return
  }
  // Shift+Enter 保留换行；Ctrl/Cmd+Enter 也当发送，照顾习惯。
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault()
    submit()
  }
}

watch(
  () => props.mode?.id,
  () => nextTick(autosize),
)
</script>

<style scoped>
.composer-root {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px 24px 16px;
  border-top: 1px solid var(--guaxi-border);
  background: var(--guaxi-content-bg);
}

.composer-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.meta-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 24px;
  padding: 0 9px;
  border: 1px solid var(--guaxi-border);
  border-radius: 999px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 12.5px;
  cursor: pointer;
  transition: border-color 0.15s ease, color 0.15s ease;
}

.meta-chip:hover {
  border-color: var(--guaxi-text-secondary);
  color: var(--guaxi-text-primary);
}

.meta-key {
  opacity: 0.66;
}

.meta-value {
  color: var(--guaxi-text-primary);
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.meta-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--guaxi-text-secondary);
}

.meta-dot--explore { background: #6b8fb5; }
.meta-dot--code { background: #7a9e7e; }
.meta-dot--research { background: #a98a5c; }
.meta-dot--computer { background: #b46a6a; }

.meta-warning {
  font-size: 12px;
  color: var(--guaxi-syntax-tag);
}

.composer-running {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  margin-left: auto;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.composer-running-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: #b47d1a;
  animation: composer-pulse 1.6s ease-in-out infinite;
}

@keyframes composer-pulse {
  0%, 100% { opacity: 0.35; }
  50% { opacity: 1; }
}

.composer-box {
  display: flex;
  flex-direction: column;
  gap: 6px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  padding: 10px 12px 8px;
  background: var(--guaxi-panel-bg);
  transition: border-color 0.15s ease;
}

.composer-box:focus-within {
  border-color: var(--guaxi-text-secondary);
}

.composer-box--running {
  border-color: rgba(180, 125, 26, 0.5);
}

.composer-input {
  width: 100%;
  min-height: 44px;
  max-height: 190px;
  border: none;
  outline: none;
  resize: none;
  background: transparent;
  color: var(--guaxi-text-primary);
  font-family: inherit;
  font-size: 14.5px;
  line-height: 1.6;
}

.composer-input::placeholder {
  color: var(--guaxi-text-secondary);
  opacity: 0.7;
}

.composer-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
}

.composer-hint {
  margin-right: auto;
  font-size: 11.5px;
  color: var(--guaxi-text-secondary);
  opacity: 0.62;
}

.composer-btn {
  height: 26px;
  padding: 0 14px;
  border-radius: 6px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  font-size: 12.5px;
  cursor: pointer;
}

.composer-btn:disabled {
  opacity: 0.42;
  cursor: default;
}

.composer-btn--stop {
  border-color: rgba(180, 125, 26, 0.6);
  color: #b47d1a;
}

@media (prefers-reduced-motion: reduce) {
  .composer-running-dot { animation: none; opacity: 0.9; }
}
</style>
