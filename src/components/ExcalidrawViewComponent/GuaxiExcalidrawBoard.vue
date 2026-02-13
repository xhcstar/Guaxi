<template>
  <div class="board-root">
    <div class="board-toolbar">
      <n-space size="small">
        <n-button size="small" type="primary" @click="handleNewScene">新建</n-button>
        <n-button size="small" @click="handleOpenScene">打开…</n-button>
        <n-button size="small" @click="handleSaveScene">保存…</n-button>
        <n-button size="small" @click="handleExportPng">导出 PNG…</n-button>
        <n-button size="small" @click="handleExportSvg">导出 SVG…</n-button>
      </n-space>

      <div class="toolbar-right">
        <n-text depth="3" class="hint">
          自动保存：{{ autosaveStatus }}
        </n-text>
      </div>
    </div>

    <div class="board-canvas" ref="mountEl"></div>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useMessage, NButton, NSpace, NText } from 'naive-ui'
import React from 'react'
import { createRoot } from 'react-dom/client'
// 使用 Excalidraw 的 dev 构建，避免其生产包内联一份 React，导致 release 版本出现双 React (#168)
import { Excalidraw } from 'excalidraw-dev'
import 'excalidraw-dev-css'

import { createLogger } from '../../utils/logger'
import { useSettingsStore } from '../../stores/settingsStore'

const logger = createLogger('GuaxiExcalidrawBoard')
const message = useMessage()
const settingsStore = useSettingsStore()

const mountEl = ref(null)
const DIAG_KEY = '__guaxi_excalidraw_react_diag__'

const AUTOSAVE_KEY = 'guaxi:excalidraw:autosave:v1'
const autosaveStatus = ref('未写入')
const autosaveTimer = ref(null)

const reactRoot = ref(null)
const excalidrawApi = ref(null)
const renderVersion = ref(0)

const theme = computed(() => (settingsStore.appearanceTheme === 'dark' ? 'dark' : 'light'))
const FORCED_LANG_CODE = 'zh-CN'
const langCode = ref(FORCED_LANG_CODE)

function logReactDiagnostics(stage) {
  try {
    const g = window
    const prev = g[DIAG_KEY]
    const current = {
      react: React,
      reactVersion: React?.version || 'unknown',
      moduleUrl: import.meta.url,
      timestamp: new Date().toISOString(),
    }

    if (!prev) {
      g[DIAG_KEY] = current
      logger.info('[diag] react baseline', {
        stage,
        reactVersion: current.reactVersion,
        moduleUrl: current.moduleUrl,
        assetPath: g.EXCALIDRAW_ASSET_PATH || null,
      })
      return
    }

    logger.info('[diag] react compare', {
      stage,
      sameReactObject: prev.react === current.react,
      firstReactVersion: prev.reactVersion,
      currentReactVersion: current.reactVersion,
      firstModuleUrl: prev.moduleUrl,
      currentModuleUrl: current.moduleUrl,
      assetPath: g.EXCALIDRAW_ASSET_PATH || null,
    })
  } catch (e) {
    logger.warn('[diag] react diagnostics failed', e)
  }
}

class ExcalidrawErrorBoundary extends React.Component {
  constructor(props) {
    super(props)
    this.state = { hasError: false, error: null }
  }

  static getDerivedStateFromError(error) {
    return { hasError: true, error: error ?? null }
  }

  componentDidCatch(error, info) {
    try {
      this.props?.onError?.(error, info)
    } catch {
      // ignore
    }
  }

  render() {
    if (this.state.hasError) {
      const raw = this.state.error
      const details = raw
        ? String(raw?.stack || raw?.message || raw)
        : 'Unknown error'

      return React.createElement(
        'div',
        {
          style: {
            height: '100%',
            width: '100%',
            display: 'flex',
            flexDirection: 'column',
            gap: '10px',
            padding: '12px',
            boxSizing: 'border-box',
            background: 'var(--guaxi-content-bg)',
            color: 'var(--guaxi-text-primary)',
          },
        },
        React.createElement('div', { style: { fontSize: '14px', fontWeight: 600 } }, '白板渲染发生异常'),
        React.createElement(
          'div',
          { style: { fontSize: '12px', color: 'var(--guaxi-text-secondary)' } },
          '请打开开发者工具查看更完整的报错堆栈（包含触发异常的具体原因）。',
        ),
        React.createElement(
          'pre',
          {
            style: {
              margin: 0,
              padding: '10px',
              borderRadius: '8px',
              background: 'var(--guaxi-panel-bg)',
              border: '1px solid var(--guaxi-border)',
              overflow: 'auto',
              flex: '1 1 auto',
              minHeight: 0,
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-word',
            },
          },
          details,
        ),
        React.createElement(
          'button',
          {
            type: 'button',
            onClick: () => this.props?.onRetry?.(),
            style: {
              alignSelf: 'flex-start',
              padding: '6px 10px',
              borderRadius: '8px',
              border: '1px solid var(--guaxi-border)',
              background: 'var(--guaxi-panel-bg)',
              cursor: 'pointer',
            },
          },
          '重试渲染',
        ),
      )
    }

    return this.props?.children
  }
}

const lastScene = ref({
  elements: [],
  appState: null,
  files: null,
})

function safeJsonParse(raw) {
  try {
    return JSON.parse(raw)
  } catch (e) {
    return null
  }
}

function toSerializableAppState(appState) {
  const src = appState && typeof appState === 'object' ? appState : {}
  const cloned = { ...src }

  // Excalidraw 的 collaborators 在运行时通常是 Map。
  // Map 无法被 JSON.stringify 正确序列化，反序列化后会变成普通对象，导致内部调用 collaborators.forEach 直接崩。
  if ('collaborators' in cloned) {
    delete cloned.collaborators
  }

  return cloned
}

function toRuntimeAppState(appState) {
  const src = appState && typeof appState === 'object' ? appState : {}
  const cloned = { ...src }

  // 运行时保证 collaborators 为 Map（至少是空 Map），避免 Excalidraw 内部 forEach 报错。
  if (!(cloned.collaborators instanceof Map)) {
    cloned.collaborators = new Map()
  }

  // 强制中文界面，避免被历史缓存/导入场景覆盖
  cloned.langCode = FORCED_LANG_CODE

  return cloned
}

function scheduleAutosave() {
  if (autosaveTimer.value) {
    clearTimeout(autosaveTimer.value)
    autosaveTimer.value = null
  }

  autosaveTimer.value = setTimeout(() => {
    autosaveTimer.value = null
    try {
      const payload = {
        ts: new Date().toISOString(),
        elements: lastScene.value?.elements || [],
        appState: toSerializableAppState(lastScene.value?.appState || null),
        files: lastScene.value?.files || null,
      }

      const raw = JSON.stringify(payload)

      // localStorage 体积有限，这里做一个粗略保护
      if (raw.length > 4 * 1024 * 1024) {
        autosaveStatus.value = '数据过大，跳过'
        logger.warn('[autosave] skipped (too large)', { size: raw.length })
        return
      }

      localStorage.setItem(AUTOSAVE_KEY, raw)
      autosaveStatus.value = '已写入'
      logger.debug('[autosave] saved', { size: raw.length })
    } catch (e) {
      autosaveStatus.value = '写入失败'
      logger.error('[autosave] failed', e)
    }
  }, 800)
}

function applySceneToApi(scene) {
  try {
    if (!excalidrawApi.value) return false

    const elements = Array.isArray(scene?.elements) ? scene.elements : []
    const appState = toRuntimeAppState(scene?.appState)
    const files = scene?.files && typeof scene.files === 'object' ? scene.files : {}

    excalidrawApi.value.updateScene({
      elements,
      appState,
      files,
    })

    logger.info('[scene] applied to excalidraw', { elements: elements.length })
    return true
  } catch (e) {
    logger.error('[scene] apply failed', e)
    return false
  }
}

function readAutosave() {
  try {
    const raw = localStorage.getItem(AUTOSAVE_KEY)
    if (!raw) return null
    const data = safeJsonParse(raw)
    if (!data) return null
    return data
  } catch (e) {
    logger.warn('[autosave] read failed', e)
    return null
  }
}

async function writeTextFileWithDialog(defaultName, text, filters) {
  const { save } = await import('@tauri-apps/plugin-dialog')
  const { writeFile } = await import('@tauri-apps/plugin-fs')

  const filePath = await save({
    defaultPath: defaultName,
    filters,
  })
  if (!filePath) return null

  const buf = new TextEncoder().encode(text)
  await writeFile(filePath, buf)
  return String(filePath)
}

async function readTextFileWithDialog(filters) {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const { readFile } = await import('@tauri-apps/plugin-fs')

  const filePath = await open({
    multiple: false,
    filters,
  })
  if (!filePath) return null

  const bytes = await readFile(filePath)
  const text = new TextDecoder().decode(bytes)
  return { filePath: String(filePath), text }
}

function buildExportScene() {
  const elements = lastScene.value?.elements || []
  const appState = lastScene.value?.appState || {}
  const files = lastScene.value?.files || {}

  return {
    type: 'excalidraw',
    version: 2,
    source: 'guaxi',
    elements,
    appState: toSerializableAppState(appState),
    files,
    exportedAt: new Date().toISOString(),
  }
}

async function handleNewScene() {
  try {
    if (!excalidrawApi.value) return

    excalidrawApi.value.updateScene({
      elements: [],
      appState: {
        ...((lastScene.value && lastScene.value.appState) || {}),
      },
      files: {},
    })

    lastScene.value = { elements: [], appState: null, files: null }
    autosaveStatus.value = '未写入'
    message.success('已新建画布')
    logger.info('[ui] new scene')
  } catch (e) {
    logger.error('[ui] new scene failed', e)
    message.error('新建失败')
  }
}

async function handleOpenScene() {
  try {
    const res = await readTextFileWithDialog([
      { name: 'Excalidraw', extensions: ['excalidraw', 'json'] },
    ])
    if (!res) return

    const data = safeJsonParse(res.text)
    if (!data) {
      message.error('文件不是合法 JSON')
      return
    }

    const ok = applySceneToApi(data)
    if (!ok) {
      message.error('加载失败（无法应用到画布）')
      return
    }

    message.success('已加载')
    logger.info('[ui] open scene', { filePath: res.filePath })
  } catch (e) {
    logger.error('[ui] open scene failed', e)
    message.error('打开失败')
  }
}

async function handleSaveScene() {
  try {
    const payload = buildExportScene()
    const text = JSON.stringify(payload, null, 2)

    const filePath = await writeTextFileWithDialog('whiteboard.excalidraw', text, [
      { name: 'Excalidraw', extensions: ['excalidraw'] },
      { name: 'JSON', extensions: ['json'] },
    ])

    if (!filePath) return
    message.success('已保存')
    logger.info('[ui] save scene', { filePath })
  } catch (e) {
    logger.error('[ui] save scene failed', e)
    message.error('保存失败')
  }
}

async function exportWith(type) {
  try {
    if (!excalidrawApi.value) return

    const mod = await import('excalidraw-dev')
    const exportToBlob = mod?.exportToBlob

    if (typeof exportToBlob !== 'function') {
      message.warning('当前 Excalidraw 版本不支持导出接口，已跳过')
      logger.warn('[export] exportToBlob not found')
      return
    }

    const elements = excalidrawApi.value.getSceneElements()
    const appState = excalidrawApi.value.getAppState()
    const files = excalidrawApi.value.getFiles()

    const blob = await exportToBlob({
      elements,
      appState,
      files,
      mimeType: type === 'svg' ? 'image/svg+xml' : 'image/png',
    })

    const { save } = await import('@tauri-apps/plugin-dialog')
    const { writeFile } = await import('@tauri-apps/plugin-fs')

    const filePath = await save({
      defaultPath: type === 'svg' ? 'whiteboard.svg' : 'whiteboard.png',
      filters: [{ name: type.toUpperCase(), extensions: [type] }],
    })
    if (!filePath) return

    const buf = new Uint8Array(await blob.arrayBuffer())
    await writeFile(filePath, buf)

    message.success(`已导出 ${type.toUpperCase()}`)
    logger.info('[export] success', { type, filePath })
  } catch (e) {
    logger.error('[export] failed', e)
    message.error('导出失败')
  }
}

async function handleExportPng() {
  await exportWith('png')
}

async function handleExportSvg() {
  await exportWith('svg')
}

function renderReactApp(initialData) {
  if (!mountEl.value) return

  if (!window.EXCALIDRAW_ASSET_PATH) {
    window.EXCALIDRAW_ASSET_PATH = '/excalidraw/'
  }
  logReactDiagnostics('render')

  if (!reactRoot.value) {
    reactRoot.value = createRoot(mountEl.value)
  }

  const onChange = (elements, appState, files) => {
    lastScene.value = { elements, appState, files }
    autosaveStatus.value = '待写入'
    scheduleAutosave()
  }

  const onApi = (api) => {
    excalidrawApi.value = api
    try {
      const currentState = api.getAppState?.() || {}
      api.updateScene({
        appState: {
          ...currentState,
          langCode: FORCED_LANG_CODE,
        },
      })
    } catch (e) {
      logger.warn('[excalidraw] force lang failed', e)
    }
    logger.info('[excalidraw] api ready')
  }

  const onBoundaryError = (error, info) => {
    logger.error('[excalidraw] react render error', error, info)
  }

  const onBoundaryRetry = () => {
    try {
      renderVersion.value += 1
      logger.info('[excalidraw] retry render', { renderVersion: renderVersion.value })
      const initial = readAutosave()
      renderReactApp(initial)
    } catch (e) {
      logger.error('[excalidraw] retry render failed', e)
    }
  }

  const normalizedInitialData = initialData
    ? {
        ...initialData,
        appState: toRuntimeAppState(initialData?.appState),
      }
    : undefined

  if (initialData?.appState && !(initialData.appState?.collaborators instanceof Map)) {
    logger.debug('[excalidraw] initialData.appState.collaborators normalized to Map')
  }

  reactRoot.value.render(
    React.createElement(
      'div',
      { style: { height: '100%', width: '100%' } },
      React.createElement(
        ExcalidrawErrorBoundary,
        { key: renderVersion.value, onError: onBoundaryError, onRetry: onBoundaryRetry },
        React.createElement(Excalidraw, {
          theme: theme.value,
          langCode: langCode.value,
          initialData: normalizedInitialData,
          excalidrawAPI: onApi,
          onChange,
        }),
      ),
    ),
  )
}

onMounted(() => {
  try {
    logReactDiagnostics('mounted')
    const initial = readAutosave()
    if (initial) {
      autosaveStatus.value = '已恢复'
      logger.info('[autosave] restored', { ts: initial?.ts })
    }

    renderReactApp(initial)
  } catch (e) {
    logger.error('[mount] failed', e)
  }
})

watch(
  () => theme.value,
  () => {
    try {
      // 主题变化时重渲染，确保 Excalidraw 主题同步
      renderReactApp(null)
      logger.info('[theme] rerender', { theme: theme.value })
    } catch (e) {
      logger.warn('[theme] rerender failed', e)
    }
  },
)

onBeforeUnmount(() => {
  try {
    if (autosaveTimer.value) {
      clearTimeout(autosaveTimer.value)
      autosaveTimer.value = null
    }
    if (reactRoot.value) {
      reactRoot.value.unmount()
      reactRoot.value = null
    }
    excalidrawApi.value = null
  } catch (e) {
    logger.warn('[unmount] cleanup failed', e)
  }
})
</script>

<style scoped>
.board-root {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--guaxi-content-bg);
}

.board-toolbar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.hint {
  user-select: none;
}

.board-canvas {
  flex: 1 1 auto;
  min-height: 0;
  height: 100%;
  width: 100%;
  overflow: hidden;
  background: var(--guaxi-content-bg);
}
</style>
