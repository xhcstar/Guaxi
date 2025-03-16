<template>
  <n-card>
    <component :is="currentComponent" />
  </n-card>
</template>

<script setup>
import { computed } from 'vue';
import { NCard } from 'naive-ui';
import NoteRepoContent from './NoteRepoContent.vue';
import PreferenceContent from './PreferenceContent.vue';
import ServerContent from './ServerContent.vue';
import AboutContent from './AboutContent.vue';
import { useSettingsStore } from '../../stores/settingsStore';

const settingsStore = useSettingsStore();

// 根据选中的菜单项动态加载组件
const currentComponent = computed(() => {
  switch (settingsStore.selectedMenu) {
    case 'noterepo':
      return NoteRepoContent;
    case 'preference':
      return PreferenceContent;
    case 'server':
      return ServerContent;
    case 'about':
      return AboutContent;
    default:
      return null;
  }
});
</script>