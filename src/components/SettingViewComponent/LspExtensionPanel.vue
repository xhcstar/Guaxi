<template>
  <div class="lsp-extension-panel" :class="{ 'is-dark': isDarkTheme }">
    <div class="lsp-extension-panel__header">
      <div class="lsp-extension-panel__hero">
        <h3 class="lsp-extension-panel__title">LSP 扩展架构</h3>
        <p class="lsp-extension-panel__description">
          这里管理 VSIX / Open VSX 语言扩展
        </p>
      </div>
      <n-space class="lsp-extension-panel__header-actions" wrap>
        <n-button size="small" :disabled="loading" @click="refreshExtensions">刷新状态</n-button>
        <n-button size="small" :disabled="loading" @click="handleImportDirectory">导入扩展目录</n-button>
        <n-button size="small" type="primary" :disabled="loading" @click="handleImportVsix">导入 VSIX</n-button>
      </n-space>
    </div>

    <div class="lsp-extension-panel__search">
      <n-input
        v-model:value="searchQuery"
        placeholder="搜索 Open VSX 语言扩展，例如 clangd / basedpyright / rust-analyzer"
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
        搜索 Open VSX
      </n-button>
    </div>

    <section v-if="searchResults.length" class="lsp-extension-panel__section">
      <div class="lsp-extension-panel__section-title">Open VSX 搜索结果</div>
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
              :loading="installingExtensionId === item.id"
              :disabled="loading && installingExtensionId !== item.id"
              @click="handleInstallOpenVsx(item)"
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
              v-if="!item.installed && item.open_vsx"
              size="small"
              type="primary"
              :loading="installingExtensionId === item.id"
              @click="handleInstallCatalogExtension(item)"
            >
              从 Open VSX 安装
            </n-button>

            <n-button
              v-if="item.installed"
              size="small"
              tertiary
              type="error"
              :loading="removingExtensionId === item.id"
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
import { computed, onMounted, ref } from 'vue'
import { useMessage } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
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
const searchResults = ref([])
const extensions = ref([])
const installingExtensionId = ref('')
const removingExtensionId = ref('')

const installedCount = computed(() =>
  extensions.value.filter((item) => item?.installed).length
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
  if (item?.installed) return '运行时待补齐'
  return '等待安装'
}

const compatibilityTagType = (level) => {
  if (String(level || '').toUpperCase() === 'A') return 'success'
  if (String(level || '').toUpperCase() === 'B') return 'warning'
  return 'default'
}

const syncRuntimeSnapshot = async () => {
  await refreshLspRuntimeExtensionSnapshot()
  logger.info('[LspExtensionPanel] runtime snapshot refreshed after extension mutation')
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
    const result = await invoke('search_open_vsx_extensions', { query })
    searchResults.value = Array.isArray(result) ? result : []
    logger.info('[LspExtensionPanel] Open VSX search completed', {
      query,
      resultCount: searchResults.value.length,
    })
  } catch (error) {
    logger.error('[LspExtensionPanel] Open VSX search failed', error)
    message.error(`搜索 Open VSX 失败: ${String(error)}`)
  } finally {
    searching.value = false
  }
}

const handleInstallOpenVsx = async (item) => {
  const extensionId = String(item?.id || '').trim()
  if (!extensionId) return

  try {
    installingExtensionId.value = extensionId
    logger.info('[LspExtensionPanel] installing extension from Open VSX', {
      extensionId,
      version: String(item?.version || ''),
      downloadUrl: String(item?.download_url || ''),
    })
    await invoke('install_open_vsx_extension', {
      extensionId,
      version: item?.version ? String(item.version) : null,
      downloadUrl: item?.download_url ? String(item.download_url) : null,
    })
    message.success(`已安装扩展 ${extensionId}`)
    await refreshExtensions()
  } catch (error) {
    logger.error('[LspExtensionPanel] install from Open VSX failed', error)
    message.error(`安装扩展失败: ${String(error)}`)
  } finally {
    installingExtensionId.value = ''
  }
}

const handleInstallCatalogExtension = async (item) => {
  await handleInstallOpenVsx({
    id: item?.open_vsx || item?.id,
    version: null,
    download_url: null,
  })
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
    await invoke('import_lsp_extension_vsix', { vsixPath })
    message.success('VSIX 扩展导入完成')
    await refreshExtensions()
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
    await invoke('import_lsp_extension_directory', { dirPath })
    message.success('扩展目录导入完成')
    await refreshExtensions()
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
    await invoke('remove_lsp_extension', { extensionId })
    message.success(`已卸载扩展 ${extensionId}`)
    await refreshExtensions()
  } catch (error) {
    logger.error('[LspExtensionPanel] remove extension failed', error)
    message.error(`卸载扩展失败: ${String(error)}`)
  } finally {
    removingExtensionId.value = ''
  }
}

onMounted(() => {
  void refreshExtensions()
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
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 10px;
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
