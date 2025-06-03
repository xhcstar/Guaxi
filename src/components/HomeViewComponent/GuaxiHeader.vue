<template>
  <div class="guaxi-header">
    <!-- 左侧：Logo 和 Icon -->
    <div style="display: flex; align-items: center; gap: 8px; padding-left: 9px;">
      <n-icon size="20" style="cursor: pointer;" @click="toggleLeftSider">
        <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 24 24">
          <g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="4" y="4" width="16" height="16" rx="2"></rect>
            <path d="M9 4v16"></path>
            <path d="M15 10l-2 2l2 2"></path>
          </g>
        </svg>
      </n-icon>
      <div style="font-weight: bold; font-size: 16px;">Guaxi</div>
    </div>

    <div class="header-center">
      <div class="search-container">
        <n-input v-model:value="searchQuery" placeholder="搜索文件或内容..." round clearable @keyup.enter="handleSearchEnter"
          @focus="showSearchOptions">
          <template #prefix>
            <n-icon :component="Search32Filled" />
          </template>
        </n-input>
      </div>
    </div>

    <!-- 右侧占位内容 -->
    <div style="gap: 8px; padding-right: 9px;">
      <n-icon class="top-right-navigation-icon" size="20" style="cursor: pointer;"
        :component="TextBulletListLtr20Filled"></n-icon>
      <n-icon class="top-right-navigation-icon" size="20" style="cursor: pointer;" :component="Screenshot24Regular"
        @click="CaptureScreenshot"></n-icon>
      <n-icon class="top-right-navigation-icon" size="20" style="cursor: pointer;" @click="toggleRightSider">
        <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 24 24">
          <g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="4" y="4" width="16" height="16" rx="2"></rect>
            <path d="M15 4v16"></path>
            <path d="M9 10l2 2l-2 2"></path>
          </g>
        </svg>
      </n-icon>
    </div>
  </div>

  <!-- 搜索下拉框 -->
  <n-dropdown v-model:show="isSearchDropdownVisible" :options="searchOptions" placement="bottom-start"
    :x="contextMenuPosition.x" :y="contextMenuPosition.y" @select="handleSearchSelect" trigger="manual"
    style="width: 300px" />
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
  DocumentSearch24Regular
} from '@vicons/fluent'
import { invoke } from '@tauri-apps/api/core'
import { sep } from '@tauri-apps/api/path'

const siderStore = useSiderStore();
const fileTreeStore = useFileTreeStore()
const isLeftSiderCollapsed = computed(() => siderStore.isLeftSiderCollapsed);
const isRightSiderCollapsed = computed(() => siderStore.isRightSiderCollapsed);
const searchType = ref('file') // 新增搜索类型状态
const toggleLeftSider = () => {
  siderStore.toggleLeftSider()
};

const toggleRightSider = () => {
  siderStore.toggleRightSider()
};

// 监听OCR结果
// listen('ocr-result', (event) => {
//   console.log('OCR结果:', event.payload)
// })

async function CaptureScreenshot() {
  try {
    const originalPath = fileTreeStore.selectedFile.path
    console.log("Original path:", originalPath)

    // 获取平台分隔符（需要await）
    const pathSeparator = await sep()

    // 1. 解析路径
    const dirPath = originalPath.substring(0, originalPath.lastIndexOf(pathSeparator))
    const fileName = originalPath.substring(
      originalPath.lastIndexOf(pathSeparator) + 1,
      originalPath.lastIndexOf('.')
    )

    // 2. 创建 images 目录
    const imagesDir = `${dirPath}${pathSeparator}images`
    console.log("Images directory:", imagesDir)
    await invoke('create_screenshot_dir', { path: imagesDir })

    // 3. 构建新路径
    const screenshotPath = `${imagesDir}${pathSeparator}${fileName}.png`
    console.log("Screenshot will be saved to:", screenshotPath)

    // 4. 调用后端接口
    await invoke('take_screenshot', { path: screenshotPath })

    console.log("截图保存成功:", screenshotPath)
  } catch (err) {
    console.error('截图失败:', err)
  }
}

// 搜索相关状态
const isSearchDropdownVisible = ref(false)
const contextMenuPosition = ref({ x: 0, y: 0 })
const searchQuery = ref('') // 添加搜索查询变量
const searchResults = ref([]) // 添加搜索结果变量
const showSearchModal = ref(false) // 添加搜索模态框显示状态变量

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
    isSearchDropdownVisible.value = true
    const rect = e.target.getBoundingClientRect()
    contextMenuPosition.value = {
      x: rect.left,
      y: rect.bottom + 5
    }
  }
}
const handleSearchEnter = () => {
  if (searchQuery.value.trim()) {
    if (searchType.value === 'file') {
      handleSearchFiles()
    } else {
      handleSearchNotes()
    }
  }
}

const handleSearchSelect = (key) => {
  searchType.value = key
  isSearchDropdownVisible.value = false
  if (searchQuery.value.trim()) {
    handleSearchEnter()
  }
}

async function handleSearchFiles() {
  try {
    const results = await invoke('fd_search', {
      pattern: searchQuery.value,
      path: fileTreeStore.activeRepoPath
    })
    searchResults.value = results.map(item => ({
      path: item.path,
      fileName: item.path.split('/').pop(),
      type: 'file'
    }))
    showSearchModal.value = true
  } catch (err) {
    console.error('文件搜索失败:', err)
  }
}

async function handleSearchNotes() {
  try {
    const results = await invoke('rg_search', {
      pattern: searchQuery.value,
      path: `${fileTreeStore.activeRepoPath}/notes`
    })
    searchResults.value = results.map(item => ({
      path: item.path,
      preview: item.preview,
      lineNumber: item.line_number,
      type: 'note'
    }))
    showSearchModal.value = true
  } catch (err) {
    console.error('笔记搜索失败:', err)
  }
}

</script>

<style scoped>
.guaxi-header {
  display: flex;
  align-items: center;
  height: 35px;
  background-color: #f5f5f5;
  box-shadow: 0px 2px 4px rgba(0, 0, 0, 0.1);
  border-bottom: 2px solid #aab4ae;
  padding: 0 10px;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 200px;
}

.header-center {
  flex: 1;
  display: flex;
  justify-content: center;
  margin-top: 0px;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 200px;
  justify-content: flex-end;
}

.search-container {
  margin: 0;
  padding: 0;  /* 添加这一行确保没有内边距 */
  width: 100%;
  max-width: 400px;
}

.n-input {
  height: 25px !important;
  line-height: 25px !important;
  min-width: 200px;
  margin: 0 !important;  /* 确保外边距为0 */
  padding: 0 !important;  /* 确保内边距为0 */
}

.n-input .n-input__border {
  border: none !important;  /* 如果有边框影响间距 */
}

.n-input .n-input-wrapper {
  padding: 0 !important;  /* 确保输入框内部没有额外内边距 */
}

/* 调整下拉菜单样式 */
.n-dropdown-menu {
  margin-top: 4px !important;
}
</style>




<!-- <script>
import { defineComponent, computed, ref, onMounted, onBeforeUnmount } from "vue";
import { useSiderStore } from '../stores/useSiderStore'

export default defineComponent({
  // setup(props, { emit }) {
  setup() {
  // Reactive state variables
  // const isLeftSiderCollapsed = ref(false);
  // const isRightSiderCollapsed = ref(true);
  
  // // 更新左侧边栏的状态并向父组件发出事件
  // const toggleLeftSider = () => {
  //   isLeftSiderCollapsed.value = !isLeftSiderCollapsed.value;
  //   emit('update-left-sider', isLeftSiderCollapsed.value); // 通过 emit 向父组件发送更新的状态
  //   console.log("Header context isLeftSiderCollapsed", isLeftSiderCollapsed.value);
  // };

  // // 更新右侧边栏的状态并向父组件发出事件
  // const toggleRightSider = () => {
  //   isRightSiderCollapsed.value = !isRightSiderCollapsed.value;
  //   emit('update-right-sider', isRightSiderCollapsed.value); // 通过 emit 向父组件发送更新的状态
  //   console.log("Header context isRightSiderCollapsed", isRightSiderCollapsed.value);
  // };
  const siderStore = useSiderStore();
  const isLeftSiderCollapsed = computed(() => siderStore.isLeftSiderCollapsed);
  const isRightSiderCollapsed = computed(() => siderStore.isRightSiderCollapsed);

  const toggleLeftSider = () => {
    siderStore.toggleLeftSider()
  };

  const toggleRightSider = () => {
    siderStore.toggleRightSider()
  };

  return {
    isLeftSiderCollapsed,
    isRightSiderCollapsed,
    toggleLeftSider,
    toggleRightSider,
  };
},
});
</script> -->