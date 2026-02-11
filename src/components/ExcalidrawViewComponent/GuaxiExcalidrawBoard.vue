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
import { Excalidraw } from '@excalidraw/excalidraw'
import '@excalidraw/excalidraw/index.css'

import { createLogger } from '../../utils/logger'
import { useSettingsStore } from '../../stores/settingsStore'

const logger = createLogger('GuaxiExcalidrawBoard')
const message = useMessage()
const settingsStore = useSettingsStore()

const mountEl = ref(null)

const AUTOSAVE_KEY = 'guaxi:excalidraw:autosave:v1'
const autosaveStatus = ref('未写入')
const autosaveTimer = ref(null)

const reactRoot = ref(null)
const excalidrawApi = ref(null)

const theme = computed(() => (settingsStore.appearanceTheme === 'dark' ? 'dark' : 'light'))

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
        appState: lastScene.value?.appState || null,
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
    const appState = scene?.appState && typeof scene.appState === 'object' ? scene.appState : {}
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
    appState,
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

    const mod = await import('@excalidraw/excalidraw')
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
    logger.info('[excalidraw] api ready')
  }

  reactRoot.value.render(
    React.createElement(
      'div',
      { style: { height: '100%', width: '100%' } },
      React.createElement(Excalidraw, {
        theme: theme.value,
        initialData: initialData || undefined,
        excalidrawAPI: onApi,
        onChange,
      }),
    ),
  )
}

onMounted(() => {
  try {
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