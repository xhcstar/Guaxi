<template>
  <div class="main-content-wrapper">
    <!-- 顶部工具栏 -->
    <div class="top-toolbar">
      <n-space justify="space-between" align="center">
        <!-- 左侧按钮组 - 添加了padding-left和间距 -->
        <n-space class="toolbar-left" :size="16">
          <!-- 任务列表图标按钮 -->
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button text @click="toggleTaskList" :type="progresses.length > 0 ? 'primary' : 'default'">
                <template #icon>
                  <n-icon :component="TaskListIcon" />
                </template>
                <n-badge v-if="progresses.length > 0" :value="progresses.length" />
              </n-button>
            </template>
            转换任务列表
          </n-tooltip>

          <!-- 预留位置1 -->
          <div class="toolbar-placeholder"></div>

          <!-- 预留位置2 -->
          <div class="toolbar-placeholder"></div>
        </n-space>

        <!-- 右侧按钮组 -->
        <n-space>
          <!-- 其他按钮... -->
        </n-space>
      </n-space>
    </div>

    <!-- 任务列表抽屉 -->
    <n-drawer v-model:show="showTaskList" width="400" placement="right">
      <n-drawer-content title="文件转换任务" closable>
        <n-list hoverable>
          <n-list-item v-for="(progress, index) in progresses" :key="index" class="task-item"
            :class="{ 'task-item-completed': progress.current >= 100 }">
            <div class="task-content">
              <div class="task-header">
                <n-text strong class="task-filename">{{ progress.fileName }}</n-text>
                <n-tag v-if="progress.current < 100" size="small" type="info" class="task-tag">
                  转换中
                </n-tag>
                <n-tag v-else size="small" type="success" class="task-tag">
                  已完成
                </n-tag>
              </div>

              <n-progress type="line" :percentage="progress.current" :indicator-placement="'inside'"
                :status="progress.current >= 100 ? 'success' : 'info'" :height="15" class="task-progress" />

              <n-text depth="3" class="task-message">
                <n-icon size="14" class="task-message-icon">
                  <InfoCircleFilled />
                </n-icon>
                {{ progress.message }}
              </n-text>
            </div>
          </n-list-item>
        </n-list>
      </n-drawer-content>
    </n-drawer>

    <!-- 搜索栏 -->
    <div class="search-container">
      <n-card class="search-card" :bordered="false">
        <n-space vertical :size="16">
          <n-space align="center" justify="space-between">
            <h3 class="search-title">搜索</h3>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button text @click="toggleSearchPanel">
                  <n-icon :size="20">
                    <svg v-if="searchPanelCollapsed" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
                      fill="currentColor">
                      <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z" />
                    </svg>
                    <svg v-else xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
                      <path d="M19 13H5v-2h14v2z" />
                    </svg>
                  </n-icon>
                </n-button>
              </template>
              {{ searchPanelCollapsed ? '展开高级搜索' : '折叠高级搜索' }}
            </n-tooltip>
          </n-space>

          <!-- 默认显示关键字搜索栏 -->
          <n-input-group>
            <n-input v-model:value="searchParams.keyword" placeholder="输入关键字搜索" clearable>
              <template #prefix>
                <n-icon :component="SearchIcon" />
              </template>
            </n-input>
            <n-button type="primary" @click="handleSearch">
              搜索
            </n-button>
          </n-input-group>

          <!-- 高级搜索条件 - 点击展开后显示 -->
          <n-space v-show="!searchPanelCollapsed" vertical :size="12">
            <n-space :size="16" wrap>
              <n-select v-model:value="searchParams.library" :options="libraryOptions" placeholder="选择文件库" clearable
                filterable style="min-width: 180px" @search="handleLibrarySearch" />

              <n-select v-model:value="searchParams.tags" :options="importedFileTagOptions" placeholder="选择标签" clearable
                filterable multiple style="min-width: 180px" @search="handleImportedFileTagSearch" />

              <n-date-picker v-model:value="searchParams.dateRange" type="daterange" clearable placeholder="选择日期范围"
                style="min-width: 220px" />

              <n-select v-model:value="searchParams.knowledgeRepo" :options="knowledgeRepoOptions" placeholder="选择知识库"
                clearable filterable style="min-width: 180px" />

              <n-select v-model:value="searchParams.fileType" :options="fileTypeOptions" placeholder="选择文件类型" clearable
                style="min-width: 150px" />
            </n-space>
          </n-space>
        </n-space>
      </n-card>
    </div>

    <div class="toolbar-container">
      <n-space vertical :size="12" class="container">
        <!-- 工具栏 -->
        <n-space justify="space-between" align="center" class="toolbar">
          <!-- 左侧按钮组 -->
          <n-space class="actions">
            <n-button @click="sortTitle" class="action-button" secondary>
              <template #icon>
                <n-icon :component="SortAscendingIcon" />
              </template>
              按标题排序
            </n-button>
            <n-button @click="clearFilters" class="action-button" secondary>
              <template #icon>
                <n-icon :component="FilterOffIcon" />
              </template>
              清除筛选
            </n-button>
          </n-space>

          <!-- 右侧添加按钮 -->
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button type="primary" @click="showAddFileModal" class="add-path-button">
                <template #icon>
                  <n-icon :component="FolderAdd20Regular" />
                </template>
                导入文件
              </n-button>
            </template>
            导入新的文件
          </n-tooltip>
        </n-space>
      </n-space>
    </div>

    <div class="table-container">
      <n-data-table ref="table" :columns="columns" :data="filteredData" :max-height="tableHeight" virtual-scroll
        virtual-scroll-x :scroll-x="scrollX" :min-row-height="85" :height-for-row="heightForRow" :header-height="85"
        :pagination="pagination" :single-line="false" class="data-table" :row-props="rowProps" :multiple="true"
        :row-key="rowKey" @update:checked-row-keys="handleCheckboxUpdate" striped />
    </div>

    <!-- 右键菜单 -->
    <n-dropdown placement="bottom-start" trigger="manual" :x="x" :y="y" :options="contextMenuOptions"
      :show="showDropdown" :on-clickoutside="onClickoutside" @select="handleContextMenuSelect"
      class="custom-dropdown" />
  </div>

  <n-modal v-model:show="isAddFileModalVisible" title="添加文件" preset="dialog" :mask-closable="false" :closable="false">
    <n-form :model="currentForm" :rules="rules" label-placement="left" label-width="auto" label-align="right"
      size="medium" class="custom-form">
      <n-form-item v-for="column in formColumns" :key="column.key" :label="column.title" :path="column.key"
        :label-style="{ fontWeight: 'bold', color: '#333', fontSize: '15px' }">
        <!-- 根据列类型渲染不同的输入组件 -->
        <template v-if="column.key === 'filePath'">
          <n-input v-model:value="currentForm[column.key]" placeholder="请输入文件路径" :input-style="{ borderRadius: '8px' }">
            <template #suffix>
              <n-tooltip trigger="hover">
                <template #trigger>
                  <n-button text class="FolderAdd20Regular" @click="openFileBrowser" :style="{ color: '#409EFF' }">
                    <n-icon :component="FolderAdd20Regular" :size="20" />
                  </n-button>
                </template>
                选择导入文件
              </n-tooltip>
            </template>
          </n-input>
        </template>
        <template v-else-if="column.key === 'importFileCreateTime' || column.key === 'importFileName' || column.key === 'type'
          || column.key === 'importFileSize'">
          <n-input v-model:value="currentForm[column.key]" :placeholder="`${column.title}`" disabled
            :input-style="{ borderRadius: '8px' }" />
        </template>
        <template v-else-if="column.key === 'knowledgeRepo'">
          <n-input v-model:value="currentForm[column.key]" :placeholder="`${column.title}`"
            :input-style="{ borderRadius: '8px' }" />
        </template>
        <!-- 导入文件标签 -->
        <template v-else-if="column.key === 'importedFileTags'">
          <n-auto-complete v-model:value="currentForm[column.key]" :options="importedFileTagOptions"
            :loading="isImportedFileTagLoading" placeholder="请输入导入文件标签" clearable
            :input-style="{ borderRadius: '8px' }" />
        </template>
        <!-- 所属库 -->
        <template v-else-if="column.key === 'library'">
          <n-auto-complete v-model:value="currentForm[column.key]" :options="libraryOptions" :loading="isLibraryLoading"
            placeholder="请输入所属文件库" clearable :input-style="{ borderRadius: '8px' }" />
        </template>
        <template v-else>
          <n-input v-model:value="currentForm[column.key]"
            :placeholder="column.key === 'tags' ? `如有多个标签请以逗号隔开` : `请输入${column.title}`"
            :input-style="{ borderRadius: '8px' }" />
        </template>
      </n-form-item>
    </n-form>

    <!-- 对话框操作按钮 -->
    <template #action>
      <n-space justify="end">
        <n-button type="primary" @click="addFile" :style="{ backgroundColor: '#409EFF', borderColor: '#409EFF' }">
          提交
        </n-button>
        <n-button @click="isAddFileModalVisible = false"
          :style="{ backgroundColor: '#F56C6C', borderColor: '#F56C6C', color: '#FFF' }">
          取消
        </n-button>
      </n-space>
    </template>
  </n-modal>
</template>

<script setup>
import { ref, onMounted, onUnmounted, computed, h, nextTick, watch } from 'vue'
import { repoStore } from '../../stores/repoStore'
import { useEventStore } from '../../stores/eventsStore'
import {
  FolderAdd20Regular,
  Delete20Regular,
  Open20Regular,
  Edit20Regular,
  Share20Regular,
  ArrowMove24Regular,
} from '@vicons/fluent'
import { NIcon, NButton, NDropdown, useMessage } from 'naive-ui'
import ShowOrEdit from './ShowOrEdit.vue'
import {
  useNoteTagsSearch,
  useImportedFileTagsSearch,
  useImportedFileLibrarySearch,
  useImportedFilesSearch,
  useLibrariesSearch,
} from './SearchUtils/useSearch'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { platform } from '@tauri-apps/plugin-os';
import { List20Regular as TaskListIcon } from '@vicons/fluent'
import { listen } from '@tauri-apps/api/event';
import { InfoCircleFilled } from '@vicons/antd'

// 添加表格高度计算
const tableHeight = computed(() => {
  const headerHeight = 85 // 表头高度
  const rowHeight = 85 // 单行高度
  const pageSize = pagination.value.pageSize || 15
  return headerHeight + (rowHeight * pageSize)
})

// 添加任务列表状态
const showTaskList = ref(false)
const toggleTaskList = () => {
  showTaskList.value = !showTaskList.value
}

// 搜索图标
const SearchIcon = {
  render() {
    return h('svg', {
      xmlns: 'http://www.w3.org/2000/svg',
      viewBox: '0 0 24 24',
      fill: 'currentColor'
    }, [
      h('path', {
        d: 'M15.5 14h-.79l-.28-.27a6.5 6.5 0 0 0 1.48-5.34c-.47-2.78-2.79-5-5.59-5.34a6.505 6.505 0 0 0-7.27 7.27c.34 2.8 2.56 5.12 5.34 5.59a6.5 6.5 0 0 0 5.34-1.48l.27.28v.79l4.25 4.25c.41.41 1.08.41 1.49 0 .41-.41.41-1.08 0-1.49L15.5 14zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z'
      })
    ]);
  }
};

// 排序图标
const SortAscendingIcon = {
  render() {
    return h('svg', {
      xmlns: 'http://www.w3.org/2000/svg',
      viewBox: '0 0 24 24',
      fill: 'currentColor'
    }, [
      h('path', {
        d: 'M19 17h3l-4 4-4-4h3V3h2v14zM9 13H5v-2h4V9H5V7h4V5H5V3H3v12h2v-2h4v-2zm2-2h2v2h-2v-2zm0-4h2v2h-2V7zm0 8h2v2h-2v-2z'
      })
    ]);
  }
};

// 清除筛选图标
const FilterOffIcon = {
  render() {
    return h('svg', {
      xmlns: 'http://www.w3.org/2000/svg',
      viewBox: '0 0 24 24',
      fill: 'currentColor'
    }, [
      h('path', {
        d: 'M19.79 5.61C20.3 4.95 19.83 4 19 4H6.83l7.97 7.97 4.99-6.36zM2.81 2.81L1.39 4.22 10 13v6c0 .55.45 1 1 1h2c.55 0 1-.45 1-1v-2.17l5.78 5.78 1.41-1.41L2.81 2.81z'
      })
    ]);
  }
};

// 引入 Pinia store
const store = repoStore()
const eventStore = useEventStore();

// 获取 message 实例
const message = useMessage();

// 搜索相关状态
const searchParams = ref({
  keyword: '',
  library: null,
  tags: [],
  dateRange: null,
  knowledgeRepo: null,
  fileType: null
});

const searchPanelCollapsed = ref(true);
const toggleSearchPanel = () => {
  searchPanelCollapsed.value = !searchPanelCollapsed.value;
};

// 文件类型选项
const fileTypeOptions = [
  { label: 'PDF', value: 'pdf' },
  { label: 'Markdown', value: 'md' },
  { label: 'HTML', value: 'html' },
  { label: 'Word', value: 'docx' },
  { label: 'Text', value: 'txt' },
  { label: 'EPUB', value: 'epub' }
];

// 知识库选项
const knowledgeRepoOptions = computed(() => {
  return store.repos.map(repo => ({
    label: repo.name,
    value: repo.name
  }));
});

// 过滤后的数据
const filteredData = computed(() => {
  let data = currentData.value;

  // 关键字搜索
  if (searchParams.value.keyword) {
    const keyword = searchParams.value.keyword.toLowerCase();
    data = data.filter(item =>
      item.importFileName?.toLowerCase().includes(keyword) ||
      item.description?.toLowerCase().includes(keyword) ||
      item.importedFileTags?.toLowerCase().includes(keyword) ||
      item.newNoteForm?.description?.toLowerCase().includes(keyword) ||
      item.newNoteForm?.notesTags?.toLowerCase().includes(keyword)
    );
  }

  // 文件库筛选
  if (searchParams.value.library) {
    data = data.filter(item => item.library === searchParams.value.library);
  }

  // 标签筛选
  if (searchParams.value.tags && searchParams.value.tags.length > 0) {
    data = data.filter(item => {
      const itemTags = item.importedFileTags?.split(',') || [];
      return searchParams.value.tags.some(tag => itemTags.includes(tag));
    });
  }

  // 日期范围筛选
  if (searchParams.value.dateRange && searchParams.value.dateRange.length === 2) {
    const [start, end] = searchParams.value.dateRange;
    data = data.filter(item => {
      const itemDate = new Date(item.importFileCreateTime).getTime();
      return itemDate >= start && itemDate <= end;
    });
  }

  // 知识库筛选
  if (searchParams.value.knowledgeRepo) {
    data = data.filter(item => item.knowledgeRepo === searchParams.value.knowledgeRepo);
  }

  // 文件类型筛选
  if (searchParams.value.fileType) {
    data = data.filter(item => item.type === searchParams.value.fileType);
  }

  return data;
});

const handleSearch = () => {
  message.success('搜索完成，找到 ' + filteredData.value.length + ' 条结果');
};

// 添加文件对话框状态
const isAddFileModalVisible = ref(false)
// 表格实例
const tableRef = ref(null)
// 分页配置
const pagination = ref({ pageSize: 15 })

let unsubscribe = null;

const newNoteForm = {
  id: 0,
  filePath: '',
  description: '',
  noteFileName: '',
  notesTags: '', // 所属库名字
  fromImportedFile: '',
  fromImportedFileLibrary: '',
  noteFileType: '',
  noteFileCreateTime: '',
  noteFileUpdateTime: '', // 笔记特有的字段
  knowledgeRepo: '', // 知识库名字
}

// 用于 importFileDB 库的表单
const newFileForm = ref({
  id: 0,
  filePath: '',
  importFileName: '',
  description: '',
  importedFileTags: '',
  type: '',
  library: '', // 所属库名字
  importFileCreateTime: '',
  importFileSize: 0,
  convetToMarkdownFilename: '',
  knowledgeRepo: '', // 知识库名字
  newNoteForm: newNoteForm,
})

// 当前表单（根据数据库类型动态切换）
const currentForm = computed(() => {
  return newFileForm.value
})

// 进度条相关状态
const progresses = ref([])
const startProgressListener = async (filePath) => {
  const fileName = filePath.split(/[\\/]/).pop() || '未知文件'

  const newProgress = {
    show: true,
    current: 0,
    total: 100,
    message: '开始转换...',
    fileName,
  }

  progresses.value.push(newProgress)
  const progressIndex = progresses.value.length - 1

  const unlisten = await listen('conversion_progress', (event) => {
    progresses.value[progressIndex] = {
      ...progresses.value[progressIndex],
      current: event.payload.current,
      message: event.payload.message
    }

    // 转换完成后3秒自动移除
    if (event.payload.current >= 100) {
      setTimeout(() => {
        if (progresses.value[progressIndex]?.current >= 100) {
          progresses.value.splice(progressIndex, 1)
        }
      }, 3000)
    }
  })

  return unlisten
}

const convertFile = async (inputPath, outputDir) => {
  try {
    await invoke('convert_to_markdown', {
      request: {
        input_path: inputPath,
        output_dir: outputDir
      }
    }, {
      target: 'main'
    })
  } catch (error) {
    console.error('Conversion failed:', error)
    throw error
  }
}

// 自定义校验函数：检查文件路径是否合法（根据系统类型动态选择匹配方式）
const validateFilePath = async (rule, value) => {
  console.log('validateFilePath', value); // 添加日志

  // 检查是否为空
  if (!value || value.trim() === '') {
    return new Error('文件路径不能为空');
  }

  // 获取当前操作系统类型
  const osPlatform = await platform();

  // Windows 路径正则表达式
  const windowsPathRegex = /^([a-zA-Z]:\\|\\\\)([^\\/:*?"<>|\r\n]+\\?)*$/;
  // Linux 路径正则表达式
  const linuxPathRegex = /^\/([^\/:*?"<>|\r\n]+\/?)*$/;
  // macOS 路径正则表达式（与 Linux 相同）
  const macPathRegex = /^\/([^\/:*?"<>|\r\n]+\/?)*$/;

  console.log('osPlatform', osPlatform);
  // 根据系统类型选择匹配方式
  if (osPlatform === 'windows') {
    if (!windowsPathRegex.test(value)) {
      return new Error('Windows 文件路径不合法，请检查路径格式');
    }
  } else if (osPlatform === 'linux') {
    if (!linuxPathRegex.test(value)) {
      return new Error('Linux 文件路径不合法，请检查路径格式');
    }
  } else if (osPlatform === 'macos') {
    if (!macPathRegex.test(value)) {
      return new Error('macOS 文件路径不合法，请检查路径格式');
    }
  } else {
    return new Error('不支持的操作系统');
  }

  return true; // 校验通过
};

// 自定义校验函数：检查标签是否合法（单个字符串或以逗号分隔的字符串）
const validateTags = (rule, value) => {
  console.log('validateTags', value); // 添加日志

  // 检查是否为空
  if (!value || value.trim() === '') {
    return new Error('标签不能为空');
  }

  // 检查是否为单个字符串或以逗号分隔的字符串
  const tags = value.split(',').map(tag => tag.trim());
  if (tags.some(tag => !tag)) {
    return new Error('标签不能为空或以多个逗号分隔');
  }

  return true; // 校验通过
};

// 校验表单
const validateForm = async () => {
  const form = currentForm.value;
  console.log('>>>>>>>>>>>> validateForm', currentForm.value);

  // 校验文件路径
  if (!form.filePath || form.filePath.trim() === '') {
    return '文件路径不能为空';
  }

  // 获取当前操作系统类型
  const osPlatform = await platform();

  console.log('osPlatform', osPlatform);
  // Windows 路径正则表达式
  const windowsPathRegex = /^([a-zA-Z]:\\|\\\\)([^\\/:*?"<>|\r\n]+\\?)*$/;
  // Linux 路径正则表达式
  const linuxPathRegex = /^\/([^\/:*?"<>|\r\n]+\/?)*$/;
  // macOS 路径正则表达式（与 Linux 相同）
  const macPathRegex = /^\/([^\/:*?"<>|\r\n]+\/?)*$/;

  // 根据系统类型选择匹配方式
  if (osPlatform === 'windows') {
    if (!windowsPathRegex.test(form.filePath)) {
      return 'Windows 文件路径不合法，请检查路径格式';
    }
  } else if (osPlatform === 'linux') {
    if (!linuxPathRegex.test(form.filePath)) {
      return 'Linux 文件路径不合法，请检查路径格式';
    }
  } else if (osPlatform === 'macos') {
    if (!macPathRegex.test(form.filePath)) {
      return 'macOS 文件路径不合法，请检查路径格式';
    }
  } else {
    return '不支持的操作系统';
  }

  // 校验标签
  if (!form.importedFileTags || form.importedFileTags.trim() === '') {
    console.log('>>>>>>>>>>>>> form', form.importedFileTags);
    console.log('>>>>>>>>>>>>> form.importedFileTags', form.importedFileTags);
    currentForm.value.importedFileTags = "default"; // 设置默认值
  } else {
    const tags = form.importedFileTags.split(',').map(tag => tag.trim());
    if (tags.some(tag => !tag)) {
      return '多个标签没有以逗号分隔';
    }
  }

  // 校验描述
  if (!form.description || form.description.trim() === '') {
    return '描述不能为空';
  }
  if (form.description.length > 240) {
    return '描述不能超过 240 个字符';
  }

  // 校验所属库
  if (!form.library || (typeof form.library === 'string' && form.library.trim() === '')) {
    currentForm.value.library = 'default'; // 设置默认值
    return '所属库不能为空';
  }

  // 校验入库时间
  if (!form.importFileCreateTime || form.importFileCreateTime.trim() === '') {
    return '入库时间不能为空';
  }

  // 校验文件大小
  if (!form.importFileSize || form.importFileSize <= 0) {
    return '文件大小不能为空';
  }

  // 校验通过
  return null;
};

// 校验table cell
const validateTableCell = (row) => {
  console.log('validateTableCell', row);
  console.log('store.currentDatabase', store.currentDatabase);

  // 校验标签
  if (!row.importedFileTags || row.importedFileTags.trim() === '') {
    console.log('row.importedFileTags', row.importedFileTags);
    row.importedFileTags = "default"; // 设置默认值
  } else {
    const tags = row.importedFileTags.split(',').map(tag => tag.trim());
    if (tags.some(tag => !tag)) {
      return '多个标签没有以逗号分隔';
    }
  }

  // 校验描述
  if (!row.description || row.description.trim() === '') {
    return '描述不能为空';
  }
  if (row.description.length > 240) {
    return '描述不能超过 240 个字符';
  }

  // 校验所属库
  if (!row.library || (typeof row.library === 'string' && row.library.trim() === '')) {
    row.library = 'default'; // 设置默认值
    return '所属库不能为空';
  }

  // 校验入库时间
  if (!row.importFileCreateTime || row.importFileCreateTime.trim() === '') {
    return '入库时间不能为空';
  }

  // 校验文件大小
  if (!row.importFileSize || row.importFileSize <= 0) {
    return '文件大小不能为空';
  }

  // 校验通过
  return null;
};

// 表单校验规则
const rules = {
  filePath: [
    { validator: validateFilePath, required: true, message: '文件路径不能为空', trigger: ['input', 'blur'] },
  ],
  importedFileTags: [
    { validator: validateTags, required: true, message: '标签不能为空', trigger: ['input', 'blur'] },
  ],
  description: [
    { required: true, message: '描述不能为空', trigger: ['input', 'blur'] },
    { max: 240, message: '描述不能超过240个字符', trigger: ['input', 'blur'] },
  ],
  library: [
    { required: true, message: '所属库不能为空', trigger: ['input', 'blur'] },
  ],
  importFileCreateTime: [
    { required: true, message: '入库时间不能为空', trigger: ['input', 'blur'] },
  ],
  importFileSize: [
    { required: true, message: '文件大小不能为空', trigger: ['input', 'blur'] },
  ],
  knowledgeRepo: [
    { required: true, message: '知识库不能为空', trigger: ['input', 'blur'] },
  ],
};

// 动态生成表单列配置
const formColumns = computed(() => {
  const columns = [
    { key: 'filePath', title: '文件路径' },
    { key: 'importFileName', title: '文件名' },
    { key: 'description', title: '文件描述' },
    { key: 'knowledgeRepo', title: '知识库' },
    { key: 'library', title: '文件库' },
    { key: 'importedFileTags', title: '文件标签' },
    { key: 'type', title: '文件类型' },
    { key: 'importFileCreateTime', title: '入库时间' },
    { key: 'importFileSize', title: '文件大小' },
  ]
  return columns
})

// 打开文件浏览器并获取文件信息
const openFileBrowser = async () => {
  try {
    const filePath = await open({
      filters: [{ name: 'All Files', extensions: ['*'] }],
    })
    if (filePath) {
      // 更新文件路径
      currentForm.value.filePath = filePath

      // 调用后台接口获取文件信息
      const fileInfo = await invoke('get_file_info', { filePath })
      console.log('File Info:', fileInfo)

      // 根据当前数据库类型更新表单数据
      currentForm.value.importFileName = fileInfo.file_name
      currentForm.value.type = fileInfo.file_type
      currentForm.value.importFileCreateTime = fileInfo.create_time
      currentForm.value.importFileSize = fileInfo.file_size
    }
  } catch (error) {
    console.error('Error opening file browser:', error)
  }
}

// 显示添加文件对话框
const showAddFileModal = () => {
  isAddFileModalVisible.value = true
}

const createImportFileList = (addedNoteInfo, currentForm) => {
  return {
    import_file_name: currentForm.importFileName,
    description: currentForm.description,
    library: currentForm.library,
    tags: currentForm.importedFileTags.split(','),
    file_type: currentForm.type,
    import_file_create_time: currentForm.importFileCreateTime,
    import_file_size: currentForm.importFileSize,
    converted_markdown_filename: `${currentForm.importFileName.replace(/\.[^/.]+$/, '')}.md`,
    knowledge_repo: currentForm.knowledgeRepo,
    note_file_input: {
      description: `${addedNoteInfo.from_filename}的笔记`,
      note_file_name: addedNoteInfo.note_file_name,
      tags: currentForm.importedFileTags.split(','), //导入文件时创建的笔记使用导入文件的标签
      note_file_type: 'md',
      from_imported_file: addedNoteInfo.from_filename,
      from_imported_file_library: addedNoteInfo.library,
      note_file_create_time: addedNoteInfo.create_time,
      note_file_update_time: addedNoteInfo.update_time,
      knowledge_repo: currentForm.knowledgeRepo,
    }
  }
};

// 提取添加文件到数据库的逻辑
const addFileToDatabase = async (fileInfo) => {
  try {
    console.log('Adding info into table imported_files database:', fileInfo);
    const fileId = await invoke('add_imported_file', {
      file: fileInfo,
    });
    console.log('File info successfully added to database, id: ', fileId);
    return fileId;
  } catch (error) {
    console.error('Error adding info into table imported_files database:', error);
    throw error;
  }
};

// 提取添加笔记到数据库的逻辑
const addNoteToDatabase = async (noteInfo) => {
  try {
    console.log('Adding info into table notes database:', noteInfo);
    const noteId = await invoke('add_note', {
      note: noteInfo,
    });
    console.log('Note info successfully added to database, id: ', noteId);
    return noteId;
  } catch (error) {
    console.error('Error adding info into table notes database:', error);
    throw error;
  }
};

// 构建文件路径
const buildConvertOutputFilePath = async (fileName, library) => {
  const fileNameWithoutExt = fileName.replace(/\.[^/.]+$/, "");
  const osPlatform = await platform(); // 使用Tauri API获取操作系统类型

  const basePath = osPlatform === 'windows'
    ? `${store.librariesRoot}\\Libraries\\${library}\\${fileNameWithoutExt}\\markdown`
    : `${store.librariesRoot}/Libraries/${library}/${fileNameWithoutExt}/markdown`;

  const fullPath = osPlatform === 'windows'
    ? `${basePath}\\${fileNameWithoutExt}\\${fileNameWithoutExt}.md`
    : `${basePath}/${fileNameWithoutExt}/${fileNameWithoutExt}.md`;

  console.log('basePath:', basePath);
  console.log('fullPath:', fullPath);

  return {
    outputDir: basePath,
    fileToCheck: fullPath
  };
};

const addFileFromWeb = async () => {
  try {
    // 校验表单
    const validationError = await validateForm(); // 等待 validateForm 的结果
    if (validationError) {
      message.error(validationError); // 确保 validationError 是字符串
      return; // 终止添加
    }

    if (!currentForm.value.library || !currentForm.value.importFileName) {
      message.error('Library or importFileName path is missing: ', currentForm.value);
      return;
    }

    // 调用 Tauri 后端接口，将文件拷贝到 Libraries 目录下
    const addedNoteInfo = await invoke('add_web_clipper_import_file', {
      library: currentForm.value.library,
      filePath: currentForm.value.filePath,
    });

    // 创建 importfileList
    const importfileList = createImportFileList(addedNoteInfo, currentForm.value);

    // 将 importfileList 写入数据库
    console.log('importfileList:', importfileList);
    const fileId = await addFileToDatabase(importfileList);

    // 添加导入的文件到 store
    store.addImportedFile({
      id: fileId,
      importFileName: currentForm.value.importFileName,
      description: currentForm.value.description,
      importedFileTags: currentForm.value.importedFileTags,
      type: currentForm.value.type,
      library: currentForm.value.library,
      importFileCreateTime: currentForm.value.importFileCreateTime,
      importFileSize: currentForm.value.importFileSize,
      convetToMarkdownFilename: `${currentForm.value.importFileName.replace(/\.[^/.]+$/, "")}.md`,
      knowledgeRepo: currentForm.value.knowledgeRepo,
      newNoteForm: {
        description: `${addedNoteInfo.from_filename}的笔记`,
        noteFileName: addedNoteInfo.note_file_name,
        notesTags: currentForm.value.importedFileTags.split(','), //导入文件时创建的笔记使用导入文件的标签
        noteFileType: 'md',
        fromImportedFile: addedNoteInfo.from_filename,
        fromImportedFileLibrary: addedNoteInfo.library,
        noteFileCreateTime: addedNoteInfo.create_time,
        noteFileUpdateTime: addedNoteInfo.update_time,
        knowledgeRepo: currentForm.value.knowledgeRepo,
      }
    });

    // 转换完成后刷新数据
    console.log('add_file currentDatabase', store.currentDatabase);
    await store.fetchRepos()
    await store.fetchFiles(store.currentDatabase)

    // 关闭对话框并清空表单
    isAddFileModalVisible.value = false;
    resetForm();
  } catch (error) {
    console.error('Error adding file:', error);
  }
};

const addFile = async () => {
  try {
    // 校验表单
    const validationError = await validateForm(); // 等待 validateForm 的结果
    if (validationError) {
      message.error(validationError); // 确保 validationError 是字符串
      return; // 终止添加
    }

    if (!currentForm.value.library || !currentForm.value.importFileName) {
      message.error('Library or importFileName path is missing: ', currentForm.value);
      return;
    }

    // 调用 Tauri 后端接口，将文件拷贝到 Libraries 目录下
    const addedNoteInfo = await invoke('add_repo_import_file', {
      library: currentForm.value.library,
      filePath: currentForm.value.filePath,
    }).catch(error => {
      // 处理不同的错误类型
      let errorMessage = 'Failed to import file';
      if (error.includes('Unsupported file type')) {
        errorMessage = 'Unsupported file type';
      }
      message.error(errorMessage);
      throw new Error(errorMessage);
    });

    // 创建 importfileList
    const importfileList = createImportFileList(addedNoteInfo, currentForm.value);

    // 将 importfileList 写入数据库
    console.log('importfileList:', importfileList);
    const fileId = await addFileToDatabase(importfileList);

    // 添加导入的文件到 store
    store.addImportedFile({
      id: fileId,
      importFileName: currentForm.value.importFileName,
      description: currentForm.value.description,
      importedFileTags: currentForm.value.importedFileTags,
      type: currentForm.value.type,
      library: currentForm.value.library,
      importFileCreateTime: currentForm.value.importFileCreateTime,
      importFileSize: currentForm.value.importFileSize,
      convetToMarkdownFilename: `${currentForm.value.importFileName.replace(/\.[^/.]+$/, "")}.md`,
      knowledgeRepo: currentForm.value.knowledgeRepo,
      newNoteForm: {
        description: `${addedNoteInfo.from_filename}的笔记`,
        noteFileName: addedNoteInfo.note_file_name,
        notesTags: currentForm.value.importedFileTags.split(','), //导入文件时创建的笔记使用导入文件的标签
        noteFileType: 'md',
        fromImportedFile: addedNoteInfo.from_filename,
        fromImportedFileLibrary: addedNoteInfo.library,
        noteFileCreateTime: addedNoteInfo.create_time,
        noteFileUpdateTime: addedNoteInfo.update_time,
        knowledgeRepo: currentForm.value.knowledgeRepo,
      }
    });

    // 转换完成后刷新数据
    console.log('add_file currentDatabase', store.currentDatabase);
    await store.fetchRepos()
    await store.fetchFiles(store.currentDatabase)

    try {
      // 构建输出目录路径
      const { outputDir, fileToCheck } = await buildConvertOutputFilePath(
        currentForm.value.importFileName,
        currentForm.value.library
      );

      // 检查文件是否已转换
      const isConverted = await invoke('check_file_converted', {
        outputDir: fileToCheck
      });

      console.log('isConverted', isConverted);
      console.log('addedNoteInfo', addedNoteInfo);
      console.log('currentForm: ', currentData.value);

      if (isConverted || addedNoteInfo.notConverted) {
        console.log('isConverted', isConverted);
        console.log('convetToMarkdownFilename: ', currentForm.value.convetToMarkdownFilename);
        message.success('已存在转换文件，无需转换');
        // 关闭对话框并清空表单
        isAddFileModalVisible.value = false;
        resetForm();
        return;
      }

      // 开始监听进度事件
      const unlisten = await startProgressListener(currentForm.value.filePath)

      // 立即关闭表单对话框
      isAddFileModalVisible.value = false

      // 开始转换流程
      await convertFile(currentForm.value.filePath, outputDir)

      // 转换完成后刷新数据
      console.log('add_file currentDatabase', store.currentDatabase);
      await store.fetchFiles(store.currentDatabase)
      resetForm()

      // 转换完成后取消监听
      unlisten()
    } catch (error) {
      // 更新进度条状态为失败
      const fileName = currentForm.value.filePath.split(/[\\/]/).pop() || '未知文件'
      const progressIndex = progresses.value.findIndex(p => p.fileName === fileName)
      if (progressIndex !== -1) {
        progresses.value[progressIndex] = {
          ...progresses.value[progressIndex],
          message: `转换失败: ${error.message || error}`,
          current: 100
        }
        // 3秒后自动移除失败的进度条
        setTimeout(() => {
          progresses.value = progresses.value.filter((_, i) => i !== progressIndex)
        }, 3000)
      }
      message.error(`添加文件失败: ${error}`)
    }

    // 关闭对话框并清空表单
    isAddFileModalVisible.value = false;
    resetForm();
  } catch (error) {
    console.error('Error adding file:', error);
  }
};

// 更新导入文件
const updateImportedFile = async (row) => {
  try {
    // 将 tags 字段转换为数组
    const importedFileTags = typeof row.importedFileTags === 'string' ? row.importedFileTags.split(',') : row.importedFileTags;

    console.log('updateImportedFile row:', row);
    // 确保 file 是一个对象
    const file = {
      import_file_name: row.importFileName,
      description: row.description,
      library: row.library,
      tags: importedFileTags,
      file_type: row.type,
      import_file_create_time: row.importFileCreateTime,
      import_file_size: row.importFileSize,
      converted_markdown_filename: `${row.importFileName.replace(/\.[^/.]+$/, '')}.md`,
      knowledge_repo: row.knowledgeRepo,
      note_file_input: {
        description: `${row.newNoteForm.description}`,
        note_file_name: row.newNoteForm.noteFileName,
        tags: row.newNoteForm.notesTags.split(','), // 使用新表单中的标签
        note_file_type: 'md',
        from_imported_file: row.importFileName,
        from_imported_file_library: row.library,
        note_file_create_time: row.newNoteForm.noteFileCreateTime,
        note_file_update_time: row.newNoteForm.noteFileUpdateTime,
        knowledge_repo: row.newNoteForm.knowledgeRepo,
      }
    };

    // 打印日志：row.id 和 file
    console.log('row.id:', row.id);
    console.log('file:', JSON.stringify(file, null, 2));

    // 调用后端接口
    await invoke('update_imported_file', {
      id: row.id,
      file: file, // 传递正确的 file 对象
    });
    console.log('更新导入文件成功:', row);
  } catch (error) {
    console.error('更新导入文件失败:', error);
  }
};

// 更新笔记
const updateNote = async (row) => {
  try {
    // 将 tags 字段转换为数组
    const notesTags = typeof row.notesTags === 'string' ? row.notesTags.split(',') : row.notesTags;

    // 确保 note 是一个对象
    const note = {
      description: row.description,
      note_file_name: row.noteFileName,
      note_file_type: row.noteFileType,
      from_imported_file: row.fromImportedFile,
      from_imported_file_library: row.fromImportedFileLibrary,
      note_file_create_time: row.noteFileCreateTime,
      note_file_update_time: row.noteFileUpdateTime,
      tags: notesTags, // 传递转换后的 tags 数组
    };

    // 打印日志：row.id 和 note
    console.log('row.id:', row.id);
    console.log('note:', JSON.stringify(note, null, 2));

    // 调用后端接口
    await invoke('update_note', {
      id: row.id,
      note: note, // 传递正确的 note 对象
    });
    console.log('更新笔记成功:', row);
  } catch (error) {
    console.error('更新笔记失败:', error);
  }
};

// 清空表单
const resetForm = () => {
  newFileForm.value = {
    id: 0,
    importFileName: '',
    description: '',
    importedFileTags: '',
    type: '',
    library: '', // 所属库名字
    importFileCreateTime: '',
    importFileSize: 0,
    convetToMarkdownFilename: '',
    knowledgeRepo: '',
    newNoteForm: {
      id: 0,
      description: '',
      noteFileName: '',
      notesTags: '', // 所属库名字
      fromImportedFile: '',
      fromImportedFileLibrary: '',
      noteFileType: '',
      noteFileCreateTime: '',
      noteFileUpdateTime: '', // 笔记特有的字段
      knowledgeRepo: '',
    }
  }
}

// 根据当前数据库类型选择数据
const currentData = computed(() => {
  console.log('store.files:', store.files)
  return store.files
})

// 右键菜单状态
const showDropdown = ref(false)
const x = ref(0)
const y = ref(0)
const selectedRow = ref(null)

// 右键菜单选项
const contextMenuOptions = [
  {
    label: '打开',
    key: 'open',
    icon: () => h(NIcon, { component: Open20Regular, size: 16 }),
  },
  {
    label: '打开导入文件所在目录',
    key: 'openDirImport',
    icon: () => h(NIcon, { component: Open20Regular, size: 16 }),
  },
  {
    label: '打开笔记文件所在目录',
    key: 'openDirNote',
    icon: () => h(NIcon, { component: Open20Regular, size: 16 }),
  },
  {
    label: '编辑',
    key: 'edit',
    icon: () => h(NIcon, { component: Edit20Regular, size: 16 }),
  },
  {
    label: '分享',
    key: 'share',
    icon: () => h(NIcon, { component: Share20Regular, size: 16 }),
  },
  {
    label: '删除',
    key: 'delete',
    icon: () => h(NIcon, { component: Delete20Regular, size: 16 }),
  },
  {
    label: '删除该条目及相关文件',
    key: 'deleteAll',
    icon: () => h(NIcon, { component: Delete20Regular, size: 16 }),
  },
  {
    label: '移动',
    key: 'move',
    icon: () => h(NIcon, { component: ArrowMove24Regular, size: 16 }),
  },
  {
    label: '导出',
    key: 'export',
    icon: () => h(NIcon, { component: ArrowMove24Regular, size: 16 }),
  },
]

// 处理右键菜单点击
const handleContextMenuSelect = (key) => {
  if (!selectedRow.value) return

  switch (key) {
    case 'open':
      handleOpen(selectedRow.value)
      break
    case 'edit':
      handleEdit(selectedRow.value)
      break
    case 'share':
      handleShare(selectedRow.value)
      break
    case 'delete':
      handleDelete(selectedRow.value)
      break
    case 'move':
      handleMove(selectedRow.value)
      break
    case 'deleteAll':
      handleDeleteAll(selectedRow.value)
      break
    case 'export':
      handleExport(selectedRow.value)
      break
    case 'openDirImport':
      handleOpenDirImport(selectedRow.value)
      break
    case 'openDirNote':
      handleOpenDirNote(selectedRow.value)
      break
    default:
      console.warn('未知操作:', key)
  }

  showDropdown.value = false
}

// 处理点击外部关闭菜单
const onClickoutside = () => {
  showDropdown.value = false
}

// 行属性配置：右键点击触发菜单
const rowProps = (row) => {
  return {
    onContextmenu: (e) => {
      e.preventDefault()
      selectedRow.value = row
      showDropdown.value = false
      nextTick(() => {
        showDropdown.value = true
        x.value = e.clientX
        y.value = e.clientY
      })
    },
  }
}

// 方法：打开文件
const handleOpen = async (row) => {
  // 1. 确定文件类型和文件名
  const rowFileType = row.type?.toLowerCase() || ''

  const rowFileName = row.importFileName || ''

  // 2. 映射文件类型
  const fileType = mapFileType(rowFileType)
  if (!fileType) {
    console.warn('未知文件类型:', rowFileType, row)
    return
  }

  // 获取不带后缀的文件名
  const baseFileName = rowFileName.includes('.')
    ? rowFileName.substring(0, rowFileName.lastIndexOf('.'))
    : rowFileName

  // 3. 准备窗口参数
  const windowLabel = `reader-${row.id}-${Date.now()}` // 唯一窗口标识
  const windowTitle = `${rowFileName || '未命名文件'}` // 使用不带后缀的文件名

  const osPlatform = await platform()
  let ReadFilePath = ''

  console.log('store.librariesRoot:', store.librariesRoot)

  if (store.currentDatabase.length > 0) {
    if (osPlatform === 'windows') {
      ReadFilePath = `${store.librariesRoot}\\Libraries\\${row.library}\\${baseFileName}\\${fileType}\\${rowFileName}`
    } else if (osPlatform === 'linux') {
      ReadFilePath = `${store.librariesRoot}/Libraries/${row.library}/${baseFileName}/${fileType}/${rowFileName}`
    } else {
      console.error('未知的操作系统平台:', osPlatform)
    }
  } else {
    console.error('未知的数据库类型:', store.currentDatabase)
  }

  try {
    // 4. 调用 Rust 命令创建窗口
    await invoke('create_reader_window', {
      label: windowLabel,
      title: windowTitle,
      fileData: {
        file_type: fileType,
        file_name: rowFileName,
        file_path: ReadFilePath,
        library: row.library,
      }
    })

    console.log('窗口创建参数:', {
      label: windowLabel,
      title: windowTitle,
      fileData: {
        type: fileType,
        name: rowFileName,
        path: ReadFilePath,
        library: row.library,
      }
    })
  } catch (error) {
    console.error('创建窗口失败:', error)
    alert(`无法打开文件: ${error.message || '未知错误'}`)
  }
}

// 文件类型映射函数
function mapFileType(rawType) {
  const typeMap = {
    'pdf': 'pdf',
    'md': 'markdown',
    'markdown': 'markdown',
    'html': 'html',
    'htm': 'html',
    'epub': 'epub',
    'doc': 'word',
    'docx': 'word',
    'word': 'word',
    'txt': 'txt'
  }
  return typeMap[rawType] || null
}

const handleDelete = async (row) => {
  try {
    // 调用删除导入文件的接口
    await invoke('delete_imported_file', { id: row.id });
    console.log('删除导入文件成功', row);

    // 刷新数据
    await store.fetchFiles(store.currentDatabase);
    await store.fetchRepos()
  } catch (error) {
    console.error('删除失败:', error);
  }
};

const handleDeleteAll = async (row) => {
  if (!row.importFileName || !row.library) {
    message.error('缺少必要信息：文件名或库名');
    return;
  }

  // 获取不带后缀的文件名
  const baseFileName = row.importFileName.includes('.')
    ? row.importFileName.substring(0, row.importFileName.lastIndexOf('.'))
    : row.importFileName;

  const osPlatform = await platform();
  let dirPath = '';

  if (osPlatform === 'windows') {
    dirPath = `${store.librariesRoot}\\Libraries\\${row.library}\\${baseFileName}`;
  } else {
    dirPath = `${store.librariesRoot}/Libraries/${row.library}/${baseFileName}`;
  }

  try {
    // 调用删除导入文件的接口
    await invoke('delete_imported_all_file', { id: row.id, path: dirPath });
    console.log('删除导入文件成功', row);

    // 刷新数据
    await store.fetchFiles(store.currentDatabase);
    await store.fetchRepos()
  } catch (error) {
    console.error('删除失败:', error);
  }
}

// 方法：编辑文件或笔记
const handleEdit = async (row) => {
  try {
    if (store.currentDatabase === 'importFileDB') {
      // 调用更新导入文件的接口
      const updatedFile = {
        id: row.id,
        import_file_name: row.importFileName,
        description: row.description,
        library: row.library,
        file_type: row.type,
        import_file_create_time: row.importFileCreateTime,
        import_file_size: row.importFileSize,
        tags: row.importedFileTags.split(','), // 将字符串转换为数组
      };
      await invoke('update_imported_file', { id: row.id, file: updatedFile });
      console.log('更新导入文件成功', row);
    } else {
      // 调用更新笔记的接口
      const updatedNote = {
        id: row.id,
        description: row.description,
        note_file_name: row.noteFileName,
        note_file_type: row.type,
        from_imported_file: row.fromImportedFile,
        from_imported_file_library: row.fromImportedFileLibrary,
        note_file_create_time: row.noteFileCreateTime,
        note_file_update_time: row.noteFileUpdateTime,
        tags: row.notesTags.split(','), // 将字符串转换为数组
      };
      await invoke('update_note', { id: row.id, note: updatedNote });
      console.log('更新笔记成功', row);
    }

    // 刷新数据
    await store.fetchFiles(store.currentDatabase);
  } catch (error) {
    console.error('编辑失败:', error);
  }
};

// 方法：分享文件
const handleShare = (row) => {
  console.log('分享', row)
}

// 方法：移动文件
const handleMove = (row) => {
  console.log('移动', row)
}

// 方法：在系统文件资源管理器中打开导入文件所在目录
const handleOpenDirImport = async (row) => {
  try {
    if (!row.importFileName || !row.library) {
      message.error('缺少必要信息：文件名或库名');
      return;
    }

    // 获取不带后缀的文件名
    const baseFileName = row.importFileName.includes('.')
      ? row.importFileName.substring(0, row.importFileName.lastIndexOf('.'))
      : row.importFileName;

    const osPlatform = await platform();
    let dirPath = '';

    if (osPlatform === 'windows') {
      dirPath = `${store.librariesRoot}\\Libraries\\${row.library}\\${baseFileName}`;
    } else {
      dirPath = `${store.librariesRoot}/Libraries/${row.library}/${baseFileName}`;
    }

    // 调用 Tauri 后端在系统文件资源管理器中打开目录
    await invoke('open_in_file_explorer', { path: dirPath });
    console.log('在文件资源管理器中打开导入文件目录:', dirPath);
  } catch (error) {
    console.error('打开导入文件目录失败:', error);
    message.error(`打开目录失败: ${error.message || error}`);
  }
};

// 方法：在系统文件资源管理器中打开笔记文件所在目录
const handleOpenDirNote = async (row) => {
  try {
    if (!row.newNoteForm?.noteFileName || !row.library) {
      message.error('缺少必要信息：笔记文件名或库名');
      return;
    }

    // 获取导入文件的基础名（不带后缀）
    const baseFileName = row.importFileName.includes('.')
      ? row.importFileName.substring(0, row.importFileName.lastIndexOf('.'))
      : row.importFileName;

    const osPlatform = await platform();
    let dirPath = '';

    if (osPlatform === 'windows') {
      dirPath = `${store.librariesRoot}\\Libraries\\${row.library}\\${baseFileName}\\markdown`;
    } else {
      dirPath = `${store.librariesRoot}/Libraries/${row.library}/${baseFileName}/markdown`;
    }

    // 调用 Tauri 后端在系统文件资源管理器中打开目录
    await invoke('open_in_file_explorer', { path: dirPath });
    console.log('在文件资源管理器中打开笔记文件目录:', dirPath);
  } catch (error) {
    console.error('打开笔记文件目录失败:', error);
    message.error(`打开目录失败: ${error.message || error}`);
  }
};

const heightForRow = () => 85;

// 标题样式
const titleStyle = {
  fontSize: '17px',
  fontWeight: 'bold',
  color: '#000000',
};

// 计算表格总宽度（动态计算方式）
const scrollX = computed(() => {
  let totalWidth = 0;
  columns.value.forEach(column => {
    totalWidth += column.width;
  });
  return totalWidth;
});

// 动态生成列配置
const columns = computed(() => {
  const columnDefinitions = [
    // 选择列
    {
      type: 'selection',
      width: 60,
      fixed: 'left',
      align: 'center',
      key: 'selection'
    },
    // 文件名列
    {
      title: () => h('span', { style: titleStyle }, '文件名'),
      key: 'importFileName',
      width: 200,
      fixed: 'left',
      render: renderEditableCell('importFileName')
    },
    // 文件描述列
    {
      title: () => h('span', { style: titleStyle }, '文件描述'),
      key: 'description',
      width: 200,
      render: renderEditableCell('description', true, 'description')
    },
    // 文件标签列
    {
      title: () => h('span', { style: titleStyle }, '文件标签'),
      key: 'importedFileTags',
      width: 150,
      render: renderEditableCell('importedFileTags', true, 'importedFileTags')
    },
    // 文件类型列
    {
      title: () => h('span', { style: titleStyle }, '文件类型'),
      key: 'type',
      width: 120,
      render: renderReadonlyCell('type')
    },
    // 文件库列
    {
      title: () => h('span', { style: titleStyle }, '文件库'),
      key: 'library',
      width: 150,
      filterOptions: [
        { label: 'Library1', value: 'Library1' },
        { label: 'Library2', value: 'Library2' },
      ],
      filter: (value, row) => row.library === value,
      render: renderEditableCell('library', true, 'libraries')
    },
    // 入库时间列
    {
      title: () => h('span', { style: titleStyle }, '入库时间'),
      key: 'importFileCreateTime',
      width: 170,
      render: renderReadonlyCell('importFileCreateTime')
    },
    // 文件大小列
    {
      title: () => h('span', { style: titleStyle }, '文件大小'),
      key: 'importFileSize',
      width: 100,
      render: renderReadonlyCell('importFileSize')
    },
    // 笔记文件名列
    {
      title: () => h('span', { style: titleStyle }, '笔记文件名'),
      key: 'newNoteForm.noteFileName',
      width: 200,
      render: renderReadonlyCell('newNoteForm.noteFileName')
    },
    // 笔记描述列
    {
      title: () => h('span', { style: titleStyle }, '笔记描述'),
      key: 'newNoteForm.description',
      width: 200,
      render: renderReadonlyCell('newNoteForm.description')
    },
    // 笔记标签列
    {
      title: () => h('span', { style: titleStyle }, '笔记标签'),
      key: 'newNoteForm.notesTags',
      width: 150,
      render: renderReadonlyCell('newNoteForm.notesTags')
    },
    // 笔记创建时间列
    {
      title: () => h('span', { style: titleStyle }, '笔记创建时间'),
      key: 'newNoteForm.noteFileCreateTime',
      width: 170,
      render: renderReadonlyCell('newNoteForm.noteFileCreateTime')
    },
    // 笔记修改时间列
    {
      title: () => h('span', { style: titleStyle }, '笔记修改时间'),
      key: 'newNoteForm.noteFileUpdateTime',
      width: 170,
      fixed: 'right',
      align: 'left',
      render: renderReadonlyCell('newNoteForm.noteFileUpdateTime')
    }
  ];

  return columnDefinitions;
});

// 渲染可编辑单元格的工厂函数
const renderEditableCell = (field, enableAutoComplete = false, searchType = '') => {
  console.log('renderEditableCell', field, enableAutoComplete, searchType);
  return (row) => h(ShowOrEdit, {
    value: getNestedValue(row, field),
    onUpdateValue: async (v) => {
      const newRow = { ...row };
      setNestedValue(newRow, field, v);
      const error = validateTableCell(newRow);
      if (error) {
        message.error(error);
        return;
      }
      setNestedValue(row, field, v);
      await updateImportedFile(row);
    },
    enableAutoComplete,
    searchType
  });
};

// 渲染只读单元格的工厂函数
const renderReadonlyCell = (field) => {
  return (row) => h(ShowOrEdit, {
    value: getNestedValue(row, field),
    readonly: true
  });
};

// 辅助函数：获取嵌套对象属性
const getNestedValue = (obj, path) => {
  return path.split('.').reduce((o, p) => o?.[p], obj);
};

// 辅助函数：设置嵌套对象属性
const setNestedValue = (obj, path, value) => {
  const parts = path.split('.');
  const last = parts.pop();
  let current = obj;

  for (const part of parts) {
    current[part] = current[part] || {};
    current = current[part];
  }

  current[last] = value;
};

// 方法：按标题排序
const sortTitle = () => {
  tableRef.value.sort('title', 'ascend')
}

// 方法：过滤所属库
const filterLibrary = () => {
  tableRef.value.filter({
    library: ['Library1'],
  })
}

// 方法：清除过滤器
const clearFilters = () => {
  tableRef.value.filter(null)
}

// 方法：清除排序
const clearSorter = () => {
  tableRef.value.sort(null)
}

// 选中的行
const checkedRowKeys = ref([])
// 定义 row-key 函数
const rowKey = (row) => row.id

// 处理选中行
const handleCheckboxUpdate = (rowKeys) => {
  checkedRowKeys.value = rowKeys
  console.log('选中的行 keys:', rowKeys)
}

// TableViewRepoView.vue
const handleFileInfoEvent = async (payload) => {
  if (!payload) {
    console.error('事件 payload 为空');
    return;
  }

  const fileInfo = payload;
  console.log('handleFileInfoEvent', payload);

  // 构建表单数据
  newFileForm.value = {
    id: 0,
    filePath: fileInfo.file_path,
    importFileName: fileInfo.file_name,
    description: fileInfo.description,
    importedFileTags: fileInfo.tags.join(','),
    type: fileInfo.file_type,
    library: fileInfo.file_repo,
    importFileCreateTime: fileInfo.create_time,
    importFileSize: fileInfo.file_size,
    knowledgeRepo: fileInfo.knowledge_repo,
    newNoteForm: {
      id: 0,
      description: `${fileInfo.file_name}的笔记`,
      noteFileName: `${fileInfo.file_name.replace(/\.[^/.]+$/, "")}_note.md`,
      notesTags: fileInfo.tags.join(','),
      noteFileType: 'md',
      fromImportedFile: fileInfo.file_name,
      fromImportedFileLibrary: fileInfo.file_repo,
      noteFileCreateTime: new Date().toISOString(),
      noteFileUpdateTime: new Date().toISOString(),
      knowledgeRepo: fileInfo.knowledge_repo,
    }
  };

  // 调用 addFile 方法
  try {
    await addFileFromWeb();
    message.success('文件添加成功');
  } catch (error) {
    console.error('添加文件失败:', error);
    message.error(`添加文件失败: ${error.message || error}`);
  }
};

// 引入搜索逻辑
const {
  noteTagOptions,
  isNoteTagLoading,
  handleNoteTagSearch,
} = useNoteTagsSearch();

const {
  importedFileTagOptions,
  isImportedFileTagLoading,
  handleImportedFileTagSearch,
} = useImportedFileTagsSearch();

const {
  importedFileOptions,
  isImportedFileLoading,
  handleImportedFileSearch,
} = useImportedFilesSearch();

const {
  libraryOptions,
  isLibraryLoading,
  handleLibrarySearch,
} = useLibrariesSearch();

const {
  importedFileLibraryOptions,
  isImportedFileLibraryLoading,
  handleImportedFileLibrarySearch,
} = useImportedFileLibrarySearch();

// 监听输入值变化并触发搜索
watch(() => currentForm.value.importedFileTags, (newValue) => {
  handleImportedFileTagSearch(newValue);
});

watch(() => currentForm.value.notesTags, (newValue) => {
  handleNoteTagSearch(newValue);
});

watch(() => currentForm.value.fromImportedFile, (newValue) => {
  handleImportedFileSearch(newValue);
});

watch(() => currentForm.value.library, (newValue) => {
  handleLibrarySearch(newValue);
});

watch(() => currentForm.value.fromImportedFileLibrary, (newValue) => {
  handleImportedFileLibrarySearch(newValue);
});

watch(() => eventStore.webClipperEvent, (newEvent) => {
  if (newEvent) {
    handleFileInfoEvent({ payload: newEvent });
    // 处理完后清除事件
    eventStore.webClipperEvent = null;
  }
});

const initAppHandle = async () => {
  try {
    await invoke('init_backend_services');
    console.log('AppHandle initialized successfully');
  } catch (error) {
    console.error('Failed to initialize AppHandle:', error);
    message.error('初始化应用句柄失败，部分功能可能受限');
  }
};

// 组件挂载时获取默认数据
onMounted(async () => {
  // 初始化 AppHandle
  await initAppHandle();

  store.get_librariesRoot()
  await store.fetchFiles(store.currentDatabase) // 默认加载数据库 1 的数据

  try {
    // 安全初始化
    await eventStore.initGlobalListeners();

    // 订阅事件（带自动注册）
    const unsubscribe = eventStore.subscribe(
      'add_web_clipper_file_event',
      (payload) => {
        console.log('处理事件:', payload);
        handleFileInfoEvent(payload);
      }
    );

    onUnmounted(unsubscribe);
  } catch (error) {
    console.error('事件系统初始化失败:', error);
  }
})

onUnmounted(() => {
  // 移除 Tauri 事件监听
  // if (unlistenBrowserExtension) {
  //   unlistenBrowserExtension();
  // }
})
</script>

<style scoped>
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

/* 工具栏左侧样式调整 */
.toolbar-left {
  padding-left: 16px;
  /* 增加左侧内边距 */
}

/* 工具栏占位元素样式 */
.toolbar-placeholder {
  width: 40px;
  /* 预留图标位置宽度 */
  height: 32px;
  /* 与按钮高度一致 */
}

.search-card {
  border-radius: 12px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.05);
  transition: all 0.3s ease;
}

.search-card:hover {
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

.search-title {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: #333;
}

.toolbar-container {
  padding: 0 16px 16px;
  width: 100%;
  box-sizing: border-box;
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

.data-table {
  flex: 1;
  width: 100% !important;
  height: 100%;
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
}

.actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.add-path-button {
  margin-left: auto;
  white-space: nowrap;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .actions {
    flex-direction: column;
  }

  .add-path-button {
    margin-left: 0;
  }
}

/* 行悬停效果 */
.hovered-row {
  background-color: #f0f7ff;
}

/* 右键菜单样式 */
.custom-dropdown .n-dropdown-menu {
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  padding: 8px 0;
}

.custom-dropdown .n-dropdown-option {
  padding: 8px 16px;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  color: #333;
  cursor: pointer;
  transition: background-color 0.2s;
}

.custom-dropdown .n-dropdown-option:hover {
  background-color: #f0f7ff;
}

.custom-dropdown .n-dropdown-option .n-icon {
  color: #666;
}

.custom-form {
  padding: 16px;
}

.custom-form .n-form-item {
  margin-bottom: 16px;
}

.custom-form .n-input {
  width: 100%;
}

.n-auto-complete {
  width: 100%;
}

.progress-container {
  position: fixed;
  bottom: 20px;
  right: 20px;
  width: 350px;
  max-height: 50vh;
  overflow-y: auto;
  z-index: 1000;
  background-color: white;
  border-radius: 8px;
  box-shadow: 0 2px 12px 0 rgba(56, 54, 54, 0.884);
  padding: 12px;
}

.progress-container .n-card {
  margin-bottom: 12px;
}

:deep(.n-progress .n-progress-graph-line-fill) {
  background-color: #409EFF !important;
}

:deep(.n-progress .n-progress-content) {
  position: absolute;
  right: 5%;
  top: 50%;
  transform: translateY(-50%);
}

/* 表格样式优化 */
:deep(.n-data-table) {
  --n-border-color: #eaeaea;
  --n-th-color: #f8f9fa;
  --n-td-color: #ffffff;
  --n-td-color-hover: #f5f7fa;
  --n-td-color-striped: #f9f9f9;
}

:deep(.n-data-table-th) {
  font-weight: 600;
  color: #333;
}

:deep(.n-data-table-td) {
  padding: 12px 16px;
}

/* 搜索输入框样式 */
:deep(.n-input-group) {
  border-radius: 20px;
  overflow: hidden;
}

:deep(.n-input),
:deep(.n-button) {
  border-radius: 16px !important;
}

/* 选择器样式 */
:deep(.n-select) {
  --n-border: 1px solid #e0e0e0;
  --n-border-hover: 1px solid #c0c4cc;
  --n-border-focus: 1px solid #409EFF;
  --n-border-radius: 8px;
}

/* 日期选择器样式 */
:deep(.n-date-picker) {
  --n-border: 1px solid #e0e0e0;
  --n-border-hover: 1px solid #c0c4cc;
  --n-border-focus: 1px solid #409EFF;
  --n-border-radius: 8px;
}

/* 任务列表样式 */
.task-item {
  padding: 12px;
  margin-bottom: 8px;
  border-radius: 8px;
  background-color: var(--n-color);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
  transition: all 0.3s ease;
}

.task-item:hover {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.task-item-completed {
  opacity: 0.8;
  background-color: var(--n-color-hover);
}

/* 使用flex布局和gap控制间距 */
.task-content {
  display: flex;
  flex-direction: column;
  gap: 8px;
  /* 统一控制子元素间距 */
}

/* 头部布局优化 */
.task-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  /* 顶部对齐 */
  gap: 8px;
  /* 元素间间距 */
}

.task-filename {
  flex: 1;
  min-width: 0;
  /* 防止flex item溢出 */
  word-break: break-word;
  /* 更好的断词 */
  line-height: 1.5;
  /* 合适的行高 */
}

/* 进度条样式优化 */
.task-progress {
  margin-block: 0;
  /* 使用逻辑属性 */
  margin-inline: 0;
  padding-top: 8px;
}

/* 消息样式 */
.task-message {
  display: flex;
  align-items: center;
  font-size: 13px;
  color: var(--n-text-color);
  gap: 6px;
  /* 图标和文本间距 */
}

.task-message-icon {
  flex-shrink: 0;
}
</style>