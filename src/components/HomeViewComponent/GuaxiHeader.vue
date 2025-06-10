<template>
  <div class="guaxi-header">
    <!-- 左侧：Logo 和 Icon -->
    <div class="header-left">
      <n-icon size="20" class="header-icon" :component="PanelLeftContract24Regular" @click="toggleLeftSider"></n-icon>
      <div class="logo-text">Guaxi</div>
    </div>

    <!-- 中间：搜索框 -->
    <div class="header-center">
      <div class="search-container">
        <n-input ref="searchInputRef" v-model:value="searchQuery" placeholder="搜索文件或内容..." round clearable
          @keyup.enter="handleSearchEnter" @focus="showSearchOptions" @blur="handleSearchBlur">
          <template #prefix>
            <n-icon :component="Search32Filled" />
          </template>
        </n-input>
      </div>
    </div>

    <!-- 右侧：功能图标 -->
    <div class="header-right">
      <n-icon class="header-icon" size="20" :component="TextBulletListLtr20Filled"></n-icon>
      <n-icon class="header-icon" size="20" :component="Screenshot24Regular" @click="CaptureScreenshot"></n-icon>
      <n-icon class="header-icon" size="20" :component="PanelRightContract24Regular" @click="toggleRightSider"></n-icon>
    </div>
  </div>

  <!-- 搜索下拉框 -->
  <n-dropdown v-model:show="isSearchDropdownVisible" :options="searchOptions" placement="bottom-start"
    :x="contextMenuPosition.x" :y="contextMenuPosition.y" @select="handleSearchSelect" trigger="manual"
    :style="dropdownStyle" />
</template>

<script setup>
import { computed, ref, h } from "vue";
import { NIcon, NDropdown } from 'naive-ui'
import { useSiderStore } from '../../stores/useSiderStore'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import {
  Screenshot24Regular,
  Search32Filled,
  TextBulletListLtr20Filled,
  DocumentSearch20Filled,
  DocumentSearch24Regular,
  PanelLeftContract24Regular,
  PanelRightContract24Regular,
} from '@vicons/fluent'
import { invoke } from '@tauri-apps/api/core'
import { sep } from '@tauri-apps/api/path'
import { useRightSiderStore } from '../../stores/rightSiderStore'


const rightSiderStore = useRightSiderStore()
const siderStore = useSiderStore();
const fileTreeStore = useFileTreeStore()
const searchInputRef = ref(null)

const isLeftSiderCollapsed = computed(() => siderStore.isLeftSiderCollapsed);
const isRightSiderCollapsed = computed(() => siderStore.isRightSiderCollapsed);
const searchType = ref('file')

const toggleLeftSider = () => {
  siderStore.toggleLeftSider()
};

const toggleRightSider = () => {
  siderStore.toggleRightSider()
};

async function CaptureScreenshot() {
  try {
    const originalPath = fileTreeStore.selectedFile.path
    console.log("[Screenshot] Original path:", originalPath)

    const pathSeparator = await sep()

    const dirPath = originalPath.substring(0, originalPath.lastIndexOf(pathSeparator))
    const fileName = originalPath.substring(
      originalPath.lastIndexOf(pathSeparator) + 1,
      originalPath.lastIndexOf('.')
    )

    const imagesDir = `${dirPath}${pathSeparator}images`
    console.log("[Screenshot] Images directory:", imagesDir)
    await invoke('create_screenshot_dir', { path: imagesDir })

    const screenshotPath = `${imagesDir}${pathSeparator}${fileName}.png`
    console.log("[Screenshot] Will be saved to:", screenshotPath)

    await invoke('take_screenshot', { path: screenshotPath })

    console.log("[Screenshot] Saved successfully:", screenshotPath)
  } catch (err) {
    console.error('[Screenshot] Failed:', err)
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

// 搜索选项
const searchOptions = [
  {
    label: '搜索文件',
    key: 'file',
    icon: () => h(NIcon, null, { default: () => h(DocumentSearch20Filled) })
  },
  {
    label: '搜索关键字',
    key: 'note',
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

    console.log('[Search] Show options menu', {
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
    console.log('[Search] Executing search:', {
      query: searchQuery.value,
      type: searchType.value
    })

    if (searchType.value === 'file') {
      handleSearchFiles()
    } else {
      handleSearchNotes()
    }
  }
}

const handleSearchSelect = (key) => {
  console.log('[Search] Selected search type:', key)
  searchType.value = key
  isSearchDropdownVisible.value = false

  if (searchQuery.value.trim()) {
    handleSearchEnter()
  }

  // 保持输入框焦点
  searchInputRef.value?.focus()
}

async function handleSearchFiles() {
  try {
    const results = await invoke('fd_search', {
      pattern: searchQuery.value,
      path: fileTreeStore.activeRepoPath
    });

    rightSiderStore.setSearchResults({
      fileResults: results.map(item => ({
        path: item.path,
        file_name: item.file_name,
        preview: item.preview,
        line_number: item.line_number,
        matches: item.matches
      })),
      noteResults: [],
      searchQuery: searchQuery.value
    });

    // 添加这行代码：如果右侧边栏是折叠状态，则展开它
    if (siderStore.isRightSiderCollapsed) {
      siderStore.toggleRightSider();
    }
  } catch (err) {
    console.error('[Search] File search failed:', err);
  }
}

async function handleSearchNotes() {
  try {
    const results = await invoke('rg_search', {
      pattern: searchQuery.value,
      path: fileTreeStore.activeRepoPath
    });

    const validResults = results.filter(item =>
      item.preview && item.line_number && !item.path.includes('.git')
    );

    rightSiderStore.setSearchResults({
      fileResults: [],
      noteResults: validResults,
      searchQuery: searchQuery.value
    });

    // 确保这行代码存在
    if (siderStore.isRightSiderCollapsed) {
      siderStore.toggleRightSider();
    }
  } catch (err) {
    console.error('[Search] Note search failed:', err);
    message.error('搜索失败: ' + (err.message || String(err)));
  }
}
</script>

<style scoped>
.guaxi-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 35px;
  background-color: #f5f5f5;
  box-shadow: 0px 2px 4px rgba(0, 0, 0, 0.1);
  border-bottom: 2px solid #aab4ae;
  padding: 0 8px;
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
  padding: 4px;
  border-radius: 4px;
}

.header-icon:hover {
  opacity: 0.8;
  transform: scale(1.05);
  background-color: rgba(0, 0, 0, 0.05);
}

/* 搜索输入框样式 */
.n-input {
  height: 25px !important;
  line-height: 25px !important;
  min-width: 200px;
  margin: 0 !important;
  padding: 0 8px !important;
  transition: all 0.2s ease;
}

:deep(.n-input__border) {
  border: none !important;
}

:deep(.n-input-wrapper) {
  padding: 0 !important;
}

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
  background-color: #f5f5f5;
}
</style>