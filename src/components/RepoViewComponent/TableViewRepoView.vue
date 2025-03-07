<template>
  <n-space vertical :size="12">
    <n-space>
      <n-button @click="sortTitle">按标题排序（升序）</n-button>
      <n-button @click="filterLibrary">过滤所属库（Library1）</n-button>
      <n-button @click="clearFilters">清除过滤器</n-button>
      <n-button @click="clearSorter">清除排序</n-button>
    </n-space>
    <n-data-table ref="table" :columns="columns" :data="currentData" :pagination="pagination" />
  </n-space>
</template>

<script setup>
import { ref, onMounted, computed } from 'vue'
import { repoStore } from '../../stores/repoStore'

// 引入 Pinia store
const store = repoStore()

// 根据当前数据库类型选择数据
const currentData = computed(() => {
  return store.currentDatabase === 'db2' ? store.notes : store.files;
})

// 表格列配置
const columns = computed(() => {
  if (store.currentDatabase === 'db2') {
    // 数据库 2 的列配置（笔记信息）
    return [
      {
        title: '笔记标题',
        key: 'title',
        defaultSortOrder: 'ascend',
        sorter: 'default',
      },
      {
        title: '笔记描述',
        key: 'description',
      },
      {
        title: '笔记类型',
        key: 'type',
      },
      {
        title: '笔记标签',
        key: 'tags',
      },
      {
        title: '创建时间',
        key: 'create_time',
      },
      {
        title: '修改时间',
        key: 'update_time',
      },
      {
        title: '所属库',
        key: 'library',
      },
      {
        title: '所属文件',
        key: 'file',
      },
      {
        title: '父级笔记',
        key: 'parent_note',
      },
      {
        title: '子级笔记',
        key: 'child_notes',
      },
    ];
  } else {
    // 数据库 1 的列配置（文件信息）
    return [
      {
        title: '标题',
        key: 'title',
        defaultSortOrder: 'ascend',
        sorter: 'default',
      },
      {
        title: '文件类型',
        key: 'type',
      },
      {
        title: '入库时间',
        key: 'create_time',
      },
      {
        title: '文件大小（字节）',
        key: 'size',
      },
      {
        title: '所属库',
        key: 'library',
        filterOptions: [
          { label: 'Library1', value: 'Library1' },
          { label: 'Library2', value: 'Library2' },
        ],
        filter(value, row) {
          return row.library === value;
        },
      },
      {
        title: '文件描述',
        key: 'description',
      },
      {
        title: '文件标签',
        key: 'tags',
      },
      {
        title: 'HTML 文件名',
        key: 'html_file_name',
      },
      {
        title: 'Markdown 文件名',
        key: 'markdown_file_name',
      },
    ];
  }
})

// 表格实例
const tableRef = ref(null)

// 分页配置
const pagination = ref({ pageSize: 10 })

// 方法：按标题排序
const sortTitle = () => {
  tableRef.value.sort('title', 'ascend')
}

// 方法：过滤所属库
const filterLibrary = () => {
  tableRef.value.filter({
    library: ['Library1']
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

// 组件挂载时获取默认数据
onMounted(async () => {
  await store.fetchFiles('db1') // 默认加载数据库 1 的数据
})
</script>