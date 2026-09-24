<template>
  <teleport to="body">
    <div
      v-if="visible"
      class="guaxi-kbctx-panel"
      :style="panelStyle"
      @mousedown.stop
      @click.stop
    >
      <div class="guaxi-kbctx-panel__header">
        <span class="guaxi-kbctx-panel__title">知识库相关上下文</span>
        <button class="guaxi-kbctx-panel__close" title="关闭" @click="close">×</button>
      </div>

      <div class="guaxi-kbctx-panel__query" :title="query">「{{ query }}」</div>

      <div v-if="loading" class="guaxi-kbctx-panel__hint">检索中…</div>
      <div v-else-if="degraded" class="guaxi-kbctx-panel__hint">向量服务未就绪,语义检索暂不可用(可在知识库里生成向量后重试)。</div>
      <div v-else-if="error" class="guaxi-kbctx-panel__hint">{{ error }}</div>

      <ul v-else class="guaxi-kbctx-panel__list">
        <li
          v-for="(c, i) in citations"
          :key="c.refId || i"
          class="guaxi-kbctx-panel__item"
          :title="c.path ? `${c.path}${c.line ? ':' + c.line : ''}` : ''"
          @click="onJump(c)"
        >
          <div class="guaxi-kbctx-panel__item-head">
            <span class="guaxi-kbctx-panel__label">{{ c.label || '(未命名)' }}</span>
            <span class="guaxi-kbctx-panel__channel">{{ channelLabel(c.channel) }}</span>
          </div>
          <div class="guaxi-kbctx-panel__snippet">{{ c.snippet }}</div>
          <div v-if="c.path" class="guaxi-kbctx-panel__path">{{ shortPath(c.path) }}{{ c.line ? ':' + c.line : '' }}</div>
        </li>
      </ul>
    </div>
  </teleport>
</template>

<!-- SCRIPT_PLACEHOLDER -->

<script setup>
import { ref, computed, watch } from 'vue'
import { useCognitionStore } from '../../stores/cognitionStore'

const props = defineProps({
  visible: { type: Boolean, default: false },
  anchor: { type: Object, default: () => ({ x: 0, y: 0 }) },
  query: { type: String, default: '' },
  kbName: { type: String, default: '' },
})
const emit = defineEmits(['update:visible', 'jump'])

const cognition = useCognitionStore()
const loading = ref(false)
const citations = ref([])
const error = ref('')
const degraded = ref(false)
// 竞态防护:连续划词时,晚到的响应用 token 比对丢弃(照抄 useSelectionTranslate 的 reqToken 模式)。
let reqToken = 0

const panelStyle = computed(() => {
  const vw = typeof window !== 'undefined' ? window.innerWidth : 1200
  const vh = typeof window !== 'undefined' ? window.innerHeight : 800
  const x = Math.max(8, Math.min(Number(props.anchor?.x ?? 0), vw - 380))
  const y = Math.max(8, Math.min(Number(props.anchor?.y ?? 0), vh - 160))
  return { left: `${x}px`, top: `${y}px` }
})

const channelLabel = (ch) => ({ semantic: '语义', graph: '图谱', episodic: '情节', percept: '感知' }[String(ch)] || String(ch || ''))
const shortPath = (p) => {
  const parts = String(p || '').split(/[\\/]/).filter(Boolean)
  return parts.length <= 2 ? String(p || '') : '…/' + parts.slice(-2).join('/')
}

const runRecall = async () => {
  const q = String(props.query || '').trim()
  if (!q) return
  const myToken = ++reqToken
  loading.value = true
  error.value = ''
  degraded.value = false
  citations.value = []
  try {
    try { await cognition.ensureEmbeddingReady() } catch (e) { /* 尽力而为,失败则由 degraded 反映 */ }
    const res = await cognition.recall(q, { kbName: props.kbName || null, k: 8 })
    if (myToken !== reqToken) return
    degraded.value = !!res?.degraded
    citations.value = Array.isArray(res?.citations) ? res.citations : []
    if (!citations.value.length && !degraded.value) error.value = '没有找到相关内容'
  } catch (e) {
    if (myToken !== reqToken) return
    error.value = '检索失败:' + String(e?.message || e)
  } finally {
    if (myToken === reqToken) loading.value = false
  }
}

watch(() => props.visible, (v) => { if (v) runRecall() })
watch(() => props.query, () => { if (props.visible) runRecall() })

const close = () => emit('update:visible', false)
const onJump = (c) => {
  const path = String(c?.path || '')
  if (!path) return
  const line = Number(c?.line || 0)
  emit('jump', { path, line: line > 0 ? line : 1 })
}
</script>

<!-- STYLE_PLACEHOLDER -->

<style scoped>
.guaxi-kbctx-panel {
  position: fixed;
  z-index: 3000;
  width: 360px;
  max-width: calc(100vw - 16px);
  max-height: 60vh;
  display: flex;
  flex-direction: column;
  background: var(--guaxi-content-bg, #fff);
  color: var(--guaxi-text-color, #1f2329);
  border: 1px solid var(--guaxi-border-color, #e0e0e6);
  border-radius: 8px;
  box-shadow: 0 8px 28px rgba(0, 0, 0, 0.18);
  padding: 10px;
  font-size: 13px;
}
.guaxi-kbctx-panel__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.guaxi-kbctx-panel__title { font-weight: 600; }
.guaxi-kbctx-panel__close {
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 18px;
  line-height: 1;
  color: inherit;
  opacity: 0.6;
}
.guaxi-kbctx-panel__close:hover { opacity: 1; }
.guaxi-kbctx-panel__query {
  margin: 6px 0;
  opacity: 0.7;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.guaxi-kbctx-panel__hint {
  padding: 12px 4px;
  opacity: 0.7;
  font-size: 12px;
}
.guaxi-kbctx-panel__list {
  list-style: none;
  margin: 0;
  padding: 0;
  overflow: auto;
}
.guaxi-kbctx-panel__item {
  padding: 8px;
  border-radius: 6px;
  cursor: pointer;
  border: 1px solid transparent;
}
.guaxi-kbctx-panel__item:hover {
  background: var(--guaxi-hover-bg, #f2f3f5);
  border-color: var(--guaxi-border-color, #e0e0e6);
}
.guaxi-kbctx-panel__item-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.guaxi-kbctx-panel__label {
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.guaxi-kbctx-panel__channel {
  flex: 0 0 auto;
  font-size: 11px;
  opacity: 0.6;
  border: 1px solid var(--guaxi-border-color, #e0e0e6);
  border-radius: 4px;
  padding: 0 5px;
}
.guaxi-kbctx-panel__snippet {
  margin-top: 4px;
  font-size: 12px;
  line-height: 1.5;
  opacity: 0.85;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.guaxi-kbctx-panel__path {
  margin-top: 4px;
  font-size: 11px;
  opacity: 0.5;
  font-family: 'Consolas', 'Menlo', monospace;
}
</style>


