<template>
  <div class="board-root">
    <div class="board-canvas" ref="mountEl"></div>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import React from 'react'
import ReactDOM from 'react-dom'
// 使用 Excalidraw 的 dev 构建，避免其生产包内联一份 React，导致 release 版本出现双 React (#168)
import { Excalidraw } from 'excalidraw-dev'
import 'excalidraw-dev-css'

import { createLogger } from '../../utils/logger'
import { useSettingsStore } from '../../stores/settingsStore'

const logger = createLogger('GuaxiExcalidrawBoard')
const settingsStore = useSettingsStore()

const mountEl = ref(null)
const DIAG_KEY = '__guaxi_excalidraw_react_diag__'

const AUTOSAVE_KEY = 'guaxi:excalidraw:autosave:v1'
const autosaveStatus = ref('未写入')
const autosaveTimer = ref(null)

const excalidrawApi = ref(null)
const renderVersion = ref(0)
const isUnmounted = ref(false)
const pendingApiFrame = ref(null)

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
  cloned.isLoading = false

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
function renderReactApp(initialData) {
  if (!mountEl.value) return

  if (!window.EXCALIDRAW_ASSET_PATH) {
    window.EXCALIDRAW_ASSET_PATH = '/excalidraw/'
  }
  logReactDiagnostics('render')

  const onChange = (elements, appState, files) => {
    lastScene.value = { elements, appState, files }
    autosaveStatus.value = '待写入'
    scheduleAutosave()
  }

  const onApi = (api) => {
    excalidrawApi.value = api

    if (isUnmounted.value) {
      logger.debug('[excalidraw] api ignored (unmounted)')
      return
    }

    // 延迟到下一帧再触发 updateScene，避免组件尚未挂载完成时触发 setState 警告
    if (pendingApiFrame.value) {
      cancelAnimationFrame(pendingApiFrame.value)
      pendingApiFrame.value = null
    }

    pendingApiFrame.value = requestAnimationFrame(() => {
      pendingApiFrame.value = null
      if (isUnmounted.value) return
      try {
        const currentState = api.getAppState?.() || {}
        api.updateScene({
          appState: {
            ...currentState,
            langCode: FORCED_LANG_CODE,
            isLoading: false,
          },
        })
        logger.info('[excalidraw] api ready', { clearedLoading: true })
      } catch (e) {
        logger.warn('[excalidraw] force lang failed', e)
      }
    })
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

  ReactDOM.render(
    React.createElement(
      'div',
      { style: { height: '100%', width: '100%' } },
      React.createElement(Excalidraw, {
        key: renderVersion.value,
        theme: theme.value,
        langCode: langCode.value,
        initialData: normalizedInitialData,
        excalidrawAPI: onApi,
        onChange,
      }),
    ),
    mountEl.value,
  )
}

onMounted(() => {
  try {
    isUnmounted.value = false
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
      if (excalidrawApi.value) {
        excalidrawApi.value.updateScene({
          appState: {
            theme: theme.value,
            langCode: FORCED_LANG_CODE,
            isLoading: false,
          },
        })
        logger.info('[theme] sync via api', { theme: theme.value })
        return
      }
      renderReactApp(null)
      logger.info('[theme] rerender', { theme: theme.value })
    } catch (e) {
      logger.warn('[theme] rerender failed', e)
    }
  },
)

onBeforeUnmount(() => {
  try {
    isUnmounted.value = true
    if (pendingApiFrame.value) {
      cancelAnimationFrame(pendingApiFrame.value)
      pendingApiFrame.value = null
    }
    if (autosaveTimer.value) {
      clearTimeout(autosaveTimer.value)
      autosaveTimer.value = null
    }
    if (mountEl.value) {
      ReactDOM.unmountComponentAtNode(mountEl.value)
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
