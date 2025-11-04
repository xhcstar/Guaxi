<!-- TableViewRepoView.vue -->
<template>
  <div class="main-content-wrapper">
    <!-- 顶部工具栏 -->
    <TopToolbar :progresses="progresses" :categorized-progresses="categorizedProgresses"
      @toggle-task-list="toggleTaskList" @backup-database="backupDatabase" @recover-database="recoverDatabase"
      @import-items="importItems" @export-items="exportItems" />

    <!-- 任务列表抽屉 -->
    <TaskListDrawer :show="showTaskList" @update:show="showTaskList = $event" :progresses="progresses" />

    <!-- 搜索栏 -->
    <SearchPanel :search-params="searchParams" @update:search-params="searchParams = $event"
      :collapsed="searchPanelCollapsed" @update:collapsed="searchPanelCollapsed = $event"
      :file-type-options="fileTypeOptions" :knowledge-repo-options="knowledgeRepoOptions"
      :library-options="libraryOptions" :imported-file-tag-options="importedFileTagOptions" @search="handleSearch"
      @library-search="handleLibrarySearch" @tag-search="handleImportedFileTagSearch" />

    <div class="toolbar-container">
      <n-space vertical :size="12" class="container">
        <!-- 工具栏 -->
        <TableToolbar @sort="sortTitle" @clear-filters="clearFilters" @add-file="showAddFileModal" />
      </n-space>
    </div>

    <div class="table-container">
      <!-- 数据表格 -->
      <DataTable ref="tableRef" :columns="columns" :data="filteredData" :table-height="tableHeight" :scroll-x="scrollX"
        :pagination="pagination" :row-key="rowKey" :row-props="rowProps" @update:checked-row-keys="handleCheckboxUpdate"
        @row-contextmenu="handleRowContextMenu" />
    </div>

    <!-- 右键菜单 -->
    <ContextMenu :show="showDropdown" @update:show="showDropdown = $event" :x="x" :y="y" :options="contextMenuOptions"
      @select="handleContextMenuSelect" @clickoutside="onClickoutside" />

    <!-- 添加文件模态框 -->
    <AddFileModal :show="isAddFileModalVisible" @update:show="isAddFileModalVisible = $event" :form="currentForm"
      @update:form="handleFormUpdate" :rules="rules" :form-columns="formColumns" :library-options="libraryOptions"
      :imported-file-tag-options="importedFileTagOptions" :is-library-loading="isLibraryLoading"
      :is-imported-file-tag-loading="isImportedFileTagLoading" @open-file-browser="openFileBrowser"
      @library-search="handleLibrarySearch" @tag-search="handleImportedFileTagSearch" @submit="addFile"
      @cancel="handleModalCancel" />
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted, computed, watch, nextTick, h } from 'vue'
import { NIcon, NButton, NDropdown, useMessage, NDataTable, NProgress, NTag, NText, NBadge, NCard, NList, NListItem, NDrawer, NDrawerContent, NModal, NForm, NFormItem, NInput, NInputGroup, NAutoComplete, NSelect, NDatePicker, NSpace, NTooltip } from 'naive-ui'
import { platform } from '@tauri-apps/plugin-os'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

// Stores
import { repoStore } from '../../stores/repoStore'
import { useEventStore } from '../../stores/eventsStore'

// Components
import TopToolbar from './components/TableViewComponents/TopToolbar.vue'
import TaskListDrawer from './components/TableViewComponents/TaskListDrawer.vue'
import SearchPanel from './components/TableViewComponents/SearchPanel.vue'
import TableToolbar from './components/TableViewComponents/TableToolbar.vue'
import DataTable from './components/TableViewComponents/DataTable.vue'
import ContextMenu from './components/TableViewComponents/ContextMenu.vue'
import AddFileModal from './components/TableViewComponents/AddFileModal.vue'
import ShowOrEdit from './common/ShowOrEdit.vue'

// Composables
import { useFormHandling } from './composables/useFormHandling.js'
import { useProgress } from './composables/useProgress.js'
import { useContextMenu } from './composables/useContextMenu.js'
import { useSearch } from './composables/useSearch.js'
import { useTableConfig } from './composables/useTableConfig.js'

// 初始化存储和消息
const store = repoStore()
const eventStore = useEventStore()
const message = useMessage()

// 使用组合式函数
const {
  progresses, // 原始任务数组（保持兼容）
  categorizedProgresses, // 新增：分类后的任务
  getProgressCountByType, // 新增：按类型获取任务数量
  showTaskList,
  toggleTaskList,
  startProgressListener,
  stopProgressListener,
  convertFile
} = useProgress({ store, message })

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
  handleModalCancel,
  resetForm,
  validateForm,
  validateFilePath,
  createImportFileList,
  addFileToDatabase
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
  selectedRow,
  contextMenuOptions,
  handleContextMenuSelect,
  onClickoutside,
  rowProps,
  handleRowContextMenu,
  handleOpen,
  handleEdit,
  handleDelete,
  handleDeleteAll,
  handleShare,
  handleMove,
  handleExport,
  handleOpenDirImport,
  handleOpenDirNote
} = useContextMenu({ store, message })

const {
  searchParams,
  searchPanelCollapsed,
  fileTypeOptions,
  knowledgeRepoOptions,
  filteredData,
  // 关键修改：获取库选项和标签选项
  libraryOptions: searchLibraryOptions, // 重命名避免冲突
  importedFileTagOptions: searchImportedFileTagOptions, // 重命名避免冲突
  handleSearch,
  toggleSearchPanel,
  handleLibrarySearch,
  handleImportedFileTagSearch
} = useSearch({
  store,
  message,
  externalState: {
    isLibraryLoading: isLibraryLoading,
    isImportedFileTagLoading: isImportedFileTagLoading
  }
})

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
  checkedRowKeys
} = useTableConfig({ store, message, ShowOrEdit })

// 文件操作实现
const backupDatabase = async () => {
  try {
    message.info('开始备份资料库...')
    // 可以在这里添加备份任务
    // progresses.value.push({ ...备份任务配置, type: 'backup' })
    await new Promise(resolve => setTimeout(resolve, 1000))
    message.success('资料库备份成功')
  } catch (error) {
    console.error('备份失败:', error)
    message.error('资料库备份失败')
  }
}

const recoverDatabase = async () => {
  try {
    message.info('开始从备份文件中恢复...')
    // 可以在这里添加恢复任务
    // progresses.value.push({ ...恢复任务配置, type: 'recovery' })
    await new Promise(resolve => setTimeout(resolve, 1000))
    message.success('资料库恢复成功')
  } catch (error) {
    console.error('恢复失败:', error)
    message.error('资料库恢复失败')
  }
}

const importItems = async () => {
  try {
    message.info('开始导入指定条目...')
    // 可以在这里添加导入任务
    // progresses.value.push({ ...导入任务配置, type: 'import' })
    await new Promise(resolve => setTimeout(resolve, 1000))
    message.success('条目导入成功')
  } catch (error) {
    console.error('导入失败:', error)
    message.error('条目导入失败')
  }
}

const exportItems = async () => {
  try {
    message.info('开始导出指定条目...')
    // 可以在这里添加导出任务
    // progresses.value.push({ ...导出任务配置, type: 'export' })
    await new Promise(resolve => setTimeout(resolve, 1000))
    message.success('条目导出成功')
  } catch (error) {
    console.error('导出失败:', error)
    message.error('条目导出失败')
  }
}

const initAppHandle = async () => {
  try {
    await invoke('init_backend_services')
    console.log('AppHandle initialized successfully')
  } catch (error) {
    console.error('Failed to initialize AppHandle:', error)
    message.error('初始化应用句柄失败，部分功能可能受限')
  }
}

const handleGlobalClick = (e) => {
  // 如果不是右键菜单相关的元素，才关闭菜单
  const isContextMenu = e.target.closest('.n-dropdown') ||
    e.target.closest('.custom-context-menu')

  if (!isContextMenu && showDropdown.value) {
    console.log('全局点击，关闭菜单')
    setTimeout(() => {
      showDropdown.value = false
    }, 200)
  }
}

// 监听输入值变化并触发搜索
watch(() => currentForm.value.importedFileTags, (newValue) => {
  handleImportedFileTagSearch(newValue)
})

watch(() => currentForm.value.notesTags, (newValue) => {
  handleNoteTagSearch(newValue)
})

watch(() => currentForm.value.fromImportedFile, (newValue) => {
  handleImportedFileSearch(newValue)
})

watch(() => currentForm.value.library, (newValue) => {
  handleLibrarySearch(newValue)
})

watch(() => currentForm.value.fromImportedFileLibrary, (newValue) => {
  handleImportedFileLibrarySearch(newValue)
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

  try {
    // 安全初始化
    await eventStore.initGlobalListeners()

    // 订阅事件（带自动注册）
    const unsubscribe = eventStore.subscribe(
      'add_web_clipper_file_event',
      (payload) => {
        console.log('处理事件:', payload)
        handleFileInfoEvent(payload)
      }
    )

    onUnmounted(unsubscribe)
  } catch (error) {
    console.error('事件系统初始化失败:', error)
  }
})

onUnmounted(() => {
  document.removeEventListener('click', handleGlobalClick, true)
})
</script>

<style scoped>
/* 样式保持不变 */
.main-content-wrapper {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.search-container {
  padding: 0 16px 16px;
}

.toolbar-container {
  padding: 0 16px 12px;
  flex-shrink: 0;
}

.table-container {
  flex: 1;
  width: 100%;
  overflow: hidden;
  padding: 0 16px 16px;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
}
</style>
