<template>
  <n-menu
    :collapsed-width="64"
    :collapsed-icon-size="24"
    :options="menuOptions"
    v-model:value="selectedMenu"
    @update:value="handleMenuSelect"
    :item-spacing="12" 
    class="custom-menu"
  />
</template>

<script setup>
import { ref, h } from 'vue';
import { NMenu, NIcon } from 'naive-ui';
import { InformationCircleOutline } from '@vicons/ionicons5';
import { FolderAdd20Regular, Image28Regular, Server24Regular } from '@vicons/fluent';
import { useSettingsStore } from '../../stores/settingsStore';

const settingsStore = useSettingsStore();
const selectedMenu = ref(settingsStore.selectedMenu);

// 侧边栏菜单选项
const menuOptions = [
  {
    label: '笔记库',
    key: 'noterepo',
    icon: () => h(NIcon, { component: FolderAdd20Regular, size: 24 }),
  },
  {
    label: '外观',
    key: 'preference',
    icon: () => h(NIcon, { component: Image28Regular, size: 24 }),
  },
  {
    label: '服务器',
    key: 'server',
    icon: () => h(NIcon, { component: Server24Regular, size: 24 }),
  },
  {
    label: '关于',
    key: 'about',
    icon: () => h(NIcon, { component: InformationCircleOutline, size: 24 }),
  },
];

// 处理菜单选择
const handleMenuSelect = (key) => {
  settingsStore.setSelectedMenu(key);
};
</script>

<style scoped>
/* 自定义菜单样式 */
.custom-menu {
  padding: 8px 0; /* 增加菜单的内边距 */
}

.custom-menu .n-menu-item {
  border-radius: 8px; /* 为菜单项添加圆角 */
  transition: background-color 0.3s ease; /* 添加过渡效果 */
}

.custom-menu .n-menu-item:hover {
  background-color: rgba(0, 0, 0, 0.05); /* 鼠标悬停时的背景色 */
}

.custom-menu .n-menu-item.n-menu-item--selected {
  background-color: rgba(0, 120, 212, 0.1); /* 选中菜单项的背景色 */
  color: #0078d4; /* 选中菜单项的文本颜色 */
}
</style>