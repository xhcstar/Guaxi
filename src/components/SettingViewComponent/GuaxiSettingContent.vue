<template>
  <div class="setting-content">
    <n-card class="setting-card">
      <component :is="currentComponent" />
    </n-card>
  </div>
</template>

<script setup>
import { computed } from 'vue';
import { NCard } from 'naive-ui';
import NoteRepoContent from './NoteRepoContent.vue';
import PreferenceContent from './PreferenceContent.vue';
import ServerContent from './ServerContent.vue';
import AboutContent from './AboutContent.vue';
import LLMModalSetting from './LLMModalSetting.vue';
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
    case 'llm':
      return LLMModalSetting;
    default:
      return null;
  }
});
</script>

<style scoped>
.setting-content {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  overflow: hidden;
}

.setting-card {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background-color: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  color: var(--guaxi-text-primary);
}

.setting-card :deep(.n-card) {
  background-color: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  color: var(--guaxi-text-primary);
}

.setting-card :deep(.n-card__header) {
  color: var(--guaxi-text-primary);
}

.setting-card :deep(.n-card__content) {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  color: var(--guaxi-text-primary);
}
</style>