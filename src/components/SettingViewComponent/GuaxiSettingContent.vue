<template>
  <div class="setting-content" :class="{ 'is-manual': isManual }">
    <n-card v-if="!isManual" class="setting-card">
      <keep-alive>
        <component :is="currentComponent" />
      </keep-alive>
    </n-card>
    <!-- 手册是整页文档，不再套一层卡片：保留 24px 内边距会让它看起来
         “嵌在中间”。这里直接铺满设置内容区，由手册自己管理滚动。 -->
    <keep-alive v-else>
      <component :is="currentComponent" />
    </keep-alive>
  </div>
</template>

<script setup>
import { computed } from 'vue';
import { NCard } from 'naive-ui';
import NoteRepoContent from './NoteRepoContent.vue';
import AppearanceContent from './AppearanceContent.vue';
import EditorContent from './EditorContent.vue';
import VoiceContent from './VoiceContent.vue';
import TerminalContent from './TerminalContent.vue';
import TransferContent from './TransferContent.vue';
import PluginContent from './PluginContent.vue';
import MaintenanceContent from './MaintenanceContent.vue';
import ServerContent from './ServerContent.vue';
import AboutContent from './AboutContent.vue';
import LLMModalSetting from './LLMModalSetting.vue';
import GuaxiManual from './GuaxiManual.vue';
import TaskManagerContent from './TaskManagerContent.vue';
import CognitionSetting from './CognitionSetting.vue';
import FeedsSetting from './FeedsSetting.vue';
import { useSettingsStore } from '../../stores/settingsStore';

const settingsStore = useSettingsStore();
const isManual = computed(() => settingsStore.selectedMenu === 'manual');

// 根据选中的菜单项动态加载组件
const currentComponent = computed(() => {
  switch (settingsStore.selectedMenu) {
    case 'noterepo':
      return NoteRepoContent;
    case 'feeds':
      return FeedsSetting;
    case 'appearance':
      return AppearanceContent;
    case 'editor':
      return EditorContent;
    case 'voice':
      return VoiceContent;
    case 'terminal':
      return TerminalContent;
    case 'transfer':
      return TransferContent;
    case 'plugins':
      return PluginContent;
    case 'maintenance':
      return MaintenanceContent;
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

.setting-content.is-manual {
  display: flex;
  flex-direction: column;
  height: calc(100vh - 30px);
  max-height: calc(100vh - 30px);
  min-height: 0;
  overflow: hidden;
}

.setting-content.is-manual > .manual-shell {
  flex: 1 1 0;
  min-height: 0;
  width: 100%;
  height: 100%;
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

<style src="./settingsPageShared.css"></style>
