<template>
  <div class="slidev-lite-editor">
    <div class="host-wrap" ref="hostRef">
      <div v-show="activePath" ref="editorRef" class="lite-editor"></div>

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
import * as monaco from 'monaco-editor'
import 'monaco-editor/esm/vs/basic-languages/markdown/markdown.contribution'
import { createLogger } from '../../utils/logger'
import { useSettingsStore } from '../../stores/settingsStore'

const MAX_EDITABLE_CHARS = 800_000

const logger = createLogger('GuaxiSlidevLiteEditor')
const settingsStore = useSettingsStore()

const props = defineProps({
  activePath: { type: String, default: '' },
  content: { type: String, default: '' },
  unsupported: { type: Boolean, default: false },
})

const emit = defineEmits(['draft-flush', 'save-request'])

const hostRef = ref(null)
const editorRef = ref(null)

let editor = null
let model = null
let editorReady = false
let settingModel = false
let editorDisposables = []

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

const getMonacoTheme = () => (settingsStore.appearanceTheme === 'dark' ? 'vs-dark' : 'vs')

const applyMonacoTheme = () => {
  try {
    monaco.editor.setTheme(getMonacoTheme())
  } catch (e) {
    logger.warn('[theme] apply failed', e)
  }
}

const disposeMonaco = () => {
  editorDisposables.forEach((d) => d?.dispose?.())
  editorDisposables = []
  if (editor) {
    editor.dispose()
    editor = null
  }
  if (model) {
    model.dispose()
    model = null
  }
  editorReady = false
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

const detectLanguage = (p) => {
  const s = String(p || '').toLowerCase()
  if (s.endsWith('.md') || s.endsWith('.mdx')) return 'markdown'
  if (s.endsWith('.yml') || s.endsWith('.yaml')) return 'yaml'
  if (s.endsWith('.json')) return 'json'
  if (s.endsWith('.js')) return 'javascript'
  if (s.endsWith('.ts')) return 'typescript'
  if (s.endsWith('.vue')) return 'html'
  return 'plaintext'
}

const updateEditorOptions = () => {
  if (!editor) return
  const readOnly = blockedLargeFile.value || props.unsupported || !activePath.value
  editor.updateOptions({ readOnly })
}

const updateCursorFromEditor = () => {
  if (!editor) return
  const pos = editor.getPosition()
  if (!pos) return
  cursor.value = { lineNumber: pos.lineNumber, column: pos.column }
}

const ensureEditor = async () => {
  if (editorReady || !editorRef.value) return

  editor = monaco.editor.create(editorRef.value, {
    automaticLayout: true,
    readOnly: true,
    wordWrap: 'on',
    minimap: { enabled: false },
    scrollBeyondLastLine: false,
    fontSize: 16,
    fontFamily: "'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'",
    padding: { top: 8, bottom: 8 },
  })

  editorDisposables.push(
    editor.onDidChangeModelContent(() => {
      if (settingModel) return
      draft.value = editor.getValue()
      markDirty('typing')
    }),
    editor.onDidChangeCursorPosition(() => {
      updateCursorFromEditor()
    }),
    editor.onDidBlurEditorWidget(() => {
      onBlur()
    })
  )

  editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
    const p = activePath.value
    if (!p) {
      logger.warn('[save] hotkey ignored: no active file')
      return
    }
    emit('save-request', { path: p })
    logger.info('[save] hotkey', { path: p })
  })

  applyMonacoTheme()
  updateEditorOptions()
  editorReady = true
  logger.info('[editor] created')
}

const clearEditorModel = () => {
  if (!editor) return
  if (model) model.dispose()
  model = monaco.editor.createModel('', 'plaintext')
  editor.setModel(model)
  draft.value = ''
  dirty.value = false
  localDirty = false
}

const activePath = computed(() => normalizePath(props.activePath || ''))

const fileName = computed(() => {
  const p = activePath.value
  if (!p) return ''
  const parts = p.split('/')
  return parts[parts.length - 1] || p
})

// 绑定路径并刷新模型内容
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
    await ensureEditor()
    clearEditorModel()
    updateEditorOptions()
    return
  }

  await ensureEditor()
  if (!editor) return

  const lang = detectLanguage(p)
  if (model) model.dispose()
  model = monaco.editor.createModel(nextContent, lang, monaco.Uri.file(p))
  editor.setModel(model)

  draft.value = nextContent
  dirty.value = false
  localDirty = false
  updateEditorOptions()
  updateCursorFromEditor()
  logger.info('[model] set model', { path: p, len: nextContent.length, lang })
}

const onBlur = () => {
  const p = activePath.value
  if (!p) return
  flushDraft(p, 'blur')
}

const insertContent = async (text) => {
  const p = activePath.value
  if (!p) {
    logger.warn('[insert] failed: no active file', { path: p })
    return false
  }

  await ensureEditor()
  if (!editor || !model) {
    logger.warn('[insert] failed: editor not ready', { path: p })
    return false
  }

  const t = String(text ?? '')
  const selection = editor.getSelection()
  const pos = editor.getPosition() || new monaco.Position(1, 1)
  const range = selection || new monaco.Range(pos.lineNumber, pos.column, pos.lineNumber, pos.column)

  editor.executeEdits('insert-content', [
    { range, text: t, forceMoveMarkers: true }
  ])

  editor.focus()
  updateCursorFromEditor()

  logger.info('[insert] ok', { path: p, len: t.length })
  return true
}

const getContent = () => {
  const p = activePath.value
  if (!p || !model) return { ok: false, path: '', content: '' }
  const text = model.getValue()
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
    if (prev && prev !== p) {
      flushDraft(prev, 'switch')
    }
    if (!p) {
      await ensureEditor()
      clearEditorModel()
      updateEditorOptions()
      return
    }
    await setModelForPath(p, props.content)
    logLayout('after-setModelForPath', { path: p })
    await nextTick()
    updateCursorFromEditor()
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
    if (model && model.getValue() === target) return
    logger.info('[watch] external content update', { path: p, len: target.length })
    await setModelForPath(p, target)
    await nextTick()
    updateCursorFromEditor()
  },
  { flush: 'post' }
)

watch(
  () => settingsStore.appearanceTheme,
  () => {
    applyMonacoTheme()
  },
  { immediate: true }
)

watch(
  () => [activePath.value, props.unsupported, blockedLargeFile.value],
  () => {
    updateEditorOptions()
  }
)

onMounted(async () => {
  logger.info('[mount]')
  await ensureEditor()
  await nextTick()
  logLayout('mounted-nextTick')
})

onUnmounted(() => {
  logger.info('[unmount]')
  if (activePath.value) {
    flushDraft(activePath.value, 'unmount')
  }
  disposeMonaco()
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

.lite-editor {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
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
