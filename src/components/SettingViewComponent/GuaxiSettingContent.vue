<template>
  <div class="setting-content">
    <n-card class="setting-card">
      <keep-alive>
        <component :is="currentComponent" />
      </keep-alive>
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
import GuaxiManual from './GuaxiManual.vue';
import TaskManagerContent from './TaskManagerContent.vue';
import CognitionSetting from './CognitionSetting.vue';
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
    case 'taskmanager':
      return TaskManagerContent;
    case 'cognition':
      return CognitionSetting;
    case 'manual':
      return GuaxiManual;
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
  scrollbar-width: thin;
  scrollbar-color: rgba(0, 0, 0, 0.26) transparent;
}

.setting-card :deep(.n-card__content::-webkit-scrollbar) {
  width: 8px;
  height: 8px;
}

.setting-card :deep(.n-card__content::-webkit-scrollbar-thumb) {
  background-color: rgba(0, 0, 0, 0.22);
  border-radius: 999px;
}

.setting-card :deep(.n-card__content::-webkit-scrollbar-thumb:hover) {
  background-color: rgba(0, 0, 0, 0.34);
}

.setting-card :deep(.n-card__content::-webkit-scrollbar-track) {
  background-color: transparent;
}

:global(:root[data-guaxi-theme='dark']) .setting-card :deep(.n-card__content) {
  scrollbar-color: rgba(255, 255, 255, 0.22) transparent;
}

:global(:root[data-guaxi-theme='dark']) .setting-card :deep(.n-card__content::-webkit-scrollbar-thumb) {
  background-color: rgba(255, 255, 255, 0.18);
}

:global(:root[data-guaxi-theme='dark']) .setting-card :deep(.n-card__content::-webkit-scrollbar-thumb:hover) {
  background-color: rgba(255, 255, 255, 0.26);
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