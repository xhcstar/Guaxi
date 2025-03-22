<template>
  <n-space vertical :size="12" class="container">
    <n-space justify="space-between" align="center" class="toolbar">
      <n-space class="actions">
        <n-button @click="sortTitle" class="action-button">按标题排序（升序）</n-button>
        <n-button @click="filterLibrary" class="action-button">过滤所属库（Library1）</n-button>
        <n-button @click="clearFilters" class="action-button">清除过滤器</n-button>
        <n-button @click="clearSorter" class="action-button">清除排序</n-button>
      </n-space>
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
    <n-form :model="currentForm" label-placement="left" label-width="auto">
      <n-form-item label="标题">
        <n-input v-model:value="currentForm.title" placeholder="请输入标题" />
      </n-form-item>
      <n-form-item label="描述">
        <n-input v-model:value="currentForm.description" placeholder="请输入描述" />
      </n-form-item>
      <n-form-item label="标签">
        <n-input v-model:value="currentForm.tags" placeholder="请输入标签" />
      </n-form-item>
      <n-form-item label="所属库">
        <n-input v-model:value="currentForm.library" placeholder="请输入所属库" />
      </n-form-item>
      <n-form-item label="文件路径">
        <n-input v-model:value="currentForm.filePath" placeholder="请输入文件路径">
          <template #suffix>
            <n-icon :component="FolderAdd20Regular" @click="openFileBrowser" />
          </template>
        </n-input>
      </n-form-item>
      <n-form-item label="文件名">
        <n-input v-model:value="currentForm.fileName" disabled />
      </n-form-item>
      <n-form-item label="文件类型">
        <n-input v-model:value="currentForm.fileType" disabled />
      </n-form-item>
      <n-form-item label="创建时间">
        <n-input v-model:value="currentForm.createTime" disabled />
      </n-form-item>
      <n-form-item v-if="store.currentDatabase === 'importFileDB'" label="文件大小">
        <n-input v-model:value="currentForm.fileSize" disabled />
      </n-form-item>
      <n-form-item v-if="store.currentDatabase === 'NotesDb'" label="更新时间">
        <n-input v-model:value="currentForm.updateTime" placeholder="请输入更新时间" />
      </n-form-item>
      <n-form-item v-if="store.currentDatabase === 'NotesDb'" label="父笔记">
        <n-input v-model:value="currentForm.parentNote" placeholder="请输入父笔记 ID" />
      </n-form-item>
      <n-form-item v-if="store.currentDatabase === 'NotesDb'" label="子笔记">
        <n-input v-model:value="currentForm.childNotes" placeholder="请输入子笔记 ID 列表" />
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
import { NIcon, NButton, NDropdown, NGradientText } from 'naive-ui'
import ShowOrEdit from './ShowOrEdit.vue'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'

// 引入 Pinia store
const store = repoStore()

// 添加文件对话框状态
const isAddFileModalVisible = ref(false)
const checkedRowKeysRef = ref()
// 表格实例
const tableRef = ref(null)
// 分页配置
const pagination = ref({ pageSize: 10 })

// 用于 importFileDB 库的表单
const newFileForm = ref({
  title: '',
  description: '',
  tags: '',
  library: '', // 所属库名字
  filePath: '',
  fileName: '',
  fileType: '',
  createTime: '',
  fileSize: '',
})

// 用于 NotesDb 库的表单
const newNoteForm = ref({
  title: '',
  description: '',
  tags: '',
  library: '', // 所属库名字
  filePath: '',
  fileName: '',
  fileType: '',
  createTime: '',
  updateTime: '', // 笔记特有的字段
  parentNote: null, // 笔记特有的字段
  childNotes: [], // 笔记特有的字段
})

// 当前表单（根据数据库类型动态切换）
const currentForm = computed(() => {
  return store.currentDatabase === 'importFileDB' ? newFileForm : newNoteForm
})

// 打开文件浏览器并获取文件信息
const openFileBrowser = async () => {
  try {
    const filePath = await open({
      filters: [{ name: 'All Files', extensions: ['*'] }],
    })
    if (filePath) {
      currentForm.value.filePath = filePath
      const fileInfo = await invoke('get_file_info', { filePath })
      currentForm.value.fileName = fileInfo.file_name
      currentForm.value.fileType = fileInfo.file_type
      currentForm.value.createTime = fileInfo.create_time
      currentForm.value.fileSize = fileInfo.file_size
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
    if (!currentForm.value.library || !currentForm.value.filePath) {
      console.error('Library or file path is missing')
      return
    }
    // 调用 Tauri 后端接口，将文件拷贝到 Libraries 目录下
    await invoke('add_repo_import_file', {
      library: currentForm.value.library,
      filePath: currentForm.value.filePath,
    })

    // 根据当前数据库类型调用不同的接口
    if (store.currentDatabase === 'importFileDB') {
      // 添加导入的文件
      store.addImportedFile({
        id: store.files.length + 1, // 自动生成 ID
        title: currentForm.value.title,
        type: currentForm.value.fileType,
        create_time: currentForm.value.createTime,
        size: parseInt(currentForm.value.fileSize), // 假设文件大小是数字
        path: currentForm.value.filePath,
        library: currentForm.value.library,
        description: currentForm.value.description,
        tags: currentForm.value.tags,
        html_file_name: `${currentForm.value.fileName}.html`,
        markdown_file_name: `${currentForm.value.fileName}.md`,
      })
    } else if (store.currentDatabase === 'NotesDb') {
      // 添加导入的笔记
      store.addImportedNote({
        id: store.notes.length + 1, // 自动生成 ID
        title: currentForm.value.title,
        description: currentForm.value.description,
        type: 'markdown', // 假设笔记类型为 markdown
        tags: currentForm.value.tags,
        create_time: currentForm.value.createTime,
        update_time: currentForm.value.updateTime || currentForm.value.createTime, // 默认与创建时间相同
        library: currentForm.value.library,
        file: `${currentForm.value.fileName}.md`,
        parent_note: currentForm.value.parentNote,
        child_notes: currentForm.value.childNotes,
      })
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
      title: '',
      description: '',
      tags: '',
      library: '',
      filePath: '',
      fileName: '',
      fileType: '',
      createTime: '',
      fileSize: '',
    }
  } else {
    newNoteForm.value = {
      title: '',
      description: '',
      tags: '',
      library: '',
      filePath: '',
      fileName: '',
      fileType: '',
      createTime: '',
      updateTime: '',
      parentNote: null,
      childNotes: [],
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
    // onClick: () => {
    //   handleEdit(row)
    // },
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
          }, '笔记标题'),
        key: 'title',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.title,
            onUpdateValue(v) {
              row.title = v
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
        key: 'file',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.file,
            onUpdateValue(v) {
              row.file = v
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
          }, '笔记类型'),
        key: 'type',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.type,
            onUpdateValue(v) {
              row.type = v
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
          }, '所属库'),
        key: 'library',
        resizable: true,
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
          }, '创建时间'),
        key: 'create_time',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.create_time,
            onUpdateValue(v) {
              row.create_time = v
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
          }, '修改时间'),
        key: 'update_time',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.update_time,
            onUpdateValue(v) {
              row.update_time = v
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
          }, '父级笔记'),
        key: 'parent_note',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.parent_note,
            onUpdateValue(v) {
              row.parent_note = v
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
          }, '子级笔记'),
        key: 'child_notes',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.child_notes,
            onUpdateValue(v) {
              row.child_notes = v
            },
          })
        },
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
        key: 'title',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.title,
            onUpdateValue(v) {
              row.title = v
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
        render(row) {
          return h(ShowOrEdit, {
            value: row.type,
            onUpdateValue(v) {
              row.type = v
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
        key: 'create_time',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.create_time,
            onUpdateValue(v) {
              row.create_time = v
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
          }, '文件大小'),
        key: 'size',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.size,
            onUpdateValue(v) {
              row.size = v
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
          }, 'HTML文件名'),
        key: 'html_file_name',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.html_file_name,
            onUpdateValue(v) {
              row.html_file_name = v
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
          }, 'Markdown文件名'),
        key: 'markdown_file_name',
        resizable: true,
        render(row) {
          return h(ShowOrEdit, {
            value: row.markdown_file_name,
            onUpdateValue(v) {
              row.markdown_file_name = v
            },
          })
        },
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
  padding: 16px;
}

.toolbar {
  margin-bottom: 16px;
}

.actions {
  display: flex;
  gap: 8px;
}

.add-path-button {
  position: absolute;
  top: 20px;
  right: 20px;
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