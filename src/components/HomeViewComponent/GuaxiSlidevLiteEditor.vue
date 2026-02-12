<template>
  <div class="slidev-lite-editor">
    <div class="host-wrap" ref="hostRef">
      <textarea
        v-if="activePath"
        ref="textareaRef"
        class="lite-textarea"
        :value="draft"
        :disabled="blockedLargeFile || unsupported"
        @input="onInput"
        @blur="onBlur"
        @click="onCursorLike"
        @keyup="onCursorLike"
      ></textarea>

      <div v-if="blockedLargeFile" class="host-mask">
        文件过大，已阻止加载到编辑器以避免卡顿（{{ blockedInfo }}）
      </div>
      <div v-else-if="!activePath" class="host-mask">请选择一个 PPT 源文件以开始编辑</div>
      <div v-else-if="unsupported" class="host-mask">该文件不支持编辑</div>
    </div>

    <div class="status-bar" v-if="activePath">
      <div class="status-left">
        <span class="status-item" :title="activePath">{{ fileName }}</span>
        <span v-if="dirty" class="status-dirty">未保存</span>
      </div>
      <div class="status-right">
        <span class="status-item">Ln {{ cursor.lineNumber }}, Col {{ cursor.column }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, onMounted, onUnmounted, ref, watch, nextTick } from 'vue'
import { createLogger } from '../../utils/logger'

const MAX_EDITABLE_CHARS = 800_000

const logger = createLogger('GuaxiSlidevLiteEditor')

const props = defineProps({
  activePath: { type: String, default: '' },
  content: { type: String, default: '' },
  unsupported: { type: Boolean, default: false },
})

const emit = defineEmits(['draft-flush'])

const hostRef = ref(null)
const textareaRef = ref(null)

let lastLayoutLogAt = 0
const logLayout = (stage, extra = {}) => {
  const now = Date.now()
  if (now - lastLayoutLogAt < 250) return
  lastLayoutLogAt = now

  try {
    const hostEl = hostRef.value
    const wrapEl = hostEl?.parentElement || null
    const rect = hostEl?.getBoundingClientRect?.() || { width: -1, height: -1, top: -1, left: -1 }
    const wrapRect = wrapEl?.getBoundingClientRect?.() || { width: -1, height: -1, top: -1, left: -1 }

    const hostStyle = hostEl ? window.getComputedStyle(hostEl) : null
    const wrapStyle = wrapEl ? window.getComputedStyle(wrapEl) : null

    logger.info('[layout]', {
      stage,
      hostClient: { w: hostEl?.clientWidth ?? -1, h: hostEl?.clientHeight ?? -1 },
      hostRect: { w: Math.round(rect.width), h: Math.round(rect.height), top: Math.round(rect.top) },
      hostCss: hostStyle ? { display: hostStyle.display, position: hostStyle.position, height: hostStyle.height } : null,
      wrapClient: { w: wrapEl?.clientWidth ?? -1, h: wrapEl?.clientHeight ?? -1 },
      wrapRect: { w: Math.round(wrapRect.width), h: Math.round(wrapRect.height), top: Math.round(wrapRect.top) },
      wrapCss: wrapStyle ? { display: wrapStyle.display, position: wrapStyle.position, height: wrapStyle.height, flex: wrapStyle.flex } : null,
      viewport: { w: window.innerWidth, h: window.innerHeight },
      ...extra,
    })
  } catch (e) {
    logger.warn('[layout] log failed', e)
  }
}

let localDirty = false
let lastDirtyAt = 0
let lastDirtyLogAt = 0

const editorBusy = ref(false)
const blockedLargeFile = ref(false)
const blockedInfo = ref('')

const dirty = ref(false)
const cursor = ref({ lineNumber: 1, column: 1 })
const draft = ref('')

const markDirty = (reason) => {
  localDirty = true
  dirty.value = true

  const now = Date.now()
  lastDirtyAt = now
  if (now - lastDirtyLogAt > 1500) {
    lastDirtyLogAt = now
    logger.debug('[draft] marked dirty', { path: activePath.value, reason })
  }
}

const flushDraft = (path, tag) => {
  const p = normalizePath(path)
  if (!p) return

  if (!localDirty) {
    logger.debug('[draft] skip flush (clean)', { path: p, tag })
    return
  }

  const text = String(draft.value ?? '')
  emit('draft-flush', { path: p, content: text, tag })

  localDirty = false

  logger.info('[draft] flushed', { path: p, tag, len: text.length })
}

const normalizePath = (p) => String(p || '').replace(/\\/g, '/')

const activePath = computed(() => normalizePath(props.activePath || ''))

const fileName = computed(() => {
  const p = activePath.value
  if (!p) return ''
  const parts = p.split('/')
  return parts[parts.length - 1] || p
})

const setModelForPath = async (path, content) => {
  const p = normalizePath(path)
  if (!p) return

  blockedLargeFile.value = false
  blockedInfo.value = ''

  const nextContent = String(content ?? '')

  if (nextContent.length > MAX_EDITABLE_CHARS) {
    blockedLargeFile.value = true
    blockedInfo.value = `${nextContent.length} chars`
    logger.warn('[model] blocked large file', { path: p, len: nextContent.length, limit: MAX_EDITABLE_CHARS })
    return
  }

  draft.value = nextContent
  dirty.value = false
  localDirty = false
  logger.info('[model] set draft', { path: p, len: nextContent.length })
}

const updateCursorFromTextarea = () => {
  const el = textareaRef.value
  if (!el) return
  const pos = Math.max(0, el.selectionStart || 0)
  const text = String(el.value || '')
  let line = 1
  let col = 1
  for (let i = 0; i < pos; i += 1) {
    if (text[i] === '\n') {
      line += 1
      col = 1
    } else {
      col += 1
    }
  }
  cursor.value = { lineNumber: line, column: col }
}

const onInput = (e) => {
  const value = String(e?.target?.value ?? '')
  draft.value = value
  markDirty('typing')
}

const onBlur = () => {
  const p = activePath.value
  if (!p) return
  flushDraft(p, 'blur')
}

const onCursorLike = () => {
  updateCursorFromTextarea()
}

const insertContent = async (text) => {
  const p = activePath.value
  const el = textareaRef.value
  if (!p || !el) {
    logger.warn('[insert] failed: no active file/textarea', { path: p })
    return false
  }

  const t = String(text ?? '')
  const start = el.selectionStart ?? 0
  const end = el.selectionEnd ?? start
  const cur = String(draft.value ?? '')
  const next = `${cur.slice(0, start)}${t}${cur.slice(end)}`

  draft.value = next
  markDirty('insert')

  await nextTick()
  const caret = start + t.length
  el.focus()
  el.setSelectionRange(caret, caret)
  updateCursorFromTextarea()

  logger.info('[insert] ok', { path: p, len: t.length })
  return true
}

const getContent = () => {
  const p = activePath.value
  if (!p) return { ok: false, path: '', content: '' }
  const text = String(draft.value ?? '')
  return { ok: true, path: p, content: text }
}

const markSaved = (path) => {
  const p = normalizePath(path)
  if (!p || p !== activePath.value) {
    logger.debug('[save] markSaved skipped', { path: p, active: activePath.value })
    return
  }

  localDirty = false
  dirty.value = false
  logger.info('[save] markSaved', { path: p })
}

defineExpose({
  insertContent,
  getContent,
  markSaved
})

watch(
  () => activePath.value,
  async (p, prev) => {
    logger.info('[watch] activePath changed', { prev, next: p })
    logLayout('watch-activePath', { prev, next: p })
    if (!p) return
    if (prev && prev !== p) {
      flushDraft(prev, 'switch')
    }
    await setModelForPath(p, props.content)
    logLayout('after-setModelForPath', { path: p })
    await nextTick()
    updateCursorFromTextarea()
  },
  { immediate: true, flush: 'post' }
)

watch(
  () => props.content,
  async (next) => {
    const p = activePath.value
    if (!p) return
    if (localDirty) return
    const target = String(next ?? '')
    if (draft.value === target) return
    logger.info('[watch] external content update', { path: p, len: target.length })
    await setModelForPath(p, target)
    await nextTick()
    updateCursorFromTextarea()
  },
  { flush: 'post' }
)

onMounted(async () => {
  logger.info('[mount]')
  await nextTick()
  logLayout('mounted-nextTick')
})

onUnmounted(() => {
  logger.info('[unmount]')
  if (activePath.value) {
    flushDraft(activePath.value, 'unmount')
  }
})
</script>

<style scoped>
.slidev-lite-editor {
  flex: 1 1 auto;
  min-height: 0;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--guaxi-content-bg, transparent);
}


.host-wrap {
  flex: 1 1 auto;
  min-height: 0;
  min-width: 0;
  position: relative;
  overflow: hidden;
}

.host {
  position: absolute;
  inset: 0;
}

.lite-textarea {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  border: none;
  outline: none;
  resize: none;
  padding: 10px;
  box-sizing: border-box;
  font-size: 16px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
  background: transparent;
  color: var(--guaxi-text-primary, #e6e6e6);
}

.host-mask {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.25);
  color: #fff;
  font-size: 12px;
  user-select: none;
  padding: 12px;
  text-align: center;
}

.status-bar {
  flex: 0 0 auto;
  margin-top: auto;
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-top: 1px solid var(--statusBar-border, rgba(128, 128, 128, 0.25));
  background: var(--statusBar-background, rgba(255, 255, 255, 0.02));
  padding: 0 10px;
  box-sizing: border-box;
  font-size: 12px;
  color: var(--guaxi-text-secondary, #8a8f98);
}

.status-left,
.status-right {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.status-item {
  max-width: 520px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.status-dirty {
  color: #d97706;
}
</style>
