<template>
  <n-input round size="small" placeholder="搜索" />
  <n-menu :options="menuOptions" @update:value="handleUpdateValue" />
</template>

<script setup >
import { ServerOutline } from '@vicons/ionicons5' // 引入服务器图标
import { NIcon, useMessage } from 'naive-ui'
import { h, ref } from 'vue'
import { repoStore } from '../../stores/repoStore'

// 渲染图标
function renderIcon(icon) {
  return () => h(NIcon, null, { default: () => h(icon) })
}

// 菜单选项
const menuOptions = [
  {
    label: '文件数据库',
    key: 'importFileDB',
    icon: renderIcon(ServerOutline)
  },
  {
    label: '笔记数据库',
    key: 'NotesDb',
    icon: renderIcon(ServerOutline)
  },
  {
    label: '数据库 3',
    key: 'db3',
    icon: renderIcon(ServerOutline)
  }
]

const message = useMessage() // 使用消息提示
const store = repoStore() // 使用 Pinia store

// 处理菜单项点击事件
function handleUpdateValue(key, item) {
  message.info(`切换到数据库: ${key}`)
  store.currentDatabase = key // 更新当前数据库
  store.fetchFiles(store.currentDatabase) // 根据 key 获取对应的数据
}
</script>