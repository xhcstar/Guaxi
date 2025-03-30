<template>
  <n-space vertical :size="12" class="container">
    <!-- 工具栏 -->
    <n-space justify="space-between" align="center" class="toolbar">
      <!-- 左侧按钮组 -->
      <n-space class="actions">
        <n-button @click="sortTitle" class="action-button">按标题排序（升序）</n-button>
        <n-button @click="filterLibrary" class="action-button">过滤所属库(Library1)</n-button>
        <n-button @click="clearFilters" class="action-button">清除过滤器</n-button>
        <n-button @click="clearSorter" class="action-button">清除排序</n-button>
      </n-space>
      <!-- 右侧添加按钮 -->
      <n-tooltip trigger="hover">
        <template #trigger>
          <n-button text class="add-path-button" @click="showAddFileModal">
            <n-icon :component="FolderAdd20Regular" :size="24" />
          </n-button>
        </template>
        导入新的文件
      </n-tooltip>
    </n-space>

  </n-space>

  <div class="main-content">
    <n-data-table ref="table" :columns="columns" :data="currentData" :header-height="28" virtual-scroll
      :max-height="500" :scroll-x="1500" :pagination="pagination" :single-line="false" class="data-table"
      :row-props="rowProps" :multiple="true" :row-key="rowKey" @update:checked-row-keys="handleCheckboxUpdate" />
    
      <!-- 进度条容器 - 固定在底部 -->
    <div class="progress-container-wrapper">
      <div class="progress-container">
        <n-card v-for="(progress, index) in progresses" :key="index">
            <n-text strong>{{ progress.fileName }}</n-text>
            <n-progress type="line" :percentage="progress.current" :indicator-placement="'inside'" status="info" />
            <n-text depth="3">{{ progress.message }}</n-text>
        </n-card>
      </div>
    </div>

      <!-- 右键菜单 -->
  <n-dropdown placement="bottom-start" trigger="manual" :x="x" :y="y" :options="contextMenuOptions" :show="showDropdown"
    :on-clickoutside="onClickoutside" @select="handleContextMenuSelect" class="custom-dropdown" />
    
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
        <template
          v-else-if="column.key === 'importFileCreateTime' || column.key === 'noteFileCreateTime' || column.key === 'noteFileUpdateTime'
    || column.key === 'noteFileType' || column.key === 'importFileName' || column.key === 'type' || column.key === 'importFileSize'">
          <n-input v-model:value="currentForm[column.key]" :placeholder="`${column.title}`" disabled
            :input-style="{ borderRadius: '8px' }" />
        </template>
        <!-- 笔记标签 -->
        <template v-else-if="column.key === 'notesTags'">
          <n-auto-complete v-model:value="currentForm[column.key]" :options="noteTagOptions" :loading="isNoteTagLoading"
            placeholder="请输入笔记标签" clearable :input-style="{ borderRadius: '8px' }" />
        </template>
        <!-- 导入文件标签 -->
        <template v-else-if="column.key === 'importedFileTags'">
          <n-auto-complete v-model:value="currentForm[column.key]" :options="importedFileTagOptions"
            :loading="isImportedFileTagLoading" placeholder="请输入导入文件标签" clearable
            :input-style="{ borderRadius: '8px' }" />
        </template>
        <!-- 导入文件 -->
        <template v-else-if="column.key === 'fromImportedFile'">
          <n-auto-complete v-model:value="currentForm[column.key]" :options="importedFileOptions"
            :loading="isImportedFileLoading" placeholder="请输入导入文件" clearable :input-style="{ borderRadius: '8px' }" />
        </template>
        <!-- 所属库 -->
        <template v-else-if="column.key === 'library'">
          <n-auto-complete v-model:value="currentForm[column.key]" :options="libraryOptions" :loading="isLibraryLoading"
            placeholder="请输入所属库" clearable :input-style="{ borderRadius: '8px' }" />
        </template>
        <!-- 导入文件所属库 -->
        <template v-else-if="column.key === 'fromImportedFileLibrary'">
          <n-auto-complete v-model:value="currentForm[column.key]" :options="importedFileLibraryOptions"
            :loading="isImportedFileLibraryLoading" placeholder="请输入导入文件所属库" clearable
            :input-style="{ borderRadius: '8px' }" />
        </template>
        <!-- 默认输入框 -->
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

<script setup lang="ts">
import { ref, onMounted, computed, h, nextTick, watch } from 'vue'
import { repoStore } from '../../stores/repoStore'
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
import { Library, Settings } from '@vicons/ionicons5'
import { listen } from '@tauri-apps/api/event';
// import { Window } from '@tauri-apps/api/window';
// import { Webview } from "@tauri-apps/api/webview"

// 引入 Pinia store
const store = repoStore()

// 获取 message 实例
const message = useMessage();
// 添加文件对话框状态
const isAddFileModalVisible = ref(false)
// 表格实例
const tableRef = ref(null)
// 分页配置
const pagination = ref({ pageSize: 15 })

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
  convetToHtmlFilename: '',
  importFileSize: '',
})

// 用于 NotesDb 库的表单
const newNoteForm = ref({
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
})

// 当前表单（根据数据库类型动态切换）
const currentForm = computed(() => {
  return store.currentDatabase === 'importFileDB' ? newFileForm.value : newNoteForm.value
})

// 进度条相关状态
const progresses = ref([])
const startProgressListener = async (filePath: string) => {
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
    // 使用Vue的响应式更新方式
    progresses.value[progressIndex] = {
      ...progresses.value[progressIndex],
      current: event.payload.current,
      message: event.payload.message
    }
    console.log('conversion_progress', event.payload.current, event.payload.message)

    // 转换完成后移除进度条
    if (event.payload.current >= 100) {
      setTimeout(() => {
        progresses.value = progresses.value.filter((_, i) => i !== progressIndex)
      }, 1000)
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
  if (store.currentDatabase === 'importFileDB') {
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
  } else if (store.currentDatabase === 'NotesDb') {
    if (!form.notesTags || form.notesTags.trim() === '') {
      console.log('form.notesTags', form.notesTags);
      currentForm.value.notesTags = "default"; // 设置默认值
    } else {
      const tags = form.notesTags.split(',').map(tag => tag.trim());
      if (tags.some(tag => !tag)) {
        return '多个标签没有以逗号分隔';
      }
    }
  } else {
    return '不支持的数据库类型';
  }

  // 校验描述
  if (!form.description || form.description.trim() === '') {
    return '描述不能为空';
  }
  if (form.description.length > 240) {
    return '描述不能超过 240 个字符';
  }

  // 校验所属库
  if (store.currentDatabase === 'importFileDB') {
    if (!form.library || (typeof form.library === 'string' && form.library.trim() === '')) {
      currentForm.value.library = 'default'; // 设置默认值
      return '所属库不能为空';
    }
  }

  // 校验入库时间
  if (store.currentDatabase === 'importFileDB') {
    if (!form.importFileCreateTime || form.importFileCreateTime.trim() === '') {
      return '入库时间不能为空';
    }
  }

  // 校验文件大小
  if (store.currentDatabase === 'importFileDB') {
    if (!form.importFileSize || form.importFileSize <= 0) {
      return '文件大小不能为空';
    }
  }

  // 校验笔记文件名（如果是 NotesDb）
  if (store.currentDatabase === 'NotesDb' && (!form.noteFileName || form.noteFileName.trim() === '')) {
    return '笔记文件名不能为空';
  }

  // 校验笔记创建时间（如果是 NotesDb）
  if (store.currentDatabase === 'NotesDb' && (!form.noteFileCreateTime || form.noteFileCreateTime.trim() === '')) {
    return '笔记创建时间不能为空';
  }

  // 校验笔记的导入文件所属库（如果是 NotesDb）
  if (store.currentDatabase === 'NotesDb' && (!form.fromImportedFileLibrary || form.fromImportedFileLibrary.trim() === '')) {
    currentForm.value.fromImportedFileLibrary = "NoImportFileAssocNoteLib"; // 设置默认值
  }

  // 校验笔记的导入文件（如果是 NotesDb）
  if (store.currentDatabase === 'NotesDb' && (!form.fromImportedFile || form.fromImportedFile.trim() === '')) {
    currentForm.value.fromImportedFile = "NoImportFileAssocNote"; // 设置默认值
  }

  // 校验通过
  return null;
};

// 校验table cell
const validateTableCell = (row) => {
  console.log('validateTableCell', row);
  console.log('store.currentDatabase', store.currentDatabase);

  // 校验标签
  if (store.currentDatabase === 'importFileDB') {
    if (!row.importedFileTags || row.importedFileTags.trim() === '') {
      console.log('row.importedFileTags', row.importedFileTags);
      row.importedFileTags = "default"; // 设置默认值
    } else {
      const tags = row.importedFileTags.split(',').map(tag => tag.trim());
      if (tags.some(tag => !tag)) {
        return '多个标签没有以逗号分隔';
      }
    }
  } else if (store.currentDatabase === 'NotesDb') {
    if (!row.notesTags || row.notesTags.trim() === '') {
      console.log('row.notesTags', row.notesTags);
      row.notesTags = "default"; // 设置默认值
    } else {
      const tags = row.notesTags.split(',').map(tag => tag.trim());
      if (tags.some(tag => !tag)) {
        return '多个标签没有以逗号分隔';
      }
    }
  } else {
    return '不支持的数据库类型';
  }

  // 校验描述
  if (!row.description || row.description.trim() === '') {
    return '描述不能为空';
  }
  if (row.description.length > 240) {
    return '描述不能超过 240 个字符';
  }

  // 校验所属库
  if (store.currentDatabase === 'importFileDB') {
    if (!row.library || (typeof row.library === 'string' && row.library.trim() === '')) {
      row.library = 'default'; // 设置默认值
      return '所属库不能为空';
    }
  }

  // 校验入库时间
  if (store.currentDatabase === 'importFileDB') {
    if (!row.importFileCreateTime || row.importFileCreateTime.trim() === '') {
      return '入库时间不能为空';
    }
  }

  // 校验文件大小
  if (store.currentDatabase === 'importFileDB') {
    if (!row.importFileSize || row.importFileSize <= 0) {
      return '文件大小不能为空';
    }
  }

  // 校验笔记文件名（如果是 NotesDb）
  if (store.currentDatabase === 'NotesDb' && (!row.noteFileName || row.noteFileName.trim() === '')) {
    return '笔记文件名不能为空';
  }

  // 校验笔记创建时间（如果是 NotesDb）
  if (store.currentDatabase === 'NotesDb' && (!row.noteFileCreateTime || row.noteFileCreateTime.trim() === '')) {
    return '笔记创建时间不能为空';
  }

  // 校验笔记的导入文件所属库（如果是 NotesDb）
  if (store.currentDatabase === 'NotesDb' && (!row.fromImportedFileLibrary || row.fromImportedFileLibrary.trim() === '')) {
    row.fromImportedFileLibrary = "NoImportFileAssocNoteLib"; // 设置默认值
  }

  // 校验笔记的导入文件（如果是 NotesDb）
  if (store.currentDatabase === 'NotesDb' && (!row.fromImportedFile || row.fromImportedFile.trim() === '')) {
    row.fromImportedFile = "NoImportFileAssocNote"; // 设置默认值
  }

  // 校验通过
  return null;
};

// 表单校验规则
const rules = {
  filePath: [
    { validator: validateFilePath, required: true, message: '文件路径不能为空', trigger: ['input', 'blur'] },
  ],
  notesTags: [
    { validator: validateTags, required: true, message: '标签不能为空', trigger: ['input', 'blur'] },
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
  noteFileName: [
    { required: true, message: '笔记文件名不能为空', trigger: ['input', 'blur'] },
  ],
  noteFileCreateTime: [
    { required: true, message: '笔记创建时间不能为空', trigger: ['input', 'blur'] },
  ],
};

// 动态生成表单列配置
const formColumns = computed(() => {
  const columns = store.currentDatabase === 'importFileDB'
    ? [
      { key: 'filePath', title: '文件路径' },
      { key: 'importFileName', title: '文件名' },
      { key: 'description', title: '文件描述' },
      { key: 'library', title: '所属库' },
      { key: 'importedFileTags', title: '文件标签' },
      { key: 'type', title: '文件类型' },
      { key: 'importFileCreateTime', title: '入库时间' },
      { key: 'importFileSize', title: '文件大小' },
    ]
    : [
      { key: 'filePath', title: '文件路径' },
      { key: 'description', title: '笔记描述' },
      { key: 'noteFileName', title: '笔记文件名' },
      { key: 'notesTags', title: '笔记标签' },
      { key: 'noteFileType', title: '笔记文件类型' },
      { key: 'fromImportedFile', title: '笔记归属导入文件' },
      { key: 'fromImportedFileLibrary', title: '导入文件所属库' },
      { key: 'noteFileCreateTime', title: '笔记创建时间' },
      { key: 'noteFileUpdateTime', title: '笔记修改时间' },
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
      if (store.currentDatabase === 'importFileDB') {
        currentForm.value.importFileName = fileInfo.file_name
        currentForm.value.type = fileInfo.file_type
        currentForm.value.importFileCreateTime = fileInfo.create_time
        currentForm.value.importFileSize = fileInfo.file_size
      } else if (store.currentDatabase === 'NotesDb') {
        currentForm.value.noteFileName = fileInfo.file_name
        currentForm.value.noteFileType = fileInfo.file_type
        currentForm.value.noteFileCreateTime = fileInfo.create_time
      }
    }
  } catch (error) {
    console.error('Error opening file browser:', error)
  }
}

// 显示添加文件对话框
const showAddFileModal = () => {
  isAddFileModalVisible.value = true
}

const createImportFileList = (form) => {
  return {
    import_file_name: form.importFileName,
    description: form.description,
    library: form.library,
    tags: form.importedFileTags.split(','),
    file_type: form.type,
    import_file_create_time: form.importFileCreateTime,
    import_file_size: form.importFileSize,
  };
};

// 提取创建 importNoteList 的逻辑
const createImportNoteList = (addedNoteInfo, currentForm) => {
  if (store.currentDatabase === 'NotesDb') {
    return {
      description: currentForm.description,
      note_file_name: currentForm.noteFileName,
      tags: currentForm.notesTags.split(','),
      note_file_type: currentForm.noteFileType,
      from_imported_file: currentForm.fromImportedFile,
      from_imported_file_library: currentForm.fromImportedFileLibrary,
      note_file_create_time: currentForm.noteFileCreateTime,
      note_file_update_time: currentForm.noteFileCreateTime,
    }
  } else {
    return {
      description: `${addedNoteInfo.from_filename}的笔记`,
      note_file_name: addedNoteInfo.note_file_name,
      tags: currentForm.importedFileTags.split(','), //导入文件时创建的笔记使用导入文件的标签
      note_file_type: 'md',
      from_imported_file: addedNoteInfo.from_filename,
      from_imported_file_library: addedNoteInfo.library,
      note_file_create_time: addedNoteInfo.create_time,
      note_file_update_time: addedNoteInfo.update_time,
    };
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

const addFile = async () => {
  try {
    // 校验表单
    const validationError = await validateForm(); // 等待 validateForm 的结果
    if (validationError) {
      message.error(validationError); // 确保 validationError 是字符串
      return; // 终止添加
    }

    if (store.currentDatabase === 'importFileDB') {
      if (!currentForm.value.library || !currentForm.value.importFileName) {
        message.error('Library or importFileName path is missing: ', currentForm.value);
        return;
      }

      // 调用 Tauri 后端接口，将文件拷贝到 Libraries 目录下
      const addedNoteInfo = await invoke('add_repo_import_file', {
        library: currentForm.value.library,
        filePath: currentForm.value.filePath,
      });

      // 创建 importfileList
      const importfileList = createImportFileList(currentForm.value);
      console.log('importfileList:', importfileList);

      // 将 importfileList 写入数据库
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
      });

      // 创建 importNoteList
      const importNoteList = createImportNoteList(addedNoteInfo, currentForm.value);
      console.log('importNoteList:', importNoteList);

      // 将 importNoteList 写入数据库
      const noteId = await addNoteToDatabase(importNoteList);

      // 添加导入的笔记到 store
      store.addImportedNote({
        id: noteId,
        description: currentForm.value.description,
        noteFileName: `${currentForm.value.noteFileName}.md`,
        type: 'md',
        notesTags: currentForm.value.notesTags,
        fromImportedFileLibrary: currentForm.value.fromImportedFileLibrary,
        noteFileCreateTime: currentForm.value.noteFileCreateTime,
        noteFileUpdateTime: currentForm.value.noteFileUpdateTime || currentForm.value.noteFileCreateTime,
      });

      try {
        // 开始监听进度事件
        const unlisten = await startProgressListener(currentForm.value.filePath)

        // 立即关闭表单对话框
        isAddFileModalVisible.value = false

        // 开始转换流程
        const outputDir = `${store.librariesRoot}\\Libraries\\${currentForm.value.library}\\${currentForm.value.importFileName.replace(/\.[^/.]+$/, "")}\\markdown`
        await convertFile(currentForm.value.filePath, outputDir)

        // 转换完成后刷新数据
        await store.fetchFiles('importFileDB')
        resetForm()

        // 转换完成后取消监听
        unlisten()
      } catch (error) {
        message.error(`添加文件失败: ${error}`)
      }
    } else if (store.currentDatabase === 'NotesDb') {
      // 创建 importNoteList
      const importNoteList = createImportNoteList(null, currentForm.value);
      console.log('importNoteList:', importNoteList);

      // 将 importNoteList 写入数据库
      const noteId = await addNoteToDatabase(importNoteList);

      // 添加导入的笔记到 store
      store.addImportedNote({
        id: noteId,
        description: currentForm.value.description,
        noteFileName: `${currentForm.value.noteFileName}.md`,
        type: 'md',
        notesTags: currentForm.value.notesTags,
        fromImportedFile: currentForm.value.fromImportedFile,
        fromImportedFileLibrary: currentForm.value.fromImportedFileLibrary,
        noteFileCreateTime: currentForm.value.noteFileCreateTime,
        noteFileUpdateTime: currentForm.value.noteFileUpdateTime || currentForm.value.noteFileCreateTime,
      });
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

    // 确保 file 是一个对象
    const file = {
      import_file_name: row.importFileName,
      description: row.description,
      library: row.library,
      file_type: row.type,
      import_file_create_time: row.importFileCreateTime,
      import_file_size: row.importFileSize,
      tags: importedFileTags, // 传递转换后的 tags 数组
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
  if (store.currentDatabase === 'importFileDB') {
    newFileForm.value = {
      id: 0,
      importFileName: '',
      description: '',
      importedFileTags: '',
      type: '',
      library: '', // 所属库名字
      importFileCreateTime: '',
      importFileSize: 0,
      convetToHtmlFilename: '',
      convetToMarkdownFilename: "",
      importFileSize: '',
    }
  } else {
    newNoteForm.value = {
      id: 0,
      description: '',
      noteFileName: '',
      notesTags: '', // 所属库名字
      fromImportedFile: '',
      fromImportedFileLibrary: '',
      noteFileType: '',
      noteFileCreateTime: '',
      noteFileUpdateTime: '', // 笔记特有的字段
    }
  }
}

// 根据当前数据库类型选择数据
const currentData = computed(() => {
  return store.currentDatabase === 'NotesDb' ? store.notes : store.files
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
    label: '移动',
    key: 'move',
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
  const rowFileType = store.currentDatabase === 'importFileDB'
    ? row.type?.toLowerCase() || ''
    : row.noteFileType?.toLowerCase() || ''

  const rowFileName = store.currentDatabase === 'importFileDB'
    ? row.importFileName || ''
    : row.noteFileName || ''

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

  if (store.currentDatabase === 'importFileDB') {
    if (osPlatform === 'windows') {
      ReadFilePath = `${store.librariesRoot}\\Libraries\\${row.library}\\${baseFileName}\\${rowFileType}\\${rowFileName}`
    } else if (osPlatform === 'linux') {
      ReadFilePath = `${store.librariesRoot}/Libraries/${row.library}/${baseFileName}/${rowFileType}/${rowFileName}`
    } else {
      console.error('未知的操作系统平台:', osPlatform)
    }
  } else if (store.currentDatabase === 'NotesDb') {
    if (osPlatform === 'windows') {
      ReadFilePath = `${store.librariesRoot}\\Libraries\\${row.library}\\${baseFileName}\\notes\\${rowFileName}`
    } else if (osPlatform === 'linux') {
      ReadFilePath = `${store.librariesRoot}/Library/${row.library}/${baseFileName}/notes/${rowFileName}`
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
      }
    })

    console.log('窗口创建参数:', {
      label: windowLabel,
      title: windowTitle,
      fileData: {
        type: fileType,
        name: rowFileName,
        path: ReadFilePath,
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
    if (store.currentDatabase === 'importFileDB') {
      // 调用删除导入文件的接口
      await invoke('delete_imported_file', { id: row.id });
      console.log('删除导入文件成功', row);
    } else {
      // 调用删除笔记的接口
      await invoke('delete_note', { id: row.id });
      console.log('删除笔记成功', row);
    }

    // 刷新数据
    await store.fetchFiles(store.currentDatabase);
  } catch (error) {
    console.error('删除失败:', error);
  }
};

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

// 表格列配置
const columns = computed(() => {
  return store.currentDatabase === 'NotesDb'
    ? [
      {
        type: 'selection',
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, '笔记描述'),
        key: 'description',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.description,
            onUpdateValue: async (v) => {
              console.log('更新笔记描述', v)
              const error = validateTableCell({ ...row, description: v });
              if (error) {
                message.error(error); // 使用 useMessage 的消息组件
                return;
              }
              row.description = v
              await updateNote(row); // 调用更新笔记的接口
            },
          })
        },
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, '笔记文件名'),
        key: 'noteFileName',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.noteFileName,
            onUpdateValue: async (v) => {
              const error = validateTableCell({ ...row, noteFileName: v });
              if (error) {
                message.error(error); // 使用 useMessage 的消息组件
                return;
              }
              row.noteFileName = v
              await updateNote(row); // 调用更新笔记的接口
            },
          })
        },
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, '笔记标签'),
        key: 'notesTags',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.notesTags,
            onUpdateValue: async (v) => {
              console.log('更新笔记标签', v)
              const error = validateTableCell({ ...row, notesTags: v });
              if (error) {
                message.error(error); // 使用 useMessage 的消息组件
                return;
              }
              row.notesTags = v
              await updateNote(row); // 调用更新笔记的接口
            },
            enableAutoComplete: true, // 启用自动补全
            searchType: 'noteTags', // 指定搜索类型
          })
        },
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, '笔记归属导入文件'),
        key: 'fromImportedFile',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.fromImportedFile,
            onUpdateValue: async (v) => {
              const error = validateTableCell({ ...row, fromImportedFile: v });
              if (error) {
                message.error(error); // 使用 useMessage 的消息组件
                return;
              }
              row.fromImportedFile = v
              await updateNote(row); // 调用更新笔记的接口
            },
            enableAutoComplete: true, // 启用自动补全
            searchType: 'fromImportedFile', // 指定搜索类型
          })
        },
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, '导入文件所属库'),
        key: 'fromImportedFileLibrary',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.fromImportedFileLibrary,
            onUpdateValue: async (v) => {
              const error = validateTableCell({ ...row, fromImportedFileLibrary: v });
              if (error) {
                message.error(error); // 使用 useMessage 的消息组件
                return;
              }
              row.fromImportedFileLibrary = v
              await updateNote(row); // 调用更新笔记的接口
            },
            enableAutoComplete: true, // 启用自动补全
            searchType: 'importedFileLibraries', // 指定搜索类型
          })
        },
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, '笔记文件类型'),
        key: 'noteFileType',
        resizable: true,
        width: 130,
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, '笔记创建时间'),
        key: 'noteFileCreateTime',
        resizable: true,
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, '笔记修改时间'),
        key: 'noteFileUpdateTime',
        resizable: true,
      },
    ]
    : [
      {
        type: 'selection',
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, '文件名'),
        key: 'importFileName',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.importFileName,
            onUpdateValue: async (v) => {
              const error = validateTableCell({ ...row, importFileName: v });
              if (error) {
                message.error(error); // 使用 useMessage 的消息组件
                return;
              }
              row.importFileName = v; // 更新前端表格数据
              await updateImportedFile(row); // 调用更新导入文件的接口
            },
          })
        },
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, '文件描述'),
        key: 'description',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.description,
            onUpdateValue: async (v) => {
              const error = validateTableCell({ ...row, description: v });
              if (error) {
                message.error(error); // 使用 useMessage 的消息组件
                return;
              }
              row.description = v; // 更新前端表格数据
              await updateImportedFile(row); // 调用更新导入文件的接口
            },
          })
        },
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, '文件标签'),
        key: 'importedFileTags',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.importedFileTags,
            onUpdateValue: async (v) => {
              const error = validateTableCell({ ...row, importedFileTags: v });
              if (error) {
                message.error(error); // 使用 useMessage 的消息组件
                return;
              }
              row.importedFileTags = v
              await updateImportedFile(row); // 调用更新导入文件的接口
            },
            enableAutoComplete: true, // 启用自动补全
            searchType: 'importedFileTags', // 指定搜索类型
          })
        },
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, '文件类型'),
        key: 'type',
        resizable: true,
        width: 100,
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, '所属库'),
        key: 'library',
        resizable: true,
        filterOptions: [
          { label: 'Library1', value: 'Library1' },
          { label: 'Library2', value: 'Library2' },
        ],
        filter(value, row) {
          return row.library === value
        },
        render(row) {
          return h(ShowOrEdit, {
            value: row.library,
            onUpdateValue: async (v) => {
              const error = validateTableCell({ ...row, library: v });
              if (error) {
                message.error(error); // 使用 useMessage 的消息组件
                return;
              }
              row.library = v
              await updateImportedFile(row); // 调用更新导入文件的接口
            },
            enableAutoComplete: true, // 启用自动补全
            searchType: 'libraries', // 指定搜索类型
          })
        },
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, '入库时间'),
        key: 'importFileCreateTime',
        resizable: true,
        width: '170',
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, '文件大小'),
        key: 'importFileSize',
        resizable: true,
        width: 100,
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, 'HTML文件名'),
        key: 'convetToHtmlFilename',
        resizable: true,
        width: '180',
      },
      {
        title: () => h('span',
          {
            style: {
              fontSize: '17px', // 调整字号
              fontWeight: 'bold', // 加粗
              color: '#000000', // 字体颜色
            }
          }, 'Markdown文件名'),
        key: 'convetToMarkdownFilename',
        resizable: true,
        width: '180',
      },
    ]
});

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

// 组件挂载时获取默认数据
onMounted(async () => {
  store.get_librariesRoot()
  await store.fetchFiles('importFileDB') // 默认加载数据库 1 的数据
})

</script>

<style scoped>
.container {
  width: 100%;
  padding: 16px;
  box-sizing: border-box;
  /* 确保内边距不影响宽度 */
}

.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
  overflow-x: auto;
  /* 允许水平滚动 */
}

.actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  /* 允许按钮换行 */
}

.add-path-button {
  margin-left: auto;
  white-space: nowrap;
  /* 防止按钮文本换行 */
}

/* 响应式设计 */
@media (max-width: 768px) {
  .actions {
    flex-direction: column;
    /* 小屏幕时按钮堆叠显示 */
  }

  .add-path-button {
    margin-left: 0;
    /* 小屏幕时取消自动左边距 */
  }
}

.data-table {
  border: 1px solid #ddd;
  border-radius: 4px;
  overflow: hidden;
}

/* 行悬停效果 */
.hovered-row {
  background-color: #e8f5e9;
  /* 淡绿色 */
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
  background-color: #e8f5e9;
  /* 淡绿色 */
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
  /* 确保宽度足够 */
}

.progress-container {
  position: fixed;
  bottom: 0px;
  right: 20px;
  width: 350px;
  max-height: 50vh;
  overflow-y: auto;
  z-index: 1000;
  background-color: white;
  border-radius: 8px;
  box-shadow: 0 2px 12px 0 rgba(56, 54, 54, 0.884);
  padding: 0px;
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
</style>