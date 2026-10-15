<template>
  <div class="dictation-card" @mousedown.stop>
    <div class="dictation-card__header">
      <span class="dictation-card__title">
        <span class="dictation-card__rec-dot" :class="{ live: recording }" />
        实时语音听写
      </span>
      <span class="dictation-card__status" :class="statusTone">{{ statusText }}</span>
    </div>

    <div class="dictation-card__row">
      <label class="dictation-card__label">麦克风</label>
      <select
        class="dictation-card__select"
        :value="selectedDeviceId"
        :disabled="recording"
        @change="$emit('update:selectedDeviceId', $event.target.value)"
      >
        <option v-if="!devices.length" value="">未检测到设备</option>
        <option v-for="d in devices" :key="d.deviceId" :value="d.deviceId">{{ d.label }}</option>
      </select>
      <button class="dictation-card__icon-btn" :disabled="recording" title="刷新设备" @click="$emit('refresh-devices')">⟳</button>
    </div>

    <div class="dictation-card__row">
      <label class="dictation-card__label">灵敏度</label>
      <input
        class="dictation-card__slider"
        type="range"
        min="0"
        max="1"
        step="0.02"
        :value="sensitivity"
        @input="$emit('update:silenceThreshold', thresholdFromSensitivity(Number($event.target.value)))"
      />
      <span class="dictation-card__hint">{{ sensitivityLabel }}</span>
    </div>

    <div class="dictation-card__meta">
      <span class="dictation-card__chip">{{ providerLabel }}</span>
      <span v-if="modelLabel" class="dictation-card__chip dictation-card__chip--muted" :title="model">{{ modelLabel }}</span>
      <span v-if="pendingCount > 0" class="dictation-card__chip dictation-card__chip--busy">转写中 {{ pendingCount }}</span>
    </div>

    <div v-if="errorText" class="dictation-card__error">{{ errorText }}</div>

    <div class="dictation-card__actions">
      <button
        class="dictation-card__primary"
        :class="{ recording }"
        @click="$emit('toggle')"
      >
        {{ recording ? '停止录音' : '开始录音' }}
      </button>
    </div>

    <div class="dictation-card__tip">
      转写文本会按片段实时插入到光标处，并自动换行。
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'

const props = defineProps({
  recording: { type: Boolean, default: false },
  status: { type: String, default: 'idle' },
  errorText: { type: String, default: '' },
  devices: { type: Array, default: () => [] },
  selectedDeviceId: { type: String, default: '' },
  silenceThreshold: { type: Number, default: 0.012 },
  pendingCount: { type: Number, default: 0 },
  provider: { type: String, default: '' },
  model: { type: String, default: '' },
})

defineEmits([
  'toggle',
  'refresh-devices',
  'update:selectedDeviceId',
  'update:silenceThreshold',
])

const statusText = computed(() => {
  switch (props.status) {
    case 'listening': return '聆听中…'
    case 'speaking': return '识别语音…'
    case 'transcribing': return '转写中…'
    case 'error': return '出错'
    default: return '未开始'
  }
})

const statusTone = computed(() => {
  if (props.status === 'error') return 'error'
  if (props.status === 'speaking' || props.status === 'transcribing') return 'active'
  if (props.status === 'listening') return 'ok'
  return ''
})

// 滑条表示的是「灵敏度」，往右越灵敏。
//
// 底层存的是静音阈值（采集到的音量 RMS 低于它就算静音，见 useSpeechDictation 里的
// `rms >= silenceThreshold`），阈值越低越灵敏——两者方向相反。
// 早先滑条直接把阈值绑成 min=0.004 / max=0.05，于是往右拖反而越来越不灵敏。
// 这里在滑条边界上做一次反向映射：灵敏度 0 → 阈值上限（最不灵敏），1 → 下限（最灵敏）。
const THRESHOLD_MIN = 0.004
const THRESHOLD_MAX = 0.05

const clampThreshold = (value) => {
  const t = Number(value)
  if (!Number.isFinite(t)) return THRESHOLD_MAX
  return Math.min(Math.max(t, THRESHOLD_MIN), THRESHOLD_MAX)
}

const sensitivity = computed(
  () => (THRESHOLD_MAX - clampThreshold(props.silenceThreshold)) / (THRESHOLD_MAX - THRESHOLD_MIN),
)

const thresholdFromSensitivity = (value) => {
  const s = Math.min(Math.max(Number(value) || 0, 0), 1)
  // 阈值只是个粗略门槛，留 4 位小数就够，免得绑定值拖出一串浮点尾巴。
  return Number((THRESHOLD_MAX - s * (THRESHOLD_MAX - THRESHOLD_MIN)).toFixed(4))
}

const sensitivityLabel = computed(() => {
  // 按滑条位置给档位，这样文字和滑块位置始终对得上。
  const s = sensitivity.value
  if (s >= 0.67) return '高'
  if (s <= 0.33) return '低'
  return '中'
})

const providerLabel = computed(() => {
  const p = String(props.provider || '').toLowerCase()
  if (p === 'sherpa-onnx') return '本地 · sherpa-onnx'
  if (p === 'siliconflow') return '远程 · SiliconFlow'
  return props.provider || '未配置'
})

const modelLabel = computed(() => {
  const m = String(props.model || '').trim()
  if (!m) return ''
  return m.split(/[\\/]/).filter(Boolean).pop() || m
})
</script>

<style scoped>
.dictation-card {
  width: 280px;
  padding: 12px;
  border-radius: 12px;
  background: var(--guaxi-panel-bg, #21252b);
  border: 1px solid var(--guaxi-border, #3e4451);
  box-shadow: 0 10px 28px rgba(0, 0, 0, 0.28);
  color: var(--guaxi-text-primary, #ddd);
  font-size: 12.5px;
  user-select: none;
}

.dictation-card__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
}
.dictation-card__title {
  display: flex;
  align-items: center;
  gap: 7px;
  font-weight: 600;
}
.dictation-card__rec-dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: var(--guaxi-text-secondary, #888);
}
.dictation-card__rec-dot.live {
  background: #e06c75;
  animation: dictation-pulse 1.1s ease-in-out infinite;
}
@keyframes dictation-pulse {
  0%, 100% { opacity: 1; box-shadow: 0 0 0 0 rgba(224, 108, 117, 0.5); }
  50% { opacity: 0.6; box-shadow: 0 0 0 4px rgba(224, 108, 117, 0); }
}

.dictation-card__status { font-size: 11px; opacity: 0.7; }
.dictation-card__status.ok { color: #98c379; opacity: 1; }
.dictation-card__status.active { color: #61afef; opacity: 1; }
.dictation-card__status.error { color: #e06c75; opacity: 1; }

.dictation-card__row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 9px;
}
.dictation-card__label {
  flex: 0 0 44px;
  color: var(--guaxi-text-secondary, #9da5b4);
}
.dictation-card__select {
  flex: 1 1 auto;
  min-width: 0;
  background: var(--guaxi-content-bg, #1e1e1e);
  color: var(--guaxi-text-primary, #ddd);
  border: 1px solid var(--guaxi-border, #3e4451);
  border-radius: 6px;
  padding: 4px 6px;
  font-size: 12px;
}
.dictation-card__select:disabled { opacity: 0.6; }
.dictation-card__slider { flex: 1 1 auto; min-width: 0; accent-color: #61afef; }
.dictation-card__hint {
  flex: 0 0 auto;
  width: 18px;
  text-align: center;
  color: var(--guaxi-text-secondary, #9da5b4);
}

.dictation-card__icon-btn {
  flex: 0 0 auto;
  border: 1px solid var(--guaxi-border, #3e4451);
  background: transparent;
  color: var(--guaxi-text-primary, #ddd);
  border-radius: 6px;
  padding: 3px 7px;
  cursor: pointer;
}
.dictation-card__icon-btn:hover:not(:disabled) { border-color: #61afef; color: #61afef; }
.dictation-card__icon-btn:disabled { opacity: 0.4; cursor: not-allowed; }

.dictation-card__meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin: 4px 0 10px;
}
.dictation-card__chip {
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
  background: rgba(97, 175, 239, 0.16);
  color: #61afef;
}
.dictation-card__chip--muted {
  background: rgba(255, 255, 255, 0.06);
  color: var(--guaxi-text-secondary, #9da5b4);
  max-width: 130px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.dictation-card__chip--busy { background: rgba(229, 192, 123, 0.18); color: #e5c07b; }

.dictation-card__error {
  margin-bottom: 10px;
  padding: 6px 8px;
  border-radius: 6px;
  background: rgba(224, 108, 117, 0.12);
  border: 1px solid rgba(224, 108, 117, 0.4);
  color: #e06c75;
  font-size: 11.5px;
  max-height: 72px;
  overflow: auto;
}

.dictation-card__actions { margin-bottom: 8px; }
.dictation-card__primary {
  width: 100%;
  padding: 7px 10px;
  border-radius: 8px;
  border: 1px solid #61afef;
  background: rgba(97, 175, 239, 0.14);
  color: #61afef;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: background-color 0.15s, color 0.15s;
}
.dictation-card__primary:hover { background: rgba(97, 175, 239, 0.24); }
.dictation-card__primary.recording {
  border-color: #e06c75;
  background: rgba(224, 108, 117, 0.16);
  color: #e06c75;
}

.dictation-card__tip {
  font-size: 11px;
  color: var(--guaxi-text-secondary, #9da5b4);
  opacity: 0.8;
  line-height: 1.4;
}
</style>
