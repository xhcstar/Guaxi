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
import { InformationCircleOutline, BookOutline } from '@vicons/ionicons5';
import { FolderAdd20Regular, Image28Regular, Server24Regular, Key24Regular, TaskListSquareLtr20Regular, BrainCircuit20Regular } from '@vicons/fluent';
import { useSettingsStore } from '../../stores/settingsStore';

const settingsStore = useSettingsStore();
const selectedMenu = ref(settingsStore.selectedMenu);

// 侧边栏菜单选项:按主题分组归类(分组只做视觉聚合,各功能项的 key 保持不变,
// 故 handleMenuSelect 与 GuaxiSettingContent 的内容切换逻辑完全不受影响)。
const menuOptions = [
  {
    type: 'group',
    label: '内容与库',
    key: 'grp-content',
    children: [
      {
        label: '笔记库',
        key: 'noterepo',
        icon: () => h(NIcon, { component: FolderAdd20Regular, size: 24 }),
      },
      {
        label: '任务管理',
        key: 'taskmanager',
        icon: () => h(NIcon, { component: TaskListSquareLtr20Regular, size: 24 }),
      },
    ],
  },
  {
    type: 'group',
    label: '外观与体验',
    key: 'grp-appearance',
    children: [
      {
        label: '外观',
        key: 'preference',
        icon: () => h(NIcon, { component: Image28Regular, size: 24 }),
      },
    ],
  },
  {
    type: 'group',
    label: 'AI 与模型',
    key: 'grp-ai',
    children: [
      {
        label: '大模型',
        key: 'llm',
        icon: () => h(NIcon, { component: Key24Regular, size: 24 }),
      },
      {
        label: '认知层',
        key: 'cognition',
        icon: () => h(NIcon, { component: BrainCircuit20Regular, size: 24 }),
      },
    ],
  },
  {
    type: 'group',
    label: '系统与服务',
    key: 'grp-system',
    children: [
      {
        label: '服务器',
        key: 'server',
        icon: () => h(NIcon, { component: Server24Regular, size: 24 }),
      },
    ],
  },
  {
    type: 'group',
    label: '帮助与关于',
    key: 'grp-help',
    children: [
      {
        label: '用户手册',
        key: 'manual',
        icon: () => h(NIcon, { component: BookOutline, size: 24 }),
      },
      {
        label: '关于',
        key: 'about',
        icon: () => h(NIcon, { component: InformationCircleOutline, size: 24 }),
      },
    ],
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

/* 分组标题:作为顶层配置分类标题,字号更醒目、字重更足 */
.custom-menu :deep(.n-menu-item-group-title) {
  height: auto;
  font-size: 14px;
  line-height: 1.5;
  font-weight: 700;
  letter-spacing: 0.5px;
  text-transform: none;
  color: var(--guaxi-text-primary);
  opacity: 0.92;
  padding-top: 14px;
  padding-bottom: 4px;
}

.custom-menu .n-menu-item {
  border-radius: 8px; /* 为菜单项添加圆角 */
  transition: background-color 0.3s ease; /* 添加过渡效果 */
}

.custom-menu .n-menu-item:hover {
  background-color: var(--guaxi-header-bg);
}

.custom-menu .n-menu-item.n-menu-item--selected {
  background-color: rgba(45, 212, 191, 0.12);
  color: var(--n-color-primary);
}

.custom-menu :deep(.n-menu-item-content__icon) {
  color: var(--guaxi-text-secondary);
  transition: color 0.2s ease;
}

.custom-menu :deep(.n-menu-item:hover .n-menu-item-content__icon),
.custom-menu :deep(.n-menu-item--selected .n-menu-item-content__icon) {
  color: var(--n-color-primary);
}

:global(:root[data-guaxi-theme='dark']) .custom-menu :deep(.n-menu-item-content__icon) {
  color: var(--guaxi-text-primary);
}
</style>