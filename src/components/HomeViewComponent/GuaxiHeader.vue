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
      <n-icon class="header-icon" size="20" :component="TextBulletListLtr20Filled" @click="openOutlinePanel"></n-icon>
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
import { computed, ref, h, onMounted, nextTick } from "vue";
import { NIcon, NDropdown } from 'naive-ui'
import { useSiderStore } from '../../stores/useSiderStore'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useRightSiderStore } from '../../stores/rightSiderStore'
import { useSettingsStore } from '../../stores/settingsStore'
import { useMessage } from 'naive-ui';
import {
  Screenshot24Regular,
  Search32Filled,
  TextBulletListLtr20Filled,
  DocumentSearch20Filled,
  DocumentSearch24Regular,
  PanelLeftContract24Regular,
  PanelRightContract24Regular,
  NotebookSection24Regular
} from '@vicons/fluent'
import { invoke } from '@tauri-apps/api/core'
import { sep } from '@tauri-apps/api/path'


const rightSiderStore = useRightSiderStore()
const settingsStore = useSettingsStore()
const siderStore = useSiderStore();
const fileTreeStore = useFileTreeStore()
const searchInputRef = ref(null)
const searchType = ref('file')

const viewKey = 'home'
const message = useMessage();
const toggleLeftSider = () => {
  siderStore.toggleLeftSider(viewKey)
}
const toggleRightSider = () => {
  siderStore.toggleRightSider(viewKey)
}

function openOutlinePanel() {
  rightSiderStore.setPanel('outline')
  rightSiderStore.open()
  // 主动展开右侧边栏
  if (siderStore.isRightSiderCollapsed[viewKey]) {
    siderStore.toggleRightSider(viewKey);
  }
}

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
  },
  {
    label: '搜索摘录笔记',
    key: 'excerpt',
    icon: () => h(NIcon, null, { default: () => h(NotebookSection24Regular) })
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
    } else if (searchType.value === 'note') {
      handleSearchNotes()
    } else if (searchType.value === 'excerpt') {
      handleSearchExcerpts()
    }
  }
}

const handleSearchSelect = (key) => {
  console.log('[Search] Selected search type:', key);

  // 重置搜索状态
  searchType.value = key;
  rightSiderStore.setSearchResults({
    lastSearchType: key
  });

  isSearchDropdownVisible.value = false;

  // 如果当前有搜索词，立即执行新类型的搜索
  if (searchQuery.value.trim()) {
    console.log('[Search] Auto-triggering search after type change');
    handleSearchEnter();
  }

  // 保持输入框焦点
  nextTick(() => {
    searchInputRef.value?.focus();
  });
};

// 通用的搜索结果规范化函数
const normalizeSearchResults = (rawResults, type) => {
  let normalized = {};
  if (!Array.isArray(rawResults)) {
    console.warn(`[Search] Expected array for ${type} results, got:`, rawResults);
    return [];
  }

  return rawResults
    .filter(item => {
      // 基础验证
      const isValid = item && typeof item === 'object';
      if (!isValid) {
        console.warn(`[Search] Invalid ${type} item filtered:`, item);
        return false;
      }
      return true;
    })
    .map(item => {
      // 规范化字段
      if (type === 'note') {
        normalized = {
          path: String(item.path || ''),
          file_name: String(item.file_name || path.basename(item.path || '')),
          preview: String(item.preview || item.text || ''),
          line_number: Number(item.line_number) || 0,
          matches: Array.isArray(item.matches) ? item.matches : [],
          _searchType: type
        }
      }

      if (type === 'file') {
        normalized = {
          path: String(item.path || ''),
          file_name: String(item.file_name || path.basename(item.path || '')),
          _searchType: type
        }
      }

      // 特殊处理摘录搜索结果
      if (type === 'excerpt') {
        normalized.excerpt_id = item.id || '';
        normalized.source_file = item.source_file || '';
      }

      return normalized;
    })
    .filter(item => {
      // 最终验证
      const isValid = item.path && (item.preview || item.file_name);
      if (!isValid) {
        console.warn(`[Search] Incomplete ${type} item filtered:`, item);
      }
      return isValid;
    });
};

async function handleSearchFiles() {
  try {
    console.log('[Search] Starting file search:', {
      query: searchQuery.value,
      path: fileTreeStore.activeRepoPath
    });

    // 清除旧结果
    rightSiderStore.setSearchResults({
      fileResults: [],
      searchQuery: searchQuery.value
    });

    console.log('[Search] Searching... ', searchQuery.value, fileTreeStore.activeRepoPath);
    // 调用搜索API
    const rawResponse = await invoke('fd_search', {
      pattern: searchQuery.value,
      path: fileTreeStore.activeRepoPath
    });

    // 解析和规范化结果
    const [rawResults = [], hasMore = false] = Array.isArray(rawResponse)
      ? rawResponse
      : [];

    console.log('{Search] File Search rawResults', rawResults);
    const validResults = normalizeSearchResults(rawResults, 'file');

    console.log('[Search] File search completed', {
      results: validResults.length,
      hasMore
    });

    // 更新存储状态
    rightSiderStore.setSearchResults({
      fileResults: validResults,
      searchQuery: searchQuery.value,
      lastSearchType: 'file',
      loading: false
    });

    // 打开搜索结果面板
    rightSiderStore.setPanel('search');
    // 如果右侧边栏折叠，展开它
    if (!siderStore.isRightSiderCollapsed[viewKey]) {
      siderStore.setRightSiderCollapsed(viewKey, false);
    }
  } catch (error) {
    console.error('[Search] File search failed:', error);
    rightSiderStore.setSearchResults({
      fileResults: [],
      searchQuery: searchQuery.value,
      error: error.message || '文件搜索失败'
    });
    message.error(`文件搜索失败: ${error.message}`);
  }
}

async function handleSearchNotes() {
  try {
    console.log('[Search] Starting note search:', {
      query: searchQuery.value,
      path: fileTreeStore.activeRepoPath
    });

    // 清除旧结果
    rightSiderStore.setSearchResults({
      noteResults: [],
      searchQuery: searchQuery.value,
      loading: true
    });

    const response = await invoke('rg_search', {
      pattern: searchQuery.value,
      path: fileTreeStore.activeRepoPath
    });

    // 智能解析响应结构
    const rawResults = (() => {
      if (!response) return [];
      if (Array.isArray(response[0])) return response[0]; // [results, hasMore]结构
      if (Array.isArray(response)) return response;      // 纯数组结构
      return [response];                                // 单结果对象
    })();

    // 规范化结果
    const validResults = normalizeSearchResults(rawResults, 'note');

    // 更新存储状态
    rightSiderStore.setSearchResults({
      noteResults: validResults,
      searchQuery: searchQuery.value,
      lastSearchType: 'note',
      loading: false
    });

    // 打开搜索结果面板
    rightSiderStore.setPanel('search');
    // 如果右侧边栏折叠，展开它
    if (!siderStore.isRightSiderCollapsed[viewKey]) {
      siderStore.setRightSiderCollapsed(viewKey, false);
    }

    console.log('[Search] Note search completed', {
      results: validResults.length
    });

  } catch (err) {
    console.error('[Search] Note search failed:', err);
    rightSiderStore.setSearchResults({
      noteResults: [],
      searchQuery: searchQuery.value,
      error: err.message || '内容搜索失败',
      loading: false
    });
    message.error(`内容搜索失败: ${err.message}`);
  }
}

async function handleSearchExcerpts() {
  try {
    console.log('[Search] Executing excerpt search:', {
      query: searchQuery.value,
      path: settingsStore.librariesRoot
    });
    const results = await invoke('search_excerpt_notes', {
      pattern: searchQuery.value,
      path: settingsStore.librariesRoot
    });
    console.log('[Search] Excerpt search results:', results);

    rightSiderStore.setSearchResults({
      excerptResults: results, // 只包含file_path和file_name的数组
      searchQuery: searchQuery.value,
      lastSearchType: 'note',
    });

    rightSiderStore.panel = 'search';
    if (!siderStore.isRightSiderCollapsed[viewKey]) {
      siderStore.setRightSiderCollapsed(viewKey, false); // 确保右侧边栏展开
    }

  } catch (err) {
    console.error('[Search] Excerpt search failed:', err);
    message.error('摘录搜索失败: ' + (err.message || String(err)));
  }
}

onMounted(async () => {
  settingsStore.get_librariesRoot()
})
</script>

<style scoped>
.guaxi-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 35px;
  /* 固定高度 */
  background-color: #f5f5f5;
  box-shadow: 0px 2px 4px rgba(0, 0, 0, 0.1);
  padding: 0 8px;
  box-sizing: border-box;
  /* 确保padding不增加总高度 */
}

/* 重置所有内部div的盒模型 */
.guaxi-header>div {
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
  padding: 0 8px !important;
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
  background-color: rgba(0, 0, 0, 0.05);
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
  background-color: #f5f5f5;
}
</style>