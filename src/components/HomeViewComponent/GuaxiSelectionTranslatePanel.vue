<template>
  <div ref="panelRef" class="pot-selection-translate-panel" :style="{ left: `${pos.left}px`, top: `${pos.top}px` }"
    @mousedown.stop @click.stop>
    <div class="panel-header" @mousedown.prevent="onHeaderMouseDown">
      <div class="panel-header-left">
        <n-button text size="tiny" :title="pinned ? '取消固定' : '固定'" @click.stop="emit('toggle-pin')">
          <Icon :icon="pinned ? pinOffIcon : pinIcon" width="16" height="16" />
        </n-button>
        <div class="panel-title">划词翻译</div>
      </div>

      <div class="panel-header-right">
        <n-button text size="tiny" title="重新翻译" :disabled="busy" @click.stop="emit('translate')">
          <Icon :icon="rotateCwIcon" width="16" height="16" />
        </n-button>
        <n-button text size="tiny" title="复制译文" :disabled="busy || !primaryText" @click.stop="emit('copy', { text: primaryText, engine: primaryEngine })">
          <Icon :icon="copyIcon" width="16" height="16" />
        </n-button>
        <n-button text size="tiny" title="关闭" @click.stop="emit('close')">
          <Icon :icon="xIcon" width="16" height="16" />
        </n-button>
      </div>
    </div>

    <div class="panel-body">
      <div class="panel-langs" @mousedown.stop @click.stop>
        <n-select v-model:value="fromLangProxy" size="tiny" :options="langOptions" :disabled="busy" style="width: 130px" />
        <n-button text size="tiny" title="交换语言" :disabled="busy" @click.stop="emit('swap-langs')">
          <Icon :icon="swapIcon" width="16" height="16" />
        </n-button>
        <n-select v-model:value="toLangProxy" size="tiny" :options="langOptions" :disabled="busy" style="width: 130px" />
      </div>

      <div class="panel-section">
        <div class="section-label">原文</div>
        <div class="section-content section-raw">{{ raw || '—' }}</div>
      </div>

      <div class="panel-divider"></div>

      <div class="panel-section">
        <div class="section-label">译文</div>

        <div v-if="busy" class="section-state">
          <n-spin size="small" />
          <span>正在翻译...</span>
        </div>

        <div v-else-if="error" class="section-content section-error">{{ error }}</div>

        <div v-else class="engine-list">
          <div v-for="it in items" :key="it.engine" class="engine-item">
            <div class="engine-item-header">
              <div class="engine-name">{{ it.engine }}</div>
              <n-button text size="tiny" title="复制" :disabled="!it.success || !it.text"
                @click.stop="emit('copy', { text: it.text || '', engine: it.engine })">
                <Icon :icon="copyIcon" width="16" height="16" />
              </n-button>
            </div>
            <div v-if="!it.success" class="engine-body engine-error">{{ it.error || '翻译失败' }}</div>
            <div v-else class="engine-body engine-text">{{ it.text || '—' }}</div>
          </div>

          <div v-if="!items || items.length === 0" class="section-content section-result">{{ result || '—' }}</div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, nextTick, watch, onUnmounted, computed } from 'vue'
import { NButton, NSpin, NSelect } from 'naive-ui'
import { Icon } from '@iconify/vue'
import pinIcon from '@iconify-icons/lucide/pin'
import pinOffIcon from '@iconify-icons/lucide/pin-off'
import copyIcon from '@iconify-icons/lucide/copy'
import rotateCwIcon from '@iconify-icons/lucide/rotate-cw'
import xIcon from '@iconify-icons/lucide/x'
import swapIcon from '@iconify-icons/lucide/arrow-left-right'

const props = defineProps({
  pos: { type: Object, default: () => ({ left: 8, top: 8 }) },
  raw: { type: String, default: '' },
  result: { type: String, default: '' },
  error: { type: String, default: '' },
  busy: { type: Boolean, default: false },
  pinned: { type: Boolean, default: false },
  items: { type: Array, default: () => [] },
  fromLang: { type: String, default: 'auto' },
  toLang: { type: String, default: 'zh' },
  langOptions: { type: Array, default: () => [] }
})

const emit = defineEmits([
  'close',
  'translate',
  'copy',
  'toggle-pin',
  'swap-langs',
  'update:pos',
  'update:fromLang',
  'update:toLang'
])

const fromLangProxy = computed({
  get: () => props.fromLang,
  set: (v) => emit('update:fromLang', v)
})

const toLangProxy = computed({
  get: () => props.toLang,
  set: (v) => emit('update:toLang', v)
})

const primaryItem = computed(() => {
  const list = Array.isArray(props.items) ? props.items : []
  return list.find((it) => it && it.success && String(it.text || '').trim()) || null
})

const primaryText = computed(() => {
  const it = primaryItem.value
  if (it && it.success) return String(it.text || '')
  return String(props.result || '')
})

const primaryEngine = computed(() => {
  const it = primaryItem.value
  if (it && it.engine) return String(it.engine || '')
  return ''
})

const panelRef = ref(null)

let dragging = false
let startX = 0
let startY = 0
let startLeft = 0
let startTop = 0

const clampToParent = (left, top) => {
  const el = panelRef.value
  const parent = el?.offsetParent
  const pw = parent?.clientWidth ?? window.innerWidth
  const ph = parent?.clientHeight ?? window.innerHeight

  const w = el?.offsetWidth ?? 360
  const h = el?.offsetHeight ?? 220

  const pad = 8
  const maxLeft = Math.max(pad, pw - w - pad)
  const maxTop = Math.max(pad, ph - h - pad)

  return {
    left: Math.min(Math.max(pad, left), maxLeft),
    top: Math.min(Math.max(pad, top), maxTop)
  }
}

const updatePos = (left, top) => {
  const p = clampToParent(left, top)
  emit('update:pos', p)
}

const onMouseMove = (e) => {
  if (!dragging) return
  const dx = e.clientX - startX
  const dy = e.clientY - startY
  updatePos(startLeft + dx, startTop + dy)
}

const onMouseUp = () => {
  if (!dragging) return
  dragging = false
  window.removeEventListener('mousemove', onMouseMove, true)
  window.removeEventListener('mouseup', onMouseUp, true)
}

const onHeaderMouseDown = (e) => {
  if (e.button !== 0) return

  dragging = true
  startX = e.clientX
  startY = e.clientY
  startLeft = Number(props.pos?.left ?? 0) || 0
  startTop = Number(props.pos?.top ?? 0) || 0

  window.addEventListener('mousemove', onMouseMove, true)
  window.addEventListener('mouseup', onMouseUp, true)
}

watch(
  () => props.pos,
  () => {
    nextTick(() => {
      updatePos(Number(props.pos?.left ?? 8), Number(props.pos?.top ?? 8))
    })
  },
  { deep: true }
)

onUnmounted(() => {
  onMouseUp()
})
</script>

<style scoped>
.pot-selection-translate-panel {
  position: absolute;
  z-index: 20;
  width: 360px;
  max-width: min(420px, calc(100vw - 24px));
  border-radius: 12px;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  box-shadow: 0 10px 26px rgba(0, 0, 0, 0.16);
  overflow: hidden;
  user-select: none;
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 8px;
  background: var(--guaxi-header-bg);
  border-bottom: 1px solid var(--guaxi-border);
  cursor: grab;
}

.panel-header:active {
  cursor: grabbing;
}

.panel-header-left,
.panel-header-right {
  display: flex;
  align-items: center;
  gap: 6px;
}

.panel-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
  opacity: 0.95;
}

.panel-body {
  padding: 10px 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.panel-langs {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
}

.panel-section {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.engine-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.engine-item {
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  overflow: hidden;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 92%, transparent);
}

.engine-item-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 8px;
  background: color-mix(in srgb, var(--guaxi-header-bg) 80%, transparent);
  border-bottom: 1px solid var(--guaxi-border);
}

.engine-name {
  font-size: 12px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
  opacity: 0.9;
}

.engine-body {
  padding: 8px 10px;
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
}

.engine-text {
  color: var(--guaxi-text-primary);
}

.engine-error {
  color: #ff6b6b;
  background: rgba(255, 107, 107, 0.06);
}

.section-label {
  font-size: 12px;
  color: var(--guaxi-text-primary);
  opacity: 0.75;
}

.section-content {
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
  padding: 8px 10px;
  border-radius: 10px;
  border: 1px solid var(--guaxi-border);
}

.section-raw {
  background: color-mix(in srgb, var(--guaxi-header-bg) 85%, transparent);
}

.section-result {
  background: color-mix(in srgb, var(--guaxi-panel-bg) 90%, transparent);
}

.section-error {
  background: rgba(255, 107, 107, 0.08);
  border-color: rgba(255, 107, 107, 0.35);
  color: #ff6b6b;
}

.panel-divider {
  height: 1px;
  background: var(--guaxi-border);
  opacity: 0.8;
}

.section-state {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--guaxi-text-primary);
  opacity: 0.8;
  padding: 2px 2px 0;
}
</style>