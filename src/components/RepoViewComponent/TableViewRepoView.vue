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

    <n-data-table ref="table" :columns="columns" :data="currentData" :header-height="28" virtual-scroll
      :max-height="500" :scroll-x="1500" :pagination="pagination" :single-line="false" class="data-table"
      :row-props="rowProps" :multiple="true" :row-key="rowKey" @update:checked-row-keys="handleCheckboxUpdate" />
  </n-space>

  <!-- 右键菜单 -->
  <n-dropdown placement="bottom-start" trigger="manual" :x="x" :y="y" :options="contextMenuOptions" :show="showDropdown"
    :on-clickoutside="onClickoutside" @select="handleContextMenuSelect" class="custom-dropdown" />

  <!-- 添加文件对话框 -->
  <n-modal v-model:show="isAddFileModalVisible" title="添加文件" preset="dialog">
    <!-- 动态生成表单 -->
    <n-form :model="currentForm" label-placement="left" label-width="auto">
      <n-form-item v-for="column in formColumns" :key="column.key" :label="column.title">
        <!-- 根据列类型渲染不同的输入组件 -->
        <template v-if="column.key === 'filePath'">
          <n-input v-model:value="currentForm[column.key]" placeholder="请输入文件路径">
            <template #suffix>
              <n-tooltip trigger="hover">
                <template #trigger>
                  <n-button text class="FolderAdd20Regular" @click="openFileBrowser">
                    <n-icon :component="FolderAdd20Regular" :size="20" />
                  </n-button>
                </template>
                选择导入文件
              </n-tooltip>
            </template>
          </n-input>
        </template>
        <template v-else-if="column.key === 'importFileCreateTime' || column.key === 'noteFileCreateTime' || column.key === 'noteFileUpdateTime'
          || column.key === 'noteFileType' || column.key === 'importFileName' || column.key === 'type' || column.key === 'fromImportedFileLibrary'
          || column.key === 'fromImportedFile' || column.key === 'importFileSize'">
          <n-input v-model:value="currentForm[column.key]" :placeholder="`${column.title}`" disabled />
        </template>
        <template v-else>
          <n-input v-model:value="currentForm[column.key]" :placeholder="`请输入${column.title}`" />
        </template>
      </n-form-item>
    </n-form>

    <template #action>
      <n-button @click="addFile">提交</n-button>
      <n-button @click="isAddFileModalVisible = false">取消</n-button>
    </template>
  </n-modal>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, h, nextTick } from 'vue'
import { repoStore } from '../../stores/repoStore'
import {
  FolderAdd20Regular,
  Delete20Regular,
  Open20Regular,
  Edit20Regular,
  Share20Regular,
  ArrowMove24Regular,
} from '@vicons/fluent'
import { NIcon, NButton, NDropdown } from 'naive-ui'
import ShowOrEdit from './ShowOrEdit.vue'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'

// 引入 Pinia store
const store = repoStore()

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
  tags: '',
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
  tags: '', // 所属库名字
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

// 动态生成表单列配置
const formColumns = computed(() => {
  const columns = store.currentDatabase === 'importFileDB'
    ? [
      { key: 'filePath', title: '文件路径' },
      { key: 'importFileName', title: '文件名' },
      { key: 'description', title: '文件描述' },
      { key: 'library', title: '所属库' },
      { key: 'tags', title: '文件标签' },
      { key: 'type', title: '文件类型' },
      { key: 'importFileCreateTime', title: '入库时间' },
      { key: 'importFileSize', title: '文件大小' },
    ]
    : [
      { key: 'filePath', title: '文件路径' },
      { key: 'description', title: '笔记描述' },
      { key: 'noteFileName', title: '笔记文件名' },
      { key: 'tags', title: '笔记标签' },
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

// 添加文件或笔记
const addFile = async () => {
  try {
    if (store.currentDatabase === 'importFileDB') {
      if (!currentForm.value.library || !currentForm.value.importFileName) {
        console.error('Library or importFileName path is missing: ', currentForm.value)
        return
      }

      // 调用 Tauri 后端接口，将文件拷贝到 Libraries 目录下
      await invoke('add_repo_import_file', {
        library: currentForm.value.library,
        filePath: currentForm.value.filePath,
      })

      const importfileList = {
        original_filesize: currentForm.value.importFileSize,
        create_time: currentForm.value.importFileCreateTime,
        original_filepath: currentForm.value.filePath,
        original_filename: currentForm.value.importFileName,
        original_filetype: currentForm.value.type,
        library: currentForm.value.library,
        description: currentForm.value.description,
        tags: currentForm.value.tags.split(','), // 将字符串转换为数组
        html_filename: `${currentForm.value.importFileName}.html`,
        html_filepath: `path/to/html/${currentForm.value.importFileName}.html`,
        html_change_time: new Date().toISOString(),
        markdown_filename: `${currentForm.value.importFileName}.md`,
        markdown_filepath: `path/to/markdown/${currentForm.value.importFileName}.md`,
        markdown_change_time: new Date().toISOString(),
        pdf_filename: `${currentForm.value.importFileName}.pdf`,
        pdf_filepath: `path/to/pdf/${currentForm.value.importFileName}.pdf`,
        pdf_change_time: new Date().toISOString(),
        word_filename: `${currentForm.value.importFileName}.docx`,
        word_filepath: `path/to/word/${currentForm.value.importFileName}.docx`,
        word_change_time: new Date().toISOString(),
        epub_filename: `${currentForm.value.importFileName}.epub`,
        epub_filepath: `path/to/epub/${currentForm.value.importFileName}.epub`,
        epub_change_time: new Date().toISOString(),
        txt_filename: `${currentForm.value.importFileName}.txt`,
        txt_filepath: `path/to/txt/${currentForm.value.importFileName}.txt`,
        txt_change_time: new Date().toISOString(),
        container_filename: `${currentForm.value.importFileName}.zip`,
        container_filepath: `path/to/container/${currentForm.value.importFileName}.zip`,
        container_change_time: new Date().toISOString(),
      };

      console.log('importfileList:', importfileList); // 调试日志
      // 调用 Tauri 后端接口，将 importfileList 写入数据库
      try {
        const fileId = await invoke('add_imported_file', {
          tableName: 'imported_files', // 确保参数名与后端一致
          importedFileInfo: importfileList, // 确保参数名与后端一致
        });
        console.log('File info successfully added to database, id: ', fileId);

        // 添加导入的文件
        store.addImportedFile({
          id: fileId,
          importFileName: currentForm.value.importFileName,
          description: currentForm.value.description,
          tags: currentForm.value.tags,
          type: currentForm.value.type,
          library: currentForm.value.library,
          importFileCreateTime: currentForm.value.importFileCreateTime,
          importFileSize: currentForm.value.importFileSize,
          convetToHtmlFilename: `${currentForm.value.importFileName}.html`,
          convetToMarkdownFilename: `${currentForm.value.importFileName}.md`,
        })
      } catch (error) {
        console.error('Error adding info into table imported_files database:', error);
      }

    } else if (store.currentDatabase === 'NotesDb') {
      // if (!currentForm.value.fromImportedFileLibrary || !currentForm.value.noteFileName) {
      //   console.error('Library or noteFileName path is missing: ', currentForm.value)
      //   return
      // }

      const importNoteList = {
        description: currentForm.value.description,
        note_type: 'markdown', // 默认笔记类型
        create_time: currentForm.value.noteFileCreateTime,
        update_time: currentForm.value.noteFileUpdateTime || currentForm.value.noteFileCreateTime,
        library: currentForm.value.fromImportedFileLibrary,
        filename: `${currentForm.value.noteFileName}.md`,
        from_filename: currentForm.value.fromImportedFile,
        parent_note: currentForm.value.parent_note || null, // 父笔记 ID
        tags: currentForm.value.tags.split(','), // 将字符串转换为数组
        child_notes: [], // 子笔记的 ID 列表
        link_to_notes: [], // 链接笔记的 ID 列表
        link_to_files: [], // 链接文件的 ID 列表
      };

      console.log('importNoteList:', importNoteList); // 调试日志
      // 调用 Tauri 后端接口，将 importNoteList 写入数据库
      try {
        const noteId = await invoke('add_note', {
          tableName: 'notes', // 确保参数名与后端一致
          note: importNoteList, // 确保参数名与后端一致
        });
        console.log('Note info successfully added to database, id: ', noteId);
        // 添加导入的笔记
        store.addImportedNote({
          id: noteId,
          description: currentForm.value.description,
          noteFileName: `${currentForm.value.noteFileName}.md`,
          type: 'markdown',
          tags: currentForm.value.tags,
          fromImportedFileLibrary: currentForm.value.fromImportedFileLibrary,
          noteFileCreateTime: currentForm.value.noteFileCreateTime,
          noteFileUpdateTime: currentForm.value.noteFileUpdateTime || currentForm.value.noteFileCreateTime,
        })
      } catch (error) {
        console.error('Error adding info into table notes database:', error);
      }
    }

    // 关闭对话框并清空表单
    isAddFileModalVisible.value = false
    resetForm()
  } catch (error) {
    console.error('Error adding file:', error)
  }
}

// 清空表单
const resetForm = () => {
  if (store.currentDatabase === 'importFileDB') {
    newFileForm.value = {
      id: 0,
      importFileName: '',
      description: '',
      tags: '',
      type: '',
      library: '', // 所属库名字
      importFileCreateTime: '',
      importFileSize: 0,
      convetToHtmlFilename: '',
      importFileSize: '',
    }
  } else {
    newNoteForm.value = {
      id: 0,
      description: '',
      noteFileName: '',
      tags: '', // 所属库名字
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
const handleOpen = (row) => {
  console.log('打开', row)
}

// 方法：删除文件
const handleDelete = (row) => {
  console.log('删除', row)
}

// 方法：编辑文件
const handleEdit = (row) => {
  console.log('编辑', row)
}

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
            onUpdateValue(v) {
              row.description = v
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
            onUpdateValue(v) {
              row.noteFileName = v
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
        key: 'tags',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.tags,
            onUpdateValue(v) {
              row.tags = v
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
          }, '笔记归属导入文件'),
        key: 'fromImportedFile',
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
          }, '导入文件所属库'),
        key: 'fromImportedFileLibrary',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.fromImportedFileLibrary,
            onUpdateValue(v) {
              row.fromImportedFileLibrary = v
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
            onUpdateValue(v) {
              row.importFileName = v
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
            onUpdateValue(v) {
              row.description = v
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
        key: 'tags',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.tags,
            onUpdateValue(v) {
              row.tags = v
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
            onUpdateValue(v) {
              row.library = v
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
})

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

// 组件挂载时获取默认数据
onMounted(async () => {
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
</style>