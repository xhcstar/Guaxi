<!-- TableViewRepoView.vue -->
<template>
  <div class="main-content-wrapper">
    <!-- 顶部工具栏 -->
    <TopToolbar :progresses="progresses" :categorized-progresses="categorizedProgresses" :conversion-tasks="conversionTasks"
      @toggle-task-list="toggleTaskList" @backup-database="backupDatabase" @recover-database="recoverDatabase"
      @import-items="importItems" @export-items="exportItems" @sync-repo="handleSyncRepo" />

    <!-- 任务列表抽屉 -->
    <TaskListDrawer :show="showTaskList" @update:show="showTaskList = $event" :progresses="progresses" :categorized-progresses="categorizedProgresses" :clear-failed-tasks="clearFailedTasks" :cancel-sync="cancelSyncTask" :conversion-tasks="conversionTasks" :retry-task="retryConversionTask" :cancel-task="cancelConversionTask" :delete-task="deleteConversionTask" :clear-finished="clearFinishedConversionTasks" />

    <!-- 搜索栏 -->

    <!-- 表格工具栏 + 视图切换 -->
    <div class="toolbar-container">
      <div class="view-toggle" role="group" aria-label="视图切换">
        <button
          class="view-toggle-btn"
          :class="{ on: viewMode === 'table' }"
          title="表格视图"
          @click="setViewMode('table')"
        >
          <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
            <path fill="currentColor" d="M3 5h18v2H3V5zm0 6h18v2H3v-2zm0 6h18v2H3v-2z" />
          </svg>
          <span>表格</span>
        </button>
        <button
          class="view-toggle-btn"
          :class="{ on: viewMode === 'shelf' }"
          title="书架视图"
          @click="setViewMode('shelf')"
        >
          <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
            <path fill="currentColor" d="M4 3h3v16H4V3zm4 0h3v16H8V3zm6.4.6l2.9.8l-3.6 13.4l-2.9-.8L14.4 3.6zM4 20h16v1H4v-1z" />
          </svg>
          <span>书架</span>
        </button>
      </div>
      <n-button v-if="checkedRowKeys.length" size="small" type="error" secondary class="batch-delete-btn"
        @click="handleBatchDelete">
        <template #icon>
          <n-icon :component="Delete20Regular" :size="16" />
        </template>
        删除选中 ({{ checkedRowKeys.length }})
      </n-button>
      <TableToolbar class="toolbar-flex" @sort="handleSort" @clear-filters="handleClearFilters" @add-file="showAddFileModal" />
    </div>

    <!-- 数据表格容器 -->
    <div v-show="viewMode === 'table'" class="table-container" ref="tableContainerRef">
      <DataTable ref="tableRef" :columns="columns" :data="sortedData" :table-height="dynamicTableHeight"
        :scroll-x="scrollX" :pagination="pagination" :row-key="rowKey" :row-props="rowProps"
        @update:checked-row-keys="handleCheckboxUpdate" @row-contextmenu="handleRowContextMenu" />
    </div>

    <!-- 书架视图容器 -->
    <div v-if="viewMode === 'shelf'" class="shelf-container">
      <BookshelfView
        :books="sortedData"
        :library="store.currentDatabase"
        @open="handleOpen"
        @contextmenu="handleBookContextMenu"
      />
    </div>

    <!-- 右键菜单 -->
    <ContextMenu :show="showDropdown" @update:show="showDropdown = $event" :x="x" :y="y" :options="contextMenuOptions"
      @select="handleContextMenuSelect" @clickoutside="onClickoutside" />

    <!-- 添加文件模态框 -->
    <AddFileModal :show="isAddFileModalVisible" @update:show="isAddFileModalVisible = $event" :form="currentForm"
      @update:form="handleFormUpdate" :rules="rules" :form-columns="formColumns"
      :knowledge-repo-options="knowledgeRepoOptionsComplete" :library-options="libraryOptionsComplete"
      :imported-file-tag-options="importedFileTagOptionsComplete" :is-library-loading="isLibraryLoadingComplete"
      :is-imported-file-tag-loading="isImportedFileTagLoadingComplete"
      :is-knowledge-repo-loading="isKnowledgeRepoLoadingComplete" :engine-status="engineStatus"
      @open-file-browser="openFileBrowser"
      @library-search-complete="handleLibrarySearchComplete" @tag-search-complete="handleImportedFileTagSearchComplete"
      @knowledge-repo-search-complete="handleKnowledgeRepoSearchComplete" @submit="addFile"
      @cancel="handleModalCancel" />

    <!-- 统一删除确认弹窗（条目删除/批量删除/右键删除共用） -->
    <ConfirmModal />
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, onMounted, onUnmounted, computed, watch, nextTick, h } from 'vue'
import { useMessage, NButton, NIcon } from 'naive-ui'
import { Delete20Regular } from '@vicons/fluent'
import { invoke } from '@tauri-apps/api/core'
import { useConfirm } from './composables/useConfirm.js'


// Stores
import { repoStore } from '../../stores/repoStore'
import { useEventStore } from '../../stores/eventsStore'

// Components
import TopToolbar from './components/TableViewComponents/TopToolbar.vue'
import TaskListDrawer from './components/TableViewComponents/TaskListDrawer.vue'
import TableToolbar from './components/TableViewComponents/TableToolbar.vue'
import DataTable from './components/TableViewComponents/DataTable.vue'
import ContextMenu from './components/TableViewComponents/ContextMenu.vue'
import AddFileModal from './components/TableViewComponents/AddFileModal.vue'
import ConfirmModal from './components/TableViewComponents/ConfirmModal.vue'
import ShowOrEdit from './common/ShowOrEdit.vue'
import BookshelfView from './BookshelfView.vue'

// Composables
import { useFormHandling } from './composables/useFormHandling.js'
import { useProgress } from './composables/useProgress.js'
import { useContextMenu } from './composables/useContextMenu.js'
import { useSearch } from './composables/useSearch.js'
import { useSearchComplete } from './SearchUtils/useSearchComplete.js'
import { useTableConfig } from './composables/useTableConfig.js'
import { useFileOperations } from './composables/useFileOperations.js'

const logger = createLogger('TableViewRepoView')
// 初始化存储和消息
const store = repoStore()
const eventStore = useEventStore()
const message = useMessage()
const { confirm } = useConfirm()

// 批量删除选中的条目（软删除，逐个调用 delete_imported_file）。用统一的自定义确认弹窗。
const handleBatchDelete = async () => {
  const ids = [...(checkedRowKeys.value || [])]
  if (!ids.length) {
    message.info('请先勾选要删除的条目')
    return
  }
  const confirmed = await confirm({
    title: '批量删除所选条目？',
    description: '选中的条目将被标记为删除（不会立即移除磁盘上的原始文件与已转换内容）。',
    count: ids.length,
    countUnit: '个条目',
    confirmLabel: '删除',
    danger: true,
  })
  if (!confirmed) return

  let ok = 0
  let fail = 0
  for (const id of ids) {
    try {
      await invoke('delete_imported_file', { id })
      ok += 1
    } catch (e) {
      fail += 1
      logger.error('批量删除失败 id=' + id, e)
    }
  }
  checkedRowKeys.value = []
  try {
    await store.fetchFiles(store.currentDatabase)
    await store.fetchRepos()
  } catch (e) {
    logger.warn('批量删除后刷新失败', e)
  }
  if (fail === 0) {
    message.success(`已删除 ${ok} 个条目`)
  } else {
    message.warning(`已删除 ${ok} 个，失败 ${fail} 个`)
  }
}
const containerHeight = ref(0)
const handleWindowResize = () => {
  nextTick(() => {
    const container = document.querySelector('.table-container')
    if (container) {
      containerHeight.value = container.clientHeight
    }
  })
}

const dynamicTableHeight = computed(() => {
  // 返回实际可用高度，不做额外减法
  return containerHeight.value > 0 ? containerHeight.value : 400
})

// 使用组合式函数
const {
  progresses, // 原始任务数组（保持兼容）
  categorizedProgresses, // 新增：分类后的任务
  getProgressCountByType, // 新增：按类型获取任务数量
  showTaskList,
  toggleTaskList,
  startProgressListener,
  convertFile,
  clearFailedTasks,
  cancelSyncTask,
  getConversionEngineStatus,
  conversionTasks,
  retryConversionTask,
  cancelConversionTask,
  deleteConversionTask,
  clearFinishedConversionTasks,
  reconvertImportedFile
} = useProgress({ store, message })

// 转换引擎装配状态（快速引擎启动自动装配；强引擎按需装配）
const engineStatus = ref({ fastReady: false, fastProvisioning: false, markerReady: false, mineruReady: false })
const refreshEngineStatus = async () => {
  try {
    engineStatus.value = await getConversionEngineStatus()
  } catch (e) {
    logger.warn('获取转换引擎状态失败', e)
  }
}

const {
  isAddFileModalVisible,
  currentForm,
  rules,
  formColumns,
  libraryOptions,
  importedFileTagOptions,
  isLibraryLoading,
  isImportedFileTagLoading,
  showAddFileModal,
  openFileBrowser,
  addFile,
  handleFileInfoEvent,
  handleFormUpdate,
  handleModalCancel
} = useFormHandling({
  store,
  message,
  startProgressListener,
  convertFile
})

const {
  showDropdown,
  x,
  y,
  contextMenuOptions,
  handleContextMenuSelect,
  onClickoutside,
  rowProps,
  handleRowContextMenu,
  handleOpen
} = useContextMenu({ store, message, reconvert: reconvertImportedFile })

// ── 视图模式：'table' 表格 / 'shelf' 书架（持久化用户偏好）──
const VIEW_MODE_KEY = 'guaxi_repoview_mode'
const viewMode = ref(localStorage.getItem(VIEW_MODE_KEY) === 'shelf' ? 'shelf' : 'table')
const setViewMode = (mode) => {
  viewMode.value = mode
  try { localStorage.setItem(VIEW_MODE_KEY, mode) } catch { /* ignore */ }
  if (mode === 'table') nextTick(handleWindowResize)
}
// 书架中右键一本书：复用表格那套右键菜单（打开/编辑/删除…）
const handleBookContextMenu = ({ event, book }) => handleRowContextMenu(event, book)

const {
  searchParams,
  searchPanelCollapsed,
  fileTypeOptions,
  knowledgeRepoOptions,
  filteredData,
  searchLibraryOptions,
  searchImportedFileTagOptions,
  handleSearch,
  handleLibrarySearch,
  handleImportedFileTagSearch,
  handleKnowledgeRepoSearch,
  isLibraryLoadingAdvanceSearch,
  isImportedFileTagLoadingAdvanceSearch,
  isKnowledgeRepoLoadingAdvanceSearch,
  clearSearch,
  initializeSearch
} = useSearch({ store, message });

const {
  handleLibrarySearchComplete,
  handleImportedFileTagSearchComplete,
  libraryOptionsComplete,
  importedFileTagOptionsComplete,
  importedFileLibraryOptionsComplete,
  noteTagOptionsComplete,
  importedFileOptionsComplete,
  isLibraryLoadingComplete,
  isImportedFileTagLoadingComplete,
  isImportedFileLibraryLoadingComplete,
  isNoteTagLoadingComplete,
  knowledgeRepoOptionsComplete,
  isKnowledgeRepoLoadingComplete,
  handleKnowledgeRepoSearchComplete
} = useSearchComplete()

// 使用表格配置
const {
  tableRef,
  tableHeight,
  pagination,
  scrollX,
  rowKey,
  columns,
  heightForRow,
  handleCheckboxUpdate,
  sortTitle,
  clearFilters,
  resetTableState,
  checkedRowKeys
} = useTableConfig({ store, message, ShowOrEdit })

const { backupDatabase: backupDatabaseOp, recoverDatabase: recoverDatabaseOp, importItems: importItemsOp, exportItems: exportItemsOp, syncLibrariesRoot: syncLibrariesRootOp, syncCurrentRepo: syncCurrentRepoOp } = useFileOperations({ store, message })

const sortMode = ref(null)

const textCollator = new Intl.Collator(['zh-Hans-CN-u-co-pinyin', 'en'], {
  sensitivity: 'base',
  numeric: true,
})

const sortedData = computed(() => {
  const data = [...filteredData.value]
  if (!sortMode.value) {
    return data
  }

  if (sortMode.value === 'name') {
    return data.sort((a, b) => textCollator.compare(a.importFileName || '', b.importFileName || ''))
  }

  if (sortMode.value === 'library') {
    return data.sort((a, b) => textCollator.compare(a.library || '', b.library || ''))
  }

  if (sortMode.value === 'date') {
    return data.sort((a, b) => {
      const ta = a.importFileCreateTime ? new Date(a.importFileCreateTime).getTime() : 0
      const tb = b.importFileCreateTime ? new Date(b.importFileCreateTime).getTime() : 0
      return ta - tb
    })
  }

  return data
})

const handleSort = (mode) => {
  sortMode.value = mode
}

const handleClearFilters = () => {
  sortMode.value = null
  clearSearch()
  resetTableState()
}

// 文件操作实现
const backupDatabase = async (scope = 'current') => {
  try {
    await backupDatabaseOp(scope)
  } catch (error) {
    logger.error('备份失败:', error)
  }
}

const recoverDatabase = async () => {
  try {
    await recoverDatabaseOp()
  } catch (error) {
    logger.error('恢复失败:', error)
  }
}

const importItems = async () => {
  try {
    await importItemsOp()
  } catch (error) {
    logger.error('导入失败:', error)
  }
}

const exportItems = async () => {
  try {
    await exportItemsOp(checkedRowKeys.value)
  } catch (error) {
    logger.error('导出失败:', error)
  }
}

const handleSyncRepo = async (key) => {
  try {
    if (key === 'libraries') {
      await syncLibrariesRootOp()
    } else if (key === 'current_repo') {
      await syncCurrentRepoOp()
    }
  } catch (error) {
    logger.error('同步失败:', error)
  }
}

const refreshRepoFilesAfterImportChange = async (payload = {}) => {
  const changedKnowledgeRepo = payload?.knowledge_repo || ''
  const currentKnowledgeRepo = store.currentDatabase || ''
  const shouldRefreshFiles = !currentKnowledgeRepo || !changedKnowledgeRepo || changedKnowledgeRepo === currentKnowledgeRepo

  logger.info('[RepoImportEvent] repo_imported_files_changed received', {
    payload,
    currentKnowledgeRepo,
    shouldRefreshFiles,
  })

  try {
    await store.fetchRepos()

    if (shouldRefreshFiles) {
      // 设计思路：后端已经完成 imported_files 入库后才会发该事件，这里只重新拉取当前知识库列表，
      // 避免旧的 Web Clipper 保存事件重复走导入流程，同时保证表格/书架视图使用同一份 store.files。
      const refreshKnowledgeRepo = currentKnowledgeRepo || changedKnowledgeRepo || store.currentDatabase
      logger.info('[RepoImportEvent] refreshing imported files', {
        refreshKnowledgeRepo,
        action: payload?.action,
        id: payload?.id,
      })
      await store.fetchFiles(refreshKnowledgeRepo)
    } else {
      logger.info('[RepoImportEvent] skip file refresh because changed repo is not active', {
        changedKnowledgeRepo,
        currentKnowledgeRepo,
      })
    }
  } catch (error) {
    logger.error('[RepoImportEvent] refresh failed', error)
  }
}

const initAppHandle = async () => {
  try {
    await invoke('init_backend_services')
    logger.info('AppHandle initialized successfully')
  } catch (error) {
    logger.error('Failed to initialize AppHandle:', error)
    message.error('初始化应用句柄失败，部分功能可能受限')
  }
}

const handleGlobalClick = (e) => {
  // 检查是否点击了菜单区域
  const isContextMenu = e.target.closest('.custom-context-menu-wrapper') ||
    e.target.closest('.n-dropdown-menu') ||
    e.target.closest('.n-dropdown')

  // 只在点击菜单外部时关闭
  if (!isContextMenu && showDropdown.value) {
    showDropdown.value = false
  }
}

// 监听输入值变化并触发搜索
watch(() => currentForm.value.importedFileTags, (newValue) => {
  handleImportedFileTagSearchComplete(newValue)
})

watch(() => currentForm.value.notesTags, (newValue) => {
  handleNoteTagSearchComplete(newValue)
})

watch(() => currentForm.value.fromImportedFile, (newValue) => {
  handleImportedFileSearchComplete(newValue)
})

watch(() => currentForm.value.library, (newValue) => {
  handleLibrarySearchComplete(newValue)
})

watch(() => currentForm.value.fromImportedFileLibrary, (newValue) => {
  handleImportedFileLibrarySearchComplete(newValue)
})

watch(() => currentForm.value.knowledgeRepo, (newValue) => {
  handleKnowledgeRepoSearchComplete(newValue)
})

watch(() => eventStore.webClipperEvent, (newEvent) => {
  if (newEvent) {
    handleFileInfoEvent({ payload: newEvent })
    // 处理完后清除事件
    eventStore.webClipperEvent = null
  }
})

// 组件挂载时获取默认数据
onMounted(async () => {
  // 初始化 AppHandle
  await initAppHandle()

  store.get_librariesRoot()
  await store.fetchFiles(store.currentDatabase) // 默认加载数据库 1 的数据
  document.addEventListener('click', handleGlobalClick, true)

  // 获取转换引擎装配状态（快速引擎会在后端启动时自动装配）
  refreshEngineStatus()
  // 短轮询几次，装配完成后更新 UI（约 30s 内）
  let engineStatusPolls = 0
  const engineStatusTimer = setInterval(async () => {
    await refreshEngineStatus()
    engineStatusPolls += 1
    if (engineStatus.value?.fastReady || engineStatusPolls >= 10) {
      clearInterval(engineStatusTimer)
    }
  }, 3000)

  // 计算表格容器高度 - 使用更准确的方式
  const calculateHeight = () => {
    nextTick(() => {
      const container = document.querySelector('.table-container')
      if (container) {
        const rect = container.getBoundingClientRect()
        containerHeight.value = rect.height
        logger.info('Table container height:', containerHeight.value)
      }
    })
  }

  calculateHeight()

  // 监听窗口大小变化
  window.addEventListener('resize', () => {
    calculateHeight()
  })

  try {
    // 安全初始化
    await eventStore.initGlobalListeners()
    await eventStore.registerEventType('repo_imported_files_changed')

    // 订阅事件（带自动注册）
    const unsubscribe = eventStore.subscribe(
      'add_web_clipper_file_event',
      (payload) => {
        logger.info('处理事件:', payload)
        handleFileInfoEvent(payload)
      }
    )

    const unsubscribeRepoImportedFilesChanged = eventStore.subscribe(
      'repo_imported_files_changed',
      refreshRepoFilesAfterImportChange
    )

    onUnmounted(unsubscribe)
    onUnmounted(unsubscribeRepoImportedFilesChanged)
  } catch (error) {
    logger.error('事件系统初始化失败:', error)
  }

  initializeSearch();
})

onUnmounted(() => {
  document.removeEventListener('click', handleGlobalClick, true)
  window.removeEventListener('resize', handleWindowResize)
})
</script>

<style scoped>
.main-content-wrapper {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', monospace;
}

.toolbar-container {
  padding: 8px 16px;
  flex-shrink: 0;
  background-color: var(--guaxi-panel-bg);
  border-bottom: 1px solid var(--guaxi-border);
  display: flex;
  align-items: center;
  gap: 12px;
}

.toolbar-flex {
  flex: 1;
  min-width: 0;
}

.batch-delete-btn {
  flex: 0 0 auto;
  border-radius: 6px;
  height: 32px;
  font-weight: 500;
}

/* 表格 / 书架 分段切换 */
.view-toggle {
  flex: 0 0 auto;
  display: inline-flex;
  padding: 2px;
  border-radius: 8px;
  background: var(--guaxi-header-bg);
  border: 1px solid var(--guaxi-border);
}
.view-toggle-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 12px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
  transition: color 0.18s ease, background-color 0.18s ease, box-shadow 0.18s ease;
}
.view-toggle-btn:hover { color: var(--n-color-primary, #2dd4bf); }
.view-toggle-btn.on {
  color: #fff;
  background: linear-gradient(135deg, #2dd4bf, #14b8a6);
  box-shadow: 0 2px 8px rgba(20, 184, 166, 0.32);
}

.shelf-container {
  flex: 1;
  width: 100%;
  min-height: 0;
  overflow: hidden;
}

.table-container {
  flex: 1;
  width: 100%;
  overflow: visible;
  padding: 0;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
</style>
