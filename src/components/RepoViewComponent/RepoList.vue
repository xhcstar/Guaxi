<template>
  <n-input round size="small" placeholder="搜索" />
  <n-menu :options="menuOptions" @update:value="handleUpdateValue" />
</template>

<script setup >
import { ServerOutline } from '@vicons/ionicons5' // 引入服务器图标
import { NIcon, useMessage } from 'naive-ui'
import { h, ref, onMounted } from 'vue'
import { repoStore } from '../../stores/repoStore'
import { invoke } from '@tauri-apps/api/core';


const message = useMessage() // 使用消息提示
const store = repoStore() // 使用 Pinia store
// 渲染图标
function renderIcon(icon) {
  return () => h(NIcon, null, { default: () => h(icon) })
}

const menuOptions = ref([]);

// 处理菜单项点击事件
function handleUpdateValue(key, item) {
  message.info(`切换到数据库: ${key}`)
  store.currentDatabase = key // 更新当前数据库
  store.fetchFiles(store.currentDatabase) // 根据 key 获取对应的数据
}

onMounted(async () => {
  try {
    const repos = await invoke('get_knowledge_repos');
    menuOptions.value = repos.map(repo => ({
      label: repo,
      key: repo,
      icon: renderIcon(ServerOutline)
    }));
  } catch (error) {
    console.error('获取知识库失败:', error);
  }
});

</script>