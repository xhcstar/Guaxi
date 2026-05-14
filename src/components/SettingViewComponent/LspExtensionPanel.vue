<template>
  <div class="lsp-extension-panel" :class="{ 'is-dark': isDarkTheme }">
    <div class="lsp-extension-panel__header">
      <div class="lsp-extension-panel__hero">
        <h3 class="lsp-extension-panel__title">LSP 扩展架构</h3>
        <p class="lsp-extension-panel__description">
          这里管理 VSCode Marketplace / Open VSX / 本地 VSIX 语言扩展
        </p>
      </div>
      <n-space class="lsp-extension-panel__header-actions" wrap>
        <n-button size="small" :disabled="loading" @click="refreshExtensions">刷新状态</n-button>
        <n-button size="small" :loading="installingRecommended" :disabled="loading" @click="handleInstallRecommendedExtensions">
          安装推荐扩展
        </n-button>
        <n-button size="small" :disabled="loading" @click="handleImportDirectory">导入扩展目录</n-button>
        <n-button size="small" type="primary" :disabled="loading" @click="handleImportVsix">导入 VSIX</n-button>
      </n-space>
    </div>

    <div class="lsp-extension-panel__search">
      <n-select
        v-model:value="searchSource"
        class="lsp-extension-panel__source"
        :options="searchSourceOptions"
        :disabled="loading || searching"
      />
      <n-input
        v-model:value="searchQuery"
        placeholder="搜索语言扩展，例如 pyright / lua / marksman / html"
        :disabled="loading || searching"
        @keydown.enter.prevent="handleSearch"
      />
      <n-button
        class="lsp-extension-panel__search-btn"
        type="primary"
        :loading="searching"
        :disabled="loading"
        @click="handleSearch"
      >
        搜索扩展
      </n-button>
    </div>

    <section v-if="searchResults.length" class="lsp-extension-panel__section">
      <div class="lsp-extension-panel__section-title">{{ searchSourceLabel }} 搜索结果</div>
      <div class="lsp-extension-panel__list">
        <article
          v-for="item in searchResults"
          :key="`${item.id}-${item.version}`"
          class="lsp-extension-panel__item"
        >
          <div class="lsp-extension-panel__item-main">
            <div class="lsp-extension-panel__item-title">
              {{ item.display_name || item.id }}
            </div>
            <div class="lsp-extension-panel__item-meta">{{ item.id }} · {{ item.version }}</div>
            <div v-if="item.description" class="lsp-extension-panel__item-desc">{{ item.description }}</div>
          </div>
          <div class="lsp-extension-panel__item-actions">
            <n-button
              size="small"
              type="primary"
              :loading="isInstallingExtension(item.id)"
              :disabled="loading && !isInstallingExtension(item.id)"
              @click="handleInstallSearchResult(item)"
            >
              安装
            </n-button>
          </div>
        </article>
      </div>
    </section>

    <section class="lsp-extension-panel__section">
      <div class="lsp-extension-panel__section-title">
        已知语言扩展
        <span class="lsp-extension-panel__section-subtitle">
          已安装 {{ installedCount }} / {{ extensions.length }}
        </span>
      </div>

      <div v-if="loading" class="lsp-extension-panel__empty">正在加载扩展状态...</div>

      <div v-else-if="extensions.length" class="lsp-extension-panel__list">
        <article v-for="item in extensions" :key="item.id" class="lsp-extension-panel__item">
          <div class="lsp-extension-panel__item-main">
            <div class="lsp-extension-panel__item-title">
              {{ item.display_name || item.id }}
              <n-tag size="small" :type="item.installed ? 'success' : 'default'">
                {{ item.installed ? '已安装' : '未安装' }}
              </n-tag>
              <n-tag size="small" :type="runtimeTagType(item)">
                {{ runtimeTagText(item) }}
              </n-tag>
              <n-tag size="small" :type="compatibilityTagType(item.compatibility_level)">
                兼容级别 {{ item.compatibility_level || 'C' }}
              </n-tag>
            </div>

            <div class="lsp-extension-panel__item-meta">
              {{ item.id }}
              <span v-if="item.version && item.version !== 'not-installed'"> · {{ item.version }}</span>
              <span v-if="item.source"> · 来源 {{ item.source }}</span>
            </div>

            <div v-if="item.languages?.length" class="lsp-extension-panel__chips">
              <span
                v-for="language in item.languages"
                :key="`${item.id}-${language}`"
                class="lsp-extension-panel__chip"
              >
                {{ language }}
              </span>
            </div>

            <div v-if="item.description" class="lsp-extension-panel__item-desc">{{ item.description }}</div>
            <div v-if="item.installed_path" class="lsp-extension-panel__path">{{ item.installed_path }}</div>
            <div v-if="item.runtime_status?.message" class="lsp-extension-panel__runtime">
              {{ item.runtime_status.message }}
            </div>
          </div>

          <div class="lsp-extension-panel__item-actions">
            <n-button
              v-if="canInstallCatalogExtension(item)"
              size="small"
              type="primary"
              :loading="isInstallingExtension(item.id)"
              @click="handleInstallCatalogExtension(item)"
            >
              安装扩展
            </n-button>

            <n-button
              v-if="item.installed"
              size="small"
              tertiary
              type="error"
              :loading="isRemovingExtension(item.id)"
              @click="handleRemoveExtension(item)"
            >
              卸载
            </n-button>
          </div>
        </article>
      </div>

      <div v-else class="lsp-extension-panel__empty">当前还没有可管理的语言扩展。</div>
    </section>
  </div>
</template>

<script setup>
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useMessage } from 'naive-ui'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { createLogger } from '../../utils/logger'
import { refreshLspRuntimeExtensionSnapshot } from '../../monaco/lspCapabilities'
import { useSettingsStore } from '../../stores/settingsStore'

const logger = createLogger('LspExtensionPanel')
const message = useMessage()
const settingsStore = useSettingsStore()

const loading = ref(false)
const searching = ref(false)
const searchQuery = ref('')
const searchSource = ref('marketplace')
const searchResults = ref([])
const extensions = ref([])
const installingExtensionId = ref('')
const removingExtensionId = ref('')
const installingRecommended = ref(false)
const activeTaskSnapshots = ref({})
const pollingTaskIds = new Set()
let panelDisposed = false

const installedCount = computed(() =>
  extensions.value.filter((item) => item?.installed).length
)

const activeTaskList = computed(() =>
  Object.values(activeTaskSnapshots.value).filter((task) =>
    ['queued', 'running'].includes(String(task?.status || ''))
  )
)

const normalizeExtensionTaskId = (value) => String(value || '').trim().toLowerCase()

const isTaskForExtension = (task, extensionId) =>
  normalizeExtensionTaskId(task?.extension_id) === normalizeExtensionTaskId(extensionId)

const isInstallingExtension = (extensionId) => {
  const id = normalizeExtensionTaskId(extensionId)
  if (!id) return false
  if (normalizeExtensionTaskId(installingExtensionId.value) === id) return true
  return activeTaskList.value.some((task) =>
    isTaskForExtension(task, id) && String(task.operation || '').startsWith('install')
  )
}

const isRemovingExtension = (extensionId) => {
  const id = normalizeExtensionTaskId(extensionId)
  if (!id) return false
  if (normalizeExtensionTaskId(removingExtensionId.value) === id) return true
  return activeTaskList.value.some((task) =>
    isTaskForExtension(task, id) && String(task.operation || '') === 'remove'
  )
}

const searchSourceOptions = [
  { label: 'VSCode Marketplace', value: 'marketplace' },
  { label: 'Open VSX', value: 'openvsx' },
]

const searchSourceLabel = computed(() =>
  searchSource.value === 'openvsx' ? 'Open VSX' : 'VSCode Marketplace'
)

// 设计说明：
// 1. 这块面板的卡片、边框和按钮都有自定义样式，不能只依赖全局 Naive UI 主题。
// 2. 这里显式读取设置仓库和根节点 dataset，给组件根节点挂 `is-dark` class，
//    让暗色模式和布局覆盖稳定落在组件内部，避免 scoped 选择器失配。
const isDarkTheme = computed(() => {
  const storeTheme = String(settingsStore.appearanceTheme || '').trim().toLowerCase()
  const datasetTheme = String(document?.documentElement?.dataset?.guaxiTheme || '').trim().toLowerCase()
  return storeTheme === 'dark' || datasetTheme === 'dark'
})

const runtimeTagType = (item) => {
  if (item?.runtime_status?.ready) return 'success'
  if (item?.installed) return 'warning'
  return 'default'
}

const runtimeTagText = (item) => {
  if (item?.runtime_status?.ready) return '运行时已就绪'
  if (item?.installed) return '运行时未就绪'
  return '等待安装'
}

const compatibilityTagType = (level) => {
  if (String(level || '').toUpperCase() === 'A') return 'success'
  if (String(level || '').toUpperCase() === 'B') return 'warning'
  return 'default'
}

const canInstallCatalogExtension = (item) => {
  const extensionId = String(item?.id || '').trim()
  if (!extensionId || item?.installed) return false

  const source = String(item?.source || '').trim().toLowerCase()
  const version = String(item?.version || '').trim().toLowerCase()

  // 设计说明：
  // 1. 已知语言列表来自后端 catalog，不只有 Marketplace/Open VSX 扩展，也包含 guaxi.mermaid-lsp 这类托管 runtime。
  // 2. 安装入口统一走 start_install_catalog_lsp_extension_task，由后端按 catalog 决定下载 VSIX、创建 Guaxi catalog 记录或下载 server。
  // 3. 因此前端不能再用 open_vsx/vscode_marketplace 字段判断是否显示安装按钮，否则 Mermaid/HTML/CSS/JSON 这类 runtime-first 条目会没有安装入口。
  return source === 'catalog' || version === 'not-installed'
}

const getTauriIpcSnapshot = () => {
  const internals = window?.__TAURI_INTERNALS__
  return {
    isTauri: isTauri(),
    hasTauriInvoke: typeof internals?.invoke === 'function',
    hasPostMessageIpc: typeof window?.ipc?.postMessage === 'function',
    protocol: String(window?.location?.protocol || ''),
    href: String(window?.location?.href || ''),
  }
}

const isTauriIpcTransportError = (error) => {
  const text = `${String(error?.name || '')} ${String(error?.message || error || '')}`
  return (
    text.includes('Failed to fetch') ||
    text.includes('ipc.localhost') ||
    text.includes('__TAURI_INTERNALS__')
  )
}

const handleTauriIpcInstallError = (operation, error) => {
  const snapshot = getTauriIpcSnapshot()

  // 设计思路：
  // 1. LSP 扩展安装依赖 Rust 侧下载、解包和 runtime 探测，必须通过 Tauri IPC 调后端命令。
  // 2. 当页面运行在普通浏览器、WebView2 IPC 注入异常，或 ipc.localhost 自定义协议无法降级成功时，
  //    前端拿到的通常只是 TypeError: Failed to fetch，和真实 Marketplace/Open VSX 下载错误长得很像。
  // 3. 这里记录一份运行时快照，并给用户更明确的重启/运行环境提示，避免误判为 LSP 插件源不可用。
  logger.error('[LspExtensionPanel] LSP install IPC/runtime failure snapshot', {
    operation,
    error: String(error),
    snapshot,
  })

  if (!isTauriIpcTransportError(error)) {
    return false
  }

  message.error('无法连接 Tauri 后端 IPC，请确认当前是在桌面应用中运行。若已在桌面端，请重启应用后再安装 LSP 扩展。')
  return true
}

const syncRuntimeSnapshot = async () => {
  await refreshLspRuntimeExtensionSnapshot()
  window.dispatchEvent(
    new CustomEvent('guaxi:lsp-setting-changed', {
      detail: { source: 'lsp-extension-runtime-snapshot' },
    })
  )
  logger.info('[LspExtensionPanel] runtime snapshot refreshed after extension mutation')
}

const notifyInstallResult = (state, extensionId, options = {}) => {
  const installedId = String(state?.id || extensionId || '').trim()
  const runtimeReady = state?.runtime_status?.ready !== false
  const runtimeMessage = String(state?.runtime_status?.message || '').trim()

  // 设计思路：
  // 1. catalog 安装现在会自动准备共享运行时，并在后端完成真实 initialize 探测。
  // 2. 因此正常安装结果应当是“已安装并验证”；如果仍出现未就绪，通常代表手动导入或旧记录需要重新安装。
  // 3. 这里保留日志记录 runtime message，便于定位是扩展入口、共享运行时还是 PATH 缺失。
  if (runtimeReady) {
    if (!options.silentSuccess) {
      message.success(`已安装并验证 LSP 扩展 ${installedId}`)
    }
    logger.info('[LspExtensionPanel] extension installed with ready runtime', {
      extensionId: installedId,
      runtimeMessage,
    })
    return
  }

  message.warning(`已安装扩展 ${installedId}，但 LSP 运行时未就绪：${runtimeMessage || '未找到可执行文件，请重新安装该扩展'}`)
  logger.warn('[LspExtensionPanel] extension installed but runtime is pending', {
    extensionId: installedId,
    runtimeMessage,
  })
}

const isFinishedTask = (task) => ['success', 'error'].includes(String(task?.status || ''))

const rememberTaskSnapshot = (task) => {
  const taskId = String(task?.task_id || '').trim()
  if (!taskId) return
  activeTaskSnapshots.value = {
    ...activeTaskSnapshots.value,
    [taskId]: task,
  }
}

const forgetTaskSnapshot = (taskId) => {
  const id = String(taskId || '').trim()
  if (!id || !activeTaskSnapshots.value[id]) return
  const next = { ...activeTaskSnapshots.value }
  delete next[id]
  activeTaskSnapshots.value = next
}

const finishExtensionTask = async (task, fallbackExtensionId = '') => {
  const operation = String(task?.operation || '')
  const extensionId = String(task?.state?.id || task?.extension_id || fallbackExtensionId || '').trim()

  if (String(task?.status || '') === 'success') {
    if (operation === 'remove') {
      message.success(`已卸载扩展 ${extensionId}`)
    } else if (task?.state) {
      notifyInstallResult(task.state, extensionId)
    } else {
      message.success(`LSP 扩展任务已完成 ${extensionId}`)
    }
    await refreshExtensions()
    return
  }

  const errorText = String(task?.error || task?.message || '未知错误')
  logger.error('[LspExtensionPanel] extension background task failed', {
    taskId: task?.task_id,
    operation,
    extensionId,
    error: errorText,
  })
  message.error(`LSP 扩展任务失败: ${errorText}`)
}

const waitForExtensionTask = async (task, fallbackExtensionId = '') => {
  const taskId = String(task?.task_id || '').trim()
  if (!taskId || pollingTaskIds.has(taskId)) return task
  pollingTaskIds.add(taskId)

  try {
    let snapshot = task
    rememberTaskSnapshot(snapshot)
    while (!panelDisposed) {
      if (isFinishedTask(snapshot)) {
        await finishExtensionTask(snapshot, fallbackExtensionId)
        return snapshot
      }
      await new Promise((resolve) => window.setTimeout(resolve, 900))
      const latest = await invoke('get_lsp_extension_task', { taskId })
      if (!latest) {
        logger.warn('[LspExtensionPanel] extension task disappeared while polling', { taskId })
        return snapshot
      }
      snapshot = latest
      rememberTaskSnapshot(snapshot)
    }
    return snapshot
  } finally {
    pollingTaskIds.delete(taskId)
    forgetTaskSnapshot(taskId)
  }
}

const startExtensionTask = async (command, args, options = {}) => {
  const extensionId = String(options.extensionId || args?.extensionId || '').trim()
  logger.info('[LspExtensionPanel] starting LSP extension background task', {
    command,
    extensionId,
  })
  const task = await invoke(command, args)
  rememberTaskSnapshot(task)
  void waitForExtensionTask(task, extensionId)
  return task
}

const resumeActiveExtensionTasks = async () => {
  try {
    const tasks = await invoke('list_lsp_extension_tasks')
    const activeTasks = Array.isArray(tasks)
      ? tasks.filter((task) => ['queued', 'running'].includes(String(task?.status || '')))
      : []
    if (!activeTasks.length) return

    logger.info('[LspExtensionPanel] resuming active LSP extension tasks', {
      count: activeTasks.length,
      taskIds: activeTasks.map((task) => task.task_id),
    })
    activeTasks.forEach((task) => {
      rememberTaskSnapshot(task)
      void waitForExtensionTask(task, task?.extension_id)
    })
  } catch (error) {
    logger.warn('[LspExtensionPanel] resume active extension tasks failed', { error: String(error) })
  }
}

const refreshExtensions = async () => {
  try {
    loading.value = true
    const result = await invoke('list_lsp_extensions')
    extensions.value = Array.isArray(result) ? result : []
    await syncRuntimeSnapshot()
    logger.info('[LspExtensionPanel] extension state refreshed', {
      count: extensions.value.length,
      installedCount: installedCount.value,
      dark: isDarkTheme.value,
    })
  } catch (error) {
    logger.error('[LspExtensionPanel] refresh extension state failed', error)
    message.error(`加载 LSP 扩展状态失败: ${String(error)}`)
  } finally {
    loading.value = false
  }
}

const handleSearch = async () => {
  const query = String(searchQuery.value || '').trim()
  if (!query) {
    searchResults.value = []
    return
  }

  try {
    searching.value = true
    const command = searchSource.value === 'openvsx'
      ? 'search_open_vsx_extensions'
      : 'search_vscode_marketplace_extensions'
    const result = await invoke(command, { query })
    searchResults.value = Array.isArray(result) ? result : []
    logger.info('[LspExtensionPanel] extension search completed', {
      query,
      source: searchSource.value,
      resultCount: searchResults.value.length,
    })
  } catch (error) {
    logger.error('[LspExtensionPanel] extension search failed', error)
    message.error(`搜索扩展失败: ${String(error)}`)
  } finally {
    searching.value = false
  }
}

const handleInstallOpenVsx = async (item, options = {}) => {
  const extensionId = String(item?.id || '').trim()
  if (!extensionId) return
  options.silentSuccess = true

  try {
    installingExtensionId.value = extensionId
    logger.info('[LspExtensionPanel] installing extension from Open VSX', {
      extensionId,
      version: String(item?.version || ''),
      downloadUrl: String(item?.download_url || ''),
    })
    await startExtensionTask('start_install_open_vsx_extension_task', {
      extensionId,
      version: item?.version ? String(item.version) : null,
      downloadUrl: item?.download_url ? String(item.download_url) : null,
    }, {
      extensionId,
    })
    if (!options.silentSuccess) {
      message.success(`已安装扩展 ${extensionId}`)
    }
  } catch (error) {
    logger.error('[LspExtensionPanel] install from Open VSX failed', error)
    if (options.throwOnError) {
      throw error
    }
    message.error(`安装扩展失败: ${String(error)}`)
  } finally {
    installingExtensionId.value = ''
  }
}

const handleInstallMarketplace = async (item, options = {}) => {
  const extensionId = String(item?.id || '').trim()
  if (!extensionId) return
  options.silentSuccess = true

  try {
    installingExtensionId.value = extensionId
    logger.info('[LspExtensionPanel] installing extension from VSCode Marketplace', {
      extensionId,
      version: String(item?.version || ''),
      downloadUrl: String(item?.download_url || ''),
    })
    await startExtensionTask('start_install_vscode_marketplace_extension_task', {
      extensionId,
      version: item?.version ? String(item.version) : null,
      downloadUrl: item?.download_url ? String(item.download_url) : null,
    }, {
      extensionId,
    })
    if (!options.silentSuccess) {
      message.success(`已安装扩展 ${extensionId}`)
    }
  } catch (error) {
    logger.error('[LspExtensionPanel] install from VSCode Marketplace failed', error)
    if (options.throwOnError) {
      throw error
    }
    message.error(`安装扩展失败: ${String(error)}`)
  } finally {
    installingExtensionId.value = ''
  }
}

const handleInstallSearchResult = async (item) => {
  if (searchSource.value === 'openvsx') {
    await handleInstallOpenVsx(item)
    return
  }
  await handleInstallMarketplace(item)
}

const handleInstallCatalogExtension = async (item) => {
  const extensionId = String(item?.id || '').trim()
  if (!extensionId) return

  try {
    installingExtensionId.value = extensionId
    // 设计说明：
    // 1. 已知语言扩展来自 shared/lsp_extensions.json，是 Guaxi 已验证过 runtime 解析方式的能力目录。
    // 2. 这里优先安装 catalog 记录，而不是先碰 Marketplace/Open VSX 网络，让 HTML/Markdown/Lua/TeX
    //    这些核心语言不会因为市场搜索或内置扩展不可下载而安装失败。
    // 3. 真正 server runtime 仍按 catalog.resolve_order 启动：扩展自带优先，缺失时走受控下载器。
    logger.info('[LspExtensionPanel] installing catalog LSP extension', {
      extensionId,
      source: String(item?.source || ''),
      openVsx: String(item?.open_vsx || ''),
      vscodeMarketplace: String(item?.vscode_marketplace || ''),
      languages: Array.isArray(item?.languages) ? item.languages : [],
    })
    await startExtensionTask('start_install_catalog_lsp_extension_task', { extensionId }, { extensionId })
  } catch (error) {
    if (!handleTauriIpcInstallError('install_catalog_lsp_extension', error)) {
      logger.error('[LspExtensionPanel] install catalog extension failed', error)
      message.error(`安装扩展失败: ${String(error)}`)
    }
  } finally {
    installingExtensionId.value = ''
  }
}

const handleInstallRecommendedExtensions = async () => {
  const targets = extensions.value
    .filter((item) => item?.recommended && !item?.installed)
    .map((item) => String(item.id || '').trim())
    .filter(Boolean)
  if (!targets.length) {
    message.info('推荐扩展均已安装')
    return
  }

  try {
    installingRecommended.value = true
    logger.info('[LspExtensionPanel] installing recommended catalog LSP extensions', {
      targets,
    })
    for (const extensionId of targets) {
      installingExtensionId.value = extensionId
      const task = await startExtensionTask('start_install_catalog_lsp_extension_task', { extensionId }, { extensionId })
      logger.info('[LspExtensionPanel] recommended catalog extension task started', {
        extensionId,
        taskId: String(task?.task_id || ''),
      })
    }
    message.success(`已启动 ${targets.length} 个推荐 LSP 扩展安装任务`)
  } catch (error) {
    if (!handleTauriIpcInstallError('install_recommended_catalog_lsp_extensions', error)) {
      logger.error('[LspExtensionPanel] install recommended catalog extensions failed', error)
      message.error(`安装推荐扩展失败: ${String(error)}`)
    }
  } finally {
    installingRecommended.value = false
    installingExtensionId.value = ''
  }
}

const handleImportVsix = async () => {
  try {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'VSIX Extension', extensions: ['vsix'] }],
    })
    const vsixPath = Array.isArray(selected) ? selected[0] : selected
    if (!vsixPath) return

    logger.info('[LspExtensionPanel] importing VSIX extension', { vsixPath })
    await startExtensionTask('start_import_lsp_extension_vsix_task', { vsixPath }, { extensionId: `local-vsix:${vsixPath}` })
    message.success('VSIX 扩展导入任务已启动')
  } catch (error) {
    logger.error('[LspExtensionPanel] import VSIX failed', error)
    message.error(`导入 VSIX 失败: ${String(error)}`)
  }
}

const handleImportDirectory = async () => {
  try {
    const selected = await open({
      multiple: false,
      directory: true,
    })
    const dirPath = Array.isArray(selected) ? selected[0] : selected
    if (!dirPath) return

    logger.info('[LspExtensionPanel] importing unpacked extension directory', { dirPath })
    await startExtensionTask('start_import_lsp_extension_directory_task', { dirPath }, { extensionId: `local-directory:${dirPath}` })
    message.success('扩展目录导入任务已启动')
  } catch (error) {
    logger.error('[LspExtensionPanel] import extension directory failed', error)
    message.error(`导入扩展目录失败: ${String(error)}`)
  }
}

const handleRemoveExtension = async (item) => {
  const extensionId = String(item?.id || '').trim()
  if (!extensionId) return

  try {
    removingExtensionId.value = extensionId
    logger.info('[LspExtensionPanel] removing installed extension', { extensionId })
    await startExtensionTask('start_remove_lsp_extension_task', { extensionId }, { extensionId })
  } catch (error) {
    logger.error('[LspExtensionPanel] remove extension failed', error)
    message.error(`卸载扩展失败: ${String(error)}`)
  } finally {
    removingExtensionId.value = ''
  }
}

onMounted(() => {
  panelDisposed = false
  void refreshExtensions()
  void resumeActiveExtensionTasks()
})

onUnmounted(() => {
  panelDisposed = true
})
</script>

<style scoped>
.lsp-extension-panel {
  /* 设计说明：
   * 1. 这里统一抽出组件级颜色变量，亮暗两套主题只切变量，不改结构。
   * 2. 顶部操作区和卡片操作区都做成可换行布局，解决按钮在窄宽度下堆叠混乱的问题。
   */
  --lsp-panel-title: #1f2937;
  --lsp-panel-text: #111827;
  --lsp-panel-muted: #6b7280;
  --lsp-panel-border: rgba(15, 23, 42, 0.08);
  --lsp-panel-border-strong: rgba(148, 163, 184, 0.22);
  --lsp-panel-surface: rgba(83, 94, 105, 0.92);
  --lsp-panel-surface-elevated: rgba(255, 255, 255, 0.88);
  --lsp-panel-chip-bg: rgba(59, 130, 246, 0.1);
  --lsp-panel-chip-text: #475068;
  --lsp-panel-shadow: 0 10px 30px rgba(15, 23, 42, 0.06);
  display: grid;
  gap: 16px;
}

.lsp-extension-panel.is-dark {
  --lsp-panel-title: #e5edf8;
  --lsp-panel-text: #f8fbff;
  --lsp-panel-muted: #a7b4c8;
  --lsp-panel-border: rgba(148, 163, 184, 0.2);
  --lsp-panel-border-strong: rgba(148, 163, 184, 0.28);
  --lsp-panel-surface: rgba(15, 23, 42, 0.92);
  --lsp-panel-surface-elevated: rgba(30, 41, 59, 0.88);
  --lsp-panel-chip-bg: rgba(56, 189, 248, 0.18);
  --lsp-panel-chip-text: #8bdcff;
  --lsp-panel-shadow: 0 18px 34px rgba(2, 6, 23, 0.28);
}

.lsp-extension-panel__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.lsp-extension-panel__hero {
  min-width: 0;
  display: grid;
  gap: 8px;
}

.lsp-extension-panel__title {
  margin: 0;
  color: var(--lsp-panel-title);
  font-size: 24px;
  font-weight: 700;
}

.lsp-extension-panel__description {
  margin: 0;
  color: var(--lsp-panel-muted);
  line-height: 1.75;
}

.lsp-extension-panel__header-actions {
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 10px;
}

.lsp-extension-panel__header-actions :deep(.n-button) {
  min-width: 112px;
}

.lsp-extension-panel__search {
  display: grid;
  grid-template-columns: minmax(180px, 240px) minmax(0, 1fr) auto;
  gap: 10px;
}

.lsp-extension-panel__source {
  min-width: 0;
}

.lsp-extension-panel__search-btn {
  min-width: 140px;
}

.lsp-extension-panel__section {
  display: grid;
  gap: 10px;
}

.lsp-extension-panel__section-title {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
  font-weight: 700;
  color: var(--lsp-panel-title);
}

.lsp-extension-panel__section-subtitle {
  font-size: 12px;
  color: var(--lsp-panel-muted);
  font-weight: 500;
}

.lsp-extension-panel__list {
  display: grid;
  gap: 10px;
}

.lsp-extension-panel__item {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding: 14px 16px;
  border-radius: 12px;
  border: 1px solid var(--lsp-panel-border);
  background: linear-gradient(135deg, var(--lsp-panel-surface-elevated), var(--lsp-panel-surface));
  box-shadow: var(--lsp-panel-shadow);
}

.lsp-extension-panel__item-main {
  min-width: 0;
  display: grid;
  gap: 6px;
}

.lsp-extension-panel__item-title {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  font-weight: 700;
  color: var(--lsp-panel-text);
}

.lsp-extension-panel__item-meta,
.lsp-extension-panel__item-desc,
.lsp-extension-panel__path,
.lsp-extension-panel__runtime,
.lsp-extension-panel__empty {
  font-size: 12px;
  line-height: 1.6;
  color: var(--lsp-panel-muted);
  word-break: break-word;
}

.lsp-extension-panel__chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.lsp-extension-panel__chip {
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--lsp-panel-chip-bg);
  color: var(--lsp-panel-chip-text);
  font-size: 12px;
  line-height: 1.4;
}

.lsp-extension-panel__item-actions {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 8px;
  width: min(180px, 100%);
  flex-shrink: 0;
}

.lsp-extension-panel__item-actions :deep(.n-button) {
  width: 100%;
  justify-content: center;
}

.lsp-extension-panel.is-dark :deep(.n-input),
.lsp-extension-panel.is-dark :deep(.n-base-selection),
.lsp-extension-panel.is-dark :deep(.n-button:not(.n-button--primary-type)) {
  --n-border: var(--lsp-panel-border-strong);
  --n-border-hover: color-mix(in srgb, var(--lsp-panel-border-strong) 70%, #8bdcff 30%);
  --n-border-pressed: color-mix(in srgb, var(--lsp-panel-border-strong) 64%, #8bdcff 36%);
  --n-color: color-mix(in srgb, var(--lsp-panel-surface) 92%, transparent);
  --n-color-hover: color-mix(in srgb, var(--lsp-panel-surface-elevated) 92%, transparent);
  --n-color-pressed: color-mix(in srgb, var(--lsp-panel-surface-elevated) 84%, transparent);
  --n-text-color: var(--lsp-panel-text);
}

@media (max-width: 900px) {
  .lsp-extension-panel__header,
  .lsp-extension-panel__item {
    flex-direction: column;
  }

  .lsp-extension-panel__search {
    grid-template-columns: 1fr;
  }

  .lsp-extension-panel__header-actions,
  .lsp-extension-panel__item-actions {
    width: 100%;
  }

  .lsp-extension-panel__header-actions :deep(.n-button),
  .lsp-extension-panel__item-actions :deep(.n-button),
  .lsp-extension-panel__search-btn {
    min-width: 0;
    width: 100%;
  }
}
</style>
