<template>
  <div class="slidev-lite-editor">
    <div class="host-wrap">
      <div ref="hostRef" class="host"></div>

      <div v-if="editorBusy" class="host-mask">正在初始化编辑器...</div>
      <div v-else-if="blockedLargeFile" class="host-mask">
        文件过大，已阻止加载到编辑器以避免卡顿（{{ blockedInfo }}）
      </div>
      <div v-else-if="!activePath" class="host-mask">请选择一个 PPT 源文件以开始编辑</div>
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
// 说明：这是一个“仅给 Slidev 窗口使用”的独立编辑器组件。
// - 不复用 GuaxiEditor / editorStore，避免影响主界面编辑器行为
// - 仍然复用 monaco-editor，但数据来源改为 slidevEditorStore（不与主窗口共享 tab/编辑状态）
// - 对外暴露 insertContent(text) 供知识库素材插入
import { computed, onMounted, onUnmounted, ref, watch, nextTick } from 'vue'
import * as monaco from 'monaco-editor'
import { useSlidevEditorStore } from '../../stores/slidevEditorStore'
import { createLogger } from '../../utils/logger'

const MAX_EDITABLE_CHARS = 800_000

const logger = createLogger('GuaxiSlidevLiteEditor')
const slidevEditorStore = useSlidevEditorStore()

const hostRef = ref(null)

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

const editor = ref(null)
let model = null
let modelPathKey = ''
let modelSwitchSeq = 0

let applyingModelUpdate = false
let syncTimer = 0
let pendingSync = { path: '', content: '' }
let lastSyncLogAt = 0

let resizeObserver = null
let prewarmTimer = 0
let prewarmIdleId = 0

const editorBusy = ref(false)
const blockedLargeFile = ref(false)
const blockedInfo = ref('')

const dirty = ref(false)
const cursor = ref({ lineNumber: 1, column: 1 })

const normalizePath = (p) => String(p || '').replace(/\\/g, '/')

const activePath = computed(() => {
  const p = slidevEditorStore.activeTab || slidevEditorStore.selectedFile?.path || ''
  return normalizePath(p)
})

const fileName = computed(() => {
  const p = activePath.value
  if (!p) return ''
  const parts = p.split('/')
  return parts[parts.length - 1] || p
})

const getTabContent = (path) => {
  const p = normalizePath(path)
  const tab = (Array.isArray(slidevEditorStore.openTabs) ? slidevEditorStore.openTabs : []).find(
    (t) => normalizePath(t?.path) === p
  )
  if (tab) return String(tab.content ?? '')
  if (normalizePath(slidevEditorStore.selectedFile?.path) === p) return String(slidevEditorStore.selectedFile?.content ?? '')
  return ''
}

const getLanguageForPath = (path) => {
  const p = String(path || '').toLowerCase()
  if (p.endsWith('.md') || p.endsWith('.markdown')) return 'markdown'
  if (p.endsWith('.yaml') || p.endsWith('.yml')) return 'yaml'
  if (p.endsWith('.json')) return 'json'
  if (p.endsWith('.ts')) return 'typescript'
  if (p.endsWith('.js')) return 'javascript'
  if (p.endsWith('.css')) return 'css'
  if (p.endsWith('.html') || p.endsWith('.htm')) return 'html'
  return 'plaintext'
}

const buildModelUri = (path) => {
  const p = normalizePath(path)
  // 这里不要用 Uri.file，避免 Windows 盘符导致差异；统一使用 file:/// URL
  const encoded = encodeURI(p.startsWith('/') ? p : `/${p}`)
  return monaco.Uri.parse(`file://${encoded}`)
}

const ensureEditor = async (initialValue = '', language = 'plaintext') => {
  if (editor.value || !hostRef.value) return

  const host = hostRef.value
  const t0 = performance.now()

  logger.info('[init] ensureEditor start', {
    hostW: host?.clientWidth ?? -1,
    hostH: host?.clientHeight ?? -1,
    language,
    len: typeof initialValue === 'string' ? initialValue.length : -1,
  })

  while (host.firstChild) host.removeChild(host.firstChild)

  let watchdog = 0
  watchdog = window.setTimeout(() => {
    logger.warn('[init] watchdog: monaco create may be blocking UI')
  }, 1500)

  try {
    logger.info('[init] monaco.editor.create start')
    logLayout('before-editor-create', { language })

    editor.value = monaco.editor.create(host, {
      value: String(initialValue ?? ''),
      language: String(language || 'plaintext'),
      automaticLayout: false,
      wordWrap: 'on',
      minimap: { enabled: false },
      scrollBeyondLastLine: false,
      fontSize: 16,
      fontFamily: "'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'",
      fontLigatures: true,
      padding: { top: 10, bottom: 10 },

      // 说明：保持最基础的 Monaco，不开启会触发额外 worker/语言服务的 UI 功能
      quickSuggestions: false,
      suggestOnTriggerCharacters: false,
      parameterHints: { enabled: false },
      hover: { enabled: false },
      lightbulb: { enabled: false },
      codeLens: false,
      folding: false,
      links: false,
      formatOnType: false,
      formatOnPaste: false,
      renderValidationDecorations: 'off',
    })

    model = editor.value?.getModel?.() || null
    logger.info('[init] monaco.editor.create done', { costMs: Math.round(performance.now() - t0) })
    logLayout('after-editor-create', { language })
  } finally {
    if (watchdog) window.clearTimeout(watchdog)
  }

  try {
    resizeObserver = new ResizeObserver(() => {
      const ed = editor.value
      if (!ed || !hostRef.value) return
      const w = hostRef.value.clientWidth
      const h = hostRef.value.clientHeight
      if (w > 0 && h > 0) ed.layout({ width: w, height: h })
    })
    resizeObserver.observe(host)
  } catch (e) {
    logger.warn('[init] ResizeObserver not available', e)
  }

  requestAnimationFrame(() => {
    try {
      const ed = editor.value
      if (!ed || !hostRef.value) return
      const w = hostRef.value.clientWidth
      const h = hostRef.value.clientHeight
      logger.info('[init] layout tick', { w, h })
      logLayout('layout-tick', { w, h })
      if (w > 0 && h > 0) ed.layout({ width: w, height: h })
    } catch (e) {
      logger.warn('[init] layout tick failed', e)
    }
  })

  editor.value.onDidChangeCursorPosition((e) => {
    cursor.value = { lineNumber: e.position.lineNumber, column: e.position.column }
  })

  const applyToStoreNoPreview = (path, content) => {
    const p = normalizePath(path)
    if (!p) return

    // 说明：编辑器内部会高频触发内容变更，这里只做状态同步，不做额外 IO
    slidevEditorStore.applyDraft(p, content)

    const tab = (Array.isArray(slidevEditorStore.openTabs) ? slidevEditorStore.openTabs : []).find(
      (t) => normalizePath(t?.path) === p
    )
    dirty.value = !!tab?.dirty
  }

  const scheduleSyncNoPreview = (path, content) => {
    pendingSync = { path, content }
    if (syncTimer) return

    syncTimer = window.setTimeout(() => {
      syncTimer = 0
      applyToStoreNoPreview(pendingSync.path, pendingSync.content)

      const now = Date.now()
      if (now - lastSyncLogAt > 1500) {
        lastSyncLogAt = now
        logger.debug('[sync] applied (no preview)', { path: pendingSync.path, len: pendingSync.content?.length ?? 0 })
      }
    }, 120)
  }

  editor.value.onDidChangeModelContent(() => {
    const p = activePath.value
    if (!p || !editor.value) return
    if (applyingModelUpdate) return

    const t0 = performance.now()
    const value = editor.value.getValue() ?? ''
    const costMs = Math.round(performance.now() - t0)
    if (costMs >= 30) {
      logger.warn('[sync] getValue slow', { path: p, costMs, len: value.length })
    }

    scheduleSyncNoPreview(p, value)
  })
}

const setModelForPath = async (path) => {
  const p = normalizePath(path)
  if (!p) return
  const seq = ++modelSwitchSeq

  editorBusy.value = true
  blockedLargeFile.value = false
  blockedInfo.value = ''

  const start = performance.now()
  let watchdog = 0

  try {
    logger.info('[model] setModelForPath start', { seq, path: p })

    watchdog = window.setTimeout(() => {
      logger.warn('[model] watchdog: setModelForPath may be blocking UI', {
        seq,
        path: p,
        costMs: Math.round(performance.now() - start),
      })
    }, 1500)

    const tEnsure0 = performance.now()
    await ensureEditor()
    logger.info('[model] ensureEditor done', { seq, path: p, costMs: Math.round(performance.now() - tEnsure0) })

    const tTick0 = performance.now()
    await nextTick()
    logger.info('[model] nextTick done', { seq, path: p, costMs: Math.round(performance.now() - tTick0) })

    if (!editor.value) {
      logger.warn('[model] editor not ready', { seq, path: p })
      return
    }

    const tContent0 = performance.now()
    const nextContent = getTabContent(p)
    logger.info('[model] getTabContent done', {
      seq,
      path: p,
      len: nextContent.length,
      costMs: Math.round(performance.now() - tContent0)
    })

    const language = getLanguageForPath(p)

    if (seq !== modelSwitchSeq) {
      logger.info('[model] aborted by newer seq', { seq, path: p, activeSeq: modelSwitchSeq })
      return
    }

    if (nextContent.length > MAX_EDITABLE_CHARS) {
      blockedLargeFile.value = true
      blockedInfo.value = `${nextContent.length} chars`
      logger.warn('[model] blocked large file', { path: p, len: nextContent.length, limit: MAX_EDITABLE_CHARS })
      return
    }

    // 如果还是同一个文件，尽量不重建 model，只同步内容
    if (model && modelPathKey === p) {
      applyingModelUpdate = true
      try {
        const tSame0 = performance.now()
        const cur = model.getValue()
        const readCurCost = Math.round(performance.now() - tSame0)

        if (cur !== nextContent) {
          const tSet0 = performance.now()
          logger.info('[model] setValue start (same file)', { seq, path: p, len: nextContent.length })
          model.setValue(nextContent)
          logger.info('[model] setValue done (same file)', { seq, path: p, costMs: Math.round(performance.now() - tSet0) })
        }

        const tLang0 = performance.now()
        monaco.editor.setModelLanguage(model, language)
        logger.info('[model] setLanguage done (same file)', { seq, path: p, costMs: Math.round(performance.now() - tLang0), readCurCostMs: readCurCost })
      } finally {
        applyingModelUpdate = false
      }

      dirty.value = false
      slidevEditorStore.markClean(p)
      return
    }

    // 说明：已定位到 setModel / setValue 在当前 Slidev 窗口环境会卡死。
    // 这里采用“最基础/最稳”的方案：切换文件时直接销毁并重建 editor（使用初始 value 注入内容）。
    // 代价：切换文件会比复用模型慢，但优先保证不卡死可编辑。

    const tRecreate0 = performance.now()
    logger.warn('[model] recreate editor (workaround)', { seq, path: p, len: nextContent.length })

    applyingModelUpdate = true
    try {
      try {
        if (editor.value) {
          const m = editor.value.getModel?.()
          if (m) {
            try { m.dispose() } catch (e) { logger.warn('[model] dispose model failed', e) }
          }
          try { editor.value.dispose() } catch (e) { logger.warn('[model] dispose editor failed', e) }
        }
      } finally {
        editor.value = null
        model = null
        modelPathKey = ''
      }

      await nextTick()
      await ensureEditor(nextContent, 'plaintext')
      await nextTick()

      modelPathKey = p
      dirty.value = false
      slidevEditorStore.markClean(p)

      logger.info('[model] recreate done', { seq, path: p, costMs: Math.round(performance.now() - tRecreate0) })
    } finally {
      applyingModelUpdate = false
    }

    logger.info('[model] switched (recreate editor)', { seq, path: p, len: nextContent.length })
  } finally {
    if (watchdog) window.clearTimeout(watchdog)
    editorBusy.value = false
    logger.info('[model] setModelForPath done', { seq, path: p, costMs: Math.round(performance.now() - start) })
  }
}

const insertContent = async (text) => {
  const p = activePath.value
  if (!p || !editor.value) {
    logger.warn('[insert] failed: no active file/editor', { path: p })
    return false
  }

  const ed = editor.value
  const t = String(text ?? '')
  const selection = ed.getSelection()
  const range = selection || new monaco.Range(1, 1, 1, 1)

  ed.executeEdits('guaxi-slidev-insert', [
    {
      range,
      text: t,
      forceMoveMarkers: true
    }
  ])
  ed.focus()

  logger.info('[insert] ok', { path: p, len: t.length })
  return true
}

defineExpose({
  insertContent
})

watch(
  () => activePath.value,
  async (p, prev) => {
    logger.info('[watch] activePath changed', { prev, next: p })
    logLayout('watch-activePath', { prev, next: p })
    if (!p) return
    await setModelForPath(p)
    logLayout('after-setModelForPath', { path: p })
  },
  { immediate: true, flush: 'post' }
)

onMounted(async () => {
  logger.info('[mount]')
  await nextTick()
  logLayout('mounted-nextTick')

  const runPrewarm = async () => {
    const start = performance.now()
    editorBusy.value = true
    try {
      await ensureEditor()
      logger.info('[prewarm] editor ready', { costMs: Math.round(performance.now() - start) })
    } catch (e) {
      logger.warn('[prewarm] failed', e)
    } finally {
      editorBusy.value = false
    }
  }

  if (typeof window.requestIdleCallback === 'function') {
    prewarmIdleId = window.requestIdleCallback(() => {
      prewarmIdleId = 0
      runPrewarm()
    }, { timeout: 1200 })
  } else {
    prewarmTimer = window.setTimeout(() => {
      prewarmTimer = 0
      runPrewarm()
    }, 60)
  }
})

onUnmounted(() => {
  logger.info('[unmount] disposing editor/model')

  if (prewarmIdleId && typeof window.cancelIdleCallback === 'function') {
    window.cancelIdleCallback(prewarmIdleId)
    prewarmIdleId = 0
  }
  if (prewarmTimer) {
    clearTimeout(prewarmTimer)
    prewarmTimer = 0
  }

  if (syncTimer) {
    clearTimeout(syncTimer)
    syncTimer = 0
  }

  try {
    resizeObserver?.disconnect?.()
  } catch (e) {
    logger.warn('[unmount] resizeObserver disconnect failed', e)
  } finally {
    resizeObserver = null
  }

  try {
    if (model) model.dispose()
  } catch (e) {
    logger.warn('[unmount] model dispose failed', e)
  } finally {
    model = null
    modelPathKey = ''
  }

  try {
    if (editor.value) editor.value.dispose()
  } catch (e) {
    logger.warn('[unmount] editor dispose failed', e)
  } finally {
    editor.value = null
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
