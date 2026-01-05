<template>
  <div class="repo-sidebar">
    <div class="repo-header">
      <n-text class="repo-title">知识库</n-text>
    </div>
    <n-menu :options="menuOptions" @update:value="handleUpdateValue" class="repo-menu" />
  </div>
</template>

<script setup>
import { BookmarksOutline } from '@vicons/ionicons5'
import { NIcon, NText, useMessage } from 'naive-ui'
import { h, ref, onMounted, computed } from 'vue'
import { repoStore } from '../../stores/repoStore'
import { invoke } from '@tauri-apps/api/core';

const message = useMessage()
const store = repoStore()

// 渲染图标
function renderIcon(icon) {
  return () => h(NIcon, null, { default: () => h(icon) })
}

// 处理菜单项点击事件
function handleUpdateValue(key, item) {
  store.currentDatabase = key
  store.fetchFiles(store.currentDatabase)
}

// 菜单选项使用计算属性
const menuOptions = computed(() => {
  return store.repos.map(repo => ({
    label: repo,
    key: repo,
    icon: renderIcon(BookmarksOutline)
  }));
});

onMounted(async () => {
  await store.fetchRepos()
  if (store.repos.length > 0) {
    store.currentDatabase = store.repos[0]
    await store.fetchFiles(store.currentDatabase)
  }
});
</script>

<style scoped>
.repo-sidebar {
  padding: 4px;
  background-color: var(--guaxi-panel-bg);
  border-right: 1px solid var(--guaxi-border);
  height: 100%;
  display: flex;
  flex-direction: column;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', monospace;
}

.repo-header {
  padding: 12px 16px;
  margin-bottom: 8px;
  border-bottom: 1px solid var(--guaxi-border);
  background-color: var(--guaxi-panel-bg);
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
}

.repo-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
  letter-spacing: 0.5px;
  text-shadow: 0 1px 1px rgba(0, 0, 0, 0.05);
  display: flex;
  align-items: center;
}

.repo-title::before {
  content: "";
  display: inline-block;
  width: 4px;
  height: 16px;
  background-color: var(--guaxi-text-primary);
  margin-right: 10px;
  border-radius: 2px;
}

.repo-menu {
  flex: 1;
  overflow-y: auto;
  background-color: var(--guaxi-panel-bg);
  border-radius: 8px;
  padding: 8px 0;
}

/* 菜单项样式 */
:deep(.n-menu-item) {
  border-radius: 6px;
  margin: 4px 8px;
  transition: all 0.2s ease;
}

:deep(.n-menu-item:hover) {
  background-color: var(--guaxi-header-bg);
}

:deep(.n-menu-item.n-menu-item--selected) {
  background-color: rgba(45, 212, 191, 0.12);
  color: var(--n-color-primary);
  font-weight: 500;
}

:deep(.n-menu-item-content__icon) {
  margin-right: 8px;
  color: var(--guaxi-text-secondary);
}

:deep(.n-menu-item--selected .n-menu-item-content__icon) {
  color: var(--n-color-primary);
}

:deep(.n-menu-item-content) {
  padding: 10px 16px;
  font-size: 15px;
}

:deep(.n-menu-item-content__text) {
  font-weight: 500;
  /* 菜单文字也适当加粗 */
}
</style>