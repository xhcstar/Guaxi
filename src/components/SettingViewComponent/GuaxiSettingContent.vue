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

.setting-card :deep(.n-input .n-input-wrapper),
.setting-card :deep(.n-input-number .n-input .n-input-wrapper) {
  background-color: var(--guaxi-panel-bg) !important;
}

.setting-card :deep(.n-input .n-input__border),
.setting-card :deep(.n-input .n-input__state-border),
.setting-card :deep(.n-input-number .n-input .n-input__border),
.setting-card :deep(.n-input-number .n-input .n-input__state-border) {
  border-color: var(--guaxi-border) !important;
}

.setting-card :deep(.n-input__input-el),
.setting-card :deep(.n-input__textarea-el),
.setting-card :deep(.n-input-number .n-input__input-el) {
  color: var(--guaxi-text-primary) !important;
}

.setting-card :deep(.n-input__input-el::placeholder),
.setting-card :deep(.n-input__textarea-el::placeholder),
.setting-card :deep(.n-input-number .n-input__input-el::placeholder) {
  color: var(--guaxi-text-secondary) !important;
}

.setting-card :deep(.n-input__prefix),
.setting-card :deep(.n-input__suffix) {
  color: var(--guaxi-text-secondary) !important;
}
</style>