<template>
  <div class="guaxi-header">
    <!-- 左侧：Logo 和 Icon -->
    <div class="header-left">
      <GuaxiSiderToggle view-key="repo" side="left" />
      <div class="logo-text">Guaxi</div>
    </div>

    <!-- 中间：搜索框 -->
    <div class="header-center">
      <div class="search-container">
        <n-input ref="searchInputRef" v-model:value="searchQuery" placeholder="搜索导入文件" round clearable
          @keyup.enter="handleSearchEnter" @focus="showSearchOptions" @blur="handleSearchBlur">
          <template #prefix>
            <n-icon :component="Search32Filled" />
          </template>
        </n-input>
      </div>
    </div>

    <!-- 右侧：功能图标 -->
    <div class="header-right">
      <n-icon class="header-icon" size="20" :component="Share24Regular" title="资料库分享与发布"
        @click="openLibrarySite"></n-icon>
      <GuaxiSiderToggle view-key="repo" side="right" />
    </div>
  </div>

  <!-- 搜索下拉框 -->
  <n-dropdown v-model:show="isSearchDropdownVisible" :options="searchOptions" placement="bottom-start"
    :x="contextMenuPosition.x" :y="contextMenuPosition.y" @select="handleSearchSelect" trigger="manual"
    :style="dropdownStyle" />
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { computed, ref, h } from "vue";
import { NIcon, NDropdown, useMessage } from 'naive-ui'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { repoStore } from '../../stores/repoStore'
import { useSearch } from './composables/useSearch.js'
import GuaxiSiderToggle from '../GuaxiSiderToggle.vue'
import {
  Screenshot24Regular,
  Search32Filled,
  TextBulletListLtr20Filled,
  DocumentSearch20Filled,
  DocumentSearch24Regular,
  Share24Regular,
} from '@vicons/fluent'
import { invoke } from '@tauri-apps/api/core'
import { sep } from '@tauri-apps/api/path'
import { useRightSiderStore } from '../../stores/rightSiderStore'

const logger = createLogger('GuaxiHeaderRepoView')
const rightSiderStore = useRightSiderStore()
const fileTreeStore = useFileTreeStore()
const store = repoStore()
const message = useMessage()
const { handleSearch: handleTableSearch } = useSearch({ store, message })
const searchInputRef = ref(null)
const searchType = ref('keyword')

// 资料库 · 分享与发布:在独立窗口打开库网站(展示文件库内容,支持本地阅读器查看/下载/联邦分享)。
async function openLibrarySite() {
  const theme = document.documentElement.dataset.guaxiTheme === 'dark' ? 'dark' : 'light'
  try {
    await invoke('open_library_site_window', { theme })
  } catch (e) {
    logger.error('打开资料库窗口失败', e)
    message.error('打开资料库窗口失败')
  }
}

async function CaptureScreenshot() {
  try {
    const originalPath = fileTreeStore.selectedFile.path
    logger.info("[Screenshot] Original path:", originalPath)

    const pathSeparator = await sep()

    const dirPath = originalPath.substring(0, originalPath.lastIndexOf(pathSeparator))
    const fileName = originalPath.substring(
      originalPath.lastIndexOf(pathSeparator) + 1,
      originalPath.lastIndexOf('.')
    )

    const imagesDir = `${dirPath}${pathSeparator}images`
    logger.info("[Screenshot] Images directory:", imagesDir)
    await invoke('create_screenshot_dir', { path: imagesDir })

    const screenshotPath = `${imagesDir}${pathSeparator}${fileName}.png`
    logger.info("[Screenshot] Will be saved to:", screenshotPath)

    await invoke('take_screenshot', { path: screenshotPath })

    logger.info("[Screenshot] Saved successfully:", screenshotPath)
  } catch (err) {
    logger.error('[Screenshot] Failed:', err)
  }
}

// 搜索相关状态
const isSearchDropdownVisible = ref(false)
const contextMenuPosition = ref({ x: 0, y: 0 })
const searchQuery = ref('')
const searchResults = ref([])
const showSearchModal = ref(false)
const dropdownWidth = ref(300)
const dropdownStyle = computed(() => ({
  width: `${dropdownWidth.value}px`,
  maxWidth: '400px'
}))

// 搜索选项（按表格列名进行搜索）
const searchOptions = [
  {
    label: '按文件名搜索',
    key: 'fileName',
    icon: () => h(NIcon, null, { default: () => h(DocumentSearch24Regular) })
  },
  {
    label: '按文件描述搜索',
    key: 'description',
    icon: () => h(NIcon, null, { default: () => h(DocumentSearch24Regular) })
  },
  {
    label: '按文件标签搜索',
    key: 'tags',
    icon: () => h(NIcon, null, { default: () => h(DocumentSearch24Regular) })
  },
  {
    label: '按所属文件库搜索',
    key: 'library',
    icon: () => h(NIcon, null, { default: () => h(DocumentSearch24Regular) })
  },
  {
    label: '按知识库搜索',
    key: 'knowledgeRepo',
    icon: () => h(NIcon, null, { default: () => h(DocumentSearch24Regular) })
  },
  {
    label: '按文件类型搜索',
    key: 'fileType',
    icon: () => h(NIcon, null, { default: () => h(DocumentSearch24Regular) })
  }
]

const showSearchOptions = (e) => {
  if (!isSearchDropdownVisible.value) {
    const inputElement = e.target
    const rect = inputElement.getBoundingClientRect()

    dropdownWidth.value = rect.width

    contextMenuPosition.value = {
      x: rect.left,
      y: rect.bottom + 5
    }

    isSearchDropdownVisible.value = true

    logger.info('[Search] Show options menu', {
      position: contextMenuPosition.value,
      width: dropdownWidth.value,
      searchType: searchType.value
    })
  }
}

const handleSearchBlur = () => {
  setTimeout(() => {
    isSearchDropdownVisible.value = false
  }, 200)
}

const handleSearchEnter = () => {
  if (searchQuery.value.trim()) {
    logger.info('[Search] Executing search:', {
      query: searchQuery.value,
      type: searchType.value
    })

    handleSearchImported()
  }
}

const handleSearchSelect = (key) => {
  logger.info('[Search] Selected search type:', key)
  searchType.value = key
  isSearchDropdownVisible.value = false

  if (searchQuery.value.trim()) {
    handleSearchEnter()
  }

  // 保持输入框焦点
  searchInputRef.value?.focus()
}

async function handleSearchImported() {
  try {
    const raw = searchQuery.value.trim()

    let keyword = ""
    let library = ""
    let importedFileTags = ""
    let fileType = ""
    let knowledgeRepo = ""

    if (!raw) {
      store.setSearchParams({
        keyword: "",
        fileType: "",
        knowledgeRepo: "",
        library: "",
        importedFileTags: "",
        importFileCreateTime: null,
        importFileSize: null,
      })
    } else if (searchType.value === 'keyword') {
      const parts = raw.split(/\s+/).filter((p) => p.length > 0)
      let keywordParts = []

      for (const part of parts) {
        const idx = part.indexOf(':')
        if (idx <= 0) {
          keywordParts.push(part)
          continue
        }
        const key = part.slice(0, idx).toLowerCase()
        const value = part.slice(idx + 1)
        if (!value) {
          keywordParts.push(part)
          continue
        }
        if (key === 'lib' || key === 'library') {
          library = value
        } else if (key === 'tag' || key === 'tags') {
          importedFileTags = value
        } else if (key === 'type' || key === 'ft' || key === 'filetype') {
          fileType = value
        } else if (key === 'repo' || key === 'knowledge' || key === 'kr') {
          knowledgeRepo = value
        } else {
          keywordParts.push(part)
        }
      }

      keyword = keywordParts.join(' ')
    } else if (searchType.value === 'library') {
      library = raw
    } else if (searchType.value === 'tags') {
      importedFileTags = raw
    } else if (searchType.value === 'fileType') {
      fileType = raw
    } else if (searchType.value === 'knowledgeRepo') {
      knowledgeRepo = raw
    } else if (searchType.value === 'fileName' || searchType.value === 'description') {
      keyword = raw
    } else {
      keyword = raw
    }

    store.setSearchParams({
      keyword,
      fileType,
      knowledgeRepo,
      library,
      importedFileTags,
      importFileCreateTime: null,
      importFileSize: null,
    })
    handleTableSearch()
  } catch (err) {
    logger.error('[Search] Imported files search failed:', err);
  }
}
</script>

<style scoped>
.guaxi-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 35px;
  background-color: var(--guaxi-header-bg);
  box-shadow: var(--guaxi-header-shadow);
  padding: 0 8px;
  box-sizing: border-box;
  border-bottom: 1px solid var(--guaxi-border);
}

/* 重置所有内部div的盒模型 */
.guaxi-header > div {
  margin: 0;
  padding: 0;
  height: 100%;
}

/* 左侧样式 */
.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 200px;
}

.logo-text {
  font-weight: bold;
  font-size: 16px;
  white-space: nowrap;
  line-height: 1;
}

/* 中间样式 */
.header-center {
  flex: 1;
  display: flex;
  justify-content: center;
  max-width: 600px;
  margin: 0 10px;
}

.search-container {
  width: 100%;
  max-width: 400px;
  display: flex;
  align-items: center;
}

/* 加强输入框样式优先级 */
.search-container :deep(.n-input) {
  height: 25px !important;
  min-height: 25px !important;
  line-height: 25px !important;
  margin: 0 !important;
  background-color: var(--guaxi-panel-bg) !important;
  overflow: hidden;
}

.search-container :deep(.n-input .n-input-wrapper) {
  background-color: var(--guaxi-panel-bg) !important;
}

.search-container :deep(.n-input .n-input__border),
.search-container :deep(.n-input .n-input__state-border) {
  border-color: var(--guaxi-border) !important;
}

.search-container :deep(.n-input__input-el) {
  color: var(--guaxi-text-primary) !important;
}

.search-container :deep(.n-input__prefix),
.search-container :deep(.n-input__suffix) {
  color: var(--guaxi-text-secondary) !important;
}

/* 右侧样式 */
.header-right {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 200px;
  justify-content: flex-end;
}

/* 图标通用样式 */
.header-icon {
  cursor: pointer;
  transition: all 0.2s ease;
  padding: 2px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.header-icon:hover {
  opacity: 0.8;
  transform: scale(1.05);
  background-color: var(--guaxi-panel-bg);
}

/* 下拉菜单样式 */
:deep(.n-dropdown-menu) {
  width: 100% !important;
  max-width: 400px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
  border-radius: 4px;
  margin-top: 4px !important;
  padding: 4px 0;
}

:deep(.n-dropdown-option) {
  padding: 8px 12px;
}

:deep(.n-dropdown-option:hover) {
  background-color: var(--guaxi-header-bg);
}
</style>
