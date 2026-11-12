<template>
  <n-menu
    :collapsed-width="64"
    :icon-size="18"
    :collapsed-icon-size="18"
    :root-indent="10"
    :indent="20"
    :theme-overrides="menuThemeOverrides"
    :options="menuOptions"
    v-model:value="selectedMenu"
    @update:value="handleMenuSelect"
    class="custom-menu"
  />
</template>

<script setup>
import { ref, h } from 'vue';
import { NMenu, NIcon } from 'naive-ui';
import { InformationCircleOutline, BookOutline } from '@vicons/ionicons5';
import {
  FolderAdd20Regular,
  Image28Regular,
  Code24Regular,
  WindowConsole20Regular,
  Server24Regular,
  Key24Regular,
  TaskListSquareLtr20Regular,
  BroadActivityFeed20Regular,
  BrainCircuit20Regular,
  Mic24Regular,
  ArrowSwap24Regular,
  PuzzlePiece24Regular,
  Wrench24Regular,
} from '@vicons/fluent';
import { useSettingsStore } from '../../stores/settingsStore';

const settingsStore = useSettingsStore();
const selectedMenu = ref(settingsStore.selectedMenu);

// 菜单主题变量统一走 theme-overrides，不去逐个 !important 压 naive 的内部类。
// 字号比正文小半档、行高压到 34px：分组标题是「章节标签」，功能项才是可点的主层级，
// 两者靠字号、字重、字距和一道细分隔线区分，而不是两行一样大的文字。
const menuThemeOverrides = {
  itemHeight: '34px',
  fontSize: '13.5px',
  itemColorHover: 'var(--guaxi-hover-bg)',
  itemColorActive: 'rgba(45, 212, 191, 0.13)',
  itemColorActiveHover: 'rgba(45, 212, 191, 0.18)',
  itemTextColor: 'var(--guaxi-text-secondary)',
  itemTextColorHover: 'var(--guaxi-text-primary)',
  itemTextColorActive: 'var(--guaxi-text-primary)',
  itemTextColorActiveHover: 'var(--guaxi-text-primary)',
  itemIconColor: 'var(--guaxi-text-secondary)',
  itemIconColorHover: 'var(--guaxi-text-primary)',
  itemIconColorActive: '#0d9488',
  itemIconColorActiveHover: '#0d9488',
};

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
        icon: () => h(NIcon, { component: FolderAdd20Regular, size: 18 }),
      },
      {
        label: '任务管理',
        key: 'taskmanager',
        icon: () => h(NIcon, { component: TaskListSquareLtr20Regular, size: 18 }),
      },
      {
        label: '知识采集',
        key: 'feeds',
        icon: () => h(NIcon, { component: BroadActivityFeed20Regular, size: 18 }),
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
        key: 'appearance',
        icon: () => h(NIcon, { component: Image28Regular, size: 18 }),
      },
      {
        label: '编辑器',
        key: 'editor',
        icon: () => h(NIcon, { component: Code24Regular, size: 18 }),
      },
      {
        label: '终端',
        key: 'terminal',
        icon: () => h(NIcon, { component: WindowConsole20Regular, size: 18 }),
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
        icon: () => h(NIcon, { component: Key24Regular, size: 18 }),
      },
      {
        label: '语音能力',
        key: 'voice',
        icon: () => h(NIcon, { component: Mic24Regular, size: 18 }),
      },
      {
        label: '认知层',
        key: 'cognition',
        icon: () => h(NIcon, { component: BrainCircuit20Regular, size: 18 }),
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
        icon: () => h(NIcon, { component: Server24Regular, size: 18 }),
      },
      {
        label: '文件传输',
        key: 'transfer',
        icon: () => h(NIcon, { component: ArrowSwap24Regular, size: 18 }),
      },
      {
        label: '插件',
        key: 'plugins',
        icon: () => h(NIcon, { component: PuzzlePiece24Regular, size: 18 }),
      },
      {
        label: '系统维护',
        key: 'maintenance',
        icon: () => h(NIcon, { component: Wrench24Regular, size: 18 }),
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
        icon: () => h(NIcon, { component: BookOutline, size: 18 }),
      },
      {
        label: '关于',
        key: 'about',
        icon: () => h(NIcon, { component: InformationCircleOutline, size: 18 }),
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
.custom-menu {
  padding: 4px 8px 12px;
}

/* 分组标题是「章节签」：小字号 + 大字距的灰字，右侧接一道细线把章节收住。
   naive 的分组标题是 flex 行，::after 会成为一个 flex 子项，正好当那条线。 */
.custom-menu :deep(.n-menu-item-group-title) {
  display: flex;
  align-items: center;
  gap: 8px;
  height: auto;
  padding: 16px 6px 4px;
  font-size: 11px;
  line-height: 1.4;
  font-weight: 600;
  letter-spacing: 0.09em;
  text-transform: uppercase;
  color: var(--guaxi-text-secondary);
  opacity: 1;
}

.custom-menu :deep(.n-menu-item-group-title)::after {
  content: '';
  flex: 1 1 auto;
  height: 1px;
  background: var(--guaxi-border-weak);
}

.custom-menu :deep(.n-menu-item-group:first-child .n-menu-item-group-title) {
  padding-top: 4px;
}

/* naive 给每个菜单项固定 6px 上间距，34px 行高下会显得松散；收到 2px。
   这些类名都渲染在子组件里，没有本组件的 scope 属性，必须走 :deep。 */
.custom-menu :deep(.n-menu-item) {
  margin-top: 2px;
}

.custom-menu :deep(.n-menu-item-content) {
  padding-right: 8px;
}

.custom-menu :deep(.n-menu-item-content::before) {
  left: 0;
  right: 0;
  border-radius: 6px;
}

.custom-menu :deep(.n-menu-item-content-header) {
  font-size: 13.5px;
  font-weight: 500;
  letter-spacing: 0.005em;
}

.custom-menu :deep(.n-menu-item-content--selected .n-menu-item-content-header) {
  font-weight: 600;
}

.custom-menu :deep(.n-menu-item-content__icon) {
  transition: color 0.2s ease;
}

/* 深色主题下主题色要提亮一档，否则选中的图标压在 #21252b 上看不清。 */
:global(:root[data-guaxi-theme='dark']) .custom-menu :deep(.n-menu-item-content__icon) {
  color: var(--guaxi-text-secondary);
}

:global(:root[data-guaxi-theme='dark']) .custom-menu :deep(.n-menu-item-content--selected .n-menu-item-content__icon) {
  color: #2dd4bf;
}
</style>
